/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: cat %S/Inputs/escape.js %s > %t.js
// RUN: %hermes -Xintl-default-locale=en-US %t.js | %FileCheck --match-full-lines %s

// Intl.NumberFormat with the unit style: the three widths, compound units,
// the plural forms of the displayed number, the parts of a unit pattern
// (plan decision 10), patterns without a number (decision 10), and the
// option errors. The expected strings and parts are ICU4C's (Node 24, ICU
// 78.2) except where a comment says otherwise.

/// The unit formatter of locale for unit at width display, with the other
/// options of extra.
function unitFormat(locale, unit, display, extra) {
  return new Intl.NumberFormat(
    locale,
    Object.assign({style: 'unit', unit: unit, unitDisplay: display}, extra),
  );
}

/// nf.format(x), escaped, then the parts of nf.formatToParts(x) as
/// type:value separated by "|", escaped. It also checks that the parts'
/// values concatenate to the format result.
function show(nf, x) {
  var s = nf.format(x);
  var p = nf.formatToParts(x);
  var joined = p.map(q => q.value).join('');
  print(
    escape(s),
    escape(p.map(q => q.type + ':' + q.value).join('|')),
    joined === s ? 'ok' : 'MISMATCH ' + escape(joined),
  );
}

// A dedicated compound unit at the three widths; the unit pattern wraps the
// signed number (decision 11).
show(unitFormat('en', 'kilometer-per-hour', 'short'), -987);
// CHECK: -987 km/h minusSign:-|integer:987|literal: |unit:km/h ok
show(unitFormat('en', 'kilometer-per-hour', 'narrow'), -987);
// CHECK-NEXT: -987km/h minusSign:-|integer:987|unit:km/h ok
show(unitFormat('en', 'kilometer-per-hour', 'long'), -987);
// CHECK-NEXT: -987 kilometers per hour minusSign:-|integer:987|literal: |unit:kilometers per hour ok
// short is the default width.
show(new Intl.NumberFormat('en', {style: 'unit', unit: 'kilometer-per-hour'}), 5);
// CHECK-NEXT: 5 km/h integer:5|literal: |unit:km/h ok

// Text on both sides of the number: the whitespace next to it is literal,
// the rest of each side one unit part (decision 10).
show(unitFormat('ja', 'kilometer-per-hour', 'long'), -987);
// CHECK-NEXT: 時速 -987 キロメートル unit:時速|literal: |minusSign:-|integer:987|literal: |unit:キロメートル ok
show(unitFormat('ko', 'kilometer-per-hour', 'long'), 1);
// CHECK-NEXT: 시속 1킬로미터 unit:시속|literal: |integer:1|unit:킬로미터 ok
show(unitFormat('zh-TW', 'kilometer-per-hour', 'long'), -1);
// CHECK-NEXT: 每小時 -1 公里 unit:每小時|literal: |minusSign:-|integer:1|literal: |unit:公里 ok
// U+202F next to the number is whitespace too.
show(unitFormat('fr', 'kilometer-per-hour', 'short'), 1234.5);
// CHECK-NEXT: 1\u202f234,5\u202fkm/h integer:1|group:\u202f|integer:234|decimal:,|fraction:5|literal:\u202f|unit:km/h ok

// The plural form is the displayed number's: "1.0" is "other" in en, and a
// negative number has the form of its absolute value.
show(unitFormat('en', 'mile', 'long'), 1);
// CHECK-NEXT: 1 mile integer:1|literal: |unit:mile ok
show(unitFormat('en', 'mile', 'long'), 1.5);
// CHECK-NEXT: 1.5 miles integer:1|decimal:.|fraction:5|literal: |unit:miles ok
show(unitFormat('en', 'mile', 'long', {minimumFractionDigits: 1}), 1);
// CHECK-NEXT: 1.0 miles integer:1|decimal:.|fraction:0|literal: |unit:miles ok
show(unitFormat('en', 'mile', 'long'), -1);
// CHECK-NEXT: -1 mile minusSign:-|integer:1|literal: |unit:mile ok
show(unitFormat('en', 'mile', 'long', {maximumFractionDigits: 0}), 1.4);
// CHECK-NEXT: 1 mile integer:1|literal: |unit:mile ok
show(unitFormat('en', 'mile', 'long', {minimumIntegerDigits: 3}), 1);
// CHECK-NEXT: 001 mile integer:001|literal: |unit:mile ok
// NaN and infinity take "other".
show(unitFormat('en', 'mile', 'long'), NaN);
// CHECK-NEXT: NaN miles nan:NaN|literal: |unit:miles ok
show(unitFormat('en', 'mile', 'long'), Infinity);
// CHECK-NEXT: ∞ miles infinity:∞|literal: |unit:miles ok

