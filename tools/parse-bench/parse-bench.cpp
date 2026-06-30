/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

/// parse-bench: Time the Hermes JSParser (FullParse) on one or more JS files
/// and report throughput in MiB/s.  Designed to produce the C++ baseline for
/// comparison with the Rust port's benchmark harness.
///
/// Usage: parse-bench [--iters=N] file1.js [file2.js ...]
///   --iters=N   Number of timed iterations per file (default: 30).
///               One warm-up iteration is run before timing begins.
///
/// Output (one line per file):
///   <file>  size=<bytes>  median=<ms> ms  throughput=<MiB/s> MiB/s
///           errored=<yes|no>
///
/// Each timed iteration allocates a fresh Context + MemoryBuffer (null-
/// terminated copy of the source) and runs JSParser(FullParse).parse().
/// A silent diagnostic handler is installed so error files do not spam
/// stderr; the errored flag is printed to stdout instead.

#include "hermes/AST/Context.h"
#include "hermes/Parser/JSParser.h"
#include "hermes/Sema/SemContext.h"
#include "hermes/Sema/SemResolve.h"
#include "hermes/Support/SourceErrorManager.h"

#include "llvh/Support/MemoryBuffer.h"
#include "llvh/Support/raw_ostream.h"

#include <algorithm>
#include <chrono>
#include <cstdio>
#include <cstring>
#include <fstream>
#include <sstream>
#include <string>
#include <vector>

using namespace hermes;

// ---------------------------------------------------------------------------
// Silent diagnostic handler: counts errors, never emits to stderr.
// ---------------------------------------------------------------------------

struct SilentDiagCtx {
  unsigned errorCount = 0;
};

static unsigned long gDiagCount = 0;
static void silentDiagHandler(const llvh::SMDiagnostic &, void *ctx) {
  // Count diagnostics (the SMDiagnostic's location is computed before this is
  // called, which is what triggers SourceMgr::getOffsets). Output suppressed.
  ++gDiagCount;
  (void)ctx;
}

// ---------------------------------------------------------------------------
// Sink to prevent the optimizer from discarding parse results.
// ---------------------------------------------------------------------------
static volatile int gSink = 0;

// When true, parseOnce also runs semantic resolution (binding/scope) after
// parse, to match OXC's parse + oxc_semantic for a fair end-to-end comparison.
static bool gRunSema = false;

// ---------------------------------------------------------------------------
// Parse \p source (len bytes) once and return true if it parsed without error.
// ---------------------------------------------------------------------------
static bool parseOnce(const char *data, size_t len) {
  auto context = std::make_shared<Context>();
  auto &sm = context->getSourceErrorManager();

  SilentDiagCtx diagCtx;
  sm.setDiagHandler(silentDiagHandler, &diagCtx);

  // getMemBufferCopy null-terminates the copy, which JSParser requires.
  auto buf = llvh::MemoryBuffer::getMemBufferCopy(
      llvh::StringRef{data, len}, "source");
  int bufId = sm.addNewSourceBuffer(std::move(buf));

  parser::JSParser p(*context, bufId, parser::FullParse);
  llvh::Optional<ESTree::ProgramNode *> r = p.parse();

  bool ok = r.hasValue() && sm.getErrorCount() == 0;
  if (gRunSema && r.hasValue()) {
    sema::SemContext semContext{*context};
    resolveASTForParser(*context, semContext, *r);
  }
  // Accumulate into the sink so the compiler cannot eliminate the parse.
  gSink += (int)(intptr_t)(r.hasValue() ? *r : nullptr);
  return ok;
}

