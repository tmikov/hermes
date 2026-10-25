/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US %s | %FileCheck --match-full-lines %s

// Numbering systems and scripts (plan Review Focus 2): the digits of the
// draft's simple-digit table, including ones ICU4X has no digits for
// (bali) and supplementary ones (mathbold); default numbering systems
// (ar-EG arab, fa arabext); signs with bidi marks; numbering systems a
// locale does not define, resolved by CLDR's inheritance: root's explicit
// arab and arabext data (en-u-nu-arab, fr-u-nu-arabext) and root's aliases
// to the locale's latn data (ar-u-nu-bali, ja-u-nu-deva). formatToParts
// concatenates to format, and no part splits a surrogate pair. The expected
// strings are ICU4C's (Node 24, ICU 78.2).

/// s with every code point outside printable ASCII as \u{X}.
function esc(s) {
  var out = '';
  for (var ch of s) {
    var c = ch.codePointAt(0);
    out += c < 0x20 || c > 0x7e ? '\\u{' + c.toString(16) + '}' : ch;
  }
  return out;
}

/// Whether s has an unpaired surrogate.
function hasLoneSurrogate(s) {
  for (var i = 0; i < s.length; i++) {
    var c = s.charCodeAt(i);
    if (c >= 0xd800 && c <= 0xdbff) {
      var d = s.charCodeAt(i + 1);
      if (!(d >= 0xdc00 && d <= 0xdfff)) return true;
      i++;
    } else if (c >= 0xdc00 && c <= 0xdfff) {
      return true;
    }
  }
  return false;
}

/// The resolved locale and numbering system of a NumberFormat for locale
/// and options, then the format of x, then its parts; and whether the
/// parts concatenate to the format and keep every surrogate pair whole.
function show(locale, options, x) {
  var nf = new Intl.NumberFormat(locale, options);
  var ro = nf.resolvedOptions();
  var s = nf.format(x);
  var parts = nf.formatToParts(x);
  var joined = '';
  var whole = true;
  for (var p of parts) {
    joined += p.value;
    if (hasLoneSurrogate(p.value)) whole = false;
  }
  print(ro.locale, ro.numberingSystem, esc(s));
  print(esc(parts.map(p => p.type + ':' + p.value).join('|')));
  print(joined === s ? 'joined ok' : 'joined differs', whole ? 'pairs ok' : 'pair split');
}

