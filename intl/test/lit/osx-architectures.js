/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The Rust triples of a macOS host build (cmake/modules/HermesIntl.cmake):
// one per CMAKE_OSX_ARCHITECTURES entry, whatever the Mac's own
// architecture, or the host triple from `cargo -vV` when it is empty.
// It is plain CMake, so it runs on every platform.
// RUN: %cmake -P %S/Inputs/osx-architectures.cmake 2>&1 | %FileCheck --match-full-lines %s
// CHECK: [] [host: aarch64-apple-darwin]: [aarch64-apple-darwin] []
// CHECK-NEXT: [arm64] [host: aarch64-apple-darwin]: [aarch64-apple-darwin] []
// CHECK-NEXT: [x86_64] [host: aarch64-apple-darwin]: [x86_64-apple-darwin] []
// CHECK-NEXT: [arm64;x86_64] [host: aarch64-apple-darwin]: [aarch64-apple-darwin;x86_64-apple-darwin] []
// CHECK-NEXT: [x86_64;arm64] [host: aarch64-apple-darwin]: [x86_64-apple-darwin;aarch64-apple-darwin] []
// CHECK-NEXT: [] [host: x86_64-apple-darwin]: [x86_64-apple-darwin] []
// CHECK-NEXT: [arm64] [host: x86_64-apple-darwin]: [aarch64-apple-darwin] []
// CHECK-NEXT: [x86_64] [host: x86_64-apple-darwin]: [x86_64-apple-darwin] []
// CHECK-NEXT: [arm64;x86_64] [host: x86_64-apple-darwin]: [aarch64-apple-darwin;x86_64-apple-darwin] []
// CHECK-NEXT: [x86_64;arm64] [host: x86_64-apple-darwin]: [x86_64-apple-darwin;aarch64-apple-darwin] []
// No host line: no triple; intl/CMakeLists.txt reports the missing cargo.
// CHECK-NEXT: [] [host: ]: [] []
// CHECK-NEXT: [arm64;x86_64] [host: ]: [aarch64-apple-darwin;x86_64-apple-darwin] []
// A host line with trailing spaces at the end of the output.
// CHECK-NEXT: [] [host: x86_64-apple-darwin]: [x86_64-apple-darwin] []

