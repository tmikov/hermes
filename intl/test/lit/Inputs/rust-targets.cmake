# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# Run by rust-targets.js: hermes_intl_rust_targets for each kind of build,
# each case with only its own variables set; then hermes_intl_foreign_hermesc,
# hermes_intl_check_rust_triples, the native libraries per triple and the
# cargo profile per configuration.

cmake_minimum_required(VERSION 3.21)
include(${CMAKE_CURRENT_LIST_DIR}/../../../../cmake/modules/HermesIntl.cmake)

# Every variable hermes_intl_rust_targets and hermes_intl_foreign_hermesc
# read. cmake -P sets some of them for the machine it runs on (UNIX, APPLE),
# so each case clears them all.
set(all_vars
  HERMES_INTL_RUST_TARGET HERMES_INTL_CARGO_HOST CMAKE_CROSSCOMPILING
  CMAKE_SYSTEM_NAME CMAKE_SYSTEM_PROCESSOR EMSCRIPTEN ANDROID
  HERMES_IS_ANDROID ANDROID_ABI CMAKE_ANDROID_ARCH_ABI
  HERMES_APPLE_TARGET_PLATFORM APPLE CMAKE_OSX_ARCHITECTURES WIN32
  CMAKE_CXX_COMPILER_ARCHITECTURE_ID CMAKE_VS_PLATFORM_NAME MSVC MINGW
  CMAKE_CXX_SIMULATE_ID CMAKE_HOST_SYSTEM_PROCESSOR)

# Set, in the caller's scope, the variables of a build: every variable in
# all_vars cleared, then the NAME VALUE pairs in ARGN, with "," for ";" in a
# value and "<empty>" for an empty one (an empty argument would be lost).
# Every case runs on a Linux x86-64 host unless it says otherwise.
macro(set_build)
  foreach(v IN LISTS all_vars)
    set(${v} "")
  endforeach()
  set(HERMES_INTL_CARGO_HOST x86_64-unknown-linux-gnu)
  set(CMAKE_HOST_SYSTEM_PROCESSOR x86_64)
  set(args ${ARGN})
  while(args)
    list(POP_FRONT args name value)
    string(REPLACE "," ";" value "${value}")
    if(value STREQUAL "<empty>")
      set(value "")
    endif()
    set(${name} "${value}")
  endwhile()
endmacro()

# Print "<label>: [<triples>] [<env var>] [<reason>]" for the build whose
# variables are the NAME VALUE pairs after \p label.
function(check label)
  set_build(${ARGN})
  hermes_intl_rust_targets(triples env_var reason)
  message("${label}: [${triples}] [${env_var}] [${reason}]")
endfunction()

# Print "hermesc <label>: [<why>]" for the build whose variables are the NAME
# VALUE pairs after \p label: why its own hermesc cannot run on the build
# machine, so that it needs IMPORT_HOST_COMPILERS.
function(check_hermesc label)
  set_build(${ARGN})
  hermes_intl_foreign_hermesc(why)
  message("hermesc ${label}: [${why}]")
endfunction()

# Host builds.
check("linux host")
check("linux host, no cargo" HERMES_INTL_CARGO_HOST "<empty>")
set(mac APPLE 1 HERMES_INTL_CARGO_HOST aarch64-apple-darwin)
check("macos" ${mac})
check("macos arm64" ${mac} CMAKE_OSX_ARCHITECTURES arm64)
check("macos x86_64" ${mac} CMAKE_OSX_ARCHITECTURES x86_64)
check("macos arm64;x86_64" ${mac} CMAKE_OSX_ARCHITECTURES "arm64,x86_64")
check("macos arm64e" ${mac} CMAKE_OSX_ARCHITECTURES arm64e)

# Android, from the NDK toolchain's variables.
set(android CMAKE_CROSSCOMPILING ON CMAKE_SYSTEM_NAME Android ANDROID 1)
foreach(abi arm64-v8a armeabi-v7a x86 x86_64 riscv64)
  check("android ${abi}" ${android} ANDROID_ABI ${abi})
