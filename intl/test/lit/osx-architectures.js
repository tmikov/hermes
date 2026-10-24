/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The check that turns the plugin off by default, and makes an explicit
// HERMES_BUILD_INTL=ON fatal, on macOS when CMAKE_OSX_ARCHITECTURES is one
// architecture other than cargo's host (cmake/modules/HermesIntl.cmake).
// It is plain CMake, so it runs on every platform.
// RUN: %cmake -P %S/Inputs/osx-architectures.cmake 2>&1 | %FileCheck --match-full-lines %s
// CHECK: [arm64] [host: aarch64-apple-darwin]: OFF
// CHECK-NEXT: [x86_64] [host: aarch64-apple-darwin]: ON
// CHECK-NEXT: [arm64] [host: x86_64-apple-darwin]: ON
// CHECK-NEXT: [x86_64] [host: x86_64-apple-darwin]: OFF
// Several architectures are the caller's other check.
// CHECK-NEXT: [arm64;x86_64] [host: aarch64-apple-darwin]: OFF
// CHECK-NEXT: [] [host: aarch64-apple-darwin]: OFF
// No host line: nothing to compare with.
// CHECK-NEXT: [arm64] []: OFF
