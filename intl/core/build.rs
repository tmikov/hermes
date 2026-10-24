// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

//! Generates the C header for the crate's `extern "C"` interface when CMake
//! asks for it through HERMES_INTL_CORE_HEADER. Plain `cargo test` skips it.

fn main() {
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=cbindgen.toml");
    println!("cargo:rerun-if-env-changed=HERMES_INTL_CORE_HEADER");
    let Ok(header) = std::env::var("HERMES_INTL_CORE_HEADER") else {
        return;
    };
    // A missing header counts as changed, so deleting it regenerates it.
    println!("cargo:rerun-if-changed={header}");
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    cbindgen::generate(&crate_dir)
        .expect("cbindgen failed to parse intl/core")
        .write_to_file(&header);
}
