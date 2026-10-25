/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: cat %S/Inputs/escape.js %s > %t.js
// RUN: %hermes -Xintl-default-locale=en-US %t.js | %FileCheck --match-full-lines %s

// The basic surface of Intl.NumberFormat with the decimal and percent
// styles: the resolved options, grouping, signs, non-finite values, the
// constructor without new, supportedLocalesOf and the bound format. The
// expected strings are ICU4C's (Node 24, ICU 78.2).

/// The parts of nf.formatToParts(x), as type:value, separated by "|" and
/// escaped.
function parts(nf, x) {
  return escape(nf.formatToParts(x).map(p => p.type + ':' + p.value).join('|'));
}

print(stringify(new Intl.NumberFormat().resolvedOptions()));
// CHECK: {"locale":"en-US","numberingSystem":"latn","style":"decimal","minimumIntegerDigits":1,"minimumFractionDigits":0,"maximumFractionDigits":3,"useGrouping":"auto","notation":"standard","signDisplay":"auto","roundingIncrement":1,"roundingMode":"halfExpand","roundingPriority":"auto","trailingZeroDisplay":"auto"}

// The currency and unit options are resolved and stored (number-format-
// currency.js and number-format-unit.js format with them); the significant
// digits replace the fraction digits; compact notation defaults useGrouping
// to "min2".
print(
  stringify(
    new Intl.NumberFormat('en', {style: 'currency', currency: 'usd'}).resolvedOptions(),
  ),
);
// CHECK-NEXT: {"locale":"en","numberingSystem":"latn","style":"currency","currency":"USD","currencyDisplay":"symbol","currencySign":"standard","minimumIntegerDigits":1,"minimumFractionDigits":2,"maximumFractionDigits":2,"useGrouping":"auto","notation":"standard","signDisplay":"auto","roundingIncrement":1,"roundingMode":"halfExpand","roundingPriority":"auto","trailingZeroDisplay":"auto"}
print(
  stringify(
    new Intl.NumberFormat('en', {style: 'currency', currency: 'JPY'}).resolvedOptions()
      .maximumFractionDigits,
  ),
);
// CHECK-NEXT: 0
print(
  stringify(
    new Intl.NumberFormat('en', {style: 'unit', unit: 'kilometer-per-hour'}).resolvedOptions(),
  ),
);
// CHECK-NEXT: {"locale":"en","numberingSystem":"latn","style":"unit","unit":"kilometer-per-hour","unitDisplay":"short","minimumIntegerDigits":1,"minimumFractionDigits":0,"maximumFractionDigits":3,"useGrouping":"auto","notation":"standard","signDisplay":"auto","roundingIncrement":1,"roundingMode":"halfExpand","roundingPriority":"auto","trailingZeroDisplay":"auto"}
print(stringify(new Intl.NumberFormat('en', {notation: 'compact'}).resolvedOptions()));
// CHECK-NEXT: {"locale":"en","numberingSystem":"latn","style":"decimal","minimumIntegerDigits":1,"minimumFractionDigits":0,"maximumFractionDigits":0,"minimumSignificantDigits":1,"maximumSignificantDigits":2,"useGrouping":"min2","notation":"compact","compactDisplay":"short","signDisplay":"auto","roundingIncrement":1,"roundingMode":"halfExpand","roundingPriority":"morePrecision","trailingZeroDisplay":"auto"}
print(stringify(new Intl.NumberFormat('en', {maximumSignificantDigits: 3}).resolvedOptions()));
// CHECK-NEXT: {"locale":"en","numberingSystem":"latn","style":"decimal","minimumIntegerDigits":1,"minimumSignificantDigits":1,"maximumSignificantDigits":3,"useGrouping":"auto","notation":"standard","signDisplay":"auto","roundingIncrement":1,"roundingMode":"halfExpand","roundingPriority":"auto","trailingZeroDisplay":"auto"}
print(new Intl.NumberFormat('en', {style: 'percent'}).resolvedOptions().maximumFractionDigits);
// CHECK-NEXT: 0