endforeach()
check("android HERMES_IS_ANDROID, CMAKE_ANDROID_ARCH_ABI"
  HERMES_IS_ANDROID ON CMAKE_ANDROID_ARCH_ABI x86_64)

# Apple target platforms, built on an arm64 Mac.
foreach(platform macosx iphoneos iphonesimulator catalyst appletvos
    appletvsimulator xros xrsimulator)
  foreach(archs arm64 "x86_64,arm64")
    check("${platform} ${archs}" ${mac} HERMES_APPLE_TARGET_PLATFORM
      ${platform} CMAKE_OSX_ARCHITECTURES "${archs}")
  endforeach()
endforeach()
check("iphonesimulator, no archs, x86_64 host" ${mac}
  HERMES_INTL_CARGO_HOST x86_64-apple-darwin
  HERMES_APPLE_TARGET_PLATFORM iphonesimulator)
check("iphonesimulator, no archs, no cargo" ${mac}
  HERMES_INTL_CARGO_HOST "<empty>" HERMES_APPLE_TARGET_PLATFORM iphonesimulator)
check("iphonesimulator, no archs, riscv64 host" ${mac}
  HERMES_INTL_CARGO_HOST riscv64gc-unknown-linux-gnu
  HERMES_APPLE_TARGET_PLATFORM iphonesimulator)
check("appletvsimulator x86_64" ${mac}
  HERMES_APPLE_TARGET_PLATFORM appletvsimulator CMAKE_OSX_ARCHITECTURES x86_64)
check("unknown apple platform" ${mac}
  HERMES_APPLE_TARGET_PLATFORM watchos CMAKE_OSX_ARCHITECTURES arm64)

# Windows with the MSVC ABI, from each variable that can name the
# architecture; MinGW gets a reason.
set(windows WIN32 1 CMAKE_SYSTEM_NAME Windows MSVC 1)
check("windows CMAKE_SYSTEM_PROCESSOR AMD64" ${windows}
  CMAKE_SYSTEM_PROCESSOR AMD64)
check("windows CMAKE_SYSTEM_PROCESSOR arm64" ${windows}
  CMAKE_CROSSCOMPILING ON CMAKE_SYSTEM_PROCESSOR arm64)
check("windows CMAKE_VS_PLATFORM_NAME x64" ${windows}
  CMAKE_VS_PLATFORM_NAME x64 CMAKE_SYSTEM_PROCESSOR ARM64)
check("windows CMAKE_VS_PLATFORM_NAME ARM64" ${windows}
  CMAKE_VS_PLATFORM_NAME ARM64)
check("windows compiler x64" ${windows}
  CMAKE_CXX_COMPILER_ARCHITECTURE_ID x64 CMAKE_VS_PLATFORM_NAME ARM64)
check("windows compiler ARM64" ${windows}
  CMAKE_CXX_COMPILER_ARCHITECTURE_ID ARM64 CMAKE_SYSTEM_PROCESSOR AMD64)
check("windows Win32" ${windows} CMAKE_VS_PLATFORM_NAME Win32)
check("windows clang simulating MSVC" WIN32 1 CMAKE_SYSTEM_NAME Windows
  CMAKE_CXX_SIMULATE_ID MSVC CMAKE_CXX_COMPILER_ARCHITECTURE_ID x64)
check("windows mingw" WIN32 1 CMAKE_SYSTEM_NAME Windows MINGW 1
  CMAKE_SYSTEM_PROCESSOR AMD64)

# Emscripten, and a cross build nothing maps.
check("emscripten"
  CMAKE_CROSSCOMPILING ON CMAKE_SYSTEM_NAME Emscripten EMSCRIPTEN 1)
check("linux arm cross"
  CMAKE_CROSSCOMPILING ON CMAKE_SYSTEM_NAME Linux CMAKE_SYSTEM_PROCESSOR arm)

# HERMES_INTL_RUST_TARGET wins over every rule above except Emscripten.
set(override HERMES_INTL_RUST_TARGET "aarch64-unknown-linux-gnu")
check("override linux host" ${override})
check("override macos" ${override} ${mac} CMAKE_OSX_ARCHITECTURES x86_64)
check("override android riscv64" ${override} ${android} ANDROID_ABI riscv64)
check("override unknown apple platform" ${override} ${mac}
  HERMES_APPLE_TARGET_PLATFORM watchos)
