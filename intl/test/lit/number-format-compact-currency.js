/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: cat %S/Inputs/escape.js %s > %t.js
// RUN: %hermes -Xintl-default-locale=en-US %t.js | %FileCheck --match-full-lines %s

// Compact notation with the currency style: CLDR's short currency patterns
// (currencyFormatLength short, compact-currency.txt), with their own
// affixes, spacing, magnitudes and digits, their alphaNextToNumber
// alternates, and the pattern "0" for a magnitude that is not compacted.
// The expected strings are ICU4C's (Node 24, ICU 78.2, CLDR 48.0) except
// where a comment says otherwise.

/// A compact currency formatter of locale for currency, with the other
/// options of extra.
function nf(locale, currency, extra) {
  return new Intl.NumberFormat(
    locale,
    Object.assign(
      {style: 'currency', currency: currency, notation: 'compact'},
      extra,
    ),
  );
}

/// nf.format(x), escaped, then the parts of nf.formatToParts(x) as
/// type:value separated by "|", escaped, and whether the parts' values
/// concatenate to the format result.
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

// fa's short pattern has a space after the currency that its ordinary
// arabext pattern lacks ("\u200e¤#,##0.00").
show(nf('fa', 'USD'), 1000);
// CHECK: \u200e$\u00a0۱\u00a0هزار literal:\u200e|currency:$|literal:\u00a0|integer:۱|literal:\u00a0|compact:هزار ok
show(nf('fa', 'USD'), -1234567);
// CHECK-NEXT: \u200e−\u200e$\u00a0۱٫۲\u00a0میلیون literal:\u200e|minusSign:−|literal:\u200e|currency:$|literal:\u00a0|integer:۱|decimal:٫|fraction:۲|literal:\u00a0|compact:میلیون ok
print(escape(new Intl.NumberFormat('fa', {style: 'currency', currency: 'USD'}).format(1000)));
// CHECK-NEXT: \u200e$۱٬۰۰۰٫۰۰

// The alphaNextToNumber alternate where the currency string ends in a
// letter next to the digits; a symbol does not take it.
show(nf('en', 'USD'), 1234);
// CHECK-NEXT: $1.2K currency:$|integer:1|decimal:.|fraction:2|compact:K ok
show(nf('en', 'CHF'), 1234);
// CHECK-NEXT: CHF\u00a01.2K currency:CHF|literal:\u00a0|integer:1|decimal:.|fraction:2|compact:K ok
show(nf('en', 'USD', {currencyDisplay: 'code'}), 1234);
// CHECK-NEXT: USD\u00a01.2K currency:USD|literal:\u00a0|integer:1|decimal:.|fraction:2|compact:K ok
show(nf('en', 'USD', {currencyDisplay: 'narrowSymbol'}), 1234);
// CHECK-NEXT: $1.2K currency:$|integer:1|decimal:.|fraction:2|compact:K ok
show(nf('ja', 'USD', {currencyDisplay: 'code'}), 12345);
// CHECK-NEXT: USD\u00a01.2万 currency:USD|literal:\u00a0|integer:1|decimal:.|fraction:2|compact:万 ok
// The name display spells the number out with the decimal compact
// patterns.
show(nf('en', 'EUR', {currencyDisplay: 'name'}), 1234);
// CHECK-NEXT: 1.2K euros integer:1|decimal:.|fraction:2|compact:K|literal: |currency:euros ok

