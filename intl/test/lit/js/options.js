/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: cat %S/Inputs/strict.js %intl_js/01-intrinsics.js %intl_js/02-shapes.js %intl_js/03-locales.js %intl_js/04-options.js %s > %t.js
// RUN: %hermes_default -Xintl-plugin=false %t.js | %FileCheck --match-full-lines %s
// RUN: %hermesc -O -emit-binary -out=%t.hbc %t.js && %hermes_default -Xintl-plugin=false -b %t.hbc | %FileCheck --match-full-lines %s

// The option getters (ECMA-262 7.3.36, ECMA-402 9.2.10 to 9.2.14) in
// isolation, with stub natives.

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

var opts = {
  a: 'x', c: 0, n: '2.7', bad: 'zzz',
  obj: {toString() { return 'lookup'; }},
};
print(GetOption(opts, 'a', 'string', undefined, 'd'));
// CHECK: x
print(GetOption(opts, 'missing', 'string', undefined, 'd'));
// CHECK-NEXT: d
print(GetOption(opts, 'c', 'boolean', undefined, true));
// CHECK-NEXT: false
print(GetOption(opts, 'obj', 'string', ['lookup', 'best fit'], 'best fit'));
// CHECK-NEXT: lookup
print(thrown(() => GetOption(opts, 'bad', 'string', ['lookup'], 'lookup')));
// CHECK-NEXT: RangeError
print(thrown(() => GetOption(opts, 'missing', 'string', undefined, REQUIRED)));
// CHECK-NEXT: TypeError
print(thrown(() => GetOption({s: Symbol()}, 's', 'string', undefined, 'd')));
// CHECK-NEXT: TypeError
// ToString uses hint string: toString wins over valueOf.
print(GetOption({o: {toString() { return 's'; }, valueOf() { return 'v'; }}}, 'o', 'string', undefined, 'd'));
// CHECK-NEXT: s

print(GetNumberOption(opts, 'n', 1, 5, 3));
// CHECK-NEXT: 2
print(GetNumberOption(opts, 'missing', 1, 5, 3));
// CHECK-NEXT: 3
print(thrown(() => GetNumberOption({v: 'abc'}, 'v', 0, 5, 1)));
// CHECK-NEXT: RangeError
print(thrown(() => GetNumberOption({v: 6}, 'v', 0, 5, 1)));
// CHECK-NEXT: RangeError
print(thrown(() => GetNumberOption({v: 1n}, 'v', 0, 5, 1)));
// CHECK-NEXT: TypeError
// ℝ(-0𝔽) is the mathematical 0, so -0 and "-0" give +0, not -0.
print(Object.is(GetNumberOption({v: -0}, 'v', 0, 5, 1), -0), Object.is(GetNumberOption({v: '-0'}, 'v', 0, 5, 1), -0));
// CHECK-NEXT: false false
print(Object.is(DefaultNumberOption(-0, 0, 5, 1), 0));
// CHECK-NEXT: true

var values = ['min2', 'auto', 'always'];
function bs(v) {
  return String(GetBooleanOrStringNumberFormatOption({g: v}, 'g', values, 'auto'));
}
print([bs(true), bs(false), bs('min2'), bs(0), bs(''), bs(undefined)].join(' '));
// CHECK-NEXT: true false min2 false false auto
// The strings "true" and "false" are ordinary strings here; NumberFormat's
// useGrouping maps them to its fallback at its own call site (Phase 3).
print(thrown(() => bs('false')));
// CHECK-NEXT: RangeError
print(thrown(() => bs('x')));
// CHECK-NEXT: RangeError

print(Object.getPrototypeOf(GetOptionsObject(undefined)));
// CHECK-NEXT: null
print(thrown(() => GetOptionsObject('x')));
// CHECK-NEXT: TypeError
print(typeof CoerceOptionsToObject('x'));
// CHECK-NEXT: object
