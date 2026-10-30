/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US -Xintl-default-time-zone=UTC %s | %FileCheck --match-full-lines %s

// The Intl.DateTimeFormat constructor and resolvedOptions: the defaults,
// the hour cycle options (plan decision 8), the styles, the calendars and
// numbering systems (decision 9), the component options and the format
// matchers (decision 6), and the constructor's other entry points.

/// The resolvedOptions of new Intl.DateTimeFormat(locales, options) as
/// JSON, or the class of the exception the constructor throws.
function ro(locales, options) {
  try {
    return JSON.stringify(new Intl.DateTimeFormat(locales, options).resolvedOptions());
  } catch (e) {
    return e.constructor.name;
  }
}

// The defaults: year, month and day numeric, no hour cycle.
print(ro());
// CHECK: {"locale":"en-US","calendar":"gregory","numberingSystem":"latn","timeZone":"UTC","year":"numeric","month":"numeric","day":"numeric"}
// An hour and a minute: the locale's 12-hour cycle, and the minute widened
// to the record's "2-digit" (best fit's width adjustment).
print(ro(undefined, {hour: 'numeric', minute: 'numeric'}));
// CHECK-NEXT: {"locale":"en-US","calendar":"gregory","numberingSystem":"latn","timeZone":"UTC","hourCycle":"h12","hour12":true,"hour":"numeric","minute":"2-digit"}

// hour12 and hourCycle. hour12: false takes the locale's 24-hour cycle;
// ja's hour12: true its first 12-hour one, h11 (CLDR's JP allows H K h).
print(ro('en', {hour12: false, hour: 'numeric'}));
// CHECK-NEXT: {"locale":"en","calendar":"gregory","numberingSystem":"latn","timeZone":"UTC","hourCycle":"h23","hour12":false,"hour":"2-digit"}
print(ro('ja', {hour12: true, hour: 'numeric'}));
// CHECK-NEXT: {"locale":"ja","calendar":"gregory","numberingSystem":"latn","timeZone":"UTC","hourCycle":"h11","hour12":true,"hour":"numeric"}
print(ro('ja', {hour12: false, hour: 'numeric'}));
// CHECK-NEXT: {"locale":"ja","calendar":"gregory","numberingSystem":"latn","timeZone":"UTC","hourCycle":"h23","hour12":false,"hour":"numeric"}
// hour12 wins over hourCycle.
print(ro('en', {hour12: true, hourCycle: 'h23', hour: 'numeric'}));
// CHECK-NEXT: {"locale":"en","calendar":"gregory","numberingSystem":"latn","timeZone":"UTC","hourCycle":"h12","hour12":true,"hour":"numeric"}
print(ro('en', {hour12: false, hourCycle: 'h11', hour: 'numeric'}));
// CHECK-NEXT: {"locale":"en","calendar":"gregory","numberingSystem":"latn","timeZone":"UTC","hourCycle":"h23","hour12":false,"hour":"2-digit"}
print(ro('en', {hourCycle: 'h24', hour: 'numeric'}).indexOf('"hourCycle":"h24","hour12":false') !== -1);
// CHECK-NEXT: true
print(ro('en', {hourCycle: 'h11', hour: 'numeric'}).indexOf('"hourCycle":"h11","hour12":true') !== -1);
// CHECK-NEXT: true
// The -u-hc keyword is used, and kept in the locale; with hour12 it is
// dropped, as is a keyword the hourCycle option overrides.
print(ro('en-u-hc-h23', {hour: 'numeric'}));
// CHECK-NEXT: {"locale":"en-u-hc-h23","calendar":"gregory","numberingSystem":"latn","timeZone":"UTC","hourCycle":"h23","hour12":false,"hour":"2-digit"}
print(ro('en-u-hc-h23', {hour12: true, hour: 'numeric'}));
// CHECK-NEXT: {"locale":"en","calendar":"gregory","numberingSystem":"latn","timeZone":"UTC","hourCycle":"h12","hour12":true,"hour":"numeric"}
print(ro('en-u-hc-h23', {hourCycle: 'h11', hour: 'numeric'}).indexOf('"locale":"en","calendar":"gregory","numberingSystem":"latn","timeZone":"UTC","hourCycle":"h11"') !== -1);
// CHECK-NEXT: true
print(ro('en', {hourCycle: 'h13'}), ro('en', {hour12: true, hourCycle: 'h13'}));
// CHECK-NEXT: RangeError RangeError
// Without an hour, there is no hour cycle to report.
print(ro('en', {hour12: true}));
// CHECK-NEXT: {"locale":"en","calendar":"gregory","numberingSystem":"latn","timeZone":"UTC","year":"numeric","month":"numeric","day":"numeric"}

