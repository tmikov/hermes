/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: cat %S/Inputs/escape.js %s > %t.js
// RUN: %hermes -Xintl-default-locale=en-US -Xintl-default-time-zone=UTC %t.js | %FileCheck --match-full-lines %s

// Intl.DateTimeFormat's formatRange and formatRangeToParts (plan Task 8):
// PartitionDateTimeRangePattern's loop over the DateTime Range Pattern
// Record, with plan decision 3's comparisons (the month by its code, the
// chinese and dangi year by the related year) and decision 11's range
// patterns. The expected strings are Node 24's (ICU 78.2) except where a
// comment says why the plugin's differ. CLDR 48.2's en interval patterns
// put U+2009 around the dash; Node's formatRange writes U+0020 where CLDR
// has U+202F before AM and PM, a V8 compatibility change its
// formatRangeToParts does not make; the plugin writes CLDR's U+202F in
// both.

var b = Date.UTC(2026, 0, 5, 9, 7, 3, 45);
var H = 3600000;
var DAY = 24 * H;

/// new Intl.DateTimeFormat(locales, options).formatRange(x, y), escaped,
/// or the class of the exception.
function range(locales, options, x, y) {
  try {
    return escape(new Intl.DateTimeFormat(locales, options).formatRange(x, y));
  } catch (e) {
    return e.constructor.name;
  }
}

/// The parts of formatRangeToParts(x, y) as type:value:source joined by
/// "|", escaped.
function rangeParts(locales, options, x, y) {
  var p = new Intl.DateTimeFormat(locales, options).formatRangeToParts(x, y);
  return escape(p.map(q => q.type + ':' + q.value + ':' + q.source).join('|'));
}

