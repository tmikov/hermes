/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: cat %S/Inputs/escape.js %s > %t.js
// RUN: %hermes -Xintl-default-locale=en-US %t.js | %FileCheck --match-full-lines %s

// Intl.NumberFormat.prototype.formatRange and formatRangeToParts (plan
// decision 15): the approximately sign and where it goes, the range
// separator of the locale's range pattern, and when the two sides collapse
// into one. The expected strings and parts are ICU4C's (Node 24, ICU 78.2)
// except where a comment says otherwise.

/// A formatter of locale with the options opts.
function nf(locale, opts) {
  return new Intl.NumberFormat(locale, opts);
}

/// The short name of a part's source: "st" for "startRange", "en" for
/// "endRange", "sh" for "shared".
var SOURCE = {startRange: 'st', endRange: 'en', shared: 'sh'};

/// f.formatRange(x, y), escaped, then the parts of f.formatRangeToParts(x,
/// y) as type:value:source separated by "|", escaped, with the short
/// sources. It also checks that the parts' values concatenate to the
/// formatRange result.
function show(f, x, y) {
  var s = f.formatRange(x, y);
  var p = f.formatRangeToParts(x, y);
  var joined = p.map(q => q.value).join('');
  print(
    escape(s),
    escape(p.map(q => q.type + ':' + q.value + ':' + SOURCE[q.source]).join('|')),
    joined === s ? 'ok' : 'MISMATCH ' + escape(joined),
  );
}

/// The name of the error f throws, or "none".
function thrown(f) {
  try {
    f();
    return 'none';
  } catch (e) {
    return e.name;
  }
}

// test262's en-US and pt-PT cases. Prefixes of one code point ("$") do
// not collapse; "+$" and "\u00a0€" do.
var usd0 = nf('en-US', {style: 'currency', currency: 'USD', maximumFractionDigits: 0});
show(usd0, 3, 5);
// CHECK: $3 – $5 currency:$:st|integer:3:st|literal: – :sh|currency:$:en|integer:5:en ok
show(usd0, 2.9, 3.1);
// CHECK-NEXT: ~$3 approximatelySign:~:sh|currency:$:sh|integer:3:sh ok
show(usd0, 1, 1);
// CHECK-NEXT: ~$1 approximatelySign:~:sh|currency:$:sh|integer:1:sh ok
show(usd0, 2.999, 3.001);
// CHECK-NEXT: ~$3 approximatelySign:~:sh|currency:$:sh|integer:3:sh ok
show(nf('en-US', {style: 'currency', currency: 'USD', signDisplay: 'always'}), 2.9, 3.1);
// CHECK-NEXT: +$2.90–3.10 plusSign:+:sh|currency:$:sh|integer:2:st|decimal:.:st|fraction:90:st|literal:–:sh|integer:3:en|decimal:.:en|fraction:10:en ok
print(nf('en-US').formatRange('987654321987654321', '987654321987654322'));
// CHECK-NEXT: 987,654,321,987,654,321–987,654,321,987,654,322
var eur0 = nf('pt-PT', {style: 'currency', currency: 'EUR', maximumFractionDigits: 0});
show(eur0, 3, 5);
// CHECK-NEXT: 3 - 5\u00a0€ integer:3:st|literal: - :sh|integer:5:en|literal:\u00a0:sh|currency:€:sh ok
show(eur0, 2.9, 3.1);
// CHECK-NEXT: ~3\u00a0€ approximatelySign:~:sh|integer:3:sh|literal:\u00a0:sh|currency:€:sh ok
print(escape(nf('pt-PT', {style: 'currency', currency: 'EUR', signDisplay: 'always'}).formatRange(2.9, 3.1)));
// CHECK-NEXT: +2,90 - 3,10\u00a0€
print(escape(nf('pt-PT').formatRange('987654321987654321', '987654321987654322')));
// CHECK-NEXT: 987\u00a0654\u00a0321\u00a0987\u00a0654\u00a0321 - 987\u00a0654\u00a0321\u00a0987\u00a0654\u00a0322

