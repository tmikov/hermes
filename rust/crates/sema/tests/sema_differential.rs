/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Byte-for-byte differential: resolve each corpus file with a C++ oracle
//! and the Rust `sema-dump`, comparing stdout, stderr AND exit status. Two
//! independent oracle/Rust pairs share the machinery below (see
//! [`ToolPair`]):
//!
//!   - `sema_differential_s0`: `hermesc -dump-sema` vs plain `sema-dump` —
//!     the driver path, `compile = true` (`sema::resolveAST`).
//!   - `sema_parser_differential`: `sema-parser-dump` vs `sema-dump
//!     --parser-entry` — the `compile = false` path (`sema::
//!     resolveASTForParser`, `SemResolve.cpp:299-310`), which has no
//!     driver-path coverage because `hermesc -dump-sema` always resolves
//!     with `compile = true` and skips the dump on error
//!     (`CompilerDriver.cpp:978-992`).
//!
//! GATE COMMANDS (the parser-entry gate additionally needs the
//! `sema-parser-dump` C++ tool):
//!
//! ```text
//! cmake --build cmake-build-asan --target hermesc
//! cmake --build cmake-build-asan --target sema-parser-dump
//! REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml \
//!     -p hermes-sema --test sema_differential -- --nocapture
//! ```
//!
//! The `sema-dump` driver lives in the unpublished `tools` crate, so this test
//! builds it and finds it through [`common::tools_bin`] rather than through
//! Cargo's `CARGO_BIN_EXE_<name>` (which is only defined for binaries of the
//! same package). A plain `cargo test` over the workspace stays green: without
//! the C++ oracles present the run skips (see the bottom of this doc).
//!
//! Flag pairing: both oracle binaries and `sema-dump` take the file as their
//! only positional argument; `-fstd-globals`/`-fno-std-globals` (which loads
//! `libhermes` as the ambient-declaration file) and `-enable-eval` both
//! default to true on both sides, and `-strict` defaults to false on both
//! (the parser-entry pair does not use ambient decls at all — see
//! `resolve_ast_for_parser`'s doc). A corpus file may opt into extra flags
//! by making its FIRST LINE exactly `// FLAGS: <args>` (see
//! `per_file_flags`); the whitespace-split args are appended, in order, to
//! BOTH binaries' argv after the pair's own extra args and before the file
//! path itself. Flagless files (everything predating this harness) behave
//! exactly as before.
//!
//! The corpus is standard JS only (S0's resolver handles literals, string
//! literals, empty statements and the directive prologue; anything else
//! panics by design). `inline-noinline.js` produces a real warning, so the
//! stderr comparison is not vacuous: it pins the diagnostic text, the
//! `file:line:col: kind:` prefix, the echoed source line and the caret
//! underline — and, transitively, that `sema-dump` installs a diagnostic
//! handler at all (a handler-less `SourceErrorManager` silently discards
//! every message).
//!
//! `parse-error.js` (S1 task 2) is hermesc's FAILURE path: a corpus file may
//! legitimately make hermesc exit nonzero, in which case all three channels
//! (stdout, stderr, exit status) are still compared byte-for-byte — stdout
//! empty on both sides, stderr carrying hermesc's driver epilogue
//! (`Emitted N errors. exiting.\n`, CompilerDriver.cpp:2104-2108) and exit
//! code 2 (`CompileStatus::ParsingFailed`, CompilerDriver.h:19-38). The
//! corpus is only required to contain at least one file the oracle SUCCEEDS
//! on (a non-degeneracy guard: an all-failing corpus would make the stdout/
//! stderr comparison above vacuous in the success case).
//!
//! BUILD-PROFILE PAIRING — THIS TEST IS DEBUG-ONLY BY DESIGN.
//!
//! Run it with a plain `cargo test`. `cargo test --release` FAILS on
//! `nested-expressions.js` (oracle reports the recursion-limit error at
//! `12:46`, the Rust side at `23:62`) and always will. That divergence is
//! NOT a port defect and must not be "fixed" by changing a limit — it is
//! the arithmetic consequence of pairing a 128-deep ASan oracle with a
//! 1024-deep release build of the port. Two independent reasons:
//!
//!   1. The recursion limits are profile-selected on BOTH sides. The C++
//!      oracles are ASan builds, so they take the `HERMES_LIMIT_STACK_DEPTH`
//!      branch (`Support/Compiler.h:106-110`): parser limit 128
//!      (`JSParserImpl.h:189-202`), resolver limit 512
//!      (`RecursiveVisitor.h:686-692`). The Rust side mirrors that branch
//!      under `cfg!(debug_assertions)` and takes C++'s release values
//!      (1024/1024) otherwise. So a `--release` `sema-dump` differentialed
//!      against the ASan oracles MISMATCHES on deep-nesting inputs — e.g.
//!      `nested-expressions.js` and `regress-nested-expressions-error.js`
//!      in this corpus. Debug-vs-ASan (what `cargo test` runs) and
//!      release-vs-release both agree; crossing them does not.
//!   2. `--release` also silently masks the `computed-fn-name.js` upstream
//!      C++ defect reproduction, whose assertions are compiled out — see the
//!      sweep-tooling note in `sema_corpus/MANIFEST.md`.
//!
//! Skip cleanly when the oracle binary is absent; set
//! `REQUIRE_DIFFERENTIAL=1` to turn a missing oracle into a hard failure
//! (used in CI).

