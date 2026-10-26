/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US -Xintl-default-time-zone=UTC %s | %FileCheck --match-full-lines %s

// The ECMA-402 versions of Date.prototype.toLocaleString,
// toLocaleDateString and toLocaleTimeString: each formats the this value's
// time value with a DateTimeFormat made from locales and options, with
// (required, defaults) (any, all), (date, date) and (time, time). The
// expected strings are ICU4C's (Node 24, ICU 78.2), except that the plugin
// keeps the U+202F before AM and PM that V8 replaces with a space.

/// The class of the exception f throws, 'none', or the message of an
/// internal error from native code.
function thrown(f) {
  try {
    f();
    return 'none';
  } catch (e) {
    var message = String(e.message);
    if (message.indexOf('HostFunction') !== -1) return 'native: ' + message;
    return e.constructor.name;
  }
}

/// s with every code unit outside printable ASCII as \uXXXX.
function esc(s) {
  var out = '';
  for (var i = 0; i < s.length; i++) {
    var c = s.charCodeAt(i);
    out += c < 0x20 || c > 0x7e ? '\\u' + (c + 0x10000).toString(16).substring(1) : s[i];
  }
  return out;
}

var t = Date.UTC(2026, 0, 5, 15, 7, 9, 123);
var d = new Date(t);

print('defaults');
// CHECK-LABEL: defaults
print(esc(d.toLocaleString('en')), '|', d.toLocaleDateString('en'), '|', esc(d.toLocaleTimeString('en')));
// CHECK-NEXT: 1/5/2026, 3:07:09\u202fPM | 1/5/2026 | 3:07:09\u202fPM
// The default locale is en-US, and the time zone UTC. Each method has its
// own default formatter, so calling one first does not change another's
// result; and again, now that the defaults exist, and with both arguments
// undefined.
print(esc(d.toLocaleString()), '|', d.toLocaleDateString(), '|', esc(d.toLocaleTimeString()));
// CHECK-NEXT: 1/5/2026, 3:07:09\u202fPM | 1/5/2026 | 3:07:09\u202fPM
print(esc(d.toLocaleTimeString()), '|', d.toLocaleDateString(undefined, undefined), '|', esc(d.toLocaleString(undefined, undefined)));
// CHECK-NEXT: 3:07:09\u202fPM | 1/5/2026 | 1/5/2026, 3:07:09\u202fPM
// A call with options does not change the default formatters.
print(d.toLocaleString(undefined, {month: 'long'}), '|', esc(d.toLocaleString()));
// CHECK-NEXT: January | 1/5/2026, 3:07:09\u202fPM
print(d.toLocaleString('de'), '|', d.toLocaleDateString('de'), '|', d.toLocaleTimeString(['de']));
// CHECK-NEXT: 5.1.2026, 15:07:09 | 5.1.2026 | 15:07:09
print(d.toLocaleString('ja-u-ca-japanese'), '|', esc(d.toLocaleString('en', {timeZone: 'America/New_York'})));
// CHECK-NEXT: R8/1/5 15:07:09 | 1/5/2026, 10:07:09\u202fAM
// The extremes of the time range.
print(esc(new Date(8.64e15).toLocaleString('en')), '|', esc(new Date(-8.64e15).toLocaleString('en')));
// CHECK-NEXT: 9/13/275760, 12:00:00\u202fAM | 4/20/271822, 12:00:00\u202fAM

