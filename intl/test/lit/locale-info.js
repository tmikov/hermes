/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes %s | %FileCheck --match-full-lines %s

// The Intl.Locale information methods on locales with data. The values are
// CLDR 48.2's, as the Rust tests in intl/core/src/info.rs pin them.

/// JSON for v, or "undefined".
function show(v) {
  return v === undefined ? 'undefined' : JSON.stringify(v);
}

/// Print method name, tag and the result of new Intl.Locale(tag)[name]().
function run(name, tag) {
  print(name, tag, show(new Intl.Locale(tag)[name]()));
}

// CalendarsOfLocale: the region's calendarPreferenceData line, filtered
// through the available calendars ("islamic" and "islamic-rgsa" are not).
run('getCalendars', 'en-US');
// CHECK: getCalendars en-US ["gregory"]
run('getCalendars', 'th');
// CHECK-NEXT: getCalendars th ["buddhist","gregory"]
run('getCalendars', 'ar-SA');
// CHECK-NEXT: getCalendars ar-SA ["gregory","islamic-umalqura"]
run('getCalendars', 'fa-IR');
// CHECK-NEXT: getCalendars fa-IR ["persian","gregory","islamic-civil","islamic-tbla"]
// The region override comes before the region.
run('getCalendars', 'en-US-u-rg-thzzzz');
// CHECK-NEXT: getCalendars en-US-u-rg-thzzzz ["buddhist","gregory"]
run('getCalendars', 'ja-u-ca-japanese');
// CHECK-NEXT: getCalendars ja-u-ca-japanese ["japanese"]

// HourCyclesOfLocale: the language-region line first, then the region's.
run('getHourCycles', 'en-US');
// CHECK-NEXT: getHourCycles en-US ["h12","h23"]
run('getHourCycles', 'ja-JP');
// CHECK-NEXT: getHourCycles ja-JP ["h23","h11","h12"]
run('getHourCycles', 'hi-IN');
// CHECK-NEXT: getHourCycles hi-IN ["h12","h23"]
// "en-001" has its own line; "001" alone gives ["h23","h12"].
run('getHourCycles', 'en-001');
// CHECK-NEXT: getHourCycles en-001 ["h12","h23"]
run('getHourCycles', 'en-u-hc-h24');
// CHECK-NEXT: getHourCycles en-u-hc-h24 ["h24"]
run('getHourCycles', 'en-US-u-rg-jpzzzz');
// CHECK-NEXT: getHourCycles en-US-u-rg-jpzzzz ["h23","h11","h12"]

// WeekInfoOfLocale.
run('getWeekInfo', 'en-US');
// CHECK-NEXT: getWeekInfo en-US {"firstDay":7,"weekend":[6,7]}
run('getWeekInfo', 'en-GB');
// CHECK-NEXT: getWeekInfo en-GB {"firstDay":1,"weekend":[6,7]}
run('getWeekInfo', 'he-IL');
// CHECK-NEXT: getWeekInfo he-IL {"firstDay":7,"weekend":[5,6]}
run('getWeekInfo', 'en-US-u-rg-gbzzzz');
// CHECK-NEXT: getWeekInfo en-US-u-rg-gbzzzz {"firstDay":1,"weekend":[6,7]}
run('getWeekInfo', 'en-US-u-fw-mon');
// CHECK-NEXT: getWeekInfo en-US-u-fw-mon {"firstDay":1,"weekend":[6,7]}
// The option "0" is "sun". en-GB's own first day is Monday.
print(show(new Intl.Locale('en', {firstDayOfWeek: 0}).getWeekInfo()));
// CHECK-NEXT: {"firstDay":7,"weekend":[6,7]}
print(show(new Intl.Locale('en-GB', {firstDayOfWeek: 0}).getWeekInfo()));
// CHECK-NEXT: {"firstDay":7,"weekend":[6,7]}

