/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US %s | %FileCheck --match-full-lines %s

// User code that replaces intrinsics before its first call to Intl does not
// change what Intl returns or throws. Nothing calls Intl before the
// tampering, so a value the plugin caches on a first call cannot hide a use
// of a replaced intrinsic; the results are compared with literal values.

// Everything this test uses after the tampering is captured first.
var define = Object.defineProperty;
var ObjectPrototype = Object.prototype;
var getPrototypeOf = Object.getPrototypeOf;
var OriginalRangeError = RangeError;
var OriginalTypeError = TypeError;
var stringify = JSON.stringify;
var aSymbol = Symbol('x');
// The test's own calls of a replaced Locale method would reach the
// replacement, so it calls the original methods through Reflect.apply,
// captured here, like everything else.
var apply = Reflect.apply;
var OriginalLocale = Intl.Locale;
var LocalePrototype = Intl.Locale.prototype;
var localeToString = LocalePrototype.toString;
var localeMaximize = LocalePrototype.maximize;
var localeMinimize = LocalePrototype.minimize;
var localeGetCalendars = LocalePrototype.getCalendars;
var localeGetWeekInfo = LocalePrototype.getWeekInfo;
var localeGetTextInfo = LocalePrototype.getTextInfo;
/// The original getter of Intl.Locale.prototype[name].
function getter(name) {
  return Object.getOwnPropertyDescriptor(LocalePrototype, name).get;
}
var getBaseName = getter('baseName');
var getVariants = getter('variants');
var getCalendar = getter('calendar');
var getFirstDayOfWeek = getter('firstDayOfWeek');
var getNumeric = getter('numeric');
var OriginalCollator = Intl.Collator;
var CollatorPrototype = Intl.Collator.prototype;
var collatorResolvedOptions = CollatorPrototype.resolvedOptions;
var collatorSupportedLocalesOf = Intl.Collator.supportedLocalesOf;
var getCompare = Object.getOwnPropertyDescriptor(CollatorPrototype, 'compare').get;
var localeCompare = String.prototype.localeCompare;
var toLocaleUpperCase = String.prototype.toLocaleUpperCase;
var boom = function () {
  throw new Error('tampered intrinsic was used');
};
/// "ok" if f throws an instance of the original constructor expected (not
/// whatever the global now holds), or what went wrong.
function thrown(f, expected) {
  try {
    f();
    return 'none';
  } catch (e) {
    return e.constructor === expected ? 'ok' : 'wrong error: ' + e;
  }
}

Array.prototype[Symbol.iterator] = boom;
Array.prototype.push = boom;
Array.prototype.indexOf = boom;
Array.prototype.slice = boom;
Array.prototype.sort = boom;
String.prototype[Symbol.split] = boom;
// A string separator does not consult @@split (ECMA-262 22.1.3.23 step 3),
// so split itself is replaced too.
String.prototype.split = boom;
String.prototype.slice = boom;
String.prototype.indexOf = boom;
String.prototype.charCodeAt = boom;
String.prototype.toLowerCase = boom;
String.prototype.toUpperCase = boom;
String.prototype.codePointAt = boom;
String.prototype.normalize = boom;
String.fromCodePoint = boom;
String.fromCharCode = boom;
Function.prototype.call = boom;
Function.prototype.apply = boom;
Reflect.apply = boom;
Reflect.ownKeys = boom;
Object.create = boom;
Object.defineProperty = boom;
Object.getOwnPropertyDescriptor = boom;
Math.min = boom;
Math.trunc = boom;
Number.isFinite = boom;
define(Array.prototype, '0', {__proto__: null, set: boom, get: boom, configurable: true});
define(Array.prototype, '1', {__proto__: null, set: boom, get: boom, configurable: true});
LocalePrototype.toString = boom;
LocalePrototype.maximize = boom;
LocalePrototype.getCalendars = boom;
LocalePrototype.getWeekInfo = boom;
LocalePrototype.getTextInfo = boom;
CollatorPrototype.resolvedOptions = boom;
define(CollatorPrototype, 'compare', {__proto__: null, get: boom, configurable: true});
Intl.Collator.supportedLocalesOf = boom;
// The names of the fields of the constructor's internal records, of
// RegionPreference's, and of the objects getWeekInfo and getTextInfo
// return. "region" is left out: the constructor reads it from its options
// object, which inherits from Object.prototype, as the spec requires. The
// Collator's records add the resolution option descriptors' fields, the
// extension keys, the locale data fields, and the fields of what
// ResolveOptions and ResolveLocale return. The Collator's own options
// ("usage", "sensitivity" and so on) are left out, since the Locale tests
// pass options objects without them; the Collator tests below pass
// undefined or an object that has every option.
var recordFields = [
  'locale', 'calendar', 'ca', 'key', 'base', 'extension', 'regionOverride',
  'firstDay', 'weekend', 'direction', 'localeMatcher', 'co', 'kn', 'kf',
  'sensitivity', 'ignorePunctuation', 'property', 'type', 'values',
  'options', 'resolvedLocale', 'resolutionOptions', 'localeData',
  'attributes', 'keywords',
];
for (var k = 0; k < recordFields.length; k++)
  define(ObjectPrototype, recordFields[k], {__proto__: null, get: boom, set: boom, configurable: true});
