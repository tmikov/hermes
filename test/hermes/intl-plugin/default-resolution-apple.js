/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// REQUIRES: apple
// Apple platforms have no search-path step in default resolution: dyld's
// errors for a leaf name cannot reliably tell "absent" from "broken", and a
// leaf name also searches the current directory. A plugin reachable only
// through DYLD_LIBRARY_PATH or the current directory is not loaded, whether
// it works or not. As in default-resolution.js, a copy of the binary in a
// scratch tree controls what it finds next to itself and in ../lib
// (nothing).

// RUN: rm -rf %t && mkdir -p %t/bin %t/lib %t/search
// RUN: cp %hermes_path %t/bin/hermes

// RUN: cp %intl_fixture_dir/libfake_intl_good.so %t/search/%intl_library_name
// RUN: env DYLD_LIBRARY_PATH=%t/search %t/bin/hermes %s | %FileCheck --match-full-lines --check-prefix=NONE %s
// RUN: cd %t/search && env -u DYLD_LIBRARY_PATH %t/bin/hermes %s | %FileCheck --match-full-lines --check-prefix=NONE %s
// RUN: echo "not a library" > %t/search/%intl_library_name
// RUN: env DYLD_LIBRARY_PATH=%t/search %t/bin/hermes %s | %FileCheck --match-full-lines --check-prefix=NONE %s
// RUN: cd %t/search && env -u DYLD_LIBRARY_PATH %t/bin/hermes %s | %FileCheck --match-full-lines --check-prefix=NONE %s
// NONE: no plugin

print(
  typeof __fakeIntlPlugin === 'object'
    ? JSON.stringify(__fakeIntlPlugin)
    : 'no plugin',
);