// Time setup / parse / teardown separately for one parse of \p data.
// Returns {setup_s, parse_s, teardown_s}.
struct Phases {
  double setup, parse, teardown;
};
static Phases parseOnceBreakdown(const char *data, size_t len) {
  using clk = std::chrono::steady_clock;
  auto t0 = clk::now();
  auto context = std::make_shared<Context>();
  auto &sm = context->getSourceErrorManager();
  SilentDiagCtx diagCtx;
  sm.setDiagHandler(silentDiagHandler, &diagCtx);
  auto buf = llvh::MemoryBuffer::getMemBufferCopy(
      llvh::StringRef{data, len}, "source");
  int bufId = sm.addNewSourceBuffer(std::move(buf));
  auto pp = std::make_unique<parser::JSParser>(*context, bufId, parser::FullParse);
  auto t1 = clk::now();
  llvh::Optional<ESTree::ProgramNode *> r = pp->parse();
  auto t2 = clk::now();
  gSink += (int)(intptr_t)(r.hasValue() ? *r : nullptr);
  // Explicit teardown: drop parser then context (frees AST + string table).
  pp.reset();
  context.reset();
  auto t3 = clk::now();
  auto secs = [](auto a, auto b) {
    return std::chrono::duration<double>(b - a).count();
  };
  return {secs(t0, t1), secs(t1, t2), secs(t2, t3)};
}

// Lex \p data to EOF (no parsing, no AST construction). Returns token count.
static size_t lexOnce(const char *data, size_t len) {
  parser::JSLexer::Allocator alloc;
  SourceErrorManager sm;
  sm.setDiagHandler(silentDiagHandler, nullptr);
  auto buf = llvh::MemoryBuffer::getMemBufferCopy(llvh::StringRef{data, len}, "source");
  parser::JSLexer lex(std::move(buf), sm, alloc);
  size_t n = 0;
  const parser::Token *tok;
  do {
    tok = lex.advance(parser::JSLexer::AllowRegExp);
    ++n;
  } while (tok->getKind() != parser::TokenKind::eof);
  gSink += (int)n;
  return n;
}

// ---------------------------------------------------------------------------
// Read entire file into a std::string (binary mode).
// ---------------------------------------------------------------------------
static bool readFile(const char *path, std::string &out) {
  std::ifstream f(path, std::ios::binary | std::ios::ate);
  if (!f) {
    return false;
  }
  std::streamsize sz = f.tellg();
  f.seekg(0, std::ios::beg);
  out.resize((size_t)sz);
  if (!f.read(out.data(), sz)) {
    return false;
  }
  return true;
}

// ---------------------------------------------------------------------------
// Compute median of a sorted vector.
// ---------------------------------------------------------------------------
static double median(std::vector<double> &v) {
  std::sort(v.begin(), v.end());
  size_t n = v.size();
  if (n == 0)
    return 0.0;
  if (n % 2 == 1)
    return v[n / 2];
  return (v[n / 2 - 1] + v[n / 2]) * 0.5;
}

