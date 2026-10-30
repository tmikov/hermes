/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US %s | %FileCheck --match-full-lines %s

// Intl.DisplayNames: of for the six types in each style, the composition of
// language names (Phase 5 plan decision 5), CanonicalCodeForDisplayNames and
// IsValidDateTimeFieldCode, the fallback, extreme codes (plan Review Focus
// 4), resolvedOptions, and the constructor's errors and order of option
// reads. The expected strings are Node's (ICU 78.2) except where the draft
// differs from V8, as decision 5 says: region, script and calendar codes are
// canonicalized before the lookup ("us" and "latn" have names), "und" has
// CLDR's name, and a language code with a part that has no name has no name
// ("en-ZY" falls back to the code, where V8 gives "English (ZY)").

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
/// new Intl.DisplayNames(locale, {type, fallback: "none"}) with the extra
/// options extra, applied to code, as JSON, or the class of the exception.
function of(locale, type, code, extra) {
  var options = {type: type, fallback: 'none'};
  for (var k in extra) options[k] = extra[k];
  try {
    return J(new Intl.DisplayNames(locale, options).of(code));
  } catch (e) {
    return e.constructor.name;
  }
}
var short = {style: 'short'};
var narrow = {style: 'narrow'};
var code = {fallback: 'code'};
var standard = {languageDisplay: 'standard'};
var shortStandard = {style: 'short', languageDisplay: 'standard'};

// Regions.
print(of('en', 'region', 'US'), of('en', 'region', 'US', short), of('en', 'region', 'US', narrow));
// CHECK: "United States" "US" "US"
print(of('en', 'region', 'GB', short));
// CHECK-NEXT: "UK"
// The draft uppercases the code before the lookup.
print(of('en', 'region', 'us'));
// CHECK-NEXT: "United States"
// Three letters are not a unicode_region_subtag; CLDR names ZZ.
print(of('en', 'region', 'ZZZ'), of('en', 'region', 'ZZ'));
// CHECK-NEXT: RangeError "Unknown Region"
print(of('en', 'region', 'ZY', code), of('en', 'region', 'ZY'));
// CHECK-NEXT: "ZY" undefined

// Scripts, title-cased before the lookup.
print(of('en', 'script', 'Hant'), of('en', 'script', 'latn'));
// CHECK-NEXT: "Traditional" "Latin"

// Languages: dialect names, the standard composition, short qualifiers.
print(of('en', 'language', 'en-US'), of('en', 'language', 'en-US', short));
// CHECK-NEXT: "American English" "US English"
print(of('en', 'language', 'en-US', standard), of('en', 'language', 'en-US', shortStandard));
// CHECK-NEXT: "English (United States)" "English (US)"
// With both a script and a region there is no dialect lookup.
print(
  of('en', 'language', 'zh-Hans-CN'),
  of('en', 'language', 'zh-Hans-CN', short),
  of('en', 'language', 'zh-Hans-CN', narrow),
  of('en', 'language', 'zh-Hans-CN', standard),
  of('en', 'language', 'zh-Hans-CN', shortStandard),
  of('en', 'language', 'zh-Hans-CN', {style: 'narrow', languageDisplay: 'standard'}),
);
// CHECK-NEXT: "Chinese (Simplified, China)" "Chinese (Simplified, China)" "Chinese (Simplified, China)" "Chinese (Simplified, China)" "Chinese (Simplified, China)" "Chinese (Simplified, China)"
// A part without a name gives the code, never a partial name.
print(of('en', 'language', 'en-ZY', code), of('en', 'language', 'en-ZY'));
// CHECK-NEXT: "en-ZY" undefined
print(of('en', 'language', 'und'));
// CHECK-NEXT: "Unknown language"
// The fallback is the canonical code.
print(of('en', 'language', 'sgn-GR', code));
// CHECK-NEXT: "gss"
// The locale's patterns and separators, and the short region in short.
// CLDR 49 has no de short name for en-GB (CLDR 48's "Englisch (GB)"), so
// the short dialect name is composed like the standard one.
print(of('de', 'language', 'zh-Hans-CN'), of('de', 'language', 'en-GB', short), of('de', 'language', 'en-GB', shortStandard));
// CHECK-NEXT: "Chinesisch (Vereinfacht, China)" "Englisch (UK)" "Englisch (UK)"
print(of('zh-Hant', 'language', 'sr-Latn-RS'), of('zh-Hant', 'language', 'zh-Hant'), of('zh-Hant', 'language', 'zh-Hant', standard));
// CHECK-NEXT: "塞爾維亞文（拉丁字母，塞爾維亞）" "繁體中文" "中文（繁體）"
print(of('ar', 'language', 'zh-Hans-CN'));
// CHECK-NEXT: "الصينية (المبسطة، الصين)"
// Parentheses in a qualifier become brackets, fullwidth ones fullwidth
// brackets.
print(of('ja', 'language', 'de-1996'), of('zh', 'language', 'en-CD'));
// CHECK-NEXT: "ドイツ語 (ドイツ語正書法[1996])" "英语（刚果［金］）"

