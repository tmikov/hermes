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
var OriginalNumberFormat = Intl.NumberFormat;
var NumberFormatPrototype = Intl.NumberFormat.prototype;
var nfResolvedOptions = NumberFormatPrototype.resolvedOptions;
var nfFormatToParts = NumberFormatPrototype.formatToParts;
var nfFormatRange = NumberFormatPrototype.formatRange;
var nfFormatRangeToParts = NumberFormatPrototype.formatRangeToParts;
var nfSupportedLocalesOf = Intl.NumberFormat.supportedLocalesOf;
var getFormat = Object.getOwnPropertyDescriptor(NumberFormatPrototype, 'format').get;
var OriginalPluralRules = Intl.PluralRules;
var PluralRulesPrototype = Intl.PluralRules.prototype;
var prResolvedOptions = PluralRulesPrototype.resolvedOptions;
var prSelect = PluralRulesPrototype.select;
var prSelectRange = PluralRulesPrototype.selectRange;
var prSupportedLocalesOf = Intl.PluralRules.supportedLocalesOf;
var symbolToPrimitive = Symbol.toPrimitive;
var stringCharCodeAt = String.prototype.charCodeAt;
var localeCompare = String.prototype.localeCompare;
var toLocaleUpperCase = String.prototype.toLocaleUpperCase;
var numberToLocaleString = Number.prototype.toLocaleString;
var bigIntToLocaleString = BigInt.prototype.toLocaleString;
var arrayToLocaleString = Array.prototype.toLocaleString;
var TypedArrayPrototype = getPrototypeOf(Int8Array.prototype);
var typedArrayToLocaleString = TypedArrayPrototype.toLocaleString;
// Built from arrays now: the constructors use the replaced iterator.
var float64s = new Float64Array([1234.5, 2]);
var bigInt64s = new BigInt64Array([5n]);
var numberBox = new Number(0.5);
var bigIntBox = Object(7n);
var boom = function () {
  throw new Error('tampered intrinsic was used');
};
/// s with every code unit outside printable ASCII as \uXXXX, through the
/// captured charCodeAt.
function esc(s) {
  var out = '';
  for (var i = 0; i < s.length; i++) {
    var c = apply(stringCharCodeAt, s, [i]);
    out += c < 0x20 || c > 0x7e ? '\\u' + (c + 0x10000).toString(16).substring(1) : s[i];
  }
  return out;
}
/// The parts of the formatToParts result parts, as type:value, separated
/// by "|". Reading a part's own properties consults no accessor.
function partsString(parts) {
  var out = '';
  for (var i = 0; i < parts.length; i++)
    out += (i === 0 ? '' : '|') + parts[i].type + ':' + parts[i].value;
  return out;
}
/// The parts of the formatRangeToParts result parts, as type:value:source,
/// separated by "|".
function rangePartsString(parts) {
  var out = '';
  for (var i = 0; i < parts.length; i++)
    out += (i === 0 ? '' : '|') + parts[i].type + ':' + parts[i].value + ':' + parts[i].source;
  return out;
}
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
Math.max = boom;
Number.prototype.valueOf = boom;
Number.prototype[symbolToPrimitive] = boom;
BigInt.prototype.valueOf = boom;
// The TypedArray override validates its this value with the captured
// %TypedArray%.prototype.at and reads its length with the captured getter.
TypedArrayPrototype.at = boom;
define(TypedArrayPrototype, 'length', {__proto__: null, get: boom, configurable: true});
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
NumberFormatPrototype.resolvedOptions = boom;
NumberFormatPrototype.formatToParts = boom;
NumberFormatPrototype.formatRange = boom;
NumberFormatPrototype.formatRangeToParts = boom;
define(NumberFormatPrototype, 'format', {__proto__: null, get: boom, configurable: true});
Intl.NumberFormat.supportedLocalesOf = boom;
// NumberFormat's plural forms come from internal plural rules, not from
// Intl.PluralRules, so the NumberFormat tests below run with these
// replaced too.
PluralRulesPrototype.resolvedOptions = boom;
PluralRulesPrototype.select = boom;
PluralRulesPrototype.selectRange = boom;
Intl.PluralRules.supportedLocalesOf = boom;
// The names of the fields of the constructor's internal records, of
// RegionPreference's, and of the objects getWeekInfo and getTextInfo
// return. "region" is left out: the constructor reads it from its options
// object, which inherits from Object.prototype, as the spec requires. The
// Collator's records add the resolution option descriptors' fields, the
// extension keys, the locale data fields, and the fields of what
// ResolveOptions and ResolveLocale return. The Collator's own options
// ("usage", "sensitivity" and so on) are left out, since the Locale tests
// pass options objects without them; the Collator tests below pass
// undefined or an object that has every option. The NumberFormat tests do
// the same; its options, and the fields of the Intl mathematical values,
// of the rounding and pattern records, and of its data, are here too. The
// PluralRules tests pass every option too ("type" is already here as a
// Locale record field), and the field of ResolvePlural's record is here.
// "numberingSystem" is both a Locale and a NumberFormat option, so the
// Locale tests pass it, as undefined.
var recordFields = [
  'locale', 'calendar', 'ca', 'key', 'base', 'extension', 'regionOverride',
  'firstDay', 'weekend', 'direction', 'localeMatcher', 'co', 'kn', 'kf',
  'sensitivity', 'ignorePunctuation', 'property', 'type', 'values',
  'options', 'resolvedLocale', 'resolutionOptions', 'localeData',
  'attributes', 'keywords', 'requestedLocales', 'opt',
  'numberingSystem', 'style', 'currency', 'currencyDisplay', 'currencySign',
  'unit', 'unitDisplay', 'notation', 'minimumIntegerDigits',
  'minimumFractionDigits', 'maximumFractionDigits',
  'minimumSignificantDigits', 'maximumSignificantDigits',
  'roundingIncrement', 'roundingMode', 'roundingPriority',
  'trailingZeroDisplay', 'compactDisplay', 'useGrouping', 'signDisplay',
  'n', 'e', 'neg', 'source', 'pattern', 'nu', 'roundingType',
  'computedRoundingPriority', 'roundedNumber', 'formattedString',
  'integerDigitsCount', 'roundingMagnitude', 'parts', 'exponent',
  'symbols', 'digits', 'primaryGroup', 'secondaryGroup', 'minimumGrouping',
  'languageId', 'percentUnit', 'plural', 'pluralPartType', 'pluralRulesKey',
  'compact', 'compactExponent', 'exponentMinus', 'magnitude', 'exact0',
  'exact1', 'patterns', 'before', 'after', 'noSign',
  'first', 'last', 'outerBefore', 'outerAfter',
  'entries', 'count', 'limit', 'availableLocales', 'relevantExtensionKeys',
  'resolutionOptionDescriptors', 'pluralCategory',
];
for (var k = 0; k < recordFields.length; k++)
  define(ObjectPrototype, recordFields[k], {__proto__: null, get: boom, set: boom, configurable: true});
