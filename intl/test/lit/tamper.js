/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US -Xintl-default-time-zone=UTC %s | %FileCheck --match-full-lines %s

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
var OriginalDateTimeFormat = Intl.DateTimeFormat;
var DateTimeFormatPrototype = Intl.DateTimeFormat.prototype;
var dtfResolvedOptions = DateTimeFormatPrototype.resolvedOptions;
var dtfFormatToParts = DateTimeFormatPrototype.formatToParts;
var dtfFormatRange = DateTimeFormatPrototype.formatRange;
var dtfFormatRangeToParts = DateTimeFormatPrototype.formatRangeToParts;
var dtfSupportedLocalesOf = Intl.DateTimeFormat.supportedLocalesOf;
var getDtfFormat = Object.getOwnPropertyDescriptor(DateTimeFormatPrototype, 'format').get;
var OriginalListFormat = Intl.ListFormat;
var ListFormatPrototype = Intl.ListFormat.prototype;
var lfResolvedOptions = ListFormatPrototype.resolvedOptions;
var lfFormat = ListFormatPrototype.format;
var lfFormatToParts = ListFormatPrototype.formatToParts;
var lfSupportedLocalesOf = Intl.ListFormat.supportedLocalesOf;
var OriginalRelativeTimeFormat = Intl.RelativeTimeFormat;
var RelativeTimeFormatPrototype = Intl.RelativeTimeFormat.prototype;
var rtfResolvedOptions = RelativeTimeFormatPrototype.resolvedOptions;
var rtfFormat = RelativeTimeFormatPrototype.format;
var rtfFormatToParts = RelativeTimeFormatPrototype.formatToParts;
var rtfSupportedLocalesOf = Intl.RelativeTimeFormat.supportedLocalesOf;
var OriginalDisplayNames = Intl.DisplayNames;
var DisplayNamesPrototype = Intl.DisplayNames.prototype;
var dnResolvedOptions = DisplayNamesPrototype.resolvedOptions;
var dnOf = DisplayNamesPrototype.of;
var dnSupportedLocalesOf = Intl.DisplayNames.supportedLocalesOf;
var OriginalDurationFormat = Intl.DurationFormat;
var DurationFormatPrototype = Intl.DurationFormat.prototype;
var dfResolvedOptions = DurationFormatPrototype.resolvedOptions;
var dfFormat = DurationFormatPrototype.format;
var dfFormatToParts = DurationFormatPrototype.formatToParts;
var dfSupportedLocalesOf = Intl.DurationFormat.supportedLocalesOf;
var OriginalSegmenter = Intl.Segmenter;
var SegmenterPrototype = Intl.Segmenter.prototype;
var sgResolvedOptions = SegmenterPrototype.resolvedOptions;
var sgSegment = SegmenterPrototype.segment;
var sgSupportedLocalesOf = Intl.Segmenter.supportedLocalesOf;
var hasOwn = Object.hasOwn;
var symbolIterator = Symbol.iterator;
var symbolToPrimitive = Symbol.toPrimitive;
var stringCharCodeAt = String.prototype.charCodeAt;
var localeCompare = String.prototype.localeCompare;
var toLocaleUpperCase = String.prototype.toLocaleUpperCase;
var numberToLocaleString = Number.prototype.toLocaleString;
var bigIntToLocaleString = BigInt.prototype.toLocaleString;
var arrayToLocaleString = Array.prototype.toLocaleString;
var TypedArrayPrototype = getPrototypeOf(Int8Array.prototype);
var typedArrayToLocaleString = TypedArrayPrototype.toLocaleString;
var DatePrototype = Date.prototype;
var dateToLocaleString = DatePrototype.toLocaleString;
var dateToLocaleDateString = DatePrototype.toLocaleDateString;
var dateToLocaleTimeString = DatePrototype.toLocaleTimeString;
var aDate = new Date(Date.UTC(2026, 0, 5, 15, 7, 9, 123));
var invalidDate = new Date(NaN);
var dateProxy = new Proxy(aDate, {});
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
Math.floor = boom;
Math.abs = boom;
Date.now = boom;
Date.prototype.getTime = boom;
Date.prototype.valueOf = boom;
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
DateTimeFormatPrototype.resolvedOptions = boom;
DateTimeFormatPrototype.formatToParts = boom;
DateTimeFormatPrototype.formatRange = boom;
DateTimeFormatPrototype.formatRangeToParts = boom;
define(DateTimeFormatPrototype, 'format', {__proto__: null, get: boom, configurable: true});
Intl.DateTimeFormat.supportedLocalesOf = boom;
ListFormatPrototype.resolvedOptions = boom;
ListFormatPrototype.format = boom;
ListFormatPrototype.formatToParts = boom;
Intl.ListFormat.supportedLocalesOf = boom;
RelativeTimeFormatPrototype.resolvedOptions = boom;
RelativeTimeFormatPrototype.format = boom;
RelativeTimeFormatPrototype.formatToParts = boom;
Intl.RelativeTimeFormat.supportedLocalesOf = boom;
DisplayNamesPrototype.resolvedOptions = boom;
DisplayNamesPrototype.of = boom;
Intl.DisplayNames.supportedLocalesOf = boom;
DurationFormatPrototype.resolvedOptions = boom;
DurationFormatPrototype.format = boom;
DurationFormatPrototype.formatToParts = boom;
Intl.DurationFormat.supportedLocalesOf = boom;
SegmenterPrototype.resolvedOptions = boom;
SegmenterPrototype.segment = boom;
Intl.Segmenter.supportedLocalesOf = boom;
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
// Locale tests pass it, as undefined. The DateTimeFormat options are here
// too, so the Locale tests pass "hourCycle" as undefined and the
// DateTimeFormat tests pass undefined or an object with every option; and
// so are the fields of DateTimeFormat's records: the resolution's, the
// [[LocaleData]] and format records', the range pattern records', the
// tokens and fields of a CLDR pattern, the offset parse, the formatter's
// data, and the keys of its tables (widths and CLDR pattern letters). The
// ListFormat tests pass every option too ("type" and "style" are already
// here); the fields of the iterator record are here, and "done", which
// IteratorStepValue reads from the iterator's own results. The
// RelativeTimeFormat tests pass every option too ("numeric" gets its
// accessor later, before them, since the Locale tests pass options objects
// without it); the fields of its records are here: the units of its data
// and the tenses and relative names of each unit.
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
  'hour12', 'hourCycle', 'timeZone', 'weekday', 'era', 'year', 'month', 'day',
  'dayPeriod', 'hour', 'minute', 'second', 'fractionalSecondDigits',
  'timeZoneName', 'formatMatcher', 'dateStyle', 'timeStyle',
  'hc', 'hourCycle12', 'hourCycle24', 'primaryIdentifier', 'sign',
  'minuteSeconds', 'fraction', 'dateTimeFormat', 'pattern12',
  'monthStandalone', 'weekdayStandalone', 'ampmWidth', 'dayPeriodWidth',
  'hourLetter', 'datePart', 'timePart', 'rangePatterns',
  'patternLetterCounts', 'letterCounts', 'skeletonWidths',
  'rangePatterns12', 'decimalSeparator', 'available', 'h12', 'h24',
  'formats', 'styles', 'dateTimeFormats', 'atTimeFormats',
  'intervalFallback', 'leapMonthPattern', 'timeZoneAppend', 'intervals',
  'skeleton', 'fields', 'letter', 'Date', 'Time', 'Connector',
  'DateTimeRangeFormat', 'family', 'Default', 'Era', 'Year', 'Month', 'Day',
  'AmPm', 'DayPeriod', 'Hour', 'Minute', 'Second', 'FractionalSecondDigits',
  'PatternParts', 'Pattern', 'Source', 'dayPeriods', 'patternParts', 'nf',
  'nf2', 'nf3', 'name', 'field', 'width', 'standalone', 'text', 'full',
  'long', 'medium', 'short', 'narrow', 'abbreviated', 'wide',
  'shortOffset', 'longOffset', 'shortGeneric', 'longGeneric',
  'G', 'y', 'U', 'r', 'M', 'L', 'd', 'E', 'c', 'a', 'b', 'B', 'h', 'H', 'K',
  'k', 'm', 's', 'S', 'z', 'O', 'v', 'symbolList', 'relatedYear', 'yearName',
  'monthCode', 'leapMonth', 'millisecond', 'inDST', 'offsetSeconds', 'names',
  'GGGG', 'GGGGG', 'MMM', 'MMMM', 'MMMMM', 'LLL', 'LLLL', 'LLLLL', 'EEE',
  'EEEE', 'EEEEE', 'ccc', 'cccc', 'ccccc', 'aaaa', 'aaaaa', 'zzzz', 'OOOO',
  'vvvv', 'iterator', 'nextMethod', 'done',
  'week', 'quarter', 'future', 'past', 'relatives', '-2', '-1', '0', '1', '2',
  '3',
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
Intl.DateTimeFormat = boom;
Intl.ListFormat = boom;
Intl.RelativeTimeFormat = boom;
Intl.DisplayNames = boom;
Intl.DurationFormat = boom;
Intl.Segmenter = boom;
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
  hourCycle: undefined,
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
  hourCycle: undefined,
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

