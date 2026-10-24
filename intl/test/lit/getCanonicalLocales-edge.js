/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes %s | %FileCheck --match-full-lines %s

// Inputs a person might pass by mistake: each gives the spec's error, or is
// skipped, and none crashes.

function thrown(f) {
  try {
    f();
    return 'none';
  } catch (e) {
    return e.constructor.name;
  }
}
print(thrown(() => Intl.getCanonicalLocales('en\uD800')));
// CHECK: RangeError
print(thrown(() => Intl.getCanonicalLocales('en-\uDC00a')));
// CHECK-NEXT: RangeError
print(thrown(() => Intl.getCanonicalLocales('a'.repeat(10000))));
// CHECK-NEXT: RangeError
// A well-formed tag of about 10,000 characters: 1200 distinct variants, given
// in descending order, which UTS 35 canonicalization sorts.
var ascending = 'en';
var descending = 'EN';
for (var i = 0; i < 1200; i++) {
  ascending += '-abc' + String(10000 + i).slice(1);
  descending += '-abc' + String(10000 + 1199 - i).slice(1);
}
var long = Intl.getCanonicalLocales(descending);
print(ascending.length, long.length, long[0] === ascending);
// CHECK-NEXT: 9602 1 true
print(thrown(() => Intl.getCanonicalLocales({get length() { throw new SyntaxError('length'); }})));
// CHECK-NEXT: SyntaxError
print(JSON.stringify(Intl.getCanonicalLocales({length: -5, 0: 'en'})));
// CHECK-NEXT: []
print(thrown(() => Intl.getCanonicalLocales({length: 2 ** 53, 0: 'en', get 1() { throw new EvalError('stop'); }})));
// CHECK-NEXT: EvalError
print(thrown(() => Intl.getCanonicalLocales([Symbol('x')])));
// CHECK-NEXT: TypeError
print(thrown(() => Intl.getCanonicalLocales([null])));
// CHECK-NEXT: TypeError
print(thrown(() => Intl.getCanonicalLocales(null)));
// CHECK-NEXT: TypeError
print(JSON.stringify(Intl.getCanonicalLocales([, 'de', , 'EN'])));
// CHECK-NEXT: ["de","en"]
print(thrown(() => Intl.getCanonicalLocales(['en_US'])));
// CHECK-NEXT: RangeError
print(thrown(() => Intl.getCanonicalLocales([''])));
// CHECK-NEXT: RangeError
// A tlang is canonicalized as a languageId (UTS 35 Annex C): language,
// region and variant aliases all apply.
print(JSON.stringify(Intl.getCanonicalLocales(['en-t-ja-DD', 'en-t-iw', 'en-t-sv-aaland'])));
// CHECK-NEXT: ["en-t-ja-de","en-t-he","en-t-sv-ax"]
