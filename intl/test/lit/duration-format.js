/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: cat %S/Inputs/escape.js %s > %t.js
// RUN: %hermes -Xintl-default-locale=en-US %t.js | %FileCheck --match-full-lines %s

// Intl.DurationFormat: format and formatToParts in each style, the unit of
// each part, the digital style and its separators, signs, numeric styles
// after other units, fractional units, other locales and numbering
// systems, resolvedOptions, the errors, the order of the field and option
// reads, and supportedLocalesOf. The expected strings are Node's (ICU
// 78.2), except where Node does not follow the draft (NaN is not integral)
// and where CLDR 49 changed the data (he's unit list).

var J = stringify;
/// The class of the exception f throws, or 'none'.
function thrown(f) {
  try {
    f();
    return 'none';
  } catch (e) {
    return e.constructor.name;
  }
}
/// A new Intl.DurationFormat(locales, options).
function df(locales, options) {
  return new Intl.DurationFormat(locales, options);
}
/// format and formatToParts of duration with a new
/// Intl.DurationFormat(locales, options).
function both(locales, options, duration) {
  var f = df(locales, options);
  print(J(f.format(duration)));
  print(J(f.formatToParts(duration)));
}

// The default style, "short", with every unit.
var all = {years: 1, months: 2, weeks: 3, days: 4, hours: 5, minutes: 6, seconds: 7, milliseconds: 8, microseconds: 9, nanoseconds: 10};
var enShort = df('en');
print(J(enShort.format(all)));
// CHECK: "1 yr, 2 mths, 3 wks, 4 days, 5 hr, 6 min, 7 sec, 8 ms, 9 μs, 10 ns"
// Every part of a number has its unit; the list literals have none.
print(J(enShort.formatToParts(all).slice(0, 4)));
// CHECK-NEXT: [{"type":"integer","value":"1","unit":"year"},{"type":"literal","value":" ","unit":"year"},{"type":"unit","value":"yr","unit":"year"},{"type":"literal","value":", "}]
print(J(df('en', {style: 'long'}).format({years: 1, days: 2, hours: 3})));
// CHECK-NEXT: "1 year, 2 days, 3 hours"
both('en', {style: 'narrow'}, {hours: 1, minutes: 2, seconds: 3});
// CHECK-NEXT: "1h 2m 3s"
// CHECK-NEXT: [{"type":"integer","value":"1","unit":"hour"},{"type":"unit","value":"h","unit":"hour"},{"type":"literal","value":" "},{"type":"integer","value":"2","unit":"minute"},{"type":"unit","value":"m","unit":"minute"},{"type":"literal","value":" "},{"type":"integer","value":"3","unit":"second"},{"type":"unit","value":"s","unit":"second"}]

// The digital style: the separators have no unit, and the fraction is part
// of the seconds.
var digital = df('en', {style: 'digital'});
both('en', {style: 'digital'}, {hours: 1, minutes: 2, seconds: 3, milliseconds: 456});
// CHECK-NEXT: "1:02:03.456"
// CHECK-NEXT: [{"type":"integer","value":"1","unit":"hour"},{"type":"literal","value":":"},{"type":"integer","value":"02","unit":"minute"},{"type":"literal","value":":"},{"type":"integer","value":"03","unit":"second"},{"type":"decimal","value":".","unit":"second"},{"type":"fraction","value":"456","unit":"second"}]
// Days stay a unit; hours, minutes and seconds are always displayed.
print(J(digital.format({days: 1, minutes: 5})));
// CHECK-NEXT: "1 day, 0:05:00"
// fractionalDigits truncates.
print(J(df('en', {style: 'digital', fractionalDigits: 2}).format({seconds: 3, milliseconds: 456, microseconds: 789})));
// CHECK-NEXT: "0:00:03.45"
// The first unit displayed carries the sign.
both('en', {style: 'digital'}, {hours: -1, minutes: -2, seconds: -3});
// CHECK-NEXT: "-1:02:03"
// CHECK-NEXT: [{"type":"minusSign","value":"-","unit":"hour"},{"type":"integer","value":"1","unit":"hour"},{"type":"literal","value":":"},{"type":"integer","value":"02","unit":"minute"},{"type":"literal","value":":"},{"type":"integer","value":"03","unit":"second"}]
// Hours not displayed: no separator before the minutes.
print(J(df('en', {style: 'digital', hoursDisplay: 'auto'}).format({minutes: 5, seconds: 6})));
// CHECK-NEXT: "05:06"