// Units: the unit collapses, and takes the end's plural form.
show(nf('en', {style: 'unit', unit: 'kilometer'}), 3, 5);
// CHECK-NEXT: 3–5 km integer:3:st|literal:–:sh|integer:5:en|literal: :sh|unit:km:sh ok
show(nf('en', {style: 'unit', unit: 'mile', unitDisplay: 'long'}), 1, 5);
// CHECK-NEXT: 1–5 miles integer:1:st|literal:–:sh|integer:5:en|literal: :sh|unit:miles:sh ok
show(nf('ru', {style: 'unit', unit: 'kilometer', unitDisplay: 'long'}), 1, 5);
// CHECK-NEXT: 1–5 километров integer:1:st|literal:–:sh|integer:5:en|literal: :sh|unit:километров:sh ok
show(nf('ja', {style: 'unit', unit: 'kilometer-per-hour', unitDisplay: 'long'}), 1, 5);
// CHECK-NEXT: 時速 1～5 キロメートル unit:時速:sh|literal: :sh|integer:1:st|literal:～:sh|integer:5:en|literal: :sh|unit:キロメートル:sh ok
// Two levels collapse separately, as in ICU4C: the outer affixes (the
// unit or currency name pattern's text) when they match, and then the
// middle ones (sign, currency, percent and compact text) only when they
// also match and have more than one code point. A sign of one code point
// is repeated, with spaces around the separator.
show(nf('en', {style: 'unit', unit: 'kilometer'}), -3, -5);
// CHECK-NEXT: -3 – -5 km minusSign:-:st|integer:3:st|literal: – :sh|minusSign:-:en|integer:5:en|literal: :sh|unit:km:sh ok
show(nf('en', {style: 'unit', unit: 'kilometer'}), -5, 5);
// CHECK-NEXT: -5 – 5 km minusSign:-:st|integer:5:st|literal: – :sh|integer:5:en|literal: :sh|unit:km:sh ok
show(nf('en', {style: 'unit', unit: 'kilometer', signDisplay: 'always'}), 3, 5);
// CHECK-NEXT: +3 – +5 km plusSign:+:st|integer:3:st|literal: – :sh|plusSign:+:en|integer:5:en|literal: :sh|unit:km:sh ok
show(nf('ja', {style: 'unit', unit: 'kilometer-per-hour', unitDisplay: 'long'}), -3, -5);
// CHECK-NEXT: 時速 -3 ～ -5 キロメートル unit:時速:sh|literal: :sh|minusSign:-:st|integer:3:st|literal: ～ :sh|minusSign:-:en|integer:5:en|literal: :sh|unit:キロメートル:sh ok
show(nf('en', {style: 'currency', currency: 'USD', currencyDisplay: 'name'}), -3, -5);
// CHECK-NEXT: -3.00 – -5.00 US dollars minusSign:-:st|integer:3:st|decimal:.:st|fraction:00:st|literal: – :sh|minusSign:-:en|integer:5:en|decimal:.:en|fraction:00:en|literal: :sh|currency:US dollars:sh ok
// Outer affixes that differ (tr's narrow "one" and "other" celsius
// patterns, "{0}°C" and "{0} °C") are repeated, with spaces. ICU4C gives
// "1–5 °C".
show(nf('tr', {style: 'unit', unit: 'celsius', unitDisplay: 'narrow'}), 1, 5);
// CHECK-NEXT: 1°C – 5 °C integer:1:st|unit:°C:st|literal: – :sh|integer:5:en|literal: :en|unit:°C:en ok
// A middle affix is dropped only if it has the currency or the percent
// sign, which every value of the formatter shows: fa's sign has two code
// points with its bidi mark, but another value has none, so it is repeated
// (the draft's no-ambiguity rule for CollapseNumberRange).
show(nf('fa', {style: 'unit', unit: 'kilometer'}), -3, -5);
// CHECK-NEXT: \u200e−۳ – \u200e−۵ کیلومتر literal:\u200e:st|minusSign:−:st|integer:۳:st|literal: – :sh|literal:\u200e:en|minusSign:−:en|integer:۵:en|literal: :sh|unit:کیلومتر:sh ok

// The percent sign is one code point: no collapse, and spaces around the
// separator.
show(nf('en', {style: 'percent'}), 0.03, 0.05);
// CHECK-NEXT: 3% – 5% integer:3:st|percentSign:%:st|literal: – :sh|integer:5:en|percentSign:%:en ok
show(nf('fr', {style: 'percent'}), 0.03, 0.05);
// CHECK-NEXT: 3–5\u00a0% integer:3:st|literal:–:sh|integer:5:en|literal:\u00a0:sh|percentSign:%:sh ok

