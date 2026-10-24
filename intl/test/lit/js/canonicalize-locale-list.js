/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: cat %S/Inputs/strict.js %intl_js/01-intrinsics.js %intl_js/02-shapes.js %intl_js/03-locales.js %intl_js/04-options.js %s > %t.js
// RUN: %hermes_default -Xintl-plugin=false %t.js | %FileCheck --match-full-lines %s
// RUN: %hermesc -O -emit-binary -out=%t.hbc %t.js && %hermes_default -Xintl-plugin=false -b %t.hbc | %FileCheck --match-full-lines %s

// CanonicalizeLocaleList (ECMA-402 9.2.1) in isolation, with stub natives.

// Stubs for what the plugin's C++ passes to setup.
var available = ['en', 'en-US', 'de', 'fr'];
var native = {
  canonicalizeLocale: function (tag) {
    if (!/^[a-z]{2,3}(-[A-Za-z0-9]{1,8})*$/.test(tag)) return undefined;
    // One UTS 35 value alias, so that callers which read a value back from
    // the canonical tag see it change.
    return tag.replace(/-u-ca-islamicc(?=-|$)/, '-u-ca-islamic-civil');
  },
  isAvailableLocale: function (tag) {
    return available.indexOf(tag) !== -1;
  },
};
var hostDefaults = {locales: ['fr'], timeZone: 'UTC'};
/// The class of the exception f throws, or 'none'.
function thrown(f) {
  try {
    f();
    return 'none';
  } catch (e) {
    return e.constructor.name;
  }
}

print(JSON.stringify(CanonicalizeLocaleList(undefined)));
// CHECK: []
print(JSON.stringify(CanonicalizeLocaleList('en')));
// CHECK-NEXT: ["en"]
print(JSON.stringify(CanonicalizeLocaleList(['de', 'en', 'de'])));
// CHECK-NEXT: ["de","en"]
print(JSON.stringify(CanonicalizeLocaleList({length: 3, 0: 'fr', 2: 'de'})));
// CHECK-NEXT: ["fr","de"]
print(JSON.stringify(CanonicalizeLocaleList({length: 1, 0: {toString() { return 'en'; }}})));
// CHECK-NEXT: ["en"]
print(thrown(() => CanonicalizeLocaleList([5])));
// CHECK-NEXT: TypeError
print(thrown(() => CanonicalizeLocaleList([null])));
// CHECK-NEXT: TypeError
print(thrown(() => CanonicalizeLocaleList([Symbol()])));
// CHECK-NEXT: TypeError
print(thrown(() => CanonicalizeLocaleList(['en_US'])));
// CHECK-NEXT: RangeError
print(thrown(() => CanonicalizeLocaleList(null)));
// CHECK-NEXT: TypeError
print(JSON.stringify(CanonicalizeLocaleList(5)));
// CHECK-NEXT: []

// Observable operations, in spec order.
var log = [];
var p = new Proxy({length: 2, 0: 'en', 1: 'fr'}, {
  get(t, k) { log.push('get ' + String(k)); return t[k]; },
  has(t, k) { log.push('has ' + String(k)); return k in t; },
});
print(JSON.stringify(CanonicalizeLocaleList(p)));
// CHECK-NEXT: ["en","fr"]
print(log.join(', '));
// CHECK-NEXT: get length, has 0, get 0, has 1, get 1