// Intl.ListFormat (plan Review Focus 2), through the references captured
// before the tampering: the constructor, resolvedOptions, format and
// formatToParts over iterables, the closing of an iterator that gives a
// value that is not a String, and supportedLocalesOf.
/// An iterable of the values values whose iterator has a return method
/// that counts its calls in returnCalls[0]. Its results are own data
/// properties, as the iterator protocol reads them through Get.
function iterable(values, returnCalls) {
  return {
    [symbolIterator]: function () {
      var i = 0;
      return {
        next: function () {
          return i < values.length ? {done: false, value: values[i++]} : {done: true, value: undefined};
        },
        return: function () {
          returnCalls[0]++;
          return {};
        },
      };
    },
  };
}
var lfOptions = {localeMatcher: 'lookup', type: 'unit', style: 'short'};
var lfEs = new OriginalListFormat(['xx', 'es'], lfOptions);
print(stringify(apply(lfResolvedOptions, lfEs, [])));
// CHECK-NEXT: {"locale":"es","type":"unit","style":"short"}
var lfCalls = [0];
print(apply(lfFormat, lfEs, [iterable(['a', 'b', 'c'], lfCalls)]), apply(lfFormat, lfEs, [iterable(['a', 'b'], lfCalls)]), lfCalls[0]);
// CHECK-NEXT: a, b, c a y b 0
print(partsString(apply(lfFormatToParts, new OriginalListFormat('he'), [iterable(['x', 'Ibiza'], lfCalls)])));
// CHECK-NEXT: element:x|literal: ו‑|element:Ibiza
// A string iterates through String.prototype[@@iterator], which is not
// replaced; an array through the replaced Array.prototype[@@iterator], which
// the draft requires.
var lfEn = new OriginalListFormat('en');
print(apply(lfFormat, lfEn, ['xyz']), apply(lfFormat, lfEn, [undefined]) === '', thrown(() => apply(lfFormat, lfEn, [['a', 'b']]), Error));
// CHECK-NEXT: x, y, and z true ok
print(thrown(() => apply(lfFormat, lfEn, [iterable(['a', 1], lfCalls)]), OriginalTypeError), lfCalls[0]);
// CHECK-NEXT: ok 1
print(thrown(() => apply(lfFormatToParts, lfEn, [null]), OriginalTypeError), thrown(() => apply(lfFormat, {}, ['a']), OriginalTypeError));
// CHECK-NEXT: ok ok
print(stringify(apply(lfSupportedLocalesOf, null, [['de', 'xx', 'th-TH']])));
// CHECK-NEXT: ["de","th-TH"]
print(thrown(() => OriginalListFormat('en'), OriginalTypeError), thrown(() => new OriginalListFormat('en', {localeMatcher: 'lookup', type: 'x'}), OriginalRangeError));
// CHECK-NEXT: ok ok

