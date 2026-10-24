/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// REQUIRES: glibc
// shermes -exec dlopen()s the compiled program RTLD_LOCAL, and with it
// libhermesvm. Here libhermesvm is a copy in a directory without the plugin,
// so the loader can only find the plugin on the library search path. It
// promotes libhermesvm to the global scope only when the plugin is there,
// which glibc's LD_DEBUG=scopes reports.

// RUN: rm -rf %t && mkdir -p %t/vm %t/search
// RUN: cp %hermesvm_lib %t/vm/libhermesvm.so

// No plugin anywhere: no Intl, and libhermesvm stays local.
// RUN: env LD_LIBRARY_PATH=%t/vm LD_DEBUG=scopes %shermes -exec %s 2> %t/absent.err | %FileCheck --match-full-lines --check-prefix=ABSENT %s
// RUN: not grep -F "libhermesvm.so [0] to global scope" %t/absent.err
// ABSENT: no Intl

// The plugin on the search path: it loads after the promotion.
// RUN: cp %intl_plugin %t/search/libhermesintl.so
// RUN: env LD_LIBRARY_PATH=%t/vm:%t/search LD_DEBUG=scopes %shermes -exec %s 2> %t/found.err | %FileCheck --match-full-lines --check-prefix=FOUND %s
// RUN: grep -F "/vm/libhermesvm.so [0] to global scope" %t/found.err
// FOUND: [object Intl]

print(
  typeof Intl === 'object' ? Object.prototype.toString.call(Intl) : 'no Intl',
);
