/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// UNSUPPORTED: apple
// The last step of default resolution, except on Apple platforms: the
// library search path. A library there counts as absent only when the C
// library's error says it was not found; anything else is fatal. As in
// default-resolution.js, a copy of the binary in a scratch tree controls what
// it finds next to itself and in ../lib (nothing).

// RUN: rm -rf %t && mkdir -p %t/bin %t/lib %t/search
// RUN: cp %hermes_path %t/bin/hermes

// On the search path.
// RUN: cp %intl_fixture_dir/libfake_intl_good.so %t/search/%intl_library_name
// RUN: env LD_LIBRARY_PATH=%t/search %t/bin/hermes %s | %FileCheck --check-prefix=FOUND %s
// FOUND: "detections":1

// Broken on the search path is fatal.
// RUN: echo "not a library" > %t/search/%intl_library_name
// RUN: env LD_LIBRARY_PATH=%t/search %not --crash %t/bin/hermes %s 2>&1 | %FileCheck --check-prefix=SEARCHBROKEN %s
// SEARCHBROKEN: LLVM ERROR: Intl plugin libhermesintl.so: cannot be loaded:

// A plugin on the search path whose own dependency is missing is broken,
// not absent.
// RUN: cp %intl_fixture_dir/libfake_intl_needs_dep.so %t/search/%intl_library_name
// RUN: env LD_LIBRARY_PATH=%t/search %not --crash %t/bin/hermes %s 2>&1 | %FileCheck --check-prefix=NEEDSDEP %s
// NEEDSDEP: LLVM ERROR: Intl plugin libhermesintl.so: cannot be loaded: {{.*}}fake_intl_dep

print(
  typeof __fakeIntlPlugin === 'object'
    ? JSON.stringify(__fakeIntlPlugin)
    : 'no plugin',
);
