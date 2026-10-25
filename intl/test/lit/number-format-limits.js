/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US %s | %FileCheck --match-full-lines %s

// Values at Hermes's BigInt size limit and decimal strings with more digits
// than a BigInt can hold. Hermes's BigInt arithmetic cannot multiply,
// divide or increment a BigInt in the top digits of the limit, and a
// BigInt holds about 19,700 decimal digits; the plugin's rounding must not
// need either (07-number.js, roundToUnit and intlMVFromDigits). The
// expected strings are ICU4C's (Node 24, ICU 78.2) except where a comment
// says otherwise.

/// The length of s, its first 24 and its last 24 code units.
function sum(s) {
  return s.length + ' ' + s.slice(0, 24) + '...' + s.slice(-24);
}

/// sum of what f returns, or the error it throws.
function t(f) {
  try {
    return sum(f());
  } catch (e) {
    return e.name + ': ' + e.message;
  }
}

// The largest BigInt Hermes allows (1024 64-bit digits with the sign), and
// one 66 bits below it.
var max = BigInt('0x7' + 'f'.repeat(16383));
var x = (1n << 65469n) + 12345n;
var en = new Intl.NumberFormat('en');

print(t(() => x.toLocaleString()));
// CHECK: 26278 13,576,446,894,917,715,2...,974,371,280,911,872,057
print(t(() => en.format(-max)));
// CHECK-NEXT: 26306 -1,001,764,965,203,423,2...,793,947,952,859,578,367
print(t(() => new Intl.NumberFormat('en', {style: 'percent'}).format(max)));
// CHECK-NEXT: 26308 100,176,496,520,342,323,...394,795,285,957,836,700%
print(
  t(() =>
    new Intl.NumberFormat('en', {maximumSignificantDigits: 3}).format(max),
  ),
);
// CHECK-NEXT: 26305 1,000,000,000,000,000,00...,000,000,000,000,000,000
print(
  t(() =>
    new Intl.NumberFormat('en', {
      notation: 'scientific',
      maximumFractionDigits: 100,
    }).format(x),
  ),
);
// CHECK-NEXT: 108 1.3576446894917715286749...491630887209086992E19708
print(t(() => new Intl.NumberFormat('en', {notation: 'compact'}).format(max)));
// CHECK-NEXT: 26290 1,001,764,965,203,423,23...753,036,169,722,793,948T

// A BigInt with many trailing zeros: 10^19700 is a multiple of every
// rounding unit, so it formats without a power of ten as large as itself
// (Hermes's 10n ** 19703n fails).
print(t(() => en.format(BigInt('1' + '0'.repeat(19700)))));
// CHECK-NEXT: 26267 100,000,000,000,000,000,...,000,000,000,000,000,000

// Rounding increments at the limit. ICU4C fails here ("Internal error. Icu
// error."); the values are the draft's: 57 rounded to a multiple of 2.5 is
// 57.5.
print(
  t(() =>
    new Intl.NumberFormat('en', {
      minimumFractionDigits: 1,
      maximumFractionDigits: 1,
      roundingIncrement: 25,
    }).format(x),
  ),
);
// CHECK-NEXT: 26280 13,576,446,894,917,715,2...74,371,280,911,872,057.5

// max ends in 578,367, which rounds to a multiple of 5000 above max. The
// rounded value is still exact: without its trailing zeros it is smaller
// than max.
print(
  t(() =>
    new Intl.NumberFormat('en', {
      maximumFractionDigits: 0,
      roundingIncrement: 5000,
    }).format(max),
  ),
);
// CHECK-NEXT: 26305 1,001,764,965,203,423,23...,793,947,952,859,580,000

// PluralRules at the limit. V8 throws a TypeError for a BigInt here; the
// categories are CLDR's en rules: x + 4n ends in 61, an ordinal "one".
print(
  new Intl.PluralRules('en').select(max),
  new Intl.PluralRules('en', {type: 'ordinal'}).select(x + 4n),
);
// CHECK-NEXT: other one

// Decimal strings with more digits than a BigInt holds. The digits far
// below any rounding position are replaced by one sticky digit, so every
// rounding mode sees whether the dropped part was zero, below half or
// above half.
var zeros = '0'.repeat(30000);
var above = '0.5' + zeros + '1';
var half = '0.5' + zeros;
var twoAbove = '2.5' + zeros + '1';
var modes = ['halfEven', 'halfTrunc', 'halfExpand', 'trunc', 'ceil', 'floor'];
for (var i = 0; i < modes.length; i++) {
  var nf0 = new Intl.NumberFormat('en', {
    maximumFractionDigits: 0,
    roundingMode: modes[i],
  });
  print(
    modes[i],
    nf0.format(above),
    nf0.format(half),
    nf0.format(twoAbove),
    nf0.format('-' + twoAbove),
  );
}
// CHECK-NEXT: halfEven 1 0 3 -3
// CHECK-NEXT: halfTrunc 1 0 3 -3
// CHECK-NEXT: halfExpand 1 1 3 -3
// CHECK-NEXT: trunc 0 0 2 -2
// CHECK-NEXT: ceil 1 1 3 -2
// CHECK-NEXT: floor 0 0 2 -3

