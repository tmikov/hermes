/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: cat %S/Inputs/strict.js %intl_js/01-intrinsics.js %intl_js/02-shapes.js %intl_js/03-locales.js %intl_js/04-options.js %s > %t.js
// RUN: %hermes_default -Xintl-plugin=false %t.js | %FileCheck --match-full-lines %s
// RUN: %hermesc -O -emit-binary -out=%t.hbc %t.js && %hermes_default -Xintl-plugin=false -b %t.hbc | %FileCheck --match-full-lines %s

// SetNumberFormatDigitOptions (ECMA-402 16.1.2) in isolation, with stub
// natives.

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

function digits(options, notation, mnfdDefault, mxfdDefault) {
  var r = {__proto__: null};
  SetNumberFormatDigitOptions(
    r, options,
    mnfdDefault === undefined ? 0 : mnfdDefault,
    mxfdDefault === undefined ? 3 : mxfdDefault,
    notation || 'standard');
  return [
    r.minimumIntegerDigits, r.minimumFractionDigits, r.maximumFractionDigits,
    r.minimumSignificantDigits, r.maximumSignificantDigits, r.roundingType,
    r.computedRoundingPriority, r.roundingIncrement, r.roundingMode,
    r.trailingZeroDisplay,
  ].join('|');
}
print(digits({}));
// CHECK: 1|0|3|||fractionDigits|auto|1|halfExpand|auto
print(digits({maximumSignificantDigits: 3}));
// CHECK-NEXT: 1|||1|3|significantDigits|auto|1|halfExpand|auto
print(digits({minimumFractionDigits: 2}));
// CHECK-NEXT: 1|2|3|||fractionDigits|auto|1|halfExpand|auto
print(digits({maximumFractionDigits: 1}));
// CHECK-NEXT: 1|0|1|||fractionDigits|auto|1|halfExpand|auto
print(digits({roundingPriority: 'lessPrecision', maximumSignificantDigits: 2}));
// CHECK-NEXT: 1|0|3|1|2|lessPrecision|lessPrecision|1|halfExpand|auto
print(digits({}, 'compact'));
// CHECK-NEXT: 1|0|0|1|2|morePrecision|morePrecision|1|halfExpand|auto
print(digits({roundingIncrement: 5, minimumFractionDigits: 2, maximumFractionDigits: 2}));
// CHECK-NEXT: 1|2|2|||fractionDigits|auto|5|halfExpand|auto
print(digits({minimumIntegerDigits: 4.9, roundingMode: 'trunc', trailingZeroDisplay: 'stripIfInteger'}));
// CHECK-NEXT: 4|0|3|||fractionDigits|auto|1|trunc|stripIfInteger
print(thrown(() => digits({minimumFractionDigits: 3, maximumFractionDigits: 1})));
// CHECK-NEXT: RangeError
// Step 13: a roundingIncrement other than 1 makes mxfdDefault mnfdDefault.
print(digits({roundingIncrement: 5}));
// CHECK-NEXT: 1|0|0|||fractionDigits|auto|5|halfExpand|auto
// Step 25.
print(digits({roundingPriority: 'morePrecision', maximumSignificantDigits: 2}));
// CHECK-NEXT: 1|0|3|1|2|morePrecision|morePrecision|1|halfExpand|auto
// Step 22.a.ii: the minimum of maximumSignificantDigits is mnsd.
print(thrown(() => digits({minimumSignificantDigits: 3, maximumSignificantDigits: 2})));
// CHECK-NEXT: RangeError
// Step 23.a.iii: mnfd is min(mnfdDefault, mxfd); here 1, not 2.
print(digits({maximumFractionDigits: 1}, 'standard', 2, 3));
// CHECK-NEXT: 1|1|1|||fractionDigits|auto|1|halfExpand|auto
// Step 23.a.iv: mxfd is max(mxfdDefault, mnfd): mxfdDefault when mnfd is
// smaller (3, not 1), mnfd when it is larger (5, not 3).
print(digits({minimumFractionDigits: 1}, 'standard', 2, 3));
// CHECK-NEXT: 1|1|3|||fractionDigits|auto|1|halfExpand|auto
print(digits({minimumFractionDigits: 5}));
// CHECK-NEXT: 1|5|5|||fractionDigits|auto|1|halfExpand|auto
print(thrown(() => digits({roundingIncrement: 5, maximumFractionDigits: 2})));
// CHECK-NEXT: RangeError
print(thrown(() => digits({roundingIncrement: 3})));
// CHECK-NEXT: RangeError
print(thrown(() => digits({roundingIncrement: 5, maximumSignificantDigits: 2})));
// CHECK-NEXT: TypeError
print(thrown(() => digits({roundingMode: 'bogus'})));
// CHECK-NEXT: RangeError

// Every option is read once, in spec order, before anything is validated.
var reads = [];
digits(new Proxy({}, {get(t, k) { reads.push(String(k)); return undefined; }}));
print(reads.join(','));
// CHECK-NEXT: minimumIntegerDigits,minimumFractionDigits,maximumFractionDigits,minimumSignificantDigits,maximumSignificantDigits,roundingIncrement,roundingMode,roundingPriority,trailingZeroDisplay
