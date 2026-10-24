/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// Without IntlPluginPath, hermes looks for the plugin next to itself, then in
// ../lib because it lives in a bin directory. The library search path, which
// comes last except on Apple platforms, is covered by
// default-resolution-search-path.js and default-resolution-apple.js.
//
// A copy of the binary in a scratch tree controls what it finds. It must be
// a copy: a hard link shares the inode with the real binary, and on macOS
// the loader's proc_pidpath can then report either link's path, so a
// concurrent run of the real binary could load this tree's plugin.

// RUN: rm -rf %t && mkdir -p %t/bin %t/lib
// RUN: cp %hermes_path %t/bin/hermes

// Nothing anywhere: no plugin, silently.
// RUN: env -u LD_LIBRARY_PATH -u DYLD_LIBRARY_PATH %t/bin/hermes %s | %FileCheck --match-full-lines --check-prefix=NONE %s
// NONE: no plugin

// In ../lib.
// RUN: cp %intl_fixture_dir/libfake_intl_good.so %t/lib/%intl_library_name
// RUN: env -u LD_LIBRARY_PATH -u DYLD_LIBRARY_PATH %t/bin/hermes %s | %FileCheck --check-prefix=FOUND %s
// FOUND: "detections":1

// Next to the binary comes first: a broken library in ../lib is not reached.
// RUN: cp %intl_fixture_dir/libfake_intl_good.so %t/bin/%intl_library_name
// RUN: echo "not a library" > %t/lib/%intl_library_name
// RUN: env -u LD_LIBRARY_PATH -u DYLD_LIBRARY_PATH %t/bin/hermes %s | %FileCheck --check-prefix=FOUND %s

// A broken library that is found is fatal.
// RUN: rm %t/bin/%intl_library_name
// RUN: env -u LD_LIBRARY_PATH -u DYLD_LIBRARY_PATH %not --crash %t/bin/hermes %s 2>&1 | %FileCheck --check-prefix=BROKEN %s
// BROKEN: LLVM ERROR: Intl plugin {{.*}}lib/libhermesintl.{{so|dylib}}: cannot be loaded:

// A plugin whose own dependency is missing is broken, not absent.
// RUN: cp %intl_fixture_dir/libfake_intl_needs_dep.so %t/lib/%intl_library_name
// RUN: env -u LD_LIBRARY_PATH -u DYLD_LIBRARY_PATH %not --crash %t/bin/hermes %s 2>&1 | %FileCheck --check-prefix=NEEDSDEP %s
// NEEDSDEP: LLVM ERROR: Intl plugin {{.*}}libhermesintl.{{so|dylib}}: cannot be loaded: {{.*}}fake_intl_dep
// RUN: rm %t/lib/%intl_library_name

// An embedder launched by a relative path that changes directory before
// creating its first runtime still finds ../lib next to its real location.
// RUN: cp %intl_chdir_host %t/bin/host
// RUN: cp %intl_fixture_dir/libfake_intl_good.so %t/lib/%intl_library_name
// RUN: cd %t && env -u LD_LIBRARY_PATH -u DYLD_LIBRARY_PATH ./bin/host / | %FileCheck --match-full-lines --check-prefix=CHDIR %s
// CHDIR: plugin

print(
  typeof __fakeIntlPlugin === 'object'
    ? JSON.stringify(__fakeIntlPlugin)
    : 'no plugin',
);