// The styles: dateStyle and timeStyle, the hour cycle of the time, and no
// component fields; component options with a style are a TypeError.
print(ro('en', {dateStyle: 'full', timeStyle: 'short'}));
// CHECK-NEXT: {"locale":"en","calendar":"gregory","numberingSystem":"latn","timeZone":"UTC","hourCycle":"h12","hour12":true,"dateStyle":"full","timeStyle":"short"}
print(ro('en', {dateStyle: 'medium'}));
// CHECK-NEXT: {"locale":"en","calendar":"gregory","numberingSystem":"latn","timeZone":"UTC","dateStyle":"medium"}
print(ro('en-GB', {timeStyle: 'long'}));
// CHECK-NEXT: {"locale":"en-GB","calendar":"gregory","numberingSystem":"latn","timeZone":"UTC","hourCycle":"h23","hour12":false,"timeStyle":"long"}
print(ro('en-GB', {timeStyle: 'short', hour12: true}));
// CHECK-NEXT: {"locale":"en-GB","calendar":"gregory","numberingSystem":"latn","timeZone":"UTC","hourCycle":"h12","hour12":true,"timeStyle":"short"}
print(ro('en', {dateStyle: 'short', hour: 'numeric'}), ro('en', {timeStyle: 'short', fractionalSecondDigits: 1}));
// CHECK-NEXT: TypeError TypeError
print(ro('en', {dateStyle: 'x'}), ro('en', {timeStyle: 'Short'}));
// CHECK-NEXT: RangeError RangeError

// Calendars without eras construct, asking native code for no era names.
print(ro('en', {calendar: 'iso8601', era: 'short', year: 'numeric'}));
// CHECK-NEXT: {"locale":"en","calendar":"iso8601","numberingSystem":"latn","timeZone":"UTC","era":"short","year":"numeric"}
print(JSON.parse(ro('en', {calendar: 'chinese', era: 'short', year: 'numeric'})).calendar);
// CHECK-NEXT: chinese
print(JSON.parse(ro('en', {calendar: 'dangi', era: 'long', year: 'numeric', month: 'long'})).calendar);
// CHECK-NEXT: dangi

// The calendar: the locale's preferred one, the -u-ca keyword, an option
// for a calendar ICU4X does not implement (the locale's default), an
// alias, and a value that is not a type (RangeError). CLDR 49 prefers
// gregory to islamic-umalqura in SA, as Node does.
print(JSON.parse(ro('th')).calendar, JSON.parse(ro('fa')).calendar, JSON.parse(ro('ar-SA')).calendar);
// CHECK-NEXT: buddhist persian gregory
print(JSON.parse(ro('ar-SA-u-ca-islamic-umalqura')).calendar, JSON.parse(ro('ar-SA-u-ca-islamic')).locale);
// CHECK-NEXT: islamic-umalqura ar-SA
print(ro('en-u-ca-japanese'));
// CHECK-NEXT: {"locale":"en-u-ca-japanese","calendar":"japanese","numberingSystem":"latn","timeZone":"UTC","era":"narrow","year":"numeric","month":"numeric","day":"numeric"}
print(ro('en', {calendar: 'islamic'}), ro('en-u-ca-islamic-rgsa'));
// CHECK-NEXT: {"locale":"en","calendar":"gregory","numberingSystem":"latn","timeZone":"UTC","year":"numeric","month":"numeric","day":"numeric"} {"locale":"en","calendar":"gregory","numberingSystem":"latn","timeZone":"UTC","year":"numeric","month":"numeric","day":"numeric"}
print(JSON.parse(ro('en', {calendar: 'ethiopic-amete-alem'})).calendar, JSON.parse(ro('en-u-ca-ethiopic-amete-alem')).locale);
// CHECK-NEXT: ethioaa en-u-ca-ethioaa
// The option wins over the keyword, which is then dropped.
print(ro('en-u-ca-buddhist', {calendar: 'hebrew'}).indexOf('{"locale":"en","calendar":"hebrew"') === 0);
// CHECK-NEXT: true
print(ro('en', {calendar: 'gregory!'}), ro('en', {calendar: ''}));
// CHECK-NEXT: RangeError RangeError
// Every supported calendar resolves to itself.
var calendars = Intl.supportedValuesOf('calendar');
var wrong = calendars.filter(c => JSON.parse(ro('en', {calendar: c})).calendar !== c);
print(calendars.length, JSON.stringify(wrong));
// CHECK-NEXT: 16 []