// A zero first unit is not displayed, so the sign moves to the next one.
print(J(enShort.format({hours: 0, minutes: -2, seconds: -3})));
// CHECK-NEXT: "-2 min, 3 sec"
// Numeric styles that follow a numeric unit: the minutes are displayed
// between displayed hours and seconds.
print(J(df('en', {hours: 'numeric'}).format({hours: 1, seconds: 5})));
// CHECK-NEXT: "1:00:05"
print(J(df('en', {minutes: 'numeric'}).format({hours: 2, minutes: 3, seconds: 4})));
// CHECK-NEXT: "2 hr, 3:04"
print(J(df('en', {seconds: 'numeric', fractionalDigits: 3}).format({seconds: 1, milliseconds: 2, microseconds: 3})));
// CHECK-NEXT: "1.002"
// Fractional milliseconds fold into the seconds, which keep their unit.
print(J(df('en', {milliseconds: 'numeric'}).format({seconds: 1, milliseconds: 2, microseconds: 3, nanoseconds: 4})));
// CHECK-NEXT: "1.002003004 sec"
// Grouping outside the digital style.
print(J(enShort.format({seconds: 10000000, nanoseconds: 1})));
// CHECK-NEXT: "10,000,000 sec, 1 ns"

// ToIntegerIfIntegral returns ℝ(-0𝔽), which is 0: nothing is displayed,
// and with display "always" the zero has no sign (plan decision 7).
print(J(enShort.format({years: -0})), J(df('en', {yearsDisplay: 'always'}).format({years: -0})));
// CHECK-NEXT: "" "0 yrs"

// Other locales.
print(J(df('de', {style: 'long'}).format({hours: 1, minutes: 30})));
// CHECK-NEXT: "1 Stunde, 30 Minuten"
print(J(df('es').format({years: 2, months: 1, days: 3})));
// CHECK-NEXT: "2 a, 1 m., 3 d"
print(J(df('fr', {style: 'digital'}).format({hours: 12, minutes: 3, seconds: 4, milliseconds: 5})));
// CHECK-NEXT: "12:03:04,005"
// da's time separator is "." (plan decision 7).
print(J(df('da', {style: 'digital'}).format({hours: 1, minutes: 2, seconds: 3})));
// CHECK-NEXT: "1.02.03"
// ar's dual has no number part.
both('ar', {style: 'long'}, {hours: 2, minutes: 5});
// CHECK-NEXT: "ساعتان و5 دقائق"
// CHECK-NEXT: [{"type":"unit","value":"ساعتان","unit":"hour"},{"type":"literal","value":" و"},{"type":"integer","value":"5","unit":"minute"},{"type":"literal","value":" ","unit":"minute"},{"type":"unit","value":"دقائق","unit":"minute"}]
print(J(df('ar-u-nu-arab', {style: 'digital'}).format({hours: 1, minutes: 2, seconds: 3})));
// CHECK-NEXT: "١:٠٢:٠٣"
print(J(df('ja', {style: 'narrow'}).format({hours: 1, minutes: 2, seconds: 3})));
// CHECK-NEXT: "1h2m3s"
// he's unit list joins every element with ", " in CLDR 49, where Node's
// CLDR 48 ends in "{0} ו-{1}".
print(J(df('he', {style: 'long'}).format({hours: 1, minutes: 2, seconds: 11})));
// CHECK-NEXT: "1 שעה, שתי דקות, 11 שניות"

