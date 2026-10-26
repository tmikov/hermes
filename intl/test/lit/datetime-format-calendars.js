/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: cat %S/Inputs/escape.js %s > %t.js
// RUN: %hermes -Xintl-default-locale=en-US -Xintl-default-time-zone=UTC %t.js | %FileCheck --match-full-lines %s

// Intl.DateTimeFormat's calendars (plan Review Focus 2): each supported
// calendar with en and with a locale that uses it, through format and
// formatToParts. The fields and names come from ICU4X (plan decisions 3
// and 4): chinese and dangi leap months (2020-05-23 starts M04L) numeric
// and text, cyclic years and related years; hebrew Adar I and II in 5784
// and Adar in 5783; the japanese era change on 2019-04-30/05-01; roc
// before 1912; ethioaa; iso8601 year 0. The expected strings are Node 24's
// (ICU 78.2) except where a comment says why the plugin's differ.

/// The format and the formatToParts parts of t, escaped.
function show(locales, options, t) {
  var dtf = new Intl.DateTimeFormat(locales, options);
  var s = dtf.format(t);
  var parts = dtf.formatToParts(t);
  var joined = parts.map(p => p.value).join('');
  print(
    escape(s) +
      ' || ' +
      escape(parts.map(p => p.type + ':' + p.value).join('|')) +
      (joined === s ? '' : ' (parts do not join)'),
  );
}

// Chinese: a cyclic year with its related ISO year, and the leap fourth
// month of 2020, numeric ("闰4", the numeric leap month pattern of plan
// decision 5) and as a name.
show('zh-u-ca-chinese', {year: 'numeric'}, Date.UTC(2019, 5, 1));
// CHECK: 2019己亥年 || relatedYear:2019|yearName:己亥|literal:年
show('zh-u-ca-chinese', {month: 'numeric', day: 'numeric'}, Date.UTC(2020, 5, 1));
// CHECK-NEXT: 闰4-10 || month:闰4|literal:-|day:10
show('zh-u-ca-chinese', {month: 'long', day: 'numeric'}, Date.UTC(2020, 5, 1));
// CHECK-NEXT: 闰四月10日 || month:闰四月|day:10|literal:日
// The day is "10": CLDR's dateFormats full has d=hanidays, a numbering
// override the draft does not apply (plan decision 17; ICU4C: "初十").
show('zh-u-ca-chinese', {dateStyle: 'full'}, Date.UTC(2020, 5, 1));
// CHECK-NEXT: 2020庚子年闰四月10星期一 || relatedYear:2020|yearName:庚子|literal:年|month:闰四月|day:10|weekday:星期一
show('en-u-ca-chinese', {dateStyle: 'full'}, Date.UTC(2020, 5, 1));
// CHECK-NEXT: Monday, Fourth Monthbis 10, 2020(geng-zi) || weekday:Monday|literal:, |month:Fourth Monthbis|literal: |day:10|literal:, |relatedYear:2020|literal:(|yearName:geng-zi|literal:)
show('en-u-ca-chinese', {month: 'numeric', day: 'numeric'}, Date.UTC(2020, 5, 1));
// CHECK-NEXT: 4bis/10 || month:4bis|literal:/|day:10
// The day before the leap month and its first day.
show('en-u-ca-chinese', {month: 'long'}, Date.UTC(2020, 4, 22));
// CHECK-NEXT: Fourth Month || month:Fourth Month
show('en-u-ca-chinese', {month: 'long'}, Date.UTC(2020, 4, 23));
// CHECK-NEXT: Fourth Monthbis || month:Fourth Monthbis
// y is the related year in chinese (plan decision 6).
show('en-u-ca-chinese', {year: 'numeric', month: 'numeric', day: 'numeric'}, Date.UTC(1984, 1, 2));
// CHECK-NEXT: 1/1/1984 || month:1|literal:/|day:1|literal:/|relatedYear:1984