print('required and defaults');
// CHECK-LABEL: required and defaults
// A field of the other kind does not stop the method's defaults:
// toLocaleDateString still adds the date to an hour, and
// toLocaleTimeString the time to a year.
print(esc(d.toLocaleDateString('en', {hour: 'numeric'})), '|', esc(d.toLocaleTimeString('en', {year: 'numeric'})));
// CHECK-NEXT: 1/5/2026, 3\u202fPM | 2026, 3:07:09\u202fPM
// A field of the method's own kind replaces its defaults.
print(d.toLocaleDateString('en', {month: 'long'}), '|', esc(d.toLocaleTimeString('en', {hour: '2-digit', minute: '2-digit'})), '|', d.toLocaleString('en', {weekday: 'long'}));
// CHECK-NEXT: January | 03:07\u202fPM | Monday
print(d.toLocaleDateString('en', {dateStyle: 'full'}), '|', esc(d.toLocaleTimeString('en', {timeStyle: 'short'})));
// CHECK-NEXT: Monday, January 5, 2026 | 3:07\u202fPM
print(esc(d.toLocaleString('en', {dateStyle: 'short', timeStyle: 'short'})), '|', d.toLocaleString('en', {dateStyle: 'medium'}));
// CHECK-NEXT: 1/5/26, 3:07\u202fPM | Jan 5, 2026
// CreateDateTimeFormat's required check: a style of the other kind is a
// TypeError, and toLocaleString accepts both.
print(thrown(() => d.toLocaleTimeString('en', {dateStyle: 'short'})), thrown(() => d.toLocaleDateString('en', {timeStyle: 'short'})));
// CHECK-NEXT: TypeError TypeError
print(thrown(() => d.toLocaleString('xx-invalid-')), thrown(() => d.toLocaleDateString('en', {timeZone: 'Nowhere'})), thrown(() => d.toLocaleTimeString('en', {hour: 'x'})));
// CHECK-NEXT: RangeError RangeError RangeError

print('invalid date');
// CHECK-LABEL: invalid date
var nan = new Date(NaN);
print(nan.toLocaleString(), '|', nan.toLocaleDateString('de'), '|', nan.toLocaleTimeString('en', {}));
// CHECK-NEXT: Invalid Date | Invalid Date | Invalid Date
// The NaN check comes before CreateDateTimeFormat: the locales are not
// canonicalized and the options are not read.
var reads = 0;
var countingOptions = {
  get timeZone() {
    reads++;
    return 'UTC';
  },
};
print(nan.toLocaleString('xx-invalid-'), '|', nan.toLocaleDateString('en', countingOptions), reads);
// CHECK-NEXT: Invalid Date | Invalid Date 0
print(d.toLocaleDateString('en', countingOptions), reads);
// CHECK-NEXT: 1/5/2026 1

print('this value');
// CHECK-LABEL: this value
// RequireInternalSlot(dateObj, [[DateValue]]): only a Date object is
// accepted. Date.prototype is an ordinary object, and a proxy of a Date
// has no [[DateValue]]. The check comes before the options are read.
var methods = [Date.prototype.toLocaleString, Date.prototype.toLocaleDateString, Date.prototype.toLocaleTimeString];
var values = [{}, Date.prototype, new Proxy(d, {}), t, '2026-01-05', undefined, null, Object.create(d)];
for (var i = 0; i < methods.length; i++) {
  var results = [];
  for (var j = 0; j < values.length; j++)
    results.push(thrown(() => methods[i].call(values[j], 'en', countingOptions)));
  print(results.join(' '), reads);
}
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError TypeError TypeError TypeError 1
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError TypeError TypeError TypeError 1
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError TypeError TypeError TypeError 1
// The TypeError names the method called.
for (var i = 0; i < methods.length; i++) {
  try {
    methods[i].call(new Proxy(d, {}));
  } catch (e) {
    print(e.message);
  }
}
// CHECK-NEXT: Date.prototype.toLocaleString called on non-Date object
// CHECK-NEXT: Date.prototype.toLocaleDateString called on non-Date object
// CHECK-NEXT: Date.prototype.toLocaleTimeString called on non-Date object
// A subclass instance is a Date; the Date's own valueOf and
// @@toPrimitive are not consulted.
class MyDate extends Date {}
var sub = new MyDate(t);
var own = new Date(t);
own.valueOf = () => 0;
own[Symbol.toPrimitive] = () => 0;
print(sub.toLocaleDateString('en'), own.toLocaleDateString('en'));
// CHECK-NEXT: 1/5/2026 1/5/2026