// bali: no ICU4X digits, so the table's.
show('en-u-nu-bali', undefined, -1234567.89);
// CHECK: en-u-nu-bali bali -\u{1b51},\u{1b52}\u{1b53}\u{1b54},\u{1b55}\u{1b56}\u{1b57}.\u{1b58}\u{1b59}
// CHECK-NEXT: minusSign:-|integer:\u{1b51}|group:,|integer:\u{1b52}\u{1b53}\u{1b54}|group:,|integer:\u{1b55}\u{1b56}\u{1b57}|decimal:.|fraction:\u{1b58}\u{1b59}
// CHECK-NEXT: joined ok pairs ok
// mathbold: supplementary digits, from the option. The groups are made
// before transliteration, so they never split a pair.
show('en', {numberingSystem: 'mathbold'}, -1234567.89);
// CHECK-NEXT: en mathbold -\u{1d7cf},\u{1d7d0}\u{1d7d1}\u{1d7d2},\u{1d7d3}\u{1d7d4}\u{1d7d5}.\u{1d7d6}\u{1d7d7}
// CHECK-NEXT: minusSign:-|integer:\u{1d7cf}|group:,|integer:\u{1d7d0}\u{1d7d1}\u{1d7d2}|group:,|integer:\u{1d7d3}\u{1d7d4}\u{1d7d5}|decimal:.|fraction:\u{1d7d6}\u{1d7d7}
// CHECK-NEXT: joined ok pairs ok
// ar-EG: arab by default, its own separators, the sign with U+061C.
show('ar-EG', undefined, -1234567.89);
// CHECK-NEXT: ar-EG arab \u{61c}-\u{661}\u{66c}\u{662}\u{663}\u{664}\u{66c}\u{665}\u{666}\u{667}\u{66b}\u{668}\u{669}
// CHECK-NEXT: literal:\u{61c}|minusSign:-|integer:\u{661}|group:\u{66c}|integer:\u{662}\u{663}\u{664}|group:\u{66c}|integer:\u{665}\u{666}\u{667}|decimal:\u{66b}|fraction:\u{668}\u{669}
// CHECK-NEXT: joined ok pairs ok
show('ar-EG', {style: 'percent'}, -1234567.89);
// CHECK-NEXT: ar-EG arab \u{61c}-\u{661}\u{662}\u{663}\u{66c}\u{664}\u{665}\u{666}\u{66c}\u{667}\u{668}\u{669}\u{66a}\u{61c}
// CHECK-NEXT: literal:\u{61c}|minusSign:-|integer:\u{661}\u{662}\u{663}|group:\u{66c}|integer:\u{664}\u{665}\u{666}|group:\u{66c}|integer:\u{667}\u{668}\u{669}|percentSign:\u{66a}|literal:\u{61c}
// CHECK-NEXT: joined ok pairs ok
// fa: arabext, U+2212 as the minus sign, after U+200E.
show('fa', undefined, -1234567.89);
// CHECK-NEXT: fa arabext \u{200e}\u{2212}\u{6f1}\u{66c}\u{6f2}\u{6f3}\u{6f4}\u{66c}\u{6f5}\u{6f6}\u{6f7}\u{66b}\u{6f8}\u{6f9}
// CHECK-NEXT: literal:\u{200e}|minusSign:\u{2212}|integer:\u{6f1}|group:\u{66c}|integer:\u{6f2}\u{6f3}\u{6f4}|group:\u{66c}|integer:\u{6f5}\u{6f6}\u{6f7}|decimal:\u{66b}|fraction:\u{6f8}\u{6f9}
// CHECK-NEXT: joined ok pairs ok
show('fa', {style: 'percent'}, -1234567.89);
// CHECK-NEXT: fa arabext \u{200e}\u{2212}\u{6f1}\u{6f2}\u{6f3}\u{66c}\u{6f4}\u{6f5}\u{6f6}\u{66c}\u{6f7}\u{6f8}\u{6f9}\u{66a}
// CHECK-NEXT: literal:\u{200e}|minusSign:\u{2212}|integer:\u{6f1}\u{6f2}\u{6f3}|group:\u{66c}|integer:\u{6f4}\u{6f5}\u{6f6}|group:\u{66c}|integer:\u{6f7}\u{6f8}\u{6f9}|percentSign:\u{66a}
// CHECK-NEXT: joined ok pairs ok
// hi-u-nu-deva: deva digits, with hi's three-then-two grouping.
show('hi-u-nu-deva', undefined, -1234567.89);
// CHECK-NEXT: hi-u-nu-deva deva -\u{967}\u{968},\u{969}\u{96a},\u{96b}\u{96c}\u{96d}.\u{96e}\u{96f}
// CHECK-NEXT: minusSign:-|integer:\u{967}\u{968}|group:,|integer:\u{969}\u{96a}|group:,|integer:\u{96b}\u{96c}\u{96d}|decimal:.|fraction:\u{96e}\u{96f}
// CHECK-NEXT: joined ok pairs ok
// ar-u-nu-bali: ar has no bali symbols, and root aliases bali's to ar's latn
// ones.
show('ar-u-nu-bali', undefined, -1234567.89);
// CHECK-NEXT: ar-u-nu-bali bali \u{200e}-\u{1b51},\u{1b52}\u{1b53}\u{1b54},\u{1b55}\u{1b56}\u{1b57}.\u{1b58}\u{1b59}
// CHECK-NEXT: literal:\u{200e}|minusSign:-|integer:\u{1b51}|group:,|integer:\u{1b52}\u{1b53}\u{1b54}|group:,|integer:\u{1b55}\u{1b56}\u{1b57}|decimal:.|fraction:\u{1b58}\u{1b59}
// CHECK-NEXT: joined ok pairs ok
// ar: latn by default.
show('ar', undefined, -1234567.89);
// CHECK-NEXT: ar latn \u{200e}-1,234,567.89
// CHECK-NEXT: literal:\u{200e}|minusSign:-|integer:1|group:,|integer:234|group:,|integer:567|decimal:.|fraction:89
// CHECK-NEXT: joined ok pairs ok
// hanidec: the one row of the table that is not a range.
show('en', {numberingSystem: 'hanidec'}, -1234567.89);
// CHECK-NEXT: en hanidec -\u{4e00},\u{4e8c}\u{4e09}\u{56db},\u{4e94}\u{516d}\u{4e03}.\u{516b}\u{4e5d}
// CHECK-NEXT: minusSign:-|integer:\u{4e00}|group:,|integer:\u{4e8c}\u{4e09}\u{56db}|group:,|integer:\u{4e94}\u{516d}\u{4e03}|decimal:.|fraction:\u{516b}\u{4e5d}
// CHECK-NEXT: joined ok pairs ok
// en-u-nu-arab: en defines no arab data, so root's explicit arab symbols,
// with en's latn grouping (root aliases decimalFormats to latn).
show('en-u-nu-arab', undefined, -1234.5);
// CHECK-NEXT: en-u-nu-arab arab \u{61c}-\u{661}\u{66c}\u{662}\u{663}\u{664}\u{66b}\u{665}
// CHECK-NEXT: literal:\u{61c}|minusSign:-|integer:\u{661}|group:\u{66c}|integer:\u{662}\u{663}\u{664}|decimal:\u{66b}|fraction:\u{665}
// CHECK-NEXT: joined ok pairs ok
show('en-u-nu-arab', {signDisplay: 'always'}, 5);
// CHECK-NEXT: en-u-nu-arab arab \u{61c}+\u{665}
// CHECK-NEXT: literal:\u{61c}|plusSign:+|integer:\u{665}
// CHECK-NEXT: joined ok pairs ok
// Root's arab percent pattern, and its currency pattern for both signs.
show('en-u-nu-arab', {style: 'percent'}, 0.5);
// CHECK-NEXT: en-u-nu-arab arab \u{665}\u{660}\u{66a}\u{61c}
// CHECK-NEXT: integer:\u{665}\u{660}|percentSign:\u{66a}|literal:\u{61c}
// CHECK-NEXT: joined ok pairs ok
var usd = {style: 'currency', currency: 'USD'};
show('en-u-nu-arab', usd, -1234.5);
// CHECK-NEXT: en-u-nu-arab arab \u{61c}-\u{661}\u{66c}\u{662}\u{663}\u{664}\u{66b}\u{665}\u{660}\u{a0}$
// CHECK-NEXT: literal:\u{61c}|minusSign:-|integer:\u{661}|group:\u{66c}|integer:\u{662}\u{663}\u{664}|decimal:\u{66b}|fraction:\u{665}\u{660}|literal:\u{a0}|currency:$
// CHECK-NEXT: joined ok pairs ok
show('en-u-nu-arab', {...usd, currencySign: 'accounting'}, -1234.5);
// CHECK-NEXT: en-u-nu-arab arab \u{61c}-\u{661}\u{66c}\u{662}\u{663}\u{664}\u{66b}\u{665}\u{660}\u{a0}$
// CHECK-NEXT: literal:\u{61c}|minusSign:-|integer:\u{661}|group:\u{66c}|integer:\u{662}\u{663}\u{664}|decimal:\u{66b}|fraction:\u{665}\u{660}|literal:\u{a0}|currency:$
// CHECK-NEXT: joined ok pairs ok
show('en-u-nu-arab', {...usd, currencyDisplay: 'code'}, 1);
// CHECK-NEXT: en-u-nu-arab arab \u{661}\u{66b}\u{660}\u{660}\u{a0}USD
// CHECK-NEXT: integer:\u{661}|decimal:\u{66b}|fraction:\u{660}\u{660}|literal:\u{a0}|currency:USD
// CHECK-NEXT: joined ok pairs ok
// Compact patterns alias to en's latn ones (so does the range pattern, which
// waits for formatRange: ICU4C gives 3 to 5 as "\u{663}\u{2013}\u{665}").
show('en-u-nu-arab', {notation: 'compact'}, 1234567);
// CHECK-NEXT: en-u-nu-arab arab \u{661}\u{66b}\u{662}M
// CHECK-NEXT: integer:\u{661}|decimal:\u{66b}|fraction:\u{662}|compact:M
// CHECK-NEXT: joined ok pairs ok
// de-u-nu-arab: root's percent pattern, not de's "#,##0 %".
show('de-u-nu-arab', undefined, -1234.5);
// CHECK-NEXT: de-u-nu-arab arab \u{61c}-\u{661}\u{66c}\u{662}\u{663}\u{664}\u{66b}\u{665}
// CHECK-NEXT: literal:\u{61c}|minusSign:-|integer:\u{661}|group:\u{66c}|integer:\u{662}\u{663}\u{664}|decimal:\u{66b}|fraction:\u{665}
// CHECK-NEXT: joined ok pairs ok
show('de-u-nu-arab', {style: 'percent'}, 0.5);
// CHECK-NEXT: de-u-nu-arab arab \u{665}\u{660}\u{66a}\u{61c}
// CHECK-NEXT: integer:\u{665}\u{660}|percentSign:\u{66a}|literal:\u{61c}
// CHECK-NEXT: joined ok pairs ok
// fr-u-nu-arabext: root's arabext symbols; root aliases arabext's percent
// and currency patterns to fr's latn ones.
show('fr-u-nu-arabext', undefined, -1234.5);
// CHECK-NEXT: fr-u-nu-arabext arabext \u{200e}-\u{200e}\u{6f1}\u{66c}\u{6f2}\u{6f3}\u{6f4}\u{66b}\u{6f5}
// CHECK-NEXT: literal:\u{200e}|minusSign:-|literal:\u{200e}|integer:\u{6f1}|group:\u{66c}|integer:\u{6f2}\u{6f3}\u{6f4}|decimal:\u{66b}|fraction:\u{6f5}
// CHECK-NEXT: joined ok pairs ok
show('fr-u-nu-arabext', {style: 'percent'}, 0.5);
// CHECK-NEXT: fr-u-nu-arabext arabext \u{6f5}\u{6f0}\u{a0}\u{66a}
// CHECK-NEXT: integer:\u{6f5}\u{6f0}|literal:\u{a0}|percentSign:\u{66a}
// CHECK-NEXT: joined ok pairs ok
show('fr-u-nu-arabext', {...usd, currencySign: 'accounting'}, -1234.5);
// CHECK-NEXT: fr-u-nu-arabext arabext (\u{6f1}\u{66c}\u{6f2}\u{6f3}\u{6f4}\u{66b}\u{6f5}\u{6f0}\u{a0}$US)
// CHECK-NEXT: literal:(|integer:\u{6f1}|group:\u{66c}|integer:\u{6f2}\u{6f3}\u{6f4}|decimal:\u{66b}|fraction:\u{6f5}\u{6f0}|literal:\u{a0}|currency:$US|literal:)
// CHECK-NEXT: joined ok pairs ok
// Root aliases deva and beng to the locale's latn data.
show('ja-u-nu-deva', undefined, -1234.5);
// CHECK-NEXT: ja-u-nu-deva deva -\u{967},\u{968}\u{969}\u{96a}.\u{96b}
// CHECK-NEXT: minusSign:-|integer:\u{967}|group:,|integer:\u{968}\u{969}\u{96a}|decimal:.|fraction:\u{96b}
// CHECK-NEXT: joined ok pairs ok
show('ja-u-nu-deva', {style: 'percent'}, 0.5);
// CHECK-NEXT: ja-u-nu-deva deva \u{96b}\u{966}%
// CHECK-NEXT: integer:\u{96b}\u{966}|percentSign:%
// CHECK-NEXT: joined ok pairs ok
show('en-u-nu-beng', undefined, -1234.5);
// CHECK-NEXT: en-u-nu-beng beng -\u{9e7},\u{9e8}\u{9e9}\u{9ea}.\u{9eb}
// CHECK-NEXT: minusSign:-|integer:\u{9e7}|group:,|integer:\u{9e8}\u{9e9}\u{9ea}|decimal:.|fraction:\u{9eb}
// CHECK-NEXT: joined ok pairs ok
show('en-u-nu-beng', {style: 'percent'}, 0.5);
// CHECK-NEXT: en-u-nu-beng beng \u{9eb}\u{9e6}%
// CHECK-NEXT: integer:\u{9eb}\u{9e6}|percentSign:%
// CHECK-NEXT: joined ok pairs ok
// Compact notation's currency patterns are their own kind of data, which
// root does not define for arab or arabext: the locale's latn patterns, with
// the numbering system's signs and separators.
var eur = {style: 'currency', currency: 'EUR'};
show('en-u-nu-arab', {...usd, notation: 'compact'}, -1234567);
// CHECK-NEXT: en-u-nu-arab arab \u{61c}-$\u{661}\u{66b}\u{662}M
// CHECK-NEXT: literal:\u{61c}|minusSign:-|currency:$|integer:\u{661}|decimal:\u{66b}|fraction:\u{662}|compact:M
// CHECK-NEXT: joined ok pairs ok
show('de-u-nu-arab', {...eur, notation: 'compact'}, -1234567);
// CHECK-NEXT: de-u-nu-arab arab \u{61c}-\u{661}\u{66b}\u{662}\u{a0}Mio.\u{a0}\u{20ac}
// CHECK-NEXT: literal:\u{61c}|minusSign:-|integer:\u{661}|decimal:\u{66b}|fraction:\u{662}|literal:\u{a0}|compact:Mio.|literal:\u{a0}|currency:\u{20ac}
// CHECK-NEXT: joined ok pairs ok
show('fr-u-nu-arabext', {...eur, notation: 'compact'}, -1234567);
// CHECK-NEXT: fr-u-nu-arabext arabext \u{200e}-\u{200e}\u{6f1}\u{66b}\u{6f2}\u{a0}M\u{a0}\u{20ac}
// CHECK-NEXT: literal:\u{200e}|minusSign:-|literal:\u{200e}|integer:\u{6f1}|decimal:\u{66b}|fraction:\u{6f2}|literal:\u{a0}|compact:M|literal:\u{a0}|currency:\u{20ac}
// CHECK-NEXT: joined ok pairs ok
// Locales' own data for numbering systems outside their own (CLDR's XML):
// fr's arab plus sign, ko's arab signs, ur's arab separators, nl's arab
// percent pattern, zh's arab currency pattern, fa's arab NaN and ar's arabext
// NaN.
show('fr-u-nu-arab', {signDisplay: 'always'}, 5);
// CHECK-NEXT: fr-u-nu-arab arab \u{200f}+\u{665}
// CHECK-NEXT: literal:\u{200f}|plusSign:+|integer:\u{665}
// CHECK-NEXT: joined ok pairs ok
show('ko-u-nu-arab', undefined, -1234.5);
// CHECK-NEXT: ko-u-nu-arab arab \u{200f}-\u{661}\u{66c}\u{662}\u{663}\u{664}\u{66b}\u{665}
// CHECK-NEXT: literal:\u{200f}|minusSign:-|integer:\u{661}|group:\u{66c}|integer:\u{662}\u{663}\u{664}|decimal:\u{66b}|fraction:\u{665}
// CHECK-NEXT: joined ok pairs ok
show('ur-u-nu-arab', undefined, -1234.5);
// CHECK-NEXT: ur-u-nu-arab arab \u{61c}-\u{661},\u{662}\u{663}\u{664}\u{60c}\u{665}
// CHECK-NEXT: literal:\u{61c}|minusSign:-|integer:\u{661}|group:,|integer:\u{662}\u{663}\u{664}|decimal:\u{60c}|fraction:\u{665}
// CHECK-NEXT: joined ok pairs ok
show('nl-u-nu-arab', {style: 'percent'}, 0.5);
// CHECK-NEXT: nl-u-nu-arab arab \u{665}\u{660}\u{a0}\u{66a}\u{61c}
// CHECK-NEXT: integer:\u{665}\u{660}|literal:\u{a0}|percentSign:\u{66a}|literal:\u{61c}
// CHECK-NEXT: joined ok pairs ok
show('zh-u-nu-arab', usd, -1234.5);
// CHECK-NEXT: zh-u-nu-arab arab \u{61c}-US$\u{661}\u{66c}\u{662}\u{663}\u{664}\u{66b}\u{665}\u{660}
// CHECK-NEXT: literal:\u{61c}|minusSign:-|currency:US$|integer:\u{661}|group:\u{66c}|integer:\u{662}\u{663}\u{664}|decimal:\u{66b}|fraction:\u{665}\u{660}
// CHECK-NEXT: joined ok pairs ok
show('fa-u-nu-arab', undefined, NaN);
// CHECK-NEXT: fa-u-nu-arab arab \u{646}\u{627}\u{639}\u{62f}\u{62f}
// CHECK-NEXT: nan:\u{646}\u{627}\u{639}\u{62f}\u{62f}
// CHECK-NEXT: joined ok pairs ok
show('ar-EG-u-nu-arabext', undefined, NaN);
// CHECK-NEXT: ar-EG-u-nu-arabext arabext \u{644}\u{64a}\u{633}\u{a0}\u{631}\u{642}\u{645}\u{64b}\u{627}
// CHECK-NEXT: nan:\u{644}\u{64a}\u{633}\u{a0}\u{631}\u{642}\u{645}\u{64b}\u{627}
// CHECK-NEXT: joined ok pairs ok
// The option wins over the keyword, which is then dropped from the locale;
// an unsupported option value is ignored and the keyword kept.
var ro = new Intl.NumberFormat('en-u-nu-thai', {numberingSystem: 'deva'}).resolvedOptions();
print(ro.locale, ro.numberingSystem);
// CHECK-NEXT: en deva
ro = new Intl.NumberFormat('en-u-nu-thai', {numberingSystem: 'xxxx'}).resolvedOptions();
print(ro.locale, ro.numberingSystem);
// CHECK-NEXT: en-u-nu-thai thai
// An unsupported keyword value is ignored.
ro = new Intl.NumberFormat('en-u-nu-xxxx').resolvedOptions();
print(ro.locale, ro.numberingSystem);
// CHECK-NEXT: en latn

// Every numbering system Intl.supportedValuesOf lists is resolved, and
// formats with ten digits of its own, one code point per digit: ASCII
// digits only for latn, and ten different digits.
var bad = [];
var systems = Intl.supportedValuesOf('numberingSystem');
for (var nu of systems) {
  var nf = new Intl.NumberFormat('en', {numberingSystem: nu, useGrouping: false});
  var digits = nf.format(1234567890);
  var cps = Array.from(digits);
  var ascii = cps.some(c => c >= '0' && c <= '9');
  if (
    nf.resolvedOptions().numberingSystem !== nu ||
    cps.length !== 10 ||
    new Set(cps).size !== 10 ||
    ascii !== (nu === 'latn') ||
    hasLoneSurrogate(digits)
  )
    bad.push(nu);
}
print(systems.length > 70, bad.length === 0 ? 'all ok' : 'bad: ' + bad.join(' '));
// CHECK-NEXT: true all ok
