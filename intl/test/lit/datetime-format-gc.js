/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US -Xintl-default-time-zone=UTC %s | %FileCheck --match-full-lines %s

// DateTimeFormats freed by the GC with their native state (plan decision
// 16: the calendar, the time zone and the ICU4X names objects) and their
// internal NumberFormats (decision 12). 2,000 DateTimeFormats alternate
// locales, calendars and time zones; half of them format before they die,
// so they own internal NumberFormats and parsed patterns too. gc() runs
// every 100 formatters and every tenth formatter is kept; the survivors
// format as the first formatter of their kind did. The script ends with
// live formatters, so the runtime frees what is left when it is destroyed.
// Under ASan, LeakSanitizer and use-after-free checks cover both.

var locales = ['en', 'de', 'ja', 'ar-EG', 'zh', 'he', 'fa', 'th', 'ko', 'ru'];
var calendars = ['gregory', 'chinese', 'hebrew', 'japanese', 'islamic-umalqura', 'persian', 'iso8601'];
var timeZones = ['UTC', 'America/New_York', 'Asia/Kolkata', '+05:45', 'Australia/Lord_Howe'];
var t = Date.UTC(2026, 0, 5, 15, 7, 9, 123);

/// The locale, calendar and time zone of formatter i. The locale changes
/// every 20 formatters, the calendar every 10 and the time zone every 30,
/// so the survivors (every tenth) cover many combinations, and the cache
/// of locale data (32 entries) is emptied again and again.
function localeOf(i) {
  return locales[Math.floor(i / 20) % locales.length];
}
function calendarOf(i) {
  return calendars[Math.floor(i / 10) % calendars.length];
}
function timeZoneOf(i) {
  return timeZones[Math.floor(i / 30) % timeZones.length];
}
function keyOf(i) {
  return localeOf(i) + ' ' + calendarOf(i) + ' ' + timeZoneOf(i);
}

/// The results of the first formatter of each kind to be checked. A later
/// formatter that disagrees is wrong.
var expectedOf = Object.create(null);

/// What dtf gives for t: format, and formatToParts joined.
function results(dtf) {
  return dtf.format(t) + '|' + dtf.formatToParts(t).map(p => p.value).join('');
}
/// Whether dtf, made as formatter i, formats as the first of its kind did.
function check(dtf, i) {
  var r = results(dtf);
  if (!(keyOf(i) in expectedOf)) expectedOf[keyOf(i)] = r;
  return expectedOf[keyOf(i)] === r;
}

var kept = [];
var keptIndex = [];
var wrong = 0;
for (var i = 0; i < 2000; i++) {
  var dtf = new Intl.DateTimeFormat(localeOf(i) + '-u-ca-' + calendarOf(i), {
    dateStyle: 'full',
    timeStyle: 'full',
    timeZone: timeZoneOf(i),
  });
  if (i % 2 === 0 && !check(dtf, i)) wrong++;
  if (i % 10 === 0) {
    kept.push(dtf);
    keptIndex.push(i);
  }
  if (i % 100 === 99) gc();
}
gc();

for (var k = 0; k < kept.length; k++) {
  if (!check(kept[k], keptIndex[k])) wrong++;
  var ro = kept[k].resolvedOptions();
  if (ro.calendar !== calendarOf(keptIndex[k]) || ro.timeZone !== timeZoneOf(keptIndex[k])) wrong++;
}
print(kept.length, 'kept,', wrong, 'wrong');
// CHECK: 200 kept, 0 wrong
var combinations = new Set();
for (var q = 0; q < keptIndex.length; q++) combinations.add(keyOf(keptIndex[q]));
print(combinations.size, 'combinations');
// CHECK-NEXT: 173 combinations

// A few of the results, so a wrong result everywhere is caught too.
print(expectedOf['en gregory UTC']);
// CHECK-NEXT: Monday, January 5, 2026 at 3:07:09{{.+}}PM Coordinated Universal Time|Monday, January 5, 2026 at 3:07:09{{.+}}PM Coordinated Universal Time
// ar-EG iso8601 in Asia/Kolkata, with arab digits: iso8601 has CLDR's
// root patterns and Gregorian names (plan decision 17; ICU 78.2 prints no
// month, and ar's "في" between date and time). {{.+}} is CLDR's U+202F.
print(expectedOf[keyOf(60)]);
// CHECK-NEXT: ٢٠٢٦ يناير ٥, الاثنين ٠٨:٣٧:٠٩{{.+}}م توقيت الهند|٢٠٢٦ يناير ٥, الاثنين ٠٨:٣٧:٠٩{{.+}}م توقيت الهند
print(expectedOf[keyOf(420)]);
// CHECK-NEXT: Dienstag, 6. Januar 2026 um 02:07:09 Lord-Howe-Sommerzeit|Dienstag, 6. Januar 2026 um 02:07:09 Lord-Howe-Sommerzeit

// The survivors' bound formats outlive a collection too.
var formats = [];
for (var m = 0; m < kept.length; m++) formats.push(kept[m].format);
kept = null;
gc();
print(formats[0](t) === expectedOf[keyOf(0)].split('|')[0], formats.length);
// CHECK-NEXT: true 200
