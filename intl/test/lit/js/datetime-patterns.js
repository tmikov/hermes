/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: cat %S/Inputs/strict.js %intl_js/01-intrinsics.js %intl_js/02-shapes.js %intl_js/03-locales.js %intl_js/04-options.js %intl_js/05-tags.js %intl_js/07-number.js %intl_js/41-number-patterns.js %intl_js/51-datetime-patterns.js %intl_js/53-datetime-ranges.js %intl_js/54-datetime-names.js %s > %t.js
// RUN: %hermes_default -Xintl-plugin=false %t.js | %FileCheck --match-full-lines %s
// RUN: %hermesc -O -emit-binary -out=%t.hbc %t.js && %hermes_default -Xintl-plugin=false -b %t.hbc | %FileCheck --match-full-lines %s

// DateTimeFormat's patterns in isolation: CLDR patterns to DateTime Format
// Records, the format matchers and the style records
// (51-datetime-patterns.js), the range patterns (53-datetime-ranges.js),
// and the name symbols and the day periods (54-datetime-names.js) (plan
// decisions 4, 6, 7 and 11). 41-number-patterns.js supplies the caches.
// The fixture is what native.dateTimePatterns("en", "gregory"),
// native.dateTimePatterns("en", "chinese") and native.dayPeriods("en")
// return for CLDR 48.2.

var EN_GREGORY = [
  'EEEE, MMMM d, y', 'MMMM d, y', 'MMM d, y', 'M/d/yy', 'h:mm:ss\u202fa zzzz',
  'h:mm:ss\u202fa z', 'h:mm:ss\u202fa', 'h:mm\u202fa', '{1}, {0}', '{1}, {0}',
  '{1}, {0}', '{1}, {0}', '{1} \'at\' {0}', '{1} \'at\' {0}', '{1}, {0}',
  '{1}, {0}', '{0}\u2009\u2013\u2009{1}', '', '{0} {1}', '49', 'Bh', 'h B',
  'Bhm', 'h:mm B', 'Bhms', 'h:mm:ss B', 'd', 'd', 'E', 'ccc', 'EBh', 'E h B',
  'EBhm', 'E h:mm B', 'EBhms', 'E h:mm:ss B', 'Ed', 'd E', 'Eh', 'E h\u202fa',
  'Ehm', 'E h:mm\u202fa', 'EHm', 'E HH:mm', 'Ehms', 'E h:mm:ss\u202fa', 'EHms',
  'E HH:mm:ss', 'Gy', 'y G', 'GyM', 'M/y G', 'GyMd', 'M/d/y G', 'GyMEd',
  'E, M/d/y G', 'GyMMM', 'MMM y G', 'GyMMMd', 'MMM d, y G', 'GyMMMEd',
  'E, MMM d, y G', 'h', 'h\u202fa', 'H', 'HH', 'hm', 'h:mm\u202fa', 'Hm',
  'HH:mm', 'hms', 'h:mm:ss\u202fa', 'Hms', 'HH:mm:ss', 'hmsv',
  'h:mm:ss\u202fa v', 'Hmsv', 'HH:mm:ss v', 'hmv', 'h:mm\u202fa v', 'Hmv',
  'HH:mm v', 'hv', 'h\u202fa v', 'Hv', 'HH v', 'M', 'L', 'Md', 'M/d', 'MEd',
  'E, M/d', 'MMM', 'LLL', 'MMMd', 'MMM d', 'MMMEd', 'E, MMM d', 'MMMMd',
  'MMMM d', 'ms', 'mm:ss', 'y', 'y', 'yM', 'M/y', 'yMd', 'M/d/y', 'yMEd',
  'E, M/d/y', 'yMMM', 'MMM y', 'yMMMd', 'MMM d, y', 'yMMMEd', 'E, MMM d, y',
  'yMMMM', 'MMMM y', '75', 'Bh', 'B', 'h B\u2009\u2013\u2009h B', 'Bh', 'h',
  'h\u2009\u2013\u2009h B', 'Bhm', 'B', 'h:mm B\u2009\u2013\u2009h:mm B',
  'Bhm', 'h', 'h:mm\u2009\u2013\u2009h:mm B', 'Bhm', 'm',
  'h:mm\u2009\u2013\u2009h:mm B', 'd', 'd', 'd\u2009\u2013\u2009d', 'Gy', 'G',
  'y G\u2009\u2013\u2009y G', 'Gy', 'y', 'y\u2009\u2013\u2009y G', 'GyM', 'G',
  'M/y G\u2009\u2013\u2009M/y G', 'GyM', 'M', 'M/y\u2009\u2013\u2009M/y G',
  'GyM', 'y', 'M/y\u2009\u2013\u2009M/y G', 'GyMd', 'd',
  'M/d/y\u2009\u2013\u2009M/d/y G', 'GyMd', 'G',
  'M/d/y G\u2009\u2013\u2009M/d/y G', 'GyMd', 'M',
  'M/d/y\u2009\u2013\u2009M/d/y G', 'GyMd', 'y',
  'M/d/y\u2009\u2013\u2009M/d/y G', 'GyMEd', 'd',
  'E, M/d/y\u2009\u2013\u2009E, M/d/y G', 'GyMEd', 'G',
  'E, M/d/y G\u2009\u2013\u2009E, M/d/y G', 'GyMEd', 'M',
  'E, M/d/y\u2009\u2013\u2009E, M/d/y G', 'GyMEd', 'y',
  'E, M/d/y\u2009\u2013\u2009E, M/d/y G', 'GyMMM', 'G',
  'MMM y G\u2009\u2013\u2009MMM y G', 'GyMMM', 'M',
  'MMM\u2009\u2013\u2009MMM y G', 'GyMMM', 'y',
  'MMM y\u2009\u2013\u2009MMM y G', 'GyMMMd', 'd',
  'MMM d\u2009\u2013\u2009d, y G', 'GyMMMd', 'G',
  'MMM d, y G\u2009\u2013\u2009MMM d, y G', 'GyMMMd', 'M',
  'MMM d\u2009\u2013\u2009MMM d, y G', 'GyMMMd', 'y',
  'MMM d, y\u2009\u2013\u2009MMM d, y G', 'GyMMMEd', 'd',
  'E, MMM d\u2009\u2013\u2009E, MMM d, y G', 'GyMMMEd', 'G',
  'E, MMM d, y G\u2009\u2013\u2009E, MMM d, y G', 'GyMMMEd', 'M',
  'E, MMM d\u2009\u2013\u2009E, MMM d, y G', 'GyMMMEd', 'y',
  'E, MMM d, y\u2009\u2013\u2009E, MMM d, y G', 'h', 'a',
  'h\u202fa\u2009\u2013\u2009h\u202fa', 'h', 'h',
  'h\u2009\u2013\u2009h\u202fa', 'H', 'H', 'HH\u2009\u2013\u2009HH', 'hm', 'a',
  'h:mm\u202fa\u2009\u2013\u2009h:mm\u202fa', 'hm', 'h',
  'h:mm\u2009\u2013\u2009h:mm\u202fa', 'hm', 'm',
  'h:mm\u2009\u2013\u2009h:mm\u202fa', 'Hm', 'H',
  'HH:mm\u2009\u2013\u2009HH:mm', 'Hm', 'm', 'HH:mm\u2009\u2013\u2009HH:mm',
  'hmv', 'a', 'h:mm\u202fa\u2009\u2013\u2009h:mm\u202fa v', 'hmv', 'h',
  'h:mm\u2009\u2013\u2009h:mm\u202fa v', 'hmv', 'm',
  'h:mm\u2009\u2013\u2009h:mm\u202fa v', 'Hmv', 'H',
  'HH:mm\u2009\u2013\u2009HH:mm v', 'Hmv', 'm',
  'HH:mm\u2009\u2013\u2009HH:mm v', 'hv', 'a',
  'h\u202fa\u2009\u2013\u2009h\u202fa v', 'hv', 'h',
  'h\u2009\u2013\u2009h\u202fa v', 'Hv', 'H', 'HH\u2009\u2013\u2009HH v', 'M',
  'M', 'M\u2009\u2013\u2009M', 'Md', 'd', 'M/d\u2009\u2013\u2009M/d', 'Md',
  'M', 'M/d\u2009\u2013\u2009M/d', 'MEd', 'd',
  'E, M/d\u2009\u2013\u2009E, M/d', 'MEd', 'M',
  'E, M/d\u2009\u2013\u2009E, M/d', 'MMM', 'M', 'MMM\u2009\u2013\u2009MMM',
  'MMMd', 'd', 'MMM d\u2009\u2013\u2009d', 'MMMd', 'M',
  'MMM d\u2009\u2013\u2009MMM d', 'MMMEd', 'd',
  'E, MMM d\u2009\u2013\u2009E, MMM d', 'MMMEd', 'M',
  'E, MMM d\u2009\u2013\u2009E, MMM d', 'y', 'y', 'y\u2009\u2013\u2009y', 'yM',
  'M', 'M/y\u2009\u2013\u2009M/y', 'yM', 'y', 'M/y\u2009\u2013\u2009M/y',
  'yMd', 'd', 'M/d/y\u2009\u2013\u2009M/d/y', 'yMd', 'M',
  'M/d/y\u2009\u2013\u2009M/d/y', 'yMd', 'y', 'M/d/y\u2009\u2013\u2009M/d/y',
  'yMEd', 'd', 'E, M/d/y\u2009\u2013\u2009E, M/d/y', 'yMEd', 'M',
  'E, M/d/y\u2009\u2013\u2009E, M/d/y', 'yMEd', 'y',
  'E, M/d/y\u2009\u2013\u2009E, M/d/y', 'yMMM', 'M',
  'MMM\u2009\u2013\u2009MMM y', 'yMMM', 'y', 'MMM y\u2009\u2013\u2009MMM y',
  'yMMMd', 'd', 'MMM d\u2009\u2013\u2009d, y', 'yMMMd', 'M',
  'MMM d\u2009\u2013\u2009MMM d, y', 'yMMMd', 'y',
  'MMM d, y\u2009\u2013\u2009MMM d, y', 'yMMMEd', 'd',
  'E, MMM d\u2009\u2013\u2009E, MMM d, y', 'yMMMEd', 'M',
  'E, MMM d\u2009\u2013\u2009E, MMM d, y', 'yMMMEd', 'y',
  'E, MMM d, y\u2009\u2013\u2009E, MMM d, y', 'yMMMM', 'M',
  'MMMM\u2009\u2013\u2009MMMM y', 'yMMMM', 'y',
  'MMMM y\u2009\u2013\u2009MMMM y',
];
var EN_CHINESE = [
  'EEEE, MMMM d, r(U)', 'MMMM d, r(U)', 'MMM d, r', 'M/d/r',
  'h:mm:ss\u202fa zzzz', 'h:mm:ss\u202fa z', 'h:mm:ss\u202fa', 'h:mm\u202fa',
  '{1}, {0}', '{1}, {0}', '{1}, {0}', '{1}, {0}', '{1} \'at\' {0}',
  '{1} \'at\' {0}', '{1}, {0}', '{1}, {0}', '{0}\u2009\u2013\u2009{1}',
  '{0}bis', '{0} {1}', '52', 'Bh', 'h B', 'Bhm', 'h:mm B', 'Bhms', 'h:mm:ss B',
  'd', 'd', 'E', 'ccc', 'EBh', 'E h B', 'EBhm', 'E h:mm B', 'EBhms',
  'E h:mm:ss B', 'Ed', 'd E', 'Gy', 'r(U)', 'GyMMM', 'MMM r', 'GyMMMd',
  'MMM d, r', 'GyMMMEd', 'E, MMM d, r', 'GyMMMM', 'MMMM r(U)', 'GyMMMMd',
  'MMMM d, r(U)', 'GyMMMMEd', 'E, MMMM d, r(U)', 'h', 'h\u202fa', 'H', 'HH',
  'hm', 'h:mm\u202fa', 'Hm', 'HH:mm', 'hms', 'h:mm:ss\u202fa', 'Hms',
  'HH:mm:ss', 'hv', 'h\u202fa v', 'Hv', 'HH v', 'M', 'L', 'Md', 'M/d', 'MEd',
  'E, M/d', 'MMM', 'LLL', 'MMMd', 'MMM d', 'MMMEd', 'E, MMM d', 'MMMMd',
  'MMMM d', 'ms', 'mm:ss', 'UM', 'M/U', 'UMd', 'M/d/U', 'UMMM', 'MMM U',
  'UMMMd', 'MMM d, U', 'y', 'r(U)', 'yyyy', 'r(U)', 'yyyyM', 'M/r', 'yyyyMd',
  'M/d/r', 'yyyyMEd', 'E, M/d/r', 'yyyyMMM', 'MMM r', 'yyyyMMMd', 'MMM d, r',
  'yyyyMMMEd', 'E, MMM d, r', 'yyyyMMMM', 'MMMM r(U)', 'yyyyMMMMd',
  'MMMM d, r(U)', 'yyyyMMMMEd', 'E, MMMM d, r(U)', 'Eh', 'E h\u202fa', 'Ehm',
  'E h:mm\u202fa', 'EHm', 'E HH:mm', 'Ehms', 'E h:mm:ss\u202fa', 'EHms',
  'E HH:mm:ss', '51', 'Bh', 'B', 'h B\u2009\u2013\u2009h B', 'Bh', 'h',
  'h\u2009\u2013\u2009h B', 'Bhm', 'B', 'h:mm B\u2009\u2013\u2009h:mm B',
  'Bhm', 'h', 'h:mm\u2009\u2013\u2009h:mm B', 'Bhm', 'm',
  'h:mm\u2009\u2013\u2009h:mm B', 'd', 'd', 'd\u2009\u2013\u2009d', 'h', 'a',
  'h\u202fa\u2009\u2013\u2009h\u202fa', 'h', 'h',
  'h\u2009\u2013\u2009h\u202fa', 'H', 'H', 'HH\u2009\u2013\u2009HH', 'hm', 'a',
  'h:mm\u202fa\u2009\u2013\u2009h:mm\u202fa', 'hm', 'h',
  'h:mm\u2009\u2013\u2009h:mm\u202fa', 'hm', 'm',
  'h:mm\u2009\u2013\u2009h:mm\u202fa', 'Hm', 'H',
  'HH:mm\u2009\u2013\u2009HH:mm', 'Hm', 'm', 'HH:mm\u2009\u2013\u2009HH:mm',
  'hmv', 'a', 'h:mm\u202fa\u2009\u2013\u2009h:mm\u202fa v', 'hmv', 'h',
  'h:mm\u2009\u2013\u2009h:mm\u202fa v', 'hmv', 'm',
  'h:mm\u2009\u2013\u2009h:mm\u202fa v', 'Hmv', 'H',
  'HH:mm\u2009\u2013\u2009HH:mm v', 'Hmv', 'm',
  'HH:mm\u2009\u2013\u2009HH:mm v', 'hv', 'a',
  'h\u202fa\u2009\u2013\u2009h\u202fa v', 'hv', 'h',
  'h\u2009\u2013\u2009h\u202fa v', 'Hv', 'H', 'HH\u2009\u2013\u2009HH v', 'M',
  'M', 'M\u2009\u2013\u2009M', 'Md', 'd', 'M/d\u2009\u2013\u2009M/d', 'Md',
  'M', 'M/d\u2009\u2013\u2009M/d', 'MEd', 'd',
  'E, M/d\u2009\u2013\u2009E, M/d', 'MEd', 'M',
  'E, M/d\u2009\u2013\u2009E, M/d', 'MMM', 'M', 'MMM\u2009\u2013\u2009MMM',
  'MMMd', 'd', 'MMM d\u2009\u2013\u2009d', 'MMMd', 'M',
  'MMM d\u2009\u2013\u2009MMM d', 'MMMEd', 'd',
  'E, MMM d\u2009\u2013\u2009E, MMM d', 'MMMEd', 'M',
  'E, MMM d\u2009\u2013\u2009E, MMM d', 'y', 'y', 'U\u2009\u2013\u2009U', 'yM',
  'M', 'M/y\u2009\u2013\u2009M/y', 'yM', 'y', 'M/y\u2009\u2013\u2009M/y',
  'yMd', 'd', 'M/d/y\u2009\u2013\u2009M/d/y', 'yMd', 'M',
  'M/d/y\u2009\u2013\u2009M/d/y', 'yMd', 'y', 'M/d/y\u2009\u2013\u2009M/d/y',
  'yMEd', 'd', 'E, M/d/y\u2009\u2013\u2009E, M/d/y', 'yMEd', 'M',
  'E, M/d/y\u2009\u2013\u2009E, M/d/y', 'yMEd', 'y',
  'E, M/d/y\u2009\u2013\u2009E, M/d/y', 'yMMM', 'M',
  'MMM\u2009\u2013\u2009MMM U', 'yMMM', 'y', 'MMM U\u2009\u2013\u2009MMM U',
  'yMMMd', 'd', 'MMM d\u2009\u2013\u2009d, U', 'yMMMd', 'M',
  'MMM d\u2009\u2013\u2009MMM d, U', 'yMMMd', 'y',
  'MMM d, U\u2009\u2013\u2009MMM d, U', 'yMMMEd', 'd',
  'E, MMM d\u2009\u2013\u2009E, MMM d, U', 'yMMMEd', 'M',
  'E, MMM d\u2009\u2013\u2009E, MMM d, U', 'yMMMEd', 'y',
  'E, MMM d, U\u2009\u2013\u2009E, MMM d, U', 'yMMMM', 'M',
  'MMMM\u2009\u2013\u2009MMMM U', 'yMMMM', 'y',
  'MMMM U\u2009\u2013\u2009MMMM U',
];
var EN_DAY_PERIODS = [
  '4', 'morning1', '0', '720', 'in the morning', 'in the morning',
  'in the morning', 'afternoon1', '720', '1080', 'in the afternoon',
  'in the afternoon', 'in the afternoon', 'evening1', '1080', '1260',
  'in the evening', 'in the evening', 'in the evening', 'night1', '1260',
  '1440', 'at night', 'at night', 'at night', 'noon', 'noon', 'n',
];