use std::path::PathBuf;
use std::process::Command;

mod common;

/// Selects which oracle/Rust binary pair [`run_differential`] exercises —
/// see the module doc for what each variant ports.
enum ToolPair {
    /// `hermesc -dump-sema` vs plain `sema-dump` (`compile = true`).
    Driver,
    /// `sema-parser-dump` vs `sema-dump --parser-entry` (`compile = false`,
    /// `resolveASTForParser`).
    Parser,
}

impl ToolPair {
    /// Path to the C++ oracle binary (relative join from the crate manifest
    /// dir `<repo>/rust/crates/sema`, matching `parser_differential.rs`).
    fn oracle_bin(&self) -> PathBuf {
        let name = match self {
            ToolPair::Driver => "hermesc",
            ToolPair::Parser => "sema-parser-dump",
        };
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../cmake-build-asan/bin")
            .join(name)
    }

    /// The `cmake --build` invocation that produces [`Self::oracle_bin`],
    /// for the "binary missing" diagnostic.
    fn oracle_build_hint(&self) -> &'static str {
        match self {
            ToolPair::Driver => "cmake --build cmake-build-asan --target hermesc",
            ToolPair::Parser => {
                "cmake --build cmake-build-asan --target sema-parser-dump"
            }
        }
    }

    /// Args always passed to the oracle before `oracle_extra`/per-file flags.
    fn oracle_base_args(&self) -> &'static [&'static str] {
        match self {
            ToolPair::Driver => &["-dump-sema"],
            ToolPair::Parser => &[],
        }
    }

    /// Args always passed to `sema-dump` before `sema_dump_extra`/per-file
    /// flags. Both pairs run the SAME `sema-dump` binary; the parser pair
    /// just adds `--parser-entry` to switch resolution entry point.
    fn rust_base_args(&self) -> &'static [&'static str] {
        match self {
            ToolPair::Driver => &[],
            ToolPair::Parser => &["--parser-entry"],
        }
    }
}

/// If the file's first line is `// FLAGS: <args>`, return the args
/// (whitespace-split). Otherwise return an empty Vec (flagless — the
/// existing corpus files that predate this harness). Exact-prefix,
/// first-line-only: a `// FLAGS:` comment anywhere else in the file (e.g. in
/// a header or body) is not a per-file-flag line and is left alone.
fn per_file_flags(src: &[u8]) -> Vec<String> {
    let first = src.split(|&b| b == b'\n').next().unwrap_or(&[]);
    let Ok(first) = std::str::from_utf8(first) else { return vec![] };
    match first.strip_prefix("// FLAGS: ") {
        Some(rest) => rest.split_whitespace().map(String::from).collect(),
        None => vec![],
    }
}

/// The first-line-only rule is load-bearing, and nothing in the corpus walk
/// can observe a regression in it: every corpus file carries a `//` header
/// comment BELOW line 1 explaining what it pins, and several of those
/// comments quote flag names. If `per_file_flags` ever scanned the whole
/// file, such a header could silently change the argv of BOTH binaries —
/// which, being symmetric, would keep the differential green while testing
/// something other than what the file's own FLAGS line says.
#[test]
fn per_file_flags_is_first_line_only() {
    // Line 1, exact prefix: the flags are taken, whitespace-split, in order.
    assert_eq!(
        per_file_flags(b"// FLAGS: -parse-flow -parse-jsx\nvar x = 1;\n"),
        vec!["-parse-flow".to_string(), "-parse-jsx".to_string()],
    );
    // The identical line on line 2 is ordinary source text.
    assert!(
        per_file_flags(b"var x = 1;\n// FLAGS: -parse-flow\n").is_empty(),
        "a FLAGS line below line 1 must be ignored",
    );
    // Including the common shape: a header comment block whose SECOND line
    // mentions the prefix.
    assert!(
        per_file_flags(b"// header\n// FLAGS: -parse-flow\nvar x = 1;\n")
            .is_empty(),
        "a FLAGS line below line 1 must be ignored even inside a header",
    );
    // No FLAGS line at all: flagless, like every pre-harness corpus file.
    assert!(per_file_flags(b"var x = 1;\n").is_empty());
}