// The globals 01-intrinsics.js captures from, except String, whose
// functions are replaced above.
var globals = ['Object', 'RangeError', 'TypeError', 'Reflect', 'Math', 'Number', 'Symbol', 'Array'];
for (var i = 0; i < globals.length; i++) globalThis[globals[i]] = boom;
Intl.Locale = boom;
Intl.Collator = boom;
// Last, because it breaks ordinary descriptor objects: accessors for the
// descriptor fields on Object.prototype. "value" is also a field of the
// records of Unicode extension keywords.
var fields = ['value', 'writable', 'enumerable', 'configurable', 'get', 'set'];
for (var j = 0; j < fields.length; j++)
  define(ObjectPrototype, fields[j], {__proto__: null, get: boom, configurable: true});

print(stringify(Intl.getCanonicalLocales(['EN-us', 'de', 'EN-us'])));
// CHECK: ["en-US","de"]
// ToObject wraps a primitive with the captured Object.
print(stringify(Intl.getCanonicalLocales(5)));
// CHECK-NEXT: []
print(stringify(Intl.supportedValuesOf('calendar')));
// CHECK-NEXT: ["buddhist","chinese","coptic","dangi","ethioaa","ethiopic","gregory","hebrew","indian","islamic-civil","islamic-tbla","islamic-umalqura","iso8601","japanese","persian","roc"]
print(stringify(Intl.supportedValuesOf('collation')));
// CHECK-NEXT: ["compat","dict","emoji","eor","phonebk","pinyin","stroke","trad","unihan","zhuyin"]
var units = Intl.supportedValuesOf('unit');
print(units.length, units[0], units[1], units[44]);
// CHECK-NEXT: 45 acre bit year

// The errors come from the intrinsics, not from the replaced globals.
print(thrown(() => Intl.supportedValuesOf('x'), OriginalRangeError));
// CHECK-NEXT: ok
print(thrown(() => Intl.supportedValuesOf(aSymbol), OriginalTypeError));
// CHECK-NEXT: ok
print(thrown(() => Intl.getCanonicalLocales(['']), OriginalRangeError));
// CHECK-NEXT: ok
print(thrown(() => Intl.getCanonicalLocales([null]), OriginalTypeError));
// CHECK-NEXT: ok
print(thrown(() => Intl.getCanonicalLocales(null), OriginalTypeError));
// CHECK-NEXT: ok

// Intl.Locale, constructed through the reference captured before the
// tampering: the constructor, toString, maximize and minimize use neither
// the replaced global nor the replaced methods.
var locale = new OriginalLocale('en-latn-us', {region: 'GB', calendar: 'gregory'});
print(apply(localeToString, locale, []));
// CHECK-NEXT: en-Latn-GB-u-ca-gregory
print(apply(localeToString, apply(localeMaximize, locale, []), []));
// CHECK-NEXT: en-Latn-GB-u-ca-gregory
print(apply(localeToString, apply(localeMinimize, locale, []), []));
// CHECK-NEXT: en-GB-u-ca-gregory
// CanonicalizeLocaleList reads the tag from the slot, not through the
// replaced toString.
print(stringify(Intl.getCanonicalLocales([locale])));
// CHECK-NEXT: ["en-Latn-GB-u-ca-gregory"]
// The getters, the variants option (UpdateLanguageId), an option that
// overrides a keyword of the tag in place (MakeLocaleRecord),
// WeekdayToUValue, and numeric.
var full = new OriginalLocale('en-latn-us-u-ca-buddhist', {
  variants: 'Fonipa',
  calendar: 'gregory',
  firstDayOfWeek: '1',
  numeric: true,
});
print(apply(localeToString, full, []));
// CHECK-NEXT: en-Latn-US-fonipa-u-ca-gregory-fw-mon-kn
print(
  apply(getBaseName, full, []),
  apply(getVariants, full, []),
  apply(getCalendar, full, []),
  apply(getFirstDayOfWeek, full, []),
  apply(getNumeric, full, []),
);
// CHECK-NEXT: en-Latn-US-fonipa fonipa gregory mon true
print(thrown(() => new OriginalLocale('en', {region: '1234'}), OriginalRangeError));
// CHECK-NEXT: ok
print(thrown(() => apply(localeToString, {}, []), OriginalTypeError));
// CHECK-NEXT: ok