/// s with every code unit outside printable ASCII as \uXXXX.
function esc(s) {
  var out = '';
  for (var i = 0; i < s.length; i++) {
    var c = s.charCodeAt(i);
    if (c >= 0x20 && c <= 0x7e) out += s[i];
    else out += '\\u' + (c + 0x10000).toString(16).slice(1);
  }
  return out;
}
/// JSON of v, escaped.
function show(v) {
  return esc(JSON.stringify(v));
}
/// The fields of a DateTime Format Record in the draft's table order, its
/// pattern, then the flags it has.
function rec(r) {
  var o = {};
  var keys = [
    'weekday', 'era', 'year', 'month', 'day', 'dayPeriod', 'hour', 'minute',
    'second', 'fractionalSecondDigits', 'timeZoneName',
  ];
  for (var i = 0; i < keys.length; i++) {
    if (r[keys[i]] !== undefined) o[keys[i]] = r[keys[i]];
  }
  o.pattern = r.pattern;
  if (r.pattern12 !== r.pattern) o.pattern12 = r.pattern12;
  if (r.monthStandalone) o.monthStandalone = true;
  if (r.weekdayStandalone) o.weekdayStandalone = true;
  if (r.ampmWidth !== undefined) o.ampmWidth = r.ampmWidth;
  if (r.dayPeriodWidth !== undefined) o.dayPeriodWidth = r.dayPeriodWidth;
  if (r.hourLetter !== undefined) o.hourLetter = r.hourLetter;
  return show(o);
}
/// A null-prototype request record with the fields of o.
function req(o) {
  var r = {__proto__: null};
  for (var k in o) r[k] = o[k];
  return r;
}
var EN = dateTimeLocaleData(EN_GREGORY, 'gregory', '.');
var EN_CN = dateTimeLocaleData(EN_CHINESE, 'chinese', '.');
/// The best-fit record for o in family.
function bf(o, family = 'h12', data = EN) {
  return BestFitFormatMatcher(req(o), data, family);
}
/// The parts of a DateTime Range Pattern Format Record, each as
/// source:"pattern".
function parts(r) {
  var out = [];
  for (var i = 0; i < r.PatternParts.length; i++) {
    var part = r.PatternParts[i];
    out.push(part.Source + ':' + show(part.Pattern));
  }
  return out.join(' ');
}
var RANGE_NAMES = [
  'Default', 'Era', 'Year', 'Month', 'Day', 'AmPm', 'DayPeriod', 'Hour',
  'Minute', 'Second', 'FractionalSecondDigits',
];
/// The fields a DateTime Range Pattern Record has, in the draft's order.
function rangeFields(r) {
  var out = [];
  for (var i = 0; i < RANGE_NAMES.length; i++) {
    if (r[RANGE_NAMES[i]] !== undefined) out.push(RANGE_NAMES[i]);
  }
  return out.join(' ');
}
/// The range patterns of the best-fit record for o in family.
function ranges(o, family = 'h12', data = EN) {
  return rangePatternsOf(bf(o, family, data), data, family);
}

