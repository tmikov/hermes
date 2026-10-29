# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# A CMake toolchain file that cross-compiles Hermes for Windows (the MSVC
# ABI) on Linux, with clang-cl, lld-link and the MSVC C runtime, C++ library
# and Windows SDK that xwin (https://github.com/Jake-Shadle/xwin) downloads.
# See "Cross-building for Windows on Linux" in intl/doc/Building.md.
#
#   cmake -G Ninja -B build-win \
#     -DCMAKE_TOOLCHAIN_FILE=utils/windows-cross/clang-cl-xwin.cmake \
#     -DCMAKE_BUILD_TYPE=Release \
#     -DIMPORT_HOST_COMPILERS=<host build>/ImportHostCompilers.cmake
#
# Settings (cache variables):
#   HERMES_WINDOWS_ARCH     x64 (default) or arm64.
#   HERMES_XWIN_DIR         the xwin splat for that architecture (default
#                           ~/.xwin): crt/ and sdk/ with xwin's default
#                           layout. With xwin 0.10.0:
#                             xwin --accept-license --crt-version 14.44.17.14
#                               --sdk-version 10.0.26100 --arch x86_64 splat
#                               --include-debug-libs --output ~/.xwin
#                           and --arch aarch64 --output ~/.xwin-aarch64 for
#                           arm64.
#   HERMES_LLVM_BIN_DIR     the directory with clang-cl, lld-link, llvm-lib,
#                           llvm-rc, llvm-mt and llvm-ml64 (default: where
#                           clang-cl is on PATH). The MSVC 14.44 headers need
#                           Clang 19 or newer.
#   HERMES_MSVC_VERSION     the MSVC version clang-cl claims to be, which
#                           must match the splat's C++ library (default
#                           19.44, for MSVC 14.44 / VS 2022 17.14). clang-cl
#                           cannot detect it from /imsvc directories.

set(CMAKE_SYSTEM_NAME Windows)
set(CMAKE_SYSTEM_VERSION 10.0)

set(HERMES_WINDOWS_ARCH "x64" CACHE STRING
  "Windows architecture to build for: x64 or arm64")
set(HERMES_XWIN_DIR "$ENV{HOME}/.xwin" CACHE PATH
  "The xwin splat (crt/ and sdk/) for HERMES_WINDOWS_ARCH")
set(HERMES_MSVC_VERSION "19.44" CACHE STRING
  "The MSVC version of the splat's C++ library (-fms-compatibility-version)")
if(NOT HERMES_LLVM_BIN_DIR)
  find_program(_hermes_clang_cl clang-cl NO_CACHE)
  if(_hermes_clang_cl)
    get_filename_component(_hermes_llvm_bin "${_hermes_clang_cl}" DIRECTORY)
  endif()
  set(HERMES_LLVM_BIN_DIR "${_hermes_llvm_bin}" CACHE PATH
    "The directory with clang-cl, lld-link, llvm-lib, llvm-rc and llvm-mt")
endif()

# try_compile projects re-read this file; give them the same settings.
list(APPEND CMAKE_TRY_COMPILE_PLATFORM_VARIABLES
  HERMES_WINDOWS_ARCH HERMES_XWIN_DIR HERMES_MSVC_VERSION HERMES_LLVM_BIN_DIR)

if(HERMES_WINDOWS_ARCH STREQUAL "x64")
  set(CMAKE_SYSTEM_PROCESSOR AMD64)
  set(_hermes_triple x86_64-pc-windows-msvc)
  set(_hermes_xwin_arch x86_64)
  set(_hermes_machine x64)
elseif(HERMES_WINDOWS_ARCH STREQUAL "arm64")
  set(CMAKE_SYSTEM_PROCESSOR ARM64)
  set(_hermes_triple aarch64-pc-windows-msvc)
  set(_hermes_xwin_arch aarch64)
  set(_hermes_machine arm64)
else()
  message(FATAL_ERROR
    "HERMES_WINDOWS_ARCH is '${HERMES_WINDOWS_ARCH}'; use x64 or arm64.")
endif()

if(NOT EXISTS "${HERMES_LLVM_BIN_DIR}/clang-cl")
  message(FATAL_ERROR
    "No clang-cl in HERMES_LLVM_BIN_DIR ('${HERMES_LLVM_BIN_DIR}'). Put an "
    "LLVM 19 or newer bin directory on PATH or pass "
    "-DHERMES_LLVM_BIN_DIR=<dir>.")