// Intl.DateTimeFormat, through the references captured before the
// tampering: the constructor with and without new, resolvedOptions, the
// format getter and its bound function (which formats Date.now() through
// the captured Date.now), formatToParts, formatRange,
// formatRangeToParts, supportedLocalesOf, and the errors.
// The time zone offsets use the captured Math.floor. The ja formatter's
// month is numeric although "long" was asked for, because ja's
// availableFormats pattern for that skeleton writes it as a number (Node
// reports the same); its era comes from the pattern's G.
/// An options object with every option DateTimeFormat reads as an own
/// property, all undefined; a test assigns the ones it uses, which changes
/// the own properties without consulting Object.prototype.
function dtfOptions() {
  return {
    localeMatcher: undefined,
    calendar: undefined,
    numberingSystem: undefined,
    hour12: undefined,
    hourCycle: undefined,
    timeZone: undefined,
    weekday: undefined,
    era: undefined,
    year: undefined,
    month: undefined,
    day: undefined,
    dayPeriod: undefined,
    hour: undefined,
    minute: undefined,
    second: undefined,
    fractionalSecondDigits: undefined,
    timeZoneName: undefined,
    formatMatcher: undefined,
    dateStyle: undefined,
    timeStyle: undefined,
  };
}
var dtf = new OriginalDateTimeFormat();
print(stringify(apply(dtfResolvedOptions, dtf, [])));
// CHECK-NEXT: {"locale":"en-US","calendar":"gregory","numberingSystem":"latn","timeZone":"UTC","year":"numeric","month":"numeric","day":"numeric"}
var dtfFormat = apply(getDtfFormat, dtf, []);
print(typeof dtfFormat, dtfFormat === apply(getDtfFormat, dtf, []), typeof dtfFormat(), dtfFormat(0), dtfFormat(-1), dtfFormat('86400000'));
// CHECK-NEXT: function true string 1/1/1970 12/31/1969 1/2/1970
var dtfCalled = OriginalDateTimeFormat('de');
print(getPrototypeOf(dtfCalled) === DateTimeFormatPrototype, apply(dtfResolvedOptions, dtfCalled, []).locale);
// CHECK-NEXT: true de
var dtfFullOptions = dtfOptions();
dtfFullOptions.localeMatcher = 'lookup';
dtfFullOptions.numberingSystem = 'arab';
dtfFullOptions.hour12 = true;
dtfFullOptions.timeZone = 'asia/calcutta';
dtfFullOptions.weekday = 'long';
dtfFullOptions.year = 'numeric';
dtfFullOptions.month = 'long';
dtfFullOptions.day = 'numeric';
dtfFullOptions.hour = 'numeric';
dtfFullOptions.minute = 'numeric';
dtfFullOptions.timeZoneName = 'short';
dtfFullOptions.formatMatcher = 'best fit';
var dtfFull = new OriginalDateTimeFormat(['xx', 'ja-JP-u-ca-japanese-hc-h23'], dtfFullOptions);
print(stringify(apply(dtfResolvedOptions, dtfFull, [])));
// CHECK-NEXT: {"locale":"ja-JP-u-ca-japanese","calendar":"japanese","numberingSystem":"arab","timeZone":"Asia/Kolkata","hourCycle":"h11","hour12":true,"weekday":"long","era":"short","year":"numeric","month":"numeric","day":"numeric","hour":"numeric","minute":"2-digit","timeZoneName":"short"}
// format and formatToParts use neither the replaced Intl.NumberFormat nor
// its replaced prototype, nor the replaced Number.prototype.toLocaleString
// (replaced for the formatting below, then put back for the
// toLocaleString tests), nor the replaced Math.floor, String methods or
// Array.prototype; the names, day periods and numbers come from internal
// records. (ICU4C writes the japanese year "8" in latn digits, from
// CLDR's y=jpanyear override, which plan decision 17 does not apply.)
Number.prototype.toLocaleString = boom;
var dtfT = Date.UTC(2026, 0, 5, 15, 7, 9, 123);
var dtfFullFormat = apply(getDtfFormat, dtfFull, []);
print(dtfFullFormat(dtfT));
// CHECK-NEXT: 令和٨年١月٥日月曜日 午後٨:٣٧ GMT+٥:٣٠
print(partsString(apply(dtfFormatToParts, dtfFull, [dtfT])));
// CHECK-NEXT: era:令和|year:٨|literal:年|month:١|literal:月|day:٥|literal:日|weekday:月曜日|literal: |dayPeriod:午後|hour:٨|literal::|minute:٣٧|literal: |timeZoneName:GMT+٥:٣٠
var dtfStyles = dtfOptions();
dtfStyles.dateStyle = 'full';
dtfStyles.timeStyle = 'short';
dtfStyles.timeZone = '-0930';
print(stringify(apply(dtfResolvedOptions, new OriginalDateTimeFormat('en-GB', dtfStyles), [])));
// CHECK-NEXT: {"locale":"en-GB","calendar":"gregory","numberingSystem":"latn","timeZone":"-09:30","hourCycle":"h23","hour12":false,"dateStyle":"full","timeStyle":"short"}
print(partsString(apply(dtfFormatToParts, new OriginalDateTimeFormat('en-GB', dtfStyles), [dtfT])));
// CHECK-NEXT: weekday:Monday|literal:, |day:5|literal: |month:January|literal: |year:2026|literal: at |hour:05|literal::|minute:37
var dtfBasic = dtfOptions();
dtfBasic.formatMatcher = 'basic';
dtfBasic.month = '2-digit';
dtfBasic.day = '2-digit';
dtfBasic.fractionalSecondDigits = 2;
print(stringify(apply(dtfResolvedOptions, new OriginalDateTimeFormat('de', dtfBasic), [])));
// CHECK-NEXT: {"locale":"de","calendar":"gregory","numberingSystem":"latn","timeZone":"UTC","month":"2-digit","day":"2-digit","fractionalSecondDigits":2}
print(partsString(apply(dtfFormatToParts, new OriginalDateTimeFormat('de', dtfBasic), [dtfT])));
// CHECK-NEXT: day:05|literal:.|month:01|literal:., |fractionalSecond:12
// A calendar with cyclic years and leap months, a named time zone, and a
// day period with a fraction of a second.
var dtfChinese = dtfOptions();
dtfChinese.dateStyle = 'full';
dtfChinese.timeStyle = 'full';
dtfChinese.timeZone = 'America/New_York';
print(esc(partsString(apply(dtfFormatToParts, new OriginalDateTimeFormat('en-u-ca-chinese', dtfChinese), [Date.UTC(2020, 5, 1, 12)]))));
// CHECK-NEXT: weekday:Monday|literal:, |month:Fourth Monthbis|literal: |day:10|literal:, |relatedYear:2020|literal:(|yearName:geng-zi|literal:) at |hour:8|literal::|minute:00|literal::|second:00|literal:\u202f|dayPeriod:AM|literal: |timeZoneName:Eastern Daylight Time
var dtfDayPeriod = dtfOptions();
dtfDayPeriod.dayPeriod = 'long';
dtfDayPeriod.hour = 'numeric';
dtfDayPeriod.minute = '2-digit';
dtfDayPeriod.second = '2-digit';
dtfDayPeriod.fractionalSecondDigits = 3;
print(apply(getDtfFormat, new OriginalDateTimeFormat('en', dtfDayPeriod), [])(dtfT));
// CHECK-NEXT: 3:07:09.123 in the afternoon
// The ranges (plan Review Focus 3, the range half), with the replacements
// above in place: an interval pattern, the fallback for a day difference,
// a date and time style range (its date shared), the fallback with a day
// period and a fraction of a second, and a chinese date with a time zone
// name; the collapsed form is below. Reading the parts' type, value and
// source consults no accessor, since they are own data properties.
print(esc(apply(dtfFormatRange, dtfFull, [dtfT, dtfT + 3600000])));
// CHECK-NEXT: \u4ee4\u548c\u0668\u5e74\u0661\u6708\u0665\u65e5\u6708\u66dc\u65e5 \u5348\u5f8c\u0668\u6642\u0663\u0667\u5206\uff5e\u0669\u6642\u0663\u0667\u5206(GMT+\u0665:\u0663\u0660)
print(esc(rangePartsString(apply(dtfFormatRangeToParts, dtfFull, [dtfT, dtfT + 86400000]))));
// CHECK-NEXT: era:\u4ee4\u548c:startRange|year:\u0668:startRange|literal:\u5e74:startRange|month:\u0661:startRange|literal:\u6708:startRange|day:\u0665:startRange|literal:\u65e5:startRange|weekday:\u6708\u66dc\u65e5:startRange|literal: :startRange|dayPeriod:\u5348\u5f8c:startRange|hour:\u0668:startRange|literal:::startRange|minute:\u0663\u0667:startRange|literal: :startRange|timeZoneName:GMT+\u0665:\u0663\u0660:startRange|literal:\uff5e:shared|era:\u4ee4\u548c:endRange|year:\u0668:endRange|literal:\u5e74:endRange|month:\u0661:endRange|literal:\u6708:endRange|day:\u0666:endRange|literal:\u65e5:endRange|weekday:\u706b\u66dc\u65e5:endRange|literal: :endRange|dayPeriod:\u5348\u5f8c:endRange|hour:\u0668:endRange|literal:::endRange|minute:\u0663\u0667:endRange|literal: :endRange|timeZoneName:GMT+\u0665:\u0663\u0660:endRange
var dtfRangeStyles = new OriginalDateTimeFormat('en-GB', dtfStyles);
print(esc(rangePartsString(apply(dtfFormatRangeToParts, dtfRangeStyles, [dtfT, dtfT + 3600000]))));
// CHECK-NEXT: weekday:Monday:shared|literal:, :shared|day:5:shared|literal: :shared|month:January:shared|literal: :shared|year:2026:shared|literal:, :shared|hour:05:startRange|literal:::startRange|minute:37:startRange|literal:\u2013:shared|hour:06:endRange|literal:::endRange|minute:37:endRange
print(esc(rangePartsString(apply(dtfFormatRangeToParts, new OriginalDateTimeFormat('en', dtfDayPeriod), [dtfT, dtfT + 4 * 3600000]))));
// CHECK-NEXT: hour:3:startRange|literal:::startRange|minute:07:startRange|literal:::startRange|second:09:startRange|literal:.:startRange|fractionalSecond:123:startRange|literal: :startRange|dayPeriod:in the afternoon:startRange|literal:\u2009\u2013\u2009:shared|hour:7:endRange|literal:::endRange|minute:07:endRange|literal:::endRange|second:09:endRange|literal:.:endRange|fractionalSecond:123:endRange|literal: :endRange|dayPeriod:in the evening:endRange
print(esc(rangePartsString(apply(dtfFormatRangeToParts, new OriginalDateTimeFormat('en-u-ca-chinese', dtfChinese), [Date.UTC(2020, 5, 1, 12), Date.UTC(2020, 5, 1, 12, 0, 30)]))));
// CHECK-NEXT: weekday:Monday:shared|literal:, :shared|month:Fourth Monthbis:shared|literal: :shared|day:10:shared|literal:, :shared|relatedYear:2020:shared|literal:(:shared|yearName:geng-zi:shared|literal:), :shared|hour:8:startRange|literal:::startRange|minute:00:startRange|literal:::startRange|second:00:startRange|literal:\u202f:startRange|dayPeriod:AM:startRange|literal: :startRange|timeZoneName:Eastern Daylight Time:startRange|literal:\u2009\u2013\u2009:shared|hour:8:endRange|literal:::endRange|minute:00:endRange|literal:::endRange|second:30:endRange|literal:\u202f:endRange|dayPeriod:AM:endRange|literal: :endRange|timeZoneName:Eastern Daylight Time:endRange
Number.prototype.toLocaleString = numberToLocaleString;
// The date passed to format is converted with ToNumber, which calls the
// object's own valueOf and @@toPrimitive (a user-visible Get and Call, as
// the draft says), and a Date's valueOf from Date.prototype, which is
// replaced here.
var ownValueOf = new Date(0);
ownValueOf.valueOf = () => 86400000;
var ownToPrimitive = {[symbolToPrimitive]: hint => (hint === 'number' ? 2 * 86400000 : 'x')};
print(dtfFormat(ownValueOf), dtfFormat(ownToPrimitive), thrown(() => dtfFormat(new Date(0)), Error));
// CHECK-NEXT: 1/2/1970 1/3/1970 ok
print(stringify(apply(dtfSupportedLocalesOf, null, [['de', 'xx', 'th-TH']])));
// CHECK-NEXT: ["de","th-TH"]
var dtfBadZone = dtfOptions();
dtfBadZone.timeZone = 'Nowhere';
var dtfBadSeconds = dtfOptions();
dtfBadSeconds.timeZone = '+05:30:00';
var dtfConflict = dtfOptions();
dtfConflict.dateStyle = 'short';
dtfConflict.hour = 'numeric';
print(
  thrown(() => new OriginalDateTimeFormat('de', dtfBadZone), OriginalRangeError),
  thrown(() => new OriginalDateTimeFormat('de', dtfBadSeconds), OriginalRangeError),
  thrown(() => new OriginalDateTimeFormat('de', dtfConflict), OriginalTypeError),
  thrown(() => new OriginalDateTimeFormat('xx-invalid-'), OriginalRangeError),
);
// CHECK-NEXT: ok ok ok ok
print(
  thrown(() => apply(dtfResolvedOptions, {}, []), OriginalTypeError),
  thrown(() => apply(getDtfFormat, {}, []), OriginalTypeError),
  thrown(() => apply(dtfFormatToParts, dtf, [aSymbol]), OriginalTypeError),
  thrown(() => apply(dtfFormatRange, dtf, [1]), OriginalTypeError),
);
// CHECK-NEXT: ok ok ok ok
print(apply(dtfFormatToParts, dtf, []).length, esc(apply(dtfFormatRange, dtf, [0, 86400000])), apply(dtfFormatRangeToParts, dtf, [0, 1]).length);
// CHECK-NEXT: 5 1/1/1970\u2009\u2013\u20091/2/1970 5
print(
  thrown(() => dtfFormat(NaN), OriginalRangeError),
  thrown(() => apply(dtfFormatToParts, dtf, [8.64e15 + 1]), OriginalRangeError),
  thrown(() => apply(dtfFormatRange, dtf, [0, NaN]), OriginalRangeError),
  thrown(() => apply(dtfFormatRangeToParts, dtf, [8.64e15 + 1, 0]), OriginalRangeError),
  thrown(() => apply(dtfFormatRangeToParts, dtf, [0, undefined]), OriginalTypeError),
  thrown(() => dtfFormat(aSymbol), OriginalTypeError),
);
// CHECK-NEXT: ok ok ok ok ok ok

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

