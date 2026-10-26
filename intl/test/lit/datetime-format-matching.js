/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: cat %S/Inputs/escape.js %s > %t.js
// RUN: %hermes -Xintl-default-locale=en-US -Xintl-default-time-zone=UTC %t.js | %FileCheck --match-full-lines %s

// Best fit matches on CLDR's skeletons (refinements of plan decision 6,
// from the Task 7 review): the cases the review compared with Node, each
// formatted at 2026-07-15 18:07:09 UTC in Asia/Kolkata with the fields
// resolvedOptions reports. The strings are Node's (ICU 78.2, CLDR 48.0)
// except where noted. zh {hour, minute, second, timeZoneName: "short"}:
// the plugin matches availableFormats only (decision 6) and takes Hmsv
// "vHH:mm:ss", with the requested specific name, and reports the hour
// "2-digit" for its HH; Node's "GMT+5:30 23:37:09" comes from zh's long
// timeFormat "z HH:mm:ss", which ICU's generator also takes as a
// candidate (its Hmsv is "vHH:mm:ss" too: Node's {..., timeZoneName:
// "shortGeneric"} is "印度时间23:37:09").

var t = Date.UTC(2026, 6, 15, 18, 7, 9);
var FIELDS = [
  'weekday', 'era', 'year', 'month', 'day', 'dayPeriod', 'hour', 'minute',
  'second', 'timeZoneName',
];
function show(locale, options) {
  options.timeZone = 'Asia/Kolkata';
  var f = new Intl.DateTimeFormat(locale, options);
  var r = f.resolvedOptions();
  var fields = [];
  for (var i = 0; i < FIELDS.length; i++) {
    if (r[FIELDS[i]] !== undefined) fields.push(FIELDS[i] + ':' + r[FIELDS[i]]);
  }
  print(locale, escape(f.format(t)), '|', fields.join(' '));
}

// A: the skeleton's month width chooses the record.
show('zh', {year: 'numeric', month: 'long', day: 'numeric'});
// CHECK: zh 2026年7月15日 | year:numeric month:numeric day:numeric
show('ja', {year: 'numeric', month: 'long', day: 'numeric'});
// CHECK-NEXT: ja 2026年7月15日 | year:numeric month:numeric day:numeric
show('zh', {month: 'short', day: 'numeric'});
// CHECK-NEXT: zh 7月15日 | month:numeric day:numeric
show('cs', {month: 'short', day: 'numeric'});
// CHECK-NEXT: cs 15. 7. | month:numeric day:numeric
// B: an era calendar's y skeleton is the one-field year record. Node does
// not report the era; the draft reports a record's {era} field.
show('ja-u-ca-japanese', {year: 'numeric'});
// CHECK-NEXT: ja-u-ca-japanese 令和8年 | era:short year:numeric
show('th', {year: 'numeric'});
// CHECK-NEXT: th พ.ศ. 2569 | era:short year:numeric
show('zh-TW-u-ca-roc', {year: 'numeric'});
// CHECK-NEXT: zh-TW-u-ca-roc 民國115年 | era:short year:numeric
// Width adjustment by the matched skeleton: cs's yMMM is "LLLL y", so
// {month: "short"} ("MMM", as the skeleton) stays long.
show('cs', {year: 'numeric', month: 'short'});
// CHECK-NEXT: cs červenec 2026 | year:numeric month:long
// Equal matches go to the skeleton first in code point order: ko dangi's
// yyyyMMMEEEEd "r년(U년) MMM d일 EEEE" over yyyyMMMMEd "... MMMM d일(E)".
// (Node reports no year for dangi; the draft reports the record's.)
show('ko-u-ca-dangi', {
  year: 'numeric',
  month: 'long',
  day: 'numeric',
  weekday: 'long',
});
// CHECK-NEXT: ko-u-ca-dangi 2026년(병오년) 6월 2일 수요일 | weekday:long year:numeric month:long day:numeric
// Equal scores go first to the smaller finer width distance: en chinese's
// GyMMMd "MMM d, r" over GyMMMMd "MMMM d, r(U)" for a numeric month
// (Node reports no era or year; the draft reports the record's year).
show('en-u-ca-chinese', {
  era: 'short',
  year: 'numeric',
  month: 'numeric',
  day: 'numeric',
});
// CHECK-NEXT: en-u-ca-chinese Mo6 2, 2026 | year:numeric month:short day:numeric
// cs's Hmv "H:mm v" and Hmvvvv "H:mm, vvvv" both score -3 for a short
// specific name; the finer distance leaves timeZoneName out.
show('cs', {hour: 'numeric', minute: 'numeric', timeZoneName: 'short'});
// CHECK-NEXT: cs 23:37 GMT+5:30 | hour:numeric minute:2-digit timeZoneName:short
// C: a record with a field the request lacks is split and glued.
show('en-GB', {weekday: 'long', hour: 'numeric'});
// CHECK-NEXT: en-GB Wednesday, 23 | weekday:long hour:2-digit
// D: a locale's own v layout shows the requested specific name.
show('zh-Hant', {
  hour: 'numeric',
  minute: 'numeric',
  second: 'numeric',
  timeZoneName: 'short',
});
// CHECK-NEXT: zh-Hant 下午11:37:09 [GMT+5:30] | hour:numeric minute:2-digit second:2-digit timeZoneName:short
show('zh-Hant', {hour: 'numeric', minute: 'numeric', timeZoneName: 'long'});
// CHECK-NEXT: zh-Hant 下午11:37 [印度標準時間] | hour:numeric minute:2-digit timeZoneName:long
show('zh', {
  hour: 'numeric',
  minute: 'numeric',
  second: 'numeric',
  timeZoneName: 'short',
});
// CHECK-NEXT: zh GMT+5:3023:37:09 | hour:2-digit minute:2-digit second:2-digit timeZoneName:short