// The globals 01-intrinsics.js captures from, except String, whose
// functions are replaced above.
var globals = ['Object', 'RangeError', 'TypeError', 'Reflect', 'Math', 'Number', 'Symbol', 'Array', 'BigInt'];
for (var i = 0; i < globals.length; i++) globalThis[globals[i]] = boom;
Intl.Locale = boom;
Intl.Collator = boom;
Intl.NumberFormat = boom;
Intl.PluralRules = boom;
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
var locale = new OriginalLocale('en-latn-us', {
  region: 'GB',
  calendar: 'gregory',
  numberingSystem: undefined,
});
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
  numberingSystem: undefined,
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

// Intl.NumberFormat, through the references captured before the
// tampering: the constructor with and without new, resolvedOptions, the
// format getter and its bound function, formatToParts and
// supportedLocalesOf. Numbers, BigInts and strings are primitives, so
// formatting them calls neither the replaced valueOf methods nor the
// replaced Number.prototype[@@toPrimitive].
var nf = new OriginalNumberFormat('de-u-nu-latn');
var format = apply(getFormat, nf, []);
print(format(1234.5), format(-1234567n), format('1e3'), format(-0), format(NaN));
// CHECK-NEXT: 1.234,5 -1.234.567 1.000 -0 NaN
print(format(2n ** 70n), format === apply(getFormat, nf, []));
// CHECK-NEXT: 1.180.591.620.717.411.303.424 true
print(partsString(apply(nfFormatToParts, nf, [-1234.5])));
// CHECK-NEXT: minusSign:-|integer:1|group:.|integer:234|decimal:,|fraction:5
var nfCalled = OriginalNumberFormat('de');
print(getPrototypeOf(nfCalled) === NumberFormatPrototype, apply(getFormat, nfCalled, [])(0.5));
// CHECK-NEXT: true 0,5
// An options object with every option, so that no option is read from the
// tampered Object.prototype; the significant digits are own properties
// that are undefined.
var nfOptions = {
  localeMatcher: 'lookup',
  numberingSystem: 'arab',
  style: 'percent',
  currency: 'eur',
  currencyDisplay: 'code',
  currencySign: 'accounting',
  unit: 'meter',
  unitDisplay: 'long',
  notation: 'standard',
  minimumIntegerDigits: 2,
  minimumFractionDigits: 1,
  maximumFractionDigits: 2,
  minimumSignificantDigits: undefined,
  maximumSignificantDigits: undefined,
  roundingIncrement: 1,
  roundingMode: 'halfEven',
  roundingPriority: 'auto',
  trailingZeroDisplay: 'auto',
  compactDisplay: 'long',
  useGrouping: 'always',
  signDisplay: 'exceptZero',
};
var nfFull = new OriginalNumberFormat(['xx', 'ar-EG'], nfOptions);
print(stringify(apply(nfResolvedOptions, nfFull, [])));
// CHECK-NEXT: {"locale":"ar-EG","numberingSystem":"arab","style":"percent","minimumIntegerDigits":2,"minimumFractionDigits":1,"maximumFractionDigits":2,"useGrouping":"always","notation":"standard","signDisplay":"exceptZero","roundingIncrement":1,"roundingMode":"halfEven","roundingPriority":"auto","trailingZeroDisplay":"auto"}
var fullFormat = apply(getFormat, nfFull, []);
print(esc(fullFormat(12.3456)), esc(fullFormat(-0.125)), esc(fullFormat(0)));
// CHECK-NEXT: \u061c+\u0661\u066c\u0662\u0663\u0664\u066b\u0665\u0666\u066a\u061c \u061c-\u0661\u0662\u066b\u0665\u066a\u061c \u0660\u0660\u066b\u0660\u066a\u061c
print(esc(partsString(apply(nfFormatToParts, nfFull, [-0.125]))));
// CHECK-NEXT: literal:\u061c|minusSign:-|integer:\u0661\u0662|decimal:\u066b|fraction:\u0665|percentSign:\u066a|literal:\u061c
print(stringify(apply(nfSupportedLocalesOf, null, [['de', 'xx', 'th-TH']])));
// CHECK-NEXT: ["de","th-TH"]
print(thrown(() => new OriginalNumberFormat('de', {localeMatcher: 'lookup', numberingSystem: 'latn', style: 'x'}), OriginalRangeError));
// CHECK-NEXT: ok
print(thrown(() => apply(nfResolvedOptions, {}, []), OriginalTypeError));
// CHECK-NEXT: ok
print(thrown(() => apply(getFormat, {}, []), OriginalTypeError));
// CHECK-NEXT: ok
print(thrown(() => apply(nfFormatToParts, nf, [aSymbol]), OriginalTypeError));
// CHECK-NEXT: ok
// ToIntlMathematicalValue's ToNumber of a symbol throws the intrinsic
// TypeError, for a symbol and for an object whose @@toPrimitive (read
// through the captured symbol) returns one.
print(thrown(() => format(aSymbol), OriginalTypeError));
// CHECK-NEXT: ok
var toPrimitiveCalls = 0;
var symbolBox = {};
symbolBox[symbolToPrimitive] = function (hint) {
  toPrimitiveCalls++;
  return aSymbol;
};
print(thrown(() => format(symbolBox), OriginalTypeError), toPrimitiveCalls);
// CHECK-NEXT: ok 1