var nines = '1.' + '9'.repeat(25000);
var long = '-' + '1'.repeat(300) + '.' + '7'.repeat(25000);
var tiny = '0.' + '0'.repeat(320) + '1234567'.repeat(4000);
var frac100 = new Intl.NumberFormat('en', {maximumFractionDigits: 100});
print(t(() => frac100.format(nines)));
// CHECK-NEXT: 1 2...2
print(
  t(() =>
    new Intl.NumberFormat('en', {
      maximumFractionDigits: 100,
      roundingMode: 'trunc',
    }).format(nines),
  ),
);
// CHECK-NEXT: 102 1.9999999999999999999999...999999999999999999999999
print(t(() => frac100.format(long)));
// CHECK-NEXT: 501 -111,111,111,111,111,111...777777777777777777777778
print(
  t(() =>
    new Intl.NumberFormat('en', {
      style: 'percent',
      maximumFractionDigits: 100,
    }).format(long),
  ),
);
// CHECK-NEXT: 505 -11,111,111,111,111,111,...77777777777777777777778%
print(
  t(() =>
    new Intl.NumberFormat('en', {maximumSignificantDigits: 21}).format(long),
  ),
);
// CHECK-NEXT: 400 -111,111,111,111,111,111...,000,000,000,000,000,000
print(
  t(() =>
    new Intl.NumberFormat('en', {
      notation: 'engineering',
      maximumFractionDigits: 100,
    }).format(tiny),
  ),
);
// CHECK-NEXT: 107 1.2345671234567123456712...6712345671234567123E-321
print(
  t(() =>
    new Intl.NumberFormat('en', {
      notation: 'compact',
      maximumFractionDigits: 100,
    }).format(long),
  ),
);
// CHECK-NEXT: 486 -111,111,111,111,111,111...77777777777777777777778T
print(
  new Intl.PluralRules('en').select(twoAbove),
  new Intl.PluralRules('en', {maximumFractionDigits: 0}).select(above),
  new Intl.PluralRules('en', {maximumFractionDigits: 0}).select(half),
  new Intl.PluralRules('en', {maximumFractionDigits: 100}).select(nines),
);
// CHECK-NEXT: other one one other

// Hexadecimal, octal and binary strings with more digits than a BigInt
// holds. Their values are beyond the range of a Number, so they are
// positive-infinity (ToIntlMathematicalValue step 9.c), as +s is, without
// a BigInt of their digits being made.
/// What f returns, or the error it throws.
function r(f) {
  try {
    return f();
  } catch (e) {
    return e.name + ': ' + e.message;
  }
}
/// The parts of nf.formatToParts(x), as type:value, separated by "|".
function parts(nf, x) {
  return nf.formatToParts(x).map(p => p.type + ':' + p.value).join('|');
}
var nonDecimal = [
  '0x' + 'f'.repeat(17000),
  ' 0X' + 'A'.repeat(17000) + ' ',
  '0o' + '7'.repeat(23000),
  '0O' + '1'.repeat(23000),
  '0b' + '1'.repeat(70000),
  '0B1' + '0'.repeat(70000),
];
var en0 = new Intl.NumberFormat('en');
var pr0 = new Intl.PluralRules('en');
for (var i = 0; i < nonDecimal.length; i++) {
  var s = nonDecimal[i];
  print(
    r(() => en0.format(s)),
    r(() => parts(en0, s)),
    r(() => en0.formatRange(1, s)),
    r(() => pr0.select(s)),
    +s,
  );
}
// CHECK-NEXT: ∞ infinity:∞ 1–∞ other Infinity
// CHECK-NEXT: ∞ infinity:∞ 1–∞ other Infinity
// CHECK-NEXT: ∞ infinity:∞ 1–∞ other Infinity
// CHECK-NEXT: ∞ infinity:∞ 1–∞ other Infinity
// CHECK-NEXT: ∞ infinity:∞ 1–∞ other Infinity
// CHECK-NEXT: ∞ infinity:∞ 1–∞ other Infinity
// Within the limit but beyond a Number, and below 2^1024, where the
// digits are kept.
print(en0.format('0x1' + '0'.repeat(256)), pr0.select('0x' + 'f'.repeat(300)));
// CHECK-NEXT: ∞ other
print(en0.format('0x' + 'f'.repeat(16)), en0.format('0b' + '1'.repeat(64)));
// CHECK-NEXT: 18,446,744,073,709,551,615 18,446,744,073,709,551,615
// A malformed literal of that length is not-a-number: every digit is
// checked before the value is.
var malformed = [
  '0x' + 'f'.repeat(17000) + 'g',
  '0o' + '7'.repeat(23000) + '8',
  '0b' + '1'.repeat(70000) + '2',
  '-0x' + 'f'.repeat(17000),
  '0x' + 'f'.repeat(17000) + '.5',
];
for (var i = 0; i < malformed.length; i++) {
  var s = malformed[i];
  print(r(() => en0.format(s)), r(() => pr0.select(s)), +s);
}
// CHECK-NEXT: NaN other NaN
// CHECK-NEXT: NaN other NaN
// CHECK-NEXT: NaN other NaN
// CHECK-NEXT: NaN other NaN
// CHECK-NEXT: NaN other NaN
// Long runs of leading zeros leave a small value, kept exactly: 2^53 + 1
// and 2^1020 - 1 are not Numbers.
print(
  r(() => en0.format('0x' + '0'.repeat(20000) + '1')),
  r(() => en0.format('0o' + '0'.repeat(23000) + '17')),
  r(() => en0.format('0b' + '0'.repeat(70000) + '101')),
  r(() => pr0.select('0x' + '0'.repeat(20000) + '1')),
);
// CHECK-NEXT: 1 15 5 one
print(r(() => en0.format('0b' + '0'.repeat(70000) + '1' + '0'.repeat(52) + '1')));
// CHECK-NEXT: 9,007,199,254,740,993
var f255 = '0x' + '0'.repeat(20000) + 'f'.repeat(255);
print(
  r(() => en0.format(f255) === en0.format(2n ** 1020n - 1n)),
  r(() => en0.format(f255)) !== en0.format(+f255),
);
// CHECK-NEXT: true true
