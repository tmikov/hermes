/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: cat %S/Inputs/escape.js %s > %t.js
// RUN: %hermes -Xintl-default-locale=en-US -Xintl-default-time-zone=UTC %t.js | %FileCheck --match-full-lines %s

// Intl.DateTimeFormat's format and formatToParts (plan Task 7):
// FormatDateTimePattern's numbers, names, hour cycles, eras, fractions of a
// second, day periods and time zone names, and the extreme and awkward
// instants of plan Review Focus 1. The expected strings are Node 24's (ICU
// 78.2) except where a comment says why the plugin's differ. Node's format
// writes U+0020 where CLDR has U+202F before AM and PM, a V8 compatibility
// change its formatToParts does not make; the plugin writes CLDR's U+202F
// in both.

var d = Date.UTC(2026, 0, 5, 15, 7, 9, 123);

/// new Intl.DateTimeFormat(locales, options).format(t), escaped, or the
/// class of the exception. t defaults to d.
function fmt(locales, options, t = d) {
  try {
    return escape(new Intl.DateTimeFormat(locales, options).format(t));
  } catch (e) {
    return e.constructor.name + ': ' + e.message;
  }
}

/// The parts of formatToParts(t) as type:value joined by "|", escaped.
function parts(locales, options, t = d) {
  var p = new Intl.DateTimeFormat(locales, options).formatToParts(t);
  return escape(p.map(x => x.type + ':' + x.value).join('|'));
}

// The default format, year, month and day numeric, in four locales; ar-EG
// has Arabic-Indic digits and right-to-left marks.
print(fmt('en'), fmt('de'), fmt('ja'), fmt('ar-EG'));
// CHECK: 1/5/2026 5.1.2026 2026/1/5 ٥\u200f/١\u200f/٢٠٢٦

// The styles.
print(fmt('en', {dateStyle: 'short'}), '|', fmt('en', {dateStyle: 'medium'}), '|', fmt('en', {dateStyle: 'long'}), '|', fmt('en', {dateStyle: 'full'}));
// CHECK-NEXT: 1/5/26 | Jan 5, 2026 | January 5, 2026 | Monday, January 5, 2026
print(fmt('en', {timeStyle: 'short'}), '|', fmt('en', {timeStyle: 'medium'}), '|', fmt('en', {timeStyle: 'long'}), '|', fmt('en', {timeStyle: 'full'}));
// CHECK-NEXT: 3:07\u202fPM | 3:07:09\u202fPM | 3:07:09\u202fPM UTC | 3:07:09\u202fPM Coordinated Universal Time
print(fmt('en-GB', {dateStyle: 'full', timeStyle: 'long'}));
// CHECK-NEXT: Monday, 5 January 2026 at 15:07:09 UTC
print(fmt('de', {dateStyle: 'full', timeStyle: 'short'}));
// CHECK-NEXT: Montag, 5. Januar 2026 um 15:07
print(fmt('ja', {dateStyle: 'full', timeStyle: 'short'}));
// CHECK-NEXT: 2026年1月5日月曜日 15:07
print(parts('en', {dateStyle: 'full', timeStyle: 'short'}));
// CHECK-NEXT: weekday:Monday|literal:, |month:January|literal: |day:5|literal:, |year:2026|literal: at |hour:3|literal::|minute:07|literal:\u202f|dayPeriod:PM

// The hour cycles at 00:07 (FormatDateTimePattern step 15.f.vi-vii).
var midnight = Date.UTC(2026, 0, 5, 0, 7);
print(
  fmt('en', {hour: 'numeric', hourCycle: 'h11'}, midnight),
  '|',
  fmt('en', {hour: 'numeric', hourCycle: 'h12'}, midnight),
  '|',
  fmt('en', {hour: 'numeric', hourCycle: 'h23'}, midnight),
  '|',
  fmt('en', {hour: 'numeric', hourCycle: 'h24'}, midnight),
);
// CHECK-NEXT: 0\u202fAM | 12\u202fAM | 00 | 24
print(fmt('en', {hour: 'numeric', minute: 'numeric', hourCycle: 'h12'}, Date.UTC(2026, 0, 5, 12, 30)));
// CHECK-NEXT: 12:30\u202fPM

// Years: 2 BC (the era year), and 2026 and 2005 2-digit, which take CLDR
// 49's en "yy", "’yy".
print(fmt('en', {year: 'numeric', era: 'short'}, -62198755200000), '|', fmt('en', {year: '2-digit'}), '|', fmt('en', {year: '2-digit'}, Date.UTC(2005, 0, 1)));
// CHECK-NEXT: 2 BC | ’26 | ’05
print(parts('en', {year: 'numeric', era: 'long'}, -62198755200000));
// CHECK-NEXT: year:2|literal: |era:Before Christ