// Dangi.
show('ko-u-ca-dangi', {dateStyle: 'full'}, Date.UTC(2020, 5, 1));
// CHECK-NEXT: 경자년 윤4월 10일 월요일 || yearName:경자|literal:년 |month:윤4월|literal: |day:10|literal:일 |weekday:월요일
show('ko-u-ca-dangi', {month: 'numeric', day: 'numeric'}, Date.UTC(2020, 5, 1));
// CHECK-NEXT: 윤4. 10. || month:윤4|literal:. |day:10|literal:.
show('en-u-ca-dangi', {dateStyle: 'full'}, Date.UTC(2020, 5, 1));
// CHECK-NEXT: Monday, Fourth Monthbis 10, 2020(geng-zi) || weekday:Monday|literal:, |month:Fourth Monthbis|literal: |day:10|literal:, |relatedYear:2020|literal:(|yearName:geng-zi|literal:)

// Calendars without eras: {era} gives no part (plan decision 6), so only
// the pattern's literal is left, as Node gives for iso8601.
show('en', {calendar: 'iso8601', era: 'short', year: 'numeric'}, 0);
// CHECK-NEXT:  1970 || literal: |year:1970
show('en', {calendar: 'chinese', era: 'short', year: 'numeric'}, 0);
// CHECK-NEXT: 1969(ji-you) || relatedYear:1969|literal:(|yearName:ji-you|literal:)
show('en', {calendar: 'dangi', era: 'short', year: 'numeric'}, 0);
// CHECK-NEXT: 1969(ji-you) || relatedYear:1969|literal:(|yearName:ji-you|literal:)

// Japanese: the era change. The first year of an era is "1": CLDR's
// y=jpanyear override ("元") is not applied (plan decision 17; ICU4C:
// "令和元年").
show('ja-u-ca-japanese', {era: 'long', year: 'numeric'}, Date.UTC(2020, 5, 1));
// CHECK-NEXT: 令和2年 || era:令和|year:2|literal:年
show('ja-u-ca-japanese', {dateStyle: 'full'}, Date.UTC(2019, 3, 30));
// CHECK-NEXT: 平成31年4月30日火曜日 || era:平成|year:31|literal:年|month:4|literal:月|day:30|literal:日|weekday:火曜日
show('ja-u-ca-japanese', {dateStyle: 'full'}, Date.UTC(2019, 4, 1));
// CHECK-NEXT: 令和1年5月1日水曜日 || era:令和|year:1|literal:年|month:5|literal:月|day:1|literal:日|weekday:水曜日
show('en-u-ca-japanese', {dateStyle: 'full'}, Date.UTC(2019, 3, 30));
// CHECK-NEXT: Tuesday, April 30, 31 Heisei || weekday:Tuesday|literal:, |month:April|literal: |day:30|literal:, |year:31|literal: |era:Heisei
show('en-u-ca-japanese', {}, Date.UTC(2019, 4, 1));
// CHECK-NEXT: 5/1/1 R || month:5|literal:/|day:1|literal:/|year:1|literal: |era:R

// Hebrew: Adar I and Adar II of the leap year 5784, Adar of 5783.
show('en-u-ca-hebrew', {month: 'long'}, Date.UTC(2024, 1, 15));
// CHECK-NEXT: Adar I || month:Adar I
show('en-u-ca-hebrew', {month: 'long'}, Date.UTC(2024, 2, 15));
// CHECK-NEXT: Adar II || month:Adar II
show('en-u-ca-hebrew', {month: 'long'}, Date.UTC(2023, 2, 15));
// CHECK-NEXT: Adar || month:Adar
show('en-u-ca-hebrew', {dateStyle: 'full'}, Date.UTC(2024, 2, 15));
// CHECK-NEXT: Friday, 5 Adar II 5784 || weekday:Friday|literal:, |day:5|literal: |month:Adar II|literal: |year:5784
// The numbers are [[NumberingSystem]]'s, not CLDR's hebr override (plan
// decision 17; ICU4C: "ה׳ באדר ב׳ תשפ״ד").
show('he-u-ca-hebrew', {dateStyle: 'long'}, Date.UTC(2024, 2, 15));
// CHECK-NEXT: 5 באדר ב׳ 5784 || day:5|literal: ב|month:אדר ב׳|literal: |year:5784
// The default {year, month, day: "numeric"}: en's yMd "d MMM y" and
// yyyyMd "M/d/y G" both score 0 by their skeletons, and the tie goes to
// the skeleton first in code point order, yMd, as ICU4C chooses.
show('en-u-ca-hebrew', {}, Date.UTC(2024, 2, 15));
// CHECK-NEXT: 5 Adar II 5784 || day:5|literal: |month:Adar II|literal: |year:5784
// A numeric month is the ordinal: Adar II is the seventh month of 5784.
show('en-u-ca-hebrew', {month: 'numeric'}, Date.UTC(2024, 2, 15));
// CHECK-NEXT: 7 || month:7
// Adar I is the leap month M05L, but hebrew has no numeric leap month
// pattern (dateTimePatterns entry 17 is empty), so it is the ordinal "6"
// with nothing around it, as Node gives.
show('en-u-ca-hebrew', {month: 'numeric'}, Date.UTC(2024, 1, 15));
// CHECK-NEXT: 6 || month:6

