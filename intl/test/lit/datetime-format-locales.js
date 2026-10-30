/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: cat %S/Inputs/escape.js %S/Inputs/datetime-locales.js %s > %t.js
// RUN: %hermes -Xintl-default-locale=en-US -Xintl-default-time-zone=UTC %t.js | %FileCheck --match-full-lines %s

// Intl.DateTimeFormat's data for many locales (plan Review Focus 5, the JS
// half; Tasks 3 and 4 check every locale in Rust): 40 locales across
// scripts and regions, each with its default calendar and numbering
// system, format dateStyle and timeStyle "full" without an exception, and
// formatToParts joins to format. For the same locales and every supported
// calendar, formatMatcher "basic" finds a record for each of the draft's
// 14 required field sets ("Internal slots"): resolvedOptions has every
// field of the set and no other, except an era in a calendar with eras
// (en japanese's yyyyMd is "M/d/y GGGGG", so it adds era "narrow", as
// Node shows).

var d = Date.UTC(2026, 0, 5, 15, 7, 9, 123);

// dateStyle and timeStyle full in each locale's default calendar and
// numbering system.
var styleProblems = [];
var calendarsSeen = new Set();
var numberingSeen = new Set();
for (var i = 0; i < LOCALES.length; i++) {
  try {
    var dtf = new Intl.DateTimeFormat(LOCALES[i], {dateStyle: 'full', timeStyle: 'full'});
    var s = dtf.format(d);
    if (dtf.formatToParts(d).map(p => p.value).join('') !== s)
      styleProblems.push(LOCALES[i] + ': parts do not join');
    var ro = dtf.resolvedOptions();
    calendarsSeen.add(ro.calendar);
    numberingSeen.add(ro.numberingSystem);
  } catch (e) {
    styleProblems.push(LOCALES[i] + ': ' + e);
  }
}
print(LOCALES.length, 'locales:', styleProblems.length === 0 ? 'ok' : styleProblems.join('; '));
// CHECK: 40 locales: ok
print([...calendarsSeen].sort().join(), '|', [...numberingSeen].sort().join());
// CHECK-NEXT: buddhist,gregory,persian | adlm,arab,arabext,beng,latn,mymr
// A few of them, checked against Node 24 (ICU 78.2). Node's format writes
// U+0020 for CLDR's U+202F (ru), and its ko AM/PM names are "AM" and "PM",
// where CLDR 49's ko format names are "오전" and "오후".
function full(locale) {
  return escape(new Intl.DateTimeFormat(locale, {dateStyle: 'full', timeStyle: 'full'}).format(d));
}
print(full('fr'));
// CHECK-NEXT: lundi 5 janvier 2026 à 15:07:09 temps universel coordonné
print(full('ru'));
// CHECK-NEXT: понедельник, 5 января 2026\u202fг. в 15:07:09 Всемирное координированное время
print(full('th'));
// CHECK-NEXT: วันจันทร์ที่ 5 มกราคม พ.ศ. 2569 เวลา 15 นาฬิกา 07 นาที 09 วินาที เวลาสากลเชิงพิกัด
print(full('ko'));
// CHECK-NEXT: 2026년 1월 5일 월요일 오후 3시 7분 9초 협정 세계시

// The draft's 14 required field sets with formatMatcher "basic", for the
// first ten locales and every supported calendar (checkBasicSets);
// datetime-format-locales-2.js to -4.js check the others.
checkBasicSets(0, 10);
// CHECK-NEXT: 2240 basic cases: ok true
print(JSON.stringify(new Intl.DateTimeFormat('en-u-ca-japanese', {formatMatcher: 'basic', year: 'numeric', month: 'numeric', day: 'numeric'}).resolvedOptions()));
// CHECK-NEXT: {"locale":"en-u-ca-japanese","calendar":"japanese","numberingSystem":"latn","timeZone":"UTC","era":"narrow","year":"numeric","month":"numeric","day":"numeric"}

// A day period alone in a 24-hour cycle: the one-field {dayPeriod} record
// is in both families (plan decision 6), so it formats and is reported.
var nine = Date.UTC(2026, 0, 5, 9);
var enDayPeriod = new Intl.DateTimeFormat('en', {dayPeriod: 'short', hourCycle: 'h23'});
var deDayPeriod = new Intl.DateTimeFormat('de', {dayPeriod: 'short'});
print(enDayPeriod.format(nine), '|', deDayPeriod.format(nine));
// CHECK-NEXT: in the morning | morgens
print(enDayPeriod.resolvedOptions().dayPeriod, deDayPeriod.resolvedOptions().dayPeriod, deDayPeriod.resolvedOptions().hourCycle);
// CHECK-NEXT: short short undefined

// CLDR 49 makes h23 the preferred hour cycle of AR, CL, PY, UY and ZM
// (hour-cycles.txt); Node 24's CLDR 48 prefers h12 there and gives
// "2:05 p. m." and "2:05 pm", with hourCycle "h12".
var fourteen = Date.UTC(2026, 0, 5, 14, 5);
var esAR = new Intl.DateTimeFormat('es-AR', {hour: 'numeric', minute: 'numeric'});
var enZM = new Intl.DateTimeFormat('en-ZM', {hour: 'numeric', minute: 'numeric'});
print(esAR.format(fourteen), esAR.resolvedOptions().hourCycle, '|', enZM.format(fourteen), enZM.resolvedOptions().hourCycle);
// CHECK-NEXT: 14:05 h23 | 14:05 h23