print('same as DateTimeFormat');
// CHECK-LABEL: same as DateTimeFormat
// Each method gives what Intl.DateTimeFormat's format gives for the same
// locales and options, with the method's defaults added when the options
// ask for no field of the required kind and no style (test262's
// returns-same-results-as-DateTimeFormat.js makes this check). An era or
// time zone name alone takes the defaults; a date field in
// toLocaleTimeString's options does not stop its time defaults.
var dateFields = {year: 'numeric', month: 'numeric', day: 'numeric'};
var timeFields = {hour: 'numeric', minute: 'numeric', second: 'numeric'};
/// options plus the defaults CreateDateTimeFormat adds for required and
/// defaults: the date fields, the time fields or both, when options has no
/// field of the required kind and no style.
function withDefaults(options, required, defaults) {
  var out = {};
  var needDefaults = true;
  if (options !== undefined) {
    for (var key in options) {
      out[key] = options[key];
      if ((required === 'date' || required === 'any') && (key === 'weekday' || key === 'year' || key === 'month' || key === 'day'))
        needDefaults = false;
      if ((required === 'time' || required === 'any') && (key === 'dayPeriod' || key === 'hour' || key === 'minute' || key === 'second' || key === 'fractionalSecondDigits'))
        needDefaults = false;
      if (key === 'dateStyle' || key === 'timeStyle') needDefaults = false;
    }
  }
  if (needDefaults) {
    if (defaults === 'date' || defaults === 'all') for (var k in dateFields) out[k] = dateFields[k];
    if (defaults === 'time' || defaults === 'all') for (var k in timeFields) out[k] = timeFields[k];
  }
  return out;
}
var kinds = [
  ['toLocaleString', 'any', 'all'],
  ['toLocaleDateString', 'date', 'date'],
  ['toLocaleTimeString', 'time', 'time'],
];
var localesList = [undefined, 'de', ['th-u-ca-gregory-nu-thai'], 'ja-u-ca-japanese', 'ar-EG', 'zh-u-ca-chinese', 'en-GB'];
var optionsList = [
  undefined,
  {},
  {hour12: false},
  {timeZone: 'Asia/Kolkata', timeZoneName: 'short'},
  {month: 'long', day: 'numeric', hour: '2-digit', minute: '2-digit'},
  {weekday: 'short', era: 'long'},
  {era: 'short'},
  {year: '2-digit'},
  {dateStyle: 'long'},
  {timeStyle: 'medium'},
  {dayPeriod: 'long'},
  {fractionalSecondDigits: 3},
];
var instants = [t, 0, -1, Date.UTC(1989, 10, 9, 17, 57), 8.64e15];
var differences = 0;
var compared = 0;
var typeErrors = 0;
for (var a = 0; a < kinds.length; a++) {
  for (var b = 0; b < localesList.length; b++) {
    for (var c = 0; c < optionsList.length; c++) {
      var options = optionsList[c];
      // A style of the other kind is the required check's TypeError, which
      // the constructor (required any) does not throw.
      if ((kinds[a][1] === 'date' && options !== undefined && options.timeStyle !== undefined) ||
          (kinds[a][1] === 'time' && options !== undefined && options.dateStyle !== undefined)) {
        for (var e = 0; e < instants.length; e++)
          if (thrown(() => new Date(instants[e])[kinds[a][0]](localesList[b], options)) === 'TypeError') typeErrors++;
        continue;
      }
      var dtf = new Intl.DateTimeFormat(localesList[b], withDefaults(options, kinds[a][1], kinds[a][2]));
      for (var e = 0; e < instants.length; e++) {
        var got = new Date(instants[e])[kinds[a][0]](localesList[b], options);
        var expected = dtf.format(instants[e]);
        compared++;
        if (got !== expected) {
          differences++;
          print(kinds[a][0], localesList[b], JSON.stringify(options), esc(got), '!==', esc(expected));
        }
      }
    }
  }
}
print(compared, 'compared,', differences, 'differences,', typeErrors, 'TypeErrors');
// CHECK-NEXT: 1190 compared, 0 differences, 70 TypeErrors
