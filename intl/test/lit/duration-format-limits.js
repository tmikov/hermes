/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US %s | %FileCheck --match-full-lines %s

// Intl.DurationFormat at its limits (plan Review Focus 4): the largest
// years, months and weeks, the smallest durations that fail
// IsValidDuration, the exact sum of the normalized seconds and of the
// fractional units, fields formatted with their exact values (plan
// decision 7), and fractionalDigits 0 to 9. Where Node's arithmetic is
// inexact, the comments give its result.

var J = JSON.stringify;
/// The class of the exception f throws, or 'none'.
function thrown(f) {
  try {
    f();
    return 'none';
  } catch (e) {
    return e.constructor.name;
  }
}
var en = new Intl.DurationFormat('en');
/// en's format of duration, or the class of the exception it throws.
function fmt(duration) {
  try {
    return J(en.format(duration));
  } catch (e) {
    return e.constructor.name;
  }
}

// Years, months and weeks must be below 2^32.
print(fmt({years: 2 ** 32 - 1}), fmt({years: 2 ** 32}), fmt({months: -(2 ** 32 - 1)}), fmt({weeks: -(2 ** 32)}));
// CHECK: "4,294,967,295 yrs" RangeError "-4,294,967,295 mths" RangeError
// The normalized seconds must be below 2^53 in magnitude.
print(fmt({seconds: 2 ** 53 - 1}), fmt({seconds: 2 ** 53}), fmt({seconds: -(2 ** 53 - 1)}), fmt({seconds: -(2 ** 53)}));
// CHECK-NEXT: "9,007,199,254,740,991 sec" RangeError "-9,007,199,254,740,991 sec" RangeError
// The sum is exact: 2^53 - 1 seconds and 999 milliseconds is below 2^53,
// and 1000 more milliseconds reach it.
print(fmt({seconds: 2 ** 53 - 1, milliseconds: 999}), fmt({seconds: 2 ** 53 - 1, milliseconds: 1000}));
// CHECK-NEXT: "9,007,199,254,740,991 sec, 999 ms" RangeError
print(fmt({seconds: 2 ** 53 - 1, nanoseconds: 999999999}), fmt({seconds: 2 ** 53 - 1, nanoseconds: 1000000000}));
// CHECK-NEXT: "9,007,199,254,740,991 sec, 999,999,999 ns" RangeError
// The other units' contributions: 104249991374 days is 9007199254713600
// seconds, below 2^53; one more day is not.
print(fmt({days: 104249991374}), fmt({days: 104249991375}), fmt({hours: 2501999792983}), fmt({hours: 2501999792984}));
// CHECK-NEXT: "104,249,991,374 days" RangeError "2,501,999,792,983 hr" RangeError
// A field is formatted with its exact value, not its shortest decimal
// string (plan decision 7): 4503599627370497000 is exactly
// 4503599627370497024 (Node shows "4,503,599,627,370,497,000 ms").
print(fmt({milliseconds: 4503599627370496000}), fmt({milliseconds: 4503599627370497000}));
// CHECK-NEXT: "4,503,599,627,370,496,000 ms" "4,503,599,627,370,497,024 ms"
// The fractional sum is exact (Node's inexact sum gives "…991000000").
print(J(new Intl.DurationFormat('en', {style: 'digital', fractionalDigits: 9}).format({seconds: 9007199, milliseconds: 254740991, nanoseconds: 1})));
// CHECK-NEXT: "0:00:9261939.991000001"
print(fmt({nanoseconds: Number.MAX_SAFE_INTEGER}), fmt({microseconds: -Number.MAX_SAFE_INTEGER}));
// CHECK-NEXT: "9,007,199,254,740,991 ns" "-9,007,199,254,740,991 μs"
// Nanoseconds near 2^53 × 10^9, which is a Number: 9007199254740991e9 is
// exactly 9007199254740990926258176, below it (Node shows the shortest
// decimal string, "9,007,199,254,740,991,000,000,000 ns").
print(fmt({nanoseconds: 9007199254740991e9}), fmt({nanoseconds: 9007199254740992e9}));
// CHECK-NEXT: "9,007,199,254,740,990,926,258,176 ns" RangeError

// fractionalDigits from 0 to 9.
var duration = {seconds: 1, milliseconds: 234, microseconds: 567, nanoseconds: 891};
print(J(new Intl.DurationFormat('en', {style: 'digital', fractionalDigits: 0}).format(duration)), J(new Intl.DurationFormat('en', {style: 'digital', fractionalDigits: 9}).format(duration)));
// CHECK-NEXT: "0:00:01" "0:00:01.234567891"
print(thrown(() => new Intl.DurationFormat('en', {fractionalDigits: 10})), thrown(() => new Intl.DurationFormat('en', {fractionalDigits: -1})), thrown(() => new Intl.DurationFormat('en', {fractionalDigits: NaN})));
// CHECK-NEXT: RangeError RangeError RangeError
// GetNumberOption floors the value.
print(new Intl.DurationFormat('en', {fractionalDigits: 2.9}).resolvedOptions().fractionalDigits);
// CHECK-NEXT: 2