// Records (decision 6's table).
print(rec(cldrPatternToRecord('MMM d, y')));
// CHECK: {"year":"numeric","month":"short","day":"numeric","pattern":"{month} {day}, {year}"}
print(rec(cldrPatternToRecord('h:mm a')));
// CHECK-NEXT: {"hour":"numeric","minute":"2-digit","pattern":"{hour}:{minute} {ampm}","ampmWidth":"abbreviated","hourLetter":"h"}
print(rec(cldrPatternToRecord('LLLL')));
// CHECK-NEXT: {"month":"long","pattern":"{month}","monthStandalone":true}
print(rec(cldrPatternToRecord("EEEE, MMMM d, y 'at' h:mm a")));
// CHECK-NEXT: {"weekday":"long","year":"numeric","month":"long","day":"numeric","hour":"numeric","minute":"2-digit","pattern":"{weekday}, {month} {day}, {year} at {hour}:{minute} {ampm}","ampmWidth":"abbreviated","hourLetter":"h"}
print(rec(cldrPatternToRecord('r(U)')));
// CHECK-NEXT: {"year":"numeric","pattern":"{relatedYear}({yearName})"}
print(rec(cldrPatternToRecord("h 'o''clock' a")));
// CHECK-NEXT: {"hour":"numeric","pattern":"{hour} o'clock {ampm}","ampmWidth":"abbreviated","hourLetter":"h"}
print(rec(cldrPatternToRecord("HH''mm")));
// CHECK-NEXT: {"hour":"2-digit","minute":"2-digit","pattern":"{hour}'{minute}","hourLetter":"H"}
print(rec(cldrPatternToRecord('ccc GGGGG yy SSSSS OOOO')));
// CHECK-NEXT: {"weekday":"short","era":"narrow","year":"2-digit","fractionalSecondDigits":3,"timeZoneName":"longOffset","pattern":"{weekday} {era} {year} {fractionalSecondDigits} {timeZoneName}","weekdayStandalone":true}
print(rec(cldrPatternToRecord('EEEEEE G zzz vvvv K:s bbbbb')));
// CHECK-NEXT: {"weekday":"short","era":"short","hour":"numeric","second":"numeric","timeZoneName":"short","pattern":"{weekday} {era} {timeZoneName} {timeZoneName} {hour}:{second} {ampm}","ampmWidth":"narrow","hourLetter":"K"}
print(rec(cldrPatternToRecord('h:mm\u202f')));
// CHECK-NEXT: {"hour":"numeric","minute":"2-digit","pattern":"{hour}:{minute}\u202f","hourLetter":"h"}
// A B the skeleton asks for gives a dayPeriod field; one it does not
// formats as the day period but adds no field.
print(rec(cldrPatternToRecord('h B', 'gregory', 'Bh')));
// CHECK-NEXT: {"dayPeriod":"short","hour":"numeric","pattern":"{hour} {dayPeriod}","dayPeriodWidth":"short","hourLetter":"h"}
print(rec(cldrPatternToRecord('h B', 'gregory', 'h')));
// CHECK-NEXT: {"hour":"numeric","pattern":"{hour} {dayPeriod}","dayPeriodWidth":"short","hourLetter":"h"}