// ISO 8601: CLDR's root patterns, the ISO year (year 0 shows as "1" and -1
// as "2", by the draft's 1 - value step, as ICU4C shows them), and
// Gregorian month names (plan decision 17; ICU 78.2 prints "2026  5" for
// dateStyle long, with no month).
show('en-u-ca-iso8601', {}, Date.UTC(2026, 0, 5));
// CHECK-NEXT: 2026-01-05 || year:2026|literal:-|month:01|literal:-|day:05
show('en-u-ca-iso8601', {dateStyle: 'long'}, Date.UTC(2026, 0, 5));
// CHECK-NEXT: 2026 January 5 || year:2026|literal: |month:January|literal: |day:5
show('en-u-ca-iso8601', {year: 'numeric', month: 'numeric', day: 'numeric'}, -62167219200000);
// CHECK-NEXT: 1-01-01 || year:1|literal:-|month:01|literal:-|day:01
show('en-u-ca-iso8601', {year: 'numeric', month: 'numeric', day: 'numeric'}, -62198755200000);
// CHECK-NEXT: 2-01-01 || year:2|literal:-|month:01|literal:-|day:01

// Roc before and after 1912.
show('en-u-ca-roc', {dateStyle: 'full'}, Date.UTC(2026, 0, 5));
// CHECK-NEXT: Monday, January 5, 115 Minguo || weekday:Monday|literal:, |month:January|literal: |day:5|literal:, |year:115|literal: |era:Minguo
show('en-u-ca-roc', {dateStyle: 'full'}, Date.UTC(1900, 0, 5));
// CHECK-NEXT: Friday, January 5, 12 B.R.O.C. || weekday:Friday|literal:, |month:January|literal: |day:5|literal:, |year:12|literal: |era:B.R.O.C.
show('zh-TW-u-ca-roc', {dateStyle: 'full'}, Date.UTC(1900, 0, 5));
// CHECK-NEXT: 民國前12年1月5日 星期五 || era:民國前|year:12|literal:年|month:1|literal:月|day:5|literal:日 |weekday:星期五