check("override windows Win32" ${override} ${windows}
  CMAKE_VS_PLATFORM_NAME Win32)
check("override emscripten" ${override} CMAKE_CROSSCOMPILING ON EMSCRIPTEN 1)
check("override linux arm cross" ${override}
  CMAKE_CROSSCOMPILING ON CMAKE_SYSTEM_NAME Linux CMAKE_SYSTEM_PROCESSOR arm)
check("override list" HERMES_INTL_RUST_TARGET
  "x86_64-apple-ios,aarch64-apple-ios-sim")

# Which builds need the host's hermesc (IMPORT_HOST_COMPILERS).
check_hermesc("linux host")
check_hermesc("linux cross" CMAKE_CROSSCOMPILING ON)
check_hermesc("android" ${android} ANDROID_ABI arm64-v8a)
set(arm_mac APPLE 1 CMAKE_HOST_SYSTEM_PROCESSOR arm64)
check_hermesc("macos" ${arm_mac})
check_hermesc("macos arm64" ${arm_mac} CMAKE_OSX_ARCHITECTURES arm64)
check_hermesc("macos x86_64" ${arm_mac} CMAKE_OSX_ARCHITECTURES x86_64)
check_hermesc("macos x86_64 on an x86_64 mac" APPLE 1
  CMAKE_HOST_SYSTEM_PROCESSOR x86_64 CMAKE_OSX_ARCHITECTURES x86_64)
check_hermesc("macos arm64 on an x86_64 mac" APPLE 1
  CMAKE_HOST_SYSTEM_PROCESSOR x86_64 CMAKE_OSX_ARCHITECTURES arm64)
check_hermesc("macos x86_64,arm64" ${arm_mac}
  CMAKE_OSX_ARCHITECTURES "x86_64,arm64")
foreach(platform macosx iphoneos iphonesimulator catalyst appletvos
    appletvsimulator xros xrsimulator)
  check_hermesc("${platform} x86_64,arm64" ${arm_mac}
    HERMES_APPLE_TARGET_PLATFORM ${platform}
    CMAKE_OSX_ARCHITECTURES "x86_64,arm64")
endforeach()
check_hermesc("macosx x86_64" ${arm_mac}
  HERMES_APPLE_TARGET_PLATFORM macosx CMAKE_OSX_ARCHITECTURES x86_64)

# Which lists of triples can be built together.
foreach(triples
    "x86_64-unknown-linux-gnu"
    "aarch64-apple-darwin,x86_64-apple-darwin"
    "x86_64-apple-ios,aarch64-apple-ios-sim"
    "aarch64-apple-ios-macabi,x86_64-apple-ios-macabi"
    "x86_64-unknown-linux-gnu,aarch64-linux-android"
    "aarch64-apple-darwin,aarch64-linux-android"
    "aarch64-apple-darwin,aarch64-apple-darwin"
    "aarch64-apple-ios-sim,aarch64-apple-tvos-sim"
    "aarch64-apple-ios,x86_64-apple-ios"
    "aarch64-apple-ios,aarch64-apple-ios-sim")
  string(REPLACE "," ";" list "${triples}")
  hermes_intl_check_rust_triples("${list}" error)
  message("together ${triples}: [${error}]")
endforeach()

# Native libraries per triple.
foreach(triple x86_64-unknown-linux-gnu aarch64-unknown-linux-musl
    aarch64-linux-android armv7-linux-androideabi aarch64-apple-darwin
    aarch64-apple-ios-macabi x86_64-pc-windows-msvc aarch64-pc-windows-msvc
    wasm32-unknown-emscripten)
  hermes_intl_rust_native_libs(${triple} libs)
  message("libs ${triple}: [${libs}]")
endforeach()

# The cargo profile per build configuration.
foreach(config Debug debug Release RelWithDebInfo MinSizeRel)
  hermes_intl_cargo_profile_dir(${config} dir)
  message("profile ${config}: ${dir}")
endforeach()