// A dialect name followed by a variant qualifier.
print(of('en', 'language', 'en-US-posix'), of('en', 'language', 'en-US-posix', short), of('en', 'language', 'en-US-posix', standard));
// CHECK-NEXT: "American English (Computer)" "US English (Computer)" "English (United States, Computer)"
// Two and three qualifiers, joined with the locale's separator.
print(of('en', 'language', 'ca-ES-valencia'), of('en', 'language', 'sr-Latn-RS-ekavsk'));
// CHECK-NEXT: "Catalan (Spain, Valencian)" "Serbian (Latin, Serbia, Serbian with Ekavian pronunciation)"
print(of('zh-Hant', 'language', 'sr-Latn-RS-ekavsk'));
// CHECK-NEXT: "塞爾維亞文（拉丁字母，塞爾維亞，易卡發音塞爾維亞文）"
// A digit{3} region code, alone and in a language code.
print(of('en', 'region', '419'), of('de', 'region', '419'), of('en', 'region', '999', code));
// CHECK-NEXT: "Latin America" "Lateinamerika" "999"
print(of('en', 'language', 'es-419'), of('en', 'language', 'es-419', standard));
// CHECK-NEXT: "Latin American Spanish" "Spanish (Latin America)"

// The fallback is the canonical code of each type (V8 returns the code as
// given).
print(
  of('en', 'region', 'zy', code),
  of('en', 'script', 'qAAA', code),
  of('en', 'currency', 'abc', code),
  of('en', 'calendar', 'ABCD', code),
  of('en', 'language', 'EN-zy', code),
);
// CHECK-NEXT: "ZY" "Qaaa" "ABC" "abcd" "en-ZY"

// Language codes: a unicode_language_id that IsWellFormedLanguageTag
// accepts, canonicalized.
print(
  of('en', 'language', 'en-u-ca-gregory'),
  of('en', 'language', 'en-x-foo'),
  of('en', 'language', 'root'),
  of('en', 'language', 'abcd'),
  of('en', 'language', 'abcd-GB'),
  of('en', 'language', 'en_GB'),
  of('en', 'language', 'i-klingon'),
);
// CHECK-NEXT: RangeError RangeError RangeError RangeError RangeError RangeError RangeError
print(of('en', 'language', 'sh'), of('en', 'language', 'sh') === of('en', 'language', 'sr-Latn'));
// CHECK-NEXT: "Serbian (Latin)" true

// Currencies.
print(of('en', 'currency', 'usd'), of('en', 'currency', 'ABC', code), of('en', 'currency', 'US'));
// CHECK-NEXT: "US Dollar" "ABC" RangeError