// Each other calendar, with en and a locale that uses it.
var d = Date.UTC(2026, 0, 5);
show('th', {dateStyle: 'full'}, d);
// CHECK-NEXT: วันจันทร์ที่ 5 มกราคม พ.ศ. 2569 || weekday:วันจันทร์|literal:ที่ |day:5|literal: |month:มกราคม|literal: |era:พ.ศ.|literal: |year:2569
show('en-u-ca-buddhist', {dateStyle: 'full'}, d);
// CHECK-NEXT: Monday, January 5, 2569 BE || weekday:Monday|literal:, |month:January|literal: |day:5|literal:, |year:2569|literal: |era:BE
show('fa', {dateStyle: 'full'}, d);
// CHECK-NEXT: ۱۴۰۴ دی ۱۵, دوشنبه || year:۱۴۰۴|literal: |month:دی|literal: |day:۱۵|literal:, |weekday:دوشنبه
show('en-u-ca-persian', {dateStyle: 'full'}, d);
// CHECK-NEXT: Monday, Dey 15, 1404 AP || weekday:Monday|literal:, |month:Dey|literal: |day:15|literal:, |year:1404|literal: |era:AP
show('ar-SA-u-ca-islamic-umalqura', {dateStyle: 'full'}, d);
// CHECK-NEXT: الاثنين، ١٦ رجب ١٤٤٧ هـ || weekday:الاثنين|literal:، |day:١٦|literal: |month:رجب|literal: |year:١٤٤٧|literal: |era:هـ
show('en-u-ca-islamic-umalqura', {dateStyle: 'full'}, d);
// CHECK-NEXT: Monday, Rajab 16, 1447 AH || weekday:Monday|literal:, |month:Rajab|literal: |day:16|literal:, |year:1447|literal: |era:AH
show('en-u-ca-islamic-civil', {dateStyle: 'full'}, d);
// CHECK-NEXT: Monday, Rajab 16, 1447 AH || weekday:Monday|literal:, |month:Rajab|literal: |day:16|literal:, |year:1447|literal: |era:AH
show('en-u-ca-islamic-tbla', {dateStyle: 'full'}, d);
// CHECK-NEXT: Monday, Rajab 17, 1447 AH || weekday:Monday|literal:, |month:Rajab|literal: |day:17|literal:, |year:1447|literal: |era:AH
show('am-u-ca-ethiopic', {dateStyle: 'full'}, d);
// CHECK-NEXT: ሰኞ፣ 27 ታኅሣሥ 2018 AM || weekday:ሰኞ|literal:፣ |day:27|literal: |month:ታኅሣሥ|literal: |year:2018|literal: |era:AM
show('en-u-ca-ethiopic', {dateStyle: 'full'}, d);
// CHECK-NEXT: Monday, Tahsas 27, 2018 AM || weekday:Monday|literal:, |month:Tahsas|literal: |day:27|literal:, |year:2018|literal: |era:AM
show('en-u-ca-ethioaa', {dateStyle: 'full'}, d);
// CHECK-NEXT: Monday, Tahsas 27, 7518 AA || weekday:Monday|literal:, |month:Tahsas|literal: |day:27|literal:, |year:7518|literal: |era:AA
show('en-u-ca-coptic', {dateStyle: 'full'}, d);
// CHECK-NEXT: Monday, Kiahk 27, 1742 AM || weekday:Monday|literal:, |month:Kiahk|literal: |day:27|literal:, |year:1742|literal: |era:AM
show('en-u-ca-indian', {dateStyle: 'full'}, d);
// CHECK-NEXT: Monday, Pausa 15, 1947 Śaka || weekday:Monday|literal:, |month:Pausa|literal: |day:15|literal:, |year:1947|literal: |era:Śaka
show('he-u-ca-hebrew', {month: 'long', day: 'numeric'}, d);
// CHECK-NEXT: 16 בטבת || day:16|literal: ב|month:טבת
show('ko-u-ca-dangi', {year: 'numeric', month: 'long', day: 'numeric'}, d);
// CHECK-NEXT: 2025년(을사년) 11월 17일 || relatedYear:2025|literal:년(|yearName:을사|literal:년) |month:11월|literal: |day:17|literal:일
// The day is "17", not CLDR's hanidays "十七" (plan decision 17).
show('zh-u-ca-chinese', {dateStyle: 'long'}, d);
// CHECK-NEXT: 2025乙巳年十一月17 || relatedYear:2025|yearName:乙巳|literal:年|month:十一月|day:17

// Every supported calendar formats the whole time range, and its parts
// join to the formatted string. (Node throws for chinese and dangi at
// 8.64e15.)
var cals = Intl.supportedValuesOf('calendar');
var problems = [];
var instants = [8.64e15, -8.64e15, 0, -1];
for (var i = 0; i < cals.length; i++) {
  for (var j = 0; j < instants.length; j++) {
    try {
      var dtf = new Intl.DateTimeFormat('en-u-ca-' + cals[i], {dateStyle: 'full', timeStyle: 'full'});
      var s = dtf.format(instants[j]);
      if (dtf.formatToParts(instants[j]).map(p => p.value).join('') !== s)
        problems.push(cals[i] + ' ' + instants[j] + ': parts do not join');
    } catch (e) {
      problems.push(cals[i] + ' ' + instants[j] + ': ' + e);
    }
  }
}
print(cals.length, problems.length === 0 ? 'ok' : problems.join('; '));
// CHECK-NEXT: 16 ok