// Best fit (decision 6), en, family h12 unless stated.
print(rec(bf({month: '2-digit', day: '2-digit'})));
// CHECK-NEXT: {"month":"2-digit","day":"2-digit","pattern":"{month}/{day}"}
print(rec(bf({hour: '2-digit', minute: '2-digit'})));
// CHECK-NEXT: {"hour":"2-digit","minute":"2-digit","pattern":"{hour}:{minute}\u202f{ampm}","ampmWidth":"abbreviated","hourLetter":"h"}
print(rec(bf({hour: 'numeric', minute: 'numeric'}, 'h24')));
// CHECK-NEXT: {"hour":"2-digit","minute":"2-digit","pattern":"{hour}:{minute}","hourLetter":"H"}
print(rec(bf({month: 'long', minute: 'numeric'})));
// CHECK-NEXT: {"month":"long","minute":"numeric","pattern":"{month} at {minute}","monthStandalone":true}
var FULL = {
  weekday: 'long', year: 'numeric', month: 'long', day: 'numeric',
  hour: 'numeric', minute: 'numeric',
};
print(rec(bf(FULL)));
// CHECK-NEXT: {"weekday":"long","year":"numeric","month":"long","day":"numeric","hour":"numeric","minute":"2-digit","pattern":"{weekday}, {month} {day}, {year} at {hour}:{minute}\u202f{ampm}","ampmWidth":"abbreviated","hourLetter":"h"}
var MEDIUM = {
  year: 'numeric', month: 'short', day: 'numeric', hour: 'numeric',
  minute: 'numeric',
};
print(rec(bf(MEDIUM)));
// CHECK-NEXT: {"year":"numeric","month":"short","day":"numeric","hour":"numeric","minute":"2-digit","pattern":"{month} {day}, {year}, {hour}:{minute}\u202f{ampm}","ampmWidth":"abbreviated","hourLetter":"h"}
print(rec(bf({hour: 'numeric', timeZoneName: 'long'})));
// CHECK-NEXT: {"hour":"numeric","timeZoneName":"long","pattern":"{hour}\u202f{ampm} {timeZoneName}","ampmWidth":"abbreviated","hourLetter":"h"}
print(rec(bf({minute: 'numeric', fractionalSecondDigits: 2})));
// CHECK-NEXT: {"minute":"2-digit","second":"2-digit","fractionalSecondDigits":2,"pattern":"{minute}:{second}.{fractionalSecondDigits}"}
print(rec(bf({minute: 'numeric'})));
// CHECK-NEXT: {"minute":"numeric","pattern":"{minute}"}
print(rec(bf({fractionalSecondDigits: 1})));
// CHECK-NEXT: {"fractionalSecondDigits":1,"pattern":"{fractionalSecondDigits}"}
print(rec(bf({fractionalSecondDigits: 2})));
// CHECK-NEXT: {"fractionalSecondDigits":2,"pattern":"{fractionalSecondDigits}"}
// A fraction with no hour, minute or second to follow comes first, joined
// by a space, as ICU4C gives ("04 in the morning").
print(rec(bf({dayPeriod: 'short', fractionalSecondDigits: 2})));
// CHECK-NEXT: {"dayPeriod":"short","fractionalSecondDigits":2,"pattern":"{fractionalSecondDigits} {dayPeriod}","dayPeriodWidth":"short"}
print(rec(bf({dayPeriod: 'long', fractionalSecondDigits: 1}, 'h24')));
// CHECK-NEXT: {"dayPeriod":"long","fractionalSecondDigits":1,"pattern":"{fractionalSecondDigits} {dayPeriod}","dayPeriodWidth":"long"}
// The width adjustment of [[dayPeriod]] is the width {dayPeriod} shows.
print(rec(bf({dayPeriod: 'narrow', hour: 'numeric'})));
// CHECK-NEXT: {"dayPeriod":"narrow","hour":"numeric","pattern":"{hour} {dayPeriod}","dayPeriodWidth":"narrow","hourLetter":"h"}
print(rec(bf({weekday: 'short', hour: 'numeric', minute: 'numeric'})));
// CHECK-NEXT: {"weekday":"short","hour":"numeric","minute":"2-digit","pattern":"{weekday} {hour}:{minute}\u202f{ampm}","ampmWidth":"abbreviated","hourLetter":"h"}
// A {timeZoneName} one-field record does not beat "HH" by dropping the
// hour, and there is one of each width.
print(rec(bf({hour: 'numeric', timeZoneName: 'short'}, 'h24')));
// CHECK-NEXT: {"hour":"2-digit","timeZoneName":"short","pattern":"{hour} {timeZoneName}","hourLetter":"H"}
print(
  rec(
    bf({
      year: 'numeric', month: 'numeric', day: 'numeric',
      timeZoneName: 'shortOffset',
    }),
  ),
);
// CHECK-NEXT: {"year":"numeric","month":"numeric","day":"numeric","timeZoneName":"shortOffset","pattern":"{month}/{day}/{year}, {timeZoneName}"}
print(rec(bf({timeZoneName: 'longGeneric'})));
// CHECK-NEXT: {"timeZoneName":"longGeneric","pattern":"{timeZoneName}"}
// Best fit charges a numeric field shown as text 13 more than the draft's
// penalty, so en-GB-like "dd/MM/y" (-12) beats "d MMM y" (-6 + -13) for
// {year, month, day: "numeric"}; basic keeps the draft's penalties.
/// list with the pattern of its availableFormats entry skeleton replaced.
function withAvailable(list, skeleton, pattern) {
  var copy = list.slice();
  for (var i = 20; i < 20 + 2 * +copy[19]; i += 2) {
    if (copy[i] === skeleton) copy[i + 1] = pattern;
  }
  return copy;
}
var GB = dateTimeLocaleData(
  withAvailable(withAvailable(EN_GREGORY, 'yMd', 'dd/MM/y'), 'yMMMd',
    'd MMM y'),
  'gregory',
  '.',
);
var YMD = {year: 'numeric', month: 'numeric', day: 'numeric'};
print(rec(bf(YMD, 'h12', GB)));
// CHECK-NEXT: {"year":"numeric","month":"2-digit","day":"2-digit","pattern":"{day}/{month}/{year}"}
print(rec(BasicFormatMatcher(req(YMD), formatsOf(GB, 'h12'))));
// CHECK-NEXT: {"year":"numeric","month":"short","day":"numeric","pattern":"{day} {month} {year}"}
// [[Year]] of {day} adds a numeric month with the year: en-GB-like data
// gives the dd/MM/y date, not best fit's text month for {day, year}.
var gbDay = rangePatternsOf(bf({day: 'numeric'}, 'h12', GB), GB, 'h12');
print(parts(gbDay.Year), rec(gbDay.Year));
// CHECK-NEXT: startRange:"{day}/{month}/{year}" shared:"\u2009\u2013\u2009" endRange:"{day}/{month}/{year}" {"year":"numeric","month":"2-digit","day":"2-digit"}
// Best fit matches on the skeleton (refinements of decision 6, from the
// Task 7 review). A: zh-like data, where yMMMMd and MMMd show numeric
// months; the skeleton's long or short month wins over "y/M/d" and "M/d".
/// list with availableFormats entry (skeleton, pattern) added at the end.
function withNewAvailable(list, skeleton, pattern) {
  var end = 20 + 2 * +list[19];
  var copy = list.slice(0, end);
  copy.push(skeleton, pattern);
  copy[19] = String(+list[19] + 1);
  return copy.concat(list.slice(end));
}
var ZH = dateTimeLocaleData(
  withNewAvailable(
    withAvailable(
      withAvailable(
        withAvailable(EN_GREGORY, 'yMd', 'y/M/d'),
        'Md',
        'M/d',
      ),
      'MMMd',
      'M\u6708d\u65e5',
    ),
    'yMMMMd',
    'y\u5e74M\u6708d\u65e5',
  ),
  'gregory',
  '.',
);
print(rec(bf({year: 'numeric', month: 'long', day: 'numeric'}, 'h12', ZH)));
// CHECK-NEXT: {"year":"numeric","month":"numeric","day":"numeric","pattern":"{year}\u5e74{month}\u6708{day}\u65e5"}
print(rec(bf({month: 'short', day: 'numeric'}, 'h12', ZH)));
// CHECK-NEXT: {"month":"numeric","day":"numeric","pattern":"{month}\u6708{day}\u65e5"}
// B: an era calendar whose y is "y G": its skeleton is the one-field year
// record, so no {year} is added, and {year} shows the era.
var ERA_Y = dateTimeLocaleData(
  withAvailable(EN_GREGORY, 'y', 'y G'),
  'japanese',
  '.',
);
print(rec(bf({year: 'numeric'}, 'h12', ERA_Y)));
// CHECK-NEXT: {"era":"short","year":"numeric","pattern":"{year} {era}"}
// [[formats]] has no {year} record of its own then, so basic, which scores
// the pattern's fields, also takes "y G".
print(
  rec(BasicFormatMatcher(req({year: 'numeric'}), formatsOf(ERA_Y, 'h12'))),
);
// CHECK-NEXT: {"era":"short","year":"numeric","pattern":"{year} {era}"}
// C: a best record with a field the request lacks is split when the
// request has date and time fields: {weekday: "long", hour} in h24 is
// E + H glued, not EHm's "E HH:mm".
print(rec(bf({weekday: 'long', hour: 'numeric'}, 'h24')));
// CHECK-NEXT: {"weekday":"long","hour":"2-digit","pattern":"{weekday}, {hour}","weekdayStandalone":true,"hourLetter":"H"}
// A date-only request keeps a best record with an extra field: MMMEd
// for {weekday, month}.
print(rec(bf({weekday: 'short', month: 'short'})));
// CHECK-NEXT: {"weekday":"short","month":"short","day":"numeric","pattern":"{weekday}, {month} {day}"}
// D: a timeZoneName of another kind is a small width difference, so a
// zh-Hant-like hmsv "ah:mm:ss [v]" shows {timeZoneName: "short"} in its
// own layout.
var HANT = dateTimeLocaleData(
  withAvailable(EN_GREGORY, 'hmsv', 'ah:mm:ss [v]'),
  'gregory',
  '.',
);
print(
  rec(
    bf(
      {
        hour: 'numeric', minute: 'numeric', second: 'numeric',
        timeZoneName: 'short',
      },
      'h12',
      HANT,
    ),
  ),
);
// CHECK-NEXT: {"hour":"numeric","minute":"2-digit","second":"2-digit","timeZoneName":"short","pattern":"{ampm}{hour}:{minute}:{second} [{timeZoneName}]","ampmWidth":"abbreviated","hourLetter":"h"}
// Width adjustment by the matched skeleton, comparing letter counts as
// ICU does (a refinement of decision 6): a cs-like yMMM "LLLL y" stays
// long for {month: "short"} ("MMM", as the skeleton), and an eu-like Ed
// "d, EEEE" becomes short for {weekday: "short"} ("EEE", not Ed's "E").
var CS = dateTimeLocaleData(
  withAvailable(withAvailable(EN_GREGORY, 'yMMM', 'LLLL y'), 'Ed', 'd, EEEE'),
  'gregory',
  '.',
);
print(rec(bf({year: 'numeric', month: 'short'}, 'h12', CS)));
// CHECK-NEXT: {"year":"numeric","month":"long","pattern":"{month} {year}","monthStandalone":true}
print(rec(bf({weekday: 'short', day: 'numeric'}, 'h12', CS)));
// CHECK-NEXT: {"weekday":"short","day":"numeric","pattern":"{day}, {weekday}"}
print(rec(bf({weekday: 'long', day: 'numeric'}, 'h12', CS)));
// CHECK-NEXT: {"weekday":"long","day":"numeric","pattern":"{day}, {weekday}"}
// A 2-digit request adjusts a numeric skeleton (h, d), a numeric one keeps
// the pattern's 2-digit field (H's "HH", m's "mm").
print(rec(bf({hour: '2-digit'})), rec(bf({hour: 'numeric'}, 'h24')));
// CHECK-NEXT: {"hour":"2-digit","pattern":"{hour}\u202f{ampm}","ampmWidth":"abbreviated","hourLetter":"h"} {"hour":"2-digit","pattern":"{hour}","hourLetter":"H"}
print(
  rec(bf({day: '2-digit'})),
  rec(bf({minute: 'numeric', second: 'numeric'})),
);
// CHECK-NEXT: {"day":"2-digit","pattern":"{day}"} {"minute":"2-digit","second":"2-digit","pattern":"{minute}:{second}"}
// The glue's connector follows the requested month, not the record's: a
// he-like MMMd "d MMMM" for {month: "short", day} with a time takes the
// medium connector ", ", not the long atTime " at ".
var HE = dateTimeLocaleData(
  withAvailable(EN_GREGORY, 'MMMd', 'd MMMM'),
  'gregory',
  '.',
);
print(
  rec(
    bf(
      {month: 'short', day: 'numeric', hour: 'numeric', minute: 'numeric'},
      'h12',
      HE,
    ),
  ),
);
// CHECK-NEXT: {"month":"long","day":"numeric","hour":"numeric","minute":"2-digit","pattern":"{day} {month}, {hour}:{minute}\u202f{ampm}","ampmWidth":"abbreviated","hourLetter":"h"}
// Equal scores go to the skeleton first in code point order, not CLDR's:
// for {weekday: "long", month: "long", day}, added MMMMEd "E d MMMM" and
// MMMEEEEd "EEEE, MMM d" both score -6, and MMMEEEEd sorts first.
var TIE = dateTimeLocaleData(
  withNewAvailable(
    withNewAvailable(EN_GREGORY, 'MMMMEd', 'E d MMMM'),
    'MMMEEEEd',
    'EEEE, MMM d',
  ),
  'gregory',
  '.',
);
print(
  rec(bf({weekday: 'long', month: 'long', day: 'numeric'}, 'h12', TIE)),
);
// CHECK-NEXT: {"weekday":"long","month":"long","day":"numeric","pattern":"{weekday}, {month} {day}"}
// Before code point order, equal scores go to the smaller finer width
// distance (unclamped letter-count differences): en chinese's GyMMMd "MMM
// d, r" and GyMMMMd "MMMM d, r(U)" both score -19 for a numeric month,
// and MMM (3 letters) is closer to numeric (1) than MMMM (4).
print(
  rec(
    bf(
      {era: 'short', year: 'numeric', month: 'numeric', day: 'numeric'},
      'h12',
      EN_CN,
    ),
  ),
);
// CHECK-NEXT: {"year":"numeric","month":"short","day":"numeric","pattern":"{month} {day}, {relatedYear}"}
// When the finer distance ties too, the skeleton first in code point
// order wins, not CLDR's first: an added EMd "EEE M/d", after en's MEd
// "E, M/d", has the same fields and letter counts, and "EMd" < "MEd".
var CODE_POINT = dateTimeLocaleData(
  withNewAvailable(EN_GREGORY, 'EMd', 'EEE M/d'),
  'gregory',
  '.',
);
print(
  rec(
    bf(
      {weekday: 'short', month: 'numeric', day: 'numeric'},
      'h12',
      CODE_POINT,
    ),
  ),
);
// CHECK-NEXT: {"weekday":"short","month":"numeric","day":"numeric","pattern":"{weekday} {month}/{day}"}
// The finer distance leaves timeZoneName out ("short" is no letter count):
// a cs-like Hmvvvv "HH:mm, vvvv" does not beat Hmv "HH:mm v" for
// {hour, minute, timeZoneName: "short"}, which both score -3.
var VVVV = dateTimeLocaleData(
  withNewAvailable(EN_GREGORY, 'Hmvvvv', 'HH:mm, vvvv'),
  'gregory',
  '.',
);
print(
  rec(
    bf(
      {hour: 'numeric', minute: 'numeric', timeZoneName: 'short'},
      'h24',
      VVVV,
    ),
  ),
);
// CHECK-NEXT: {"hour":"2-digit","minute":"2-digit","timeZoneName":"short","pattern":"{hour}:{minute} {timeZoneName}","hourLetter":"H"}
// A B the request did not ask for shows as AM/PM in best fit, of the B's
// width (a refinement of decision 6): with en's h made zh-TW-like "h B"
// and "h BBBB", {hour} shows {ampm} abbreviated and wide; basic and the
// styles (timeFormats short made "h:mm B") keep the B.
var B_DATA = dateTimeLocaleData(
  (function () {
    var list = withAvailable(EN_GREGORY, 'h', 'h B');
    list[7] = 'h:mm B';
    // Add an h interval for a B difference, as zh-TW's "Bh時至Bh時".
    var intervals = 20 + 2 * +list[19];
    list[intervals] = String(+list[intervals] + 1);
    list.push('h', 'B', 'h B\u2009\u2013\u2009h B');
    return list;
  })(),
  'gregory',
  '.',
);
var bHour = bf({hour: 'numeric'}, 'h12', B_DATA);
print(
  rec(bHour),
  show(nameSymbolsOf(bHour, undefined, 'gregory', EN_DAY_PERIODS)),
);
// CHECK-NEXT: {"hour":"numeric","pattern":"{hour} {ampm}","ampmWidth":"abbreviated","hourLetter":"h"} "a"
var BBBB_DATA = dateTimeLocaleData(
  withAvailable(EN_GREGORY, 'h', 'h BBBB'),
  'gregory',
  '.',
);
var bbbbHour = bf({hour: 'numeric'}, 'h12', BBBB_DATA);
print(
  rec(bbbbHour),
  show(nameSymbolsOf(bbbbHour, undefined, 'gregory', EN_DAY_PERIODS)),
);
// CHECK-NEXT: {"hour":"numeric","pattern":"{hour} {ampm}","ampmWidth":"wide","hourLetter":"h"} "aaaa"
print(
  rec(BasicFormatMatcher(req({hour: 'numeric'}), formatsOf(B_DATA, 'h12'))),
);
// CHECK-NEXT: {"hour":"numeric","pattern":"{hour} {dayPeriod}","dayPeriodWidth":"short","hourLetter":"h"}
print(rec(DateTimeStyleFormat(undefined, 'short', B_DATA.styles.h12)));
// CHECK-NEXT: {"hour":"numeric","minute":"2-digit","pattern":"{hour}:{minute} {dayPeriod}","dayPeriodWidth":"short","hourLetter":"h"}
// A time style replaced by best fit for the other hour cycle (decision 7)
// is still a style record and keeps the B: no CLDR 48.2 locale has a
// 24-hour time style and 12-hour patterns with B, so a fixture makes one.
var B_STYLE = dateTimeLocaleData(
  (function () {
    var list = withAvailable(EN_GREGORY, 'hm', 'h:mm B');
    list[7] = 'HH:mm';
    return list;
  })(),
  'gregory',
  '.',
);
print(rec(DateTimeStyleFormat(undefined, 'short', B_STYLE.styles.h12)));
// CHECK-NEXT: {"hour":"2-digit","minute":"2-digit","pattern":"{hour}:{minute} {dayPeriod}","dayPeriodWidth":"short","hourLetter":"h"}
// Its ranges take the intervals as for the B, as ICU4C's do: [[AmPm]] the
// B interval, [[DayPeriod]] (not shown) the hour's; its fallback shows
// AM/PM.
var bRanges = rangePatternsOf(bHour, B_DATA, 'h12');
print(parts(bRanges.AmPm), bRanges.DayPeriod === bRanges.Hour);
// CHECK-NEXT: startRange:"{hour} {dayPeriod}" shared:"\u2009\u2013\u2009" endRange:"{hour} {dayPeriod}" true
print(parts(bRanges.Hour), '|', parts(bRanges.Default));
// CHECK-NEXT: startRange:"{hour}" shared:"\u2009\u2013\u2009" endRange:"{hour}" shared:"\u202f{ampm}" | startRange:"{month}/{day}/{year}, {hour} {ampm}" shared:"\u2009\u2013\u2009" endRange:"{month}/{day}/{year}, {hour} {ampm}"
// The best-fit result is a copy: the cached record keeps its widths.
print(rec(bf({month: 'numeric', day: 'numeric'})));
// CHECK-NEXT: {"month":"numeric","day":"numeric","pattern":"{month}/{day}"}