// Signs: a sign alone is never dropped, since another value has none (the
// draft's no-ambiguity rule). ICU4C collapses fa's two-code-point sign
// ("\u200e−۳–۵").
show(nf('en'), -5, 5);
// CHECK-NEXT: -5 – 5 minusSign:-:st|integer:5:st|literal: – :sh|integer:5:en ok
show(nf('en'), -3, -5);
// CHECK-NEXT: -3 – -5 minusSign:-:st|integer:3:st|literal: – :sh|minusSign:-:en|integer:5:en ok
show(nf('fa'), -3, -5);
// CHECK-NEXT: \u200e−۳ – \u200e−۵ literal:\u200e:st|minusSign:−:st|integer:۳:st|literal: – :sh|literal:\u200e:en|minusSign:−:en|integer:۵:en ok
show(nf('en'), -0, 0);
// CHECK-NEXT: -0 – 0 minusSign:-:st|integer:0:st|literal: – :sh|integer:0:en ok

// Spaces go around the separator only when the start side repeats an
// affix (or a side shows an exponent), as in ICU4C: not when only the end
// side has one.
show(nf('en'), 5, -5);
// CHECK-NEXT: 5–-5 integer:5:st|literal:–:sh|minusSign:-:en|integer:5:en ok
show(nf('en', {signDisplay: 'exceptZero'}), 0, 5);
// CHECK-NEXT: 0–+5 integer:0:st|literal:–:sh|plusSign:+:en|integer:5:en ok
show(nf('en', {signDisplay: 'exceptZero'}), 0, Infinity);
// CHECK-NEXT: 0–+∞ integer:0:st|literal:–:sh|plusSign:+:en|infinity:∞:en ok

// x > y does not throw, for Numbers and BigInts.
show(nf('en'), 5, 3);
// CHECK-NEXT: 5–3 integer:5:st|literal:–:sh|integer:3:en ok
show(nf('en'), 23n, 12n);
// CHECK-NEXT: 23–12 integer:23:st|literal:–:sh|integer:12:en ok

// The infinities.
show(nf('en'), 0, Infinity);
// CHECK-NEXT: 0–∞ integer:0:st|literal:–:sh|infinity:∞:en ok
show(nf('en'), -Infinity, Infinity);
// CHECK-NEXT: -∞ – ∞ minusSign:-:st|infinity:∞:st|literal: – :sh|infinity:∞:en ok
show(nf('en'), '1e400', '1e400');
// CHECK-NEXT: ~∞ approximatelySign:~:sh|infinity:∞:sh ok

// NaN throws a RangeError, undefined a TypeError, from both methods.
var en = nf('en');
print(thrown(() => en.formatRange(NaN, 1)), thrown(() => en.formatRange(1, NaN)), thrown(() => en.formatRange('x', 1)));
// CHECK-NEXT: RangeError RangeError RangeError
print(thrown(() => en.formatRangeToParts(NaN, 1)), thrown(() => en.formatRangeToParts(1, NaN)));
// CHECK-NEXT: RangeError RangeError
print(thrown(() => en.formatRange(undefined, 1)), thrown(() => en.formatRange(1)), thrown(() => en.formatRangeToParts(1, undefined)));
// CHECK-NEXT: TypeError TypeError TypeError
// undefined is checked before NaN.
print(thrown(() => en.formatRange(undefined, NaN)));
// CHECK-NEXT: TypeError