// ru has three integer forms and one for fractions.
var ruKilometers = unitFormat('ru', 'kilometer', 'long');
print([1, 2, 5, 21, 1.5].map(x => ruKilometers.format(x)).join(' | '));
// CHECK-NEXT: 1 километр | 2 километра | 5 километров | 21 километр | 1,5 километра

// Units built from the per-unit pattern and the compound pattern
// (decision 14), and units ICU4X has no names for.
show(unitFormat('en', 'byte-per-second', 'short'), 5);
// CHECK-NEXT: 5 byte/s integer:5|literal: |unit:byte/s ok
show(unitFormat('en', 'byte-per-second', 'long'), 1);
// CHECK-NEXT: 1 byte per second integer:1|literal: |unit:byte per second ok
show(unitFormat('en', 'kilogram-per-liter', 'long'), 1);
// CHECK-NEXT: 1 kilogram per liter integer:1|literal: |unit:kilogram per liter ok
show(unitFormat('en', 'gigabyte-per-hour', 'short'), 2);
// CHECK-NEXT: 2 GB/h integer:2|literal: |unit:GB/h ok
show(unitFormat('en', 'mile-per-gallon', 'long'), 1);
// CHECK-NEXT: 1 mile per gallon integer:1|literal: |unit:mile per gallon ok
show(unitFormat('en', 'celsius', 'short'), -3);
// CHECK-NEXT: -3°C minusSign:-|integer:3|unit:°C ok
show(unitFormat('en', 'celsius', 'long'), -3);
// CHECK-NEXT: -3 degrees Celsius minusSign:-|integer:3|literal: |unit:degrees Celsius ok

// The percent unit is a unit: the number is not scaled, and "%" is a unit
// part. Its short and narrow forms use the locale's percent patterns, with
// their sign placement, spacing, bidi marks and grouping sizes; its long
// form uses CLDR's unit pattern, as ICU4C does.
show(unitFormat('en', 'percent', 'short'), 50);
// CHECK-NEXT: 50% integer:50|unit:% ok
show(unitFormat('en', 'percent', 'long'), 50);
// CHECK-NEXT: 50 percent integer:50|literal: |unit:percent ok
show(unitFormat('en', 'percent', 'short'), -1);
// CHECK-NEXT: -1% minusSign:-|integer:1|unit:% ok
show(unitFormat('en', 'percent', 'long'), -1);
// CHECK-NEXT: -1 percent minusSign:-|integer:1|literal: |unit:percent ok
show(unitFormat('en', 'percent', 'short', {signDisplay: 'always'}), 5);
// CHECK-NEXT: +5% plusSign:+|integer:5|unit:% ok
show(unitFormat('en', 'percent', 'short'), NaN);
// CHECK-NEXT: NaN% nan:NaN|unit:% ok
show(unitFormat('de', 'percent', 'short'), 1);
// CHECK-NEXT: 1\u00a0% integer:1|literal:\u00a0|unit:% ok
show(unitFormat('de', 'percent', 'short'), -1);
// CHECK-NEXT: -1\u00a0% minusSign:-|integer:1|literal:\u00a0|unit:% ok
show(unitFormat('de', 'percent', 'narrow'), -1);
// CHECK-NEXT: -1\u00a0% minusSign:-|integer:1|literal:\u00a0|unit:% ok
show(unitFormat('de', 'percent', 'long'), -1);
// CHECK-NEXT: -1 Prozent minusSign:-|integer:1|literal: |unit:Prozent ok
show(unitFormat('tr', 'percent', 'short'), -1);
// CHECK-NEXT: -%1 minusSign:-|unit:%|integer:1 ok
show(unitFormat('tr', 'percent', 'short'), 0);
// CHECK-NEXT: %0 unit:%|integer:0 ok
show(unitFormat('tr', 'percent', 'long'), -1);
// CHECK-NEXT: yüzde -1 unit:yüzde|literal: |minusSign:-|integer:1 ok
show(unitFormat('ar', 'percent', 'short'), -1);
// CHECK-NEXT: \u200e-1\u200e%\u200e literal:\u200e|minusSign:-|integer:1|literal:\u200e|unit:%|literal:\u200e ok
show(unitFormat('ar-EG', 'percent', 'narrow'), -1);
// CHECK-NEXT: \u061c-١٪\u061c literal:\u061c|minusSign:-|integer:١|unit:٪|literal:\u061c ok
// bn groups percents by threes and decimals three then two.
show(unitFormat('bn', 'percent', 'short'), 1234567);
// CHECK-NEXT: ১,২৩৪,৫৬৭% integer:১|group:,|integer:২৩৪|group:,|integer:৫৬৭|unit:% ok
show(unitFormat('bn', 'percent', 'long'), 1234567);
// CHECK-NEXT: ১২,৩৪,৫৬৭শতাংশ integer:১২|group:,|integer:৩৪|group:,|integer:৫৬৭|unit:শতাংশ ok

