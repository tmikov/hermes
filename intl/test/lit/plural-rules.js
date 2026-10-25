/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US %s | %FileCheck --match-full-lines %s

// Intl.PluralRules: the resolved options, cardinal and ordinal selection
// with the digit options, the notations (plan decision 13), selectRange,
// supportedLocalesOf and the constructor's errors. Where V8 has the same
// option, the expected categories are ICU4C's (Node 24, ICU 78.2); V8 has
// no notation option for PluralRules.

/// The class of the exception f throws, or 'none'.
function thrown(f) {
  try {
    f();
    return 'none';
  } catch (e) {
    return e.constructor.name;
  }
}

print(JSON.stringify(new Intl.PluralRules('en').resolvedOptions()));
// CHECK: {"locale":"en","type":"cardinal","notation":"standard","minimumIntegerDigits":1,"minimumFractionDigits":0,"maximumFractionDigits":3,"pluralCategories":["one","other"],"roundingIncrement":1,"roundingMode":"halfExpand","roundingPriority":"auto","trailingZeroDisplay":"auto"}
// The default locale; compactDisplay only with the compact notation; the
// significant digits replace the fraction digits. PluralRules has no
// relevant extension keys, so the -u- keywords are dropped.
print(JSON.stringify(new Intl.PluralRules().resolvedOptions().locale));
// CHECK-NEXT: "en-US"
print(
  JSON.stringify(
    new Intl.PluralRules('fr-u-nu-arab', {
      type: 'ordinal',
      notation: 'compact',
      compactDisplay: 'long',
      minimumSignificantDigits: 2,
    }).resolvedOptions(),
  ),
);
// CHECK-NEXT: {"locale":"fr","type":"ordinal","notation":"compact","compactDisplay":"long","minimumIntegerDigits":1,"minimumSignificantDigits":2,"maximumSignificantDigits":21,"pluralCategories":["one","other"],"roundingIncrement":1,"roundingMode":"halfExpand","roundingPriority":"auto","trailingZeroDisplay":"auto"}
print(JSON.stringify(new Intl.PluralRules('en', {compactDisplay: 'long'}).resolvedOptions().compactDisplay));
// CHECK-NEXT: undefined
// Each call returns a new array.
var en = new Intl.PluralRules('en');
print(en.resolvedOptions().pluralCategories === en.resolvedOptions().pluralCategories);
// CHECK-NEXT: false

// Cardinal selection rounds with the digit options first, and the
// operands keep the trailing zeros ("1.0" is v = 1).
print(en.select(1), en.select('1'), en.select(0), en.select(2), en.select(-1), en.select(1n));
// CHECK-NEXT: one one other other one one
print(new Intl.PluralRules('en', {minimumFractionDigits: 1}).select('1.0'));
// CHECK-NEXT: other
print(new Intl.PluralRules('en', {maximumFractionDigits: 0}).select(1.4));
// CHECK-NEXT: one
print(new Intl.PluralRules('en', {maximumSignificantDigits: 1}).select(1.04), en.select(1.0004));
// CHECK-NEXT: one one
print(en.select(NaN), en.select(Infinity), en.select(-Infinity), en.select(), en.select(-0));
// CHECK-NEXT: other other other other other
print(thrown(() => en.select(Symbol())), en.select(10n ** 400n), en.select(10n ** 400n + 1n));
// CHECK-NEXT: TypeError other other

// Ordinal selection.
var enOrdinal = new Intl.PluralRules('en', {type: 'ordinal'});
print(enOrdinal.select(1), enOrdinal.select(22), enOrdinal.select(23), enOrdinal.select(11), enOrdinal.select(4));
// CHECK-NEXT: one two few other other
print(JSON.stringify(enOrdinal.resolvedOptions().pluralCategories));
// CHECK-NEXT: ["one","two","few","other"]

// Every category, in the order zero, one, two, few, many, other.
var ar = new Intl.PluralRules('ar');
print(JSON.stringify(ar.resolvedOptions().pluralCategories));
// CHECK-NEXT: ["zero","one","two","few","many","other"]
print(ar.select(0), ar.select(1), ar.select(2), ar.select(3), ar.select(11), ar.select(100));
// CHECK-NEXT: zero one two few many other

// test262's fr notation table (intl402/PluralRules/prototype/select/
// notation.js), with decision 13's "one" for 1e-6 in scientific and
// engineering notation: CLDR's e operand is the exponent only when it is
// positive, so i = 0 selects "one".
var notations = ['standard', 'engineering', 'scientific', 'compact'];
for (var value of [1e6, 1.5e6, 1e-6]) {
  print(
    value,
    notations.map(n => new Intl.PluralRules('fr', {notation: n}).select(value)).join(' '),
  );
}
// CHECK-NEXT: 1000000 many many many many
// CHECK-NEXT: 1500000 other many many many
// CHECK-NEXT: 0.000001 one one one one

// More than 18 fraction digits keep their operands exactly: f = 1 is
// bs's "one".
var bs = new Intl.PluralRules('bs', {maximumFractionDigits: 20});
print(bs.select('0.00000000000000000001'), bs.select('0.00000000000000000002'), bs.select('0.00000000000000000005'));
// CHECK-NEXT: one few other
// A negative exponent adds nothing (c = 0), so the value's own digits
// select: bs 0.0001 is f = 1, "one", in every notation. Moving its point
// right by the engineering exponent -6 would give f = 100.
var bsValue = 0.0001;
print(notations.map(n => new Intl.PluralRules('bs', {notation: n, maximumFractionDigits: 4}).select(bsValue)).join(' '));
// CHECK-NEXT: one one one one