// Compact: compact text alone is never dropped, since another value of the
// formatter has none or a different one: "3–5 thousand" would also be the
// range from 3 to 5000 (the draft's no-ambiguity rule for
// CollapseNumberRange). ICU4C collapses it. With the currency in the
// dropped affix ("$3–5K") the collapse is unambiguous, as in ICU4C.
var long = nf('en', {notation: 'compact', compactDisplay: 'long'});
show(long, 3000, 5000000);
// CHECK-NEXT: 3 thousand – 5 million integer:3:st|literal: :st|compact:thousand:st|literal: – :sh|integer:5:en|literal: :en|compact:million:en ok
show(long, 3000, 5000);
// CHECK-NEXT: 3 thousand – 5 thousand integer:3:st|literal: :st|compact:thousand:st|literal: – :sh|integer:5:en|literal: :en|compact:thousand:en ok
show(long, 3, 5000);
// CHECK-NEXT: 3–5 thousand integer:3:st|literal:–:sh|integer:5:en|literal: :en|compact:thousand:en ok
show(nf('en', {notation: 'compact'}), 3000, 5000);
// CHECK-NEXT: 3K – 5K integer:3:st|compact:K:st|literal: – :sh|integer:5:en|compact:K:en ok
show(nf('en', {style: 'currency', currency: 'USD', notation: 'compact'}), 3000, 5000);
// CHECK-NEXT: $3–5K currency:$:sh|integer:3:st|literal:–:sh|integer:5:en|compact:K:sh ok
show(nf('en', {style: 'currency', currency: 'USD', notation: 'compact'}), 3000, 5000000);
// CHECK-NEXT: $3K – $5M currency:$:st|integer:3:st|compact:K:st|literal: – :sh|currency:$:en|integer:5:en|compact:M:en ok
show(nf('sw', {notation: 'compact'}), 3000, 5000);
// CHECK-NEXT: elfu\u00a03 – elfu\u00a05 compact:elfu:st|literal:\u00a0:st|integer:3:st|literal: – :sh|compact:elfu:en|literal:\u00a0:en|integer:5:en ok
// A side without a number (fr long's exact-1 "mille", decision 10) never
// collapses.
show(nf('fr', {notation: 'compact', compactDisplay: 'long'}), 1000, 2000);
// CHECK-NEXT: mille – 2 mille compact:mille:st|literal: – :sh|integer:2:en|literal: :en|compact:mille:en ok
// The unit collapses and the one-code-point compact symbol is repeated.
// The exponents need to be equal only for the middle level, where the
// compact text is: with different exponents the unit still collapses.
show(nf('en', {style: 'unit', unit: 'kilometer', notation: 'compact'}), 3000, 5000);
// CHECK-NEXT: 3K – 5K km integer:3:st|compact:K:st|literal: – :sh|integer:5:en|compact:K:en|literal: :sh|unit:km:sh ok
show(nf('en', {style: 'unit', unit: 'kilometer', notation: 'compact'}), 3000, 5000000);
// CHECK-NEXT: 3K – 5M km integer:3:st|compact:K:st|literal: – :sh|integer:5:en|compact:M:en|literal: :sh|unit:km:sh ok

// Scientific: the exponent parts are numbers, so they never collapse; a
// side's numbers run from its first numeric part to its last, across the
// bidi mark of ar-EG's exponent minus sign. Sides that both show an
// exponent get spaces around the separator, as in ICU4C.
var sci = nf('en', {notation: 'scientific'});
show(sci, 3000, 5000);
// CHECK-NEXT: 3E3 – 5E3 integer:3:st|exponentSeparator:E:st|exponentInteger:3:st|literal: – :sh|integer:5:en|exponentSeparator:E:en|exponentInteger:3:en ok
show(sci, 3000, 50000);
// CHECK-NEXT: 3E3 – 5E4 integer:3:st|exponentSeparator:E:st|exponentInteger:3:st|literal: – :sh|integer:5:en|exponentSeparator:E:en|exponentInteger:4:en ok
show(nf('ar-EG', {notation: 'scientific'}), 0.001, 0.005);
// CHECK-NEXT: ١أس\u061c-٣ – ٥أس\u061c-٣ integer:١:st|exponentSeparator:أس:st|literal:\u061c:st|exponentMinusSign:-:st|exponentInteger:٣:st|literal: – :sh|integer:٥:en|exponentSeparator:أس:en|literal:\u061c:en|exponentMinusSign:-:en|exponentInteger:٣:en ok
// With an exponent the middle level never collapses: ICU4C's
// "\u200e-3E0 – 5E0" for -3 to -5 is also its range from -3 to 5.
show(nf('ar', {notation: 'scientific'}), -3, -5);
// CHECK-NEXT: \u200e-3E0 – \u200e-5E0 literal:\u200e:st|minusSign:-:st|integer:3:st|exponentSeparator:E:st|exponentInteger:0:st|literal: – :sh|literal:\u200e:en|minusSign:-:en|integer:5:en|exponentSeparator:E:en|exponentInteger:0:en ok
show(nf('en', {style: 'currency', currency: 'USD', currencyDisplay: 'code', notation: 'scientific'}), 3000, 5000);
// CHECK-NEXT: USD\u00a03E3 – USD\u00a05E3 currency:USD:st|literal:\u00a0:st|integer:3:st|exponentSeparator:E:st|exponentInteger:3:st|literal: – :sh|currency:USD:en|literal:\u00a0:en|integer:5:en|exponentSeparator:E:en|exponentInteger:3:en ok
show(nf('en', {style: 'unit', unit: 'kilometer', notation: 'scientific'}), 3000, 5000);
// CHECK-NEXT: 3E3 – 5E3 km integer:3:st|exponentSeparator:E:st|exponentInteger:3:st|literal: – :sh|integer:5:en|exponentSeparator:E:en|exponentInteger:3:en|literal: :sh|unit:km:sh ok

