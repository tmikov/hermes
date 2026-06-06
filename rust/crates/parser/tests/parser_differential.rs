/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Byte-for-byte differential: parse each corpus file with the C++ hermesc
//! (`-dump-ast -dump-source-location=both`, pretty-print on by default) and
//! the Rust `ast-dump` (`--pretty --dump-source-location`), compare stdout.
//!
//! The gate is deliberately trivia-only (empty/whitespace/comments) for phase
//! P0; later parser phases extend the corpus with real JS. Skip cleanly when
//! hermesc is absent; set `REQUIRE_DIFFERENTIAL=1` to turn a missing hermesc
//! into a hard failure (used in CI).

use std::path::PathBuf;
use std::process::Command;

/// Path to the C++ hermesc oracle.
/// `CARGO_MANIFEST_DIR` is `<repo>/rust/crates/parser`; three ancestor hops
/// reach the repo root (`nth(3)`).
fn hermesc_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap()
        .join("cmake-build-asan/bin/hermesc")
}

/// Directory that holds the parser differential corpus.
fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/parser_corpus")
}

#[test]
fn parser_differential_p0() {
    let hermesc = hermesc_bin();
    if !hermesc.exists() {
        if std::env::var("REQUIRE_DIFFERENTIAL").is_ok() {
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

    let mut checked = 0usize;
    for f in &files {
        let c = Command::new(&hermesc)
            .args(["-dump-ast", "-dump-source-location=both"])
            .arg(f)
            .output()
            .expect("failed to run hermesc");
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
        checked += 1;
    }
    eprintln!("parser differential: {checked} corpus files matched");
}