// Grouping by the locale's sizes: three and three, three then two.
print(
  new Intl.NumberFormat('en-US').format(1234567.891),
  new Intl.NumberFormat('de').format(1234567.891),
  new Intl.NumberFormat('en-IN').format(1234567.891),
);
// CHECK-NEXT: 1,234,567.891 1.234.567,891 12,34,567.891
print(parts(new Intl.NumberFormat('en-US'), -1234.5));
// CHECK-NEXT: minusSign:-|integer:1|group:,|integer:234|decimal:.|fraction:5

// Percent: the value times 100, with the locale's percent pattern.
print(
  stringify(new Intl.NumberFormat('en-US', {style: 'percent'}).format(0.256)),
  stringify(new Intl.NumberFormat('de', {style: 'percent'}).format(0.256)),
);
// CHECK-NEXT: "26%" "26\u00a0%"
print(parts(new Intl.NumberFormat('de', {style: 'percent'}), -0.256));
// CHECK-NEXT: minusSign:-|integer:26|literal:\u00a0|percentSign:%
print(stringify(new Intl.NumberFormat('tr', {style: 'percent'}).format(-0.5)));
// CHECK-NEXT: "-%50"
// Percents group by the percent pattern's own sizes: bn groups decimals by
// three then two, and percents by threes.
print(
  new Intl.NumberFormat('bn', {style: 'percent'}).format(12345.67),
  new Intl.NumberFormat('bn').format(1234567),
);
// CHECK-NEXT: ১,২৩৪,৫৬৭% ১২,৩৪,৫৬৭
// blo's percent pattern "% #,#0" has a primary size of 2, and its decimal
// pattern one of 3.
print(
  stringify(new Intl.NumberFormat('blo', {style: 'percent'}).format(12345678.9)),
  stringify(new Intl.NumberFormat('blo').format(1234567890)),
);
// CHECK-NEXT: "%\u00a012\u00a034\u00a056\u00a078\u00a090" "1\u00a0234\u00a0567\u00a0890"
print(new Intl.NumberFormat('en-IN', {style: 'percent'}).format(1234567));
// CHECK-NEXT: 12,34,56,700%

// useGrouping: false never groups; "always" ignores the locale's minimum
// grouping digits (pl has 2); "min2" needs two digits in the first group.
print(new Intl.NumberFormat('en', {useGrouping: false}).format(1234567));
// CHECK-NEXT: 1234567
print(
  stringify(new Intl.NumberFormat('pl', {useGrouping: 'always'}).format(1234)),
  stringify(new Intl.NumberFormat('pl').format(1234)),
  stringify(new Intl.NumberFormat('pl').format(12345)),
);
// CHECK-NEXT: "1\u00a0234" "1234" "12\u00a0345"
print(
  new Intl.NumberFormat('en', {useGrouping: 'min2'}).format(1234),
  new Intl.NumberFormat('en', {useGrouping: 'min2'}).format(12345),
  new Intl.NumberFormat('en', {useGrouping: true}).resolvedOptions().useGrouping,
  new Intl.NumberFormat('en', {useGrouping: 'false'}).resolvedOptions().useGrouping,
  new Intl.NumberFormat('en', {useGrouping: false}).resolvedOptions().useGrouping,
);
// CHECK-NEXT: 1234 12,345 always auto false

// signDisplay.
var exceptZero = new Intl.NumberFormat('en', {signDisplay: 'exceptZero'});
print(exceptZero.format(0), exceptZero.format(-0), exceptZero.format(1), exceptZero.format(-1));
// CHECK-NEXT: 0 0 +1 -1
var always = new Intl.NumberFormat('en', {signDisplay: 'always'});
print(always.format(0), always.format(-0), always.format(5), always.format(NaN));
// CHECK-NEXT: +0 -0 +5 +NaN
var negative = new Intl.NumberFormat('en', {signDisplay: 'negative'});
print(negative.format(-0), negative.format(-0.0001), negative.format(-1));
// CHECK-NEXT: 0 0 -1
print(new Intl.NumberFormat('en', {signDisplay: 'never'}).format(-5));
// CHECK-NEXT: 5
print(new Intl.NumberFormat('en').format(-0));
// CHECK-NEXT: -0

