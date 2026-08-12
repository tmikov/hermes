/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Byte-for-byte differential: parse each corpus file with the C++
//! `json-parse-dump` oracle and the Rust `json-parse-dump`, compare stdout.
//!
//! `err_deep_nesting.json` pins the value-nesting limit both sides gained in
//! upstream `b21856de4` (see `json/parser.rs`'s module doc). Its 2000 levels
//! are past the limit in EVERY build profile — 128 for an ASan C++ /
//! debug Rust build, 1024 for a release one — so, unlike the JS parser's
//! recursion boundary, this file is profile-INSENSITIVE and safe to keep in
//! a differential corpus: all four pairings produce the same `ERROR 1`. The
//! exact trip depth is pinned instead by
//! `parser/tests/upstream_defect_fixes.rs`.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

mod common;

fn cpp_bin() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../cmake-build-asan/bin/json-parse-dump")
}

fn run(bin: &Path, src: &[u8]) -> String {
    use std::io::Write;
    let mut child = Command::new(bin)
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn json-parse-dump");
    child.stdin.take().unwrap().write_all(src).unwrap();
    let out = child.wait_with_output().unwrap();
    String::from_utf8(out.stdout).expect("utf-8 stdout")
}

#[test]
fn json_corpus_differential() {
    let cpp = cpp_bin();
    if !cpp.exists() {
        if std::env::var_os("REQUIRE_DIFFERENTIAL").is_some() {
            panic!(
                "REQUIRE_DIFFERENTIAL=1 but json-parse-dump not built at {cpp:?}"
            );
        }
        eprintln!("skip: json-parse-dump (C++) not built at {cpp:?}");
        return;
    }
    // The Rust driver lives in the unpublished `tools` crate; `tools_bin`
    // builds it and returns its path (see tests/common/mod.rs).
    let rust = common::tools_bin("json-parse-dump");
    let dir =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/json_corpus");
    let mut count = 0;
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("read json_corpus dir")
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.extension().and_then(|e| e.to_str()) == Some("json")
        })
        .collect();
    files.sort();
    for path in files {
        let src = std::fs::read(&path).unwrap();
        let cpp_out = run(&cpp, &src);
        let rust_out = run(&rust, &src);
        assert_eq!(
            cpp_out,
            rust_out,
            "differential mismatch on {:?}\n  C++ : {cpp_out:?}\n  Rust: {rust_out:?}",
            path.file_name().unwrap()
        );
        count += 1;
    }
    eprintln!("json differential: {count} corpus files matched");
    assert!(count > 0, "no corpus files found");
}
