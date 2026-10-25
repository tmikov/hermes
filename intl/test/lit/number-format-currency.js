/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: cat %S/Inputs/escape.js %s > %t.js
// RUN: %hermes -Xintl-default-locale=en-US %t.js | %FileCheck --match-full-lines %s

// Intl.NumberFormat with the currency style: the four currency displays,
// accounting, the currency digits, currencies CLDR does not know, NaN and
// infinity next to a currency, and the plural forms of currency names
// (plan decisions 8, 10, 11 and 13). The expected strings and parts are
// ICU4C's (Node 24, ICU 78.2).

/// The currency formatter of locale for currency, with the other options
/// of extra.
function currencyFormat(locale, currency, extra) {
  return new Intl.NumberFormat(
    locale,
    Object.assign({style: 'currency', currency: currency}, extra),
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

// The symbol display, en USD.
show(currencyFormat('en', 'USD'), 1234.5);
// CHECK: $1,234.50 currency:$|integer:1|group:,|integer:234|decimal:.|fraction:50 ok

// Accounting: the negative pattern has parentheses and no minus sign, and
// negative zero takes it too (decision 11).
show(currencyFormat('en', 'USD', {currencySign: 'accounting'}), -1);
// CHECK-NEXT: ($1.00) literal:(|currency:$|integer:1|decimal:.|fraction:00|literal:) ok
show(currencyFormat('en', 'USD', {currencySign: 'accounting'}), -0);
// CHECK-NEXT: ($0.00) literal:(|currency:$|integer:0|decimal:.|fraction:00|literal:) ok
show(currencyFormat('en', 'USD', {currencySign: 'accounting'}), 1);
// CHECK-NEXT: $1.00 currency:$|integer:1|decimal:.|fraction:00 ok

// The code display, with CLDR's currency spacing (U+00A0) between the code
// and a digit.
show(currencyFormat('en', 'USD', {currencyDisplay: 'code'}), 1234.5);
// CHECK-NEXT: USD\u00a01,234.50 currency:USD|literal:\u00a0|integer:1|group:,|integer:234|decimal:.|fraction:50 ok

// The name display: the plural form of the displayed number, so "1.00" is
// "other" in en; a literal space and the name as a currency part.
show(currencyFormat('en', 'USD', {currencyDisplay: 'name'}), 1);
// CHECK-NEXT: 1.00 US dollars integer:1|decimal:.|fraction:00|literal: |currency:US dollars ok
show(currencyFormat('en', 'USD', {currencyDisplay: 'name', maximumFractionDigits: 0}), 1);
// CHECK-NEXT: 1 US dollar integer:1|literal: |currency:US dollar ok
show(currencyFormat('en', 'EUR', {currencyDisplay: 'name'}), 1);
// CHECK-NEXT: 1.00 euros integer:1|decimal:.|fraction:00|literal: |currency:euros ok
// The sign comes from the decimal patterns, and the name pattern wraps the
// signed number; there is no accounting form for names.
show(currencyFormat('en', 'USD', {currencyDisplay: 'name'}), -1);
// CHECK-NEXT: -1.00 US dollars minusSign:-|integer:1|decimal:.|fraction:00|literal: |currency:US dollars ok
show(currencyFormat('en', 'USD', {currencyDisplay: 'name', currencySign: 'accounting'}), -1);
// CHECK-NEXT: -1.00 US dollars minusSign:-|integer:1|decimal:.|fraction:00|literal: |currency:US dollars ok
show(currencyFormat('en', 'USD', {currencyDisplay: 'name', signDisplay: 'always'}), 1);
// CHECK-NEXT: +1.00 US dollars plusSign:+|integer:1|decimal:.|fraction:00|literal: |currency:US dollars ok
// Leading zeros of minimumIntegerDigits keep the plural form.
show(
  currencyFormat('en', 'USD', {currencyDisplay: 'name', maximumFractionDigits: 0, minimumIntegerDigits: 3}),
  1,
);
// CHECK-NEXT: 001 US dollar integer:001|literal: |currency:US dollar ok

// ru has four plural forms of a currency name.
var ruNames = currencyFormat('ru', 'RUB', {currencyDisplay: 'name', minimumFractionDigits: 0});
print([1, 2, 5, 21, 1.5].map(x => ruNames.format(x)).join(' | '));
// CHECK-NEXT: 1 российский рубль | 2 российских рубля | 5 российских рублей | 21 российский рубль | 1,5 российского рубля
// ar picks "two" for 2.00, and ar-EG's signs carry an ALM as literal text.
show(currencyFormat('ar', 'EGP', {currencyDisplay: 'name'}), 2);
// CHECK-NEXT: 2.00 جنيهان مصريان integer:2|decimal:.|fraction:00|literal: |currency:جنيهان مصريان ok
show(currencyFormat('ar-EG', 'EGP', {currencyDisplay: 'name'}), -2);
// CHECK-NEXT: \u061c-٢٫٠٠ جنيهان مصريان literal:\u061c|minusSign:-|integer:٢|decimal:٫|fraction:٠٠|literal: |currency:جنيهان مصريان ok
// fr's name pattern has the singular for 1.
show(currencyFormat('fr', 'USD', {currencyDisplay: 'name'}), 1);
// CHECK-NEXT: 1,00 dollar des États-Unis integer:1|decimal:,|fraction:00|literal: |currency:dollar des États-Unis ok

// de puts the euro after the number; ja's yen has no fraction digits.
show(currencyFormat('de', 'EUR'), 1234.5);
// CHECK-NEXT: 1.234,50\u00a0€ integer:1|group:.|integer:234|decimal:,|fraction:50|literal:\u00a0|currency:€ ok
show(currencyFormat('ja', 'JPY'), 1234.5);
// CHECK-NEXT: ￥1,235 currency:￥|integer:1|group:,|integer:235 ok

// ja's name pattern has no space, from the locale's own pattern.
show(currencyFormat('ja', 'USD', {currencyDisplay: 'name'}), 1234.5);
// CHECK-NEXT: 1,234.50米ドル integer:1|group:,|integer:234|decimal:.|fraction:50|currency:米ドル ok

// A currency CLDR does not list shows its code, in every display, and has
// two digits (decision 8); XXX has its own symbol.
show(currencyFormat('en', 'ABC'), 1);
// CHECK-NEXT: ABC\u00a01.00 currency:ABC|literal:\u00a0|integer:1|decimal:.|fraction:00 ok
show(currencyFormat('en', 'abc', {currencyDisplay: 'name'}), 2);
// CHECK-NEXT: 2.00 ABC integer:2|decimal:.|fraction:00|literal: |currency:ABC ok
show(currencyFormat('en', 'XXX'), 1);
// CHECK-NEXT: ¤1.00 currency:¤|integer:1|decimal:.|fraction:00 ok

// The digit options override the currency's digits.
show(currencyFormat('en', 'USD', {minimumFractionDigits: 0}), 1);
// CHECK-NEXT: $1 currency:$|integer:1 ok

// The narrow symbol of CAD is "$"; its symbol is "CA$".
show(currencyFormat('en', 'CAD', {currencyDisplay: 'narrowSymbol'}), 1);
// CHECK-NEXT: $1.00 currency:$|integer:1|decimal:.|fraction:00 ok
show(currencyFormat('en', 'CAD'), 1);
// CHECK-NEXT: CA$1.00 currency:CA$|integer:1|decimal:.|fraction:00 ok

// No currency spacing next to NaN or infinity, which are not digits; de's
// pattern has its own space. The name display takes "other" for both.
show(currencyFormat('en', 'USD', {currencyDisplay: 'code'}), NaN);
// CHECK-NEXT: USDNaN currency:USD|nan:NaN ok
show(currencyFormat('en', 'USD', {currencyDisplay: 'code'}), -Infinity);
// CHECK-NEXT: -USD∞ minusSign:-|currency:USD|infinity:∞ ok
show(currencyFormat('de', 'EUR', {currencyDisplay: 'code'}), NaN);
// CHECK-NEXT: NaN\u00a0EUR nan:NaN|literal:\u00a0|currency:EUR ok
show(currencyFormat('en', 'USD', {currencyDisplay: 'name'}), NaN);
// CHECK-NEXT: NaN US dollars nan:NaN|literal: |currency:US dollars ok
show(currencyFormat('en', 'USD', {currencyDisplay: 'name'}), Infinity);
// CHECK-NEXT: ∞ US dollars infinity:∞|literal: |currency:US dollars ok

// signDisplay "always" shows the plus sign for zero.
show(currencyFormat('en', 'USD', {signDisplay: 'always'}), 0);
// CHECK-NEXT: +$0.00 plusSign:+|currency:$|integer:0|decimal:.|fraction:00 ok

// Grouping and numbering systems apply inside the currency pattern; he and
// ar-EG keep the bidi marks of their patterns as literal text. CLDR's ar
// symbol for EGP ends in U+200F, which becomes a literal part, as in ICU4C.
show(currencyFormat('en-IN', 'INR'), 1234567.5);
// CHECK-NEXT: ₹12,34,567.50 currency:₹|integer:12|group:,|integer:34|group:,|integer:567|decimal:.|fraction:50 ok
show(currencyFormat('hi-u-nu-deva', 'INR'), 1234.5);
// CHECK-NEXT: ₹१,२३४.५० currency:₹|integer:१|group:,|integer:२३४|decimal:.|fraction:५० ok
show(currencyFormat('he', 'ILS'), -1);
// CHECK-NEXT: \u200f\u200e-1.00\u00a0\u200f₪ literal:\u200f\u200e|minusSign:-|integer:1|decimal:.|fraction:00|literal:\u00a0\u200f|currency:₪ ok
show(currencyFormat('ar-EG', 'EGP'), -1234.5);
// CHECK-NEXT: \u061c-\u200f١٬٢٣٤٫٥٠\u00a0ج.م.\u200f literal:\u061c|minusSign:-|literal:\u200f|integer:١|group:٬|integer:٢٣٤|decimal:٫|fraction:٥٠|literal:\u00a0|currency:ج.م.|literal:\u200f ok

// The name display uses the currency's digits too: none for JPY.
print(currencyFormat('en', 'JPY', {currencyDisplay: 'name'}).format(1234.5));
// CHECK-NEXT: 1,235 Japanese yen

// The currency style's own separators, CLDR's currencyGroup (de-AT, whose
// group separator is U+00A0) and currencyDecimal (fr-CH, whose decimal
// separator is ","), in every display; other styles keep the plain ones.
show(currencyFormat('de-AT', 'EUR'), 1234.5);
// CHECK-NEXT: €\u00a01.234,50 currency:€|literal:\u00a0|integer:1|group:.|integer:234|decimal:,|fraction:50 ok
show(currencyFormat('de-AT', 'EUR', {currencyDisplay: 'name'}), 1234.5);
// CHECK-NEXT: 1.234,50 Euro integer:1|group:.|integer:234|decimal:,|fraction:50|literal: |currency:Euro ok
show(currencyFormat('fr-CH', 'CHF'), -123.5);
// CHECK-NEXT: -123.50\u00a0CHF minusSign:-|integer:123|decimal:.|fraction:50|literal:\u00a0|currency:CHF ok
show(currencyFormat('fr-CH', 'CHF', {currencyDisplay: 'code'}), -123.5);
// CHECK-NEXT: -123.50\u00a0CHF minusSign:-|integer:123|decimal:.|fraction:50|literal:\u00a0|currency:CHF ok
print(escape(new Intl.NumberFormat('de-AT').format(1234.5)));
// CHECK-NEXT: 1\u00a0234,5
print(escape(new Intl.NumberFormat('fr-CH').format(-123.5)));
// CHECK-NEXT: -123,5
// A numbering system root aliases to latn has the locale's latn
// symbols, the currency separators included, whether nu is an option or
// the -u-nu- keyword; the decimal style next to each keeps the plain
// separators. fr-CH's group separator is "'" in CLDR 48.2, U+202F in
// ICU4C's CLDR 48.0.
show(currencyFormat('de-AT', 'EUR', {numberingSystem: 'deva'}), 1234.5);
// CHECK-NEXT: €\u00a0१.२३४,५० currency:€|literal:\u00a0|integer:१|group:.|integer:२३४|decimal:,|fraction:५० ok
show(currencyFormat('de-AT-u-nu-deva', 'EUR'), 1234.5);
// CHECK-NEXT: €\u00a0१.२३४,५० currency:€|literal:\u00a0|integer:१|group:.|integer:२३४|decimal:,|fraction:५० ok
show(new Intl.NumberFormat('de-AT', {numberingSystem: 'deva'}), 1234.5);
// CHECK-NEXT: १\u00a0२३४,५ integer:१|group:\u00a0|integer:२३४|decimal:,|fraction:५ ok
show(
  currencyFormat('de-AT', 'EUR', {numberingSystem: 'beng', currencyDisplay: 'name'}),
  1234.5,
);
// CHECK-NEXT: ১.২৩৪,৫০ Euro integer:১|group:.|integer:২৩৪|decimal:,|fraction:৫০|literal: |currency:Euro ok
show(currencyFormat('fr-CH', 'CHF', {numberingSystem: 'deva'}), -1234.5);
// CHECK-NEXT: -१'२३४.५०\u00a0CHF minusSign:-|integer:१|group:'|integer:२३४|decimal:.|fraction:५०|literal:\u00a0|currency:CHF ok
show(new Intl.NumberFormat('fr-CH', {numberingSystem: 'deva'}), -1234.5);
// CHECK-NEXT: -१'२३४,५ minusSign:-|integer:१|group:'|integer:२३४|decimal:,|fraction:५ ok
show(
  currencyFormat('fr-CH', 'CHF', {numberingSystem: 'thai', currencyDisplay: 'code'}),
  -123.5,
);
// CHECK-NEXT: -๑๒๓.๕๐\u00a0CHF minusSign:-|integer:๑๒๓|decimal:.|fraction:๕๐|literal:\u00a0|currency:CHF ok
// Root's arab and arabext symbols are explicit and have no currency
// separators, so the currency style uses their plain ones (UTS 35's
// currencyDecimal and currencyGroup). ICU4C differs: it takes the
// missing symbols from the locale's latn ones ("١.٢٣٤" for
// de-AT, "." as fr-CH's decimal).
show(currencyFormat('de-AT', 'EUR', {numberingSystem: 'arab'}), 1234.5);
// CHECK-NEXT: ١٬٢٣٤٫٥٠\u00a0€ integer:١|group:٬|integer:٢٣٤|decimal:٫|fraction:٥٠|literal:\u00a0|currency:€ ok
show(new Intl.NumberFormat('de-AT', {numberingSystem: 'arab'}), 1234.5);
// CHECK-NEXT: ١٬٢٣٤٫٥ integer:١|group:٬|integer:٢٣٤|decimal:٫|fraction:٥ ok
show(currencyFormat('fr-CH', 'CHF', {numberingSystem: 'arabext'}), -1234.5);
// CHECK-NEXT: \u200e-\u200e۱٬۲۳۴٫۵۰\u00a0CHF literal:\u200e|minusSign:-|literal:\u200e|integer:۱|group:٬|integer:۲۳۴|decimal:٫|fraction:۵۰|literal:\u00a0|currency:CHF ok
// Per-currency formats, CLDR's <currencies><currency type="X"> pattern,
// decimal and group. UTS 35's lateral inheritance (tr35.md, "Lateral
// Inheritance") resolves each in the locale chain: at each locale, the
// currency's own value, else that locale's own currencyFormat pattern (of
// the numbering system and of standard or accounting) or symbols decimal
// or group, which ends the search; only then the parent. en-150 and its
// children format EUR with "¤#,##0.00" and other currencies with their own
// pattern "#,##0.00 ¤"; en-DE keeps its own separators. ICU4C applies the
// locale's default currency's format to every currency ("US$1,234.50" for
// en-DE USD) and en-150's separators with it ("€1,234.50" for en-DE EUR).
show(currencyFormat('en-150', 'EUR'), -1234.5);
// CHECK-NEXT: -€1,234.50 minusSign:-|currency:€|integer:1|group:,|integer:234|decimal:.|fraction:50 ok
show(currencyFormat('en-150', 'USD'), -1234.5);
// CHECK-NEXT: -1,234.50\u00a0US$ minusSign:-|integer:1|group:,|integer:234|decimal:.|fraction:50|literal:\u00a0|currency:US$ ok
show(currencyFormat('en-DE', 'EUR'), 1234.5);
// CHECK-NEXT: €1.234,50 currency:€|integer:1|group:.|integer:234|decimal:,|fraction:50 ok
show(currencyFormat('en-DE', 'USD'), 1234.5);
// CHECK-NEXT: 1.234,50\u00a0US$ integer:1|group:.|integer:234|decimal:,|fraction:50|literal:\u00a0|currency:US$ ok
show(currencyFormat('en-AT', 'EUR'), 1234.5);
// CHECK-NEXT: €\u00a01.234,50 currency:€|literal:\u00a0|integer:1|group:.|integer:234|decimal:,|fraction:50 ok
// en-CH has its own standard and accounting currency patterns, which come
// before en-150's EUR pattern for latn; deva has none in en-CH, so it
// takes en-150's.
show(currencyFormat('en-CH', 'EUR'), -1234.5);
// CHECK-NEXT: €-1'234.50 currency:€|minusSign:-|integer:1|group:'|integer:234|decimal:.|fraction:50 ok
show(currencyFormat('en-CH', 'EUR', {numberingSystem: 'deva'}), -1234.5);
// CHECK-NEXT: -€१'२३४.५० minusSign:-|currency:€|integer:१|group:'|integer:२३४|decimal:.|fraction:५० ok
// en-PL has its own accounting pattern but no standard one (only its
// alphaNextToNumber form): standard takes en-150's EUR pattern, accounting
// en-PL's.
show(currencyFormat('en-PL', 'EUR'), -1234.5);
// CHECK-NEXT: -€1.234,50 minusSign:-|currency:€|integer:1|group:.|integer:234|decimal:,|fraction:50 ok
show(currencyFormat('en-PL', 'EUR', {currencySign: 'accounting'}), -1234.5);
// CHECK-NEXT: (1.234,50\u00a0€) literal:(|integer:1|group:.|integer:234|decimal:,|fraction:50|literal:\u00a0|currency:€|literal:) ok
// it-CH's own separators and standard pattern come before it's ITL ones;
// its accounting pattern has only an alphaNextToNumber form, so accounting
// takes it's ITL pattern, with it-CH's separators.
show(currencyFormat('it-CH', 'ITL'), -1234567.5);
// CHECK-NEXT: ITL-1'234'568 currency:ITL|minusSign:-|integer:1|group:'|integer:234|group:'|integer:568 ok
show(currencyFormat('it-CH', 'ITL', {currencySign: 'accounting'}), -1234567.5);
// CHECK-NEXT: -ITL\u00a01'234'568 minusSign:-|currency:ITL|literal:\u00a0|integer:1|group:'|integer:234|group:'|integer:568 ok
// The pattern has no alphaNextToNumber form, so CLDR's currency spacing
// applies to it. Found before the locale's accounting pattern, it is the
// accounting pattern too (lateral inheritance's "unless otherwise
// specified" only picks the fallback source), as ICU4C formats it.
show(currencyFormat('tr', 'TRY', {currencyDisplay: 'code'}), 1234.5);
// CHECK-NEXT: TRY\u00a01.234,50 currency:TRY|literal:\u00a0|integer:1|group:.|integer:234|decimal:,|fraction:50 ok
show(currencyFormat('tr', 'TRY', {currencySign: 'accounting'}), -1234.5);
// CHECK-NEXT: -₺1.234,50 minusSign:-|currency:₺|integer:1|group:.|integer:234|decimal:,|fraction:50 ok
show(currencyFormat('en-150', 'EUR', {currencySign: 'accounting'}), -1234.5);
// CHECK-NEXT: -€1,234.50 minusSign:-|currency:€|integer:1|group:,|integer:234|decimal:.|fraction:50 ok
// pt-CV's escudo has the decimal separator "$" (its symbol is U+200B);
// other currencies keep ",". ICU4C gives "1234$50\u00a0US$" for USD.
show(currencyFormat('pt-CV', 'CVE'), -1234.5);
// CHECK-NEXT: -1234$50\u00a0\u200b minusSign:-|integer:1234|decimal:$|fraction:50|literal:\u00a0|currency:\u200b ok
show(currencyFormat('pt-CV', 'USD'), -1234.5);
// CHECK-NEXT: -1234,50\u00a0US$ minusSign:-|integer:1234|decimal:,|fraction:50|literal:\u00a0|currency:US$ ok
show(currencyFormat('pt-PT', 'PTE'), 1234567.5);
// CHECK-NEXT: 1,234,567$50\u00a0\u200b integer:1|group:,|integer:234|group:,|integer:567|decimal:$|fraction:50|literal:\u00a0|currency:\u200b ok
show(currencyFormat('pt-CV', 'CVE', {currencyDisplay: 'name'}), -1234.5);
// CHECK-NEXT: -1234$50 escudos cabo-verdianos minusSign:-|integer:1234|decimal:$|fraction:50|literal: |currency:escudos cabo-verdianos ok
// Compact notation: the separators apply to every magnitude, and the
// currency's pattern to the numbers it does not compact.
show(currencyFormat('pt-CV', 'CVE', {notation: 'compact'}), -1234.5);
// CHECK-NEXT: -1$2\u00a0mil\u00a0\u200b minusSign:-|integer:1|decimal:$|fraction:2|literal:\u00a0|compact:mil|literal:\u00a0|currency:\u200b ok
show(currencyFormat('en-DE', 'EUR', {notation: 'compact'}), 999);
// CHECK-NEXT: €999 currency:€|integer:999 ok
show(currencyFormat('en-DE', 'EUR', {notation: 'compact'}), 1234);
// CHECK-NEXT: €1,2K currency:€|integer:1|decimal:,|fraction:2|compact:K ok
// Every numbering system: the currency's own values are found in the
// chain before root's arab and arabext symbols and patterns, which supply
// only the separators the currency does not have.
show(currencyFormat('en-150', 'EUR', {numberingSystem: 'deva'}), -1234.5);
// CHECK-NEXT: -€१,२३४.५० minusSign:-|currency:€|integer:१|group:,|integer:२३४|decimal:.|fraction:५० ok
show(currencyFormat('en-150', 'EUR', {numberingSystem: 'arab'}), -1234.5);
// CHECK-NEXT: \u061c-€١٬٢٣٤٫٥٠ literal:\u061c|minusSign:-|currency:€|integer:١|group:٬|integer:٢٣٤|decimal:٫|fraction:٥٠ ok
show(currencyFormat('pt-CV', 'CVE', {numberingSystem: 'deva'}), -1234.5);
// CHECK-NEXT: -१२३४$५०\u00a0\u200b minusSign:-|integer:१२३४|decimal:$|fraction:५०|literal:\u00a0|currency:\u200b ok
show(currencyFormat('pt-CV', 'CVE', {numberingSystem: 'arab'}), -1234.5);
// CHECK-NEXT: \u061c-١٢٣٤$٥٠\u00a0\u200b literal:\u061c|minusSign:-|integer:١٢٣٤|decimal:$|fraction:٥٠|literal:\u00a0|currency:\u200b ok
