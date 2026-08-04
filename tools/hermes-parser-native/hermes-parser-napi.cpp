/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include <chrono>
#include <cstdlib>
#include <cstring>
#include <memory>
#include <string>
#include <vector>

#include "hermes/AST/Context.h"
#include "hermes/AST/ESTree.h"
#include "hermes/Parser/FlowHelpers.h"
#include "hermes/Parser/JSParser.h"
#include "hermes/Sema/SemContext.h"
#include "hermes/Sema/SemResolve.h"

#include "llvh/ADT/StringRef.h"
#include "llvh/Support/MemoryBuffer.h"

#include "ContainerWriter.h"
#include "HermesParserDiagHandler.h"
#include "HermesParserJSSerializer.h"

#include "node_api.h"

using namespace hermes;

namespace {

// ---------------------------------------------------------------------------
// Optional phase timing
// ---------------------------------------------------------------------------
//
// Attributes the wall time of a `parse()` call to the individual phases that
// make it up, so a benchmark can say where the native side's time actually
// goes instead of treating the whole Node-API call as one opaque number.
//
// Disabled unless the environment variable HERMES_PARSER_NATIVE_PHASE_TIMING
// is set to something other than "0" when the module is loaded. When
// disabled, each probe costs one load of a global bool and a predictable
// branch; when enabled it additionally costs one steady_clock read per phase
// boundary (roughly 20-30ns each on Linux/x86-64 via the vDSO). The env var
// is read once at module init, not per call, so the disabled path never
// touches the environment.
//
// The accumulators are plain globals with no synchronization: a Node-API
// addon instance is confined to the thread of the environment that loaded
// it, and this module registers no async work. Loading the same addon into
// multiple worker threads while phase timing is enabled would race, which is
// acceptable for a measurement-only facility that is off by default.

/// Names of the timed phases, in the order they occur within one call.
/// Kept in sync with \c PhaseTimings::ns by \c kNumPhases.
enum Phase {
  /// Copying the source string out of the JS engine as UTF-8, plus reading
  /// the boolean options off the options object.
  PH_SOURCE_IN,
  /// Building the Context, registering the source buffer and the `@flow`
  /// pragma scan performed when `detectFlow` is set. This is dominated by
  /// fixed per-call cost rather than by anything proportional to input size.
  PH_CONTEXT_INIT,
  /// Constructing the JSParser (and therefore its lexer) for this source.
  PH_PARSER_INIT,
  /// JSParser::parse() itself: the actual parsing.
  PH_PARSE,
  /// Walking the AST and writing the program/position/string buffers.
  PH_SERIALIZE,
  /// Semantic resolution (resolveASTForParser), which the wasm reference
  /// also performs and which can reject otherwise well-formed programs.
  PH_SEMA,
  /// Assembling the three serializer buffers into one contiguous container.
  PH_CONTAINER,
  /// Allocating the result ArrayBuffer and memcpy'ing the container into it,
  /// plus building the `{buffer}` wrapper object.
  PH_COPY_OUT,
  /// Destruction of the parser, the AST arena and the Context, which happens
  /// after the return value has been built but before the call returns.
  PH_TEARDOWN,
  kNumPhases,
};

/// Nanosecond accumulators, one per \c Phase, plus a call counter.
struct PhaseTimings {
  uint64_t ns[kNumPhases] = {};
  uint64_t calls = 0;
};

/// Whether phase timing was requested at module load time.
bool phaseTimingEnabled = false;

/// Accumulated timings. Only written when \c phaseTimingEnabled.
PhaseTimings phaseTimings;

/// \return the current steady-clock reading in nanoseconds, or 0 when phase
/// timing is disabled (in which case no caller uses the value).
inline uint64_t nowNs() {
  if (!phaseTimingEnabled) {
    return 0;
  }
  return (uint64_t)std::chrono::duration_cast<std::chrono::nanoseconds>(
             std::chrono::steady_clock::now().time_since_epoch())
      .count();
}

/// Add the interval [\p start, now] to \p phase and \return the reading taken
/// for "now", so it can be reused as the next phase's start without a second
/// clock read.
inline uint64_t endPhase(Phase phase, uint64_t start) {
  if (!phaseTimingEnabled) {
    return 0;
  }
  uint64_t now = nowNs();
  phaseTimings.ns[phase] += now - start;
  return now;
}

/// Records the teardown phase from its destructor. Declared as the *first*
/// local of \c parse so that it is destroyed *last* — after the Context, the
/// parser and the AST arena — which is the only way to observe how much of a
/// call is spent freeing them, since that work happens after the return value
/// has already been constructed.
class TeardownTimer {
 public:
  ~TeardownTimer() {
    if (phaseTimingEnabled && start_ != 0) {
      phaseTimings.ns[PH_TEARDOWN] += nowNs() - start_;
    }
  }

