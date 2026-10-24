/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// A host that loads libhermesvm with dlopen by a relative path, RTLD_LOCAL,
// and then changes directory still finds the plugin next to libhermesvm.
// dladdr reports the relative path the library was loaded by, which no
// longer names it; on Linux the loader finds the library's file in
// /proc/self/maps instead.

// REQUIRES: linux, hermesvm_lib
// RUN: rm -rf %t && mkdir -p %t/lib
// RUN: cp %intl_hermesvm_lib %t/lib/libhermesvm.so
// RUN: cp %intl_fixture_dir/libfake_intl_good.so %t/lib/%intl_library_name
// RUN: cd %t && env -u LD_LIBRARY_PATH %intl_relative_host lib/libhermesvm.so / | %FileCheck --match-full-lines %s

// The kernel writes a newline in a name as "\012"; the loader undoes that.
// The library is reached through a symlink, and the kernel names its real
// directory.
// RUN: mkdir -p %t/$'new\nline'
// RUN: cp -r %t/lib %t/$'new\nline'/lib
// RUN: ln -s $'new\nline' %t/plain
// RUN: cd %t && env -u LD_LIBRARY_PATH %intl_relative_host plain/lib/libhermesvm.so / | %FileCheck --match-full-lines %s

// The kernel appends " (deleted)" to a file removed after it was mapped; its
// directory is still the one to search.
// RUN: cd %t && env -u LD_LIBRARY_PATH %intl_relative_host lib/libhermesvm.so / delete | %FileCheck --match-full-lines %s

// CHECK: plugin