// test262's en-US.js for formatRangeToParts, restated: the default format
// takes the fallback for every difference, so each date is whole; the
// year, month short and day format shares what the interval pattern
// shares.
var date1 = Date.UTC(2019, 0, 3);
var date2 = Date.UTC(2019, 0, 5);
var date3 = Date.UTC(2019, 2, 4);
var date4 = Date.UTC(2020, 2, 4);
print(rangeParts('en-US', undefined, date1, date1));
// CHECK: month:1:shared|literal:/:shared|day:3:shared|literal:/:shared|year:2019:shared
print(rangeParts('en-US', undefined, date1, date2));
// CHECK-NEXT: month:1:startRange|literal:/:startRange|day:3:startRange|literal:/:startRange|year:2019:startRange|literal:\u2009–\u2009:shared|month:1:endRange|literal:/:endRange|day:5:endRange|literal:/:endRange|year:2019:endRange
print(rangeParts('en-US', undefined, date1, date3));
// CHECK-NEXT: month:1:startRange|literal:/:startRange|day:3:startRange|literal:/:startRange|year:2019:startRange|literal:\u2009–\u2009:shared|month:3:endRange|literal:/:endRange|day:4:endRange|literal:/:endRange|year:2019:endRange
print(rangeParts('en-US', undefined, date1, date4));
// CHECK-NEXT: month:1:startRange|literal:/:startRange|day:3:startRange|literal:/:startRange|year:2019:startRange|literal:\u2009–\u2009:shared|month:3:endRange|literal:/:endRange|day:4:endRange|literal:/:endRange|year:2020:endRange
print(rangeParts('en-US', undefined, date2, date3));
// CHECK-NEXT: month:1:startRange|literal:/:startRange|day:5:startRange|literal:/:startRange|year:2019:startRange|literal:\u2009–\u2009:shared|month:3:endRange|literal:/:endRange|day:4:endRange|literal:/:endRange|year:2019:endRange
print(rangeParts('en-US', undefined, date2, date4));
// CHECK-NEXT: month:1:startRange|literal:/:startRange|day:5:startRange|literal:/:startRange|year:2019:startRange|literal:\u2009–\u2009:shared|month:3:endRange|literal:/:endRange|day:4:endRange|literal:/:endRange|year:2020:endRange
print(rangeParts('en-US', undefined, date3, date4));
// CHECK-NEXT: month:3:startRange|literal:/:startRange|day:4:startRange|literal:/:startRange|year:2019:startRange|literal:\u2009–\u2009:shared|month:3:endRange|literal:/:endRange|day:4:endRange|literal:/:endRange|year:2020:endRange
var ymd = {year: 'numeric', month: 'short', day: 'numeric'};
print(rangeParts('en-US', ymd, date1, date1));
// CHECK-NEXT: month:Jan:shared|literal: :shared|day:3:shared|literal:, :shared|year:2019:shared
print(rangeParts('en-US', ymd, date1, date2));
// CHECK-NEXT: month:Jan:shared|literal: :shared|day:3:startRange|literal:\u2009–\u2009:shared|day:5:endRange|literal:, :shared|year:2019:shared
print(rangeParts('en-US', ymd, date1, date3));
// CHECK-NEXT: month:Jan:startRange|literal: :startRange|day:3:startRange|literal:\u2009–\u2009:shared|month:Mar:endRange|literal: :endRange|day:4:endRange|literal:, :shared|year:2019:shared
print(rangeParts('en-US', ymd, date1, date4));
// CHECK-NEXT: month:Jan:startRange|literal: :startRange|day:3:startRange|literal:, :startRange|year:2019:startRange|literal:\u2009–\u2009:shared|month:Mar:endRange|literal: :endRange|day:4:endRange|literal:, :endRange|year:2020:endRange
print(rangeParts('en-US', ymd, date2, date3));
// CHECK-NEXT: month:Jan:startRange|literal: :startRange|day:5:startRange|literal:\u2009–\u2009:shared|month:Mar:endRange|literal: :endRange|day:4:endRange|literal:, :shared|year:2019:shared
print(rangeParts('en-US', ymd, date2, date4));
// CHECK-NEXT: month:Jan:startRange|literal: :startRange|day:5:startRange|literal:, :startRange|year:2019:startRange|literal:\u2009–\u2009:shared|month:Mar:endRange|literal: :endRange|day:4:endRange|literal:, :endRange|year:2020:endRange
print(rangeParts('en-US', ymd, date3, date4));
// CHECK-NEXT: month:Mar:startRange|literal: :startRange|day:4:startRange|literal:, :startRange|year:2019:startRange|literal:\u2009–\u2009:shared|month:Mar:endRange|literal: :endRange|day:4:endRange|literal:, :endRange|year:2020:endRange

// A day difference in dateStyle medium: the month and the year are shared.
print(rangeParts('en', {dateStyle: 'medium'}, Date.UTC(2026, 0, 5), Date.UTC(2026, 0, 9)));
// CHECK-NEXT: month:Jan:shared|literal: :shared|day:5:startRange|literal:\u2009–\u2009:shared|day:9:endRange|literal:, :shared|year:2026:shared

// Hours and minutes: AM/PM differs; then the hour, with AM/PM shared; a
// 24-hour cycle takes [[Hour]]'s pattern for an AM/PM difference; a day
// difference takes [[Default]], with the date.
var hm = {hour: 'numeric', minute: 'numeric'};
print(range('en', hm, b, b + 5 * H), '|', range('en', hm, b + 4 * H, b + 9 * H));
// CHECK-NEXT: 9:07\u202fAM\u2009–\u20092:07\u202fPM | 1:07\u2009–\u20096:07\u202fPM
print(range('en', {hour: 'numeric', minute: 'numeric', hourCycle: 'h23'}, b, b + 5 * H));
// CHECK-NEXT: 09:07\u2009–\u200914:07
print(range('en', hm, b, b + 4 * DAY + 4 * H));
// CHECK-NEXT: 1/5/2026, 9:07\u202fAM\u2009–\u20091/9/2026, 1:07\u202fPM
print(rangeParts('en', hm, b + 4 * H, b + 9 * H));
// CHECK-NEXT: hour:1:startRange|literal:::startRange|minute:07:startRange|literal:\u2009–\u2009:shared|hour:6:endRange|literal:::endRange|minute:07:endRange|literal:\u202f:shared|dayPeriod:PM:shared