// Bidi marks next to the number (U+200E, U+200F, U+061C), alone or with
// whitespace, are literal parts like whitespace; a literal of the unit
// pattern next to one of the sign pattern is one part, as in ICU4C.
show(unitFormat('he', 'gigabyte', 'short'), -1);
// CHECK-NEXT: GB\u200f\u200e-1 unit:GB|literal:\u200f\u200e|minusSign:-|integer:1 ok
show(unitFormat('he', 'byte-per-second', 'narrow'), 1);
// CHECK-NEXT: B\u200f1/שנ׳ unit:B|literal:\u200f|integer:1|unit:/שנ׳ ok
show(unitFormat('fa', 'kilometer-per-hour', 'short'), 1);
// CHECK-NEXT: ۱\u200e km/h integer:۱|literal:\u200e |unit:km/h ok
show(unitFormat('fa', 'fluid-ounce', 'short'), -2);
// CHECK-NEXT: \u200e−۲\u200e fl oz literal:\u200e|minusSign:−|integer:۲|literal:\u200e |unit:fl oz ok

// de long liter is the same in the singular and the plural.
show(unitFormat('de', 'liter', 'long'), 2);
// CHECK-NEXT: 2 Liter integer:2|literal: |unit:Liter ok

// ar long meter "one" is "متر", without a number (decision 10): it is used
// as one unit part when no sign is shown, as ICU4C does. When a sign is
// shown, the plugin takes the "other" pattern and keeps the sign, where
// ICU4C drops it ("متر" for -1 and for +1).
show(unitFormat('ar', 'meter', 'long'), 1);
// CHECK-NEXT: متر unit:متر ok
show(unitFormat('ar', 'meter', 'long', {signDisplay: 'never'}), -1);
// CHECK-NEXT: متر unit:متر ok
show(unitFormat('ar', 'meter', 'long'), -1);
// CHECK-NEXT: \u200e-1 متر literal:\u200e|minusSign:-|integer:1|literal: |unit:متر ok
show(unitFormat('ar', 'meter', 'long', {signDisplay: 'always'}), 1);
// CHECK-NEXT: \u200e+1 متر literal:\u200e|plusSign:+|integer:1|literal: |unit:متر ok
show(unitFormat('ar', 'meter', 'long'), 0);
// CHECK-NEXT: 0 متر integer:0|literal: |unit:متر ok
show(unitFormat('ar-EG', 'meter', 'long'), -2);
// CHECK-NEXT: \u061c-٢ متر literal:\u061c|minusSign:-|integer:٢|literal: |unit:متر ok

// style "unit" needs a unit, and the unit must be sanctioned.
try {
  new Intl.NumberFormat('en', {style: 'unit'});
} catch (e) {
  print(e.name);
}
// CHECK-NEXT: TypeError
try {
  new Intl.NumberFormat('en', {style: 'unit', unit: 'furlong'});
} catch (e) {
  print(e.name);
}
// CHECK-NEXT: RangeError
try {
  new Intl.NumberFormat('en', {style: 'unit', unit: 'meter-per-furlong'});
} catch (e) {
  print(e.name);
}
// CHECK-NEXT: RangeError

// Every sanctioned unit formats at every width in en: the output contains
// the formatted number and differs from the plain number's format
// (test262's units.js check), its parts concatenate to it, and it has a
// unit part with text other than whitespace.
var units = Intl.supportedValuesOf('unit');
var widths = ['long', 'short', 'narrow'];
var plain = new Intl.NumberFormat('en').format(1234.5);
var bad = [];
for (var u = 0; u < units.length; u++) {
  for (var w = 0; w < widths.length; w++) {
    var nf = unitFormat('en', units[u], widths[w]);
    var s = nf.format(1234.5);
    var p = nf.formatToParts(1234.5);
    var hasUnit = p.some(q => q.type === 'unit' && q.value.trim() !== '');
    var joined = p.map(q => q.value).join('');
    if (s.indexOf(plain) === -1 || s === plain || !hasUnit || joined !== s)
      bad.push(units[u] + ' ' + widths[w]);
  }
}
print(units.length, 'units', bad.length === 0 ? 'ok' : bad.join());
// CHECK-NEXT: 45 units ok