endif()
foreach(_dir crt/include crt/lib/${_hermes_xwin_arch} sdk/include/ucrt
    sdk/lib/um/${_hermes_xwin_arch})
  if(NOT IS_DIRECTORY "${HERMES_XWIN_DIR}/${_dir}")
    message(FATAL_ERROR
      "HERMES_XWIN_DIR ('${HERMES_XWIN_DIR}') has no ${_dir}. With xwin "
      "0.10.0, run 'xwin --accept-license --crt-version 14.44.17.14 "
      "--sdk-version 10.0.26100 --arch ${_hermes_xwin_arch} splat "
      "--include-debug-libs --output <dir>' and pass "
      "-DHERMES_XWIN_DIR=<dir>.")
  endif()
endforeach()

set(CMAKE_C_COMPILER "${HERMES_LLVM_BIN_DIR}/clang-cl")
set(CMAKE_CXX_COMPILER "${HERMES_LLVM_BIN_DIR}/clang-cl")
set(CMAKE_C_COMPILER_TARGET ${_hermes_triple})
set(CMAKE_CXX_COMPILER_TARGET ${_hermes_triple})
set(CMAKE_LINKER "${HERMES_LLVM_BIN_DIR}/lld-link")
set(CMAKE_AR "${HERMES_LLVM_BIN_DIR}/llvm-lib")
set(CMAKE_RC_COMPILER "${HERMES_LLVM_BIN_DIR}/llvm-rc")
set(CMAKE_MT "${HERMES_LLVM_BIN_DIR}/llvm-mt")
# No Boost.Context fibers by default. Its x64 code is MASM, and llvm-ml64
# (LLVM 23) assembles its "lea rcx, label" with an absolute 32-bit address
# (IMAGE_REL_AMD64_ADDR32) where ml64 uses a RIP-relative one; the address
# is truncated in an image above 4 GB (hermes.exe is at 0x140000000), and
# the first fiber switch jumps to it. On arm64 its code is armasm, which
# LLVM does not have. Hermes then runs JS on the thread's own stack.
set(HERMES_ALLOW_BOOST_CONTEXT 0 CACHE STRING
  "Use Boost.Context fibers: 0 (off), 1 (auto) or 2 (force on)")
set(CMAKE_ASM_MASM_COMPILER "${HERMES_LLVM_BIN_DIR}/llvm-ml64")

# The splat's headers, as system headers, and its libraries.
set(_hermes_includes "")
foreach(_dir crt/include sdk/include/ucrt sdk/include/um sdk/include/shared)
  string(APPEND _hermes_includes " /imsvc \"${HERMES_XWIN_DIR}/${_dir}\"")
endforeach()
# /Zc:twoPhase parses templates where they are defined, as MSVC does in the
# C++20 mode Hermes uses with cl; clang-cl in C++17 mode would delay it to
# the end of the file, where a later using-directive can make names
# ambiguous.
set(_hermes_cflags "-fms-compatibility-version=${HERMES_MSVC_VERSION}")
string(APPEND _hermes_cflags " /Zc:twoPhase${_hermes_includes}")
set(CMAKE_C_FLAGS_INIT "${_hermes_cflags}")
set(CMAKE_CXX_FLAGS_INIT "${_hermes_cflags}")
set(CMAKE_RC_FLAGS_INIT "${_hermes_includes}")
set(_hermes_ldflags "/machine:${_hermes_machine}")
foreach(_dir crt/lib sdk/lib/um sdk/lib/ucrt)
  string(APPEND _hermes_ldflags
    " /libpath:\"${HERMES_XWIN_DIR}/${_dir}/${_hermes_xwin_arch}\"")
endforeach()
set(CMAKE_EXE_LINKER_FLAGS_INIT "${_hermes_ldflags}")
set(CMAKE_SHARED_LINKER_FLAGS_INIT "${_hermes_ldflags}")
set(CMAKE_MODULE_LINKER_FLAGS_INIT "${_hermes_ldflags}")

# Find libraries and headers in the splat only; programs (python, cargo) on
# the host.
set(CMAKE_FIND_ROOT_PATH "${HERMES_XWIN_DIR}")
set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_PACKAGE ONLY)