// Seconds have no interval pattern in CLDR: the fallback, each time whole.
// fractionalSecondDigits 2 compares hundredths of a second, as the draft's
// step 15.d.iv says, so .045 and .049 collapse to the single format, every
// part shared (decision 17: ICU4C compares milliseconds, and Node gives
// "9:07:03.04 AM – 9:07:03.04 AM").
var hms = {hour: 'numeric', minute: 'numeric', second: 'numeric'};
print(range('en', hms, b, b + 7000));
// CHECK-NEXT: 9:07:03\u202fAM\u2009–\u20099:07:10\u202fAM
var hmsf = {hour: 'numeric', minute: 'numeric', second: 'numeric', fractionalSecondDigits: 2};
print(range('en', hmsf, b, b + 120), '|', range('en', hmsf, b, b + 4));
// CHECK-NEXT: 9:07:03.04\u202fAM\u2009–\u20099:07:03.16\u202fAM | 9:07:03.04\u202fAM
print(rangeParts('en', hmsf, b, b + 4));
// CHECK-NEXT: hour:9:shared|literal:::shared|minute:07:shared|literal:::shared|second:03:shared|literal:.:shared|fractionalSecond:04:shared|literal:\u202f:shared|dayPeriod:AM:shared

// A day period: an AM/PM difference takes [[DayPeriod]]'s pattern; a day
// period difference in the afternoon is shown (decision 11: ICU4C checks
// no day period difference and gives "1:07 – 6:07 in the evening").
var bhm = {hour: 'numeric', minute: 'numeric', dayPeriod: 'short'};
print(range('en', bhm, b, b + 5 * H));
// CHECK-NEXT: 9:07 in the morning\u2009–\u20092:07 in the afternoon
print(range('en', bhm, b + 4 * H, b + 9 * H));
// CHECK-NEXT: 1:07 in the afternoon\u2009–\u20096:07 in the evening

// Date and time styles: the date part shared, joined by the medium
// standard connector for every style, as ICU4C joins them (a refinement of
// decision 11, replacing the date style's connector of ruling 81: it and
// pl's long connector is "{1} {0}", and Node gives "5 gennaio 2026,
// 09:07–11:07"); a day difference shows both dates.
print(range('en', {dateStyle: 'medium', timeStyle: 'short'}, b, b + H));
// CHECK-NEXT: Jan 5, 2026, 9:07\u2009–\u200910:07\u202fAM
print(range('en', {dateStyle: 'medium', timeStyle: 'short'}, b, b + 30 * H));
// CHECK-NEXT: Jan 5, 2026, 9:07\u202fAM\u2009–\u2009Jan 6, 2026, 3:07\u202fPM
print(range('en', {dateStyle: 'full', timeStyle: 'short'}, b, b + H));
// CHECK-NEXT: Monday, January 5, 2026, 9:07\u2009–\u200910:07\u202fAM
print(rangeParts('en', {dateStyle: 'medium', timeStyle: 'short'}, b, b + H));
// CHECK-NEXT: month:Jan:shared|literal: :shared|day:5:shared|literal:, :shared|year:2026:shared|literal:, :shared|hour:9:startRange|literal:::startRange|minute:07:startRange|literal:\u2009–\u2009:shared|hour:10:endRange|literal:::endRange|minute:07:endRange|literal:\u202f:shared|dayPeriod:AM:shared
print(range('it', {dateStyle: 'long', timeStyle: 'short'}, b, b + 2 * H), '|', range('pl', {dateStyle: 'long', timeStyle: 'short'}, b, b + 2 * H));
// CHECK-NEXT: 5 gennaio 2026, 09:07–11:07 | 5 stycznia 2026, 09:07–11:07
print(range('sk', {dateStyle: 'medium', timeStyle: 'short'}, b, b + 2 * H), '|', range('de', {dateStyle: 'long', timeStyle: 'short'}, b, b + 2 * H));
// CHECK-NEXT: 5. 1. 2026, 9:07\u2009–\u200911:07 | 5. Januar 2026, 09:07–11:07 Uhr

