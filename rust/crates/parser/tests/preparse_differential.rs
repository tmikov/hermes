/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Byte-for-byte differential: run every corpus .js through the C++
//! preparse-dump oracle and the Rust preparse-dump, assert byte-equal stdout.
//!
//! Two corpora:
//!   tests/parser_corpus_lazy/ — the lazy-specific corpus (Phase L1.2)
//!   tests/parser_corpus/      — the standard JS corpus for breadth coverage
//!
//! Skip cleanly when the C++ binary is absent; set REQUIRE_DIFFERENTIAL=1
//! to turn a missing binary into a hard failure (used in CI).

use std::path::{Path, PathBuf};
use std::process::Command;

/// Path to the C++ preparse-dump oracle binary.
fn cpp_bin() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../cmake-build-asan/bin/preparse-dump")
}

/// Run every `.js` file in `corpus` through both binaries and assert
/// byte-identical stdout. Returns the number of files tested.
///
/// `extra` is forwarded as additional arguments to BOTH binaries before the
/// file path (e.g. `&["--parse-flow"]`).
fn run_differential(corpus: &str, extra: &[&str]) -> usize {
    let cpp = cpp_bin();
    if !cpp.exists() {
        if std::env::var_os("REQUIRE_DIFFERENTIAL").is_some() {
            panic!(
                "REQUIRE_DIFFERENTIAL=1 but preparse-dump not built at {cpp:?}"
            );
        }
        eprintln!("skip: preparse-dump (C++) not built at {cpp:?}");
        return 0;
    }
    let rust = Path::new(env!("CARGO_BIN_EXE_preparse-dump"));
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(corpus);
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| {
            panic!("corpus dir missing {}: {e}", dir.display())
        })
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().map(|e| e == "js").unwrap_or(false))
        .collect();
    files.sort();
    if files.is_empty() {
        eprintln!(
            "preparse differential ({corpus}): no .js files (trivial pass)"
        );
        return 0;
    }
    for path in &files {
        let cpp_out = Command::new(&cpp)
            .args(extra)
            .arg(path)
            .output()
            .expect("spawn preparse-dump (C++)");
        let rust_out = Command::new(rust)
            .args(extra)
            .arg(path)
            .output()
            .expect("spawn preparse-dump (Rust)");
        let cpp_s = String::from_utf8_lossy(&cpp_out.stdout);
        let rust_s = String::from_utf8_lossy(&rust_out.stdout);
        assert_eq!(
            cpp_s,
            rust_s,
            "preparse differential mismatch on {:?}\n  C++ : {cpp_s:?}\n  \
             Rust: {rust_s:?}",
            path.file_name().unwrap()
        );
    }
    eprintln!(
        "preparse differential ({corpus}): {} files matched",
        files.len()
    );
    files.len()
}

#[test]
fn preparse_differential_lazy_corpus() {
    run_differential("tests/parser_corpus_lazy", &[]);
}

#[test]
fn preparse_differential_parser_corpus() {
    run_differential("tests/parser_corpus", &[]);
}

#[test]
fn preparse_differential_flow_corpus() {
    // Flow ambiguous grammar ON (hermesc -parse-flow defaults to ALL);
    // both binaries get the identical flag.
    run_differential("tests/parser_corpus_flow", &["--parse-flow"]);
}

#[test]
fn preparse_differential_ts_corpus() {
    run_differential("tests/parser_corpus_ts", &["--parse-ts"]);
}