// The Date overrides (plan Review Focus 3, the overrides half), called as
// methods: they use neither the replaced Intl.DateTimeFormat nor its
// replaced prototype methods and format getter, neither the replaced
// Intl.NumberFormat nor Number.prototype.toLocaleString (replaced again
// here), and neither the replaced Date.prototype.getTime and valueOf nor
// the replaced Math.floor; the time value comes from the captured getTime.
// The first calls without locales and options create the default
// DateTimeFormats now.
Number.prototype.toLocaleString = boom;
print(esc(aDate.toLocaleString()), aDate.toLocaleDateString(), esc(aDate.toLocaleTimeString()));
// CHECK-NEXT: 1/5/2026, 3:07:09\u202fPM 1/5/2026 3:07:09\u202fPM
// Again, now that the defaults exist.
print(esc(aDate.toLocaleString(undefined, undefined)), aDate.toLocaleDateString(), esc(aDate.toLocaleTimeString()));
// CHECK-NEXT: 1/5/2026, 3:07:09\u202fPM 1/5/2026 3:07:09\u202fPM
// The options objects from the DateTimeFormat tests above, with every
// option: the same strings as format gives there.
print(aDate.toLocaleString(['xx', 'ja-JP-u-ca-japanese-hc-h23'], dtfFullOptions));
// CHECK-NEXT: 令和٨年١月٥日月曜日 午後٨:٣٧ GMT+٥:٣٠
print(aDate.toLocaleString('en-GB', dtfStyles), '|', aDate.toLocaleTimeString('en', dtfDayPeriod));
// CHECK-NEXT: Monday, 5 January 2026 at 05:37 | 3:07:09.123 in the afternoon
// toLocaleDateString's date defaults with only a time field asked for.
var dateHour = dtfOptions();
dateHour.hour = '2-digit';
print(esc(aDate.toLocaleDateString('de', dateHour)));
// CHECK-NEXT: 5.1.2026, 15 Uhr
// The Date's own valueOf is not consulted: its [[DateValue]] is 0.
print(ownValueOf.toLocaleDateString(), invalidDate.toLocaleString(), invalidDate.toLocaleTimeString('xx-invalid-'));
// CHECK-NEXT: 1/1/1970 Invalid Date Invalid Date
print(
  thrown(() => apply(dateToLocaleString, {}, []), OriginalTypeError),
  thrown(() => apply(dateToLocaleDateString, DatePrototype, []), OriginalTypeError),
  thrown(() => apply(dateToLocaleTimeString, dateProxy, []), OriginalTypeError),
  thrown(() => apply(dateToLocaleString, 0, []), OriginalTypeError),
);
// CHECK-NEXT: ok ok ok ok
print(
  thrown(() => aDate.toLocaleDateString('en-GB', dtfStyles), OriginalTypeError),
  thrown(() => aDate.toLocaleTimeString('de', dtfConflict), OriginalTypeError),
  thrown(() => aDate.toLocaleString('xx-invalid-'), OriginalRangeError),
  thrown(() => aDate.toLocaleDateString('de', dtfBadZone), OriginalRangeError),
);
// CHECK-NEXT: ok ok ok ok

