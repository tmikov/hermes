/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// Where hermes_intl_plugin_output_dir (cmake/modules/HermesIntl.cmake) puts
// the plugin relative to hermesvm, under a single- and a multi-configuration
// generator. In a framework build hermesintl.framework must be
// hermesvm.framework's sibling in every configuration, where the loader's
// sibling framework candidate and the plugin's LC_RPATH look; otherwise it
// goes in lib/, or in bin/<Config> (with a copy next to hermesvm) under a
// multi-configuration generator. A small project with FRAMEWORK set on a
// fake hermesvm, configured with Ninja and Ninja Multi-Config. It runs on
// Linux only: there FRAMEWORK changes nothing else and a shared library
// follows LIBRARY_OUTPUT_DIRECTORY, so the output directories compare
// directly. On macOS FRAMEWORK makes hermesvm a real bundle, and on Windows
// a DLL follows RUNTIME_OUTPUT_DIRECTORY (bin/ even in a single
// configuration). The real Apple layout, with both frameworks, Ninja
// Multi-Config and the Xcode generator, is checked on a Mac (Phase 6 Mac
// step M9b); the Windows layout by the spec's Windows checklist.
// REQUIRES: linux, ninja
// RUN: bash %S/Inputs/plugin-output-dir.sh %cmake %t 2>&1 | %FileCheck --match-full-lines %s

// CHECK: framework-single Release: vm lib, plugin lib: same
// CHECK-NEXT: framework-multi Debug: vm lib/Debug, plugin lib/Debug: same
// CHECK-NEXT: framework-multi Release: vm lib/Release, plugin lib/Release: same
// CHECK-NEXT: framework-multi-dirs Debug: vm fw/Debug, plugin fw/Debug: same
// CHECK-NEXT: framework-multi-dirs Release: vm fw-release, plugin fw-release: same
// CHECK-NEXT: plain-single Release: vm lib, plugin lib: same
// CHECK-NEXT: plain-multi Debug: vm lib/Debug, plugin bin/Debug: different
// CHECK-NEXT: plain-multi Release: vm lib/Release, plugin bin/Release: different
// CHECK-NOT: {{.}}