// Calendars: lowercased, not resolved as aliases; only the supported
// calendars have names.
print(of('en', 'calendar', 'gregory'), of('en', 'calendar', 'GREGORY'), of('en', 'calendar', 'islamic'));
// CHECK-NEXT: "Gregorian Calendar" "Gregorian Calendar" undefined
print(of('en', 'calendar', 'gregorian'), of('en', 'calendar', 'islamic_civil'));
// CHECK-NEXT: RangeError RangeError

// Date-time fields, case-sensitive.
print(of('en', 'dateTimeField', 'year'), of('en', 'dateTimeField', 'year', short), of('en', 'dateTimeField', 'year', narrow));
// CHECK-NEXT: "year" "yr." "yr"
print(of('en', 'dateTimeField', 'weekday'));
// CHECK-NEXT: "day of the week"
print(of('de', 'dateTimeField', 'era'), of('de', 'dateTimeField', 'era', short), of('de', 'dateTimeField', 'era', narrow));
// CHECK-NEXT: "Epoche" "Ep." "E"
print(of('en', 'dateTimeField', 'Year'), of('en', 'dateTimeField', 'millisecond'));
// CHECK-NEXT: RangeError RangeError

// Review Focus 4: a tag with 100 distinct variants falls back to the code.
var variants = [];
for (var i = 0; i < 100; i++) variants.push('var' + String(1000 + i).substring(1));
var longTag = 'en-' + variants.join('-');
var dnCode = new Intl.DisplayNames('en', {type: 'language'});
print(dnCode.of(longTag) === longTag, thrown(() => dnCode.of(longTag)));
// CHECK-NEXT: true none
// The code goes through ToString.
var dnRegion = new Intl.DisplayNames('en', {type: 'region'});
print(thrown(() => dnRegion.of(Symbol())), dnRegion.of({toString() { return 'US'; }}));
// CHECK-NEXT: TypeError United States

// resolvedOptions: languageDisplay only for type "language", and no
// extension in the locale.
print(J(new Intl.DisplayNames('en', {type: 'language'}).resolvedOptions()));
// CHECK-NEXT: {"locale":"en","style":"long","type":"language","fallback":"code","languageDisplay":"dialect"}
print(J(new Intl.DisplayNames('en-u-ca-chinese', {type: 'region'}).resolvedOptions()));
// CHECK-NEXT: {"locale":"en","style":"long","type":"region","fallback":"code"}

// The options are required, and type is required.
print(
  thrown(() => new Intl.DisplayNames('en')),
  thrown(() => new Intl.DisplayNames('en', {})),
  thrown(() => new Intl.DisplayNames('en', null)),
);
// CHECK-NEXT: TypeError TypeError TypeError

/// An options object whose getters record the names they are read by in
/// log; type is type.
function loggingOptions(log, type) {
  var options = {};
  var names = ['localeMatcher', 'style', 'type', 'fallback', 'languageDisplay'];
  names.forEach(function (name) {
    Object.defineProperty(options, name, {
      get() {
        log.push(name);
        return name === 'type' ? type : undefined;
      },
    });
  });
  return options;
}
var log = [];
new Intl.DisplayNames('en', loggingOptions(log, 'language'));
print(log.join());
// CHECK-NEXT: localeMatcher,style,type,fallback,languageDisplay
log = [];
new Intl.DisplayNames('en', loggingOptions(log, 'region'));
print(log.join());
// CHECK-NEXT: localeMatcher,style,type,fallback,languageDisplay

// The object is made from new.target before the options are read: a
// throwing prototype getter wins.
function MyError() {}
var newTarget = function () {}.bind(null);
Object.defineProperty(newTarget, 'prototype', {
  get() {
    throw new MyError();
  },
});
log = [];
print(
  thrown(() => Reflect.construct(Intl.DisplayNames, ['en', loggingOptions(log, 'region')], newTarget)),
  thrown(() => Reflect.construct(Intl.DisplayNames, ['en'], newTarget)),
  log.length,
);
// CHECK-NEXT: MyError MyError 0