// Intl.RelativeTimeFormat (plan Review Focus 2), through the references
// captured before the tampering, last, once Number.prototype.toLocaleString
// is replaced too: the constructor, resolvedOptions, format and
// formatToParts use neither the replaced Intl.NumberFormat, Intl.PluralRules
// and their prototype methods, nor the replaced Math, Math.abs, BigInt and
// Number.prototype.toLocaleString, and make their results without
// consulting the accessors on Object.prototype and Array.prototype.
/// The parts of the formatToParts result parts, as type:value or
/// type:value:unit for a part with a unit, separated by "|".
function unitPartsString(parts) {
  var out = '';
  for (var i = 0; i < parts.length; i++) {
    out += (i === 0 ? '' : '|') + parts[i].type + ':' + parts[i].value;
    if (hasOwn(parts[i], 'unit')) out += ':' + parts[i].unit;
  }
  return out;
}
// "numeric", a RelativeTimeFormat option and the name of a slot, gets an
// accessor on Object.prototype only now: the Locale and Collator tests
// above pass options objects without it. The options objects below have it
// as an own property.
define(ObjectPrototype, 'numeric', {__proto__: null, get: boom, set: boom, configurable: true});
var rtfOptions = {localeMatcher: 'lookup', numberingSystem: 'arab', style: 'short', numeric: 'auto'};
var rtfEnArab = new OriginalRelativeTimeFormat(['xx', 'en'], rtfOptions);
print(stringify(apply(rtfResolvedOptions, rtfEnArab, [])));
// CHECK-NEXT: {"locale":"en","style":"short","numeric":"auto","numberingSystem":"arab"}
print(apply(rtfFormat, rtfEnArab, [-1, 'day']), apply(rtfFormat, rtfEnArab, [-0, 'second']), apply(rtfFormat, rtfEnArab, [1000, 'seconds']));
// CHECK-NEXT: yesterday now in ١٬٠٠٠ sec.
print(unitPartsString(apply(rtfFormatToParts, rtfEnArab, [-1000, 'quarter'])));
// CHECK-NEXT: integer:١:quarter|group:٬:quarter|integer:٠٠٠:quarter|literal: qtrs. ago
var rtfEn = new OriginalRelativeTimeFormat('en');
print(apply(rtfFormat, rtfEn, [-1, 'day']), '|', unitPartsString(apply(rtfFormatToParts, rtfEn, [-1234.5, 'hours'])));
// CHECK-NEXT: 1 day ago | integer:1:hour|group:,:hour|integer:234:hour|decimal:.:hour|fraction:5:hour|literal: hours ago
// The plural rules: pl's "few" form, and ar's pattern without {0}.
print(apply(rtfFormat, new OriginalRelativeTimeFormat('pl'), [-2, 'year']), unitPartsString(apply(rtfFormatToParts, new OriginalRelativeTimeFormat('ar'), [-1, 'second'])));
// CHECK-NEXT: 2 lata temu literal:قبل ثانية واحدة
print(stringify(apply(rtfSupportedLocalesOf, null, [['de', 'xx', 'th-TH']])));
// CHECK-NEXT: ["de","th-TH"]
print(
  thrown(() => OriginalRelativeTimeFormat('en'), OriginalTypeError),
  thrown(() => new OriginalRelativeTimeFormat('en', null), OriginalTypeError),
  thrown(() => new OriginalRelativeTimeFormat('en', {localeMatcher: 'lookup', numberingSystem: undefined, style: 'x'}), OriginalRangeError),
);
// CHECK-NEXT: ok ok ok
print(
  thrown(() => apply(rtfFormat, rtfEn, [NaN, 'day']), OriginalRangeError),
  thrown(() => apply(rtfFormatToParts, rtfEn, [1, 'Day']), OriginalRangeError),
  thrown(() => apply(rtfFormat, {}, [1, 'day']), OriginalTypeError),
  thrown(() => apply(rtfFormat, rtfEn, [aSymbol, 'day']), OriginalTypeError),
);
// CHECK-NEXT: ok ok ok ok