// TextDirectionOfLocale: the script, or the likely one.
print('getTextInfo', 'en', new Intl.Locale('en').getTextInfo().direction);
// CHECK-NEXT: getTextInfo en ltr
print('getTextInfo', 'ar', new Intl.Locale('ar').getTextInfo().direction);
// CHECK-NEXT: getTextInfo ar rtl
print('getTextInfo', 'az-Arab', new Intl.Locale('az-Arab').getTextInfo().direction);
// CHECK-NEXT: getTextInfo az-Arab rtl

// TimeZonesOfLocale: the zones of the tag's own region, sorted.
run('getTimeZones', 'ja-JP');
// CHECK-NEXT: getTimeZones ja-JP ["Asia/Tokyo"]
run('getTimeZones', 'ar-PS');
// CHECK-NEXT: getTimeZones ar-PS ["Asia/Gaza","Asia/Hebron"]
run('getTimeZones', 'en');
// CHECK-NEXT: getTimeZones en undefined
var us = new Intl.Locale('en-US').getTimeZones();
var sorted = us.slice().sort();
print(
  'getTimeZones en-US',
  us.length > 1,
  us.every((z, i) => z === sorted[i]),
  us.indexOf('America/New_York') !== -1,
);
// CHECK-NEXT: getTimeZones en-US true true true

// NumberingSystemsOfLocale: the found locale's numbering system.
run('getNumberingSystems', 'ar-EG');
// CHECK-NEXT: getNumberingSystems ar-EG ["arab"]
run('getNumberingSystems', 'en-u-nu-thai');
// CHECK-NEXT: getNumberingSystems en-u-nu-thai ["thai"]
run('getNumberingSystems', 'fa');
// CHECK-NEXT: getNumberingSystems fa ["arabext"]

// CollationsOfLocale: the found locale's tailorings, sorted.
run('getCollations', 'de');
// CHECK-NEXT: getCollations de ["emoji","eor","phonebk"]
run('getCollations', 'en-u-co-phonebk');
// CHECK-NEXT: getCollations en-u-co-phonebk ["phonebk"]

// Every array is a fresh Array, from data and from a keyword alike.
var arrayMethods = [
  'getCalendars',
  'getCollations',
  'getHourCycles',
  'getNumberingSystems',
  'getTimeZones',
];
var plain = new Intl.Locale('en-US');
var keyed = new Intl.Locale('en-US-u-ca-gregory-co-phonebk-hc-h23-nu-latn');
for (var i = 0; i < arrayMethods.length; i++) {
  var m = arrayMethods[i];
  var a = plain[m]();
  var b = plain[m]();
  var c = keyed[m]();
  print(
    m,
    Array.isArray(a) && Object.getPrototypeOf(a) === Array.prototype,
    a !== b,
    Array.isArray(c) && keyed[m]() !== c,
  );
}
// CHECK-NEXT: getCalendars true true true
// CHECK-NEXT: getCollations true true true
// CHECK-NEXT: getHourCycles true true true
// CHECK-NEXT: getNumberingSystems true true true
// CHECK-NEXT: getTimeZones true true true

/// "<key>:<w><e><c>" for each own property of o, wec being 1 or 0 for
/// writable, enumerable and configurable data properties.
function attributes(o) {
  return Object.getOwnPropertyNames(o)
    .map(k => {
      var d = Object.getOwnPropertyDescriptor(o, k);
      return k + ':' + (+d.writable) + (+d.enumerable) + (+d.configurable);
    })
    .join(' ');
}

// The result objects inherit from Object.prototype and have writable,
// enumerable, configurable data properties; each call makes new ones.
var text = plain.getTextInfo();
print(
  'getTextInfo',
  Object.getPrototypeOf(text) === Object.prototype,
  text !== plain.getTextInfo(),
  attributes(text),
);
// CHECK-NEXT: getTextInfo true true direction:111
var week = plain.getWeekInfo();
var week2 = plain.getWeekInfo();
print(
  'getWeekInfo',
  Object.getPrototypeOf(week) === Object.prototype,
  week !== week2,
  week.weekend !== week2.weekend,
  Object.getPrototypeOf(week.weekend) === Array.prototype,
  attributes(week),
);
// CHECK-NEXT: getWeekInfo true true true true firstDay:111 weekend:111
