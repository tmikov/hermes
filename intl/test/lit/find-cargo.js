/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// Where the CMake build (hermes_intl_find_cargo in
// cmake/modules/HermesIntl.cmake) and utils/build-apple-framework.sh find
// cargo: on PATH, else in $CARGO_HOME/bin, else in ~/.cargo/bin, which a
// build launched from a GUI (a Gradle daemon, an Xcode build phase) may not
// have on PATH. The script runs the rustc next to the cargo it found, and
// both name the places searched when there is none. Fake cargos in a
// scratch directory, so it runs on every platform with bash.
// RUN: bash %S/Inputs/find-cargo.sh %cmake %t 2>&1 | %FileCheck --match-full-lines %s

// CHECK: cmake, on PATH:
// CHECK-NEXT: cargo: <t>/pathdir/cargo, host: [path-host]
// CHECK-NEXT: searched: on PATH or in <t>/cargohome/bin or <t>/home/.cargo/bin
// CHECK-NEXT: script, on PATH:
// CHECK-NEXT: cargo: [<t>/pathdir/cargo]
// CHECK-NEXT: check passed
// CHECK-NEXT: rustc: <t>/pathdir/rustc
// CHECK-NEXT: cmake, CARGO_HOME:
// CHECK-NEXT: cargo: <t>/cargohome/bin/cargo, host: [cargo-home-host]
// CHECK-NEXT: searched: on PATH or in <t>/cargohome/bin or <t>/home/.cargo/bin
// CHECK-NEXT: script, CARGO_HOME:
// CHECK-NEXT: cargo: [<t>/cargohome/bin/cargo]
// CHECK-NEXT: check passed
// CHECK-NEXT: rustc: <t>/cargohome/bin/rustc
// CHECK-NEXT: cmake, HOME:
// CHECK-NEXT: cargo: <t>/home/.cargo/bin/cargo, host: [home-host]
// CHECK-NEXT: searched: on PATH or in <t>/home/.cargo/bin
// CHECK-NEXT: script, HOME:
// CHECK-NEXT: cargo: [<t>/home/.cargo/bin/cargo]
// CHECK-NEXT: check passed
// CHECK-NEXT: rustc: <t>/home/.cargo/bin/rustc
// CHECK-NEXT: cmake, CARGO_HOME without cargo:
// CHECK-NEXT: cargo: <t>/home/.cargo/bin/cargo, host: [home-host]
// CHECK-NEXT: searched: on PATH or in <t>/nocargo/bin or <t>/home/.cargo/bin
// CHECK-NEXT: script, CARGO_HOME without cargo:
// CHECK-NEXT: cargo: [<t>/home/.cargo/bin/cargo]
// CHECK-NEXT: check passed
// CHECK-NEXT: rustc: <t>/home/.cargo/bin/rustc
// CHECK-NEXT: cmake, none:
// CHECK-NEXT: cargo: HERMES_INTL_CARGO-NOTFOUND, host: []
// CHECK-NEXT: searched: on PATH or in <t>/nocargo/bin or <t>/nohome/.cargo/bin
// CHECK-NEXT: script, none:
// CHECK-NEXT: cargo: []
// CHECK-NEXT: error: the Intl plugin (hermesintl.framework) needs Rust, but cargo was not found on PATH or in <t>/nocargo/bin or <t>/nohome/.cargo/bin.
// CHECK-NEXT: Install Rust from https://rustup.rs, then run: rustup target add x86_64-apple-darwin aarch64-apple-darwin
// CHECK-NEXT: cmake, none, no CARGO_HOME or HOME:
// CHECK-NEXT: cargo: HERMES_INTL_CARGO-NOTFOUND, host: []
// CHECK-NEXT: searched: on PATH
// CHECK-NEXT: script, none, no CARGO_HOME or HOME:
// CHECK-NEXT: cargo: []
// CHECK-NEXT: error: the Intl plugin (hermesintl.framework) needs Rust, but cargo was not found on PATH.
// CHECK-NEXT: Install Rust from https://rustup.rs, then run: rustup target add x86_64-apple-darwin aarch64-apple-darwin