// Currencies: code, accounting, and other locales' range patterns.
show(nf('en', {style: 'currency', currency: 'USD', currencyDisplay: 'code'}), 3, 5);
// CHECK-NEXT: USD\u00a03.00–5.00 currency:USD:sh|literal:\u00a0:sh|integer:3:st|decimal:.:st|fraction:00:st|literal:–:sh|integer:5:en|decimal:.:en|fraction:00:en ok
show(nf('ja', {style: 'currency', currency: 'JPY'}), 3, 5);
// CHECK-NEXT: ￥3 ～ ￥5 currency:￥:st|integer:3:st|literal: ～ :sh|currency:￥:en|integer:5:en ok
show(nf('de', {style: 'currency', currency: 'EUR'}), 3, 5);
// CHECK-NEXT: 3,00–5,00\u00a0€ integer:3:st|decimal:,:st|fraction:00:st|literal:–:sh|integer:5:en|decimal:,:en|fraction:00:en|literal:\u00a0:sh|currency:€:sh ok
show(nf('en', {style: 'currency', currency: 'USD', currencyDisplay: 'name'}), 1, 5);
// CHECK-NEXT: 1.00–5.00 US dollars integer:1:st|decimal:.:st|fraction:00:st|literal:–:sh|integer:5:en|decimal:.:en|fraction:00:en|literal: :sh|currency:US dollars:sh ok
var accounting = nf('en', {style: 'currency', currency: 'USD', currencySign: 'accounting'});
// The accounting ")" dropped from the start is not shown for every value,
// but the "($" dropped from the end has the currency, which is, so the
// collapse is unambiguous.
show(accounting, -3, -5);
// CHECK-NEXT: ($3.00–5.00) literal:(:sh|currency:$:sh|integer:3:st|decimal:.:st|fraction:00:st|literal:–:sh|integer:5:en|decimal:.:en|fraction:00:en|literal:):sh ok
// ICU4C gives the same string, with ")" and the separator in one part.
show(accounting, -3, 5);
// CHECK-NEXT: ($3.00) – $5.00 literal:(:st|currency:$:st|integer:3:st|decimal:.:st|fraction:00:st|literal:):st|literal: – :sh|currency:$:en|integer:5:en|decimal:.:en|fraction:00:en ok

