# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# Print the Rust target triples the Intl plugin's core is built for in the
# Apple framework slice HERMES_APPLE_TARGET_PLATFORM with architectures
# CMAKE_OSX_ARCHITECTURES, separated by spaces, using the build's own mapping
# (hermes_intl_rust_targets in cmake/modules/HermesIntl.cmake). Run by
# utils/build-apple-framework.sh rust_targets <slice>:
#
#   cmake -DHERMES_APPLE_TARGET_PLATFORM=<slice> \
#     -DCMAKE_OSX_ARCHITECTURES="x86_64;arm64" -P utils/intl-rust-targets.cmake
#
# A slice with no mapping prints the reason to stderr and fails.

cmake_minimum_required(VERSION 3.21)
include(${CMAKE_CURRENT_LIST_DIR}/../cmake/modules/HermesIntl.cmake)

if(NOT HERMES_APPLE_TARGET_PLATFORM OR NOT CMAKE_OSX_ARCHITECTURES)
  message(FATAL_ERROR
    "Set HERMES_APPLE_TARGET_PLATFORM and CMAKE_OSX_ARCHITECTURES")
endif()
# Only the Apple rule applies; nothing else of the machine running this.
foreach(v HERMES_INTL_RUST_TARGET EMSCRIPTEN ANDROID HERMES_IS_ANDROID
          ANDROID_ABI CMAKE_ANDROID_ARCH_ABI)
  unset(${v})
endforeach()
hermes_intl_rust_targets(triples env_var reason)
if(reason)
  message(FATAL_ERROR "${reason}")
endif()
list(JOIN triples " " out)
# message() writes to stderr; the triples go to stdout.
execute_process(COMMAND ${CMAKE_COMMAND} -E echo "${out}")
