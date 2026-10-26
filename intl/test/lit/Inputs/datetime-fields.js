/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The field-survival check that datetime-format-fields*.js share: their RUN
// lines put this file in front of the test. For every supported calendar,
// each locale of LOCALES, each option bag of BAGS without a time zone name
// and with each of ZONES, and hour12 undefined, true and false, best fit
// must keep every requested field (the final review's finding that it
// dropped minutes and day periods): resolvedOptions reports it, and it is
// a part of formatToParts, of formatRangeToParts for two times of a day in
// Asia/Kolkata, and, in both halves, of formatRangeToParts for two days
// (which uses [[Default]] for a bag without date fields). A dayPeriod part
// is also what AM/PM shows as, so resolvedOptions is what checks a day
// period. The one exception is decision 6's: a dayPeriod requested with an
// hour is dropped in a 24-hour cycle.

var LOCALES = ['th', 'fa', 'ja', 'he', 'zh-TW', 'zh-HK', 'de', 'en', 'en-GB', 'ar'];
var BAGS = [
  {hour: 'numeric'},
  {hour: '2-digit', minute: '2-digit'},
  {hour: 'numeric', minute: 'numeric'},
  {hour: 'numeric', minute: 'numeric', second: 'numeric'},
  {minute: 'numeric'},
  {minute: 'numeric', second: 'numeric'},
  {second: 'numeric'},
  {dayPeriod: 'short'},
  {dayPeriod: 'long', hour: 'numeric'},
  {dayPeriod: 'short', hour: 'numeric', minute: 'numeric'},
  {dayPeriod: 'narrow', minute: 'numeric'},
  {dayPeriod: 'long', second: 'numeric'},
  {hour: 'numeric', minute: 'numeric', second: 'numeric', fractionalSecondDigits: 3},
  {minute: 'numeric', fractionalSecondDigits: 2},
  {fractionalSecondDigits: 1},
  {month: 'short', day: 'numeric', hour: 'numeric', minute: 'numeric'},
  {weekday: 'short', dayPeriod: 'short', hour: 'numeric'},
];
var ZONES = [undefined, 'short', 'longOffset', 'shortGeneric'];
var HOUR12 = [undefined, true, false];
/// The part type of each field.
var PART_TYPES = {
  weekday: 'weekday', era: 'era', month: 'month', day: 'day',
  dayPeriod: 'dayPeriod', hour: 'hour', minute: 'minute', second: 'second',
  fractionalSecondDigits: 'fractionalSecond', timeZoneName: 'timeZoneName',
};
var T0 = Date.UTC(2026, 0, 5, 9, 7, 3, 456);
var T1 = Date.UTC(2026, 0, 5, 15, 9, 5, 789);
var T2 = Date.UTC(2026, 0, 6, 10, 8, 4, 567);

/// Whether parts has a part of type whose source, if source is given, is
/// source.
function hasPart(parts, type, source) {
  for (var i = 0; i < parts.length; i++) {
    if (parts[i].type === type && (source === undefined || parts[i].source === source))
      return true;
  }
  return false;
}

/// The joined values of parts, for a failure's message.
function joined(parts) {
  var s = '';
  for (var i = 0; i < parts.length; i++) s += parts[i].value;
  return s;
}

/// The problems of one configuration, as a string, or '' if none.
function problemsOf(locale, calendar, bag) {
  var f = new Intl.DateTimeFormat(locale, bag);
  var r = f.resolvedOptions();
  var p0 = f.formatToParts(T0);
  var day = f.formatRangeToParts(T0, T1);
  var days = f.formatRangeToParts(T0, T2);
  var problems = '';
  for (var key in PART_TYPES) {
    if (bag[key] === undefined) continue;
    if (
      key === 'dayPeriod' &&
      bag.hour !== undefined &&
      (r.hourCycle === 'h23' || r.hourCycle === 'h24')
    )
      continue;
    var type = PART_TYPES[key];
    if (r[key] === undefined) problems += ' resolved:' + key;
    if (!hasPart(p0, type)) problems += ' format:' + key + '="' + joined(p0) + '"';
    if (!hasPart(day, type))
      problems += ' sameDay:' + key + '="' + joined(day) + '"';
    if (!hasPart(days, type, 'startRange') || !hasPart(days, type, 'endRange'))
      problems += ' twoDays:' + key + '="' + joined(days) + '"';
  }
  return problems;
}

/// Check the locales of LOCALES from index from to index to, and print each
/// problem and the totals.
function checkFieldSurvival(from, to) {
  var calendars = Intl.supportedValuesOf('calendar');
  var count = 0;
  var failures = 0;
  for (var l = from; l < to; l++) {
    for (var c = 0; c < calendars.length; c++) {
      var locale = LOCALES[l] + '-u-ca-' + calendars[c];
      for (var b = 0; b < BAGS.length; b++) {
        for (var z = 0; z < ZONES.length; z++) {
          for (var h = 0; h < HOUR12.length; h++) {
            var bag = Object.assign({timeZone: 'Asia/Kolkata'}, BAGS[b]);
            if (ZONES[z] !== undefined) bag.timeZoneName = ZONES[z];
            if (HOUR12[h] !== undefined) bag.hour12 = HOUR12[h];
            count++;
            var problems = problemsOf(locale, calendars[c], bag);
            if (problems !== '') {
              failures++;
              print('FAIL', locale, JSON.stringify(bag), problems);
            }
          }
        }
      }
    }
  }
  print(count + ' configurations, ' + failures + ' failures');
}