// Dates: a year difference in a format without a year adds the year; a
// month difference shares the year; weekdays go with their days.
print(range('en', {month: 'short', day: 'numeric'}, b, b + 400 * DAY));
// CHECK-NEXT: Jan 5, 2026\u2009–\u2009Feb 9, 2027
print(range('en', {dateStyle: 'long'}, b, b + 40 * DAY));
// CHECK-NEXT: January 5\u2009–\u2009February 14, 2026
print(range('en', {weekday: 'long', month: 'long', day: 'numeric'}, b, b + DAY));
// CHECK-NEXT: Monday, January 5\u2009–\u2009Tuesday, January 6
print(range('en', {month: 'short', day: 'numeric'}, b, b + DAY), '|', range('en', {month: 'short', day: 'numeric'}, b, b + 31 * DAY));
// CHECK-NEXT: Jan 5\u2009–\u20096 | Jan 5\u2009–\u2009Feb 5

// A date with a time lacking the year, month or day: a difference in a
// field it lacks adds that field and each smaller one of year, month and
// day it lacks, numeric (a refinement of decision 11, in
// 53-datetime-ranges.js, as ICU4C adds them: FB(P) showed "Jan 5, 9 AM –
// Jan 5, 9 AM" a year apart). A difference in a field it shows is FB(P).
var mdh = {month: 'short', day: 'numeric', hour: 'numeric'};
print(range('en', mdh, b, b + 31 * DAY), '|', range('en', mdh, b, b + 365 * DAY));
// CHECK-NEXT: Jan 5, 9\u202fAM\u2009–\u2009Feb 5, 9\u202fAM | Jan 5, 2026, 9\u202fAM\u2009–\u2009Jan 5, 2027, 9\u202fAM
var dh = {day: 'numeric', hour: 'numeric'};
print(range('en', dh, b, b + DAY), '|', range('en', dh, b, b + 31 * DAY), '|', range('en', dh, b, b + 365 * DAY));
// CHECK-NEXT: 5, 9\u202fAM\u2009–\u20096, 9\u202fAM | 1/5, 9\u202fAM\u2009–\u20092/5, 9\u202fAM | 1/5/2026, 9\u202fAM\u2009–\u20091/5/2027, 9\u202fAM
var wh = {weekday: 'long', hour: 'numeric'};
print(range('en', wh, b, b + DAY), '|', range('en', wh, b, b + 31 * DAY), '|', range('en', wh, b, b + 365 * DAY));
// CHECK-NEXT: 5 Monday, 9\u202fAM\u2009–\u20096 Tuesday, 9\u202fAM | Monday, 1/5, 9\u202fAM\u2009–\u2009Thursday, 2/5, 9\u202fAM | Monday, 1/5/2026, 9\u202fAM\u2009–\u2009Tuesday, 1/5/2027, 9\u202fAM
var mh = {month: 'long', hour: 'numeric'};
print(range('en', mh, b, b + DAY), '|', range('en', mh, b, b + 365 * DAY));
// CHECK-NEXT: January 5 at 9\u202fAM\u2009–\u2009January 6 at 9\u202fAM | January 5, 2026 at 9\u202fAM\u2009–\u2009January 5, 2027 at 9\u202fAM
var yh = {year: 'numeric', hour: 'numeric'};
print(range('en', yh, b, b + 31 * DAY), '|', range('en', yh, b, b + 365 * DAY));
// CHECK-NEXT: 1/5/2026, 9\u202fAM\u2009–\u20092/5/2026, 9\u202fAM | 2026, 9\u202fAM\u2009–\u20092027, 9\u202fAM

