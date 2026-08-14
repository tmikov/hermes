/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

/// sema-parser-dump: parse a JS file, resolve it via
/// `sema::resolveASTForParser` (the `compile = false` entry point used by
/// `hermes-parser-wasm.cpp:104`, `lib/Sema/SemResolve.cpp:295-306`), and
/// dump the resulting `SemContext` + annotated AST via `sema::semDump`
/// (`SemResolve.h:111`) UNCONDITIONALLY — even when resolution reported
/// errors. This is the C++ oracle for the Rust `sema_parser_differential`
/// test: `hermesc -dump-sema` always resolves with `compile = true`
/// (`CompilerDriver.cpp:960-965`: `resolveAST` failure returns `nullptr`
/// before the `DumpSema` dump at :969-974 ever runs), so there is no
/// driver-path coverage for the `compile = false` entry point that
/// `resolveASTForParser` is. This tool exists purely to exercise it.
///
/// OUTPUT CONTRACT
///   Parse the file (honoring `-parse-flow`). If parsing fails (no AST),
///   print nothing to stdout and exit 2 (diagnostics already went to
///   stderr as they were produced — see below).
///   Otherwise call `resolveASTForParser` and then `semDump` UNCONDITIONALLY
///   — regardless of whether resolution itself succeeded — passing
///   `flowContext = nullptr` (this tool has no Flow type checker; matches
///   the untyped `semDump` arm, `SemResolve.cpp:260-283`). Exit 2 iff
///   `sm.getErrorCount() != 0` (from parsing and/or resolution combined),
///   else exit 0.
///
///   Diagnostics (errors and warnings) go to stderr via the
///   `SourceErrorManager`'s default `printDiagnosticHelper`-based handler
///   (installed by its constructor, `SourceErrorManager.cpp:20-24`) —
///   nothing extra to wire up, unlike `preparse-dump`, which installs a
///   no-op handler specifically to SUPPRESS printing.
///
/// Args: [--parse-flow|-parse-flow]
///       [--Xparse-flow-match|-Xparse-flow-match] <file|->
///   - means read from stdin.
///
///   `-Xparse-flow-match` does NOT imply `-parse-flow`, exactly as hermesc's
///   hidden flag does not (CompilerDriver.cpp:1298-1302 sets the two
///   independently) — a corpus file that wants a Flow `match` spells both,
///   the way the upstream lit tests do.
///
/// No `-ferror-limit`: `SourceErrorManager`'s default is unbounded
/// (`error_limit_` unset), and neither this tool nor
/// `hermes-parser-wasm.cpp` (the other `resolveASTForParser` caller) sets
/// one.

#include "hermes/AST/Context.h"
#include "hermes/Parser/JSParser.h"
#include "hermes/Sema/SemContext.h"
#include "hermes/Sema/SemResolve.h"
#include "hermes/Support/OSCompat.h"
#include "hermes/Support/SourceErrorManager.h"

#include "llvh/Support/MemoryBuffer.h"
#include "llvh/Support/raw_ostream.h"

#include <cstring>

using namespace hermes;

int main(int argc, char **argv) {
  const char *filePath = nullptr;
  bool parseFlow = false;
  bool parseFlowMatch = false;
  for (int i = 1; i < argc; ++i) {
    const char *arg = argv[i];
    // Both spellings: `--parse-flow` (this tool's original) and
    // `-parse-flow` (hermesc's own spelling, which is what a corpus file's
    // `// FLAGS:` line carries — the differential harness appends those args
    // verbatim to BOTH binaries' argv, and the Rust `command_line` parser
    // accepts either dash count for a `long` option).
    if (std::strcmp(arg, "--parse-flow") == 0 ||
        std::strcmp(arg, "-parse-flow") == 0) {
      parseFlow = true;
      continue;
    }
    // Both spellings again, for the same reason: a `// FLAGS:` line has to
    // name the flag once for both binaries, and the Rust `sema-dump` only
    // understands the DOUBLE-dash form of a `-X` option (its `command_line`
    // single-dash path would read `-X` as a short option with an attached
    // value), while LLVM's `cl` — i.e. hermesc — accepts either.
    if (std::strcmp(arg, "--Xparse-flow-match") == 0 ||
        std::strcmp(arg, "-Xparse-flow-match") == 0) {
      parseFlowMatch = true;
      continue;
    }
    if (filePath != nullptr) {
      llvh::errs() << argv[0] << ": too many arguments\n";
      return 1;
    }
    filePath = arg;
  }
  if (filePath == nullptr) {
    llvh::errs() << "Usage: " << argv[0]
                 << " [--parse-flow|-parse-flow] "
                    "[--Xparse-flow-match|-Xparse-flow-match] <file|->\n";
    return 1;
  }

  auto fileBufOrErr =
      llvh::MemoryBuffer::getFileOrSTDIN(llvh::StringRef(filePath));
  if (!fileBufOrErr) {
    llvh::errs() << argv[0] << ": error reading '" << filePath
                 << "': " << fileBufOrErr.getError().message() << "\n";
    return 1;
  }

  Context ctx;
  if (parseFlow)
    ctx.setParseFlow(ParseFlowSetting::ALL);
  ctx.setParseFlowMatch(parseFlowMatch);
  SourceErrorManager &sm = ctx.getSourceErrorManager();
  // `SourceErrorOutputOptions::showColors` defaults to `true`
  // (SourceErrorManager.h:35), but the Rust renderer this tool is an
  // oracle for has no color support at all (`support/src/render.rs`) — it
  // always emits plain text. Match hermesc's own
  // `guessErrorOutputOptions()` (CompilerDriver.cpp:776-791), which asks
  // `oscompat::should_color(STDERR_FILENO)` rather than hardcoding either
  // way: under the differential harness, stderr is a pipe (not a tty), so
  // this evaluates to `false` on both sides, keeping stderr colorless and
  // byte-comparable; interactively (a real tty) this tool colorizes like
  // hermesc does. `preferredMaxErrorWidth` is left at its `UnlimitedWidth`
  // default, which is what `guessErrorOutputOptions()` also produces for a
  // non-tty stderr (and `cl::MaxDiagnosticWidth` defaults to 0, i.e. "don't
  // override").
  SourceErrorOutputOptions outputOptions;
  outputOptions.showColors = oscompat::should_color(STDERR_FILENO);
  sm.setOutputOptions(outputOptions);

  uint32_t bufId = sm.addNewSourceBuffer(std::move(fileBufOrErr.get()));

  llvh::Optional<ESTree::ProgramNode *> parsedJs;
  {
    parser::JSParser jsParser(ctx, bufId, parser::FullParse);
    parsedJs = jsParser.parse();
  }
  if (!parsedJs) {
    // Diagnostics were already printed as they were produced; nothing to
    // dump without an AST.
    return sm.getErrorCount() != 0 ? 2 : 0;
  }
  ESTree::ProgramNode *root = *parsedJs;

  sema::SemContext semCtx(ctx);
  // `compile = false`: resolve without preparing the AST for compilation
  // (no ambient decls, no compile-only errors/rewrites) — the exact call
  // `hermes-parser-wasm.cpp:104` makes. The return value is intentionally
  // ignored: whether resolution succeeded or failed, we dump unconditionally
  // (that's the whole point of this tool) and derive the exit code from
  // `sm.getErrorCount()` below.
  sema::resolveASTForParser(ctx, semCtx, root);

  sema::semDump(llvh::outs(), ctx, semCtx, /* flowContext */ nullptr, root);

  return sm.getErrorCount() != 0 ? 2 : 0;
}
