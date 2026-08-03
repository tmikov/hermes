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
/// Args: [--parse-flow] <file|->
///   - means read from stdin.
///
/// No `-ferror-limit`: `SourceErrorManager`'s default is unbounded
/// (`error_limit_` unset), and neither this tool nor
/// `hermes-parser-wasm.cpp` (the other `resolveASTForParser` caller) sets
/// one.

#include "hermes/AST/Context.h"
#include "hermes/Parser/JSParser.h"
#include "hermes/Sema/SemContext.h"
#include "hermes/Sema/SemResolve.h"
#include "hermes/Support/SourceErrorManager.h"

#include "llvh/Support/MemoryBuffer.h"
#include "llvh/Support/raw_ostream.h"

#include <cstring>

using namespace hermes;

int main(int argc, char **argv) {
  const char *filePath = nullptr;
  bool parseFlow = false;
  for (int i = 1; i < argc; ++i) {
    const char *arg = argv[i];
    if (std::strcmp(arg, "--parse-flow") == 0) {
      parseFlow = true;
      continue;
    }
    if (filePath != nullptr) {
      llvh::errs() << argv[0] << ": too many arguments\n";
      return 1;
    }
    filePath = arg;
  }
  if (filePath == nullptr) {
    llvh::errs() << "Usage: " << argv[0] << " [--parse-flow] <file|->\n";
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
  SourceErrorManager &sm = ctx.getSourceErrorManager();

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
