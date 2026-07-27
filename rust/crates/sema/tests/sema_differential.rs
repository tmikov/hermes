/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Byte-for-byte differential: resolve each corpus file with the C++ hermesc
//! and the Rust `sema-dump`, comparing stdout, stderr AND exit status.
//!
//! GATE COMMAND (this test needs the `sema-dump` bin, which is behind the
//! `dump-bin` feature — a test cannot depend on its own crate's features any
//! other way, hence the `#![cfg]` below and the `--features` flag):
//!
//! ```text
//! REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml \
//!     -p sema --features dump-bin --test sema_differential -- --nocapture
//! ```
//!
//! Without `--features dump-bin` the whole file compiles away to nothing, so
//! a plain `cargo test` over the workspace stays green (and silently skips
//! this oracle).
//!
//! Flag pairing: none needed. `hermesc -dump-sema` and `sema-dump` both take
//! the file as their only argument; `-Xstd-globals` (which loads `libhermes`
//! as the ambient-declaration file) defaults to true on the hermesc side and
//! is unconditional on ours, and `-strict` defaults to false on both.
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
//! Skip cleanly when hermesc is absent; set `REQUIRE_DIFFERENTIAL=1` to turn
//! a missing hermesc into a hard failure (used in CI).

#![cfg(feature = "dump-bin")]

use std::path::PathBuf;
use std::process::Command;

/// Path to the C++ hermesc oracle (relative join from the crate manifest dir
/// `<repo>/rust/crates/sema`, matching `parser_differential.rs`).
fn hermesc_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../cmake-build-asan/bin/hermesc")
}

/// Run every `.js` file in `corpus` through hermesc (with `hermesc_extra`
/// appended to the base flags) and sema-dump (with `sema_dump_extra`
/// appended), asserting byte-identical stdout, byte-identical stderr and
/// identical exit status. Skips (or hard-fails under
/// `REQUIRE_DIFFERENTIAL=1`) when hermesc is missing.
fn run_differential(
    corpus: &str,
    hermesc_extra: &[&str],
    sema_dump_extra: &[&str],
) {
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
            "skipping sema_differential: hermesc not found at {} \
             (set REQUIRE_DIFFERENTIAL=1 to force)",
            hermesc.display()
        );
        return;
    }

    // CARGO_BIN_EXE_sema-dump is set by Cargo to the path of the sema-dump
    // binary in the current build profile. It only exists because this test
    // is compiled with the `dump-bin` feature, which is what makes the
    // `[[bin]]`'s `required-features` satisfied.
    let sema_dump = PathBuf::from(env!("CARGO_BIN_EXE_sema-dump"));
    assert!(
        sema_dump.exists(),
        "sema-dump binary not found at {}; run: cargo build \
         --manifest-path rust/Cargo.toml -p sema --features dump-bin \
         --bin sema-dump",
        sema_dump.display()
    );

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

    for f in &files {
        let c = Command::new(&hermesc)
            .args(["-dump-sema"])
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
        let r = Command::new(&sema_dump)
            .args(sema_dump_extra)
            .arg(f)
            .output()
            .expect("failed to run sema-dump");
        // Compare the raw bytes, not a lossy UTF-8 decode: two distinct
        // invalid-UTF-8 byte sequences can both map to U+FFFD and compare
        // equal as `String`s while still being a real byte-for-byte
        // divergence. The lossy strings are only for the assert message.
        assert!(
            c.stdout == r.stdout,
            "sema dump mismatch (stdout) for {}:\n--- hermesc ---\n{}\n\
             --- sema-dump ---\n{}",
            f.display(),
            String::from_utf8_lossy(&c.stdout),
            String::from_utf8_lossy(&r.stdout)
        );
        assert!(
            c.stderr == r.stderr,
            "sema dump mismatch (stderr) for {}:\n--- hermesc ---\n{}\n\
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
    eprintln!(
        "sema differential ({corpus}): {} corpus files matched",
        files.len()
    );
}

#[test]
fn sema_differential_s0() {
    run_differential("tests/sema_corpus", &[], &[]);
}
