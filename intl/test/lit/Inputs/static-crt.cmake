# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# Run by static-crt.js: what hermes_intl_static_crt reports for MSVC runtime
# settings and compiler flags.

cmake_minimum_required(VERSION 3.21)
include(${CMAKE_CURRENT_LIST_DIR}/../../../../cmake/modules/HermesIntl.cmake)

# Print the case and the result: the runtime, then the flags of two
# variables, CMAKE_CXX_FLAGS and CMAKE_C_FLAGS_RELEASE.
function(check runtime cxx c_release)
  set(CMAKE_CXX_FLAGS "${cxx}")
  set(CMAKE_C_FLAGS_RELEASE "${c_release}")
  hermes_intl_static_crt("${runtime}" result
    CMAKE_CXX_FLAGS CMAKE_C_FLAGS_RELEASE)
  message("[${runtime}] [${cxx}] [${c_release}]: [${result}]")
endfunction()

check("" "" "")
check("MultiThreadedDLL" "" "")
check("MultiThreadedDebugDLL" "" "")
check("MultiThreaded$<$<CONFIG:Debug>:Debug>DLL" "" "")
check("MultiThreaded" "" "")
check("MultiThreadedDebug" "" "")
check("MultiThreaded$<$<CONFIG:Debug>:Debug>" "" "")
check("MultiThreaded$<$<CONFIG:Debug>:DebugDLL>" "" "")
check("$<IF:$<CONFIG:Debug>,MultiThreadedDebugDLL,MultiThreadedDLL>" "" "")
check("" "/O2 /MD" "/MD")
check("" "/O2 /MT" "")
check("" "" "/O2 -MTd")
check("" "/O2 /MTX" "/DMT")