/// Run every `.js` file in `corpus` through `pair`'s C++ oracle (with
/// `oracle_extra` appended to the pair's base flags) and `sema-dump` (with
/// `rust_extra` appended to the pair's base flags), asserting byte-identical
/// stdout, byte-identical stderr and identical exit status. Skips (or
/// hard-fails under `REQUIRE_DIFFERENTIAL=1`) when the oracle binary is
/// missing. `read_dir` is non-recursive, so any subdirectory under `corpus`
/// (e.g. a `pending/` holding files not yet expected to match) is
/// automatically excluded from the walk without extra filtering.
fn run_differential(
    pair: ToolPair,
    corpus: &str,
    oracle_extra: &[&str],
    rust_extra: &[&str],
) {
    let oracle = pair.oracle_bin();
    if !oracle.exists() {
        if std::env::var_os("REQUIRE_DIFFERENTIAL").is_some() {
            panic!(
                "REQUIRE_DIFFERENTIAL set but oracle not found at {}; \
                 build: {}",
                oracle.display(),
                pair.oracle_build_hint()
            );
        }
        eprintln!(
            "skipping sema_differential: oracle not found at {} \
             (set REQUIRE_DIFFERENTIAL=1 to force)",
            oracle.display()
        );
        return;
    }

    // `sema-dump` is a binary of the `tools` crate, so Cargo neither builds it
    // for this test nor defines `CARGO_BIN_EXE_sema-dump`; `tools_bin` builds
    // it (once per test process) and returns its exact path. Both pairs use
    // this same binary — see `ToolPair::rust_base_args`.
    let sema_dump = common::tools_bin("sema-dump");

    let corpus_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(corpus);
    let mut files: Vec<PathBuf> = std::fs::read_dir(&corpus_dir)
        .unwrap_or_else(|e| panic!("{} dir missing: {e}", corpus_dir.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().map(|e| e == "js").unwrap_or(false))
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "{} has no .js files: the differential would pass vacuously",
        corpus_dir.display()
    );

    // Non-degeneracy guard: a corpus where the oracle fails on every file
    // would make the success-case byte comparison above vacuous (every file
    // could "pass" by both sides independently emitting unrelated garbage to
    // stderr with a matching nonzero exit code). At least one file must
    // exercise the success path.
    let mut oracle_successes = 0usize;

    for f in &files {
        let src = std::fs::read(f)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", f.display()));
        let file_flags = per_file_flags(&src);

        let c = Command::new(&oracle)
            .args(pair.oracle_base_args())
            .args(oracle_extra)
            .args(&file_flags)
            .arg(f)
            .output()
            .expect("failed to run oracle");
        if c.status.success() {
            oracle_successes += 1;
        }
        let r = Command::new(&sema_dump)
            .args(pair.rust_base_args())
            .args(rust_extra)
            .args(&file_flags)
            .arg(f)
            .output()
            .expect("failed to run sema-dump");
        // Compare the raw bytes, not a lossy UTF-8 decode: two distinct
        // invalid-UTF-8 byte sequences can both map to U+FFFD and compare
        // equal as `String`s while still being a real byte-for-byte
        // divergence. The lossy strings are only for the assert message.
        assert!(
            c.stdout == r.stdout,
            "sema dump mismatch (stdout) for {}:\n--- oracle ---\n{}\n\
             --- sema-dump ---\n{}",
            f.display(),
            String::from_utf8_lossy(&c.stdout),
            String::from_utf8_lossy(&r.stdout)
        );
        assert!(
            c.stderr == r.stderr,
            "sema dump mismatch (stderr) for {}:\n--- oracle ---\n{}\n\
             --- sema-dump ---\n{}",
            f.display(),
            String::from_utf8_lossy(&c.stderr),
            String::from_utf8_lossy(&r.stderr)
        );
        assert_eq!(
            c.status.code(),
            r.status.code(),
            "sema dump mismatch (exit status) for {}",
            f.display()
        );
    }
    assert!(
        oracle_successes > 0,
        "{} has no file the oracle succeeds on: the success-path byte \
         comparison would be vacuous",
        corpus_dir.display()
    );
    eprintln!(
        "sema differential ({corpus}): {} corpus files matched \
         ({oracle_successes} succeeded on the oracle)",
        files.len()
    );
}

#[test]
fn sema_differential_s0() {
    run_differential(ToolPair::Driver, "tests/sema_corpus", &[], &[]);
}

/// Differential for the `compile = false` entry point (`resolveASTForParser`,
/// `SemResolve.cpp:299-310`): `sema-parser-dump` vs `sema-dump
/// --parser-entry` over `tests/sema_corpus_parser`. See the module doc for
/// why this needs a separate oracle from `sema_differential_s0`.
#[test]
fn sema_parser_differential() {
    run_differential(ToolPair::Parser, "tests/sema_corpus_parser", &[], &[]);
}