// The numbering system: the locale's default, an option, a keyword.
print(JSON.parse(ro('ar-EG')).numberingSystem, JSON.parse(ro('en', {numberingSystem: 'thai'})).numberingSystem);
// CHECK-NEXT: arab thai
print(ro('de-u-nu-deva').indexOf('{"locale":"de-u-nu-deva","calendar":"gregory","numberingSystem":"deva"') === 0);
// CHECK-NEXT: true
print(ro('en', {numberingSystem: 'xyz'}), ro('en', {numberingSystem: 'latn!'}));
// CHECK-NEXT: {"locale":"en","calendar":"gregory","numberingSystem":"latn","timeZone":"UTC","year":"numeric","month":"numeric","day":"numeric"} RangeError

// The components: era alone adds year, month and day; an invalid
// fractionalSecondDigits or component value is a RangeError.
print(ro('en', {era: 'short'}));
// CHECK-NEXT: {"locale":"en","calendar":"gregory","numberingSystem":"latn","timeZone":"UTC","era":"short","year":"numeric","month":"numeric","day":"numeric"}
print(ro('en', {fractionalSecondDigits: 0}), ro('en', {fractionalSecondDigits: 4}), ro('en', {month: 'longer'}), ro('en', {timeZoneName: 'shortish'}));
// CHECK-NEXT: RangeError RangeError RangeError RangeError
print(ro('en', {fractionalSecondDigits: 2.9}));
// CHECK-NEXT: {"locale":"en","calendar":"gregory","numberingSystem":"latn","timeZone":"UTC","fractionalSecondDigits":2}
print(ro('en', {weekday: 'long', timeZoneName: 'short'}));
// CHECK-NEXT: {"locale":"en","calendar":"gregory","numberingSystem":"latn","timeZone":"UTC","weekday":"long","timeZoneName":"short"}
print(ro('en', {dayPeriod: 'short'}));
// CHECK-NEXT: {"locale":"en","calendar":"gregory","numberingSystem":"latn","timeZone":"UTC","dayPeriod":"short"}
// The basic matcher reports the record's widths; best fit adjusts them to
// the request.
print(ro('en', {formatMatcher: 'basic', month: '2-digit', day: '2-digit'}));
// CHECK-NEXT: {"locale":"en","calendar":"gregory","numberingSystem":"latn","timeZone":"UTC","month":"numeric","day":"numeric"}
print(ro('en', {month: '2-digit', day: '2-digit'}));
// CHECK-NEXT: {"locale":"en","calendar":"gregory","numberingSystem":"latn","timeZone":"UTC","month":"2-digit","day":"2-digit"}
print(ro('en', {formatMatcher: 'best'}));
// CHECK-NEXT: RangeError

