# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# A CMake toolchain file that builds Hermes for Windows on Linux with
# Microsoft's own compiler, linker and assemblers (cl, link, lib, ml64 or
# armasm64, rc, mt), run under Wine through the wrapper scripts of msvc-wine
# (https://github.com/mstorsjo/msvc-wine). See "Cross-building for Windows
# on Linux" in intl/doc/Building.md.
#
#   cmake -G Ninja -B build-win \
#     -DCMAKE_TOOLCHAIN_FILE=utils/windows-cross/msvc-wine.cmake \
#     -DCMAKE_POLICY_DEFAULT_CMP0141=NEW -DCMAKE_BUILD_TYPE=Release \
#     -DIMPORT_HOST_COMPILERS=<host build>/ImportHostCompilers.cmake
#
# msvc-wine setup (tested with msvc-wine 514f8ea34, Wine 9.0, msitools;
# Visual Studio 2022 17.14 Build Tools, MSVC 14.44.35207, Windows SDK
# 10.0.26100; running the downloader means accepting Microsoft's license):
#
#   git clone https://github.com/mstorsjo/msvc-wine ~/opt/msvc-wine
#   ~/opt/msvc-wine/vsdownload.py --major 17 --accept-license \
#     --architecture x64 arm64 --host-arch x64 --dest ~/opt/msvc
#   ~/opt/msvc-wine/install.sh ~/opt/msvc
#
# The wrappers run in the default Wine prefix (WINEPREFIX, ~/.wine).
#
# Settings (cache variables):
#   HERMES_WINDOWS_ARCH     x64 (default) or arm64.
#   HERMES_MSVC_WINE_DIR    the directory install.sh set up (default
#                           ~/opt/msvc), with bin/x64 and bin/arm64.
#
# Needs CMake 3.25 or newer. -DCMAKE_POLICY_DEFAULT_CMP0141=NEW is
# required (the file checks both): debug
# information is then embedded in the objects (/Z7, CMake's "Embedded"
# format), also in CMake's own test compiles, so cl never starts
# mspdbsrv.exe, which needs Wine's winbind support.

set(CMAKE_SYSTEM_NAME Windows)
set(CMAKE_SYSTEM_VERSION 10.0)

set(HERMES_WINDOWS_ARCH "x64" CACHE STRING
  "Windows architecture to build for: x64 or arm64")
set(HERMES_MSVC_WINE_DIR "$ENV{HOME}/opt/msvc" CACHE PATH
  "The msvc-wine installation (the directory given to install.sh)")

# try_compile projects re-read this file; give them the same settings.
list(APPEND CMAKE_TRY_COMPILE_PLATFORM_VARIABLES
  HERMES_WINDOWS_ARCH HERMES_MSVC_WINE_DIR)

if(HERMES_WINDOWS_ARCH STREQUAL "x64")
  set(CMAKE_SYSTEM_PROCESSOR AMD64)
elseif(HERMES_WINDOWS_ARCH STREQUAL "arm64")
  set(CMAKE_SYSTEM_PROCESSOR ARM64)
else()
  message(FATAL_ERROR
    "HERMES_WINDOWS_ARCH is '${HERMES_WINDOWS_ARCH}'; use x64 or arm64.")
endif()

set(_hermes_msvc_bin "${HERMES_MSVC_WINE_DIR}/bin/${HERMES_WINDOWS_ARCH}")
if(HERMES_WINDOWS_ARCH STREQUAL "x64")
  set(_hermes_msvc_asm ml64)
else()
  set(_hermes_msvc_asm armasm64)
endif()
foreach(_tool cl link lib rc mt ${_hermes_msvc_asm})
  if(NOT EXISTS "${_hermes_msvc_bin}/${_tool}")
    message(FATAL_ERROR
      "HERMES_MSVC_WINE_DIR ('${HERMES_MSVC_WINE_DIR}') has no "
      "bin/${HERMES_WINDOWS_ARCH}/${_tool}. Install msvc-wine there "
      "('vsdownload.py --accept-license --architecture x64 arm64 "
      "--dest <dir>' and 'install.sh <dir>') and pass "
      "-DHERMES_MSVC_WINE_DIR=<dir>.")
  endif()
endforeach()
find_program(_hermes_wine NAMES wine64 wine NO_CACHE)
if(NOT _hermes_wine)
  message(FATAL_ERROR
    "No wine or wine64 on PATH; msvc-wine runs Microsoft's tools with it.")
endif()

set(CMAKE_C_COMPILER "${_hermes_msvc_bin}/cl")
set(CMAKE_CXX_COMPILER "${_hermes_msvc_bin}/cl")
set(CMAKE_LINKER "${_hermes_msvc_bin}/link")
set(CMAKE_AR "${_hermes_msvc_bin}/lib")
set(CMAKE_RC_COMPILER "${_hermes_msvc_bin}/rc")
set(CMAKE_MT "${_hermes_msvc_bin}/mt")
# Boost.Context's fcontext code: MASM on x64, armasm on arm64 (its own
# ASM_ARMASM language, external/boost/.../libs/context/cmake).
if(HERMES_WINDOWS_ARCH STREQUAL "x64")
  set(CMAKE_ASM_MASM_COMPILER "${_hermes_msvc_bin}/ml64")
else()
  set(CMAKE_ASM_ARMASM_COMPILER "${_hermes_msvc_bin}/armasm64")
endif()

# Embedded debug information (/Z7). CMAKE_MSVC_DEBUG_INFORMATION_FORMAT
# applies only when policy CMP0141 is NEW at the first project(), and
# Hermes's cmake_minimum_required, which comes before this file is read,
# leaves it unset; only CMAKE_POLICY_DEFAULT_CMP0141 on the command line is
# early enough. try_compile projects inherit the policy and the format.
if(CMAKE_VERSION VERSION_LESS 3.25)
  message(FATAL_ERROR
    "This toolchain file needs CMake 3.25 or newer (this is "
    "${CMAKE_VERSION}): older versions have no policy CMP0141 or "
    "CMAKE_MSVC_DEBUG_INFORMATION_FORMAT, so cl would use /Zi, which "
    "needs mspdbsrv.exe and Wine's winbind support.")
endif()
get_property(_hermes_in_try_compile GLOBAL PROPERTY IN_TRY_COMPILE)
if(NOT _hermes_in_try_compile AND
    NOT CMAKE_POLICY_DEFAULT_CMP0141 STREQUAL "NEW")
  message(FATAL_ERROR
    "Pass -DCMAKE_POLICY_DEFAULT_CMP0141=NEW with this toolchain file, so "
    "that debug information is embedded (/Z7). Otherwise cl uses /Zi, "
    "which needs mspdbsrv.exe and Wine's winbind support, and fails with "
    "'fatal error C1902: Program database manager mismatch'.")
endif()
# As CMake's default, debug information only in Debug and RelWithDebInfo.
set(CMAKE_MSVC_DEBUG_INFORMATION_FORMAT
  "$<$<CONFIG:Debug,RelWithDebInfo>:Embedded>")

# The wrappers find headers and libraries through INCLUDE and LIB, which
# they set themselves; programs (python, cargo) come from the host.
set(CMAKE_FIND_ROOT_PATH "${HERMES_MSVC_WINE_DIR}")
set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_PACKAGE ONLY)