// Below the first compact magnitude, zero, NaN and the infinities: the
// ordinary currency patterns, without compaction. 999.9 rounds up into
// the 10^3 entry.
show(nf('en', 'USD'), 999);
// CHECK-NEXT: $999 currency:$|integer:999 ok
show(nf('en', 'USD'), 999.9);
// CHECK-NEXT: $1K currency:$|integer:1|compact:K ok
show(nf('en', 'USD'), 0);
// CHECK-NEXT: $0 currency:$|integer:0 ok
show(nf('en', 'USD'), -0);
// CHECK-NEXT: -$0 minusSign:-|currency:$|integer:0 ok
show(nf('en', 'USD'), NaN);
// CHECK-NEXT: $NaN currency:$|nan:NaN ok
show(nf('en', 'USD'), -Infinity);
// CHECK-NEXT: -$∞ minusSign:-|currency:$|infinity:∞ ok
show(nf('en', 'USD', {currencyDisplay: 'code'}), Infinity);
// CHECK-NEXT: USD∞ currency:USD|infinity:∞ ok
// Past the largest magnitude, the largest entry's exponent.
show(nf('en', 'USD'), 1e15);
// CHECK-NEXT: $1000T currency:$|integer:1000|compact:T ok
show(nf('en', 'USD', {maximumFractionDigits: 2}), 1234.5);
// CHECK-NEXT: $1.23K currency:$|integer:1|decimal:.|fraction:23|compact:K ok
show(nf('en-IN', 'INR'), 1234567);
// CHECK-NEXT: ₹12L currency:₹|integer:12|compact:L ok

// The pattern "0": de and ja compact no currency amount below 10^6 and
// 10^4. ja's patterns are by four digits.
show(nf('de', 'EUR'), 1234);
// CHECK-NEXT: 1234\u00a0€ integer:1234|literal:\u00a0|currency:€ ok
show(nf('de', 'EUR'), -1234567);
// CHECK-NEXT: -1,2\u00a0Mio.\u00a0€ minusSign:-|integer:1|decimal:,|fraction:2|literal:\u00a0|compact:Mio.|literal:\u00a0|currency:€ ok
show(nf('ja', 'JPY'), 1234);
// CHECK-NEXT: ￥1234 currency:￥|integer:1234 ok
show(nf('ja', 'JPY'), 12345);
// CHECK-NEXT: ￥1.2万 currency:￥|integer:1|decimal:.|fraction:2|compact:万 ok
// vec's 10^3 "one" is "0" where "other" is compact. Where the plural
// category of the compacted number picks "0", the number is not compacted
// (UTS 35, Compact Number Formats: "0" means the non-compact format): 1000
// is the unscaled 1000, rounded as compact notation rounds a number it
// does not compact. ICU4C keeps the scaled number ("1\u202fEUR"). Values
// whose compacted number is "other" are compacted.
show(nf('vec', 'EUR'), 1000);
// CHECK-NEXT: 1000\u202fEUR integer:1000|literal:\u202f|currency:EUR ok
show(nf('vec', 'EUR'), 1000.4);
// CHECK-NEXT: 1000\u202fEUR integer:1000|literal:\u202f|currency:EUR ok
show(nf('vec', 'EUR'), -1000);
// CHECK-NEXT: -1000\u202fEUR minusSign:-|integer:1000|literal:\u202f|currency:EUR ok
show(nf('vec', 'EUR'), 1400);
// CHECK-NEXT: 1,4\u00a0mila\u202fEUR integer:1|decimal:,|fraction:4|literal:\u00a0|compact:mila|literal:\u202f|currency:EUR ok
show(nf('vec', 'EUR'), 2000);
// CHECK-NEXT: 2\u00a0mila\u202fEUR integer:2|literal:\u00a0|compact:mila|literal:\u202f|currency:EUR ok
print(escape(nf('vec', 'EUR').formatRange(1000, 2000)));
// CHECK-NEXT: 1000\u202fEUR - 2\u00a0mila\u202fEUR
// The decimal compact patterns have the same "0" (vec only, in CLDR 48.2).
var vecCompact = new Intl.NumberFormat('vec', {notation: 'compact'});
show(vecCompact, 1000);
// CHECK-NEXT: 1000 integer:1000 ok
show(vecCompact, 1100);
// CHECK-NEXT: 1,1\u00a0mila integer:1|decimal:,|fraction:1|literal:\u00a0|compact:mila ok
print(escape(vecCompact.formatRange(1000, 1100)));
// CHECK-NEXT: 1000-1,1\u00a0mila
// PluralRules takes the exponent of the number shown: 1000 is "other"
// with c = 0, where the compacted 1 thousand would be "one".
var vecRules = new Intl.PluralRules('vec', {notation: 'compact'});
print(vecRules.select(1000), vecRules.select(1100), vecRules.select(1), new Intl.PluralRules('vec').select(1));
// CHECK-NEXT: other other one one