// Intl.DisplayNames (plan Review Focus 2), through the references captured
// before the tampering, last: the constructor, resolvedOptions, of and
// supportedLocalesOf use no replaced intrinsic, and make their results
// without consulting the accessors on Object.prototype and Array.prototype.
// "fallback" and "languageDisplay", DisplayNames options and the names of
// slots, and "language", "script", "region" and "variants", the parts of a
// language code, get accessors only now: the Locale tests above pass
// options objects without them, and the Locale constructor reads its
// language, script, region and variants options from Object.prototype too.
// The options objects below have every option as an own property.
var dnFields = ['fallback', 'languageDisplay', 'language', 'script', 'region', 'variants'];
for (var k = 0; k < dnFields.length; k++)
  define(ObjectPrototype, dnFields[k], {__proto__: null, get: boom, set: boom, configurable: true});
/// An options object with every DisplayNames option: type and style as
/// given, localeMatcher "lookup", fallback "none" and languageDisplay
/// languageDisplay.
function dnOptions(type, style, languageDisplay) {
  return {localeMatcher: 'lookup', style: style, type: type, fallback: 'none', languageDisplay: languageDisplay};
}
var dnDe = new OriginalDisplayNames(['xx', 'de'], dnOptions('language', 'short', 'standard'));
print(stringify(apply(dnResolvedOptions, dnDe, [])));
// CHECK-NEXT: {"locale":"de","style":"short","type":"language","fallback":"none","languageDisplay":"standard"}
print(apply(dnOf, dnDe, ['zh-Hans-CN']), '|', apply(dnOf, dnDe, ['en-GB']), '|', apply(dnOf, dnDe, ['de-1996']), '|', apply(dnOf, dnDe, ['en-ZY']));
// CHECK-NEXT: Chinesisch (Vereinfacht, China) | Englisch (UK) | Deutsch (Neue deutsche Rechtschreibung) | undefined
// The dialect name, and the fullwidth brackets.
var dnEn = new OriginalDisplayNames('en', dnOptions('language', 'long', 'dialect'));
var dnZh = new OriginalDisplayNames('zh', dnOptions('language', 'long', 'dialect'));
print(apply(dnOf, dnEn, ['en-US']), '|', apply(dnOf, dnEn, ['sh']), '|', apply(dnOf, dnZh, ['en-CD']));
// CHECK-NEXT: American English | Serbian (Latin) | 英语（刚果［金］）
// The codes of the other types are canonicalized with the captured
// functions.
print(
  apply(dnOf, new OriginalDisplayNames('en', dnOptions('region', 'long', 'dialect')), ['us']),
  '|', apply(dnOf, new OriginalDisplayNames('en', dnOptions('script', 'long', 'dialect')), ['latn']),
  '|', apply(dnOf, new OriginalDisplayNames('en', dnOptions('currency', 'long', 'dialect')), ['usd']),
  '|', apply(dnOf, new OriginalDisplayNames('en', dnOptions('calendar', 'long', 'dialect')), ['GREGORY']),
  '|', apply(dnOf, new OriginalDisplayNames('en', dnOptions('dateTimeField', 'short', 'dialect')), ['year']),
);
// CHECK-NEXT: United States | Latin | US Dollar | Gregorian Calendar | yr.
print(stringify(apply(dnSupportedLocalesOf, null, [['de', 'xx', 'th-TH']])));
// CHECK-NEXT: ["de","th-TH"]
print(
  thrown(() => OriginalDisplayNames('en', dnOptions('region', 'long', 'dialect')), OriginalTypeError),
  thrown(() => new OriginalDisplayNames('en'), OriginalTypeError),
  thrown(() => new OriginalDisplayNames('en', null), OriginalTypeError),
  thrown(() => new OriginalDisplayNames('en', dnOptions('x', 'long', 'dialect')), OriginalRangeError),
  thrown(() => new OriginalDisplayNames('en', dnOptions(undefined, 'long', 'dialect')), OriginalTypeError),
);
// CHECK-NEXT: ok ok ok ok ok
var dnRegion = new OriginalDisplayNames('en', dnOptions('region', 'long', 'dialect'));
print(
  thrown(() => apply(dnOf, dnRegion, ['ZZZ']), OriginalRangeError),
  thrown(() => apply(dnOf, dnEn, ['en-u-ca-gregory']), OriginalRangeError),
  thrown(() => apply(dnOf, dnRegion, [aSymbol]), OriginalTypeError),
  thrown(() => apply(dnOf, {}, ['US']), OriginalTypeError),
  thrown(() => apply(dnResolvedOptions, {}, []), OriginalTypeError),
);
// CHECK-NEXT: ok ok ok ok ok