// Fractions of a second: the decimal separator of the locale, and the
// digits floor(ms × 10^(n - 3)).
print(fmt('en', {second: 'numeric', fractionalSecondDigits: 3}), fmt('de', {second: 'numeric', fractionalSecondDigits: 3}), fmt('en', {fractionalSecondDigits: 2}), fmt('en', {fractionalSecondDigits: 1}));
// CHECK-NEXT: 9.123 9,123 12 1
print(fmt('en', {second: 'numeric', fractionalSecondDigits: 3}, Date.UTC(2026, 0, 5, 0, 0, 7, 5)), fmt('en', {second: 'numeric', fractionalSecondDigits: 2}, Date.UTC(2026, 0, 5, 0, 0, 7, 99)));
// CHECK-NEXT: 7.005 7.09

// Day periods: the 24 hours of a day in en give test262's five long
// values in order (dayPeriod-long-en.js); noon only at 12 with no minute
// shown.
var dayPeriods = new Intl.DateTimeFormat('en', {dayPeriod: 'long'});
var seen = [];
for (var h = 0; h < 24; h++) {
  var p = dayPeriods.format(Date.UTC(2017, 11, 12, h));
  if (seen[seen.length - 1] !== p) seen.push(p);
}
print(seen.join(','));
// CHECK-NEXT: in the morning,noon,in the afternoon,in the evening,at night
print(fmt('en', {hour: 'numeric', minute: 'numeric', dayPeriod: 'short'}, Date.UTC(2026, 0, 5, 9, 7)));
// CHECK-NEXT: 9:07 in the morning
print(fmt('en', {hour: 'numeric', dayPeriod: 'short'}, Date.UTC(2026, 0, 5, 12)), '|', fmt('en', {hour: 'numeric', minute: 'numeric', dayPeriod: 'short'}, Date.UTC(2026, 0, 5, 12, 30)));
// CHECK-NEXT: 12 noon | 12:30 in the afternoon
print(fmt('en', {dayPeriod: 'narrow'}, Date.UTC(2026, 0, 5, 12)), '|', fmt('en', {dayPeriod: 'narrow'}, Date.UTC(2026, 0, 5, 23)));
// CHECK-NEXT: n | at night
// CLDR 49 has no flexible day period names for scn, so a day period shows
// the hour's AM/PM name in the same width (the Task 1 ruling; Node gives
// the same). lij has them since CLDR 49: "da mattin" (morning1,
// abbreviated) and "de poidisnâ" (afternoon1, wide).
var morning = Date.UTC(2026, 0, 5, 9, 30);
var afternoon = Date.UTC(2026, 0, 5, 15, 30);
print(fmt('lij', {dayPeriod: 'short'}, morning), fmt('lij', {dayPeriod: 'long'}, afternoon), fmt('scn', {dayPeriod: 'short'}, morning), fmt('scn', {dayPeriod: 'narrow'}, afternoon));
// CHECK-NEXT: da mattin de poidisnâ AM PM
print(parts('lij', {dayPeriod: 'short'}, morning));
// CHECK-NEXT: dayPeriod:da mattin

// Time zone names.
print(fmt('en', {timeZone: 'America/New_York', timeZoneName: 'short'}, Date.UTC(2026, 6, 1, 12)));
// CHECK-NEXT: 7/1/2026, EDT
print(fmt('en', {timeZone: 'America/New_York', timeZoneName: 'long'}, Date.UTC(2026, 0, 1, 12)));
// CHECK-NEXT: 1/1/2026, Eastern Standard Time
print(fmt('en', {timeZone: 'America/New_York', timeZoneName: 'shortGeneric'}));
// CHECK-NEXT: 1/5/2026, ET
print(fmt('en', {timeZone: 'Asia/Kolkata', timeZoneName: 'longOffset'}));
// CHECK-NEXT: 1/5/2026, GMT+05:30
print(fmt('en', {timeZone: '-03:30', hour: 'numeric', timeZoneName: 'short'}));
// CHECK-NEXT: 11\u202fAM GMT-3:30
print(parts('en', {timeZone: '-03:30', hour: 'numeric', timeZoneName: 'short'}));
// CHECK-NEXT: hour:11|literal:\u202f|dayPeriod:AM|literal: |timeZoneName:GMT-3:30

// The whole time range.
var dateFields = {weekday: 'long', year: 'numeric', month: 'long', day: 'numeric'};
print(fmt('en', dateFields, 8.64e15));
// CHECK-NEXT: Saturday, September 13, 275760
print(fmt('en', {weekday: 'long', year: 'numeric', month: 'long', day: 'numeric', era: 'short'}, -8.64e15));
// CHECK-NEXT: Tuesday, April 20, 271822 BC

