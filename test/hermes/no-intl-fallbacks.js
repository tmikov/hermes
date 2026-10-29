/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: _HERMES_TEST_LOCALE=en_US %hermes -Xintl-plugin=false %s | %FileCheck --match-full-lines %s
// RUN: _HERMES_TEST_LOCALE=en_US %hermes -Xintl-plugin=false -Xintl %s | %FileCheck --match-full-lines %s
// RUN: _HERMES_TEST_LOCALE=en_US %hermes -Xintl-plugin=false -Xintl=false %s | %FileCheck --match-full-lines %s

// Without the Intl plugin there is no Intl, and the locale-sensitive
// built-ins use Hermes's own fallbacks, none of which throws. The deprecated
// -Xintl changes nothing. no-intl-fallbacks-tr.js checks that the case
// mappings still follow the default locale.

"use strict";

print(typeof Intl);
// CHECK: undefined

print((1234.5).toLocaleString(), Infinity.toLocaleString(),
      (-Infinity).toLocaleString(), NaN.toLocaleString());
// CHECK-NEXT: 1234.5 Infinity -Infinity NaN

print((12n).toLocaleString());
// CHECK-NEXT: 12

print([1, 2].toLocaleString(), new Uint8Array([1, 2]).toLocaleString());
// CHECK-NEXT: 1,2 1,2

// ECMA-262's Array.prototype.toLocaleString and
// %TypedArray%.prototype.toLocaleString call each element's toLocaleString
// with no arguments; ECMA-402's pass on locales and options.
var element = {
  toLocaleString: function () {
    return 'args=' + arguments.length;
  },
};
print([element].toLocaleString('de', {}));
// CHECK-NEXT: args=0
var numberToLocaleString = Number.prototype.toLocaleString;
Number.prototype.toLocaleString = function () {
  return 'args=' + arguments.length;
};
print(new Uint8Array([1]).toLocaleString('de', {}));
// CHECK-NEXT: args=0
Number.prototype.toLocaleString = numberToLocaleString;

var invalid = new Date(NaN);
print(invalid.toLocaleString(), '|', invalid.toLocaleDateString(), '|',
      invalid.toLocaleTimeString());
// CHECK-NEXT: Invalid Date | Invalid Date | Invalid Date
var date = new Date(2020, 0, 2, 3, 4, 5);
function nonEmpty(s) {
  return typeof s === 'string' && s.length > 0;
}
print(nonEmpty(date.toLocaleString()), nonEmpty(date.toLocaleDateString()),
      nonEmpty(date.toLocaleTimeString()));
// CHECK-NEXT: true true true

print('I'.toLocaleLowerCase(), 'i'.toLocaleUpperCase());
// CHECK-NEXT: i I

print('a'.localeCompare('b') < 0, 'b'.localeCompare('a') > 0,
      'a'.localeCompare('a'));
// CHECK-NEXT: true true 0