// Signs: CLDR's minus sign goes where its pattern puts it, before the
// whole pattern and outside the bidi marks (ar), and outside the compact
// text. ICU4C puts sw's sign inside it ("US$\u00a0elfu\u00a0-1.2"), from
// sw's explicit negative pattern, which the plugin does not use (the sign
// goes outside the compact pattern, as for decimals).
show(nf('ar', 'USD'), -1234);
// CHECK-NEXT: \u200e-\u200f1.2\u00a0ألف\u00a0US$ literal:\u200e|minusSign:-|literal:\u200f|integer:1|decimal:.|fraction:2|literal:\u00a0|compact:ألف|literal:\u00a0|currency:US$ ok
show(nf('ar', 'USD'), 3000);
// CHECK-NEXT: \u200f3\u00a0آلاف\u00a0US$ literal:\u200f|integer:3|literal:\u00a0|compact:آلاف|literal:\u00a0|currency:US$ ok
show(nf('sw', 'USD'), -1234);
// CHECK-NEXT: -US$\u00a0elfu\u00a01.2 minusSign:-|currency:US$|literal:\u00a0|compact:elfu|literal:\u00a0|integer:1|decimal:.|fraction:2 ok
show(nf('en', 'USD', {signDisplay: 'always'}), 1234);
// CHECK-NEXT: +$1.2K plusSign:+|currency:$|integer:1|decimal:.|fraction:2|compact:K ok
show(nf('en', 'USD', {signDisplay: 'exceptZero'}), -1234);
// CHECK-NEXT: -$1.2K minusSign:-|currency:$|integer:1|decimal:.|fraction:2|compact:K ok
show(nf('en', 'USD', {signDisplay: 'never'}), -1234);
// CHECK-NEXT: $1.2K currency:$|integer:1|decimal:.|fraction:2|compact:K ok

// Accounting: the ordinary accounting negative pattern around the short
// pattern's currency and number. ICU4C ignores accounting in compact
// notation ("-$1.2K").
show(nf('en', 'USD', {currencySign: 'accounting'}), -1234);
// CHECK-NEXT: ($1.2K) literal:(|currency:$|integer:1|decimal:.|fraction:2|compact:K|literal:) ok
show(nf('en', 'CHF', {currencySign: 'accounting'}), -1234);
// CHECK-NEXT: (CHF\u00a01.2K) literal:(|currency:CHF|literal:\u00a0|integer:1|decimal:.|fraction:2|compact:K|literal:) ok
show(nf('fr', 'EUR', {currencySign: 'accounting'}), -1234);
// CHECK-NEXT: (1,2\u00a0k\u00a0€) literal:(|integer:1|decimal:,|fraction:2|literal:\u00a0|compact:k|literal:\u00a0|currency:€|literal:) ok
show(nf('fa', 'USD', {currencySign: 'accounting'}), -1234);
// CHECK-NEXT: \u200e($\u00a0۱٫۲\u00a0هزار) literal:\u200e(|currency:$|literal:\u00a0|integer:۱|decimal:٫|fraction:۲|literal:\u00a0|compact:هزار|literal:) ok
show(nf('en', 'USD', {currencySign: 'accounting', signDisplay: 'always'}), 1234);
// CHECK-NEXT: +$1.2K plusSign:+|currency:$|integer:1|decimal:.|fraction:2|compact:K ok
// de's accounting pattern is its standard one, with a minus sign.
show(nf('de', 'EUR', {currencySign: 'accounting'}), -1234567);
// CHECK-NEXT: -1,2\u00a0Mio.\u00a0€ minusSign:-|integer:1|decimal:,|fraction:2|literal:\u00a0|compact:Mio.|literal:\u00a0|currency:€ ok

