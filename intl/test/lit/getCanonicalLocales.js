/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes %s | %FileCheck --match-full-lines %s

print(JSON.stringify(Intl.getCanonicalLocales()));
// CHECK: []
print(JSON.stringify(Intl.getCanonicalLocales('EN-us')));
// CHECK-NEXT: ["en-US"]
print(JSON.stringify(Intl.getCanonicalLocales(['iw', 'he', 'de-DE'])));
// CHECK-NEXT: ["he","de-DE"]
print(JSON.stringify(Intl.getCanonicalLocales('EN-latn-us-u-CA-gregory-KN-true')));
// CHECK-NEXT: ["en-Latn-US-u-ca-gregory-kn"]
print(JSON.stringify(Intl.getCanonicalLocales('en-u-ca-islamicc')));
// CHECK-NEXT: ["en-u-ca-islamic-civil"]
var a = Intl.getCanonicalLocales('en');
var b = Intl.getCanonicalLocales('en');
print(Array.isArray(a), a !== b);
// CHECK-NEXT: true true