// A B the request did not ask for shows as AM/PM in best fit, as ICU does
// (a refinement of decision 6): zh-TW's h is "Bh時", but {hour} shows
// 上午 and 下午, and reports no dayPeriod. Its ranges take CLDR's
// intervals as for the B, as ICU4C's do; the styles keep the B.
var zhTwHour = new Intl.DateTimeFormat('zh-TW', {
  hour: 'numeric',
  timeZone: 'UTC',
});
print(
  zhTwHour.format(Date.UTC(2026, 0, 5, 0)),
  zhTwHour.format(Date.UTC(2026, 0, 5, 6)),
  zhTwHour.format(Date.UTC(2026, 0, 5, 12)),
  zhTwHour.format(Date.UTC(2026, 0, 5, 20)),
  zhTwHour.resolvedOptions().dayPeriod,
);
// CHECK-NEXT: 上午12時 上午6時 下午12時 下午8時 undefined
print(
  new Intl.DateTimeFormat('zh-TW', {
    hour: 'numeric',
    minute: 'numeric',
    timeZone: 'UTC',
  }).format(Date.UTC(2026, 0, 5, 1, 7)),
);
// CHECK-NEXT: 上午1:07
print(
  zhTwHour.formatRange(Date.UTC(2026, 0, 5, 1), Date.UTC(2026, 0, 5, 3)),
  zhTwHour.formatRange(Date.UTC(2026, 0, 5, 9), Date.UTC(2026, 0, 5, 20)),
);
// CHECK-NEXT: 凌晨1時至3時 上午9時至晚上8時
print(
  new Intl.DateTimeFormat('zh-TW', {
    timeStyle: 'short',
    timeZone: 'UTC',
  }).format(Date.UTC(2026, 0, 5, 1, 7)),
);
// CHECK-NEXT: 凌晨1:07

// E: best fit ranks a record first by the requested fields it lacks that
// its step 6 cannot append (all but timeZoneName and
// fractionalSecondDigits), then by the score (a refinement of decision 6
// from the final review). th's buddhist data has Hv "HH v" (-123) and Hm
// "HH:mm" (-126) but no Hmv, so {hour, minute, timeZoneName} takes Hm and
// appends the zone, in the format and in the ranges, whose [[Default]] (two
// days) best-fits the time again. Each line is the format at 13:07 UTC, a
// range 9:07 to 20:07, a range to 10:08 the next day, and the fields
// resolvedOptions reports. Node gives the same, but for its 1-digit hour in
// the same-day range of a 2-digit request (decision 11's documented
// difference) and fa's date part of the two-day range.
function line(locale, options) {
  options.timeZone = 'UTC';
  var f = new Intl.DateTimeFormat(locale, options);
  var r = f.resolvedOptions();
  var fields = [];
  for (var i = 0; i < FIELDS.length; i++) {
    if (r[FIELDS[i]] !== undefined) fields.push(FIELDS[i] + ':' + r[FIELDS[i]]);
  }
  print(
    locale,
    escape(f.format(Date.UTC(2026, 0, 5, 13, 7))),
    '|',
    escape(f.formatRange(Date.UTC(2026, 0, 5, 9, 7), Date.UTC(2026, 0, 5, 20, 7))),
    '|',
    escape(f.formatRange(Date.UTC(2026, 0, 5, 9, 7), Date.UTC(2026, 0, 6, 10, 8))),
    '|',
    r.hourCycle,
    fields.join(' '),
  );
}
line('th', {hour: '2-digit', minute: '2-digit', timeZoneName: 'short'});
// CHECK-NEXT: th 13:07 UTC | 09:07–20:07 UTC | 5/1/2569 09:07 UTC - 6/1/2569 10:08 UTC | h23 hour:2-digit minute:2-digit timeZoneName:short
line('fa', {hour: 'numeric', minute: 'numeric', timeZoneName: 'short'});
// CHECK-NEXT: fa ۱۳:۰۷ (UTC) | ۹:۰۷ تا ۲۰:۰۷ (UTC) | ۱۴۰۴/۱۰/۱۵،\u200f ۹:۰۷ (UTC) تا ۱۴۰۴/۱۰/۱۶،\u200f ۱۰:۰۸ (UTC) | h23 hour:numeric minute:2-digit timeZoneName:short
line('ja-u-ca-japanese', {hour: 'numeric', minute: 'numeric', timeZoneName: 'short'});
// CHECK-NEXT: ja-u-ca-japanese 13:07 UTC | 9時07分～20時07分(UTC) | R8/1/5 9:07 UTC～R8/1/6 10:08 UTC | h23 hour:numeric minute:2-digit timeZoneName:short

