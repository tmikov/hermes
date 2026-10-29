/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The Apple framework build of the plugin: the architectures each slice's
// plugin is built for (cmake/modules/HermesIntl.cmake: the tvOS and visionOS
// simulator plugins are arm64 only, while hermesvm keeps x86_64; their
// xcframework slices still list x86_64, which apple-xcframework.js tests), the
// LC_RPATH that finds hermesvm.framework from the shallow and deep bundles,
// and the Rust targets utils/build-apple-framework.sh asks for per slice.
// It tests the helpers in cmake/modules/HermesIntl.cmake and the script's
// mapping, not how intl/CMakeLists.txt applies them to the framework build,
// which only an Apple build shows. It is plain CMake and bash, so it runs
// on every platform.
// RUN: %cmake -P %S/Inputs/apple-framework.cmake 2>&1 | %FileCheck --match-full-lines %s
// RUN: bash %S/Inputs/apple-slices.sh %cmake 2>&1 | %FileCheck --check-prefix=SCRIPT --match-full-lines %s

// Only the tvOS and visionOS simulators change the architectures.
// CHECK: archs macosx x86_64;arm64: []
// CHECK-NEXT: archs iphoneos arm64: []
// CHECK-NEXT: archs iphonesimulator x86_64;arm64: []
// CHECK-NEXT: archs catalyst x86_64;arm64: []
// CHECK-NEXT: archs appletvos arm64: []
// CHECK-NEXT: archs appletvsimulator x86_64;arm64: [arm64]
// CHECK-NEXT: archs xros arm64: []
// CHECK-NEXT: archs xrsimulator x86_64;arm64: [arm64]
// CHECK-NEXT: archs appletvsimulator arm64: []
// CHECK-NEXT: archs xrsimulator arm64;x86_64: [arm64]
// CHECK-NEXT: archs macos host: []
// CHECK-NEXT: archs override other order: [arm64;x86_64]
// CHECK-NEXT: archs not apple: []
// CHECK-NEXT: archs none: []

// Deep bundles (macOS, Mac Catalyst, a plain macOS framework build) and
// shallow ones.
// CHECK-NEXT: rpath []: @loader_path/../../..
// CHECK-NEXT: rpath [macosx]: @loader_path/../../..
// CHECK-NEXT: rpath [catalyst]: @loader_path/../../..
// CHECK-NEXT: rpath [iphoneos]: @loader_path/..
// CHECK-NEXT: rpath [iphonesimulator]: @loader_path/..
// CHECK-NEXT: rpath [appletvos]: @loader_path/..
// CHECK-NEXT: rpath [appletvsimulator]: @loader_path/..
// CHECK-NEXT: rpath [xros]: @loader_path/..
// CHECK-NEXT: rpath [xrsimulator]: @loader_path/..

// SCRIPT: macosx: x86_64-apple-darwin aarch64-apple-darwin
// SCRIPT-NEXT: iphoneos: aarch64-apple-ios
// SCRIPT-NEXT: iphonesimulator: x86_64-apple-ios aarch64-apple-ios-sim
// SCRIPT-NEXT: catalyst: x86_64-apple-ios-macabi aarch64-apple-ios-macabi
// SCRIPT-NEXT: xros: aarch64-apple-visionos
// SCRIPT-NEXT: xrsimulator: aarch64-apple-visionos-sim
// SCRIPT-NEXT: appletvos: aarch64-apple-tvos
// SCRIPT-NEXT: appletvsimulator: aarch64-apple-tvos-sim
// SCRIPT-NOT: {{.}}