// Options are read with ToString and ToNumber, after ToObject of options.
print(ro('en', {hour: {toString: () => 'numeric'}, fractionalSecondDigits: '3'}));
// CHECK-NEXT: {"locale":"en","calendar":"gregory","numberingSystem":"latn","timeZone":"UTC","hourCycle":"h12","hour12":true,"hour":"numeric","minute":"2-digit","second":"2-digit","fractionalSecondDigits":3}
print(ro('en', null), ro('en', 5) === ro('en'));
// CHECK-NEXT: TypeError true

// Without new, Intl.DateTimeFormat makes a new DateTimeFormat.
var called = Intl.DateTimeFormat('de');
print(called instanceof Intl.DateTimeFormat, called.resolvedOptions().locale);
// CHECK-NEXT: true de
print(JSON.stringify(Intl.DateTimeFormat.supportedLocalesOf(['de', 'xx'])));
// CHECK-NEXT: ["de"]
print(JSON.stringify(Intl.DateTimeFormat.supportedLocalesOf('en-u-ca-japanese', {localeMatcher: 'lookup'})));
// CHECK-NEXT: ["en-u-ca-japanese"]
var dtf = new Intl.DateTimeFormat();
print(typeof dtf.format, dtf.format === dtf.format, dtf.format.length, dtf.format.name === '');
// CHECK-NEXT: function true 1 true
print(Object.prototype.toString.call(dtf));
// CHECK-NEXT: [object Intl.DateTimeFormat]
// A resolvedOptions result is a new ordinary object each time.
print(dtf.resolvedOptions() !== dtf.resolvedOptions(), Object.getPrototypeOf(dtf.resolvedOptions()) === Object.prototype);
// CHECK-NEXT: true true

// Best-fit and basic matches are cached in the locale data, keyed by the
// matcher, the hour-cycle family and the requested fields
// (cachedFormatMatch, a refinement of decision 13), 32 entries, emptied
// when full. The cache is unobservable: 104 requests over 64 distinct
// keys (more than it holds), in two rounds, with each matcher and both
// hour cycles, give the same strings, and the requests that differ only
// in the matcher or the hour cycle keep their own results.
var cacheBags = [];
var cacheFields = [['minute', '2-digit'], ['month', 'narrow'], ['hour', 'numeric'], ['weekday', 'long'], ['year', '2-digit']];
for (var i = 0; i < cacheFields.length; i++) {
  for (var j = 0; j < cacheFields.length; j++) {
    var bag = {timeZone: 'UTC'};
    bag[cacheFields[i][0]] = cacheFields[i][1];
    if (j !== i) bag[cacheFields[j][0]] = cacheFields[j][1];
    cacheBags.push(bag);
  }
}
cacheBags.push({timeZone: 'UTC', dayPeriod: 'short', minute: 'numeric'});
function cacheRound() {
  var out = [];
  for (var k = 0; k < cacheBags.length; k++) {
    var options = cacheBags[k];
    var matchers = ['basic', 'best fit'];
    for (var m = 0; m < 2; m++) {
      for (var h = 0; h < 2; h++) {
        var f = new Intl.DateTimeFormat('en', Object.assign({formatMatcher: matchers[m], hour12: h === 0}, options));
        out.push(f.format(Date.UTC(2026, 0, 5, 13, 7)) + '|' + f.formatRange(Date.UTC(2026, 0, 5, 9, 7), Date.UTC(2026, 0, 5, 20, 7)));
      }
    }
  }
  return out.join('\n');
}
var round1 = cacheRound();
print(cacheBags.length, round1 === cacheRound());
// CHECK-NEXT: 26 true
function cached(options) {
  return new Intl.DateTimeFormat('en', Object.assign({timeZone: 'UTC'}, options)).format(Date.UTC(2026, 0, 5, 13, 7)).replace(' ', ' ');
}
print(cached({minute: '2-digit'}), cached({minute: '2-digit', formatMatcher: 'basic'}), cached({month: 'narrow'}), cached({month: 'narrow', formatMatcher: 'basic'}));
// CHECK-NEXT: 07 7 J Jan
print(cached({hour: 'numeric', hour12: true}), cached({hour: 'numeric', hour12: false}), cached({hour: 'numeric', hour12: true}));
// CHECK-NEXT: 1 PM 13 1 PM
