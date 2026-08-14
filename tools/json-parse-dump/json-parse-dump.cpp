/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

/// json-parse-dump: Drive hermes::parser::JSONParser + JSONEmitter and print
/// either canonical (non-pretty) JSON to stdout, or a bench timing line.
/// This is the C++ oracle for the Rust json_parse_dump differential test;
/// the Rust tool must produce byte-for-byte identical output.
///
/// OUTPUT CONTRACT:
///
/// Parse mode (default):
///   On success (parsed value AND sm.getErrorCount()==0):
///     Write canonical JSON via JSONEmitter to stdout.  NO trailing newline.
///   On error:
///     Print exactly "ERROR <errorCount>\n" to stdout.
///
/// Bench mode (--bench=N):
///   Read the source into memory once; loop N times, each iteration creating a
///   fresh JSLexer::Allocator + JSONFactory + SourceErrorManager + MemoryBuffer
///   and calling parser.parse().  Time the loop with std::chrono::steady_clock.
///   Print one line:
///     "parsed <N>x, <ms> ms, <MB/s> MB/s\n"
///   where ms = total milliseconds (one decimal, e.g. "12.3") and
///   MB/s = (sourceBytes * N) / seconds / 1e6 (two decimals).
///
/// Args: [--bench=N] [--convert-surrogates] <file|->
///   -  means read from stdin.
///   --convert-surrogates  passes convertSurrogates=true to JSONParser.
///
/// See also: tools/js-lexer-dump/  (lexer oracle, same harness pattern)

#include "hermes/Parser/JSONParser.h"
#include "hermes/Support/JSONEmitter.h"
#include "hermes/Support/SourceErrorManager.h"

#include "llvh/Support/MemoryBuffer.h"
#include "llvh/Support/raw_ostream.h"

#include <chrono>
#include <cstdio>
#include <cstring>
#include <string>

using namespace hermes;
using namespace hermes::parser;

static void usage(const char *argv0) {
  llvh::errs() << "Usage: " << argv0
               << " [--bench=N] [--convert-surrogates] <file|->\n"
               << "  Parse JSON and emit canonical output to stdout.\n"
               << "  --bench=N           Parse N times, print timing.\n"
               << "  --convert-surrogates  Pass convertSurrogates=true.\n"
               << "  Use - to read from stdin.\n";
}

int main(int argc, char **argv) {
  // Parse arguments.
  int benchCount = 0;
  bool convertSurrogates = false;
  const char *filePath = nullptr;

  for (int i = 1; i < argc; ++i) {
    const char *arg = argv[i];
    if (std::strncmp(arg, "--bench=", 8) == 0) {
      benchCount = std::atoi(arg + 8);
      if (benchCount <= 0) {
        llvh::errs() << argv[0] << ": --bench value must be > 0\n";
        usage(argv[0]);
        return 1;
      }
    } else if (std::strcmp(arg, "--convert-surrogates") == 0) {
      convertSurrogates = true;
    } else if (arg[0] == '-' && arg[1] == '-') {
      llvh::errs() << argv[0] << ": unknown flag '" << arg << "'\n";
      usage(argv[0]);
      return 1;
    } else {
      if (filePath != nullptr) {
        llvh::errs() << argv[0] << ": too many positional arguments\n";
        usage(argv[0]);
        return 1;
      }
      filePath = arg;
    }
  }

  if (filePath == nullptr) {
    usage(argv[0]);
    return 1;
  }

  // Read input into a MemoryBuffer (handles file or stdin via "-").
  auto fileBufOrErr =
      llvh::MemoryBuffer::getFileOrSTDIN(llvh::StringRef(filePath));
  if (!fileBufOrErr) {
    llvh::errs() << argv[0] << ": error reading '" << filePath
                 << "': " << fileBufOrErr.getError().message() << "\n";
    return 1;
  }

  if (benchCount > 0) {
    // --- BENCH MODE ---
    // Keep the raw bytes in a std::string so we can re-create MemoryBuffers.
    std::string sourceData(
        fileBufOrErr.get()->getBuffer().data(),
        fileBufOrErr.get()->getBufferSize());
    size_t sourceBytes = sourceData.size();

    auto t0 = std::chrono::steady_clock::now();
    for (int iter = 0; iter < benchCount; ++iter) {
      JSLexer::Allocator alloc;
      JSONFactory factory(alloc);
      SourceErrorManager sm;
      // Suppress all diagnostics during bench to avoid I/O overhead.
      // Install a no-op handler so nothing is printed to stderr.
      sm.setDiagHandler(
          [](const llvh::SMDiagnostic &, void *) {}, nullptr);
      // Create a fresh MemoryBuffer over the in-memory source.
      auto buf = llvh::MemoryBuffer::getMemBufferCopy(
          llvh::StringRef(sourceData.data(), sourceBytes), "json");
      JSONParser parser(factory, std::move(buf), sm, convertSurrogates);
      (void)parser.parse();
    }
    auto t1 = std::chrono::steady_clock::now();

    double seconds =
        std::chrono::duration<double>(t1 - t0).count();
    double ms = seconds * 1000.0;
    double mbps = (double)sourceBytes * benchCount / seconds / 1e6;

    // Print timing: "parsed Nx, X.X ms, Y.YY MB/s\n"
    char buf[256];
    std::snprintf(
        buf,
        sizeof(buf),
        "parsed %dx, %.1f ms, %.2f MB/s\n",
        benchCount,
        ms,
        mbps);
    llvh::outs() << buf;
  } else {
    // --- PARSE MODE ---
    JSLexer::Allocator alloc;
    JSONFactory factory(alloc);
    SourceErrorManager sm;
    // Suppress diagnostic output — we report errors via "ERROR N" ourselves.
    // Install a no-op handler so nothing is printed to stderr.
    sm.setDiagHandler(
        [](const llvh::SMDiagnostic &, void *) {}, nullptr);

    JSONParser parser(
        factory, std::move(fileBufOrErr.get()), sm, convertSurrogates);
    llvh::Optional<JSONValue *> parsed = parser.parse();

    if (parsed.hasValue() && sm.getErrorCount() == 0) {
      // Emit canonical (non-pretty) JSON, no trailing newline.
      std::string s;
      llvh::raw_string_ostream os(s);
      JSONEmitter emitter(os);
      parsed.getValue()->emitInto(emitter);
      os.flush();
      llvh::outs() << s;
    } else {
      // Error path: print "ERROR <count>\n" to stdout.
      llvh::outs() << "ERROR " << sm.getErrorCount() << "\n";
    }
  }

  return 0;
}
