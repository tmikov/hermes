/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US %s | %FileCheck --match-full-lines %s

// Intl.RelativeTimeFormat: format and formatToParts in each style and
// numeric option, relative names, plural forms, patterns without {0}, the
// unit of each part, numbering systems, extreme values (plan Review Focus
// 4), the errors, the order of the conversions and of the option reads,
// resolvedOptions and supportedLocalesOf. The expected strings are Node's
// (ICU 78.2), except gd's "an-eararais", CLDR's name for +3 days, which
// the draft uses (plan decision 4; Node prints a number), and 1.0005 days
// (below).

var J = JSON.stringify;
/// The class of the exception f throws, or 'none'.
function thrown(f) {
  try {
    f();
    return 'none';
  } catch (e) {
    return e.constructor.name;
  }
}
/// A new Intl.RelativeTimeFormat(locales, options).
function rtf(locales, options) {
  return new Intl.RelativeTimeFormat(locales, options);
}

var en = rtf('en');
var enAuto = rtf('en', {numeric: 'auto'});
// The past pattern carries the sign: the number is the magnitude.
print(J(en.format(-1, 'day')));
// CHECK: "1 day ago"
print(J(enAuto.format(-1, 'day')));
// CHECK-NEXT: "yesterday"
// ToString(-0) is "0", so -0 has the name of 0.
print(J(enAuto.format(-0, 'second')));
// CHECK-NEXT: "now"
// A relative name is one literal part, with no unit.
print(J(enAuto.formatToParts(1, 'days')));
// CHECK-NEXT: [{"type":"literal","value":"tomorrow"}]
print(J(enAuto.formatToParts(1, 'quarter')));
// CHECK-NEXT: [{"type":"literal","value":"next quarter"}]
// Every part of the number has the unit; the pattern's literals do not.
print(J(en.formatToParts(-1234.5, 'hours')));
// CHECK-NEXT: [{"type":"integer","value":"1","unit":"hour"},{"type":"group","value":",","unit":"hour"},{"type":"integer","value":"234","unit":"hour"},{"type":"decimal","value":".","unit":"hour"},{"type":"fraction","value":"5","unit":"hour"},{"type":"literal","value":" hours ago"}]
print(J(rtf('en', {style: 'narrow'}).formatToParts(3, 'month')));
// CHECK-NEXT: [{"type":"literal","value":"in "},{"type":"integer","value":"3","unit":"month"},{"type":"literal","value":"mo"}]
// -0 is in the past, and its number is "0", not "-0".
print(J(rtf('en', {style: 'short'}).formatToParts(-0, 'week')));
// CHECK-NEXT: [{"type":"integer","value":"0","unit":"week"},{"type":"literal","value":" wk. ago"}]
// -2 and +2 have names in de and ja, +3 in gd.
print(J(rtf('de', {numeric: 'auto'}).format(-2, 'day')));
// CHECK-NEXT: "vorgestern"
print(J(rtf('ja', {numeric: 'auto'}).format(2, 'day')));
// CHECK-NEXT: "明後日"
print(J(rtf('gd', {numeric: 'auto'}).format(3, 'day')), J(rtf('gd').format(3, 'day')));
// CHECK-NEXT: "an-eararais" "an ceann 3 làithean"
print(J(rtf('de').formatToParts(1234567.891, 'year')));
// CHECK-NEXT: [{"type":"literal","value":"in "},{"type":"integer","value":"1","unit":"year"},{"type":"group","value":".","unit":"year"},{"type":"integer","value":"234","unit":"year"},{"type":"group","value":".","unit":"year"},{"type":"integer","value":"567","unit":"year"},{"type":"decimal","value":",","unit":"year"},{"type":"fraction","value":"891","unit":"year"},{"type":"literal","value":" Jahren"}]
print(J(rtf('fr').format(-1.5, 'minute')));
// CHECK-NEXT: "il y a 1,5 minute"
// ar's past "one" pattern has no {0}: one literal part.
print(J(rtf('ar').formatToParts(-1, 'second')));
// CHECK-NEXT: [{"type":"literal","value":"قبل ثانية واحدة"}]
print(J(rtf('ar').format(-3, 'hour')));
// CHECK-NEXT: "قبل 3 ساعات"
// he's "two" pattern has the word for two days, and the number in
// parentheses.
print(J(rtf('he').formatToParts(2, 'day')));
// CHECK-NEXT: [{"type":"literal","value":"בעוד יומיים ("},{"type":"integer","value":"2","unit":"day"},{"type":"literal","value":")"}]
// pl's "few" form; ru's narrow future pattern has a literal "+".
print(J(rtf('pl', {style: 'short'}).format(-2, 'year')));
// CHECK-NEXT: "2 lata temu"
print(J(rtf('ru', {style: 'narrow'}).format(21, 'second')));
// CHECK-NEXT: "+21 с"

// Numbering systems, from the -u-nu keyword and from the option.
print(J(rtf('en-u-nu-arab').formatToParts(1000, 'second')));
// CHECK-NEXT: [{"type":"literal","value":"in "},{"type":"integer","value":"١","unit":"second"},{"type":"group","value":"٬","unit":"second"},{"type":"integer","value":"٠٠٠","unit":"second"},{"type":"literal","value":" seconds"}]
print(J(rtf('en', {numberingSystem: 'hanidec'}).format(12, 'day')));
// CHECK-NEXT: "in 一二 days"

