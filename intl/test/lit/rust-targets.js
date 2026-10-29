/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The Rust target triples, deployment-target variable and reason that
// hermes_intl_rust_targets (cmake/modules/HermesIntl.cmake) gives for each
// kind of build; which builds need the host's hermesc; which lists of
// triples can be built together; the native libraries per triple; and the
// cargo profile per configuration. It is plain CMake, so it runs on every
// platform.
// RUN: %cmake -P %S/Inputs/rust-targets.cmake 2>&1 | %FileCheck --match-full-lines %s
// Host builds: the host triple from cargo -vV; macOS per architecture.
// CHECK: linux host: [x86_64-unknown-linux-gnu] [] []
// CHECK-NEXT: linux host, no cargo: [] [] []
// CHECK-NEXT: macos: [aarch64-apple-darwin] [MACOSX_DEPLOYMENT_TARGET] []
// CHECK-NEXT: macos arm64: [aarch64-apple-darwin] [MACOSX_DEPLOYMENT_TARGET] []
// CHECK-NEXT: macos x86_64: [x86_64-apple-darwin] [MACOSX_DEPLOYMENT_TARGET] []
// CHECK-NEXT: macos arm64;x86_64: [aarch64-apple-darwin;x86_64-apple-darwin] [MACOSX_DEPLOYMENT_TARGET] []
// CHECK-NEXT: macos arm64e: [] [] [macOS has no Rust target for architecture arm64e; set HERMES_INTL_RUST_TARGET to the Rust target triple]
// Android, by ABI.
// CHECK-NEXT: android arm64-v8a: [aarch64-linux-android] [] []
// CHECK-NEXT: android armeabi-v7a: [armv7-linux-androideabi] [] []
// CHECK-NEXT: android x86: [i686-linux-android] [] []
// CHECK-NEXT: android x86_64: [x86_64-linux-android] [] []
// CHECK-NEXT: android riscv64: [] [] [Android ABI 'riscv64' has no Rust target mapping; set HERMES_INTL_RUST_TARGET to the Rust target triple]
// CHECK-NEXT: android HERMES_IS_ANDROID, CMAKE_ANDROID_ARCH_ABI: [x86_64-linux-android] [] []
// Apple target platforms. The tvOS and visionOS simulators have no x86_64
// triple, so x86_64;arm64 gives only the arm64 one. With no architectures
// and no cargo, there is no triple and no reason: intl/ reports the
// missing cargo.
// CHECK-NEXT: macosx arm64: [aarch64-apple-darwin] [MACOSX_DEPLOYMENT_TARGET] []
// CHECK-NEXT: macosx x86_64,arm64: [x86_64-apple-darwin;aarch64-apple-darwin] [MACOSX_DEPLOYMENT_TARGET] []
// CHECK-NEXT: iphoneos arm64: [aarch64-apple-ios] [IPHONEOS_DEPLOYMENT_TARGET] []
// CHECK-NEXT: iphoneos x86_64,arm64: [] [] [HERMES_APPLE_TARGET_PLATFORM=iphoneos has no Rust target for architecture x86_64; set HERMES_INTL_RUST_TARGET to the Rust target triple]
// CHECK-NEXT: iphonesimulator arm64: [aarch64-apple-ios-sim] [IPHONEOS_DEPLOYMENT_TARGET] []
// CHECK-NEXT: iphonesimulator x86_64,arm64: [x86_64-apple-ios;aarch64-apple-ios-sim] [IPHONEOS_DEPLOYMENT_TARGET] []
// CHECK-NEXT: catalyst arm64: [aarch64-apple-ios-macabi] [IPHONEOS_DEPLOYMENT_TARGET] []
// CHECK-NEXT: catalyst x86_64,arm64: [x86_64-apple-ios-macabi;aarch64-apple-ios-macabi] [IPHONEOS_DEPLOYMENT_TARGET] []
// CHECK-NEXT: appletvos arm64: [aarch64-apple-tvos] [TVOS_DEPLOYMENT_TARGET] []
// CHECK-NEXT: appletvos x86_64,arm64: [] [] [HERMES_APPLE_TARGET_PLATFORM=appletvos has no Rust target for architecture x86_64; set HERMES_INTL_RUST_TARGET to the Rust target triple]
// CHECK-NEXT: appletvsimulator arm64: [aarch64-apple-tvos-sim] [TVOS_DEPLOYMENT_TARGET] []
// CHECK-NEXT: appletvsimulator x86_64,arm64: [aarch64-apple-tvos-sim] [TVOS_DEPLOYMENT_TARGET] []
// CHECK-NEXT: xros arm64: [aarch64-apple-visionos] [XROS_DEPLOYMENT_TARGET] []
// CHECK-NEXT: xros x86_64,arm64: [] [] [HERMES_APPLE_TARGET_PLATFORM=xros has no Rust target for architecture x86_64; set HERMES_INTL_RUST_TARGET to the Rust target triple]
// CHECK-NEXT: xrsimulator arm64: [aarch64-apple-visionos-sim] [XROS_DEPLOYMENT_TARGET] []
// CHECK-NEXT: xrsimulator x86_64,arm64: [aarch64-apple-visionos-sim] [XROS_DEPLOYMENT_TARGET] []
// CHECK-NEXT: iphonesimulator, no archs, x86_64 host: [x86_64-apple-ios] [IPHONEOS_DEPLOYMENT_TARGET] []
// CHECK-NEXT: iphonesimulator, no archs, no cargo: [] [] []
// CHECK-NEXT: iphonesimulator, no archs, riscv64 host: [] [] [HERMES_APPLE_TARGET_PLATFORM=iphonesimulator has no Rust target for architecture 'riscv64gc-unknown-linux-gnu'; set HERMES_INTL_RUST_TARGET to the Rust target triple]
// CHECK-NEXT: appletvsimulator x86_64: [] [] [HERMES_APPLE_TARGET_PLATFORM=appletvsimulator has no Rust target for x86_64; set HERMES_INTL_RUST_TARGET to the Rust target triple]
// CHECK-NEXT: unknown apple platform: [] [] [HERMES_APPLE_TARGET_PLATFORM=watchos has no Rust target for architecture arm64; set HERMES_INTL_RUST_TARGET to the Rust target triple]
// Windows with the MSVC ABI: the compiler's architecture, else the VS
// platform, else the system processor. MinGW gets a reason.
// CHECK-NEXT: windows CMAKE_SYSTEM_PROCESSOR AMD64: [x86_64-pc-windows-msvc] [] []
// CHECK-NEXT: windows CMAKE_SYSTEM_PROCESSOR arm64: [aarch64-pc-windows-msvc] [] []
// CHECK-NEXT: windows CMAKE_VS_PLATFORM_NAME x64: [x86_64-pc-windows-msvc] [] []
// CHECK-NEXT: windows CMAKE_VS_PLATFORM_NAME ARM64: [aarch64-pc-windows-msvc] [] []
// CHECK-NEXT: windows compiler x64: [x86_64-pc-windows-msvc] [] []
// CHECK-NEXT: windows compiler ARM64: [aarch64-pc-windows-msvc] [] []
// CHECK-NEXT: windows Win32: [] [] [Windows architecture 'Win32' has no Rust target mapping; set HERMES_INTL_RUST_TARGET to the Rust target triple]
// CHECK-NEXT: windows clang simulating MSVC: [x86_64-pc-windows-msvc] [] []
// CHECK-NEXT: windows mingw: [] [] [Windows builds without the MSVC ABI have no Rust target mapping; set HERMES_INTL_RUST_TARGET to the Rust target triple]
// Emscripten (permanent), and a cross build nothing maps.
// CHECK-NEXT: emscripten: [] [] [the plugin is not built for Emscripten; embedders can register a plugin with registerIntlPlugin]
// CHECK-NEXT: linux arm cross: [] [] [a cross build for Linux arm has no Rust target mapping; set HERMES_INTL_RUST_TARGET to the Rust target triple]
// HERMES_INTL_RUST_TARGET wins over every rule except Emscripten.
// CHECK-NEXT: override linux host: [aarch64-unknown-linux-gnu] [] []
// CHECK-NEXT: override macos: [aarch64-unknown-linux-gnu] [] []
// CHECK-NEXT: override android riscv64: [aarch64-unknown-linux-gnu] [] []
// CHECK-NEXT: override unknown apple platform: [aarch64-unknown-linux-gnu] [] []
// CHECK-NEXT: override windows Win32: [aarch64-unknown-linux-gnu] [] []
// CHECK-NEXT: override emscripten: [] [] [the plugin is not built for Emscripten; embedders can register a plugin with registerIntlPlugin]
// CHECK-NEXT: override linux arm cross: [aarch64-unknown-linux-gnu] [] []
// CHECK-NEXT: override list: [x86_64-apple-ios;aarch64-apple-ios-sim] [IPHONEOS_DEPLOYMENT_TARGET] []
// Builds whose own hermesc cannot run here: cross builds, Apple target
// platforms other than macosx, and macOS without the host architecture.
// CHECK-NEXT: hermesc linux host: []
// CHECK-NEXT: hermesc linux cross: [a cross build]
// CHECK-NEXT: hermesc android: [a cross build]
// CHECK-NEXT: hermesc macos: []
// CHECK-NEXT: hermesc macos arm64: []
// CHECK-NEXT: hermesc macos x86_64: [a build for CMAKE_OSX_ARCHITECTURES=x86_64 on arm64]
// CHECK-NEXT: hermesc macos x86_64 on an x86_64 mac: []
// CHECK-NEXT: hermesc macos arm64 on an x86_64 mac: [a build for CMAKE_OSX_ARCHITECTURES=arm64 on x86_64]
// CHECK-NEXT: hermesc macos x86_64,arm64: []
// CHECK-NEXT: hermesc macosx x86_64,arm64: []
// CHECK-NEXT: hermesc iphoneos x86_64,arm64: [HERMES_APPLE_TARGET_PLATFORM=iphoneos]
// CHECK-NEXT: hermesc iphonesimulator x86_64,arm64: [HERMES_APPLE_TARGET_PLATFORM=iphonesimulator]
// CHECK-NEXT: hermesc catalyst x86_64,arm64: [HERMES_APPLE_TARGET_PLATFORM=catalyst]
// CHECK-NEXT: hermesc appletvos x86_64,arm64: [HERMES_APPLE_TARGET_PLATFORM=appletvos]
// CHECK-NEXT: hermesc appletvsimulator x86_64,arm64: [HERMES_APPLE_TARGET_PLATFORM=appletvsimulator]
// CHECK-NEXT: hermesc xros x86_64,arm64: [HERMES_APPLE_TARGET_PLATFORM=xros]
// CHECK-NEXT: hermesc xrsimulator x86_64,arm64: [HERMES_APPLE_TARGET_PLATFORM=xrsimulator]
// CHECK-NEXT: hermesc macosx x86_64: [a build for CMAKE_OSX_ARCHITECTURES=x86_64 on arm64]
// Several triples must be Apple triples of one platform and of different
// architectures.
// CHECK-NEXT: together x86_64-unknown-linux-gnu: []
// CHECK-NEXT: together aarch64-apple-darwin,x86_64-apple-darwin: []
// CHECK-NEXT: together x86_64-apple-ios,aarch64-apple-ios-sim: []
// CHECK-NEXT: together aarch64-apple-ios-macabi,x86_64-apple-ios-macabi: []
// CHECK-NEXT: together x86_64-unknown-linux-gnu,aarch64-linux-android: [only Apple targets can be built together, not x86_64-unknown-linux-gnu]
// CHECK-NEXT: together aarch64-apple-darwin,aarch64-linux-android: [only Apple targets can be built together, not aarch64-linux-android]
// CHECK-NEXT: together aarch64-apple-darwin,aarch64-apple-darwin: [two of the triples are for aarch64]
// CHECK-NEXT: together aarch64-apple-ios-sim,aarch64-apple-tvos-sim: [aarch64-apple-tvos-sim is for appletvsimulator, not iphonesimulator; one build combines the architectures of one Apple platform]
// CHECK-NEXT: together aarch64-apple-ios,x86_64-apple-ios: [x86_64-apple-ios is for iphonesimulator, not iphoneos; one build combines the architectures of one Apple platform]
// CHECK-NEXT: together aarch64-apple-ios,aarch64-apple-ios-sim: [aarch64-apple-ios-sim is for iphonesimulator, not iphoneos; one build combines the architectures of one Apple platform]
// The system libraries per triple.
// CHECK-NEXT: libs x86_64-unknown-linux-gnu: [Threads::Threads;dl;m]
// CHECK-NEXT: libs aarch64-unknown-linux-musl: [Threads::Threads;dl;m]
// CHECK-NEXT: libs aarch64-linux-android: [dl;log;m]
// CHECK-NEXT: libs armv7-linux-androideabi: [dl;log;m]
// CHECK-NEXT: libs aarch64-apple-darwin: []
// CHECK-NEXT: libs aarch64-apple-ios-macabi: []
// CHECK-NEXT: libs x86_64-pc-windows-msvc: [kernel32;ntdll;userenv;ws2_32;dbghelp]
// CHECK-NEXT: libs aarch64-pc-windows-msvc: [kernel32;ntdll;userenv;ws2_32;dbghelp]
// CHECK-NEXT: libs wasm32-unknown-emscripten: []
// The cargo profile directory per configuration.
// CHECK-NEXT: profile Debug: debug
// CHECK-NEXT: profile debug: debug
// CHECK-NEXT: profile Release: release
// CHECK-NEXT: profile RelWithDebInfo: release
// CHECK-NEXT: profile MinSizeRel: release
