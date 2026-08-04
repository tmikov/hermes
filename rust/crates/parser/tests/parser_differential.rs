/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Byte-for-byte differential: parse each corpus file with the C++ hermesc and
//! the Rust `ast-dump`, compare stdout.
//!
//! Flag pairing (must stay aligned or the bytes diverge for non-trivial input):
//! - hermesc `-dump-source-location=both` = emit both `loc` and `range`, which
//!   is what the Rust side emits with `--dump-source-location` (LocAndRange).
//! - hermesc pretty-prints by default (`-pretty` init(true)), so the Rust side
//!   passes `--pretty`; no flag is needed on the hermesc side.
//! - hermesc's `-Xinclude-raw-ast-prop` defaults to true, so it always emits the
//!   `"raw"` field (e.g. on NumericLiteral); the Rust dumper omits it unless
//!   `--include-raw-ast-prop` is passed, so the harness always passes it.
//!
//! Two corpora:
//! - `tests/parser_corpus`: standard JS, no extra flags (phases P0-P4).
//! - `tests/parser_corpus_flow`: Flow type syntax; hermesc gets `-parse-flow`
//!   and ast-dump gets `--parse-flow` (phase P5).
//!
//! The error side of the recursion-depth boundary (the "Too many nested …"
//! diagnostics) is deliberately not pinned in this corpus — it is profile-
//! sensitive the same way `sema_differential.rs`'s BUILD-PROFILE PAIRING note
//! describes, and is pinned instead by `sema_corpus/nested-expressions.js`
//! and `parser/tests/recursion_depth_limit.rs`; the clean side alone lives
//! here, in `parser_corpus/nested-parens-limit.js`.
//!
//! Skip cleanly when hermesc is absent; set `REQUIRE_DIFFERENTIAL=1` to turn a
//! missing hermesc into a hard failure (used in CI).

use std::path::PathBuf;
use std::process::Command;

/// Path to the C++ hermesc oracle (sibling style: relative join from the crate
/// manifest dir `<repo>/rust/crates/parser`).
fn hermesc_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../cmake-build-asan/bin/hermesc")
}

/// Run every `.js` file in `corpus` through hermesc (with `hermesc_extra`
/// appended to the base flags) and ast-dump (with `ast_dump_extra` appended),
/// asserting byte-identical stdout. Skips (or hard-fails under
/// `REQUIRE_DIFFERENTIAL=1`) when hermesc is missing.
fn run_differential(corpus: &str, hermesc_extra: &[&str], ast_dump_extra: &[&str]) {
    let hermesc = hermesc_bin();
    if !hermesc.exists() {
        if std::env::var_os("REQUIRE_DIFFERENTIAL").is_some() {
            panic!(
                "REQUIRE_DIFFERENTIAL set but hermesc not found at {}; \
                 build: cmake --build cmake-build-asan --target hermesc",
                hermesc.display()
            );
        }
        eprintln!(
            "skipping parser_differential: hermesc not found at {} \
             (set REQUIRE_DIFFERENTIAL=1 to force)",
            hermesc.display()
        );
        return;
    }

    // CARGO_BIN_EXE_ast-dump is set by Cargo to the path of the ast-dump
    // binary in the current build profile, exactly like json_differential.rs
    // uses CARGO_BIN_EXE_json-parse-dump.
    let ast_dump = PathBuf::from(env!("CARGO_BIN_EXE_ast-dump"));
    assert!(
        ast_dump.exists(),
        "ast-dump binary not found at {}; run: cargo build --manifest-path rust/Cargo.toml -p parser --bin ast-dump",
        ast_dump.display()
    );

    let corpus_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(corpus);
    let mut files: Vec<PathBuf> = std::fs::read_dir(&corpus_dir)
        .unwrap_or_else(|e| panic!("{} dir missing: {e}", corpus_dir.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().map(|e| e == "js").unwrap_or(false))
        .collect();
    files.sort();
    // An empty corpus dir is a trivial pass: some feature corpora are populated
    // in later sub-tasks (P6.3/6.4/6.5), but the dir + test exist from P6.0 so
    // the harness is wired. A `.gitkeep` keeps the empty dir in git.
    if files.is_empty() {
        eprintln!("parser differential ({corpus}): no .js files (trivial pass)");
        return;
    }

    for f in &files {
        let c = Command::new(&hermesc)
            .args(["-dump-ast", "-dump-source-location=both"])
            .args(hermesc_extra)
            .arg(f)
            .output()
            .expect("failed to run hermesc");
        // Guard against a silent oracle failure (e.g. a future flag rename):
        // a nonzero hermesc exit would otherwise just empty its stdout.
        assert!(
            c.status.success(),
            "hermesc failed on {}:\n{}",
            f.display(),
            String::from_utf8_lossy(&c.stderr)
        );
        let r = Command::new(&ast_dump)
            .args(["--pretty", "--dump-source-location", "--include-raw-ast-prop"])
            .args(ast_dump_extra)
            .arg(f)
            .output()
            .expect("failed to run ast-dump");
        assert_eq!(
            String::from_utf8_lossy(&c.stdout),
            String::from_utf8_lossy(&r.stdout),
            "AST dump mismatch for {}",
            f.display()
        );
    }
    eprintln!(
        "parser differential ({corpus}): {} corpus files matched",
        files.len()
    );
}

#[test]
fn parser_differential_p0() {
    run_differential("tests/parser_corpus", &[], &[]);
}

#[test]
fn parser_differential_flow() {
    run_differential("tests/parser_corpus_flow", &["-parse-flow"], &["--parse-flow"]);
}

#[test]
fn parser_differential_flow_component() {
    run_differential(
        "tests/parser_corpus_flow_component",
        &["-parse-flow", "-Xparse-component-syntax"],
        &["--parse-flow", "--parse-component-syntax"],
    );
}

#[test]
fn parser_differential_flow_records() {
    run_differential(
        "tests/parser_corpus_flow_records",
        &["-parse-flow", "-Xparse-flow-records"],
        &["--parse-flow", "--parse-flow-records"],
    );
}

#[test]
fn parser_differential_flow_match() {
    run_differential(
        "tests/parser_corpus_flow_match",
        &["-parse-flow", "-Xparse-flow-match"],
        &["--parse-flow", "--parse-flow-match"],
    );
}

#[test]
fn parser_differential_ts() {
    run_differential("tests/parser_corpus_ts", &["-parse-ts"], &["--parse-ts"]);
}

#[test]
fn parser_differential_jsx() {
    run_differential("tests/parser_corpus_jsx", &["-parse-jsx"], &["--parse-jsx"]);
}

#[test]
fn parser_differential_jsx_flow() {
    // JSX with Flow type-arguments on an opening tag (jsx.cpp:124-132). This is
    // the only JSX production not reachable under standalone `-parse-jsx`, so it
    // gets its own corpus run with BOTH flags enabled.
    run_differential(
        "tests/parser_corpus_jsx_flow",
        &["-parse-jsx", "-parse-flow"],
        &["--parse-jsx", "--parse-flow"],
    );
}