// Plan Review Focus 1: each instant and zone in every timeZoneName width,
// through format and formatToParts, whose values join to format's result;
// no "Exception in HostFunction:" appears. Node differs in four places.
// Three come from ICU4X's names (plan decision 4): the generic names of UTC are
// ICU4X's "UTC" and "Coordinated Universal Time" (ICU4C: "GMT+0" and
// "GMT+00:00"); before 1970 a zone's names use its 1970 metazone, and a
// local mean time offset matches none of its offsets, so New York's
// generic names in 1800 fall back to "GMT-4:56:02" (ICU4C: the location
// format "New York Time"); and the far future's names follow ICU4X's last
// metazone (plan decision 17), so New York in July 275000 is "EDT",
// "Eastern Daylight Time", "ET" and "Eastern Time" (ICU4C: "GMT-4",
// "GMT-04:00" and "New York Time"). The fourth comes from the data: Dublin's
// long generic name is "Irish Time" from CLDR 49's new Irish metazone,
// where Node's CLDR 48 gives the location format "Ireland Time".
var instants = [
  ['UTC', 8.64e15],
  ['UTC', -8.64e15],
  ['UTC', 0],
  ['UTC', -1],
  ['UTC', Date.UTC(-271821, 3, 20)],
  ['America/New_York', Date.UTC(2026, 2, 8, 6, 59, 59, 999)],
  ['America/New_York', Date.UTC(2026, 2, 8, 7)],
  ['America/New_York', Date.UTC(2026, 10, 1, 5, 59, 59, 999)],
  ['America/New_York', Date.UTC(2026, 10, 1, 6)],
  ['Australia/Lord_Howe', Date.UTC(2026, 0, 15)],
  ['Australia/Lord_Howe', Date.UTC(2026, 6, 15)],
  ['America/New_York', Date.UTC(1800, 0, 1, 12)],
  ['America/New_York', Date.UTC(275000, 6, 1, 12)],
  ['Europe/Dublin', Date.UTC(2026, 6, 1, 12)],
];
var widths = ['short', 'long', 'shortOffset', 'longOffset', 'shortGeneric', 'longGeneric'];
for (var i = 0; i < instants.length; i++) {
  var names = [];
  var formatted;
  for (var w = 0; w < widths.length; w++) {
    var dtf = new Intl.DateTimeFormat('en', {
      timeZone: instants[i][0],
      year: 'numeric',
      month: 'short',
      day: 'numeric',
      hour: 'numeric',
      minute: 'numeric',
      second: 'numeric',
      fractionalSecondDigits: 3,
      timeZoneName: widths[w],
    });
    var s = dtf.format(instants[i][1]);
    var ps = dtf.formatToParts(instants[i][1]);
    if (ps.map(x => x.value).join('') !== s) print('parts do not join: ' + s);
    names.push(ps.filter(x => x.type === 'timeZoneName')[0].value);
    if (w === 0) formatted = s.substring(0, s.length - names[0].length - 1);
  }
  print(instants[i][0], escape(formatted), '|', names.join(' | '));
}
// CHECK-NEXT: UTC Sep 13, 275760, 12:00:00.000\u202fAM | UTC | Coordinated Universal Time | GMT+0 | GMT+00:00 | UTC | Coordinated Universal Time
// CHECK-NEXT: UTC Apr 20, 271822, 12:00:00.000\u202fAM | UTC | Coordinated Universal Time | GMT+0 | GMT+00:00 | UTC | Coordinated Universal Time
// CHECK-NEXT: UTC Jan 1, 1970, 12:00:00.000\u202fAM | UTC | Coordinated Universal Time | GMT+0 | GMT+00:00 | UTC | Coordinated Universal Time
// CHECK-NEXT: UTC Dec 31, 1969, 11:59:59.999\u202fPM | UTC | Coordinated Universal Time | GMT+0 | GMT+00:00 | UTC | Coordinated Universal Time
// CHECK-NEXT: UTC Apr 20, 271822, 12:00:00.000\u202fAM | UTC | Coordinated Universal Time | GMT+0 | GMT+00:00 | UTC | Coordinated Universal Time
// CHECK-NEXT: America/New_York Mar 8, 2026, 1:59:59.999\u202fAM | EST | Eastern Standard Time | GMT-5 | GMT-05:00 | ET | Eastern Time
// CHECK-NEXT: America/New_York Mar 8, 2026, 3:00:00.000\u202fAM | EDT | Eastern Daylight Time | GMT-4 | GMT-04:00 | ET | Eastern Time
// CHECK-NEXT: America/New_York Nov 1, 2026, 1:59:59.999\u202fAM | EDT | Eastern Daylight Time | GMT-4 | GMT-04:00 | ET | Eastern Time
// CHECK-NEXT: America/New_York Nov 1, 2026, 1:00:00.000\u202fAM | EST | Eastern Standard Time | GMT-5 | GMT-05:00 | ET | Eastern Time
// CHECK-NEXT: Australia/Lord_Howe Jan 15, 2026, 11:00:00.000\u202fAM | GMT+11 | Lord Howe Daylight Time | GMT+11 | GMT+11:00 | Lord Howe Island Time | Lord Howe Time
// CHECK-NEXT: Australia/Lord_Howe Jul 15, 2026, 10:30:00.000\u202fAM | GMT+10:30 | Lord Howe Standard Time | GMT+10:30 | GMT+10:30 | Lord Howe Island Time | Lord Howe Time
// CHECK-NEXT: America/New_York Jan 1, 1800, 7:03:58.000\u202fAM | GMT-4:56:02 | GMT-04:56:02 | GMT-4:56:02 | GMT-04:56:02 | GMT-4:56:02 | GMT-04:56:02
// CHECK-NEXT: America/New_York Jul 1, 275000, 8:00:00.000\u202fAM | EDT | Eastern Daylight Time | GMT-4 | GMT-04:00 | ET | Eastern Time
// CHECK-NEXT: Europe/Dublin Jul 1, 2026, 1:00:00.000\u202fPM | GMT+1 | Irish Standard Time | GMT+1 | GMT+01:00 | Ireland Time | Irish Time

