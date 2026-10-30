/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: cat %S/Inputs/escape.js %s > %t.js
// RUN: %hermes -Xintl-default-locale=en-US -Xintl-default-time-zone=UTC %t.js | %FileCheck --match-full-lines %s

// Intl.DateTimeFormat's numbers in other numbering systems: every number
// goes through an internal NumberFormat of [[NumberingSystem]] (plan
// decision 12), and the ASCII digits of a time zone name are written in
// its digits (decision 4). The expected strings are Node 24's (ICU 78.2),
// except that Node's format writes U+0020 for CLDR's U+202F before AM and
// PM, and that ar's zone names keep ICU4X's U+200E before the sign (the
// Task 4 ruling; ICU4C has none).

var t = Date.UTC(2020, 5, 1, 12);

/// The format and the formatToParts parts of x, escaped.
function show(locales, options, x = t) {
  var dtf = new Intl.DateTimeFormat(locales, options);
  var s = dtf.format(x);
  var parts = dtf.formatToParts(x);
  print(
    escape(s) +
      ' || ' +
      escape(parts.map(p => p.type + ':' + p.value).join('|')) +
      (parts.map(p => p.value).join('') === s ? '' : ' (parts do not join)'),
  );
}

show('en-u-nu-arab', {timeZoneName: 'short', timeZone: 'Asia/Kolkata'});
// CHECK: ٦/١/٢٠٢٠, GMT+٥:٣٠ || month:٦|literal:/|day:١|literal:/|year:٢٠٢٠|literal:, |timeZoneName:GMT+٥:٣٠
show('en-u-nu-thai', {hour: 'numeric', timeZoneName: 'shortOffset', timeZone: 'Asia/Kolkata'});
// CHECK-NEXT: ๕\u202fPM GMT+๕:๓๐ || hour:๕|literal:\u202f|dayPeriod:PM|literal: |timeZoneName:GMT+๕:๓๐
show('en', {numberingSystem: 'arab', timeZone: '+05:30', timeZoneName: 'longOffset'});
// CHECK-NEXT: ٦/١/٢٠٢٠, GMT+٠٥:٣٠ || month:٦|literal:/|day:١|literal:/|year:٢٠٢٠|literal:, |timeZoneName:GMT+٠٥:٣٠
show('ar-EG', {timeZoneName: 'short', timeZone: 'Asia/Kolkata'});
// CHECK-NEXT: ١\u200f/٦\u200f/٢٠٢٠، غرينتش\u200e+٥:٣٠ || day:١|literal:\u200f/|month:٦|literal:\u200f/|year:٢٠٢٠|literal:، |timeZoneName:غرينتش\u200e+٥:٣٠
show('ar-EG', {timeStyle: 'full', timeZone: 'Asia/Kolkata'});
// CHECK-NEXT: ٥:٣٠:٠٠ م توقيت الهند || hour:٥|literal::|minute:٣٠|literal::|second:٠٠|literal: |dayPeriod:م|literal: |timeZoneName:توقيت الهند

// Digits outside the BMP, and a locale whose default numbering system is
// not latn.
show('en-u-nu-mathbold', {});
// CHECK-NEXT: 𝟔/𝟏/𝟐𝟎𝟐𝟎 || month:𝟔|literal:/|day:𝟏|literal:/|year:𝟐𝟎𝟐𝟎
show('fa', {});
// CHECK-NEXT: ۱۳۹۹/۳/۱۲ || year:۱۳۹۹|literal:/|month:۳|literal:/|day:۱۲

// 2-digit fields keep the last two code points, not code units: each
// mathbold digit is two code units. CLDR 49's en "yy" is "’yy", so the
// year is not the first part.
show('en-u-nu-mathbold', {year: '2-digit', month: '2-digit', day: '2-digit'});
// CHECK-NEXT: 𝟎𝟔/𝟎𝟏/𝟐𝟎 || month:𝟎𝟔|literal:/|day:𝟎𝟏|literal:/|year:𝟐𝟎
var twoDigits = new Intl.DateTimeFormat('en-u-nu-mathbold', {year: '2-digit'})
  .formatToParts(t)
  .filter(function (p) { return p.type === 'year'; })[0].value;
print(twoDigits.length, twoDigits.codePointAt(0).toString(16), twoDigits.codePointAt(2).toString(16));
// CHECK-NEXT: 4 1d7d0 1d7ce
show('en-u-nu-mathbold', {hour: '2-digit', minute: '2-digit', second: '2-digit', fractionalSecondDigits: 3}, t + 45045);
// CHECK-NEXT: 𝟏𝟐:𝟎𝟎:𝟒𝟓.𝟎𝟒𝟓\u202fPM || hour:𝟏𝟐|literal::|minute:𝟎𝟎|literal::|second:𝟒𝟓|literal:.|fractionalSecond:𝟎𝟒𝟓|literal:\u202f|dayPeriod:PM
show('en-u-nu-adlm', {second: 'numeric', fractionalSecondDigits: 2}, t + 3456);
// CHECK-NEXT: 𞥓.𞥔𞥕 || second:𞥓|literal:.|fractionalSecond:𞥔𞥕

// Algorithmic-looking systems with simple digit mappings, and Devanagari.
show('en-u-nu-hanidec', {});
// CHECK-NEXT: 六/一/二〇二〇 || month:六|literal:/|day:一|literal:/|year:二〇二〇
show('zh-u-nu-hanidec', {dateStyle: 'full'});
// CHECK-NEXT: 二〇二〇年六月一日星期一 || year:二〇二〇|literal:年|month:六|literal:月|day:一|literal:日|weekday:星期一
show('en-u-nu-deva', {dateStyle: 'medium', timeStyle: 'medium'});
// CHECK-NEXT: Jun १, २०२०, १२:००:००\u202fPM || month:Jun|literal: |day:१|literal:, |year:२०२०|literal:, |hour:१२|literal::|minute:००|literal::|second:००|literal:\u202f|dayPeriod:PM
show('hi-u-nu-deva', {dateStyle: 'medium', timeStyle: 'long', timeZone: 'Asia/Kolkata'});
// CHECK-NEXT: १ जून २०२०, ५:३०:०० pm IST || day:१|literal: |month:जून|literal: |year:२०२०|literal:, |hour:५|literal::|minute:३०|literal::|second:००|literal: |dayPeriod:pm|literal: |timeZoneName:IST

// The numbering system is the resolved one: an unsupported -u-nu falls
// back to the locale's default.
print(new Intl.DateTimeFormat('ar-EG-u-nu-xxxx').resolvedOptions().numberingSystem, escape(new Intl.DateTimeFormat('ar-EG-u-nu-latn').format(t)));
// CHECK-NEXT: arab 1\u200f/6\u200f/2020