// The notation adds the exponent and keeps the value: en ordinal
// scientific 21 is "2.1" with exponent 1, still "one".
var enOrdinalScientific = new Intl.PluralRules('en', {type: 'ordinal', notation: 'scientific'});
print(enOrdinalScientific.select(21), enOrdinalScientific.select(22), enOrdinalScientific.select(0.21));
// CHECK-NEXT: one two other
// PluralRules rounds the whole value (10001 is ru's "one"), while
// NumberFormat selects for the number it displays (1E4, "many").
print(new Intl.PluralRules('ru', {notation: 'scientific'}).select(10001));
// CHECK-NEXT: one
print(
  new Intl.NumberFormat('ru', {
    notation: 'scientific',
    style: 'unit',
    unit: 'meter',
    unitDisplay: 'long',
  }).format(10001),
);
// CHECK-NEXT: 1E4 метров
// Engineering takes the exponent down to a multiple of 3; compact takes
// the locale's compact exponent (none below 1000 in fr, and the long
// table's for compactDisplay long), and rounds to two significant digits.
var frEngineering = new Intl.PluralRules('fr', {notation: 'engineering'});
print(frEngineering.select(1.5e5), frEngineering.select(2.5e6), new Intl.PluralRules('fr').select(2.5e6));
// CHECK-NEXT: other many other
var frCompact = new Intl.PluralRules('fr', {notation: 'compact'});
var frCompactLong = new Intl.PluralRules('fr', {notation: 'compact', compactDisplay: 'long'});
print(frCompact.select(999), frCompact.select(1000), frCompact.select(1234567), frCompactLong.select(1234567));
// CHECK-NEXT: other other many many
print(frCompact.select(1.4), frCompact.select(1.6), frCompact.select(0));
// CHECK-NEXT: one one one
print(new Intl.PluralRules('en', {notation: 'compact'}).select(1.04));
// CHECK-NEXT: one

// selectRange: the category of the end, through CLDR's plural ranges.
// en lists no one to one range, so 1 to 1 is the end's "one" either way;
// ICU4C gives "other".
print(en.selectRange(1, 5), en.selectRange(1, 1), en.selectRange(102, 201), en.selectRange(5, 1));
// CHECK-NEXT: other one other other
// The draft's ResolvePluralRange returns the start's category when both
// ends format the same (step 4). lv shows it: 10 and 0 are both "zero",
// and CLDR's lv ranges map zero to zero to "other", so 10 to 10 is "zero"
// (step 4) while 0 to 10 is "other" (the ranges).
var lv = new Intl.PluralRules('lv');
print(lv.select(10), lv.select(0), lv.selectRange(10, 10), lv.selectRange(0, 10));
// CHECK-NEXT: zero zero zero other
print(new Intl.PluralRules('fr').selectRange(1, 1.5), new Intl.PluralRules('fr').selectRange(0, 1));
// CHECK-NEXT: one one
// CLDR's fr ranges list no pair "other" to "one". UTS 35 Part 3, "Plural
// Ranges": "If there is no value for a <start,end> pair, the default
// result is end", so 2 to 1 is "one", as ICU4X gives; ICU4C gives "other".
print(new Intl.PluralRules('fr').selectRange(2, 1));
// CHECK-NEXT: one
// The ends round first: the same rounded string gives the start's
// category.
print(new Intl.PluralRules('en', {maximumFractionDigits: 0}).selectRange(0.9, 1.4));
// CHECK-NEXT: one
print(en.selectRange(Infinity, Infinity), en.selectRange(1, Infinity), en.selectRange(-Infinity, 1));
// CHECK-NEXT: other other other
print(thrown(() => en.selectRange(NaN, 1)), thrown(() => en.selectRange(1, NaN)));
// CHECK-NEXT: RangeError RangeError
print(thrown(() => en.selectRange(1)), thrown(() => en.selectRange(undefined, 1)), thrown(() => en.selectRange(1, Symbol())));
// CHECK-NEXT: TypeError TypeError TypeError

// supportedLocalesOf and the constructor's errors.
print(JSON.stringify(Intl.PluralRules.supportedLocalesOf(['de', 'xx', 'th-TH'])));
// CHECK-NEXT: ["de","th-TH"]
print(thrown(() => Intl.PluralRules()), thrown(() => Intl.PluralRules('en')));
// CHECK-NEXT: TypeError TypeError
print(
  thrown(() => new Intl.PluralRules('en', {type: 'x'})),
  thrown(() => new Intl.PluralRules('en', {notation: 'x'})),
  thrown(() => new Intl.PluralRules('en', {compactDisplay: 'x'})),
  thrown(() => new Intl.PluralRules('en', {maximumFractionDigits: 101})),
  thrown(() => new Intl.PluralRules('en', {roundingIncrement: 3})),
  thrown(() => new Intl.PluralRules('', {})),
  thrown(() => new Intl.PluralRules('en', null)),
);
// CHECK-NEXT: RangeError RangeError RangeError RangeError RangeError RangeError TypeError
print(Object.prototype.toString.call(en));
// CHECK-NEXT: [object Intl.PluralRules]