// Basic: the draft's BasicFormatMatcher over [[formats]], no width
// adjustment.
function basic(o, family = 'h12', data = EN) {
  return BasicFormatMatcher(req(o), formatsOf(data, family));
}
print(rec(basic({month: '2-digit', day: '2-digit'})));
// CHECK-NEXT: {"month":"numeric","day":"numeric","pattern":"{month}/{day}"}
print(
  rec(
    basic({
      hour: 'numeric', minute: 'numeric', second: 'numeric',
      fractionalSecondDigits: 3,
    }),
  ),
);
// CHECK-NEXT: {"hour":"numeric","minute":"2-digit","second":"2-digit","fractionalSecondDigits":3,"pattern":"{hour}:{minute}:{second}.{fractionalSecondDigits}\u202f{ampm}","ampmWidth":"abbreviated","hourLetter":"h"}
print(rec(basic({fractionalSecondDigits: 1})));
// CHECK-NEXT: {"fractionalSecondDigits":1,"pattern":"{fractionalSecondDigits}"}
print(rec(basic({fractionalSecondDigits: 2})));
// CHECK-NEXT: {"fractionalSecondDigits":2,"pattern":"{fractionalSecondDigits}"}
// [[formats]] has the time records with each time zone name appended, so
// basic keeps the hour of {hour, timeZoneName} in both families.
print(rec(basic({hour: 'numeric', timeZoneName: 'short'})));
// CHECK-NEXT: {"hour":"numeric","timeZoneName":"short","pattern":"{hour}\u202f{ampm} {timeZoneName}","ampmWidth":"abbreviated","hourLetter":"h"}
print(rec(basic({hour: 'numeric', timeZoneName: 'short'}, 'h24')));
// CHECK-NEXT: {"hour":"2-digit","timeZoneName":"short","pattern":"{hour} {timeZoneName}","hourLetter":"H"}
print(
  rec(
    basic({
      year: 'numeric', month: 'short', day: 'numeric', hour: 'numeric',
      minute: 'numeric', timeZoneName: 'longOffset',
    }),
  ),
);
// CHECK-NEXT: {"year":"numeric","month":"short","day":"numeric","hour":"numeric","minute":"2-digit","timeZoneName":"longOffset","pattern":"{month} {day}, {year}, {hour}:{minute}\u202f{ampm} {timeZoneName}","ampmWidth":"abbreviated","hourLetter":"h"}
// CLDR's Ehm and the glued "ccc" + hm both score -6 here; the first, Ehm,
// wins (step 10.c's >).
print(rec(basic({weekday: 'short', hour: 'numeric', minute: 'numeric'})));
// CHECK-NEXT: {"weekday":"short","hour":"numeric","minute":"2-digit","pattern":"{weekday} {hour}:{minute}\u202f{ampm}","ampmWidth":"abbreviated","hourLetter":"h"}
print(rec(basic(FULL)));
// CHECK-NEXT: {"weekday":"short","year":"numeric","month":"short","day":"numeric","hour":"numeric","minute":"2-digit","pattern":"{weekday}, {month} {day}, {year}, {hour}:{minute}\u202f{ampm}","ampmWidth":"abbreviated","hourLetter":"h"}
// basicFormatMatch, which CreateDateTimeFormat uses, gives the same record
// as BasicFormatMatcher over the whole [[formats]] List, without gluing
// every pair: over 48 requests made by a fixed pseudo-random sequence,
// in both families of two locales' data.
var BASIC_VALUES = [
  ['weekday', [undefined, 'short', 'long', 'narrow']],
  ['era', [undefined, 'short', 'long']],
  ['year', [undefined, 'numeric', '2-digit']],
  ['month', [undefined, 'numeric', '2-digit', 'short', 'long']],
  ['day', [undefined, 'numeric', '2-digit']],
  ['dayPeriod', [undefined, 'short']],
  ['hour', [undefined, 'numeric', '2-digit']],
  ['minute', [undefined, 'numeric', '2-digit']],
  ['second', [undefined, 'numeric']],
  ['fractionalSecondDigits', [undefined, 1, 3]],
  ['timeZoneName', [undefined, 'short', 'longOffset', 'shortGeneric']],
];
var seed = 12345;
var differences = 0;
var gluedChosen = 0;
for (var n = 0; n < 48; n++) {
  var o = {};
  for (var b = 0; b < BASIC_VALUES.length; b++) {
    seed = (seed * 1103515245 + 12345) % 2147483648;
    var values = BASIC_VALUES[b][1];
    o[BASIC_VALUES[b][0]] = values[seed % values.length];
  }
  var data = n % 2 === 0 ? EN : GB;
  var family = n % 4 < 2 ? 'h12' : 'h24';
  var fast = basicFormatMatch(req(o), data, family);
  if (fast !== BasicFormatMatcher(req(o), formatsOf(data, family))) differences++;
  if (fast.datePart !== undefined) gluedChosen++;
}
print('basicFormatMatch differences:', differences, 'glued:', gluedChosen > 20);
// CHECK-NEXT: basicFormatMatch differences: 0 glued: true

// Required subsets: the union of the two families has a record for each of
// the draft's 14 field sets.
var REQUIRED = [
  'weekday year month day hour minute second fractionalSecondDigits',
  'weekday year month day hour minute second',
  'weekday year month day',
  'year month day',
  'year month',
  'month day',
  'month',
  'hour minute second fractionalSecondDigits',
  'hour minute second',
  'hour minute',
  'dayPeriod hour minute second fractionalSecondDigits',
  'dayPeriod hour minute second',
  'dayPeriod hour minute',
  'dayPeriod hour',
];
var FIELDS = [
  'weekday', 'era', 'year', 'month', 'day', 'dayPeriod', 'hour', 'minute',
  'second', 'fractionalSecondDigits', 'timeZoneName',
];
function fieldSet(r) {
  var out = [];
  for (var i = 0; i < FIELDS.length; i++) {
    if (r[FIELDS[i]] !== undefined) out.push(FIELDS[i]);
  }
  return out.join(' ');
}
var union = formatsOf(EN, 'h12').concat(formatsOf(EN, 'h24'));
var missing = [];
for (var i = 0; i < REQUIRED.length; i++) {
  var found = false;
  for (var j = 0; j < union.length && !found; j++) {
    found = fieldSet(union[j]) === REQUIRED[i];
  }
  if (!found) missing.push(REQUIRED[i]);
}
print('required subsets', REQUIRED.length, 'missing:', show(missing));
// CHECK-NEXT: required subsets 14 missing: []
print(rec(bf({dayPeriod: 'short', hour: 'numeric'}, 'h24')));
// CHECK-NEXT: {"hour":"2-digit","pattern":"{hour}","hourLetter":"H"}
print(
  rec(
    matchDateTimeFormat(
      req({dayPeriod: 'short', hour: 'numeric'}),
      EN,
      'h24',
      'basic',
    ),
  ),
);
// CHECK-NEXT: {"hour":"2-digit","pattern":"{hour}","hourLetter":"H"}
print(rec(bf({dayPeriod: 'short'}, 'h24')));
// CHECK-NEXT: {"dayPeriod":"short","pattern":"{dayPeriod}","dayPeriodWidth":"short"}
print(rec(bf({dayPeriod: 'short', hour: 'numeric'})));
// CHECK-NEXT: {"dayPeriod":"short","hour":"numeric","pattern":"{hour} {dayPeriod}","dayPeriodWidth":"short","hourLetter":"h"}

