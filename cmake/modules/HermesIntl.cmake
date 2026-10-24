# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# Checks that decide whether the Intl plugin (intl/) can build here, used by
# the top-level CMakeLists.txt and intl/CMakeLists.txt.
# intl/test/lit/osx-architectures.js and unsupported-reason.js run them
# through cmake -P.

# Set \p out to ON if \p archs, a CMAKE_OSX_ARCHITECTURES value, is a single
# architecture other than the one cargo builds for, and OFF otherwise.
# \p cargo_vv is the output of `cargo -vV`, whose "host:" line names that
# architecture; Rust spells arm64 "aarch64". cargo always builds the Rust
# core for the host, so for another architecture libhermesintl would find no
# core to link. Several architectures are left to the caller's own check,
# and a missing host line gives OFF.
function(hermes_intl_osx_arch_mismatch archs cargo_vv out)
  set(mismatch OFF)
  if(archs AND NOT archs MATCHES ";" AND
     cargo_vv MATCHES "(^|\n)host: ([A-Za-z0-9_]+)-")
    set(host_arch ${CMAKE_MATCH_2})
    if(host_arch STREQUAL "aarch64")
      set(host_arch arm64)
    endif()
    if(NOT archs STREQUAL host_arch)
      set(mismatch ON)
    endif()
  endif()
  set(${out} ${mismatch} PARENT_SCOPE)
endfunction()

# Set \p out to the kind of build the Intl plugin cannot build for, from the
# variables in scope, or to "" if it can build here. It completes "does not
# build for ...". \p osx_arch_mismatch is the result of
# hermes_intl_osx_arch_mismatch. Phase 0 builds for the host only, on Linux
# and macOS.
function(hermes_intl_unsupported_reason osx_arch_mismatch out)
  if(CMAKE_CROSSCOMPILING)
    set(reason "cross builds")
  elseif(HERMES_IS_ANDROID)
    set(reason "Android")
  elseif(HERMES_APPLE_TARGET_PLATFORM)
    string(CONCAT reason
      "Apple targets "
      "(HERMES_APPLE_TARGET_PLATFORM=${HERMES_APPLE_TARGET_PLATFORM})")
  elseif(WIN32)
    set(reason "Windows")
  elseif(EMSCRIPTEN)
    set(reason "Emscripten")
  elseif(CMAKE_OSX_ARCHITECTURES MATCHES ";")
    set(reason "several CMAKE_OSX_ARCHITECTURES (${CMAKE_OSX_ARCHITECTURES})")
  elseif(osx_arch_mismatch)
    string(CONCAT reason
      "CMAKE_OSX_ARCHITECTURES=${CMAKE_OSX_ARCHITECTURES}, which is not the "
      "architecture cargo builds for")
  else()
    set(reason "")
  endif()
  set(${out} "${reason}" PARENT_SCOPE)
endfunction()