/// An options object with every option NumberFormat reads as an own
/// property, undefined except for the fraction digits and useGrouping.
function decimalOptions(minimumFractionDigits, maximumFractionDigits, useGrouping) {
  return {
    localeMatcher: undefined,
    numberingSystem: undefined,
    style: undefined,
    currency: undefined,
    currencyDisplay: undefined,
    currencySign: undefined,
    unit: undefined,
    unitDisplay: undefined,
    notation: undefined,
    minimumIntegerDigits: undefined,
    minimumFractionDigits: minimumFractionDigits,
    maximumFractionDigits: maximumFractionDigits,
    minimumSignificantDigits: undefined,
    maximumSignificantDigits: undefined,
    roundingIncrement: undefined,
    roundingMode: undefined,
    roundingPriority: undefined,
    trailingZeroDisplay: undefined,
    compactDisplay: undefined,
    useGrouping: useGrouping,
    signDisplay: undefined,
  };
}
// useGrouping "min2" uses the captured Math.max; so does
// SetNumberFormatDigitOptions when only minimumFractionDigits is given, and
// the captured Math.min when only maximumFractionDigits is.
var min2 = apply(getFormat, new OriginalNumberFormat('en', decimalOptions(undefined, undefined, 'min2')), []);
print(min2(1234), min2(12345));
// CHECK-NEXT: 1234 12,345
var minFD = new OriginalNumberFormat('en', decimalOptions(1, undefined, undefined));
print(apply(getFormat, minFD, [])(1), apply(getFormat, minFD, [])(1.23456), apply(nfResolvedOptions, minFD, []).maximumFractionDigits);
// CHECK-NEXT: 1.0 1.235 3
var maxFD = new OriginalNumberFormat('en', decimalOptions(undefined, 1, undefined));
print(apply(getFormat, maxFD, [])(1), apply(getFormat, maxFD, [])(1.25), apply(nfResolvedOptions, maxFD, []).minimumFractionDigits);
// CHECK-NEXT: 1 1.3 0

