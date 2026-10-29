# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# Run by osx-architectures.js: the host triple read from `cargo -vV`, and
# the triples hermes_intl_rust_targets gives a macOS host build for each
# CMAKE_OSX_ARCHITECTURES on an arm64 and an x86-64 Mac.

cmake_minimum_required(VERSION 3.21)
include(${CMAKE_CURRENT_LIST_DIR}/../../../../cmake/modules/HermesIntl.cmake)

set(arm_host "cargo 1.90.0\nrelease: 1.90.0\nhost: aarch64-apple-darwin\n")
set(intel_host "cargo 1.90.0\nrelease: 1.90.0\nhost: x86_64-apple-darwin\n")

# Print the case and the result. Only the macOS variables are set; cmake -P
# sets UNIX or APPLE for the machine it runs on, so the rest are cleared.
function(check archs cargo_vv)
  foreach(v HERMES_INTL_RUST_TARGET CMAKE_CROSSCOMPILING CMAKE_SYSTEM_NAME
      EMSCRIPTEN ANDROID HERMES_IS_ANDROID ANDROID_ABI CMAKE_ANDROID_ARCH_ABI
      HERMES_APPLE_TARGET_PLATFORM WIN32)
    set(${v} "")
  endforeach()
  set(APPLE 1)
  set(CMAKE_OSX_ARCHITECTURES "${archs}")
  hermes_intl_cargo_host("${cargo_vv}" HERMES_INTL_CARGO_HOST)
  hermes_intl_rust_targets(triples env_var reason)
  message("[${archs}] [host: ${HERMES_INTL_CARGO_HOST}]: "
    "[${triples}] [${reason}]")
endfunction()

foreach(host arm_host intel_host)
  foreach(archs "" arm64 x86_64 "arm64;x86_64" "x86_64;arm64")
    check("${archs}" "${${host}}")
  endforeach()
endforeach()
# No host line (cargo not found): the host build has no triple, and
# intl/CMakeLists.txt reports the missing cargo.
check("" "")
# Explicit architectures do not need the host.
check("arm64;x86_64" "")
# The host line may end the output, and may have trailing spaces.
check("" "host: x86_64-apple-darwin  ")
