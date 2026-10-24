/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The reason configure gives when the plugin is off by default, and in the
// fatal error for HERMES_BUILD_INTL=ON (cmake/modules/HermesIntl.cmake):
// one per kind of build the plugin does not build for, none for a host build.
// RUN: %cmake -P %S/Inputs/unsupported-reason.cmake 2>&1 | %FileCheck --match-full-lines %s
// CHECK: : []
// CHECK-NEXT: CMAKE_CROSSCOMPILING: [cross builds]
// CHECK-NEXT: HERMES_IS_ANDROID: [Android]
// CHECK-NEXT: HERMES_APPLE_TARGET_PLATFORM: [Apple targets (HERMES_APPLE_TARGET_PLATFORM=iphoneos)]
// CHECK-NEXT: WIN32: [Windows]
// CHECK-NEXT: EMSCRIPTEN: [Emscripten]
// CHECK-NEXT: CMAKE_OSX_ARCHITECTURES: [several CMAKE_OSX_ARCHITECTURES (arm64;x86_64)]
// CHECK-NEXT: CMAKE_OSX_ARCHITECTURES: [CMAKE_OSX_ARCHITECTURES=x86_64, which is not the architecture cargo builds for]
// CHECK-NEXT: CMAKE_OSX_ARCHITECTURES: []