  /// Begin measuring teardown from \p start. Until this is called the timer
  /// is inert, so error paths that return before the AST exists contribute
  /// nothing.
  void arm(uint64_t start) {
    start_ = start;
  }

 private:
  uint64_t start_ = 0;
};

/// Read an optional boolean property from \p obj, defaulting to false.
bool boolOption(napi_env env, napi_value obj, const char *name) {
  napi_value prop;
  if (napi_get_named_property(env, obj, name, &prop) != napi_ok) {
    return false;
  }

  napi_valuetype type;
  if (napi_typeof(env, prop, &type) != napi_ok || type != napi_boolean) {
    return false;
  }

  bool value = false;
  if (napi_get_value_bool(env, prop, &value) != napi_ok) {
    return false;
  }
  return value;
}

/// Set \p name on \p obj to the given uint32 value.
void setUint32(napi_env env, napi_value obj, const char *name, uint32_t v) {
  napi_value num;
  if (napi_create_uint32(env, v, &num) == napi_ok) {
    napi_set_named_property(env, obj, name, num);
  }
}

/// Set \p name on \p obj to the given UTF-8 string.
void setString(
    napi_env env,
    napi_value obj,
    const char *name,
    const std::string &v) {
  napi_value str;
  if (napi_create_string_utf8(env, v.data(), v.size(), &str) == napi_ok) {
    napi_set_named_property(env, obj, name, str);
  }
}

/// Build the `{error, line, column}` descriptor. The caller in JavaScript
/// turns this into a SyntaxError; Node-API cannot construct one directly.
napi_value errorResult(
    napi_env env,
    const std::string &message,
    uint32_t line,
    uint32_t column) {
  napi_value obj;
  if (napi_create_object(env, &obj) != napi_ok) {
    napi_throw_error(env, nullptr, "failed to allocate error result object");
    return nullptr;
  }
  setString(env, obj, "error", message);
  setUint32(env, obj, "line", line);
  setUint32(env, obj, "column", column);
  return obj;
}

/// \return true if any comment in \p context's doc block for \p fileBufId
/// contains an `@flow` pragma.
bool hasFlowPragma(Context &context, uint32_t fileBufId) {
  std::vector<parser::StoredComment> comments =
      parser::getCommentsInDocBlock(context, fileBufId);
  return parser::hasFlowPragma(comments);
}

/// Parse a source string and return either `{buffer}` or
/// `{error, line, column}`.
///
/// The Context/parser setup and error-handling lifecycle below is copied
/// verbatim (adjusted for Node-API instead of Emscripten exports) from
/// tools/hermes-parser/hermes-parser-wasm.cpp, which is the working
/// reference for how these options and diagnostics are wired up.
napi_value parse(napi_env env, napi_callback_info info) {
  // Declared first so it is destroyed last; see TeardownTimer.
  TeardownTimer teardownTimer;
  uint64_t phaseStart = nowNs();

  size_t argc = 2;
  napi_value argv[2];
  if (napi_get_cb_info(env, info, &argc, argv, nullptr, nullptr) != napi_ok) {
    napi_throw_error(env, nullptr, "failed to read call arguments");
    return nullptr;
  }
  if (argc < 2) {
    napi_throw_type_error(
        env, nullptr, "parse(source, options) requires two arguments");
    return nullptr;
  }

  // Copy the source out as NUL-terminated UTF-8. napi_get_value_string_utf8
  // NUL-terminates for us, so the parser's zero-termination requirement is
  // satisfied without a separate guard.
  size_t sourceLen = 0;
  if (napi_get_value_string_utf8(env, argv[0], nullptr, 0, &sourceLen) !=
      napi_ok) {
    napi_throw_type_error(env, nullptr, "source must be a string");
    return nullptr;
  }
  std::vector<char> source(sourceLen + 1, '\0');
  if (napi_get_value_string_utf8(
          env, argv[0], source.data(), source.size(), &sourceLen) != napi_ok) {
    napi_throw_error(env, nullptr, "failed to read source");
    return nullptr;
  }

  const bool detectFlow = boolOption(env, argv[1], "detectFlow");
  const bool componentSyntax =
      boolOption(env, argv[1], "enableExperimentalComponentSyntax");
  const bool matchSyntax =
      boolOption(env, argv[1], "enableExperimentalFlowMatchSyntax");
  const bool recordSyntax =
      boolOption(env, argv[1], "enableExperimentalFlowRecordSyntax");
  const bool tokens = boolOption(env, argv[1], "tokens");
  const bool allowReturnOutsideFunction =
      boolOption(env, argv[1], "allowReturnOutsideFunction");

  phaseStart = endPhase(PH_SOURCE_IN, phaseStart);

  // Set up custom diagnostic handler for error reporting.
  auto context = std::make_shared<Context>();
  auto &sm = context->getSourceErrorManager();
  const auto &diagHandler = HermesParserDiagHandler(sm);

  // Declared after \c diagHandler so that it is destroyed first. \c result
  // owns the parser and a reference to the context, whose destructors run
  // against the SourceErrorManager that \c diagHandler is registered on; the
  // handler must therefore still be alive at that point.
  ParseResult result;

  auto fileBuf = llvh::MemoryBuffer::getMemBuffer(
      llvh::StringRef{source.data(), sourceLen});
  int fileBufId = sm.addNewSourceBuffer(std::move(fileBuf));

  auto parseFlowSetting = detectFlow && !hasFlowPragma(*context, fileBufId)
      ? ParseFlowSetting::UNAMBIGUOUS
      : ParseFlowSetting::ALL;
  context->setParseFlow(parseFlowSetting);
  context->setParseFlowComponentSyntax(componentSyntax);
  context->setParseFlowMatch(matchSyntax);
  context->setParseFlowRecords(recordSyntax);
  context->setParseJSX(true);
  context->setUseCJSModules(true);
  context->setAllowReturnOutsideFunction(allowReturnOutsideFunction);

  phaseStart = endPhase(PH_CONTEXT_INIT, phaseStart);

  std::unique_ptr<parser::JSParser> jsParser =
      std::make_unique<parser::JSParser>(
          *context, fileBufId, parser::FullParse);
  jsParser->setStoreComments(true);
  jsParser->setStoreTokens(tokens);

  phaseStart = endPhase(PH_PARSER_INIT, phaseStart);

  llvh::Optional<ESTree::ProgramNode *> parsedJs = jsParser->parse();

  phaseStart = endPhase(PH_PARSE, phaseStart);

  // Return the first error if any were detected during parsing.
  if (diagHandler.hasError()) {
    return errorResult(
        env,
        diagHandler.getErrorString(),
        diagHandler.getErrorLine(),
        diagHandler.getErrorColumn());
  }

  // Return a generic error if no AST was produced but no specific error was
  // detected.
  if (!parsedJs) {
    return errorResult(env, "Failed to parse source", 0, 0);
  }

  // Keep the context and parser alive on the result: serialize() below
  // dereferences result.parser_ (e.g. for comments/tokens).
  result.context_ = context;
  result.parser_ = std::move(jsParser);
  serialize(*parsedJs, &sm, result, tokens);

  phaseStart = endPhase(PH_SERIALIZE, phaseStart);

  // Run semantic validation after the AST has been serialized. This mirrors
  // the reference (tools/hermes-parser/hermes-parser-wasm.cpp): resolution
  // never changes the already-serialized AST bytes, but it does reject
  // programs that parse syntactically yet are semantically invalid (e.g.
  // `continue` outside a loop), which must surface as parse errors too.
  sema::SemContext semContext{*context};
  resolveASTForParser(*context, semContext, *parsedJs);

  phaseStart = endPhase(PH_SEMA, phaseStart);

  // Return the first error if any were detected during semantic validation.
  if (diagHandler.hasError()) {
    return errorResult(
        env,
        diagHandler.getErrorString(),
        diagHandler.getErrorLine(),
        diagHandler.getErrorColumn());
  }

  auto container = writeContainer(
      result.programBuffer_, result.positionBuffer_, result.stringTable_);

  phaseStart = endPhase(PH_CONTAINER, phaseStart);

  void *data = nullptr;
  napi_value arrayBuffer;
  if (napi_create_arraybuffer(env, container.size(), &data, &arrayBuffer) !=
      napi_ok) {
    napi_throw_error(env, nullptr, "failed to allocate result buffer");
    return nullptr;
  }
  memcpy(data, container.data(), container.size());

  napi_value obj;
  if (napi_create_object(env, &obj) != napi_ok) {
    napi_throw_error(env, nullptr, "failed to allocate result object");
    return nullptr;
  }
  napi_set_named_property(env, obj, "buffer", arrayBuffer);

  phaseStart = endPhase(PH_COPY_OUT, phaseStart);
  if (phaseTimingEnabled) {
    ++phaseTimings.calls;
    // Everything from here to the caller regaining control is destruction of
    // the parser, the AST arena and the Context.
    teardownTimer.arm(phaseStart);
  }
  return obj;
}

/// Human-readable property names for the phases reported by
/// `getPhaseTimings()`, in \c Phase order.
const char *const kPhaseNames[kNumPhases] = {
    "sourceIn",
    "contextInit",
    "parserInit",
    "parse",
    "serialize",
    "sema",
    "container",
    "copyOut",
    "teardown",
};

/// `getPhaseTimings()` -> `{enabled, calls, <phase>: nanoseconds, ...}`.
/// All phase values are 0 when timing is disabled.
napi_value getPhaseTimings(napi_env env, napi_callback_info info) {
  napi_value obj;
  if (napi_create_object(env, &obj) != napi_ok) {
    napi_throw_error(env, nullptr, "failed to allocate timings object");
    return nullptr;
  }

  napi_value enabled;
  if (napi_get_boolean(env, phaseTimingEnabled, &enabled) == napi_ok) {
    napi_set_named_property(env, obj, "enabled", enabled);
  }

  // Nanosecond counts are reported as doubles. A double holds integers
  // exactly up to 2^53 ns, i.e. over 100 days of accumulated time in a single
  // phase, so no precision is lost at any plausible benchmark length.
  napi_value calls;
  if (napi_create_double(env, (double)phaseTimings.calls, &calls) == napi_ok) {
    napi_set_named_property(env, obj, "calls", calls);
  }

  for (int i = 0; i < kNumPhases; ++i) {
    napi_value v;
    if (napi_create_double(env, (double)phaseTimings.ns[i], &v) == napi_ok) {
      napi_set_named_property(env, obj, kPhaseNames[i], v);
    }
  }
  return obj;
}

/// `resetPhaseTimings()` -> undefined. Zeroes every accumulator.
napi_value resetPhaseTimings(napi_env env, napi_callback_info info) {
  phaseTimings = PhaseTimings{};
  napi_value undef;
  napi_get_undefined(env, &undef);
  return undef;
}

/// Module initializer. Registers `parse` on the exports object.
napi_value init(napi_env env, napi_value exports) {
  const char *timing = getenv("HERMES_PARSER_NATIVE_PHASE_TIMING");
  phaseTimingEnabled = timing != nullptr && timing[0] != '\0' &&
      strcmp(timing, "0") != 0;

  const struct {
    const char *name;
    napi_callback cb;
  } fns[] = {
      {"parse", parse},
      {"getPhaseTimings", getPhaseTimings},
      {"resetPhaseTimings", resetPhaseTimings},
  };

  for (const auto &entry : fns) {
    napi_value fn;
    if (napi_create_function(
            env, entry.name, NAPI_AUTO_LENGTH, entry.cb, nullptr, &fn) !=
        napi_ok) {
      napi_throw_error(env, nullptr, "failed to create an exported function");
      return nullptr;
    }
    if (napi_set_named_property(env, exports, entry.name, fn) != napi_ok) {
      napi_throw_error(env, nullptr, "failed to export a function");
      return nullptr;
    }
  }
  return exports;
}

} // namespace

NAPI_MODULE(NODE_GYP_MODULE_NAME, init)
