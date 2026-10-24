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
// /proc/self/maps instead. The unit tests in unittests/IntlLoader cover the
// choice between readings of the pathname with a fake stat.

// REQUIRES: linux, hermesvm_lib
// RUN: rm -rf %t && mkdir -p %t/lib
// RUN: cp %intl_hermesvm_lib %t/lib/libhermesvm.so
// RUN: cp %intl_fixture_dir/libfake_intl_good.so %t/lib/%intl_library_name
// RUN: cd %t && env -u LD_LIBRARY_PATH %intl_relative_host lib/libhermesvm.so / | %FileCheck --match-full-lines %s

// The kernel writes a newline in a name as "\012". The library is reached
// through a symlink, and the kernel names its real directory.
// RUN: cp -r %t/lib %t/$'new\nline'
// RUN: ln -s $'new\nline' %t/plain
// RUN: cd %t && env -u LD_LIBRARY_PATH %intl_relative_host plain/libhermesvm.so / | %FileCheck --match-full-lines %s

// No other escape exists: a directory literally named "lib\141" is not
// "liba", even when a liba/libhermesvm.so exists.
// RUN: cp -r %t/lib %t/'lib\141'
// RUN: mkdir -p %t/liba && cp %t/lib/libhermesvm.so %t/liba/
// RUN: cd %t && env -u LD_LIBRARY_PATH %intl_relative_host 'lib\141/libhermesvm.so' / | %FileCheck --match-full-lines %s

// A backslash is not escaped either, so "\012" in the kernel's text may be
// four characters of the name. The file's identity decides: here the
// directory is literally "x\012y", and a decoy library sits in "x<newline>y".
// RUN: cp -r %t/lib %t/'x\012y'
// RUN: mkdir -p %t/$'x\ny' && cp %t/lib/libhermesvm.so %t/$'x\ny'/
// RUN: cd %t && env -u LD_LIBRARY_PATH %intl_relative_host 'x\012y/libhermesvm.so' / | %FileCheck --match-full-lines %s

// The kernel appends " (deleted)" to a file removed after it was mapped, and
// no file has that name: the loader falls back to dladdr's path. Loaded by an absolute path in a directory literally named
// "lib\142" (not "libb"), that still finds the plugin.
// RUN: cp -r %t/lib %t/'lib\142'
// RUN: env -u LD_LIBRARY_PATH %intl_relative_host %t/'lib\142/libhermesvm.so' / delete | %FileCheck --match-full-lines %s
// Loaded by a relative path, the fallback names nothing from the new
// directory, and there is no plugin; that is not fatal.
// RUN: cp -r %t/lib %t/gone
// RUN: cd %t && env -u LD_LIBRARY_PATH %intl_relative_host gone/libhermesvm.so / delete | %FileCheck --match-full-lines --check-prefix=NONE %s

// CHECK: plugin
// NONE: no plugin