// Interval widths follow ICU4C's adjustment (a refinement of decision 11,
// in 53-datetime-ranges.js): a field takes the format's width only when
// the format's pattern writes it with another letter count than the
// interval skeleton, the interval pattern writes it with the skeleton's
// count, and both are of the same kind; else the interval pattern's width
// stays. de's yMd "dd.–dd.MM.y" stays two-digit for "d.M.y", as ja's,
// nl's and pl's do, zh's H "HH–HH" and root chinese's Md "MM-dd"; he's
// MMMEd keeps its EEEE; en's MMMd takes the format's MMMM; fi's MMMM
// "LLL–LLLL", in two widths, takes the format's. All as Node gives.
print(range('de', undefined, b, b + DAY), '|', range('nl', undefined, b, b + DAY), '|', range('pl', undefined, b, b + DAY));
// CHECK-NEXT: 05.–06.01.2026 | 05-01-2026\u2009–\u200906-01-2026 | 05–06.01.2026
print(range('ja', undefined, b, b + DAY), '|', range('ja', {year: 'numeric', month: 'short', day: 'numeric'}, b, b + DAY));
// CHECK-NEXT: 2026/01/05～2026/01/06 | 2026/01/05～2026/01/06
print(range('zh', {hour: 'numeric'}, b, b + 2 * H), '|', range('ko-u-ca-dangi', {month: 'numeric', day: 'numeric'}, b, b + 3 * DAY));
// CHECK-NEXT: 09–11 | 11-17\u2009–\u200911-20
print(range('en', {year: 'numeric', month: 'long', day: 'numeric'}, b, b + DAY), '|', range('fi', {month: 'long'}, b, b + 31 * DAY), '|', range('he', {weekday: 'short', month: 'short', day: 'numeric'}, b, b + DAY));
// CHECK-NEXT: January 5\u2009–\u20096, 2026 | tammikuu–helmikuu | יום שני, 5 בינו׳ – יום שלישי, 6 בינו׳
// So does a field of the other kind than the format's, whose literals are
// written for the interval pattern's own kind (a refinement of decision
// 11): ko's yMMMd day interval "y년 M월 d일~d일" has a numeric month,
// which with the format's "MMM" ("1월") showed "1월월"; fi's yM month
// interval "LLL–LLLL y" has text months, CLDR's own interval (Node uses
// the fallback, "1.2026–2.2026").
print(range('ko', {year: 'numeric', month: 'short', day: 'numeric'}, b, b + DAY), '|', range('fi', {year: 'numeric', month: 'numeric'}, b, b + 31 * DAY));
// CHECK-NEXT: 2026년 1월 5일~6일 | tammi–helmi 2026

// A weekday with an hour, a time zone name, and a minute alone.
print(range('en', {weekday: 'long', hour: 'numeric'}, b, b + H));
// CHECK-NEXT: Monday, 9\u2009–\u200910\u202fAM
print(range('en', {hour: 'numeric', timeZoneName: 'short'}, b, b + H));
// CHECK-NEXT: 9\u2009–\u200910\u202fAM UTC
print(range('en', {minute: 'numeric'}, b, b + 60000));
// CHECK-NEXT: 7\u2009–\u20098

// 2-digit fields keep their widths in the interval pattern (decision 11:
// ICU4C's intervals ignore 2-digit requests and give "1/5/26 – 1/6/26").
print(range('en', {year: '2-digit', month: '2-digit', day: '2-digit'}, b, b + DAY));
// CHECK-NEXT: 01/05/26\u2009–\u200901/06/26