/// An options object with every option NumberFormat reads as an own
/// property, for the currency or unit style: undefined except for the
/// options given.
function styleOptions(style, currency, currencyDisplay, currencySign, unit, unitDisplay) {
  return {
    localeMatcher: undefined,
    numberingSystem: undefined,
    style: style,
    currency: currency,
    currencyDisplay: currencyDisplay,
    currencySign: currencySign,
    unit: unit,
    unitDisplay: unitDisplay,
    notation: undefined,
    minimumIntegerDigits: undefined,
    minimumFractionDigits: undefined,
    maximumFractionDigits: undefined,
    minimumSignificantDigits: undefined,
    maximumSignificantDigits: undefined,
    roundingIncrement: undefined,
    roundingMode: undefined,
    roundingPriority: undefined,
    trailingZeroDisplay: undefined,
    compactDisplay: undefined,
    useGrouping: undefined,
    signDisplay: undefined,
  };
}
// The currency and unit styles: the currency patterns, the plural-dependent
// name and unit patterns, their splitting, and the internal plural rules.
var usdName = new OriginalNumberFormat('en', styleOptions('currency', 'usd', 'name', undefined, undefined, undefined));
print(apply(getFormat, usdName, [])(1), partsString(apply(nfFormatToParts, usdName, [-1])));
// CHECK-NEXT: 1.00 US dollars minusSign:-|integer:1|decimal:.|fraction:00|literal: |currency:US dollars
var eurCode = new OriginalNumberFormat('de', styleOptions('currency', 'EUR', 'code', 'accounting', undefined, undefined));
print(esc(apply(getFormat, eurCode, [])(-1234.5)), esc(apply(getFormat, eurCode, [])(NaN)));
// CHECK-NEXT: -1.234,50\u00a0EUR NaN\u00a0EUR
var jaUnit = new OriginalNumberFormat('ja', styleOptions('unit', undefined, undefined, undefined, 'kilometer-per-hour', 'long'));
print(partsString(apply(nfFormatToParts, jaUnit, [-987])));
// CHECK-NEXT: unit:時速|literal: |minusSign:-|integer:987|literal: |unit:キロメートル
var milesLong = apply(getFormat, new OriginalNumberFormat('en', styleOptions('unit', undefined, undefined, undefined, 'mile', 'long')), []);
print(milesLong(1), milesLong(1.5));
// CHECK-NEXT: 1 mile 1.5 miles

