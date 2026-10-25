/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US %s | %FileCheck --match-full-lines %s

// Extreme and odd values through format and formatToParts (plan Review
// Focus 1). Each formats exactly what the draft says: Numbers through
// Number::toString, BigInts and decimal strings exactly, and strings by
// the StringNumericLiteral grammar, with RoundMVResult deciding only
// infinity and zero. The expected strings are ICU4C's (Node 24, ICU 78.2),
// which agrees with the draft on all of them.

var en = new Intl.NumberFormat('en');

/// format(v), then whether formatToParts(v) concatenates to it.
function show(nf, v) {
  var s = nf.format(v);
  var joined = nf.formatToParts(v).map(p => p.value).join('');
  print(s, joined === s ? 'parts ok' : 'parts differ: ' + joined);
}

/// The parts of nf.formatToParts(x), as type:value, separated by "|".
function parts(nf, x) {
  return nf.formatToParts(x).map(p => p.type + ':' + p.value).join('|');
}

// 1e308, a Number close to the largest finite one, with all 309 digits.
show(en, 1e308);
// CHECK: 100,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000,000 parts ok
// The smallest denormal rounds to 0 with the default three fraction digits.
show(en, 5e-324);
// CHECK-NEXT: 0 parts ok
show(en, -0);
// CHECK-NEXT: -0 parts ok
// A BigInt keeps every digit.
show(en, 2n ** 1000n);
// CHECK-NEXT: 10,715,086,071,862,673,209,484,250,490,600,018,105,614,048,117,055,336,074,437,503,883,703,510,511,249,361,224,931,983,788,156,958,581,275,946,729,175,531,468,251,871,452,856,923,140,435,984,577,574,698,574,803,934,567,774,824,230,985,421,074,605,062,371,141,877,954,182,153,046,474,983,581,941,267,398,767,559,165,543,946,077,062,914,571,196,477,686,542,167,660,429,831,652,624,386,837,205,668,069,376 parts ok
print(
  new Intl.NumberFormat('en', {maximumSignificantDigits: 21}).format(2n ** 1000n) ===
    '10,715,086,071,862,673,209,500' + ',000'.repeat(93),
);
// CHECK-NEXT: true
show(en, 12345678901234567890n);
// CHECK-NEXT: 12,345,678,901,234,567,890 parts ok
print(parts(en, 2n ** 70n));
// CHECK-NEXT: integer:1|group:,|integer:180|group:,|integer:591|group:,|integer:620|group:,|integer:717|group:,|integer:411|group:,|integer:303|group:,|integer:424
// A decimal string is exact, past the precision of a Number.
show(en, '123456789012345678901234567890.5');
// CHECK-NEXT: 123,456,789,012,345,678,901,234,567,890.5 parts ok
// A fraction 1001 digits long rounds to 0.
show(en, '0.' + '0'.repeat(1000) + '1');
// CHECK-NEXT: 0 parts ok
// Beyond the range of a Number: infinity, and negative zero.
show(en, '1e400');
// CHECK-NEXT: ∞ parts ok
print(parts(en, '1e400'));
// CHECK-NEXT: infinity:∞
show(en, '-1e-400');
// CHECK-NEXT: -0 parts ok
print(parts(en, '-1e-400'));
// CHECK-NEXT: minusSign:-|integer:0
// White space around the literal, hexadecimal, and what the grammar
// rejects: a signed hexadecimal literal and a numeric separator. The empty
// string is 0.
show(en, ' 12 ');
// CHECK-NEXT: 12 parts ok
show(en, '0x1F');
// CHECK-NEXT: 31 parts ok
show(en, '-0x1F');
// CHECK-NEXT: NaN parts ok
print(parts(en, '-0x1F'));
// CHECK-NEXT: nan:NaN
show(en, '1_000');
// CHECK-NEXT: NaN parts ok
show(en, '');
// CHECK-NEXT: 0 parts ok
show(en, -1.5e-7);
// CHECK-NEXT: -0 parts ok
show(en, 0.1 + 0.2);
// CHECK-NEXT: 0.3 parts ok

// One hundred fraction digits, the most the options allow.
var mfd100 = new Intl.NumberFormat('en', {maximumFractionDigits: 100});
show(mfd100, '0.' + '0'.repeat(99) + '1');
// CHECK-NEXT: 0.0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001 parts ok
show(mfd100, 5e-324);
// CHECK-NEXT: 0 parts ok
// The percent scale is exact too.
show(new Intl.NumberFormat('en', {style: 'percent', maximumFractionDigits: 2}), '0.123456');
// CHECK-NEXT: 12.35% parts ok
// 21 integer digits, the most the options allow.
show(new Intl.NumberFormat('en', {minimumIntegerDigits: 21, useGrouping: false}), 1);
// CHECK-NEXT: 000000000000000000001 parts ok
