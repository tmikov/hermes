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
//!
//! The gate is deliberately trivia-only (empty/whitespace/comments) for phase
//! P0; later parser phases extend the corpus with real JS. Skip cleanly when
//! hermesc is absent; set `REQUIRE_DIFFERENTIAL=1` to turn a missing hermesc
//! into a hard failure (used in CI).

use std::path::PathBuf;
use std::process::Command;

/// Path to the C++ hermesc oracle (sibling style: relative join from the crate
/// manifest dir `<repo>/rust/crates/parser`).
fn hermesc_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../cmake-build-asan/bin/hermesc")
}

/// Directory that holds the parser differential corpus.
fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/parser_corpus")
}

#[test]
fn parser_differential_p0() {
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

    let mut files: Vec<PathBuf> = std::fs::read_dir(corpus_dir())
        .expect("parser_corpus dir missing")
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().map(|e| e == "js").unwrap_or(false))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "parser_corpus is empty");

    for f in &files {
        let c = Command::new(&hermesc)
            .args(["-dump-ast", "-dump-source-location=both"])
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
            .args(["--pretty", "--dump-source-location"])
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
    eprintln!("parser differential: {} corpus files matched", files.len());
}