// chinese and dangi: a leap month and its base month differ by their month
// codes (decision 3), though both have the number 4. 2020-05-01 is day 9
// of M04 and 2020-05-31 day 9 of M04L. ICU4C compares the month number
// alone, so Node collapses each range to the start ("4-9", "四月9日").
// ko's dangi intervals are root's chinese ones, which have no 일 (Node:
// "4월 9–12" for a day difference), and whose Md "MM-dd" keeps two digits.
var m04 = Date.UTC(2020, 4, 1);
var m04L = Date.UTC(2020, 4, 31);
print(range('zh-u-ca-chinese', {month: 'numeric', day: 'numeric'}, m04, m04L), '|', range('zh-u-ca-chinese', {month: 'long', day: 'numeric'}, m04, m04L));
// CHECK-NEXT: 4-9至闰4-9 | 四月9日至闰四月9日
print(range('ko-u-ca-dangi', {month: 'numeric', day: 'numeric'}, m04, m04L), '|', range('ko-u-ca-dangi', {month: 'long', day: 'numeric'}, m04, m04L));
// CHECK-NEXT: 04-09\u2009–\u2009윤04-09 | 4월 9\u2009–\u2009윤4월 9
// 1984-02-02 and 2044-01-30 are both the first day of a jia-zi year: the
// cyclic years are equal and the related years differ (decision 3). The
// year-only interval pattern would drop the related year, so the fallback
// is used (decisions 6 and 11).
var jiazi1 = Date.UTC(1984, 1, 2);
var jiazi2 = Date.UTC(2044, 0, 30);
print(range('en-u-ca-chinese', {year: 'numeric'}, jiazi1, jiazi2), '|', range('en-u-ca-dangi', {year: 'numeric'}, jiazi1, jiazi2));
// CHECK-NEXT: 1984(jia-zi)\u2009–\u20092044(jia-zi) | 1984(jia-zi)\u2009–\u20092044(jia-zi)
var cymd = {year: 'numeric', month: 'numeric', day: 'numeric'};
print(range('en-u-ca-chinese', cymd, jiazi1, jiazi2), '|', range('en-u-ca-dangi', cymd, jiazi1, jiazi2));
// CHECK-NEXT: 1/1/1984\u2009–\u20091/1/2044 | 1/1/1984\u2009–\u20091/1/2044
print(rangeParts('en-u-ca-chinese', cymd, jiazi1, jiazi2));
// CHECK-NEXT: month:1:startRange|literal:/:startRange|day:1:startRange|literal:/:startRange|relatedYear:1984:startRange|literal:\u2009–\u2009:shared|month:1:endRange|literal:/:endRange|day:1:endRange|literal:/:endRange|relatedYear:2044:endRange

// Equal formatted ends collapse to the single format, every part shared;
// so do different instants whose shown fields are equal.
print(range('en', {dateStyle: 'medium'}, b, b + H), '|', rangeParts('en', hm, b, b + 30000));
// CHECK-NEXT: Jan 5, 2026 | hour:9:shared|literal:::shared|minute:07:shared|literal:\u202f:shared|dayPeriod:AM:shared
// An end before the start does not throw.
print(range('en', hm, b + H, b));
// CHECK-NEXT: 10:07\u2009–\u20099:07\u202fAM

// The argument checks: undefined is a TypeError, a time value TimeClip
// makes NaN a RangeError, in either position, for both methods.
var dtf = new Intl.DateTimeFormat('en');
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
  thrown(() => dtf.formatRange(undefined, b)),
  thrown(() => dtf.formatRangeToParts(b, undefined)),
  thrown(() => dtf.formatRange(NaN, b)),
  thrown(() => dtf.formatRange(b, 8.64e15 + 1)),
  thrown(() => dtf.formatRangeToParts(-8.64e15 - 1, b)),
  thrown(() => dtf.formatRangeToParts(b, Infinity)),
  thrown(() => dtf.formatRange(8.64e15, -8.64e15)),
);
// CHECK-NEXT: TypeError TypeError RangeError RangeError RangeError RangeError none
// The arguments are converted in order: the first's valueOf runs, then the
// second's, which throws; no range is formatted.
var order = [];
print(
  thrown(() =>
    dtf.formatRange(
      {valueOf: () => (order.push('start'), b)},
      {valueOf: () => { order.push('end'); throw new EvalError(); }},
    ),
  ),
  order.join(),
);
// CHECK-NEXT: EvalError start,end
// Both undefined checks come before the conversions, so nothing is
// converted.
order = [];
print(thrown(() => dtf.formatRange({valueOf: () => (order.push('start'), b)}, undefined)), order.length);
// CHECK-NEXT: TypeError 0

