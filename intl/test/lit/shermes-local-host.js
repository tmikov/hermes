/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// shermes -exec compiles this file into a shared object and dlopen()s it
// RTLD_LOCAL, so libhermesvm, its dependency, stays out of the global symbol
// scope. The loader promotes libhermesvm to global before loading the
// plugin, so the plugin, which links no JSI provider, still binds to it.
// RUN: %shermes -exec %s | %FileCheck --match-full-lines %s

print(Object.prototype.toString.call(Intl));
// CHECK: [object Intl]
