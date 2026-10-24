# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# Run by unsupported-reason.js: hermes_intl_unsupported_reason for each kind
# of build, each case with only its own variables set.

include(${CMAKE_CURRENT_LIST_DIR}/../../../../cmake/modules/HermesIntl.cmake)

# Print the reason for a build where variable \p var is \p value, and the
# architectures check gave \p mismatch.
function(check var value mismatch)
  if(var)
    set(${var} "${value}")
  endif()
  hermes_intl_unsupported_reason(${mismatch} reason)
  message("${var}: [${reason}]")
endfunction()

# cmake -P leaves the platform variables unset, so each case sets its own.
check("" "" OFF)
check(CMAKE_CROSSCOMPILING ON OFF)
check(HERMES_IS_ANDROID ON OFF)
check(HERMES_APPLE_TARGET_PLATFORM iphoneos OFF)
check(WIN32 1 OFF)
check(EMSCRIPTEN 1 OFF)
check(CMAKE_OSX_ARCHITECTURES "arm64;x86_64" OFF)
check(CMAKE_OSX_ARCHITECTURES x86_64 ON)
# A single architecture that matches is fine.
check(CMAKE_OSX_ARCHITECTURES arm64 OFF)