// TimeClip (PartitionDateTimePattern step 1): NaN and values beyond
// 8.64e15 are a RangeError; a fraction is truncated toward zero, and -0 is
// 0.
var dtf3 = new Intl.DateTimeFormat('en', {second: 'numeric', fractionalSecondDigits: 3});
function thrown(f) {
  try {
    return 'none: ' + f();
  } catch (e) {
    return e.constructor.name;
  }
}
print(
  thrown(() => dtf3.format(NaN)),
  thrown(() => dtf3.format(8.64e15 + 1)),
  thrown(() => dtf3.format(-8.64e15 - 1)),
  thrown(() => dtf3.formatToParts(Infinity)),
  thrown(() => dtf3.formatToParts(-Infinity)),
  thrown(() => dtf3.format('2026-01-05')),
);
// CHECK-NEXT: RangeError RangeError RangeError RangeError RangeError RangeError
print(dtf3.format(1.9), dtf3.format(-1.9), dtf3.format(-0), dtf3.format('5'), dtf3.format(new Date(2026, 0, 5, 1, 2, 3, 4)));
// CHECK-NEXT: 0.001 59.999 0.000 0.005 3.004
// format() formats Date.now(); a day-only format is the same unless the
// day changes between the two calls.
var dayFormat = new Intl.DateTimeFormat('en');
var before = dayFormat.format(Date.now());
var now = dayFormat.format();
print(now === before || now === dayFormat.format(Date.now()), dayFormat.formatToParts().length);
// CHECK-NEXT: true 5

// formatToParts: the types of FormatDateTimePattern, and fresh ordinary
// objects with own data properties type and value.
print(parts('en', {hour: 'numeric', minute: 'numeric', second: 'numeric', fractionalSecondDigits: 2, timeZoneName: 'short'}));
// CHECK-NEXT: hour:3|literal::|minute:07|literal::|second:09|literal:.|fractionalSecond:12|literal:\u202f|dayPeriod:PM|literal: |timeZoneName:UTC
var tp = new Intl.DateTimeFormat('en').formatToParts(d);
print(Array.isArray(tp), Object.getPrototypeOf(tp[0]) === Object.prototype, JSON.stringify(Object.getOwnPropertyDescriptor(tp[0], 'type')), Object.keys(tp[0]).join());
// CHECK-NEXT: true true {"value":"month","writable":true,"enumerable":true,"configurable":true} type,value
print(tp[0] !== new Intl.DateTimeFormat('en').formatToParts(d)[0]);
// CHECK-NEXT: true

// The bound format and formatToParts of one formatter agree, again and
// again (the parsed pattern and the internal NumberFormats are reused).
var reused = new Intl.DateTimeFormat('de', {dateStyle: 'medium', timeStyle: 'medium'});
var results = [];
for (var k = 0; k < 3; k++) {
  results.push(reused.format(d + k * 86400000));
  results.push(reused.formatToParts(d + k * 86400000).map(x => x.value).join(''));
}
print(results.join(' | '));
// CHECK-NEXT: 05.01.2026, 15:07:09 | 05.01.2026, 15:07:09 | 06.01.2026, 15:07:09 | 06.01.2026, 15:07:09 | 07.01.2026, 15:07:09 | 07.01.2026, 15:07:09