// Calendars: chinese writes y as {relatedYear}; iso8601 has no era name.
print(rec(cldrPatternToRecord('M/d/y', 'chinese')));
// CHECK-NEXT: {"year":"numeric","month":"numeric","day":"numeric","pattern":"{month}/{day}/{relatedYear}"}
print(
  rec(bf({year: 'numeric', month: 'numeric', day: 'numeric'}, 'h12', EN_CN)),
);
// CHECK-NEXT: {"year":"numeric","month":"numeric","day":"numeric","pattern":"{month}/{day}/{relatedYear}"}
print(rec(bf({year: 'numeric'}, 'h12', EN_CN)));
// CHECK-NEXT: {"year":"numeric","pattern":"{relatedYear}({yearName})"}
print(
  show(
    nameSymbolsOf(
      cldrPatternToRecord('G y', 'iso8601'),
      undefined,
      'iso8601',
      EN_DAY_PERIODS,
    ),
  ),
);
// CHECK-NEXT: ""
print(
  show(
    nameSymbolsOf(
      cldrPatternToRecord('G y'),
      undefined,
      'gregory',
      EN_DAY_PERIODS,
    ),
  ),
);
// CHECK-NEXT: "G"
print(
  show(
    nameSymbolsOf(
      cldrPatternToRecord('MMM U', 'chinese'),
      undefined,
      'chinese',
      EN_DAY_PERIODS,
    ),
  ),
);
// CHECK-NEXT: "MMM U"
print(
  show(
    nameSymbolsOf(
      cldrPatternToRecord('G U'),
      undefined,
      'gregory',
      EN_DAY_PERIODS,
    ),
  ),
);
// CHECK-NEXT: "G"
print(
  show(
    nameSymbolsOf(
      cldrPatternToRecord('G U', 'dangi'),
      undefined,
      'dangi',
      EN_DAY_PERIODS,
    ),
  ),
);
// CHECK-NEXT: "U"
// "U – U" would drop the related years, so a year difference takes FB(P).
print(parts(ranges({year: 'numeric'}, 'h12', EN_CN).Year));
// CHECK-NEXT: startRange:"{relatedYear}({yearName})" shared:"\u2009\u2013\u2009" endRange:"{relatedYear}({yearName})"
print(EN_CN.leapMonthPattern, show(EN.leapMonthPattern));
// CHECK-NEXT: {0}bis ""

// Separators: the records of each fraction separator are cached apart, in
// either order of creation.
dataCachePut(dateTimePatternsCache, 'en;gregory', EN_GREGORY);
var MS = {minute: 'numeric', second: 'numeric', fractionalSecondDigits: 1};
function sepCase(sep) {
  var data = cachedDateTimeLocaleData('en', 'gregory', sep);
  return (
    esc(BestFitFormatMatcher(req(MS), data, 'h12').pattern) +
    ' ' +
    esc(BasicFormatMatcher(req(MS), formatsOf(data, 'h12')).pattern)
  );
}
print(sepCase('.'), '|', sepCase('\u066b'), '|', sepCase('.'));
// CHECK-NEXT: {minute}:{second}.{fractionalSecondDigits} {minute}:{second}.{fractionalSecondDigits} | {minute}:{second}\u066b{fractionalSecondDigits} {minute}:{second}\u066b{fractionalSecondDigits} | {minute}:{second}.{fractionalSecondDigits} {minute}:{second}.{fractionalSecondDigits}
dateTimeLocaleDataCache = newDataCache(DATE_TIME_LOCALE_DATA_CACHE_SIZE);
print(sepCase('\u066b'), '|', sepCase('.'), '|', sepCase('\u066b'));
// CHECK-NEXT: {minute}:{second}\u066b{fractionalSecondDigits} {minute}:{second}\u066b{fractionalSecondDigits} | {minute}:{second}.{fractionalSecondDigits} {minute}:{second}.{fractionalSecondDigits} | {minute}:{second}\u066b{fractionalSecondDigits} {minute}:{second}\u066b{fractionalSecondDigits}
print(
  cachedDateTimeLocaleData('en', 'gregory', '.') ===
    cachedDateTimeLocaleData('en', 'gregory', '.'),
  cachedDateTimeLocaleData('en', 'gregory', '.') ===
    cachedDateTimeLocaleData('en', 'gregory', '\u066b'),
);
// CHECK-NEXT: true false

// Styles (decision 7).
var full_short = DateTimeStyleFormat('full', 'short', EN.styles.h12);
print(rec(full_short));
// CHECK-NEXT: {"weekday":"long","year":"numeric","month":"long","day":"numeric","hour":"numeric","minute":"2-digit","pattern":"{weekday}, {month} {day}, {year} at {hour}:{minute}\u202f{ampm}","ampmWidth":"abbreviated","hourLetter":"h"}
print(rec(DateTimeStyleFormat(undefined, 'short', EN.styles.h24)));
// CHECK-NEXT: {"hour":"2-digit","minute":"2-digit","pattern":"{hour}:{minute}","hourLetter":"H"}
print(rec(DateTimeStyleFormat(undefined, 'full', EN.styles.h24)));
// CHECK-NEXT: {"hour":"2-digit","minute":"2-digit","second":"2-digit","timeZoneName":"long","pattern":"{hour}:{minute}:{second} {timeZoneName}","hourLetter":"H"}
print(rec(DateTimeStyleFormat(undefined, 'short', EN.styles.h12)));
// CHECK-NEXT: {"hour":"numeric","minute":"2-digit","pattern":"{hour}:{minute}\u202f{ampm}","ampmWidth":"abbreviated","hourLetter":"h"}
print(
  rec(DateTimeStyleFormat('medium', undefined, EN.styles.h12)),
  DateTimeStyleFormat('medium', undefined, EN.styles.h12) ===
    EN.styles.h12.Date.medium,
);
// CHECK-NEXT: {"year":"numeric","month":"short","day":"numeric","pattern":"{month} {day}, {year}"} true
print(rec(DateTimeStyleFormat('short', 'medium', EN.styles.h24)));
// CHECK-NEXT: {"year":"2-digit","month":"numeric","day":"numeric","hour":"2-digit","minute":"2-digit","second":"2-digit","pattern":"{month}/{day}/{year}, {hour}:{minute}:{second}","hourLetter":"H"}
print(
  full_short.rangePatterns === full_short.rangePatterns12,
  rangeFields(full_short.rangePatterns),
);
// CHECK-NEXT: true Default Era Year Month Day AmPm DayPeriod Hour Minute
print(parts(full_short.rangePatterns.Hour));
// CHECK-NEXT: shared:"{weekday}, {month} {day}, {year}, " startRange:"{hour}:{minute}" shared:"\u2009\u2013\u2009" endRange:"{hour}:{minute}" shared:"\u202f{ampm}"
print(
  DateTimeStyleFormat('full', 'short', EN.styles.h12).rangePatterns ===
    full_short.rangePatterns,
);
// CHECK-NEXT: true

