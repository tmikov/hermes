/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: cat %S/Inputs/escape.js %s > %t.js
// RUN: %hermes -Xintl-default-locale=en-US %t.js | %FileCheck --match-full-lines %s

// Intl.NumberFormat with the scientific, engineering and compact notations
// (plan decision 12): the exponent parts and their numbering system, the
// compact entries of ICU4X's data and their plural forms, compact patterns
// without a number (decision 10), and the plural forms of unit and currency
// names with a notation's exponent (decision 13). The expected strings and
// parts are ICU4C's (Node 24, ICU 78.2) except where a comment says
// otherwise.

/// A formatter of locale with the options opts.
function nf(locale, opts) {
  return new Intl.NumberFormat(locale, opts);
}

/// The options of notation, with the other options of extra.
function notation(n, extra) {
  return Object.assign({notation: n}, extra);
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

var sci = nf('en', notation('scientific'));
var eng = nf('en', notation('engineering'));
var short = nf('en', notation('compact'));
var long = nf('en', notation('compact', {compactDisplay: 'long'}));
var frLong = nf('fr', notation('compact', {compactDisplay: 'long'}));

// Scientific: the sub-pattern "{number}{scientificSeparator}
// {scientificExponent}" with the locale's exponent symbol.
show(sci, 123456);
// CHECK: 1.235E5 integer:1|decimal:.|fraction:235|exponentSeparator:E|exponentInteger:5 ok
show(sci, 0.000123);
// CHECK-NEXT: 1.23E-4 integer:1|decimal:.|fraction:23|exponentSeparator:E|exponentMinusSign:-|exponentInteger:4 ok
show(sci, -123456);
// CHECK-NEXT: -1.235E5 minusSign:-|integer:1|decimal:.|fraction:235|exponentSeparator:E|exponentInteger:5 ok
show(sci, 0);
// CHECK-NEXT: 0E0 integer:0|exponentSeparator:E|exponentInteger:0 ok
show(sci, -0);
// CHECK-NEXT: -0E0 minusSign:-|integer:0|exponentSeparator:E|exponentInteger:0 ok
show(sci, NaN);
// CHECK-NEXT: NaN nan:NaN ok
show(sci, -Infinity);
// CHECK-NEXT: -∞ minusSign:-|infinity:∞ ok
// Rounding that reaches the next power of ten takes the next exponent.
show(nf('en', notation('scientific', {maximumFractionDigits: 0})), 9.6);
// CHECK-NEXT: 1E1 integer:1|exponentSeparator:E|exponentInteger:1 ok
show(sci, 1e21);
// CHECK-NEXT: 1E21 integer:1|exponentSeparator:E|exponentInteger:21 ok
show(sci, 5e-324);
// CHECK-NEXT: 5E-324 integer:5|exponentSeparator:E|exponentMinusSign:-|exponentInteger:324 ok
// Engineering: exponents that are multiples of 3.
show(eng, 123456);
// CHECK-NEXT: 123.456E3 integer:123|decimal:.|fraction:456|exponentSeparator:E|exponentInteger:3 ok
show(eng, 0.000345);
// CHECK-NEXT: 345E-6 integer:345|exponentSeparator:E|exponentMinusSign:-|exponentInteger:6 ok
show(eng, -0.0345);
// CHECK-NEXT: -34.5E-3 minusSign:-|integer:34|decimal:.|fraction:5|exponentSeparator:E|exponentMinusSign:-|exponentInteger:3 ok

// The exponent's digits are the numbering system's (decision 12), and its
// minus sign has the bidi marks of the locale's minus sign.
show(nf('ar-EG', notation('scientific')), -123456);
// CHECK-NEXT: \u061c-١٫٢٣٥أس٥ literal:\u061c|minusSign:-|integer:١|decimal:٫|fraction:٢٣٥|exponentSeparator:أس|exponentInteger:٥ ok
show(nf('ar-EG', notation('scientific')), 0.000123);
// CHECK-NEXT: ١٫٢٣أس\u061c-٤ integer:١|decimal:٫|fraction:٢٣|exponentSeparator:أس|literal:\u061c|exponentMinusSign:-|exponentInteger:٤ ok
show(nf('fa', notation('scientific')), -0.0012);
// CHECK-NEXT: \u200e−۱٫۲×۱۰^\u200e−۳ literal:\u200e|minusSign:−|integer:۱|decimal:٫|fraction:۲|exponentSeparator:×۱۰^|literal:\u200e|exponentMinusSign:−|exponentInteger:۳ ok
show(nf('ps', notation('scientific')), -0.0012);
// CHECK-NEXT: \u200e-\u200e۱٫۲×۱۰^\u200e-\u200e۳ literal:\u200e|minusSign:-|literal:\u200e|integer:۱|decimal:٫|fraction:۲|exponentSeparator:×۱۰^|literal:\u200e|exponentMinusSign:-|literal:\u200e|exponentInteger:۳ ok
show(nf('ar', notation('scientific', {signDisplay: 'always'})), 0.0012);
// CHECK-NEXT: \u200e+1.2E\u200e-3 literal:\u200e|plusSign:+|integer:1|decimal:.|fraction:2|exponentSeparator:E|literal:\u200e|exponentMinusSign:-|exponentInteger:3 ok
show(nf('en-u-nu-mathbold', notation('scientific')), 0.00001);
// CHECK-NEXT: 𝟏E-𝟓 integer:𝟏|exponentSeparator:E|exponentMinusSign:-|exponentInteger:𝟓 ok
show(nf('hi-u-nu-deva', notation('engineering')), 1.5e-7);
// CHECK-NEXT: १५०E-९ integer:१५०|exponentSeparator:E|exponentMinusSign:-|exponentInteger:९ ok
show(nf('de', notation('scientific')), 0.00123);
// CHECK-NEXT: 1,23E-3 integer:1|decimal:,|fraction:23|exponentSeparator:E|exponentMinusSign:-|exponentInteger:3 ok

// The other styles put the scientific number in their patterns, and a
// unit's plural form is that of the displayed significand with the
// exponent (decision 13): ru 1E4 is "many".
show(nf('en', notation('scientific', {style: 'percent'})), 12.34);
// CHECK-NEXT: 1E3% integer:1|exponentSeparator:E|exponentInteger:3|percentSign:% ok
show(nf('en', notation('scientific', {style: 'currency', currency: 'USD'})), 1234);
// CHECK-NEXT: $1.234E3 currency:$|integer:1|decimal:.|fraction:234|exponentSeparator:E|exponentInteger:3 ok
show(nf('en', notation('scientific', {style: 'unit', unit: 'meter', unitDisplay: 'long'})), 1234);
// CHECK-NEXT: 1.234E3 meters integer:1|decimal:.|fraction:234|exponentSeparator:E|exponentInteger:3|literal: |unit:meters ok
show(nf('ru', notation('scientific', {style: 'unit', unit: 'meter', unitDisplay: 'long'})), 10001);
// CHECK-NEXT: 1E4 метров integer:1|exponentSeparator:E|exponentInteger:4|literal: |unit:метров ok

// Compact short: the default rounding keeps two significant digits for
// numbers below 100 and rounds half away from zero (1650 is 1.7K, where
// ICU4X's own formatter would round half to even).
show(short, 1234);
// CHECK-NEXT: 1.2K integer:1|decimal:.|fraction:2|compact:K ok
show(short, 999999);
// CHECK-NEXT: 1M integer:1|compact:M ok
show(short, 1650);
// CHECK-NEXT: 1.7K integer:1|decimal:.|fraction:7|compact:K ok
show(short, -1234);
// CHECK-NEXT: -1.2K minusSign:-|integer:1|decimal:.|fraction:2|compact:K ok
show(short, 999.9);
// CHECK-NEXT: 1K integer:1|compact:K ok
show(short, 0);
// CHECK-NEXT: 0 integer:0 ok
show(short, 0.0001234);
// CHECK-NEXT: 0.00012 integer:0|decimal:.|fraction:00012 ok
// Beyond the last entry, the last entry's exponent (decision 12).
show(short, 1e16);
// CHECK-NEXT: 10,000T integer:10|group:,|integer:000|compact:T ok
show(short, 1e21);
// CHECK-NEXT: 1,000,000,000T integer:1|group:,|integer:000|group:,|integer:000|group:,|integer:000|compact:T ok
show(long, 1234);
// CHECK-NEXT: 1.2 thousand integer:1|decimal:.|fraction:2|literal: |compact:thousand ok
show(long, 1e6);
// CHECK-NEXT: 1 million integer:1|literal: |compact:million ok

// Magnitudes below the first entry are not compact: ja's first entry
// starts at 10^4, de short's at 10^6. Compact grouping is "min2".
show(nf('ja', notation('compact')), 9876);
// CHECK-NEXT: 9876 integer:9876 ok
show(nf('ja', notation('compact')), 98765);
// CHECK-NEXT: 9.9万 integer:9|decimal:.|fraction:9|compact:万 ok
show(nf('ko', notation('compact')), 12345);
// CHECK-NEXT: 1.2만 integer:1|decimal:.|fraction:2|compact:만 ok
show(nf('zh-TW', notation('compact', {compactDisplay: 'long'})), 123456789);
// CHECK-NEXT: 1.2億 integer:1|decimal:.|fraction:2|compact:億 ok
show(nf('de', notation('compact')), 9876);
// CHECK-NEXT: 9876 integer:9876 ok
show(nf('de', notation('compact')), 98765);
// CHECK-NEXT: 98.765 integer:98|group:.|integer:765 ok
show(nf('de', notation('compact')), 987654321);
// CHECK-NEXT: 988\u00a0Mio. integer:988|literal:\u00a0|compact:Mio. ok
show(nf('de', notation('compact', {compactDisplay: 'long'})), 1000);
// CHECK-NEXT: 1 Tausend integer:1|literal: |compact:Tausend ok
show(nf('de', notation('compact', {compactDisplay: 'long'})), 2000000);
// CHECK-NEXT: 2 Millionen integer:2|literal: |compact:Millionen ok
// Text inside a compact pattern is one compact part.
show(nf('es', notation('compact')), 1234567890);
// CHECK-NEXT: 1235\u00a0M integer:1235|literal:\u00a0|compact:M ok
show(nf('es', notation('compact')), 12345678901);
// CHECK-NEXT: 12\u00a0mil\u00a0M integer:12|literal:\u00a0|compact:mil\u00a0M ok
// The bidi marks at the ends of a compact pattern's text are literal parts.
show(nf('he', notation('compact')), -1234);
// CHECK-NEXT: \u200e-1.2K\u200f literal:\u200e|minusSign:-|integer:1|decimal:.|fraction:2|compact:K|literal:\u200f ok

// The pattern comes from the entry of the displayed number's magnitude:
// ar's 10^3 entry has a "few" form that its 10^4 entry lacks, though both
// have exponent 3. 9999 rounds to 10 thousand, which is in the 10^4 entry.
show(nf('ar', notation('compact')), 3000);
// CHECK-NEXT: 3\u00a0آلاف integer:3|literal:\u00a0|compact:آلاف ok
show(nf('ar', notation('compact')), 30000);
// CHECK-NEXT: 30\u00a0ألف integer:30|literal:\u00a0|compact:ألف ok
show(nf('ar', notation('compact')), 2999);
// CHECK-NEXT: 3\u00a0آلاف integer:3|literal:\u00a0|compact:آلاف ok
show(nf('ar', notation('compact')), 9999);
// CHECK-NEXT: 10\u00a0ألف integer:10|literal:\u00a0|compact:ألف ok
show(nf('ar', notation('compact', {compactDisplay: 'long'})), 3000000);
// CHECK-NEXT: 3 ملايين integer:3|literal: |compact:ملايين ok
show(nf('ar', notation('compact', {compactDisplay: 'long'})), 30000000);
// CHECK-NEXT: 30 مليون integer:30|literal: |compact:مليون ok
show(nf('ar-EG', notation('compact')), -12345);
// CHECK-NEXT: \u061c-١٢\u00a0ألف literal:\u061c|minusSign:-|integer:١٢|literal:\u00a0|compact:ألف ok

// Compact with the currency style: CLDR's short currency patterns
// (number-format-compact-currency.js has more).
show(nf('en', notation('compact', {style: 'currency', currency: 'USD'})), 1234);
// CHECK-NEXT: $1.2K currency:$|integer:1|decimal:.|fraction:2|compact:K ok
// The plugin differs here: ICU4C ignores currencySign "accounting" in
// compact notation ("-$1.2K"); the plugin follows the draft's
// GetNumberFormatPattern, which takes [[CurrencySign]].
show(nf('en', notation('compact', {style: 'currency', currency: 'USD', currencySign: 'accounting'})), -1234);
// CHECK-NEXT: ($1.2K) literal:(|currency:$|integer:1|decimal:.|fraction:2|compact:K|literal:) ok
show(nf('de', notation('compact', {style: 'currency', currency: 'EUR'})), 1234567);
// CHECK-NEXT: 1,2\u00a0Mio.\u00a0€ integer:1|decimal:,|fraction:2|literal:\u00a0|compact:Mio.|literal:\u00a0|currency:€ ok
show(nf('de', notation('compact', {style: 'currency', currency: 'EUR'})), 1234);
// CHECK-NEXT: 1234\u00a0€ integer:1234|literal:\u00a0|currency:€ ok
show(nf('en', notation('compact', {compactDisplay: 'long', style: 'currency', currency: 'USD', currencyDisplay: 'name'})), 1000);
// CHECK-NEXT: 1 thousand US dollars integer:1|literal: |compact:thousand|literal: |currency:US dollars ok
// The code, symbol and narrowSymbol displays use the short currency
// patterns whatever compactDisplay is, as ICU4C does: CLDR has compact
// currency patterns only in short form. The name display above keeps the
// long ones, and resolvedOptions still reports "long".
var usdLong = nf('en', notation('compact', {compactDisplay: 'long', style: 'currency', currency: 'USD'}));
show(usdLong, 1234);
// CHECK-NEXT: $1.2K currency:$|integer:1|decimal:.|fraction:2|compact:K ok
show(nf('en', notation('compact', {compactDisplay: 'long', style: 'currency', currency: 'USD', currencyDisplay: 'code'})), 1234);
// CHECK-NEXT: USD\u00a01.2K currency:USD|literal:\u00a0|integer:1|decimal:.|fraction:2|compact:K ok
show(nf('de', notation('compact', {compactDisplay: 'long', style: 'currency', currency: 'EUR'})), 1234567);
// CHECK-NEXT: 1,2\u00a0Mio.\u00a0€ integer:1|decimal:,|fraction:2|literal:\u00a0|compact:Mio.|literal:\u00a0|currency:€ ok
print(usdLong.resolvedOptions().compactDisplay);
// CHECK-NEXT: long

// Compact plural forms: the displayed number alone selects, with exponent
// 0 (decision 12), so ru long 1000 is "one"; a unit's form is selected for
// the whole displayed number (decision 13), "many" for 1 thousand.
show(nf('ru', notation('compact', {compactDisplay: 'long'})), 1000);
// CHECK-NEXT: 1 тысяча integer:1|literal: |compact:тысяча ok
show(nf('ru', notation('compact', {compactDisplay: 'long'})), 5000);
// CHECK-NEXT: 5 тысяч integer:5|literal: |compact:тысяч ok
show(nf('ru', notation('compact', {compactDisplay: 'long', style: 'unit', unit: 'kilometer', unitDisplay: 'long'})), 1000);
// CHECK-NEXT: 1 тысяча километров integer:1|literal: |compact:тысяча|literal: |unit:километров ok

// fr long has the exact-1 pattern "mille", which has no number: one
// compact part, used only when no sign is shown (decision 10).
show(frLong, 1000);
// CHECK-NEXT: mille compact:mille ok
show(frLong, 1000.4);
// CHECK-NEXT: mille compact:mille ok
show(frLong, 2000);
// CHECK-NEXT: 2 mille integer:2|literal: |compact:mille ok
show(frLong, -1000);
// CHECK-NEXT: -1 millier minusSign:-|integer:1|literal: |compact:millier ok
show(frLong, 1e6);
// CHECK-NEXT: 1 million integer:1|literal: |compact:million ok
// The plugin differs here: ICU4C gives "+mille"; decision 10 uses a
// pattern without a number only when no sign is shown, so "always" takes
// "one".
show(nf('fr', notation('compact', {compactDisplay: 'long', signDisplay: 'always'})), 1000);
// CHECK-NEXT: +1 millier plusSign:+|integer:1|literal: |compact:millier ok
// The plugin differs here: ICU4C gives "1 millier"; with signDisplay
// "never" the displayed number is exactly 1 and no sign is shown, so the
// exact-1 pattern applies (decisions 10 and 12).
show(nf('fr', notation('compact', {compactDisplay: 'long', signDisplay: 'never'})), -1000);
// CHECK-NEXT: mille compact:mille ok
// The plugin differs here: ICU4C gives "mille"; "1,0" has a fraction
// digit, so it is not exactly 1 (decision 12) and takes its category
// "one".
show(nf('fr', notation('compact', {compactDisplay: 'long', minimumFractionDigits: 1})), 1000);
// CHECK-NEXT: 1,0 millier integer:1|decimal:,|fraction:0|literal: |compact:millier ok

// it long's "one" pattern is "mille", without a number, and "one" is only
// exactly 1 in it. With a sign the "other" pattern is used (decision 10);
// the plugin differs here: ICU4C gives "-mille".
var itLong = nf('it', notation('compact', {compactDisplay: 'long'}));
show(itLong, 1000);
// CHECK-NEXT: mille compact:mille ok
show(itLong, -1000);
// CHECK-NEXT: -1 mila minusSign:-|integer:1|literal: |compact:mila ok
show(itLong, 2000);
// CHECK-NEXT: 2 mila integer:2|literal: |compact:mila ok

// A significand that rounds to zero keeps its exponent (ComputeExponent
// step 7) and takes the first entry with that exponent, the one of its
// magnitude before rounding: ak long's 10^3 entry has "apem", its 10^4
// entry "mpem".
var toZero = {roundingIncrement: 5000, maximumFractionDigits: 0, roundingMode: 'trunc'};
show(nf('en', notation('compact', toZero)), 1234);
// CHECK-NEXT: 0K integer:0|compact:K ok
show(nf('en', notation('compact', toZero)), -1234);
// CHECK-NEXT: -0K minusSign:-|integer:0|compact:K ok
show(nf('fr', notation('compact', Object.assign({compactDisplay: 'long'}, toZero))), 1234);
// CHECK-NEXT: 0 millier integer:0|literal: |compact:millier ok
show(nf('ar', notation('compact', toZero)), 1234);
// CHECK-NEXT: 0\u00a0ألف integer:0|literal:\u00a0|compact:ألف ok
show(nf('ak', notation('compact', Object.assign({compactDisplay: 'long'}, toZero))), 1234);
// CHECK-NEXT: apem 0 compact:apem|literal: |integer:0 ok

// The resolved options of the notations: compact defaults useGrouping to
// "min2", the others to "auto".
function opts(o) {
  var r = new Intl.NumberFormat('en', o).resolvedOptions();
  print(
    [
      r.notation,
      r.compactDisplay,
      r.useGrouping,
      r.minimumFractionDigits,
      r.maximumFractionDigits,
      r.maximumSignificantDigits,
      r.roundingPriority,
    ].join(' '),
  );
}
opts({notation: 'compact'});
// CHECK-NEXT: compact short min2 0 0 2 morePrecision
opts({notation: 'compact', compactDisplay: 'long', useGrouping: 'always'});
// CHECK-NEXT: compact long always 0 0 2 morePrecision
opts({notation: 'scientific'});
// CHECK-NEXT: scientific  auto 0 3  auto
opts({notation: 'engineering'});
// CHECK-NEXT: engineering  auto 0 3  auto
