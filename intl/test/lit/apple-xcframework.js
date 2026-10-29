/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// utils/intl-xcframework-sim-slices.py, which utils/build-apple-framework.sh
// runs on hermesintl.xcframework: the plugin's tvOS and visionOS simulator
// slices are arm64 only, but list x86_64 and take hermesvm's slice names, so
// an app's x86_64 simulator build through CocoaPods finds and embeds them
// (the linker ignores the arm64 binary for x86_64, and an x86_64 process
// finds no usable plugin, which the loader treats as no plugin). Every other
// slice is untouched, a second run changes nothing, and an entry the script
// does not expect, or a malformed Info.plist, fails it without changing
// anything. It is plain Python on a fake xcframework, so it runs on every
// platform.
// RUN: bash %S/Inputs/apple-xcframework.sh %python %t 2>&1 | %FileCheck --match-full-lines %s

// CHECK: first run:
// CHECK-NEXT: hermesintl.xcframework: tvos-arm64-simulator -> tvos-arm64_x86_64-simulator (lists x86_64; the binary is arm64 only)
// CHECK-NEXT: hermesintl.xcframework: xros-arm64-simulator -> xros-arm64_x86_64-simulator (lists x86_64; the binary is arm64 only)
// CHECK-NEXT: entry ios-arm64 arm64 ios - hermesintl.framework/hermesintl from ios-arm64
// CHECK-NEXT: entry ios-arm64_x86_64-maccatalyst arm64,x86_64 ios maccatalyst hermesintl.framework/Versions/1/hermesintl from ios-arm64_x86_64-maccatalyst
// CHECK-NEXT: entry ios-arm64_x86_64-simulator arm64,x86_64 ios simulator hermesintl.framework/hermesintl from ios-arm64_x86_64-simulator
// CHECK-NEXT: entry macos-arm64_x86_64 arm64,x86_64 macos - hermesintl.framework/Versions/1/hermesintl from macos-arm64_x86_64
// CHECK-NEXT: entry tvos-arm64 arm64 tvos - hermesintl.framework/hermesintl from tvos-arm64
// CHECK-NEXT: entry tvos-arm64_x86_64-simulator arm64,x86_64 tvos simulator hermesintl.framework/hermesintl from tvos-arm64-simulator
// CHECK-NEXT: entry xros-arm64 arm64 xros - hermesintl.framework/hermesintl from xros-arm64
// CHECK-NEXT: entry xros-arm64_x86_64-simulator arm64,x86_64 xros simulator hermesintl.framework/hermesintl from xros-arm64-simulator
// CHECK-NEXT: dirs ios-arm64 ios-arm64_x86_64-maccatalyst ios-arm64_x86_64-simulator macos-arm64_x86_64 tvos-arm64 tvos-arm64_x86_64-simulator xros-arm64 xros-arm64_x86_64-simulator

// CHECK-NEXT: second run:
// CHECK-NEXT: second run changed nothing

// CHECK-NEXT: valid tvOS entry, unexpected visionOS entry:
// CHECK-NEXT: error: {{.*}}bad/hermesintl.xcframework: unexpected xros simulator entry xros-arm64e-simulator (arm64e); expected xros-arm64-simulator (arm64)
// CHECK-NEXT: exit status 1
// CHECK-NEXT: xcframework unchanged

// CHECK-NEXT: malformed Info.plist:
// CHECK-NEXT: error: cannot read {{.*}}malformed/hermesintl.xcframework/Info.plist: {{.+}}
// CHECK-NEXT: exit status 1
// CHECK-NEXT: xcframework unchanged
// CHECK-NOT: {{.}}
