/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes %s | %FileCheck --match-full-lines %s
// RUN: %hermes_default %s | %FileCheck --match-full-lines %s

// The plugin installs Intl with the attributes ECMA-262 18 gives the global
// object's data properties (writable, non-enumerable, configurable), and
// Intl's @@toStringTag (ECMA-402 8.1.1).

var d = Object.getOwnPropertyDescriptor(globalThis, 'Intl');
print(typeof d.value, d.writable, d.enumerable, d.configurable);
// CHECK: object true false true
print(Object.getPrototypeOf(Intl) === Object.prototype);
// CHECK-NEXT: true
print(Object.prototype.toString.call(Intl));
// CHECK-NEXT: [object Intl]
var t = Object.getOwnPropertyDescriptor(Intl, Symbol.toStringTag);
print(t.value, t.writable, t.enumerable, t.configurable);
// CHECK-NEXT: Intl false false true