/// An options object with every option NumberFormat reads as an own
/// property, for a notation: undefined except for the options given.
function notationOptions(style, currency, notation, compactDisplay) {
  return {
    localeMatcher: undefined,
    numberingSystem: undefined,
    style: style,
    currency: currency,
    currencyDisplay: undefined,
    currencySign: undefined,
    unit: undefined,
    unitDisplay: undefined,
    notation: notation,
    minimumIntegerDigits: undefined,
    minimumFractionDigits: undefined,
    maximumFractionDigits: undefined,
    minimumSignificantDigits: undefined,
    maximumSignificantDigits: undefined,
    roundingIncrement: undefined,
    roundingMode: undefined,
    roundingPriority: undefined,
    trailingZeroDisplay: undefined,
    compactDisplay: compactDisplay,
    useGrouping: undefined,
    signDisplay: undefined,
  };
}
// The notations: the exponent parts and their bidi marks, the compact
// tables and their entries, the exact-1 pattern without a number, and
// compact currency.
var sciEn = new OriginalNumberFormat('en', notationOptions(undefined, undefined, 'scientific', undefined));
print(partsString(apply(nfFormatToParts, sciEn, [-0.000123])));
// CHECK-NEXT: minusSign:-|integer:1|decimal:.|fraction:23|exponentSeparator:E|exponentMinusSign:-|exponentInteger:4
var sciAr = new OriginalNumberFormat('ar-EG', notationOptions(undefined, undefined, 'engineering', undefined));
print(esc(partsString(apply(nfFormatToParts, sciAr, [0.000123]))));
// CHECK-NEXT: integer:\u0661\u0662\u0663|exponentSeparator:\u0623\u0633|literal:\u061c|exponentMinusSign:-|exponentInteger:\u0666
var frLong = new OriginalNumberFormat('fr', notationOptions(undefined, undefined, 'compact', 'long'));
print(apply(getFormat, frLong, [])(1000), '/', apply(getFormat, frLong, [])(-1000), '/', partsString(apply(nfFormatToParts, frLong, [2000])));
// CHECK-NEXT: mille / -1 millier / integer:2|literal: |compact:mille
var deEur = new OriginalNumberFormat('de', notationOptions('currency', 'EUR', 'compact', undefined));
print(esc(apply(getFormat, deEur, [])(1234567)), esc(apply(getFormat, deEur, [])(1234)));
// CHECK-NEXT: 1,2\u00a0Mio.\u00a0\u20ac 1234\u00a0\u20ac

// The ranges: the collapsed and full forms, the approximately sign before
// a sign, where the plus sign would be and before an accounting negative,
// the bidi marks of a sign, and the errors.
var rangeUsd = new OriginalNumberFormat('en', notationOptions('currency', 'USD', undefined, undefined));
print(apply(nfFormatRange, rangeUsd, [3, 5]), '/', apply(nfFormatRange, rangeUsd, [-3, 5]), '/', apply(nfFormatRange, rangeUsd, [2.999, 3.001]));
// CHECK-NEXT: $3.00 – $5.00 / -$3.00 – $5.00 / ~$3.00
print(rangePartsString(apply(nfFormatRangeToParts, rangeUsd, [-3, -3])));
// CHECK-NEXT: approximatelySign:~:shared|minusSign:-:shared|currency:$:shared|integer:3:shared|decimal:.:shared|fraction:00:shared
var rangeAccounting = new OriginalNumberFormat('en', styleOptions('currency', 'USD', undefined, 'accounting', undefined, undefined));
print(apply(nfFormatRange, rangeAccounting, [-3, -5]), '/', apply(nfFormatRange, rangeAccounting, [-3, -3]), '/', apply(nfFormatRange, rangeAccounting, [3, 3]));
// CHECK-NEXT: ($3.00–5.00) / ~($3.00) / ~$3.00
print(rangePartsString(apply(nfFormatRangeToParts, jaUnit, [1, 5])));
// CHECK-NEXT: unit:時速:shared|literal: :shared|integer:1:startRange|literal:～:shared|integer:5:endRange|literal: :shared|unit:キロメートル:shared
print(rangePartsString(apply(nfFormatRangeToParts, jaUnit, [-3, -5])));
// CHECK-NEXT: unit:時速:shared|literal: :shared|minusSign:-:startRange|integer:3:startRange|literal: ～ :shared|minusSign:-:endRange|integer:5:endRange|literal: :shared|unit:キロメートル:shared
print(apply(nfFormatRange, new OriginalNumberFormat('en', notationOptions(undefined, undefined, 'compact', 'long')), [3000, 5000000]));
// CHECK-NEXT: 3 thousand – 5 million
print(esc(apply(nfFormatRange, sciAr, [0.001, 0.005])), esc(apply(nfFormatRange, nfFull, [-0.03, -0.03])));
// CHECK-NEXT: \u0661\u0623\u0633\u061c-\u0663 \u2013 \u0665\u0623\u0633\u061c-\u0663 ~\u061c-\u0660\u0663\u066b\u0660\u066a\u061c
print(apply(nfFormatRange, nf, [2n ** 70n, -1]), apply(nfFormatRange, nf, [-0, '1e3']));
// CHECK-NEXT: 1.180.591.620.717.411.303.424–-1 -0 – 1.000
print(thrown(() => apply(nfFormatRange, nf, [NaN, 1]), OriginalRangeError), thrown(() => apply(nfFormatRangeToParts, nf, [1, undefined]), OriginalTypeError), thrown(() => apply(nfFormatRange, {}, [1, 2]), OriginalTypeError));
// CHECK-NEXT: ok ok ok