// F: a request with a dayPeriod and no hour is matched in the 12-hour
// family's records (formatsFamilyOf, a refinement of decision 6): the
// 24-hour family has no record with a day period and a minute or second.
// The record's hour shows by the resolved hour cycle. Node appends the day
// period as "7 ├AM/PM: in the afternoon┤".
line('en', {dayPeriod: 'short', minute: 'numeric', hour12: false});
// CHECK-NEXT: en 13:07 in the afternoon | 9:07 in the morning\u2009–\u200920:07 in the evening | 1/5/2026, 9:07 in the morning\u2009–\u20091/6/2026, 10:08 in the morning | h23 dayPeriod:short hour:numeric minute:2-digit
line('en', {dayPeriod: 'long', second: 'numeric', hour12: false});
// CHECK-NEXT: en 13:07:00 in the afternoon | 9:07:00 in the morning\u2009–\u200920:07:00 in the evening | 1/5/2026, 9:07:00 in the morning\u2009–\u20091/6/2026, 10:08:00 in the morning | h23 dayPeriod:long hour:numeric minute:2-digit second:2-digit
line('ja', {dayPeriod: 'short', minute: 'numeric'});
// CHECK-NEXT: ja 昼13:07 | 朝9:07～夜20:07 | 2026/1/5 朝9:07～2026/1/6 朝10:08 | h23 dayPeriod:short hour:numeric minute:2-digit
line('en', {dayPeriod: 'short', minute: 'numeric', hour12: false, formatMatcher: 'basic'});
// CHECK-NEXT: en 13:07 in the afternoon | 9:07 in the morning\u2009–\u200920:07 in the evening | 1/5/2026, 9:07 in the morning\u2009–\u20091/6/2026, 10:08 in the morning | h23 dayPeriod:short hour:numeric minute:2-digit

// G: a record whose unrequested B shows as AM/PM keeps the fields step 6
// appends in the record its range patterns take CLDR's intervals from
// (withDayPeriod), so zh-TW roc's same-day range keeps its zone, as Node
// does.
line('zh-TW-u-ca-roc', {hour: 'numeric', minute: 'numeric', timeZoneName: 'short'});
// CHECK-NEXT: zh-TW-u-ca-roc 下午1:07 UTC | 上午9:07至晚上8:07 [UTC] | 民國115/1/5 上午9:07 UTC\u2009–\u2009民國115/1/6 上午10:08 UTC | h12 hour:numeric minute:2-digit timeZoneName:short

// H: a style record's B counts as a dayPeriod field when its interval
// skeleton is chosen (a refinement of decision 11), so zh-TW's timeStyle
// "short" "Bh:mm" takes CLDR's Bhm intervals, as Node does.
line('zh-TW', {timeStyle: 'short'});
// CHECK-NEXT: zh-TW 下午1:07 | 上午9:07 – 晚上8:07 | 2026/1/5\u2009上午9:07\u2009–\u20092026/1/6\u2009上午10:08 | h12
print(
  new Intl.DateTimeFormat('zh-Hant', {timeStyle: 'short', timeZone: 'UTC'})
    .formatRange(Date.UTC(2026, 0, 5, 9, 7), Date.UTC(2026, 0, 5, 10, 7)),
);
// CHECK-NEXT: 上午9:07–10:07