// Other numbering systems. Below the first compact magnitude en-u-nu-arab
// has root's arab currency pattern, as ICU4C gives; its short patterns are
// en's latn ones, since root has no short ones for arab. ba-u-nu-deva has
// ba's.
show(nf('en-u-nu-arab', 'USD'), 999);
// CHECK-NEXT: ٩٩٩\u00a0$ integer:٩٩٩|literal:\u00a0|currency:$ ok
show(nf('en-u-nu-arab', 'USD'), -1234567);
// CHECK-NEXT: \u061c-$١٫٢M literal:\u061c|minusSign:-|currency:$|integer:١|decimal:٫|fraction:٢|compact:M ok
show(nf('ba-u-nu-deva', 'USD'), 1234);
// CHECK-NEXT: १.२\u00a0мең\u00a0$ integer:१|decimal:.|fraction:२|literal:\u00a0|compact:мең|literal:\u00a0|currency:$ ok
// kok's 10^14 alternate: latn's has the pattern's digits. kok.xml gives
// thai (and deva) the alternate "¤000LCr", whose three digits make the
// exponent 12; ICU4C, which does not use the alternate, gives
// "USD\u00a0๑.๒हज.निख.".
show(nf('kok', 'USD', {currencyDisplay: 'code'}), 123e12);
// CHECK-NEXT: USD\u00a01.2हज.निख. currency:USD|literal:\u00a0|integer:1|decimal:.|fraction:2|compact:हज.निख. ok
show(nf('kok-u-nu-thai', 'USD', {currencyDisplay: 'code'}), 123e12);
// CHECK-NEXT: USD\u00a0๑๒๓LCr currency:USD|literal:\u00a0|integer:๑๒๓|compact:LCr ok

// Ranges collapse the shared currency and compact text.
print(escape(nf('en', 'USD').formatRange(3000, 5000)));
// CHECK-NEXT: $3–5K
print(escape(nf('en', 'USD').formatRange(3000, 5000000)));
// CHECK-NEXT: $3K – $5M
print(escape(nf('en', 'CHF').formatRange(3000, 5000)));
// CHECK-NEXT: CHF\u00a03–5K
print(escape(nf('fa', 'USD').formatRange(3000, 5000)));
// CHECK-NEXT: \u200e$\u00a0۳–۵\u00a0هزار
print(escape(nf('fr', 'EUR').formatRange(3000, 5000)));
// CHECK-NEXT: 3–5\u00a0k\u00a0€
print(escape(nf('de', 'EUR').formatRange(3000, 5000)));
// CHECK-NEXT: 3000–5000\u00a0€
print(escape(nf('en', 'USD').formatRange(999, 5000)));
// CHECK-NEXT: $999 – $5K
// The plugin's accounting form (ICU4C: "-$5–3K").
print(escape(nf('en', 'USD', {currencySign: 'accounting'}).formatRange(-5000, -3000)));
// CHECK-NEXT: ($5–3K)

// Every locale, at every compact magnitude: one currency part, and no two
// spaces in a row.
var bad = [];
var locales = Intl.NumberFormat.supportedLocalesOf(
  ['af', 'ar', 'bn', 'de', 'en', 'es', 'fa', 'fr', 'hi', 'ja', 'kok', 'ru', 'sw', 'vec', 'zh'],
);
for (var i = 0; i < locales.length; i++) {
  for (var display of ['symbol', 'code', 'narrowSymbol']) {
    var f = nf(locales[i], 'CHF', {currencyDisplay: display});
    for (var x = 1234; x < 1e16; x *= 10) {
      var parts = f.formatToParts(-x);
      var s = f.format(-x);
      var currencies = parts.filter(p => p.type === 'currency').length;
      var spaced = false;
      for (var j = 1; j < s.length; j++) {
        var a = s.charCodeAt(j - 1), b = s.charCodeAt(j);
        var space = c => c === 0x20 || c === 0xa0 || c === 0x202f;
        if (space(a) && space(b)) spaced = true;
      }
      if (currencies !== 1 || spaced) bad.push(locales[i] + ' ' + display + ' ' + escape(s));
    }
  }
}
print(locales.length, bad.length === 0 ? 'ok' : bad.join('; '));
// CHECK-NEXT: 15 ok