// ---------------------------------------------------------------------------
// Usage.
// ---------------------------------------------------------------------------
static void usage(const char *argv0) {
  llvh::errs() << "Usage: " << argv0
               << " [--iters=N] file1.js [file2.js ...]\n"
               << "  --iters=N   Timed iterations per file (default: 30)\n";
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------
int main(int argc, char **argv) {
  int iters = 30;
  bool breakdown = false;
  bool lexOnly = false;
  std::vector<const char *> files;

  for (int i = 1; i < argc; ++i) {
    const char *arg = argv[i];
    if (std::strncmp(arg, "--iters=", 8) == 0) {
      iters = std::atoi(arg + 8);
      if (iters <= 0) {
        llvh::errs() << argv[0] << ": --iters must be positive\n";
        return 1;
      }
    } else if (std::strcmp(arg, "--breakdown") == 0) {
      breakdown = true;
    } else if (std::strcmp(arg, "--lex-only") == 0) {
      lexOnly = true;
    } else if (std::strcmp(arg, "--sema") == 0) {
      gRunSema = true;
    } else if (arg[0] == '-' && arg[1] == '-') {
      llvh::errs() << argv[0] << ": unknown flag '" << arg << "'\n";
      usage(argv[0]);
      return 1;
    } else {
      files.push_back(arg);
    }
  }

  if (files.empty()) {
    usage(argv[0]);
    return 1;
  }

  // Header.
  llvh::outs() << "parse-bench: C++ Hermes JSParser (FullParse) baseline\n";
  llvh::outs() << "iters=" << iters << " (+ 1 warm-up)"
               << (lexOnly ? "  mode=LEX-ONLY" : "  mode=PARSE") << "\n\n";

  for (const char *path : files) {
    // Read file once.
    std::string src;
    if (!readFile(path, src)) {
      llvh::errs() << argv[0] << ": cannot read '" << path << "'\n";
      continue;
    }
    size_t bytes = src.size();

    if (lexOnly) {
      gDiagCount = 0;
      size_t ntok = lexOnce(src.data(), bytes); // warm-up
      unsigned long diags = gDiagCount; // diagnostics emitted by ONE lex
      std::vector<double> times;
      for (int it = 0; it < iters; ++it) {
        auto t0 = std::chrono::steady_clock::now();
        lexOnce(src.data(), bytes);
        auto t1 = std::chrono::steady_clock::now();
        times.push_back(std::chrono::duration<double, std::milli>(t1 - t0).count());
      }
      double m = median(times);
      double mib = (m > 0) ? (double)bytes / (m * 1e-3) / (1024.0 * 1024.0) : 0.0;
      const char *nm = path;
      for (const char *q = path; *q; ++q)
        if (*q == '/') nm = q + 1;
      llvh::outs() << nm << "  size=" << bytes << "  tokens=" << ntok
                   << "  diags=" << diags
                   << "  median=" << llvh::format("%.3f", m) << " ms"
                   << "  LEX=" << llvh::format("%.1f", mib) << " MiB/s\n";
      continue;
    }

    if (breakdown) {
      parseOnceBreakdown(src.data(), bytes); // warm-up
      std::vector<double> setup, parse, teardown;
      for (int it = 0; it < iters; ++it) {
        Phases p = parseOnceBreakdown(src.data(), bytes);
        setup.push_back(p.setup * 1e3);
        parse.push_back(p.parse * 1e3);
        teardown.push_back(p.teardown * 1e3);
      }
      double s = median(setup), pa = median(parse), td = median(teardown);
      double parseMib = (pa > 0) ? (double)bytes / (pa * 1e-3) / (1024.0 * 1024.0) : 0.0;
      double fullMib =
          (s + pa + td > 0) ? (double)bytes / ((s + pa + td) * 1e-3) / (1024.0 * 1024.0) : 0.0;
      const char *nm = path;
      for (const char *q = path; *q; ++q)
        if (*q == '/') nm = q + 1;
      llvh::outs() << nm << "  size=" << bytes << "\n"
                   << "  setup=" << llvh::format("%.3f", s) << " ms"
                   << "  parse=" << llvh::format("%.3f", pa) << " ms"
                   << "  teardown=" << llvh::format("%.3f", td) << " ms\n"
                   << "  PARSE-ONLY=" << llvh::format("%.1f", parseMib) << " MiB/s"
                   << "  full-iter=" << llvh::format("%.1f", fullMib) << " MiB/s\n";
      continue;
    }

    // Warm-up (not timed).
    bool ok = parseOnce(src.data(), bytes);

    // Timed iterations.
    std::vector<double> times;
    times.reserve((size_t)iters);
    for (int it = 0; it < iters; ++it) {
      auto t0 = std::chrono::steady_clock::now();
      bool iterOk = parseOnce(src.data(), bytes);
      auto t1 = std::chrono::steady_clock::now();
      ok = ok && iterOk; // propagate any per-iter error
      double ms =
          std::chrono::duration<double, std::milli>(t1 - t0).count();
      times.push_back(ms);
    }

    double medMs = median(times);
    // MiB/s = bytes / (medMs / 1000) / (1024*1024)
    double mibPerSec =
        (medMs > 0.0) ? (double)bytes / (medMs * 1e-3) / (1024.0 * 1024.0)
                      : 0.0;

    // Derive a short display name (basename).
    const char *name = path;
    for (const char *p = path; *p; ++p) {
      if (*p == '/')
        name = p + 1;
    }

    llvh::outs() << name << "\n"
                 << "  size=" << bytes << " bytes"
                 << "  median=" << llvh::format("%.3f", medMs) << " ms"
                 << "  throughput=" << llvh::format("%.1f", mibPerSec)
                 << " MiB/s"
                 << "  errored=" << (ok ? "no" : "yes") << "\n";
  }

  // Prevent the sink from being optimized out.
  if (gSink == 0x7fffffff) {
    llvh::outs() << "(sink=" << gSink << ")\n";
  }

  return 0;
}