// Intl.DurationFormat (plan Review Focus 2), through the references
// captured before the tampering: the constructor, resolvedOptions,
// format, formatToParts and supportedLocalesOf use neither the replaced
// Intl.NumberFormat, Intl.ListFormat and their prototype methods (replaced
// above), nor the replaced BigInt, Math.abs and Array.prototype, and make
// their results without consulting the accessors on Object.prototype and
// Array.prototype. The DurationFormat options, which are also the fields
// of a duration, and the fields of DurationFormat's records get accessors
// only now. The options objects and durations below have every option and
// field as an own property, since the draft reads both with Get.
var dfUnits = ['years', 'months', 'weeks', 'days', 'hours', 'minutes', 'seconds', 'milliseconds', 'microseconds', 'nanoseconds'];
var dfFields = ['fractionalDigits', 'display', 'unitOptions', 'digitalBase', 'numberFormatUnit', 'twoDigitHours', 'hourMinuteSeparator', 'minuteSecondSeparator', 'numberFormats', 'listFormat'];
for (var k = 0; k < dfUnits.length; k++) {
  define(ObjectPrototype, dfUnits[k], {__proto__: null, get: boom, set: boom, configurable: true});
  define(ObjectPrototype, dfUnits[k] + 'Display', {__proto__: null, get: boom, set: boom, configurable: true});
}
for (var k = 0; k < dfFields.length; k++)
  define(ObjectPrototype, dfFields[k], {__proto__: null, get: boom, set: boom, configurable: true});
/// An options object with every DurationFormat option: style and
/// fractionalDigits as given, the numbering system nu, and the hours and
/// milliseconds styles hours and milliseconds; the other units and every
/// display undefined.
function dfOptions(style, fractionalDigits, nu, hours, milliseconds) {
  return {
    localeMatcher: 'lookup', numberingSystem: nu, style: style,
    years: undefined, yearsDisplay: undefined, months: undefined, monthsDisplay: undefined,
    weeks: undefined, weeksDisplay: undefined, days: undefined, daysDisplay: undefined,
    hours: hours, hoursDisplay: undefined, minutes: undefined, minutesDisplay: undefined,
    seconds: undefined, secondsDisplay: undefined, milliseconds: milliseconds, millisecondsDisplay: undefined,
    microseconds: undefined, microsecondsDisplay: undefined, nanoseconds: undefined, nanosecondsDisplay: undefined,
    fractionalDigits: fractionalDigits,
  };
}
/// A duration with every field: days, hours, minutes, seconds,
/// milliseconds and nanoseconds as given, the others undefined.
function dfDuration(days, hours, minutes, seconds, milliseconds, nanoseconds) {
  return {
    years: undefined, months: undefined, weeks: undefined, days: days, hours: hours, minutes: minutes,
    seconds: seconds, milliseconds: milliseconds, microseconds: undefined, nanoseconds: nanoseconds,
  };
}
var dfFr = new OriginalDurationFormat(['xx', 'fr'], dfOptions('digital', 3, undefined, undefined, undefined));
print(stringify(apply(dfResolvedOptions, dfFr, [])));
// CHECK-NEXT: {"locale":"fr","numberingSystem":"latn","style":"digital","years":"short","yearsDisplay":"auto","months":"short","monthsDisplay":"auto","weeks":"short","weeksDisplay":"auto","days":"short","daysDisplay":"auto","hours":"numeric","hoursDisplay":"always","minutes":"2-digit","minutesDisplay":"always","seconds":"2-digit","secondsDisplay":"always","milliseconds":"numeric","millisecondsDisplay":"auto","microseconds":"numeric","microsecondsDisplay":"auto","nanoseconds":"numeric","nanosecondsDisplay":"auto","fractionalDigits":3}
print(esc(apply(dfFormat, dfFr, [dfDuration(2, 1, 2, 3, 456, 7)])));
// CHECK-NEXT: 2\u202fj et 1:02:03,456
print(unitPartsString(apply(dfFormatToParts, dfFr, [dfDuration(undefined, -1, undefined, -3, -5, undefined)])));
// CHECK-NEXT: minusSign:-:hour|integer:1:hour|literal::|integer:00:minute|literal::|integer:03:second|decimal:,:second|fraction:005:second
// The unit styles, with the list literals and the grouping; a fractional
// unit folds into the one before it with exact BigInt arithmetic (Node's
// inexact sum gives "4,503,599,627,370,498 sec").
var dfEn = new OriginalDurationFormat('en', dfOptions('long', undefined, undefined, undefined, undefined));
print(apply(dfFormat, dfEn, [dfDuration(1, 2, 3, 4, undefined, undefined)]), '|', unitPartsString(apply(dfFormatToParts, dfEn, [dfDuration(undefined, 1234, undefined, 5, undefined, undefined)])));
// CHECK-NEXT: 1 day, 2 hours, 3 minutes, 4 seconds | integer:1:hour|group:,:hour|integer:234:hour|literal: :hour|unit:hours:hour|literal:, |integer:5:second|literal: :second|unit:seconds:second
var dfFrac = new OriginalDurationFormat('en', dfOptions('short', undefined, undefined, undefined, 'numeric'));
print(apply(dfFormat, dfFrac, [dfDuration(undefined, undefined, undefined, 1, 4503599627370497000, 1)]));
// CHECK-NEXT: 4,503,599,627,370,498.024000001 sec
// The numbering system, and ar's dual without a number part.
print(apply(dfFormat, new OriginalDurationFormat('ar', dfOptions('long', undefined, 'arab', undefined, undefined)), [dfDuration(undefined, 2, 5, undefined, undefined, undefined)]));
// CHECK-NEXT: ساعتان و٥ دقائق
print(stringify(apply(dfSupportedLocalesOf, null, [['de', 'xx', 'th-TH']])));
// CHECK-NEXT: ["de","th-TH"]
print(
  thrown(() => OriginalDurationFormat('en'), OriginalTypeError),
  thrown(() => new OriginalDurationFormat('en', null), OriginalTypeError),
  thrown(() => new OriginalDurationFormat('en', dfOptions('x', undefined, undefined, undefined, undefined)), OriginalRangeError),
  thrown(() => new OriginalDurationFormat('en', dfOptions('digital', undefined, undefined, undefined, 'long')), OriginalRangeError),
  thrown(() => new OriginalDurationFormat('en', dfOptions('short', 10, undefined, undefined, undefined)), OriginalRangeError),
);
// CHECK-NEXT: ok ok ok ok ok
print(
  thrown(() => apply(dfFormat, dfEn, ['PT1H']), OriginalRangeError),
  thrown(() => apply(dfFormat, dfEn, [1]), OriginalTypeError),
  thrown(() => apply(dfFormat, dfEn, [dfDuration(undefined, undefined, undefined, undefined, undefined, undefined)]), OriginalTypeError),
  thrown(() => apply(dfFormatToParts, dfEn, [dfDuration(undefined, 1.5, undefined, undefined, undefined, undefined)]), OriginalRangeError),
  thrown(() => apply(dfFormat, dfEn, [dfDuration(1, -1, undefined, undefined, undefined, undefined)]), OriginalRangeError),
  thrown(() => apply(dfFormat, dfEn, [dfDuration(undefined, aSymbol, undefined, undefined, undefined, undefined)]), OriginalTypeError),
  thrown(() => apply(dfFormat, {}, [dfDuration(1, undefined, undefined, undefined, undefined, undefined)]), OriginalTypeError),
  thrown(() => apply(dfResolvedOptions, {}, []), OriginalTypeError),
);
// CHECK-NEXT: ok ok ok ok ok ok ok ok

