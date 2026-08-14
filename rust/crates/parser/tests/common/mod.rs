/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Shared helper for the differential tests that shell out to a Rust CLI
//! driver (`ast-dump`, `json-parse-dump`, `preparse-dump`, `sema-dump`).
//!
//! This file is kept BYTE-IDENTICAL in `rust/crates/parser/tests/common/mod.rs`
//! and `rust/crates/sema/tests/common/mod.rs` — both crates' differentials need
//! it, and a `tests/` helper cannot be shared across packages without adding a
//! crate for it. `diff` the two after touching either; the identity is also
//! enforced by `common_helper_copies_are_byte_identical` in the unpublished
//! `tools` crate (`rust/crates/tools/tests/common_copies_identical.rs`).
//!
//! Those drivers live in the unpublished `tools` crate, not in the crate under
//! test, so Cargo's `CARGO_BIN_EXE_<name>` — which is only defined for binaries
//! of the *same* package — is not available here. `tools_bin` restores both
//! halves of what that env var gave us:
//!
//! 1. **The build guarantee.** Cargo builds a package's own bins before running
//!    its tests, but it does not build another package's bins. So we run a
//!    nested `cargo build -p tools --bin <name>` before the first use in a test
//!    process (memoised, so the 8 corpus tests in one binary build once).
//! 2. **The path.** The build is asked for `--message-format=json`, and the
//!    executable path is read out of the `compiler-artifact` line. That is
//!    exact under any profile, `CARGO_TARGET_DIR`, or `--target`, unlike
//!    guessing the layout from `current_exe`.
//!
//! A nested cargo invocation is safe here: `cargo test` releases the build
//! directory lock before it runs test executables, and concurrent calls from
//! parallel test threads are serialised by the memo mutex.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, OnceLock};

/// Memo of already-built binaries, keyed by bin name.
fn cache() -> &'static Mutex<HashMap<String, PathBuf>> {
    static CACHE: OnceLock<Mutex<HashMap<String, PathBuf>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Build the `tools` binary `name` (once per test process) and return its
/// absolute path. Panics with the cargo diagnostics if the build fails.
pub fn tools_bin(name: &str) -> PathBuf {
    let mut cache = cache().lock().expect("tools_bin cache poisoned");
    if let Some(p) = cache.get(name) {
        return p.clone();
    }

    // The workspace manifest, reached from `<repo>/rust/crates/<this crate>`.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
    // `CARGO` is set by cargo when it runs a test binary; fall back for a
    // hand-run test executable.
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let out = Command::new(cargo)
        .arg("build")
        .arg("--manifest-path")
        .arg(&manifest)
        .args(["-p", "tools", "--bin", name])
        // Same profile the test itself was built with.
        .args(if cfg!(debug_assertions) {
            &[][..]
        } else {
            &["--release"][..]
        })
        // json-render-diagnostics keeps human-readable errors on stderr while
        // stdout carries the machine-readable artifact records.
        .arg("--message-format=json-render-diagnostics")
        .output()
        .expect("failed to spawn cargo to build the tools crate");
    assert!(
        out.status.success(),
        "cargo build -p tools --bin {name} failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Exactly one artifact of this build has a non-null `executable`: the bin
    // we asked for. Extract it without pulling in a JSON dependency.
    let stdout = String::from_utf8_lossy(&out.stdout);
    let path = stdout
        .lines()
        .find_map(|line| {
            let rest = line.split("\"executable\":\"").nth(1)?;
            let path = rest.split('"').next()?;
            (!path.is_empty()).then(|| PathBuf::from(path))
        })
        .unwrap_or_else(|| {
            panic!("no executable path in `cargo build -p tools --bin {name}` output")
        });
    assert!(
        path.exists(),
        "tools binary {name} reported at {} but missing",
        path.display()
    );

    cache.insert(name.to_string(), path.clone());
    path
}