// Range patterns (decision 11).
var yMMMd = ranges({year: 'numeric', month: 'short', day: 'numeric'});
print(rangeFields(yMMMd));
// CHECK-NEXT: Default Era Year Month Day
print(parts(yMMMd.Day));
// CHECK-NEXT: shared:"{month} " startRange:"{day}" shared:"\u2009\u2013\u2009" endRange:"{day}" shared:", {year}"
print(parts(yMMMd.Month));
// CHECK-NEXT: startRange:"{month} {day}" shared:"\u2009\u2013\u2009" endRange:"{month} {day}" shared:", {year}"
print(parts(yMMMd.Year));
// CHECK-NEXT: startRange:"{month} {day}, {year}" shared:"\u2009\u2013\u2009" endRange:"{month} {day}, {year}"
print(parts(yMMMd.Default));
// CHECK-NEXT: startRange:"{month} {day}, {year}" shared:"\u2009\u2013\u2009" endRange:"{month} {day}, {year}"
print(rec(yMMMd.Day));
// CHECK-NEXT: {"year":"numeric","month":"short","day":"numeric"}
var yMd = ranges({year: 'numeric', month: 'numeric', day: 'numeric'});
print(rangeFields(yMd), parts(yMd.Day));
// CHECK-NEXT: Default Era Year Month Day startRange:"{month}/{day}/{year}" shared:"\u2009\u2013\u2009" endRange:"{month}/{day}/{year}"
var hm = ranges({hour: 'numeric', minute: 'numeric'});
print(rangeFields(hm));
// CHECK-NEXT: Default Era Year Month Day AmPm DayPeriod Hour Minute
print(parts(hm.AmPm));
// CHECK-NEXT: startRange:"{hour}:{minute}\u202f{ampm}" shared:"\u2009\u2013\u2009" endRange:"{hour}:{minute}\u202f{ampm}"
print(parts(hm.Hour), hm.DayPeriod === hm.Hour);
// CHECK-NEXT: startRange:"{hour}:{minute}" shared:"\u2009\u2013\u2009" endRange:"{hour}:{minute}" shared:"\u202f{ampm}" true
print(parts(hm.Minute));
// CHECK-NEXT: startRange:"{hour}:{minute}" shared:"\u2009\u2013\u2009" endRange:"{hour}:{minute}" shared:"\u202f{ampm}"
print(parts(hm.Default));
// CHECK-NEXT: startRange:"{month}/{day}/{year}, {hour}:{minute}\u202f{ampm}" shared:"\u2009\u2013\u2009" endRange:"{month}/{day}/{year}, {hour}:{minute}\u202f{ampm}"
print(rec(hm.Default));
// CHECK-NEXT: {"year":"numeric","month":"numeric","day":"numeric","hour":"numeric","minute":"2-digit","ampmWidth":"abbreviated"}
// A record without an era, in a calendar with eras, has an [[Era]] that
// shows them (a refinement of decision 11); a time-only record's
// [[Year]], [[Month]] and [[Day]] are then [[Default]], so a different
// day still takes [[Default]].
print(
  parts(hm.Era),
  hm.Year === hm.Default,
  hm.Month === hm.Default,
  hm.Day === hm.Default,
);
// CHECK-NEXT: startRange:"{month}/{day}/{year} {era}, {hour}:{minute}\u202f{ampm}" shared:"\u2009\u2013\u2009" endRange:"{month}/{day}/{year} {era}, {hour}:{minute}\u202f{ampm}" true true true
var Hm = ranges({hour: 'numeric', minute: 'numeric'}, 'h24');
print(rangeFields(Hm), Hm.AmPm === Hm.Hour, Hm.DayPeriod === Hm.Hour);
// CHECK-NEXT: Default Era Year Month Day AmPm DayPeriod Hour Minute true true
print(parts(Hm.Hour));
// CHECK-NEXT: startRange:"{hour}:{minute}" shared:"\u2009\u2013\u2009" endRange:"{hour}:{minute}"
var Bhm = ranges({dayPeriod: 'short', hour: 'numeric', minute: 'numeric'});
print(rangeFields(Bhm), Bhm.AmPm === Bhm.DayPeriod);
// CHECK-NEXT: Default Era Year Month Day AmPm DayPeriod Hour Minute true
print(parts(Bhm.DayPeriod));
// CHECK-NEXT: startRange:"{hour}:{minute} {dayPeriod}" shared:"\u2009\u2013\u2009" endRange:"{hour}:{minute} {dayPeriod}"
print(parts(Bhm.Hour));
// CHECK-NEXT: startRange:"{hour}:{minute}" shared:"\u2009\u2013\u2009" endRange:"{hour}:{minute}" shared:" {dayPeriod}"
var h = ranges({hour: 'numeric'});
print(rangeFields(h), h.DayPeriod === h.Hour, parts(h.Hour));
// CHECK-NEXT: Default Era Year Month Day AmPm DayPeriod Hour true startRange:"{hour}" shared:"\u2009\u2013\u2009" endRange:"{hour}" shared:"\u202f{ampm}"
var H = ranges({hour: 'numeric'}, 'h24');
print(rangeFields(H), H.AmPm === H.Hour, H.DayPeriod === H.Hour);
// CHECK-NEXT: Default Era Year Month Day AmPm DayPeriod Hour true true
var Bh = ranges({dayPeriod: 'short', hour: 'numeric'});
print(rangeFields(Bh), Bh.AmPm === Bh.DayPeriod, parts(Bh.Hour));
// CHECK-NEXT: Default Era Year Month Day AmPm DayPeriod Hour true startRange:"{hour}" shared:"\u2009\u2013\u2009" endRange:"{hour}" shared:" {dayPeriod}"
var hms = ranges({hour: 'numeric', minute: 'numeric', second: 'numeric'});
var hmsFB = parts(hms.Hour);
print(rangeFields(hms), parts(hms.Hour));
// CHECK-NEXT: Default Era Year Month Day AmPm DayPeriod Hour Minute Second startRange:"{hour}:{minute}:{second}\u202f{ampm}" shared:"\u2009\u2013\u2009" endRange:"{hour}:{minute}:{second}\u202f{ampm}"
print(parts(hms.Minute) === hmsFB, parts(hms.Second) === hmsFB);
// CHECK-NEXT: true true
var MMMd = ranges({month: 'short', day: 'numeric'});
print(rangeFields(MMMd), parts(MMMd.Year));
// CHECK-NEXT: Default Era Year Month Day startRange:"{month} {day}, {year}" shared:"\u2009\u2013\u2009" endRange:"{month} {day}, {year}"
print(parts(MMMd.Day), parts(MMMd.Era));
// CHECK-NEXT: shared:"{month} " startRange:"{day}" shared:"\u2009\u2013\u2009" endRange:"{day}" startRange:"{month} {day}, {year} {era}" shared:"\u2009\u2013\u2009" endRange:"{month} {day}, {year} {era}"
var d = ranges({day: 'numeric'});
print(rangeFields(d), parts(d.Month));
// CHECK-NEXT: Default Era Year Month Day startRange:"{month}/{day}" shared:"\u2009\u2013\u2009" endRange:"{month}/{day}"
// A date and time record without an era: [[Era]] is FB of the best fit
// with the era; [[Year]] to [[Day]] are FB(P).
var glued = ranges(MEDIUM);
print(rangeFields(glued), parts(glued.Era));
// CHECK-NEXT: Default Era Year Month Day AmPm DayPeriod Hour Minute startRange:"{month} {day}, {year} {era}, {hour}:{minute}\u202f{ampm}" shared:"\u2009\u2013\u2009" endRange:"{month} {day}, {year} {era}, {hour}:{minute}\u202f{ampm}"
print(
  glued.Era === glued.Default,
  glued.Year === glued.Default,
  glued.Month === glued.Default,
  glued.Day === glued.Default,
  parts(glued.Default),
);
// CHECK-NEXT: false true true true startRange:"{month} {day}, {year}, {hour}:{minute}\u202f{ampm}" shared:"\u2009\u2013\u2009" endRange:"{month} {day}, {year}, {hour}:{minute}\u202f{ampm}"
print(parts(glued.Hour));
// CHECK-NEXT: shared:"{month} {day}, {year}, " startRange:"{hour}:{minute}" shared:"\u2009\u2013\u2009" endRange:"{hour}:{minute}" shared:"\u202f{ampm}"
print(parts(glued.AmPm), glued.DayPeriod === glued.Hour);
// CHECK-NEXT: shared:"{month} {day}, {year}, " startRange:"{hour}:{minute}\u202f{ampm}" shared:"\u2009\u2013\u2009" endRange:"{hour}:{minute}\u202f{ampm}" true
// A record with a date and a time that is not glued: the date part is the
// best fit for its date fields, joined by the standard dateTimeFormat.
var Ehm = ranges({weekday: 'short', hour: 'numeric', minute: 'numeric'});
print(rangeFields(Ehm), parts(Ehm.Hour));
// CHECK-NEXT: Default Era Year Month Day AmPm DayPeriod Hour Minute shared:"{weekday}, " startRange:"{hour}:{minute}" shared:"\u2009\u2013\u2009" endRange:"{hour}:{minute}" shared:"\u202f{ampm}"
print(parts(Ehm.Default));
// CHECK-NEXT: startRange:"{weekday} {hour}:{minute}\u202f{ampm}" shared:"\u2009\u2013\u2009" endRange:"{weekday} {hour}:{minute}\u202f{ampm}"
// A field the interval pattern shows that R lacks takes the interval
// pattern's width, and its name is asked for (as ICU4C: en-u-ca-hebrew
// {year} "5784 – 5786 AM").
/// list with the pattern of its interval entry (skeleton, letter) replaced.
function withInterval(list, skeleton, letter, pattern) {
  var copy = list.slice();
  var start = 20 + 2 * +copy[19] + 1;
  for (var i = start; i < copy.length; i += 3) {
    if (copy[i] === skeleton && copy[i + 1] === letter) copy[i + 2] = pattern;
  }
  return copy;
}
var EN_ERA = withInterval(EN_GREGORY, 'y', 'y', 'y\u2009\u2013\u2009y G');
var hebrew = dateTimeLocaleData(EN_ERA, 'hebrew', '.');
var yRecord = bf({year: 'numeric'}, 'h12', hebrew);
var yRanges = rangePatternsOf(yRecord, hebrew, 'h12');
print(parts(yRanges.Year), rec(yRanges.Year));
// CHECK-NEXT: startRange:"{year}" shared:"\u2009\u2013\u2009" endRange:"{year}" shared:" {era}" {"era":"short","year":"numeric"}
print(
  show(nameSymbolsOf(yRecord, yRanges, 'hebrew', EN_DAY_PERIODS)),
  show(nameSymbolsOf(yRecord, yRanges, 'iso8601', EN_DAY_PERIODS)),
);
// CHECK-NEXT: "G" ""
var EN_AMPM = withInterval(
  EN_GREGORY, 'Bh', 'h', 'h\u2009\u2013\u2009h a',
);
var ampmData = dateTimeLocaleData(EN_AMPM, 'gregory', '.');
var bhAmpm = bf({dayPeriod: 'short', hour: 'numeric'}, 'h12', ampmData);
var bhAmpmRanges = rangePatternsOf(bhAmpm, ampmData, 'h12');
print(parts(bhAmpmRanges.Hour), rec(bhAmpmRanges.Hour));
// CHECK-NEXT: startRange:"{hour}" shared:"\u2009\u2013\u2009" endRange:"{hour}" shared:" {ampm}" {"dayPeriod":"short","hour":"numeric","ampmWidth":"abbreviated","dayPeriodWidth":"short"}
print(show(nameSymbolsOf(bhAmpm, bhAmpmRanges, 'gregory', EN_DAY_PERIODS)));
// CHECK-NEXT: "G a"
var EN_B = withInterval(EN_GREGORY, 'Hv', 'H', 'B HH\u2009\u2013\u2009HH v');
var bData = dateTimeLocaleData(EN_B, 'gregory', '.');
var hvRecord = bf(
  {hour: 'numeric', timeZoneName: 'shortGeneric'}, 'h24', bData,
);
var hvRanges = rangePatternsOf(hvRecord, bData, 'h24');
print(parts(hvRanges.Hour), rec(hvRanges.Hour));
// CHECK-NEXT: shared:"{dayPeriod} " startRange:"{hour}" shared:"\u2009\u2013\u2009" endRange:"{hour}" shared:" {timeZoneName}" {"hour":"2-digit","timeZoneName":"shortGeneric","dayPeriodWidth":"short"}
var noAbbreviated = EN_DAY_PERIODS.slice();
noAbbreviated[1 + 6 + 3] = '';
print(
  show(nameSymbolsOf(hvRecord, hvRanges, 'gregory', EN_DAY_PERIODS)),
  show(nameSymbolsOf(hvRecord, hvRanges, 'gregory', noAbbreviated)),
);
// CHECK-NEXT: "v G" "v G a"
// zh-Hant's "Bh時至h時" under skeleton h: the B the skeleton does not ask
// for sets the day period width but adds no [[dayPeriod]] field.
var EN_HANT = withInterval(EN_GREGORY, 'h', 'h', 'Bh\u6642\u81f3h\u6642');
var hantData = dateTimeLocaleData(EN_HANT, 'gregory', '.');
var hantRanges = rangePatternsOf(bf({hour: 'numeric'}, 'h12', hantData),
  hantData, 'h12');
print(parts(hantRanges.Hour), rec(hantRanges.Hour));
// CHECK-NEXT: shared:"{dayPeriod}" startRange:"{hour}" shared:"\u6642\u81f3" endRange:"{hour}" shared:"\u6642" {"hour":"numeric","ampmWidth":"abbreviated","dayPeriodWidth":"short"}
// A DateTimeStyleFormat record joins its time ranges to the date with the
// date style's standard dateTimeFormat, as its pattern uses the date
// style's atTime one: sk's medium "d. M. y" takes medium's "{1}, {0}",
// not short's "{1} {0}".
var SK = EN_GREGORY.slice();
SK[2] = 'd. M. y';
SK[11] = '{1} {0}';
SK[15] = '{1} {0}';
var skData = dateTimeLocaleData(SK, 'gregory', '.');
var skFormat = DateTimeStyleFormat('medium', 'short', skData.styles.h12);
print(esc(skFormat.pattern), parts(skFormat.rangePatterns.Hour));
// CHECK-NEXT: {day}. {month}. {year}, {hour}:{minute}\u202f{ampm} shared:"{day}. {month}. {year}, " startRange:"{hour}:{minute}" shared:"\u2009\u2013\u2009" endRange:"{hour}:{minute}" shared:"\u202f{ampm}"
// The interval skeleton is chosen with best fit's kind penalty: for the
// numeric "M/d/y", a 2-digit "yMMdd" (-12) beats "yMMMd" (-6 + -13), so
// its whole dates are used, not "MMM d – d, y" with a numeric month.
var EN_MMDD = EN_GREGORY.slice();
for (var i = 20 + 2 * +EN_MMDD[19] + 1; i < EN_MMDD.length; i += 3) {
  if (EN_MMDD[i] === 'yMd') EN_MMDD[i] = 'yMMdd';
}
var mmddData = dateTimeLocaleData(EN_MMDD, 'gregory', '.');
var mmddRanges = rangePatternsOf(
  bf({year: 'numeric', month: 'numeric', day: 'numeric'}, 'h12', mmddData),
  mmddData,
  'h12',
);
print(parts(mmddRanges.Day));
// CHECK-NEXT: startRange:"{month}/{day}/{year}" shared:"\u2009\u2013\u2009" endRange:"{month}/{day}/{year}"
var rangeRecord = bf({hour: 'numeric'});
print(
  rangePatternsOf(rangeRecord, EN, 'h12') ===
    rangePatternsOf(rangeRecord, EN, 'h12'),
);
// CHECK-NEXT: true