// Intl.PluralRules, through the references captured before the
// tampering: the constructor, resolvedOptions with its pluralCategories,
// select in every notation (the rounding, the exponent of the notation and
// the compact table), selectRange and supportedLocalesOf.
/// An options object with every option PluralRules reads as an own
/// property: undefined except for the options given.
function pluralOptions(type, notation, compactDisplay, minimumFractionDigits) {
  return {
    localeMatcher: undefined,
    type: type,
    notation: notation,
    compactDisplay: compactDisplay,
    minimumIntegerDigits: undefined,
    minimumFractionDigits: minimumFractionDigits,
    maximumFractionDigits: undefined,
    minimumSignificantDigits: undefined,
    maximumSignificantDigits: undefined,
    roundingIncrement: undefined,
    roundingMode: undefined,
    roundingPriority: undefined,
    trailingZeroDisplay: undefined,
  };
}
var prEn = new OriginalPluralRules('en', pluralOptions(undefined, undefined, undefined, 1));
print(stringify(apply(prResolvedOptions, prEn, [])));
// CHECK-NEXT: {"locale":"en","type":"cardinal","notation":"standard","minimumIntegerDigits":1,"minimumFractionDigits":1,"maximumFractionDigits":3,"pluralCategories":["one","other"],"roundingIncrement":1,"roundingMode":"halfExpand","roundingPriority":"auto","trailingZeroDisplay":"auto"}
print(apply(prSelect, prEn, [1]), apply(prSelect, prEn, [1n]), apply(prSelect, prEn, ['1']), apply(prSelect, prEn, [NaN]));
// CHECK-NEXT: other other other other
var prArOrdinal = new OriginalPluralRules('ar', pluralOptions('ordinal', undefined, undefined, undefined));
print(stringify(apply(prResolvedOptions, new OriginalPluralRules('ar', pluralOptions()), []).pluralCategories));
// CHECK-NEXT: ["zero","one","two","few","many","other"]
print(apply(prSelect, prArOrdinal, [2]), stringify(apply(prResolvedOptions, prArOrdinal, []).pluralCategories));
// CHECK-NEXT: other ["other"]
var prOrdinalScientific = new OriginalPluralRules('en', pluralOptions('ordinal', 'scientific', undefined, undefined));
print(apply(prSelect, prOrdinalScientific, [21]), apply(prSelect, prOrdinalScientific, [22]));
// CHECK-NEXT: one two
var prFrEngineering = new OriginalPluralRules('fr', pluralOptions(undefined, 'engineering', undefined, undefined));
var prFrCompact = new OriginalPluralRules('fr', pluralOptions(undefined, 'compact', 'long', undefined));
print(apply(prSelect, prFrEngineering, [1.5e6]), apply(prSelect, prFrCompact, [1234567]), apply(prSelect, prFrCompact, [1.5]));
// CHECK-NEXT: many many one
print(stringify(apply(prResolvedOptions, prFrCompact, [])));
// CHECK-NEXT: {"locale":"fr","type":"cardinal","notation":"compact","compactDisplay":"long","minimumIntegerDigits":1,"minimumFractionDigits":0,"maximumFractionDigits":0,"minimumSignificantDigits":1,"maximumSignificantDigits":2,"pluralCategories":["one","many","other"],"roundingIncrement":1,"roundingMode":"halfExpand","roundingPriority":"morePrecision","trailingZeroDisplay":"auto"}
var prFr = new OriginalPluralRules('fr', pluralOptions());
print(apply(prSelectRange, prFr, [1, 1.5]), apply(prSelectRange, prFr, [0, 2]), apply(prSelectRange, prEn, [1, 1]));
// CHECK-NEXT: one other other
print(stringify(apply(prSupportedLocalesOf, null, [['de', 'xx', 'th-TH']])));
// CHECK-NEXT: ["de","th-TH"]
print(thrown(() => OriginalPluralRules('en'), OriginalTypeError), thrown(() => new OriginalPluralRules('en', {localeMatcher: 'lookup', type: 'x'}), OriginalRangeError));
// CHECK-NEXT: ok ok
print(thrown(() => apply(prSelectRange, prFr, [NaN, 1]), OriginalRangeError), thrown(() => apply(prSelectRange, prFr, [1]), OriginalTypeError), thrown(() => apply(prSelect, {}, [1]), OriginalTypeError), thrown(() => apply(prSelect, prFr, [aSymbol]), OriginalTypeError));
// CHECK-NEXT: ok ok ok ok

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