// The approximately sign goes before the first sign, where the plus sign
// would be for a value that is not negative, and before an accounting
// negative (decision 15; ICU4C gives "($3.00)" there, without the sign).
show(accounting, 3, 3);
// CHECK-NEXT: ~$3.00 approximatelySign:~:sh|currency:$:sh|integer:3:sh|decimal:.:sh|fraction:00:sh ok
show(accounting, -3, -3);
// CHECK-NEXT: ~($3.00) approximatelySign:~:sh|literal:(:sh|currency:$:sh|integer:3:sh|decimal:.:sh|fraction:00:sh|literal:):sh ok
show(nf('en'), 3, 3);
// CHECK-NEXT: ~3 approximatelySign:~:sh|integer:3:sh ok
show(nf('en'), -3, -3);
// CHECK-NEXT: ~-3 approximatelySign:~:sh|minusSign:-:sh|integer:3:sh ok
show(nf('en', {signDisplay: 'always'}), 3, 3);
// CHECK-NEXT: ~+3 approximatelySign:~:sh|plusSign:+:sh|integer:3:sh ok
show(nf('en', {signDisplay: 'never'}), -3, -3);
// CHECK-NEXT: ~3 approximatelySign:~:sh|integer:3:sh ok
show(nf('en', {style: 'currency', currency: 'USD', signDisplay: 'never'}), -3, -3);
// CHECK-NEXT: ~$3.00 approximatelySign:~:sh|currency:$:sh|integer:3:sh|decimal:.:sh|fraction:00:sh ok
show(nf('en', {style: 'currency', currency: 'USD', currencyDisplay: 'name'}), 3, 3);
// CHECK-NEXT: ~3.00 US dollars approximatelySign:~:sh|integer:3:sh|decimal:.:sh|fraction:00:sh|literal: :sh|currency:US dollars:sh ok
show(nf('fr', {style: 'currency', currency: 'EUR'}), -3, -3);
// CHECK-NEXT: ≃-3,00\u00a0€ approximatelySign:≃:sh|minusSign:-:sh|integer:3:sh|decimal:,:sh|fraction:00:sh|literal:\u00a0:sh|currency:€:sh ok
show(nf('nb'), 3, 3);
// CHECK-NEXT: ca.3 approximatelySign:ca.:sh|integer:3:sh ok
// signDisplay "never" shows a negative with the zero pattern, so the sign
// goes where the plus sign stands, inside the pattern's own bidi mark, as
// for a positive value (and as in ICU4C).
var egpNever = nf('ar', {style: 'currency', currency: 'EGP', signDisplay: 'never'});
show(egpNever, -3, -3);
// CHECK-NEXT: \u200f~3.00\u00a0ج.م.\u200f literal:\u200f:sh|approximatelySign:~:sh|integer:3:sh|decimal:.:sh|fraction:00:sh|literal:\u00a0:sh|currency:ج.م.:sh|literal:\u200f:sh ok
show(egpNever, 3, 3);
// CHECK-NEXT: \u200f~3.00\u00a0ج.م.\u200f literal:\u200f:sh|approximatelySign:~:sh|integer:3:sh|decimal:.:sh|fraction:00:sh|literal:\u00a0:sh|currency:ج.م.:sh|literal:\u200f:sh ok
// The draft compares the FormatNumeric strings: two values that both
// display "0%" are approximate. ICU4C gives "0% – 0%".
show(nf('en', {style: 'percent', signDisplay: 'exceptZero'}), -0.0001, 0.0001);
// CHECK-NEXT: ~0% approximatelySign:~:sh|integer:0:sh|percentSign:%:sh ok
// It goes before the bidi marks of the sign, which belong to CLDR's sign
// symbol, and after the pattern's own marks.
show(nf('ar-EG'), -3, -3);
// CHECK-NEXT: ~\u061c-٣ approximatelySign:~:sh|literal:\u061c:sh|minusSign:-:sh|integer:٣:sh ok
show(nf('ar-EG', {signDisplay: 'always'}), 3, 3);
// CHECK-NEXT: ~\u061c+٣ approximatelySign:~:sh|literal:\u061c:sh|plusSign:+:sh|integer:٣:sh ok
show(nf('ar', {style: 'currency', currency: 'EGP'}), 3, 3);
// CHECK-NEXT: \u200f~3.00\u00a0ج.م.\u200f literal:\u200f:sh|approximatelySign:~:sh|integer:3:sh|decimal:.:sh|fraction:00:sh|literal:\u00a0:sh|currency:ج.م.:sh|literal:\u200f:sh ok
show(nf('ar', {style: 'currency', currency: 'EGP'}), -3, -3);
// CHECK-NEXT: \u200f~\u200e-3.00\u00a0ج.م.\u200f literal:\u200f:sh|approximatelySign:~:sh|literal:\u200e:sh|minusSign:-:sh|integer:3:sh|decimal:.:sh|fraction:00:sh|literal:\u00a0:sh|currency:ج.م.:sh|literal:\u200f:sh ok
show(nf('ar-EG', {style: 'currency', currency: 'EGP'}), -3, -3);
// CHECK-NEXT: ~\u061c-\u200f٣٫٠٠\u00a0ج.م.\u200f approximatelySign:~:sh|literal:\u061c:sh|minusSign:-:sh|literal:\u200f:sh|integer:٣:sh|decimal:٫:sh|fraction:٠٠:sh|literal:\u00a0:sh|currency:ج.م.:sh|literal:\u200f:sh ok
// A pattern without a number (ar long "one" meter, decision 10) has no
// plus sign; the sign goes first. ICU4C gives "متر", without it.
show(nf('ar', {style: 'unit', unit: 'meter', unitDisplay: 'long'}), 1, 1);
// CHECK-NEXT: ~متر approximatelySign:~:sh|unit:متر:sh ok

