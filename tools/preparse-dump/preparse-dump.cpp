/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

/// preparse-dump: Drive JSParser::preParseBuffer and print the PreParse
/// side-table in a canonical text format.  This is the C++ oracle for the
/// Rust preparse_differential test.
///
/// OUTPUT CONTRACT
///   On success: "PREPARSE <n>\n" followed by one line per entry (sorted by
///     start offset): "<start> <end> <strict> <arrow> <arrowArgs> <dirCount>
///     [dir...]\n"
///   On error: "ERROR <count>\n"
///
/// Args: <file|->
///   - means read from stdin.

#include "hermes/AST/Context.h"
#include "hermes/Parser/JSParser.h"
#include "hermes/Parser/PreParser.h"
#include "hermes/Support/SourceErrorManager.h"

#include "llvh/Support/MemoryBuffer.h"
#include "llvh/Support/raw_ostream.h"

#include <algorithm>
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

using namespace hermes;
using namespace hermes::parser;

int main(int argc, char **argv) {
  const char *filePath = nullptr;
  for (int i = 1; i < argc; ++i) {
    const char *arg = argv[i];
    if (filePath != nullptr) {
      llvh::errs() << argv[0] << ": too many arguments\n";
      return 1;
    }
    filePath = arg;
  }
  if (filePath == nullptr) {
    llvh::errs() << "Usage: " << argv[0] << " <file|->\n";
    return 1;
  }

  auto fileBufOrErr =
      llvh::MemoryBuffer::getFileOrSTDIN(llvh::StringRef(filePath));
  if (!fileBufOrErr) {
    llvh::errs() << argv[0] << ": error reading '" << filePath
                 << "': " << fileBufOrErr.getError().message() << "\n";
    return 1;
  }

  auto ctx = std::make_shared<Context>();
  SourceErrorManager &sm = ctx->getSourceErrorManager();
  // Suppress diagnostics — we report errors via "ERROR N" ourselves.
  sm.setDiagHandler([](const llvh::SMDiagnostic &, void *) {}, nullptr);

  uint32_t bufId = sm.addNewSourceBuffer(std::move(fileBufOrErr.get()));
  auto parser = JSParser::preParseBuffer(*ctx, bufId, /*strict=*/false);
  if (!parser) {
    llvh::outs() << "ERROR " << sm.getErrorCount() << "\n";
    return 0;
  }

  const char *bufStart = sm.getSourceBuffer(bufId)->getBufferStart();
  PreParsedBufferInfo *info = ctx->getPreParsedBufferInfo(bufId);

  // Collect and sort by start offset (the DenseMap is unordered).
  std::vector<std::pair<size_t, const PreParsedFunctionInfo *>> v;
  v.reserve(info->functionInfo.size());
  for (auto &kv : info->functionInfo) {
    size_t startOff = (size_t)(kv.first.getPointer() - bufStart);
    v.push_back({startOff, &kv.second});
  }
  std::sort(v.begin(), v.end(), [](auto &a, auto &b) {
    return a.first < b.first;
  });

  llvh::outs() << "PREPARSE " << v.size() << "\n";
  for (auto &e : v) {
    size_t endOff = (size_t)(e.second->end.getPointer() - bufStart);
    llvh::outs() << e.first << " " << endOff << " "
                 << (e.second->strictMode ? 1 : 0) << " "
                 << (e.second->containsArrowFunctions ? 1 : 0) << " "
                 << (e.second->mayContainArrowFunctionsUsingArguments ? 1 : 0)
                 << " " << e.second->directives.size();
    for (auto &d : e.second->directives)
      llvh::outs() << " " << d;
    llvh::outs() << "\n";
  }
  return 0;
}