// Intl.Segmenter (plan Review Focus 2), through the references captured
// before the tampering, last: the constructor, resolvedOptions, segment,
// containing, the Segments objects' @@iterator, the iterators' next and
// supportedLocalesOf use none of the replaced intrinsics (String.prototype
// .slice, Math.trunc and Number.isFinite among them), and make the segment
// data objects and iterator results without consulting the accessors on
// Object.prototype. The Segmenter option and the fields of segment data
// objects get accessors only now, and "value", the iterator results'
// field, gets a setter too ("done" has one already). The options objects
// below have every option as an own property.
var sgFields = ['granularity', 'segment', 'index', 'input', 'isWordLike', 'value'];
for (var k = 0; k < sgFields.length; k++)
  define(ObjectPrototype, sgFields[k], {__proto__: null, get: boom, set: boom, configurable: true});
var sgWord = new OriginalSegmenter(['xx', 'en'], {localeMatcher: 'lookup', granularity: 'word'});
print(stringify(apply(sgResolvedOptions, sgWord, [])));
// CHECK-NEXT: {"locale":"en","granularity":"word"}
var sgSegments = apply(sgSegment, sgWord, ['Hi, w\ud800rld!']);
// The prototypes' methods are captured from the first Segments object and
// its iterator, then replaced: the plugin never calls them through the
// prototypes.
var SegmentsPrototype = getPrototypeOf(sgSegments);
var sgContaining = SegmentsPrototype.containing;
var sgIterate = SegmentsPrototype[symbolIterator];
var sgIt = apply(sgIterate, sgSegments, []);
var SegmentIteratorPrototype = getPrototypeOf(sgIt);
var sgNext = SegmentIteratorPrototype.next;
SegmentsPrototype.containing = boom;
SegmentsPrototype[symbolIterator] = boom;
SegmentIteratorPrototype.next = boom;
getPrototypeOf(SegmentIteratorPrototype)[symbolIterator] = boom;
print(stringify(apply(sgContaining, sgSegments, [6.7])), apply(sgContaining, sgSegments, [Infinity]), apply(sgContaining, sgSegments, [-1]));
// CHECK-NEXT: {"segment":"rld","index":6,"input":"Hi, w\ud800rld!","isWordLike":true} undefined undefined
/// The segments the iterator it gives from where it stands, each as
/// stringify of the iterator result, through the captured next.
function sgRest(it) {
  var out = '';
  for (;;) {
    var r = apply(sgNext, it, []);
    out += stringify(r);
    if (r.done) return out;
  }
}
print(sgRest(sgIt));
// CHECK-NEXT: {"value":{"segment":"Hi","index":0,"input":"Hi, w\ud800rld!","isWordLike":true},"done":false}{"value":{"segment":",","index":2,"input":"Hi, w\ud800rld!","isWordLike":false},"done":false}{"value":{"segment":" ","index":3,"input":"Hi, w\ud800rld!","isWordLike":false},"done":false}{"value":{"segment":"w","index":4,"input":"Hi, w\ud800rld!","isWordLike":true},"done":false}{"value":{"segment":"\ud800","index":5,"input":"Hi, w\ud800rld!","isWordLike":false},"done":false}{"value":{"segment":"rld","index":6,"input":"Hi, w\ud800rld!","isWordLike":true},"done":false}{"value":{"segment":"!","index":9,"input":"Hi, w\ud800rld!","isWordLike":false},"done":false}{"done":true}
// Grapheme and sentence segment data objects have no isWordLike.
var sgSentence = new OriginalSegmenter('en', {localeMatcher: 'best fit', granularity: 'sentence'});
print(sgRest(apply(sgIterate, apply(sgSegment, sgSentence, ['One. Two.']), [])));
// CHECK-NEXT: {"value":{"segment":"One. ","index":0,"input":"One. Two."},"done":false}{"value":{"segment":"Two.","index":5,"input":"One. Two."},"done":false}{"done":true}
var sgGrapheme = new OriginalSegmenter(undefined, undefined);
print(stringify(apply(sgResolvedOptions, sgGrapheme, [])), stringify(apply(sgContaining, apply(sgSegment, sgGrapheme, ['ab']), [1])));
// CHECK-NEXT: {"locale":"en-US","granularity":"grapheme"} {"segment":"b","index":1,"input":"ab"}
print(stringify(apply(sgSupportedLocalesOf, null, [['de', 'xx', 'th-TH']])));
// CHECK-NEXT: ["de","th-TH"]
print(
  thrown(() => OriginalSegmenter('en'), OriginalTypeError),
  thrown(() => new OriginalSegmenter('en', null), OriginalTypeError),
  thrown(() => new OriginalSegmenter('en', {localeMatcher: 'lookup', granularity: 'line'}), OriginalRangeError),
  thrown(() => apply(sgSegment, sgWord, [aSymbol]), OriginalTypeError),
  thrown(() => apply(sgContaining, sgSegments, [1n]), OriginalTypeError),
  thrown(() => apply(sgNext, sgSegments, []), OriginalTypeError),
  thrown(() => apply(sgContaining, sgIt, [0]), OriginalTypeError),
  thrown(() => apply(sgResolvedOptions, {}, []), OriginalTypeError),
);
// CHECK-NEXT: ok ok ok ok ok ok ok ok