// formatRangeToParts returns ordinary objects with type, value and source
// own data properties, in that order, in an ordinary array.
var p = dtf.formatRangeToParts(b, b + DAY);
print(
  Array.isArray(p),
  Object.getPrototypeOf(p[0]) === Object.prototype,
  Object.keys(p[0]).join(),
  JSON.stringify(Object.getOwnPropertyDescriptor(p[0], 'source')),
);
// CHECK-NEXT: true true type,value,source {"value":"startRange","writable":true,"enumerable":true,"configurable":true}

// A date with a time joins the date to the time range's own record: the
// interval pattern's day period, AM/PM and hour widths stay (rulings 79,
// 127 and 132), and its names are loaded. zh-TW's AM/PM interval for h,
// "ah時至ah時", shows the AM/PM, and its hour interval "Bh時至h時" a day
// period (the all-pairs check below covers it); zh's and it's hour
// intervals keep "HH".
var zt = {weekday: 'short', hour: 'numeric'};
print(range('zh-TW', zt, b, b + 5 * H));
// CHECK-NEXT: 週一 上午9時至下午2時
print(rangeParts('zh-TW', zt, b, b + 5 * H));
// CHECK-NEXT: weekday:週一:shared|literal: :shared|dayPeriod:上午:startRange|hour:9:startRange|literal:時至:shared|dayPeriod:下午:endRange|hour:2:endRange|literal:時:shared
print(range('zh', {month: 'numeric', day: 'numeric', hour: 'numeric'}, b, b + 2 * H), '|', range('zh', zt, b, b + 2 * H), '|', range('it', zt, b, b + 2 * H), '|', range('ru', {month: 'numeric', day: 'numeric', hour: 'numeric'}, b, b + 2 * H));
// CHECK-NEXT: 1/5 09–11 | 周一 09–11 | lun, 09–11 | 05.01, 09–11\u00a0ч
// The widths compare the letter counts of the format's own pattern (ruling
// 132), not those of a separately fitted time part: be's and lt's
// {weekday, hour} patterns write "h", their time parts "hh"; hr's and
// bs's likewise with minutes.
var beo = {weekday: 'long', hour: 'numeric', hour12: true};
print(range('be', beo, b, b + 2 * H), '|', range('lt', beo, b, b + 2 * H));
// CHECK-NEXT: панядзелак, 9–11\u202fAM | pirmadienis 9–11\u202fpriešpiet
var hro = {weekday: 'short', hour: 'numeric', minute: '2-digit', hour12: true};
print(range('hr', hro, b, b + 2 * H), '|', range('bs', hro, b, b + 2 * H));
// CHECK-NEXT: pon 9:07\u2009–\u200911:07\u202fAM | pon 9:07\u2009–\u200911:07\u202fa.\u202fm.

// A weekday counts as a day for [[Day]] (a refinement of decision 11), so
// a {weekday} format collapses on one day and takes [[Day]]'s FB(P) for
// different days, as Node gives.
var wl = {weekday: 'long'};
print(range('en', wl, b, b + 1), '|', range('en', wl, b, b + H), '|', range('en', wl, b, b + DAY), '|', range('en', wl, b, b + 7 * DAY));
// CHECK-NEXT: Monday | Monday | Monday\u2009–\u2009Tuesday | Monday\u2009–\u2009Monday
print(rangeParts('en', wl, b, b + H), '|', range('de', wl, b, b + H), '|', range('ja', wl, b, b + H));
// CHECK-NEXT: weekday:Monday:shared | Montag | 月曜日