// Extreme values (plan Review Focus 4).
print(J(en.format(1e21, 'day')));
// CHECK-NEXT: "in 1,000,000,000,000,000,000,000 days"
// The plural category comes from the same rounding as the digits.
print(J(en.format(0.0001, 'day')));
// CHECK-NEXT: "in 0 days"
print(J(en.format(-Number.MIN_VALUE, 'second')));
// CHECK-NEXT: "0 seconds ago"
print(J(en.format(Number.MAX_VALUE, 'year').slice(0, 14)));
// CHECK-NEXT: "in 179,769,313"
// The Number is read as NumberFormat reads it (ToIntlMathematicalValue,
// through Number::toString), not as its exact value, which the draft's
// PartitionRelativeTimePattern step 11 says: 1000000000000000100 is exactly
// 1000000000000000128, and its digits are NumberFormat's.
var nfEn = new Intl.NumberFormat('en');
print(J(en.format(1000000000000000100, 'day')), en.format(1000000000000000100, 'day') === 'in ' + nfEn.format(1000000000000000100) + ' days');
// CHECK-NEXT: "in 1,000,000,000,000,000,100 days" true
// 1.0005 is exactly 1.000499999999999944..., but its Number::toString is
// "1.0005", which rounds half-expand to 1.001, as NumberFormat gives. Node
// formats the exact value here ("in 1 day") although its NumberFormat
// gives 1.001.
print(J(en.format(1.0005, 'day')), J(nfEn.format(1.0005)));
// CHECK-NEXT: "in 1.001 days" "1.001"
print(
  thrown(() => en.format(NaN, 'day')),
  thrown(() => en.format(Infinity, 'day')),
  thrown(() => en.formatToParts(-Infinity, 'day')),
  thrown(() => en.format(1, 'days ')),
  thrown(() => en.format(1, 'Day')),
  thrown(() => en.formatToParts(1, 'millisecond')),
);
// CHECK-NEXT: RangeError RangeError RangeError RangeError RangeError RangeError
// The value is converted before the unit.
var order = [];
var value = {
  valueOf() {
    order.push('value');
    throw new SyntaxError();
  },
};
var unit = {
  toString() {
    order.push('unit');
    throw new EvalError();
  },
};
print(thrown(() => en.format(value, unit)), thrown(() => en.formatToParts(value, unit)), order.join());
// CHECK-NEXT: SyntaxError SyntaxError value,value
// NaN is rejected after the unit is converted but before the unit is
// validated.
print(thrown(() => en.format(NaN, unit)), thrown(() => en.format('  6  ', 'days')), en.format('  6  ', 'days'));
// CHECK-NEXT: EvalError none in 6 days

// resolvedOptions.
print(J(rtf('en-u-nu-thai', {style: 'short'}).resolvedOptions()));
// CHECK-NEXT: {"locale":"en-u-nu-thai","style":"short","numeric":"always","numberingSystem":"thai"}
// native is not a numbering system of RelativeTimeFormat's locale data.
print(J(rtf('de-u-nu-native').resolvedOptions()));
// CHECK-NEXT: {"locale":"de","style":"long","numeric":"always","numberingSystem":"latn"}
// ca is not a relevant extension key.
print(J(rtf('en-u-ca-japanese-nu-latn').resolvedOptions()));
// CHECK-NEXT: {"locale":"en-u-nu-latn","style":"long","numeric":"always","numberingSystem":"latn"}
// The option overrides the keyword, which then leaves the locale.
print(J(rtf('en-u-nu-thai', {numberingSystem: 'arab', numeric: 'auto'}).resolvedOptions()));
// CHECK-NEXT: {"locale":"en","style":"long","numeric":"auto","numberingSystem":"arab"}
// A fresh object each time.
print(en.resolvedOptions() !== en.resolvedOptions());
// CHECK-NEXT: true

// The constructor: options null throws (coerce-options), a primitive is
// converted with ToObject, and new is required.
print(thrown(() => rtf('en', null)), thrown(() => Intl.RelativeTimeFormat()), thrown(() => Intl.RelativeTimeFormat('en')));
// CHECK-NEXT: TypeError TypeError TypeError
print(J(rtf('en', 5).resolvedOptions().style));
// CHECK-NEXT: "long"
print(thrown(() => rtf('en', {style: 'wide'})), thrown(() => rtf('en', {numeric: 'never'})), thrown(() => rtf('en', {numberingSystem: 'a'})));
// CHECK-NEXT: RangeError RangeError RangeError
// The options are read in the order localeMatcher, numberingSystem, style,
// numeric.
var read = [];
rtf(
  'en',
  new Proxy(
    {},
    {
      get(t, k) {
        read.push(String(k));
        return undefined;
      },
    },
  ),
);
print(read.join());
// CHECK-NEXT: localeMatcher,numberingSystem,style,numeric

// supportedLocalesOf.
print(J(Intl.RelativeTimeFormat.supportedLocalesOf(['de', 'xx', 'th-TH'])));
// CHECK-NEXT: ["de","th-TH"]