// NaN and the infinities.
var en = new Intl.NumberFormat('en');
print(en.format(NaN), en.format(Infinity), en.format(-Infinity));
// CHECK-NEXT: NaN ∞ -∞
print(parts(en, -Infinity), parts(en, NaN));
// CHECK-NEXT: minusSign:-|infinity:∞ nan:NaN

// Bidi marks around the sign are literal parts (he).
print(parts(new Intl.NumberFormat('he'), -1));
// CHECK-NEXT: literal:\u200e|minusSign:-|integer:1

// Without new, Intl.NumberFormat makes a NumberFormat from its prototype.
var called = Intl.NumberFormat('de');
print(
  Object.getPrototypeOf(called) === Intl.NumberFormat.prototype,
  called.resolvedOptions().locale,
  called.format(1234.5),
);
// CHECK-NEXT: true de 1.234,5
print(stringify(Intl.NumberFormat.supportedLocalesOf(['de', 'xx'])));
// CHECK-NEXT: ["de"]
// The bound format is made once.
var nf = new Intl.NumberFormat();
print(nf.format === nf.format, nf.format(5), [1, 22, 333].map(nf.format).join(' '));
// CHECK-NEXT: true 5 1 22 333
print(Object.prototype.toString.call(new Intl.NumberFormat()));
// CHECK-NEXT: [object Intl.NumberFormat]

// The option errors.
/// The class of the exception f throws, or 'none'.
function thrown(f) {
  try {
    f();
    return 'none';
  } catch (e) {
    return e.constructor.name;
  }
}
print(
  thrown(() => new Intl.NumberFormat('en', {style: 'currency'})),
  thrown(() => new Intl.NumberFormat('en', {currency: 'US'})),
  thrown(() => new Intl.NumberFormat('en', {style: 'unit'})),
  thrown(() => new Intl.NumberFormat('en', {unit: 'furlong'})),
  thrown(() => new Intl.NumberFormat('en', {unit: 'meter-per-furlong'})),
  thrown(() => new Intl.NumberFormat('en', {style: 'x'})),
  thrown(() => new Intl.NumberFormat('en', {useGrouping: 'x'})),
  thrown(() => new Intl.NumberFormat('en', {notation: 'x'})),
);
// CHECK-NEXT: TypeError RangeError TypeError RangeError RangeError RangeError RangeError RangeError
// A currency or unit option is checked whatever the style.
print(
  new Intl.NumberFormat('en', {currency: 'eur', unit: 'meter-per-second'}).resolvedOptions()
    .currency,
);
// CHECK-NEXT: undefined

// A formatter parses each of its patterns once and keeps the parts
// (cachedPartitionPattern, cachedSplitPluralPattern). One formatter that
// uses several sign patterns in turn, and one that uses several unit and
// compact patterns in turn, give each value its own pattern, twice over.
var acc = new Intl.NumberFormat('en', {
  style: 'currency',
  currency: 'USD',
  currencySign: 'accounting',
  signDisplay: 'always',
});
var ruKm = new Intl.NumberFormat('ru', {
  style: 'unit',
  unit: 'kilometer',
  unitDisplay: 'long',
  notation: 'compact',
  compactDisplay: 'long',
});
for (var round = 0; round < 2; round++) {
  print([-1, 1, 0, NaN, -Infinity].map(v => acc.format(v)).join('|'));
  print([1, 2, 5, 1000, 2000, 5000000, 21].map(v => ruKm.format(v)).join('|'));
}
// CHECK-NEXT: ($1.00)|+$1.00|+$0.00|+$NaN|($∞)
// CHECK-NEXT: 1 километр|2 километра|5 километров|1 тысяча километров|2 тысячи километров|5 миллионов километров|21 километр
// CHECK-NEXT: ($1.00)|+$1.00|+$0.00|+$NaN|($∞)
// CHECK-NEXT: 1 километр|2 километра|5 километров|1 тысяча километров|2 тысячи километров|5 миллионов километров|21 километр