// Decision 11's coverage check, through the public methods: every pair of
// hours of a day (at 7 minutes and 3 seconds past) for the h, H, Bh, hm,
// Hm, Bhm and hms field sets, a date and time set (Eh, {weekday, hour}),
// and a date set (E, {weekday}). For the sets with a time, two different
// times never collapse to one (the loop stops at no field above the
// difference), and no pair of the same day takes [[Default]], which would
// show the date (or, for Eh, a weekday per end). Equal times collapse.
// Every E pair collapses, the day being the same. No part lacks its
// value. Each line prints the field set and the number of failures.
var day0 = Date.UTC(2026, 0, 5, 0, 7, 3);
var fieldSets = [
  ['h', {hour: 'numeric', hourCycle: 'h12'}],
  ['H', {hour: 'numeric', hourCycle: 'h23'}],
  ['Bh', {hour: 'numeric', dayPeriod: 'short'}],
  ['hm', {hour: 'numeric', minute: 'numeric', hourCycle: 'h12'}],
  ['Hm', {hour: 'numeric', minute: 'numeric', hourCycle: 'h23'}],
  ['Bhm', {hour: 'numeric', minute: 'numeric', dayPeriod: 'short'}],
  ['hms', {hour: 'numeric', minute: 'numeric', second: 'numeric', hourCycle: 'h12'}],
  ['Eh', {weekday: 'short', hour: 'numeric'}],
  ['E', {weekday: 'long'}],
];
var allPairsLocales = ['en', 'en-GB', 'de', 'fr', 'ja', 'zh', 'zh-TW', 'ko', 'es', 'ru', 'ar'];
for (var li = 0; li < allPairsLocales.length; li++) {
  var line = allPairsLocales[li];
  for (var fi = 0; fi < fieldSets.length; fi++) {
    var f = new Intl.DateTimeFormat(allPairsLocales[li], fieldSets[fi][1]);
    var failures = 0;
    var example = '';
    for (var i = 0; i < 24; i++) {
      for (var j = 0; j < 24; j++) {
        var parts = f.formatRangeToParts(day0 + i * H, day0 + j * H);
        var collapsed = parts.every(q => q.source === 'shared');
        var hasDate = parts.some(q => q.type === 'day' || q.type === 'year' || (q.type === 'weekday' && q.source !== 'shared'));
        var noValue = parts.some(q => typeof q.value !== 'string');
        var expectCollapsed = fieldSets[fi][0] === 'E' || i === j;
        if (collapsed !== expectCollapsed || hasDate || noValue) {
          failures++;
          if (example === '') example = ' ' + i + '-' + j + ': ' + escape(f.formatRange(day0 + i * H, day0 + j * H));
        }
      }
    }
    line += ' ' + fieldSets[fi][0] + ':' + failures + example;
  }
  print(line);
}
// CHECK-NEXT: en h:0 H:0 Bh:0 hm:0 Hm:0 Bhm:0 hms:0 Eh:0 E:0
// CHECK-NEXT: en-GB h:0 H:0 Bh:0 hm:0 Hm:0 Bhm:0 hms:0 Eh:0 E:0
// CHECK-NEXT: de h:0 H:0 Bh:0 hm:0 Hm:0 Bhm:0 hms:0 Eh:0 E:0
// CHECK-NEXT: fr h:0 H:0 Bh:0 hm:0 Hm:0 Bhm:0 hms:0 Eh:0 E:0
// CHECK-NEXT: ja h:0 H:0 Bh:0 hm:0 Hm:0 Bhm:0 hms:0 Eh:0 E:0
// CHECK-NEXT: zh h:0 H:0 Bh:0 hm:0 Hm:0 Bhm:0 hms:0 Eh:0 E:0
// CHECK-NEXT: zh-TW h:0 H:0 Bh:0 hm:0 Hm:0 Bhm:0 hms:0 Eh:0 E:0
// CHECK-NEXT: ko h:0 H:0 Bh:0 hm:0 Hm:0 Bhm:0 hms:0 Eh:0 E:0
// CHECK-NEXT: es h:0 H:0 Bh:0 hm:0 Hm:0 Bhm:0 hms:0 Eh:0 E:0
// CHECK-NEXT: ru h:0 H:0 Bh:0 hm:0 Hm:0 Bhm:0 hms:0 Eh:0 E:0
// CHECK-NEXT: ar h:0 H:0 Bh:0 hm:0 Hm:0 Bhm:0 hms:0 Eh:0 E:0