// resolvedOptions: the draft's order, no fractionalDigits when it is
// undefined, and "numeric" for a fractional unit.
print(JSON.stringify(df('en').resolvedOptions()));
// CHECK-NEXT: {"locale":"en","numberingSystem":"latn","style":"short","years":"short","yearsDisplay":"auto","months":"short","monthsDisplay":"auto","weeks":"short","weeksDisplay":"auto","days":"short","daysDisplay":"auto","hours":"short","hoursDisplay":"auto","minutes":"short","minutesDisplay":"auto","seconds":"short","secondsDisplay":"auto","milliseconds":"short","millisecondsDisplay":"auto","microseconds":"short","microsecondsDisplay":"auto","nanoseconds":"short","nanosecondsDisplay":"auto"}
print(JSON.stringify(df('en', {style: 'digital', fractionalDigits: 3}).resolvedOptions()));
// CHECK-NEXT: {"locale":"en","numberingSystem":"latn","style":"digital","years":"short","yearsDisplay":"auto","months":"short","monthsDisplay":"auto","weeks":"short","weeksDisplay":"auto","days":"short","daysDisplay":"auto","hours":"numeric","hoursDisplay":"always","minutes":"2-digit","minutesDisplay":"always","seconds":"2-digit","secondsDisplay":"always","milliseconds":"numeric","millisecondsDisplay":"auto","microseconds":"numeric","microsecondsDisplay":"auto","nanoseconds":"numeric","nanosecondsDisplay":"auto","fractionalDigits":3}
// The numbering system from the locale and from the option.
print(df('en-u-nu-thai').resolvedOptions().numberingSystem, df('en', {numberingSystem: 'arab'}).resolvedOptions().numberingSystem, df('ar-EG').resolvedOptions().numberingSystem);
// CHECK-NEXT: thai arab arab

// The errors.
print(thrown(() => Intl.DurationFormat()), thrown(() => df('en', null)), thrown(() => df('en', 'short')));
// CHECK-NEXT: TypeError TypeError TypeError
// A unit after a numeric unit must be numeric; a fractional unit cannot be
// displayed always.
print(thrown(() => df('en', {style: 'digital', minutes: 'long'})), thrown(() => df('en', {milliseconds: 'numeric', millisecondsDisplay: 'always'})));
// CHECK-NEXT: RangeError RangeError
print(thrown(() => df('en', {style: 'x'})), thrown(() => df('en', {years: 'numeric'})), thrown(() => df('en', {numberingSystem: 'x'})));
// CHECK-NEXT: RangeError RangeError RangeError
print(thrown(() => enShort.format('PT1H')), thrown(() => enShort.format(1)), thrown(() => enShort.format({})), thrown(() => enShort.format()));
// CHECK-NEXT: RangeError TypeError TypeError TypeError
print(thrown(() => enShort.format({hours: 1.5})), thrown(() => enShort.format({hours: 1, minutes: -1})), thrown(() => enShort.format({hours: Infinity})), thrown(() => enShort.formatToParts({hours: NaN})));
// CHECK-NEXT: RangeError RangeError RangeError RangeError
// NaN is not an integral Number (ToIntegerIfIntegral step 2); Node formats
// it as 0. A field that is undefined counts as absent; any other value is converted.
print(thrown(() => enShort.format({hours: undefined})), J(enShort.format({hours: '2', minutes: null, seconds: true})));
// CHECK-NEXT: TypeError "2 hr, 1 sec"

// The fields are read in alphabetical order, each converted after its Get.
var log = [];
var traced = new Proxy({}, {
  get(target, key) {
    log.push('get ' + String(key));
    return {
      valueOf() {
        log.push('valueOf ' + String(key));
        return 1;
      },
    };
  },
});
enShort.format(traced);
print(log.join(', '));
// CHECK-NEXT: get days, valueOf days, get hours, valueOf hours, get microseconds, valueOf microseconds, get milliseconds, valueOf milliseconds, get minutes, valueOf minutes, get months, valueOf months, get nanoseconds, valueOf nanoseconds, get seconds, valueOf seconds, get weeks, valueOf weeks, get years, valueOf years

// The options are read in the draft's order.
log = [];
var tracedOptions = new Proxy({}, {
  get(target, key) {
    log.push(String(key));
    return undefined;
  },
});
df('en', tracedOptions);
print(log.join(','));
// CHECK-NEXT: localeMatcher,numberingSystem,style,years,yearsDisplay,months,monthsDisplay,weeks,weeksDisplay,days,daysDisplay,hours,hoursDisplay,minutes,minutesDisplay,seconds,secondsDisplay,milliseconds,millisecondsDisplay,microseconds,microsecondsDisplay,nanoseconds,nanosecondsDisplay,fractionalDigits

// supportedLocalesOf.
print(JSON.stringify(Intl.DurationFormat.supportedLocalesOf(['de', 'xx', 'th-TH'])));
// CHECK-NEXT: ["de","th-TH"]
