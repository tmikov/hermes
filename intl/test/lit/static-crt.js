/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The static MSVC C runtime check of an Intl build
// (hermes_intl_static_crt in cmake/modules/HermesIntl.cmake): any setting
// that selects /MT or /MTd in some configuration is reported. It is plain
// CMake, so it runs on every platform.
// RUN: %cmake -P %S/Inputs/static-crt.cmake 2>&1 | %FileCheck --match-full-lines %s
// CHECK: [] [] []: []
// CHECK-NEXT: [MultiThreadedDLL] [] []: []
// CHECK-NEXT: [MultiThreadedDebugDLL] [] []: []
// CHECK-NEXT: [MultiThreaded$<$<CONFIG:Debug>:Debug>DLL] [] []: []
// CHECK-NEXT: [MultiThreaded] [] []: [CMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded]
// CHECK-NEXT: [MultiThreadedDebug] [] []: [CMAKE_MSVC_RUNTIME_LIBRARY=MultiThreadedDebug]
// CHECK-NEXT: [MultiThreaded$<$<CONFIG:Debug>:Debug>] [] []: [CMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded$<$<CONFIG:Debug>:Debug>]
// Static in every configuration but Debug.
// CHECK-NEXT: [MultiThreaded$<$<CONFIG:Debug>:DebugDLL>] [] []: [CMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded$<$<CONFIG:Debug>:DebugDLL>]
// Shared in both configurations, but not in the one accepted form.
// CHECK-NEXT: [$<IF:$<CONFIG:Debug>,MultiThreadedDebugDLL,MultiThreadedDLL>] [] []: [CMAKE_MSVC_RUNTIME_LIBRARY=$<IF:$<CONFIG:Debug>,MultiThreadedDebugDLL,MultiThreadedDLL>]
// CHECK-NEXT: [] [/O2 /MD] [/MD]: []
// CHECK-NEXT: [] [/O2 /MT] []: [/MT in CMAKE_CXX_FLAGS]
// CHECK-NEXT: [] [] [/O2 -MTd]: [-MTd in CMAKE_C_FLAGS_RELEASE]
// CHECK-NEXT: [] [/O2 /MTX] [/DMT]: []