// Numbering systems: the digits and the locale's range pattern.
show(nf('en-u-nu-arab'), 3, 5);
// CHECK-NEXT: ٣–٥ integer:٣:st|literal:–:sh|integer:٥:en ok
show(nf('ar-EG'), 3, 5);
// CHECK-NEXT: ٣–٥ integer:٣:st|literal:–:sh|integer:٥:en ok
show(nf('ko'), 3, 5);
// CHECK-NEXT: 3~5 integer:3:st|literal:~:sh|integer:5:en ok

// The parts are ordinary objects with own data properties.
var part = en.formatRangeToParts(3, 5)[1];
print(Object.getPrototypeOf(part) === Object.prototype, JSON.stringify(Object.getOwnPropertyDescriptors(part)));
// CHECK-NEXT: true {"type":{"value":"literal","writable":true,"enumerable":true,"configurable":true},"value":{"value":"–","writable":true,"enumerable":true,"configurable":true},"source":{"value":"shared","writable":true,"enumerable":true,"configurable":true}}

// mk's separator "\u2009–\u2009" already has whitespace (U+2009) at both
// ends, so no spaces are added where they would be: spaces go only where
// the separator has none. ICU4C adds ASCII spaces ("-3 \u2009–\u2009 5").
show(nf('mk'), -3, 5);
// CHECK-NEXT: -3\u2009–\u20095 minusSign:-:st|integer:3:st|literal:\u2009–\u2009:sh|integer:5:en ok

// The draft's no-ambiguity rule for CollapseNumberRange: two pairs that
// format differently without collapsing (their two FormatNumeric strings
// differ) never give the same range.
var sweepLocales = ['en', 'fr', 'de', 'pt-PT', 'ar', 'ar-EG', 'fa', 'bg', 'sw', 'ja', 'he'];
var sweepOptions = [
  {},
  {signDisplay: 'always'},
  {signDisplay: 'exceptZero'},
  {style: 'currency', currency: 'EUR', signDisplay: 'always'},
  {style: 'currency', currency: 'USD', currencySign: 'accounting', signDisplay: 'always'},
  {style: 'percent', signDisplay: 'exceptZero'},
  {style: 'percent'},
  {style: 'currency', currency: 'USD'},
  {style: 'currency', currency: 'EUR', currencyDisplay: 'code'},
  {style: 'currency', currency: 'USD', currencySign: 'accounting'},
  {style: 'unit', unit: 'kilometer'},
  {notation: 'scientific'},
  {notation: 'engineering'},
  {notation: 'compact'},
  {notation: 'compact', compactDisplay: 'long'},
  {style: 'currency', currency: 'USD', notation: 'compact'},
  {style: 'unit', unit: 'kilometer', unitDisplay: 'long', notation: 'compact', compactDisplay: 'long'},
];
var sweepValues = [3, 5, -3, -5, 0, 3000, 5000, -3000, 5000000, 0.001];
var collisions = [];
sweepLocales.forEach(function (locale) {
  sweepOptions.forEach(function (opts) {
    var f = nf(locale, opts);
    var formatted = sweepValues.map(v => f.format(v));
    var seen = Object.create(null);
    sweepValues.forEach(function (x, i) {
      sweepValues.forEach(function (y, j) {
        var range = f.formatRange(x, y);
        var key = formatted[i] === formatted[j] ? formatted[i] : formatted[i] + '\u0001' + formatted[j];
        if (seen[range] === undefined) seen[range] = {key: key, pair: x + ',' + y};
        else if (seen[range].key !== key)
          collisions.push(locale + ' ' + JSON.stringify(opts) + ' ' + seen[range].pair + ' and ' + x + ',' + y + ': ' + escape(range));
      });
    });
  });
});
print('collisions', collisions.length, collisions.slice(0, 3).join('; '));
// CHECK-NEXT: collisions 0