// The toLocaleString overrides, called as methods: they use neither the
// replaced Intl.NumberFormat nor its replaced prototype methods and format
// getter, neither the replaced valueOf methods nor the replaced
// Number.prototype[@@toPrimitive], and neither the replaced
// %TypedArray%.prototype.at nor its replaced length getter. The first call
// without locales and options creates the default NumberFormat now.
print((1234.5).toLocaleString(), (1234.5).toLocaleString('de'), (-12345n).toLocaleString(), (12345n).toLocaleString(['de']));
// CHECK-NEXT: 1,234.5 1.234,5 -12,345 12.345
// ThisNumberValue and ThisBigIntValue unwrap the objects through the
// captured valueOf methods.
print(numberBox.toLocaleString(), bigIntBox.toLocaleString('de'));
// CHECK-NEXT: 0.5 7
// The options object from the NumberFormat test above, with every option.
print(esc((-0.125).toLocaleString(['xx', 'ar-EG'], nfOptions)));
// CHECK-NEXT: \u061c-\u0661\u0662\u066b\u0665\u066a\u061c
// The list overrides pass locales and options to each element's
// toLocaleString, which here is the Number or BigInt override; the
// separator, and the empty string for null.
print([1234.5, 7n, null].toLocaleString('de', decimalOptions(2, 2, undefined)));
// CHECK-NEXT: 1.234,50,7,00,
print(float64s.toLocaleString('de'), bigInt64s.toLocaleString('de', decimalOptions(1, 1, undefined)));
// CHECK-NEXT: 1.234,5,2 5,0
print(
  thrown(() => apply(numberToLocaleString, '1', []), OriginalTypeError),
  thrown(() => apply(bigIntToLocaleString, 1, []), OriginalTypeError),
  thrown(() => apply(arrayToLocaleString, null, []), OriginalTypeError),
  thrown(() => apply(typedArrayToLocaleString, [1], []), OriginalTypeError),
);
// CHECK-NEXT: ok ok ok ok
print(
  thrown(() => (1).toLocaleString('de', {localeMatcher: 'lookup', numberingSystem: 'latn', style: 'x'}), OriginalRangeError),
  thrown(() => (1n).toLocaleString('xx-invalid-'), OriginalRangeError),
  thrown(() => [1].toLocaleString('xx-invalid-'), OriginalRangeError),
  thrown(() => float64s.toLocaleString('xx-invalid-'), OriginalRangeError),
);
// CHECK-NEXT: ok ok ok ok
