# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# Run by apple-framework.js: the architectures the plugin of each Apple slice
# is built for, and the LC_RPATH of hermesintl.framework per platform.

cmake_minimum_required(VERSION 3.21)
include(${CMAKE_CURRENT_LIST_DIR}/../../../../cmake/modules/HermesIntl.cmake)

# Print "archs <platform> <CMAKE_OSX_ARCHITECTURES>: [<plugin archs>]" for
# the Apple slice \p platform built for \p osx_archs ("," for ";"), from the
# triples hermes_intl_rust_targets maps it to.
function(check_slice platform osx_archs)
  string(REPLACE "," ";" osx_archs "${osx_archs}")
  foreach(v HERMES_INTL_RUST_TARGET EMSCRIPTEN ANDROID HERMES_IS_ANDROID
            ANDROID_ABI CMAKE_ANDROID_ARCH_ABI)
    set(${v} "")
  endforeach()
  set(HERMES_APPLE_TARGET_PLATFORM ${platform})
  set(CMAKE_OSX_ARCHITECTURES "${osx_archs}")
  set(HERMES_INTL_CARGO_HOST aarch64-apple-darwin)
  hermes_intl_rust_targets(triples env_var reason)
  hermes_intl_plugin_osx_architectures("${triples}" "${osx_archs}" archs)
  message("archs ${platform} ${osx_archs}: [${archs}]")
endfunction()

# Print "archs <label>: [<plugin archs>]" for triples \p triples and
# CMAKE_OSX_ARCHITECTURES \p osx_archs ("," for ";", "-" for empty).
function(check_archs label triples osx_archs)
  string(REPLACE "," ";" triples "${triples}")
  string(REPLACE "," ";" osx_archs "${osx_archs}")
  if(osx_archs STREQUAL "-")
    set(osx_archs "")
  endif()
  hermes_intl_plugin_osx_architectures("${triples}" "${osx_archs}" archs)
  message("archs ${label}: [${archs}]")
endfunction()

# The slices of utils/build-apple-framework.sh, with its architectures.
check_slice(macosx "x86_64,arm64")
check_slice(iphoneos arm64)
check_slice(iphonesimulator "x86_64,arm64")
check_slice(catalyst "x86_64,arm64")
check_slice(appletvos arm64)
check_slice(appletvsimulator "x86_64,arm64")
check_slice(xros arm64)
check_slice(xrsimulator "x86_64,arm64")
check_slice(appletvsimulator arm64)
check_slice(xrsimulator "arm64,x86_64")

check_archs("macos host" aarch64-apple-darwin -)
check_archs("override other order" "aarch64-apple-ios-sim,x86_64-apple-ios"
  "x86_64,arm64")
check_archs("not apple" x86_64-unknown-linux-gnu arm64)
check_archs("none" "" arm64)

foreach(platform "" macosx catalyst iphoneos iphonesimulator appletvos
                 appletvsimulator xros xrsimulator)
  hermes_intl_framework_rpath("${platform}" rpath)
  message("rpath [${platform}]: ${rpath}")
endforeach()