// The information methods, through the references captured before the
// tampering: RegionPreference, the native lists, AvailableCalendars and
// the result objects use none of the replaced intrinsics.
print(stringify(apply(localeGetCalendars, new OriginalLocale('th'), [])));
// CHECK-NEXT: ["buddhist","gregory"]
var week = apply(localeGetWeekInfo, new OriginalLocale('en-US'), []);
print(stringify(week.weekend), week.firstDay);
// CHECK-NEXT: [6,7] 7
var text = apply(localeGetTextInfo, new OriginalLocale('ar'), []);
print(text.direction, getPrototypeOf(text) === ObjectPrototype);
// CHECK-NEXT: rtl true

// Intl.Collator, through the references captured before the tampering: the
// constructor with and without new, resolvedOptions, the compare getter and
// its bound function, and supportedLocalesOf.
var collator = new OriginalCollator('de-u-co-phonebk-kn');
print(stringify(apply(collatorResolvedOptions, collator, [])));
// CHECK-NEXT: {"locale":"de-u-co-phonebk-kn","usage":"sort","sensitivity":"variant","ignorePunctuation":false,"collation":"phonebk","numeric":true,"caseFirst":"false"}
var compare = apply(getCompare, collator, []);
print(compare('ä', 'ad'), compare('2', '10'), compare('a', 'A'));
// CHECK-NEXT: 1 -1 -1
print(compare === apply(getCompare, collator, []));
// CHECK-NEXT: true
var called = OriginalCollator('da');
print(
  getPrototypeOf(called) === CollatorPrototype,
  stringify(apply(collatorResolvedOptions, called, [])),
);
// CHECK-NEXT: true {"locale":"da","usage":"sort","sensitivity":"variant","ignorePunctuation":false,"collation":"default","numeric":false,"caseFirst":"upper"}
// An options object with every option, so that no option is read from the
// tampered Object.prototype.
var options = {
  localeMatcher: 'lookup',
  usage: 'search',
  collation: 'eor',
  numeric: false,
  caseFirst: 'lower',
  sensitivity: 'base',
  ignorePunctuation: true,
};
var full = new OriginalCollator(['xx', 'de-u-co-phonebk'], options);
print(stringify(apply(collatorResolvedOptions, full, [])));
// CHECK-NEXT: {"locale":"de","usage":"search","sensitivity":"base","ignorePunctuation":true,"collation":"eor","numeric":false,"caseFirst":"lower"}
print(apply(getCompare, full, [])('a-A', 'aa'));
// CHECK-NEXT: 0
print(stringify(apply(collatorSupportedLocalesOf, null, [['de', 'xx', 'th-TH']])));
// CHECK-NEXT: ["de","th-TH"]
print(thrown(() => new OriginalCollator('de', {usage: 'x'}), OriginalRangeError));
// CHECK-NEXT: ok
// Every option read before the error is an own property.
print(
  thrown(
    () => new OriginalCollator('de', {localeMatcher: 'lookup', usage: 'sort', collation: 'a'}),
    OriginalRangeError,
  ),
);
// CHECK-NEXT: ok
print(thrown(() => apply(collatorResolvedOptions, {}, []), OriginalTypeError));
// CHECK-NEXT: ok
print(thrown(() => apply(getCompare, {}, []), OriginalTypeError));
// CHECK-NEXT: ok

// The String overrides, called as methods (Function.prototype.call is
// replaced): localeCompare uses neither the replaced Intl.Collator nor the
// replaced compare getter or resolvedOptions, with or without locales; the
// case mapping uses none of the replaced String methods.
print('a'.localeCompare('b'), 'a'.localeCompare('A'), 'b'.localeCompare('a'));
// CHECK-NEXT: -1 -1 1
// Again, now that the default collator exists.
print('a'.localeCompare('b'), 'a'.localeCompare('a', undefined, undefined));
// CHECK-NEXT: -1 0
print('ä'.localeCompare('z', 'sv'), 'ä'.localeCompare('z', ['de']));
// CHECK-NEXT: 1 -1
print('2'.localeCompare('10', 'de-u-kn'));
// CHECK-NEXT: -1
// The options object from the Collator test above, with every option.
print('a-A'.localeCompare('aa', ['xx', 'de-u-co-phonebk'], options));
// CHECK-NEXT: 0
print('i'.toLocaleUpperCase('tr'), 'I'.toLocaleLowerCase(['az']), 'ab'.toLocaleUpperCase());
// CHECK-NEXT: İ ı AB
print('ΆΒ'.toLocaleLowerCase('el'), 'άβ'.toLocaleUpperCase(['el-GR']));
// CHECK-NEXT: άβ ΑΒ
print(thrown(() => apply(localeCompare, null, ['a']), OriginalTypeError));
// CHECK-NEXT: ok
print(thrown(() => 'a'.localeCompare('b', 'xx-invalid-'), OriginalRangeError));
// CHECK-NEXT: ok
print(thrown(() => apply(toLocaleUpperCase, undefined, []), OriginalTypeError));
// CHECK-NEXT: ok
print(thrown(() => 'a'.toLocaleUpperCase(['']), OriginalRangeError));
// CHECK-NEXT: ok