// Names (decision 4).
var fullRecord = bf(FULL);
print(
  show(
    nameSymbolsOf(
      fullRecord,
      rangePatternsOf(fullRecord, EN, 'h12'),
      'gregory',
      EN_DAY_PERIODS,
    ),
  ),
);
// CHECK-NEXT: "EEEE MMMM a G"
var NAME_CASES = [
  'ccc LLLLL GGGGG v aaaaa', 'EEE MMMMM zzzz', 'cccc LLL GGGG z', 'EEEEE O',
  'ccccc OOOO', 'vvvv aaaa', 'LLLL h:mm a',
];
var names = [];
for (var i = 0; i < NAME_CASES.length; i++) {
  names.push(
    nameSymbolsOf(
      cldrPatternToRecord(NAME_CASES[i]),
      undefined,
      'gregory',
      EN_DAY_PERIODS,
    ),
  );
}
print(show(names));
// CHECK-NEXT: ["ccc LLLLL GGGGG v aaaaa","EEE MMMMM zzzz","cccc LLL GGGG z","EEEEE O","ccccc OOOO","vvvv aaaa","LLLL a"]
// The widths of each range pattern count: MMMM d's [[Year]] is the best
// fit for MMMM d y, whose month is long too. The [[Era]] patterns of a
// record without an era ask for the abbreviated era names (G).
var mmmRecord = bf({month: 'long', day: 'numeric'});
print(
  show(
    nameSymbolsOf(
      mmmRecord,
      rangePatternsOf(mmmRecord, EN, 'h12'),
      'gregory',
      EN_DAY_PERIODS,
    ),
  ),
);
// CHECK-NEXT: "MMMM G"
var bhRecord = bf({dayPeriod: 'short', hour: 'numeric'});
print(
  show(
    nameSymbolsOf(
      bhRecord,
      rangePatternsOf(bhRecord, EN, 'h12'),
      'gregory',
      EN_DAY_PERIODS,
    ),
  ),
);
// CHECK-NEXT: "G"
// A period with no name in the width used shows the AM/PM name instead,
// so its symbol is included.
var noNames = EN_DAY_PERIODS.slice();
noNames[1 + 6 + 3] = '';
print(
  show(nameSymbolsOf(bhRecord, undefined, 'gregory', noNames)),
  show(
    nameSymbolsOf(
      cldrPatternToRecord('h BBBB', 'gregory', 'Bh'),
      undefined,
      'gregory',
      noNames,
    ),
  ),
);
// CHECK-NEXT: "a" ""
var noWide = EN_DAY_PERIODS.slice();
noWide[noWide.length - 2] = '';
print(
  show(
    nameSymbolsOf(
      cldrPatternToRecord('h BBBB', 'gregory', 'Bh'),
      undefined,
      'gregory',
      noWide,
    ),
  ),
);
// CHECK-NEXT: "aaaa"

// Day periods (decision 4), en.
print(
  dayPeriodOf(EN_DAY_PERIODS, 0, 0, 0, false, false),
  dayPeriodOf(EN_DAY_PERIODS, 6, 0, 0, false, false),
  dayPeriodOf(EN_DAY_PERIODS, 12, 0, 0, false, false),
  dayPeriodOf(EN_DAY_PERIODS, 12, 30, 0, false, false),
  dayPeriodOf(EN_DAY_PERIODS, 12, 30, 0, true, false),
  dayPeriodOf(EN_DAY_PERIODS, 12, 0, 0, true, false),
  dayPeriodOf(EN_DAY_PERIODS, 12, 0, 5, true, true),
  dayPeriodOf(EN_DAY_PERIODS, 12, 0, 5, true, false),
  dayPeriodOf(EN_DAY_PERIODS, 11, 59, 59, true, true),
  dayPeriodOf(EN_DAY_PERIODS, 17, 59, 0, true, false),
  dayPeriodOf(EN_DAY_PERIODS, 18, 0, 0, true, false),
  dayPeriodOf(EN_DAY_PERIODS, 21, 0, 0, false, false),
  dayPeriodOf(EN_DAY_PERIODS, 23, 59, 0, true, false),
);
// CHECK-NEXT: morning1 morning1 noon noon afternoon1 noon afternoon1 noon morning1 afternoon1 evening1 night1 night1
print(
  dayPeriodName(EN_DAY_PERIODS, 'noon', 'narrow'),
  '|',
  dayPeriodName(EN_DAY_PERIODS, 'night1', 'long'),
  '|',
  dayPeriodName(EN_DAY_PERIODS, 'morning1', 'short'),
);
// CHECK-NEXT: n | at night | in the morning
// A rule set whose period wraps midnight and has no noon.
var WRAP = ['2', 'day', '360', '1080', 'd', 'day', 'D', 'night', '1080', '360',
  'n', 'night', 'N', '', '', ''];
print(
  dayPeriodOf(WRAP, 12, 0, 0, false, false),
  dayPeriodOf(WRAP, 23, 0, 0, false, false),
  dayPeriodOf(WRAP, 3, 0, 0, false, false),
  dayPeriodOf(WRAP, 6, 0, 0, false, false),
  dayPeriodName(WRAP, 'night', 'narrow'),
);
// CHECK-NEXT: day night night day N

// Refinements of decisions 6 and 11 from the final review.
/// list without its availableFormats entry skeleton.
function withoutAvailable(list, skeleton) {
  var end = 20 + 2 * +list[19];
  var copy = list.slice(0, 20);
  for (var i = 20; i < end; i += 2) {
    if (list[i] !== skeleton) copy.push(list[i], list[i + 1]);
  }
  copy[19] = String((copy.length - 20) / 2);
  return copy.concat(list.slice(end));
}
// Best fit ranks a record first by the requested fields it lacks that
// step 6 cannot append: th-like data without Hmv and Hmsv takes Hm (-126)
// and appends the zone, not Hv "HH v" (-123), which has no minute.
print(
  unappendableMissingCount(
    req({hour: 'numeric', minute: 'numeric', fractionalSecondDigits: 1, timeZoneName: 'short'}),
    req({hour: 'numeric'}),
  ),
);
// CHECK-NEXT: 1
var TH = dateTimeLocaleData(
  withoutAvailable(withoutAvailable(EN_GREGORY, 'Hmv'), 'Hmsv'),
  'gregory',
  '.',
);
var thHm = bf({hour: '2-digit', minute: '2-digit', timeZoneName: 'short'}, 'h24', TH);
print(rec(thHm));
// CHECK-NEXT: {"hour":"2-digit","minute":"2-digit","timeZoneName":"short","pattern":"{hour}:{minute} {timeZoneName}","hourLetter":"H"}
print(parts(rangePatternsOf(thHm, TH, 'h24').Default));
// CHECK-NEXT: startRange:"{month}/{day}/{year}, {hour}:{minute} {timeZoneName}" shared:"\u2009\u2013\u2009" endRange:"{month}/{day}/{year}, {hour}:{minute} {timeZoneName}"
// A dayPeriod without an hour is matched in the 12-hour family's records,
// in either family: "h24" has none with a day period and a minute.
print(
  formatsFamilyOf(req({dayPeriod: 'short', minute: 'numeric'}), 'h24'),
  formatsFamilyOf(req({dayPeriod: 'short', hour: 'numeric'}), 'h24'),
  formatsFamilyOf(req({minute: 'numeric'}), 'h24'),
);
// CHECK-NEXT: h12 h24 h24
print(rec(bf({dayPeriod: 'short', minute: 'numeric'}, 'h24')));
// CHECK-NEXT: {"dayPeriod":"short","hour":"numeric","minute":"2-digit","pattern":"{hour}:{minute} {dayPeriod}","dayPeriodWidth":"short","hourLetter":"h"}
// A record whose unrequested B shows as AM/PM gets the appended fields in
// its withDayPeriod too, whose intervals its ranges take: zh-TW-like data
// with hm "Bh:mm" and no hmv or hmsv.
var ZHB = dateTimeLocaleData(
  withoutAvailable(
    withoutAvailable(withAvailable(EN_GREGORY, 'hm', 'Bh:mm'), 'hmv'),
    'hmsv',
  ),
  'gregory',
  '.',
);
var zhbHm = bf({hour: 'numeric', minute: 'numeric', timeZoneName: 'short'}, 'h12', ZHB);
print(show(zhbHm.pattern), '|', show(zhbHm.withDayPeriod.pattern));
// CHECK-NEXT: "{ampm}{hour}:{minute} {timeZoneName}" | "{dayPeriod}{hour}:{minute} {timeZoneName}"
print(parts(rangePatternsOf(zhbHm, ZHB, 'h12').Hour));
// CHECK-NEXT: startRange:"{hour}:{minute}" shared:"\u2009\u2013\u2009" endRange:"{hour}:{minute}" shared:"\u202f{ampm} {timeZoneName}"
// A style record's unrequested B counts as a dayPeriod field when its
// interval skeleton is chosen: a timeFormats short "h:mm B" takes Bhm's
// intervals, not hm's.
var styleB = EN_GREGORY.slice();
styleB[7] = 'h:mm B';
var STYLE_B = dateTimeLocaleData(styleB, 'gregory', '.');
var styleBRanges = rangePatternsOf(
  DateTimeStyleFormat(undefined, 'short', STYLE_B.styles.h12),
  STYLE_B,
  'h12',
);
print(parts(styleBRanges.Hour), '|', parts(styleBRanges.DayPeriod));
// CHECK-NEXT: startRange:"{hour}:{minute}" shared:"\u2009\u2013\u2009" endRange:"{hour}:{minute}" shared:" {dayPeriod}" | startRange:"{hour}:{minute} {dayPeriod}" shared:"\u2009\u2013\u2009" endRange:"{hour}:{minute} {dayPeriod}"
