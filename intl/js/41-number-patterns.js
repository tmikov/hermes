/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// ECMA-402 "Abstract Operations for NumberFormat Objects": the parts of a
// formatted number (PartitionNumberPattern, PartitionNotationSubPattern,
// GetNumberFormatPattern, GetNotationSubPattern, FormatNumeric,
// FormatNumericToParts), the ranges (PartitionNumberRangePattern,
// FormatApproximately, CollapseNumberRange, FormatNumericRange,
// FormatNumericRangeToParts), the digit table, grouping, PartitionPattern
// (9.2.15) and the caches of the native data, following the draft
// https://tc39.es/ecma402/ dated October 24, 2026. The number data comes
// from native code as lists of strings (plan decision 5); the formatting is
// done here (decision 1).

/// The indices of the entries of a numberSymbols list (intl/jsi/
/// NativeHelpers.def). The three patterns of a style are consecutive: zero,
/// positive, negative.
var SYM_DECIMAL = 0;
var SYM_GROUP = 1;
var SYM_MINUS = 2;
var SYM_PLUS = 3;
var SYM_PERCENT = 4;
var SYM_EXPONENT = 5;
var SYM_INFINITY = 6;
var SYM_NAN = 7;
var SYM_APPROXIMATELY = 8;
var SYM_PRIMARY_GROUP = 9;
var SYM_SECONDARY_GROUP = 10;
var SYM_MINIMUM_GROUPING = 11;
var SYM_DECIMAL_PATTERNS = 12;
var SYM_PERCENT_PATTERNS = 15;
var SYM_RANGE_PATTERN = 18;
var SYM_PERCENT_PRIMARY_GROUP = 19;
var SYM_PERCENT_SECONDARY_GROUP = 20;

/// The indices of the entries of a currencyPatterns list for the code,
/// symbol and narrowSymbol displays (intl/jsi/NativeHelpers.def): the
/// zero, positive and negative patterns, the currency string, and the
/// patterns for a number that is not digits (NaN, infinity), which have no
/// currency spacing.
var CUR_PATTERNS = 0;
var CUR_STRING = 3;
var CUR_NON_DIGIT_PATTERNS = 4;

/// The index of each plural category in the lists of plural-dependent
/// patterns (the name display of currencyPatterns, unitPatterns): zero,
/// one, two, few, many, other.
var PLURAL_CATEGORY_INDEX = {
  __proto__: null,
  zero: 0,
  one: 1,
  two: 2,
  few: 3,
  many: 4,
  other: 5,
};
var PLURAL_OTHER = 5;

/// The String of the one code point cp (ECMA-262 UTF16EncodeCodePoint).
function codePointToString(cp) {
  // 1. If cp ≤ 0xFFFF, return the String value consisting of the code unit
  //    whose numeric value is cp.
  if (cp <= 0xffff) return StringFromCharCode(cp);
  // 2. Let cu1 be the code unit whose numeric value is
  //    floor((cp - 0x10000) / 0x400) + 0xD800.
  // 3. Let cu2 be the code unit whose numeric value is
  //    ((cp - 0x10000) modulo 0x400) + 0xDC00.
  // 4. Return the string-concatenation of cu1 and cu2.
  var c = cp - 0x10000;
  return StringFromCharCode(
    0xd800 + MathFloor(c / 0x400),
    0xdc00 + (c % 0x400),
  );
}

/// The ten digits from the code point first to first + 9, as Strings.
function digitRange(first) {
  // 1. Let digits be a new empty List.
  var digits = [];
  // 2. For each integer i from 0 to 9, append the String of the code point
  //    first + i to digits.
  for (var i = 0; i < 10; i++) listPush(digits, codePointToString(first + i));
  // 3. Return digits.
  return digits;
}

/// The digits of the code points in cps, as Strings.
function digitList(cps) {
  // 1. Let digits be a new empty List.
  var digits = [];
  // 2. For each code point cp of cps, append the String of cp to digits.
  for (var i = 0; i < cps.length; i++)
    listPush(digits, codePointToString(cps[i]));
  // 3. Return digits.
  return digits;
}

/// The draft's table "Numbering systems with simple digit mappings"
/// (PartitionNotationSubPattern): each numbering system's ten digits, as
/// Strings, digit 0 first. Every row is here, and every numbering system
/// in numbering-systems.txt has a row (number-format-nu.js checks it).
/// Native digits are never used (decision 4).
var NUMBERING_SYSTEM_DIGITS = {
  __proto__: null,
  adlm: digitRange(0x1e950),
  ahom: digitRange(0x11730),
  arab: digitRange(0x0660),
  arabext: digitRange(0x06f0),
  bali: digitRange(0x1b50),
  beng: digitRange(0x09e6),
  bhks: digitRange(0x11c50),
  brah: digitRange(0x11066),
  cakm: digitRange(0x11136),
  cham: digitRange(0xaa50),
  deva: digitRange(0x0966),
  diak: digitRange(0x11950),
  fullwide: digitRange(0xff10),
  gara: digitRange(0x10d40),
  gong: digitRange(0x11da0),
  gonm: digitRange(0x11d50),
  gujr: digitRange(0x0ae6),
  gukh: digitRange(0x16130),
  guru: digitRange(0x0a66),
  hanidec: digitList([
    0x3007, 0x4e00, 0x4e8c, 0x4e09, 0x56db, 0x4e94, 0x516d, 0x4e03, 0x516b,
    0x4e5d,
  ]),
  hmng: digitRange(0x16b50),
  hmnp: digitRange(0x1e140),
  java: digitRange(0xa9d0),
  kali: digitRange(0xa900),
  kawi: digitRange(0x11f50),
  khmr: digitRange(0x17e0),
  knda: digitRange(0x0ce6),
  krai: digitRange(0x16d70),
  lana: digitRange(0x1a80),
  lanatham: digitRange(0x1a90),
  laoo: digitRange(0x0ed0),
  latn: digitRange(0x0030),
  lepc: digitRange(0x1c40),
  limb: digitRange(0x1946),
  mathbold: digitRange(0x1d7ce),
  mathdbl: digitRange(0x1d7d8),
  mathmono: digitRange(0x1d7f6),
  mathsanb: digitRange(0x1d7ec),
  mathsans: digitRange(0x1d7e2),
  mlym: digitRange(0x0d66),
  modi: digitRange(0x11650),
  mong: digitRange(0x1810),
  mroo: digitRange(0x16a60),
  mtei: digitRange(0xabf0),
  mymr: digitRange(0x1040),
  mymrepka: digitRange(0x116da),
  mymrpao: digitRange(0x116d0),
  mymrshan: digitRange(0x1090),
  mymrtlng: digitRange(0xa9f0),
  nagm: digitRange(0x1e4f0),
  newa: digitRange(0x11450),
  nkoo: digitRange(0x07c0),
  olck: digitRange(0x1c50),
  onao: digitRange(0x1e5f1),
  orya: digitRange(0x0b66),
  osma: digitRange(0x104a0),
  outlined: digitRange(0x1ccf0),
  rohg: digitRange(0x10d30),
  saur: digitRange(0xa8d0),
  segment: digitRange(0x1fbf0),
  shrd: digitRange(0x111d0),
  sind: digitRange(0x112f0),
  sinh: digitRange(0x0de6),
  sora: digitRange(0x110f0),
  sund: digitRange(0x1bb0),
  sunu: digitRange(0x11bf0),
  takr: digitRange(0x116c0),
  talu: digitRange(0x19d0),
  tamldec: digitRange(0x0be6),
  telu: digitRange(0x0c66),
  thai: digitRange(0x0e50),
  tibt: digitRange(0x0f20),
  tirh: digitRange(0x114d0),
  tnsa: digitRange(0x16ac0),
  tols: digitRange(0x11de0),
  vaii: digitRange(0xa620),
  wara: digitRange(0x118e0),
  wcho: digitRange(0x1e2f0),
};

/// How many results each cache of native data keeps (decision 16). When a
/// cache holds this many and another is added, it is emptied first.
var NUMBER_DATA_CACHE_SIZE = 64;

/// How many internal plural-rules objects their cache keeps (decision 16).
var PLURAL_RULES_CACHE_SIZE = 32;

/// A new, empty cache of native data requests: a null-prototype map from
/// the request's key (its arguments, joined by ";") to the result, the
/// number of entries, and the size limit, which is NUMBER_DATA_CACHE_SIZE
/// when limit is undefined. None of the Strings joined contains ";": they
/// are canonical language identifiers, Unicode types, ISO currency codes,
/// option values and Booleans. A request is keyed by what native code
/// looks the data up by (decision 7): the language identifier of
/// [[Locale]], never the whole tag, whose -u- keywords are already in the
/// other arguments, so that "en-u-nu-thai" and "en" with numberingSystem
/// "thai" share an entry.
function newDataCache(limit = undefined) {
  // 1. Return the Record { [[Entries]]: an empty map, [[Count]]: 0,
  //    [[Limit]]: limit }. NUMBER_DATA_CACHE_SIZE is read when an entry is
  //    added, because 40-NumberFormat.js makes a cache before this file
  //    sets it.
  return {__proto__: null, entries: {__proto__: null}, count: 0, limit: limit};
}

/// The result cached under key in cache, or undefined.
function dataCacheGet(cache, key) {
  // 1. Return the entry of cache for key.
  return cache.entries[key];
}

/// Cache value under key, emptying cache first if it is full.
function dataCachePut(cache, key, value) {
  // 1. If cache is full, empty it.
  var limit = cache.limit === undefined ? NUMBER_DATA_CACHE_SIZE : cache.limit;
  if (cache.count >= limit) {
    cache.entries = {__proto__: null};
    cache.count = 0;
  }
  // 2. Add value under key.
  cache.entries[key] = value;
  cache.count++;
}

/// The cache of numberSymbols results.
var numberSymbolsCache = newDataCache();

/// native.numberSymbols(languageId, nu), through its cache. languageId is
/// GetLocaleBaseName of a [[Locale]]: the language identifier native code
/// looks the symbols up by. The list is shared and never modified.
function cachedNumberSymbols(languageId, nu) {
  // 1. Let key be languageId and nu, joined by ";".
  var key = languageId + ';' + nu;
  // 2. If the cache has a result for key, return it.
  var symbols = dataCacheGet(numberSymbolsCache, key);
  if (symbols !== undefined) return symbols;
  // 3. Otherwise ask native code for languageId and nu, and cache the
  //    result.
  symbols = native.numberSymbols(languageId, nu);
  dataCachePut(numberSymbolsCache, key, symbols);
  return symbols;
}

/// The cache of currencyPatterns results.
var currencyPatternsCache = newDataCache();

/// native.currencyPatterns(languageId, nu, currency, display, accounting),
/// through its cache. languageId is as for cachedNumberSymbols, currency an
/// uppercase ISO currency code, display a [[CurrencyDisplay]] value, and
/// accounting a Boolean. The list is shared and never modified.
function cachedCurrencyPatterns(languageId, nu, currency, display, accounting) {
  // 1. Let key be the arguments, joined by ";".
  var key =
    languageId + ';' + nu + ';' + currency + ';' + display + ';' + accounting;
  // 2. If the cache has a result for key, return it.
  var patterns = dataCacheGet(currencyPatternsCache, key);
  if (patterns !== undefined) return patterns;
  // 3. Otherwise ask native code, and cache the result.
  patterns = native.currencyPatterns(
    languageId,
    nu,
    currency,
    display,
    accounting,
  );
  dataCachePut(currencyPatternsCache, key, patterns);
  return patterns;
}

/// The cache of currencySeparators results.
var currencySeparatorsCache = newDataCache();

/// native.currencySeparators(languageId, nu, currency), through its cache:
/// the decimal and grouping separators of the currency style. The
/// arguments are as for cachedCurrencyPatterns. The list is shared and
/// never modified.
function cachedCurrencySeparators(languageId, nu, currency) {
  // 1. Let key be the arguments, joined by ";".
  var key = languageId + ';' + nu + ';' + currency;
  // 2. If the cache has a result for key, return it.
  var separators = dataCacheGet(currencySeparatorsCache, key);
  if (separators !== undefined) return separators;
  // 3. Otherwise ask native code, and cache the result.
  separators = native.currencySeparators(languageId, nu, currency);
  dataCachePut(currencySeparatorsCache, key, separators);
  return separators;
}

/// The cache of unitPatterns results.
var unitPatternsCache = newDataCache();

/// native.unitPatterns(languageId, unit, width), through its cache.
/// languageId is as for cachedNumberSymbols, unit a well-formed unit
/// identifier and width a [[UnitDisplay]] value. The list is shared and
/// never modified.
function cachedUnitPatterns(languageId, unit, width) {
  // 1. Let key be the arguments, joined by ";".
  var key = languageId + ';' + unit + ';' + width;
  // 2. If the cache has a result for key, return it.
  var patterns = dataCacheGet(unitPatternsCache, key);
  if (patterns !== undefined) return patterns;
  // 3. Otherwise ask native code, and cache the result.
  patterns = native.unitPatterns(languageId, unit, width);
  dataCachePut(unitPatternsCache, key, patterns);
  return patterns;
}

/// The number of strings of each entry of a compactPatterns list, after
/// its first string (intl/jsi/NativeHelpers.def).
var COMPACT_ENTRY_SIZE = 10;

/// The cache of compact tables, keyed as the compactPatterns requests.
var compactTableCache = newDataCache();

/// The compact patterns of native.compactPatterns(languageId, nu, long),
/// through its cache, as a compact table: a List of null-prototype records
/// { magnitude, exponent, exact0, exact1, patterns }, one per entry of
/// ICU4X's data in increasing magnitude (decision 12). magnitude is the
/// magnitude the entry starts at and exponent its exponent, as Numbers;
/// exact0 and exact1 are the patterns for exactly 0 and 1, or undefined
/// when the entry has none; patterns is the List of the zero, one, two,
/// few, many and other patterns (PLURAL_CATEGORY_INDEX). The patterns are
/// in CLDR form ("{0}K"), and one may lack "{0}" (fr long's exact-1
/// "mille"). languageId is as for cachedNumberSymbols and long a Boolean.
/// The table is shared and never modified. Intl.PluralRules uses it too,
/// for the exponent of its compact notation (compactExponentOfMagnitude).
function cachedCompactTable(languageId, nu, long) {
  // 1. Let key be the arguments, joined by ";".
  var key = languageId + ';' + nu + ';' + long;
  // 2. If the cache has a table for key, return it.
  var table = dataCacheGet(compactTableCache, key);
  if (table !== undefined) return table;
  // 3. Otherwise ask native code, and build the table from its list: entry
  //    0 is the number of entries, then each entry has COMPACT_ENTRY_SIZE
  //    strings.
  var list = native.compactPatterns(languageId, nu, long);
  var count = +list[0];
  table = [];
  for (var i = 0; i < count; i++) {
    var base = 1 + i * COMPACT_ENTRY_SIZE;
    var patterns = [];
    for (var j = 0; j < 6; j++) listPush(patterns, list[base + 4 + j]);
    listPush(table, {
      __proto__: null,
      magnitude: +list[base],
      exponent: +list[base + 1],
      exact0: list[base + 2] === '' ? undefined : list[base + 2],
      exact1: list[base + 3] === '' ? undefined : list[base + 3],
      patterns: patterns,
    });
  }
  // 4. Cache the table, and return it.
  dataCachePut(compactTableCache, key, table);
  return table;
}

/// The number of strings of each entry of a compactCurrencyPatterns list,
/// after its first string (intl/jsi/NativeHelpers.def).
var COMPACT_CURRENCY_ENTRY_SIZE = 26;

/// The cache of compact currency tables, keyed as the
/// compactCurrencyPatterns requests.
var compactCurrencyTableCache = newDataCache();

/// The compact patterns of the currency style, from
/// native.compactCurrencyPatterns(languageId, nu, currency, display,
/// accounting), through its cache, as a compact table like
/// cachedCompactTable's whose records also have signs: the List of the
/// zero, one, two, few, many and other sign patterns, each a List of the
/// zero, positive and negative patterns, whose {number} is the compact
/// pattern of the same category in patterns. exact0 and exact1 are
/// undefined: CLDR's short currency formats have none. An entry CLDR does
/// not compact has exponent 0, and its patterns and signs are not used.
/// The arguments are as for cachedCurrencyPatterns, with display "code",
/// "symbol" or "narrowSymbol". The table is shared and never modified.
function cachedCompactCurrencyTable(
  languageId,
  nu,
  currency,
  display,
  accounting,
) {
  // 1. Let key be the arguments, joined by ";".
  var key =
    languageId + ';' + nu + ';' + currency + ';' + display + ';' + accounting;
  // 2. If the cache has a table for key, return it.
  var table = dataCacheGet(compactCurrencyTableCache, key);
  if (table !== undefined) return table;
  // 3. Otherwise ask native code, and build the table from its list: entry
  //    0 is the number of entries, then each entry has
  //    COMPACT_CURRENCY_ENTRY_SIZE strings, four per category after the
  //    magnitude and the exponent.
  var list = native.compactCurrencyPatterns(
    languageId,
    nu,
    currency,
    display,
    accounting,
  );
  var count = +list[0];
  table = [];
  for (var i = 0; i < count; i++) {
    var base = 1 + i * COMPACT_CURRENCY_ENTRY_SIZE;
    var patterns = [];
    var signs = [];
    for (var j = 0; j < 6; j++) {
      var at = base + 2 + j * 4;
      listPush(patterns, list[at]);
      listPush(signs, [list[at + 1], list[at + 2], list[at + 3]]);
    }
    listPush(table, {
      __proto__: null,
      magnitude: +list[base],
      exponent: +list[base + 1],
      exact0: undefined,
      exact1: undefined,
      patterns: patterns,
      signs: signs,
    });
  }
  // 4. Cache the table, and return it.
  dataCachePut(compactCurrencyTableCache, key, table);
  return table;
}

/// The entry of the compact table table for a number of magnitude
/// magnitude: the last entry that starts at or below magnitude, or
/// undefined if there is none, for a number that is not compact
/// (decision 12).
function compactEntryOfMagnitude(table, magnitude) {
  // 1. Let found be undefined.
  var found = undefined;
  // 2. For each entry of table, in increasing magnitude, if it starts at
  //    or below magnitude, set found to it.
  for (var i = 0; i < table.length && table[i].magnitude <= magnitude; i++)
    found = table[i];
  // 3. Return found.
  return found;
}

/// The function from a magnitude to the exponent of the compact table
/// table that ComputeExponent and ComputeExponentForMagnitude take
/// (decision 12): the exponent of the entry of the magnitude, and 0 when
/// no entry starts at or below it. NumberFormat keeps one in its data;
/// Intl.PluralRules makes one from cachedCompactTable for its locale's
/// default numbering system.
function compactExponentOfMagnitude(table) {
  // 1. Return a function of magnitude that returns the exponent of
  //    compactEntryOfMagnitude(table, magnitude), or 0 if that is
  //    undefined.
  return function (magnitude) {
    var entry = compactEntryOfMagnitude(table, magnitude);
    return entry === undefined ? 0 : entry.exponent;
  };
}

/// The bidi marks that CLDR's minus or plus sign carries: the run of bidi
/// marks right before the placeholder "{signType}" ("minusSign" or
/// "plusSign") in pattern, a decimal pattern of numberSymbols (entry 14 for
/// the minus sign, 13 for the plus sign), where native code put them as
/// literal text, and the run right after it. Returns the null-prototype
/// record { before, after } of two Strings, which may be empty. ICU4C's
/// exponentMinusSign parts have the same marks as literal parts around them
/// (ar-EG "\u061c-" before the exponent), and ICU4C puts the approximately
/// sign before them.
function signMarks(pattern, signType) {
  var parts = PartitionPattern(pattern);
  var before = '';
  var after = '';
  // 1. Find the placeholder signType of the pattern.
  for (var i = 0; i < parts.length; i++) {
    if (parts[i].type !== signType) continue;
    // 2. Let before be the bidi marks at the end of the literal before it.
    if (i > 0 && parts[i - 1].type === 'literal') {
      var text = parts[i - 1].value;
      var start = text.length;
      while (start > 0 && isBidiMark(charCodeAt(text, start - 1))) start--;
      before = stringSlice(text, start);
    }
    // 3. Let after be the bidi marks at the start of the literal after it.
    if (i + 1 < parts.length && parts[i + 1].type === 'literal') {
      var next = parts[i + 1].value;
      var end = 0;
      while (end < next.length && isBidiMark(charCodeAt(next, end))) end++;
      after = stringSlice(next, 0, end);
    }
    break;
  }
  // 4. Return the two runs.
  return {__proto__: null, before: before, after: after};
}

/// The cache of internal plural-rules objects, PLURAL_RULES_CACHE_SIZE
/// entries (decision 16).
var pluralRulesCache = newDataCache(PLURAL_RULES_CACHE_SIZE);

/// The key of the internal plural-rules object of locale and type in
/// pluralRulesCache: locale and type, joined by ";".
function pluralRulesKey(locale, type) {
  // 1. Return the string-concatenation of locale, ";" and type.
  return locale + ';' + type;
}

/// The internal plural-rules object for the language identifier locale
/// (GetLocaleBaseName of a [[Locale]]) and type, "cardinal" or "ordinal":
/// a null-prototype object with the native plural rules attached by
/// pluralRulesInit. The objects are cached per locale and type; nothing
/// outside this file holds one, so user code never reaches them, and one
/// that leaves the cache is freed with its native state by the GC.
function internalPluralRules(locale, type) {
  // 1. Let key be pluralRulesKey(locale, type).
  var key = pluralRulesKey(locale, type);
  // 2. If the cache has an object for key, return it.
  var pluralRules = dataCacheGet(pluralRulesCache, key);
  if (pluralRules !== undefined) return pluralRules;
  // 3. Otherwise make one, attach the plural rules of locale and type, and
  //    cache it.
  pluralRules = {__proto__: null};
  native.pluralRulesInit(pluralRules, locale, type === 'ordinal');
  dataCachePut(pluralRulesCache, key, pluralRules);
  return pluralRules;
}

/// The cardinal plural category of the number a NumberFormat displays
/// (decision 13), for data, the formatter's numberFormatData record:
/// formattedString, the ASCII digits and "." that FormatNumericToString
/// gave for the scaled value, times 10^exponent, where exponent is the one
/// PartitionNumberPattern scaled the value by. The category is that of
/// [[Locale]], through internalPluralRules. The cache key was built with
/// the data, so a call that finds the object builds no string.
function numberFormatPlural(data, formattedString, exponent) {
  // 1. Let pluralRules be the cached internal plural rules of the
  //    formatter's language identifier; if the cache has dropped them, make
  //    them again.
  var pluralRules = dataCacheGet(pluralRulesCache, data.pluralRulesKey);
  if (pluralRules === undefined)
    pluralRules = internalPluralRules(data.languageId, 'cardinal');
  // 2. Return the category native code selects for formattedString ×
  //    10^exponent.
  return native.pluralSelect(pluralRules, formattedString, exponent);
}

/// Whether c, a code unit, is a bidi mark that CLDR puts next to numbers
/// and symbols: U+200E LEFT-TO-RIGHT MARK, U+200F RIGHT-TO-LEFT MARK or
/// U+061C ARABIC LETTER MARK. Next to the number they are literal text, as
/// in ICU4C's parts.
function isBidiMark(c) {
  // 1. Return whether c is one of those code points.
  return c === 0x200e || c === 0x200f || c === 0x061c;
}

/// Whether c, a code unit, separates a plural-dependent pattern's text
/// from its number (decision 10): ECMA-262 WhiteSpace and LineTerminator,
/// or a bidi mark.
function isPluralPatternSeparator(c) {
  // 1. Return whether c is whitespace or a bidi mark.
  return isStrWhiteSpaceChar(c) || isBidiMark(c);
}

/// Append a "literal" part with value to the List parts, joined to the
/// last part if that is a literal too: text that the draft's single
/// pattern would have as one literal, when this file builds the pattern
/// from pieces (a unit or name pattern around a sign pattern, a currency
/// string with bidi marks).
function appendLiteral(parts, value) {
  // 1. If the last part of parts is a literal, replace it with a literal
  //    of its value and value.
  var last = parts.length - 1;
  if (last >= 0 && parts[last].type === 'literal') {
    listSet(parts, last, {
      __proto__: null,
      type: 'literal',
      value: parts[last].value + value,
    });
    return;
  }
  // 2. Otherwise append the literal.
  appendPart(parts, 'literal', value);
}

/// Append the text s to the List parts as a part of type type, with the
/// bidi marks at its ends as literal parts: the currency string of the
/// code, symbol and narrowSymbol displays (CLDR's ar symbol of EGP ends in
/// U+200F), and the text of a compact pattern (he short "{0}K" followed by
/// U+200F), as in ICU4C's parts.
function appendMarkedText(parts, type, s) {
  // 1. Let start and end be the bounds of s without the bidi marks at its
  //    ends.
  var start = 0;
  var end = s.length;
  while (start < end && isBidiMark(charCodeAt(s, start))) start++;
  while (end > start && isBidiMark(charCodeAt(s, end - 1))) end--;
  // 2. Append the leading marks, the text and the trailing marks, each if
  //    it is not empty.
  if (start > 0) appendLiteral(parts, stringSlice(s, 0, start));
  if (end > start) appendPart(parts, type, stringSlice(s, start, end));
  if (end < s.length) appendLiteral(parts, stringSlice(s, end));
}

/// The parts of a plural-dependent pattern in CLDR form (a unit, currency
/// name or compact pattern, decision 10), as a List of null-prototype
/// records { type, value }. The pattern is split around "{0}", which gives
/// the record { type: "number", value: undefined } for the number. The
/// whitespace and bidi marks next to "{0}" (isPluralPatternSeparator, by
/// code unit) become a "literal" record, and the rest of each side one
/// record of type partType. A pattern without "{0}" is one record of type
/// partType with the whole pattern, and no number.
function splitPluralPattern(pattern, partType) {
  // 1. Let result be a new empty List.
  var result = [];
  // 2. Let index be StringIndexOf(pattern, "{0}", 0).
  var index = stringIndexOf(pattern, '{0}', 0);
  // 3. If index is not-found, then
  //    a. Append the Record { [[Type]]: partType, [[Value]]: pattern } to
  //       result, and return result.
  if (index === -1) {
    appendPart(result, partType, pattern);
    return result;
  }
  // 4. Let before be the substring of pattern from 0 to index, and end the
  //    index in before where its trailing separators start.
  var before = stringSlice(pattern, 0, index);
  var end = before.length;
  while (end > 0 && isPluralPatternSeparator(charCodeAt(before, end - 1)))
    end--;
  // 5. Append the text of before, then its trailing separators, if they
  //    are not empty.
  if (end > 0) appendPart(result, partType, stringSlice(before, 0, end));
  if (end < before.length)
    appendPart(result, 'literal', stringSlice(before, end));
  // 6. Append the number.
  appendPart(result, 'number', undefined);
  // 7. Let after be the substring of pattern from index + 3, and start the
  //    index in after where its leading separators end.
  var after = stringSlice(pattern, index + 3);
  var start = 0;
  while (
    start < after.length &&
    isPluralPatternSeparator(charCodeAt(after, start))
  )
    start++;
  // 8. Append the leading separators of after, then its text, if they are
  //    not empty.
  if (start > 0) appendPart(result, 'literal', stringSlice(after, 0, start));
  if (start < after.length)
    appendPart(result, partType, stringSlice(after, start));
  // 9. Return result.
  return result;
}

/// The data a NumberFormat formats with (decision 5), for the record slots
/// of its resolved slots: [[Locale]], [[NumberingSystem]], [[Style]], and
/// the currency or unit slots of its style. Returns the null-prototype
/// record { languageId (GetLocaleBaseName([[Locale]])), symbols (the
/// numberSymbols list), digits (the numbering system's ten digits, or
/// undefined for latn), decimalSeparator and groupSeparator (the
/// separators of the style), primaryGroup, secondaryGroup and
/// minimumGrouping (the grouping sizes of the style, as Numbers), currency
/// (the currencyPatterns list for the currency style, else undefined), unit
/// (the unitPatterns list for the unit style, else undefined), percentUnit
/// (whether the style is unit "percent" at width short or narrow, which
/// formats with the percent patterns and has no unit list), plural and
/// pluralPartType (the plural-dependent patterns that wrap the sign
/// patterns, the unit list or the currency name list, and the type of
/// their text, "unit" or "currency"; both undefined for the other styles),
/// pluralRulesKey (the pluralRulesCache key of the cardinal plural rules of
/// languageId), compact and compactExponent (for [[Notation]] "compact",
/// the compact table of [[CompactDisplay]], or for the currency style
/// except its name display the compact currency table, and its
/// compactExponentOfMagnitude function; else undefined), compactCurrency
/// (whether compact is the compact currency table), exponentMinus
/// (for "scientific" and "engineering", the signMarks of the minus sign in
/// the decimal negative pattern; else undefined), and patternParts and
/// splitParts, empty null-prototype objects in which cachedPartitionPattern
/// and cachedSplitPluralPattern keep the parts of each pattern the
/// formatter has used }. The constructor calls this once, after it has
/// resolved every slot.
function numberFormatData(slots) {
  // 1. Let languageId be GetLocaleBaseName([[Locale]]), the language
  //    identifier native code looks the data up by (decision 7).
  var languageId = GetLocaleBaseName(slots.locale);
  // 2. Let symbols be the number symbols of languageId and
  //    [[NumberingSystem]] (native code resolves a numbering system the
  //    locale does not define as CLDR's inheritance does).
  var nu = slots.numberingSystem;
  var symbols = cachedNumberSymbols(languageId, nu);
  // 3. The percent style groups with the sizes of the percent pattern, the
  //    others with those of the decimal pattern; both with the locale's
  //    minimum grouping digits. The sizes are decimal Strings.
  //    The short and narrow percent unit is formatted with the percent
  //    patterns too, as ICU4C does, so it groups as they do.
  var style = slots.style;
  var percentUnit =
    style === 'unit' &&
    slots.unit === 'percent' &&
    slots.unitDisplay !== 'long';
  var percent = style === 'percent' || percentUnit;
  //    The currency style has separators of its own: the currency's own
  //    (pt-CV's escudo has the decimal separator "$"), else CLDR's
  //    currencyDecimal or currencyGroup of the locale (de-AT's group is
  //    U+00A0 and its currency group "."; fr-CH's decimal is "," and its
  //    currency decimal "."), as native code resolves them.
  var decimalSeparator = symbols[SYM_DECIMAL];
  var groupSeparator = symbols[SYM_GROUP];
  if (style === 'currency') {
    var separators = cachedCurrencySeparators(languageId, nu, slots.currency);
    decimalSeparator = separators[0];
    groupSeparator = separators[1];
  }
  var primary =
    +symbols[percent ? SYM_PERCENT_PRIMARY_GROUP : SYM_PRIMARY_GROUP];
  var secondary =
    +symbols[percent ? SYM_PERCENT_SECONDARY_GROUP : SYM_SECONDARY_GROUP];
  // 4. The currency style has the currency patterns of [[Currency]] and
  //    [[CurrencyDisplay]], the accounting ones for [[CurrencySign]]
  //    "accounting"; the name display has no accounting form (decision 11).
  //    Compact notation uses them for a number it does not compact (step 7
  //    has the others).
  var currency = undefined;
  var accounting = false;
  if (style === 'currency') {
    var display = slots.currencyDisplay;
    accounting = display !== 'name' && slots.currencySign === 'accounting';
    currency = cachedCurrencyPatterns(
      languageId,
      nu,
      slots.currency,
      display,
      accounting,
    );
  }
  // 5. The unit style has the unit patterns of [[Unit]] and [[UnitDisplay]],
  //    except for the percent unit that uses the percent patterns.
  var unit = undefined;
  if (style === 'unit' && !percentUnit)
    unit = cachedUnitPatterns(languageId, slots.unit, slots.unitDisplay);
  // 6. The unit list, or the currency list of the name display, depends on
  //    the plural form (decisions 10 and 13).
  var plural = undefined;
  var pluralPartType = undefined;
  if (unit !== undefined) {
    plural = unit;
    pluralPartType = 'unit';
  } else if (style === 'currency' && slots.currencyDisplay === 'name') {
    plural = currency;
    pluralPartType = 'currency';
  }
  // 7. The compact notation has the compact table of its display
  //    (decision 12), and the scientific and engineering notations the bidi
  //    marks of the minus sign of their exponent.
  //    The draft's GetNotationSubPattern step 6.c takes the patterns of
  //    [[CompactDisplay]]. The currency style's code, symbol and
  //    narrowSymbol displays have CLDR's short currency patterns instead,
  //    whatever it is, as ICU4C does, because CLDR has compact currency
  //    patterns only in short form (en USD long 1234 is "$1.2K", as ICU4C
  //    gives): their own magnitudes, exponents and compact text, and the
  //    sign patterns that wrap the compact pattern, with the currency and
  //    its spacing (fa USD 1000 has U+00A0 after "$", which its ordinary
  //    pattern lacks). The name display spells the number out and keeps the
  //    decimal ones, long for "long" ("1.2 thousand US dollars").
  //    resolvedOptions still reports [[CompactDisplay]].
  var notation = slots.notation;
  var compact = undefined;
  var compactExponent = undefined;
  var compactCurrency = false;
  if (notation === 'compact') {
    compactCurrency = style === 'currency' && slots.currencyDisplay !== 'name';
    compact = compactCurrency
      ? cachedCompactCurrencyTable(
          languageId,
          nu,
          slots.currency,
          slots.currencyDisplay,
          accounting,
        )
      : cachedCompactTable(languageId, nu, slots.compactDisplay === 'long');
    compactExponent = compactExponentOfMagnitude(compact);
  }
  var exponentMinus = undefined;
  if (notation === 'scientific' || notation === 'engineering')
    exponentMinus = signMarks(symbols[SYM_DECIMAL_PATTERNS + 2], 'minusSign');
  // 8. Return the record. latn needs no transliteration; every other
  //    numbering system the constructor can resolve has a row in the
  //    table.
  return {
    __proto__: null,
    languageId: languageId,
    symbols: symbols,
    digits: nu === 'latn' ? undefined : NUMBERING_SYSTEM_DIGITS[nu],
    decimalSeparator: decimalSeparator,
    groupSeparator: groupSeparator,
    primaryGroup: primary,
    secondaryGroup: secondary,
    minimumGrouping: +symbols[SYM_MINIMUM_GROUPING],
    currency: currency,
    unit: unit,
    percentUnit: percentUnit,
    plural: plural,
    pluralPartType: pluralPartType,
    pluralRulesKey: pluralRulesKey(languageId, 'cardinal'),
    compact: compact,
    compactExponent: compactExponent,
    compactCurrency: compactCurrency,
    exponentMinus: exponentMinus,
    patternParts: {__proto__: null},
    splitParts: {__proto__: null},
  };
}

/// PartitionPattern(pattern) for a sign pattern or notation sub-pattern of
/// the formatter whose numberFormatData record is data. Each pattern is
/// parsed once per formatter: the patterns come from its data, so there
/// are few of them, and the List and its records are never changed after
/// they are made, so every format can share them.
function cachedPartitionPattern(data, pattern) {
  // 1. If data has the parts of pattern, return them.
  var parts = data.patternParts[pattern];
  if (parts !== undefined) return parts;
  // 2. Otherwise compute them, keep them in data, and return them.
  parts = PartitionPattern(pattern);
  data.patternParts[pattern] = parts;
  return parts;
}

/// splitPluralPattern(pattern, partType) for a plural-dependent pattern of
/// the formatter whose numberFormatData record is data, split once per
/// formatter, as cachedPartitionPattern parses. The key is partType, a
/// space and pattern; partType has no space.
function cachedSplitPluralPattern(data, pattern, partType) {
  // 1. If data has the parts of pattern for partType, return them.
  var key = partType + ' ' + pattern;
  var parts = data.splitParts[key];
  if (parts !== undefined) return parts;
  // 2. Otherwise compute them, keep them in data, and return them.
  parts = splitPluralPattern(pattern, partType);
  data.splitParts[key] = parts;
  return parts;
}

// 9.2.15 PartitionPattern ( pattern )
// Returns a List of null-prototype records { type, value }, value undefined
// for a placeholder.
function PartitionPattern(pattern) {
  // 1. Let result be a new empty List.
  var result = [];
  // 2. Let placeholderEnd be -1.
  var placeholderEnd = -1;
  // 3. Let placeholderStart be StringIndexOf(pattern, "{", 0).
  var placeholderStart = stringIndexOf(pattern, '{', 0);
  // 4. Repeat, while placeholderStart is not not-found,
  while (placeholderStart !== -1) {
    // a. Let literal be the substring of pattern from placeholderEnd + 1
    //    to placeholderStart.
    var literal = stringSlice(pattern, placeholderEnd + 1, placeholderStart);
    // b. If literal is not the empty String, then
    //    i. Append the Record { [[Type]]: "literal", [[Value]]: literal }
    //       to result.
    if (literal !== '')
      listPush(result, {__proto__: null, type: 'literal', value: literal});
    // c. Set placeholderEnd to StringIndexOf(pattern, "}",
    //    placeholderStart).
    placeholderEnd = stringIndexOf(pattern, '}', placeholderStart);
    // d. Assert: placeholderEnd is not not-found and placeholderStart <
    //    placeholderEnd.
    if (placeholderEnd === -1)
      throw new _TypeError('Intl internal error: bad pattern ' + pattern);
    // e. Let placeholderName be the substring of pattern from
    //    placeholderStart + 1 to placeholderEnd.
    var placeholderName = stringSlice(
      pattern,
      placeholderStart + 1,
      placeholderEnd,
    );
    // f. Append the Record { [[Type]]: placeholderName, [[Value]]:
    //    undefined } to result.
    listPush(result, {
      __proto__: null,
      type: placeholderName,
      value: undefined,
    });
    // g. Set placeholderStart to StringIndexOf(pattern, "{",
    //    placeholderEnd).
    placeholderStart = stringIndexOf(pattern, '{', placeholderEnd);
  }
  // 5. Let tail be the substring of pattern from placeholderEnd + 1.
  var tail = stringSlice(pattern, placeholderEnd + 1);
  // 6. If tail is not the empty String, then
  //    a. Append the Record { [[Type]]: "literal", [[Value]]: tail } to
  //       result.
  if (tail !== '')
    listPush(result, {__proto__: null, type: 'literal', value: tail});
  // 7. Return result.
  return result;
}

/// Append the part { type, value } to the List parts.
function appendPart(parts, type, value) {
  // 1. Append the Record { [[Type]]: type, [[Value]]: value } to parts.
  listPush(parts, {__proto__: null, type: type, value: value});
}

/// The ASCII digits of s replaced by the ten Strings digits, one code unit
/// at a time (the transliteration of PartitionNotationSubPattern's simple
/// digit mapping); s itself when digits is undefined (latn).
function transliterate(s, digits) {
  // 1. If digits is undefined, return s.
  if (digits === undefined) return s;
  // 2. Let transliterated be the empty String.
  var transliterated = '';
  // 3. For each code unit c of s, do
  for (var position = 0; position < s.length; position++) {
    var c = charCodeAt(s, position);
    // a. If 0x0030 ≤ c ≤ 0x0039, append digits[c - 0x0030]; otherwise
    //    append c.
    transliterated +=
      c >= 0x30 && c <= 0x39 ? digits[c - 0x30] : StringFromCharCode(c);
  }
  // 4. Return transliterated.
  return transliterated;
}

/// The groups of the ASCII integer digits intDigits, left to right, for the
/// [[UseGrouping]] value useGrouping (not false) and the grouping sizes of
/// data (decision 9): with primary size p, secondary size s, minimum
/// grouping g and n digits, "always" groups when n > p, "auto" when
/// n >= p + g, and "min2" when n >= p + max(g, 2). The groups are the last p
/// digits and then groups of s. These are "the ILND set of locations within
/// intDigits" of PartitionNotationSubPattern.
function numberGroups(intDigits, useGrouping, data) {
  var p = data.primaryGroup;
  var s = data.secondaryGroup;
  var g = data.minimumGrouping;
  var n = intDigits.length;
  // 1. Decide whether to group. A locale without a primary size never
  //    groups.
  var group;
  if (!(p > 0)) group = false;
  else if (useGrouping === 'always') group = n > p;
  else if (useGrouping === 'auto') group = n >= p + g;
  else group = n >= p + MathMax(g, 2);
  // 2. If not, the digits are one group.
  var groups = [];
  if (!group) {
    listPush(groups, intDigits);
    return groups;
  }
  // 3. Let rest be the number of digits before the last p, at least 1
  //    here. A locale without a secondary size uses the primary one.
  if (!(s > 0)) s = p;
  var rest = n - p;
  // 4. Split rest into groups of s counted from its end: the first group
  //    has the remainder, or s digits.
  var first = rest % s;
  if (first === 0) first = s;
  listPush(groups, stringSlice(intDigits, 0, first));
  for (var position = first; position < rest; position += s)
    listPush(groups, stringSlice(intDigits, position, position + s));
  // 5. Append the last p digits, and return the groups.
  listPush(groups, stringSlice(intDigits, rest));
  return groups;
}

// GetNumberFormatPattern ( numberFormat, x )
// x is an Intl mathematical value. The patterns are the data's (decision
// 5): a style's zero, positive and negative patterns are consecutive
// entries of a list, the symbols list or the currency patterns. The unit
// style and the currency name display use the decimal patterns, which
// PartitionNumberPattern wraps in the plural-dependent unit or name
// pattern (decisions 10 and 11). Returns the null-prototype record
// { pattern, noSign }: the draft's pattern, and whether it is the zero
// pattern, that is, whether no sign is shown, which decides where a
// plural-dependent pattern without a number may be used (decision 10).
// With approximately true (FormatApproximately), the positive pattern takes
// the place of the zero pattern, and noSign is still true: the parts are
// those of the zero pattern, with the plus sign where the approximately
// sign goes (decision 15).
function GetNumberFormatPattern(
  numberFormat,
  x,
  approximately = false,
  compactSigns = undefined,
) {
  // 1. Let resolvedLocaleData be numberFormat.[[LocaleData]].
  // 2. Let patterns be resolvedLocaleData.[[patterns]].
  // 3. Assert: patterns is a Record.
  //    The patterns are in numberFormat's data, requested once by the
  //    constructor for its locale, numbering system and style options.
  var data = NumberFormatSlots.get(numberFormat, 'data');
  var list = data.symbols;
  // 4. Let style be numberFormat.[[Style]].
  var style = NumberFormatSlots.get(numberFormat, 'style');
  var patterns;
  if (style === 'percent') {
    // 5. If style is "percent", then
    //    a. Set patterns to patterns.[[percent]].
    patterns = SYM_PERCENT_PATTERNS;
  } else if (style === 'unit') {
    // 6. Else if style is "unit", then
    //    a. Let unit be numberFormat.[[Unit]].
    //    b. Let unitDisplay be numberFormat.[[UnitDisplay]].
    //    c. Set patterns to patterns.[[unit]].
    //    d. If patterns doesn't have a field [[<unit>]], then
    //       i. Set unit to "fallback".
    //    e. Set patterns to patterns.[[<unit>]].
    //    f. Set patterns to patterns.[[<unitDisplay>]].
    //    The sign patterns of a unit are the decimal ones; the unit's own
    //    pattern, which depends on the plural form, wraps them (decision
    //    11). Every unit the constructor accepts has data. The percent
    //    unit at widths short and narrow takes the percent patterns
    //    instead, whose percent sign is its unit part, as in ICU4C.
    patterns = data.percentUnit ? SYM_PERCENT_PATTERNS : SYM_DECIMAL_PATTERNS;
  } else if (style === 'currency') {
    // 7. Else if style is "currency", then
    //    a. Let currency be numberFormat.[[Currency]].
    //    b. Let currencyDisplay be numberFormat.[[CurrencyDisplay]].
    //    c. Let currencySign be numberFormat.[[CurrencySign]].
    //    d. Set patterns to patterns.[[currency]].
    //    e. If patterns doesn't have a field [[<currency>]], then
    //       i. Set currency to "fallback".
    //    f. Set patterns to patterns.[[<currency>]].
    //    g. Set patterns to patterns.[[<currencyDisplay>]].
    //    h. Set patterns to patterns.[[<currencySign>]].
    //    The data is the currencyPatterns list of currency, currencyDisplay
    //    and currencySign; native code gives a currency it has no data for
    //    its code. The name display has the decimal sign patterns, which
    //    the plural-dependent name pattern wraps (decision 11). The other
    //    displays have their own, and a number that is not digits (NaN,
    //    infinity) takes the ones without currency spacing. A number that
    //    compact notation compacts takes compactSigns, the sign patterns of
    //    its short currency pattern (compactCurrencySigns).
    if (NumberFormatSlots.get(numberFormat, 'currencyDisplay') === 'name') {
      patterns = SYM_DECIMAL_PATTERNS;
    } else if (compactSigns !== undefined) {
      list = compactSigns;
      patterns = CUR_PATTERNS;
    } else {
      list = data.currency;
      patterns =
        x === 'nan' || x === 'posInf' || x === 'negInf'
          ? CUR_NON_DIGIT_PATTERNS
          : CUR_PATTERNS;
    }
  } else {
    // 8. Else,
    //    a. Assert: style is "decimal".
    //    b. Set patterns to patterns.[[decimal]].
    patterns = SYM_DECIMAL_PATTERNS;
  }
  var category;
  if (x === 'negInf') {
    // 9. If x is negative-infinity, then
    //    a. Let category be negative-non-zero.
    category = 'negativeNonZero';
  } else if (x === 'negZero') {
    // 10. Else if x is negative-zero, then
    //     a. Let category be negative-zero.
    category = 'negativeZero';
  } else if (x === 'nan') {
    // 11. Else if x is not-a-number, then
    //     a. Let category be positive-zero.
    category = 'positiveZero';
  } else if (x === 'posInf') {
    // 12. Else if x is positive-infinity, then
    //     a. Let category be positive-non-zero.
    category = 'positiveNonZero';
  } else {
    // 13. Else,
    //     a. Assert: x is a mathematical value.
    //     b. If x < 0, then
    //        i. Let category be negative-non-zero.
    //     c. Else if x > 0, then
    //        i. Let category be positive-non-zero.
    //     d. Else,
    //        i. Let category be positive-zero.
    if (x.n === 0n) category = 'positiveZero';
    else category = x.neg ? 'negativeNonZero' : 'positiveNonZero';
  }
  // The offsets of zeroPattern, positivePattern and negativePattern.
  var zeroPattern = patterns;
  var positivePattern = patterns + 1;
  var negativePattern = patterns + 2;
  var positive = category === 'positiveNonZero' || category === 'positiveZero';
  // 14. Let signDisplay be numberFormat.[[SignDisplay]].
  var signDisplay = NumberFormatSlots.get(numberFormat, 'signDisplay');
  var pattern;
  if (signDisplay === 'never') {
    // 15. If signDisplay is "never", then
    //     a. Let pattern be patterns.[[zeroPattern]].
    pattern = zeroPattern;
  } else if (signDisplay === 'auto') {
    // 16. Else if signDisplay is "auto", then
    //     a. If category is positive-non-zero or positive-zero, then
    //        i. Let pattern be patterns.[[zeroPattern]].
    //     b. Else,
    //        i. Let pattern be patterns.[[negativePattern]].
    pattern = positive ? zeroPattern : negativePattern;
  } else if (signDisplay === 'always') {
    // 17. Else if signDisplay is "always", then
    //     a. If category is positive-non-zero or positive-zero, then
    //        i. Let pattern be patterns.[[positivePattern]].
    //     b. Else,
    //        i. Let pattern be patterns.[[negativePattern]].
    pattern = positive ? positivePattern : negativePattern;
  } else if (signDisplay === 'exceptZero') {
    // 18. Else if signDisplay is "exceptZero", then
    if (category === 'positiveZero' || category === 'negativeZero') {
      // a. If category is positive-zero or negative-zero, then
      //    i. Let pattern be patterns.[[zeroPattern]].
      pattern = zeroPattern;
    } else if (category === 'positiveNonZero') {
      // b. Else if category is positive-non-zero, then
      //    i. Let pattern be patterns.[[positivePattern]].
      pattern = positivePattern;
    } else {
      // c. Else,
      //    i. Let pattern be patterns.[[negativePattern]].
      pattern = negativePattern;
    }
  } else {
    // 19. Else,
    //     a. Assert: signDisplay is "negative".
    //     b. If category is negative-non-zero, then
    //        i. Let pattern be patterns.[[negativePattern]].
    //     c. Else,
    //        i. Let pattern be patterns.[[zeroPattern]].
    pattern = category === 'negativeNonZero' ? negativePattern : zeroPattern;
  }
  // 20. Return pattern, and whether it is the zero pattern. For
  //     FormatApproximately, the zero pattern is the positive one.
  var noSign = pattern === zeroPattern;
  if (approximately && noSign) pattern = positivePattern;
  return {
    __proto__: null,
    pattern: list[pattern],
    noSign: noSign,
  };
}

// GetNotationSubPattern ( numberFormat, exponent )
// The compact patterns depend on the plural form of the displayed number
// and come in CLDR's form (decisions 5 and 12), so for the compact
// notation with a non-zero exponent this takes the rounded significand x,
// its formattedString and noSign (GetNumberFormatPattern's), and returns
// the CLDR pattern compactSubPattern chooses, which
// PartitionNotationSubPattern splits with splitPluralPattern. The other
// sub-patterns are in the draft's form.
function GetNotationSubPattern(
  numberFormat,
  exponent,
  x,
  formattedString,
  noSign,
) {
  // 1. Let resolvedLocaleData be numberFormat.[[LocaleData]].
  // 2. Let notationSubPatterns be resolvedLocaleData.[[notationSubPatterns]].
  // 3. Assert: notationSubPatterns is a Record.
  // 4. Let notation be numberFormat.[[Notation]].
  var notation = NumberFormatSlots.get(numberFormat, 'notation');
  // 5. If notation is "scientific" or notation is "engineering", then
  //    a. Return notationSubPatterns.[[scientific]].
  //    It is the same for every locale (decision 12); the separator is
  //    the locale's exponent symbol.
  if (notation === 'scientific' || notation === 'engineering')
    return '{number}{scientificSeparator}{scientificExponent}';
  // 6. Else if exponent is not 0, then
  //    a. Assert: notation is "compact".
  //    b. Let compactDisplay be numberFormat.[[CompactDisplay]].
  //    c. Let compactPatterns be
  //       notationSubPatterns.[[compact]].[[<compactDisplay>]].
  //    d. Return compactPatterns.[[<exponent>]].
  //    The formatter's data has the compact table of [[CompactDisplay]].
  if (exponent !== 0) {
    var data = NumberFormatSlots.get(numberFormat, 'data');
    return compactSubPattern(data, x, formattedString, exponent, noSign);
  }
  // 7. Else,
  //    a. Return "{number}".
  return '{number}';
}

/// The compact pattern, in CLDR form, for the rounded significand x (an
/// Intl mathematical value, not zero or negative zero for a non-zero
/// exponent unless rounding made it so) with the String formattedString and
/// the non-zero exponent, for data, a numberFormatData record with a
/// compact table. noSign is whether the number pattern shows no sign.
///
/// The entry is that of the displayed number's magnitude, the magnitude of
/// x plus exponent, among the entries with that exponent: consecutive
/// entries can share an exponent and differ in their patterns (in ar, 3000
/// takes the "few" pattern of the 10^3 entry, and 30000 is in the 10^4
/// entry, whose "few" pattern is "other"'s), as ICU4X selects by magnitude.
/// The pattern is ICU4X's choice (PluralElements::get, decision 12): when
/// the displayed number is exactly 0 or 1, with no fraction digits, the
/// entry's exact-0 or exact-1 pattern if it has one; otherwise the pattern
/// of the plural category of formattedString alone, with exponent 0 (ru
/// long 1000 is "one"). A pattern without "{0}" is used only
/// when noSign; otherwise the category's pattern is, or the "other" one if
/// that lacks "{0}" too (decision 10).
function compactSubPattern(data, x, formattedString, exponent, noSign) {
  // 1. Let entry be the entry of the displayed magnitude.
  var zero = x === 'negZero' || x.n === 0n;
  var entry = compactEntryOfExponent(data.compact, x, exponent);
  // 2. If the displayed number is exactly 0 or 1 with no fraction digits
  //    and entry has a pattern for it, let exact be that pattern.
  var exact = undefined;
  if (stringIndexOf(formattedString, '.', 0) === -1) {
    if (zero) exact = entry.exact0;
    else if (x.n === 1n && x.e === 0) exact = entry.exact1;
  }
  // 3. If exact has "{0}", or no sign is shown, return it.
  if (
    exact !== undefined &&
    (noSign || stringIndexOf(exact, '{0}', 0) !== -1)
  )
    return exact;
  // 4. Let pattern be entry's pattern for the plural category of
  //    formattedString with exponent 0.
  var pattern =
    entry.patterns[
      PLURAL_CATEGORY_INDEX[numberFormatPlural(data, formattedString, 0)]
    ];
  // 5. If pattern has no "{0}" and a sign is shown, let pattern be the
  //    "other" pattern, which has "{0}".
  if (!noSign && stringIndexOf(pattern, '{0}', 0) === -1)
    pattern = entry.patterns[PLURAL_OTHER];
  // 6. Return pattern.
  return pattern;
}

/// The entry of the compact table table for the rounded significand x
/// (as compactSubPattern takes it) and the non-zero exponent: the last
/// entry with exponent exponent that starts at or below the displayed
/// magnitude, the magnitude of x plus exponent, or the first entry with
/// exponent exponent if none does (also for a significand rounded to zero).
function compactEntryOfExponent(table, x, exponent) {
  // 1. Let magnitude be the displayed magnitude, -1 for zero.
  var zero = x === 'negZero' || x.n === 0n;
  var magnitude = zero ? -1 : intlMVMagnitude(x) + exponent;
  // 2. Find the entry.
  var entry = undefined;
  for (var i = 0; i < table.length; i++) {
    var candidate = table[i];
    if (candidate.exponent !== exponent) continue;
    if (entry === undefined || candidate.magnitude <= magnitude)
      entry = candidate;
  }
  // 3. Return it.
  return entry;
}

/// Whether the compact pattern compactSubPattern chooses for the rounded
/// significand x with the String formattedString and the non-zero exponent
/// is CLDR's "0" (`{0}` alone in the compact table): no compact form, so
/// the number is not compacted. CLDR 48.2 has it for one count of a
/// magnitude whose other counts are compact only in vec, for the decimal
/// and the currency patterns. An exact-0 or exact-1 pattern that applies
/// is never "0".
function compactPatternIsNone(data, x, formattedString, exponent) {
  // 1. Let entry be the entry of the displayed magnitude.
  var entry = compactEntryOfExponent(data.compact, x, exponent);
  // 2. If the displayed number is exactly 0 or 1 with no fraction digits
  //    and entry has a pattern for it, that pattern applies.
  if (stringIndexOf(formattedString, '.', 0) === -1) {
    var zero = x === 'negZero' || x.n === 0n;
    var exact = zero
      ? entry.exact0
      : x.n === 1n && x.e === 0
        ? entry.exact1
        : undefined;
    if (exact !== undefined) return exact === '{0}';
  }
  // 3. Otherwise return whether the pattern of the plural category of
  //    formattedString with exponent 0 is "0".
  return (
    entry.patterns[
      PLURAL_CATEGORY_INDEX[numberFormatPlural(data, formattedString, 0)]
    ] === '{0}'
  );
}

/// The sign patterns of the compact currency table of data, a
/// numberFormatData record, for the rounded significand x with the String
/// formattedString and the non-zero exponent: those of the entry
/// compactSubPattern chooses for the same number, in the plural category
/// it chooses, so that the compact pattern goes into its own sign patterns.
/// CLDR's short currency patterns all have a number and no exact-0 or
/// exact-1 forms, so the category is that of formattedString alone.
function compactCurrencySigns(data, x, formattedString, exponent) {
  // 1. Let entry be the entry of the displayed magnitude.
  var entry = compactEntryOfExponent(data.compact, x, exponent);
  // 2. Return its sign patterns for the plural category of formattedString
  //    with exponent 0.
  return entry.signs[
    PLURAL_CATEGORY_INDEX[numberFormatPlural(data, formattedString, 0)]
  ];
}

// PartitionNumberPattern ( numberFormat, x )
// x is an Intl mathematical value. Returns the null-prototype record
// { parts, exponent }: parts is the draft's List of { type, value }
// records, and exponent the one ComputeExponent chose (0 for NaN, the
// infinities and negative zero), which the range operations also use.
// approximately is GetNumberFormatPattern's, for FormatApproximately. The
// record also has outerBefore and outerAfter, the code units of the
// parts' text that come from the plural-dependent unit or currency name
// pattern before and after the number (wrapInPluralPattern), which
// CollapseNumberRange collapses on their own, and noSign, whether the
// pattern was the zero pattern (GetNumberFormatPattern), which
// FormatApproximately uses.
function PartitionNumberPattern(numberFormat, x, approximately = false) {
  var data = NumberFormatSlots.get(numberFormat, 'data');
  var symbols = data.symbols;
  var style = NumberFormatSlots.get(numberFormat, 'style');
  // 1. Let exponent be 0.
  var exponent = 0;
  var formattedString;
  if (x === 'nan') {
    // 2. If x is not-a-number, then
    //    a. Let formattedString be an ILD String value indicating the NaN
    //       value.
    formattedString = symbols[SYM_NAN];
  } else if (x === 'posInf') {
    // 3. Else if x is positive-infinity, then
    //    a. Let formattedString be an ILD String value indicating positive
    //       infinity.
    formattedString = symbols[SYM_INFINITY];
  } else if (x === 'negInf') {
    // 4. Else if x is negative-infinity, then
    //    a. Let formattedString be an ILD String value indicating negative
    //       infinity. The minus sign comes from the negative pattern.
    formattedString = symbols[SYM_INFINITY];
  } else {
    // 5. Else,
    //    The record SetNumberFormatDigitOptions filled, with [[Notation]].
    var rounding = NumberFormatSlots.get(numberFormat, 'rounding');
    //    a. If x is not negative-zero, then
    if (x !== 'negZero') {
      //  i. Assert: x is a mathematical value.
      //  ii. If numberFormat.[[Style]] is "percent", set x be 100 × x.
      if (style === 'percent') x = intlMVScale(x, 2);
      //  iii. Set exponent to ComputeExponent(numberFormat, x).
      //       The compact notation's exponents come from its compact table
      //       (decision 12).
      exponent = ComputeExponent(rounding, x, data.compactExponent);
      //  iv. Set x to x × 10^-exponent.
      var unscaled = x;
      x = intlMVScale(x, 0 - exponent);
    }
    //    b. Let formatNumberResult be FormatNumericToString(numberFormat,
    //       x).
    var formatNumberResult = FormatNumericToString(rounding, x);
    //       Where the compact pattern the plural category picks is CLDR's
    //       "0", the number is not compacted (UTS 35, Compact Number
    //       Formats): it is formatted again unscaled, with exponent 0, as a
    //       number below the first compact magnitude is (vec 1000 is
    //       "1000", where its 10^3 "one" is "0" and "other" is compact).
    if (
      exponent !== 0 &&
      data.compact !== undefined &&
      compactPatternIsNone(
        data,
        formatNumberResult.roundedNumber,
        formatNumberResult.formattedString,
        exponent,
      )
    ) {
      exponent = 0;
      formatNumberResult = FormatNumericToString(rounding, unscaled);
    }
    //    c. Let formattedString be formatNumberResult.[[FormattedString]].
    formattedString = formatNumberResult.formattedString;
    //    d. Set x to formatNumberResult.[[RoundedNumber]].
    x = formatNumberResult.roundedNumber;
  }
  // 6. Let pattern be GetNumberFormatPattern(numberFormat, x).
  //    noSign is whether it is the zero pattern (decision 10). A compacted
  //    currency amount takes the sign patterns of its short currency
  //    pattern.
  var compactSigns = undefined;
  if (data.compactCurrency && exponent !== 0)
    compactSigns = compactCurrencySigns(data, x, formattedString, exponent);
  var signPattern = GetNumberFormatPattern(
    numberFormat,
    x,
    approximately,
    compactSigns,
  );
  var pattern = signPattern.pattern;
  var noSign = signPattern.noSign;
  // 7. Let result be a new empty List.
  var result = [];
  // 8. Let patternParts be PartitionPattern(pattern).
  //    For the unit style and the currency name display, the sign pattern's
  //    parts go where the number is in the plural-dependent pattern, and
  //    that pattern's text gives the unit and currency parts (decision 10).
  var wrapped = wrapInPluralPattern(
    data,
    x,
    formattedString,
    exponent,
    noSign,
    cachedPartitionPattern(data, pattern),
  );
  var patternParts = wrapped.parts;
  // 9. For each Record { [[Type]], [[Value]] } patternPart of patternParts,
  //    do
  for (var i = 0; i < patternParts.length; i++) {
    var patternPart = patternParts[i];
    // a. Let partType be patternPart.[[Type]].
    var partType = patternPart.type;
    if (partType === 'literal') {
      // b. If partType is "literal", then
      //    i. Append the Record { [[Type]]: "literal", [[Value]]:
      //       patternPart.[[Value]] } to result.
      //    It joins a literal before it: the bidi marks of a currency
      //    string are literals next to the pattern's.
      appendLiteral(result, patternPart.value);
    } else if (partType === 'number') {
      // c. Else if partType is "number", then
      //    i. Let notationSubParts be
      //       PartitionNotationSubPattern(numberFormat, x, formattedString,
      //       exponent).
      //       It also takes noSign, for the compact patterns.
      var notationSubParts = PartitionNotationSubPattern(
        numberFormat,
        x,
        formattedString,
        exponent,
        noSign,
      );
      //    ii. Set result to the list-concatenation of result and
      //        notationSubParts.
      //        A literal joins a literal before it: the bidi marks at the
      //        start of a compact pattern's text are literals.
      for (var j = 0; j < notationSubParts.length; j++) {
        if (notationSubParts[j].type === 'literal')
          appendLiteral(result, notationSubParts[j].value);
        else listPush(result, notationSubParts[j]);
      }
    } else if (partType === 'plusSign') {
      // d. Else if partType is "plusSign", then
      //    i. Let plusSignSymbol be the ILND String representing the plus
      //       sign.
      //    ii. Append the Record { [[Type]]: "plusSign", [[Value]]:
      //        plusSignSymbol } to result.
      appendPart(result, 'plusSign', symbols[SYM_PLUS]);
    } else if (partType === 'minusSign') {
      // e. Else if partType is "minusSign", then
      //    i. Let minusSignSymbol be the ILND String representing the minus
      //       sign.
      //    ii. Append the Record { [[Type]]: "minusSign", [[Value]]:
      //        minusSignSymbol } to result.
      appendPart(result, 'minusSign', symbols[SYM_MINUS]);
    } else if (partType === 'percentSign' && style === 'percent') {
      // f. Else if partType is "percentSign" and numberFormat.[[Style]] is
      //    "percent", then
      //    i. Let percentSignSymbol be the ILND String representing the
      //       percent sign.
      //    ii. Append the Record { [[Type]]: "percentSign", [[Value]]:
      //        percentSignSymbol } to result.
      appendPart(result, 'percentSign', symbols[SYM_PERCENT]);
    } else if (partType === 'percentSign' && style === 'unit') {
      // g'. The percent unit at widths short and narrow formats with the
      //     percent patterns (GetNumberFormatPattern), and their percent
      //     sign is its unit: an ILD String representing unit, as in g.
      appendPart(result, 'unit', symbols[SYM_PERCENT]);
    } else if (partType === 'unit' && style === 'unit') {
      // g. Else if partType is "unitPrefix" and numberFormat.[[Style]] is
      //    "unit", then
      //    i. Let unit be numberFormat.[[Unit]].
      //    ii. Let unitDisplay be numberFormat.[[UnitDisplay]].
      //    iii. Let unitString be an ILD String value representing unit
      //         before x in unitDisplay form, which may depend on x in
      //         languages having different plural forms.
      //    iv. Append the Record { [[Type]]: "unit", [[Value]]: unitString }
      //        to result.
      // h. Else if partType is "unitSuffix" and numberFormat.[[Style]] is
      //    "unit", then
      //    i-iv. The same, with unitString after x.
      //    wrapInPluralPattern put the unit text before and after the
      //    number, in the plural form of x, as "unit" parts.
      appendPart(result, 'unit', patternPart.value);
    } else if (partType === 'currency' && style === 'currency') {
      // j. Else if partType is "currencyPrefix" and numberFormat.[[Style]]
      //    is "currency", then
      //    i. Let currency be numberFormat.[[Currency]].
      //    ii. Let currencyDisplay be numberFormat.[[CurrencyDisplay]].
      //    iii. Let currencyString be an ILD String value representing
      //         currency before x in currencyDisplay form, which may depend
      //         on x in languages having different plural forms.
      //    iv. Append the Record { [[Type]]: "currency", [[Value]]:
      //        currencyString } to result.
      // k. Else if partType is "currencySuffix" ..., the same after x.
      //    For the name display, wrapInPluralPattern put the name text, in
      //    the plural form of x, as "currency" parts.
      appendPart(result, 'currency', patternPart.value);
    } else if (
      (partType === 'currencyPrefix' || partType === 'currencySuffix') &&
      style === 'currency'
    ) {
      // j-k. The same for the code, symbol and narrowSymbol displays, whose
      //      patterns have the placeholders: currencyString is the currency
      //      string of the data, which does not depend on x. If the
      //      implementation does not have such a representation of
      //      currency, native code gives currency itself. Bidi marks at
      //      its ends are literal parts.
      appendMarkedText(result, 'currency', data.currency[CUR_STRING]);
    } else {
      // i. Else if partType is "currencyCode" and numberFormat.[[Style]] is
      //    "currency", then ...: the data's patterns never have it.
      // l. Else,
      //    i. Let unknown be an ILND String based on x and partType.
      //    ii. Append the Record { [[Type]]: "unknown", [[Value]]: unknown }
      //        to result.
      //    The data's patterns have no other placeholder.
      appendPart(result, 'unknown', '');
    }
  }
  // 10. Return result, with the exponent, the lengths of the outer text and
  //     noSign.
  return {
    __proto__: null,
    parts: result,
    exponent: exponent,
    outerBefore: wrapped.outerBefore,
    outerAfter: wrapped.outerAfter,
    noSign: noSign,
  };
}

/// The pattern parts PartitionNumberPattern formats with, from the parts
/// signParts of the sign pattern GetNumberFormatPattern chose for x, and
/// data, the formatter's numberFormatData record. The unit style (except
/// the short and narrow percent unit) and the currency name display wrap
/// them in the unit or name pattern of the plural category of the
/// displayed number, formattedString × 10^exponent (decision 13), split by
/// splitPluralPattern: the sign pattern's parts take the place of the
/// number, and the pattern's text becomes "unit" or "currency" parts with
/// their values (decision 10). NaN and the infinities take "other". A
/// pattern without "{0}" is used only when no sign is shown; otherwise the
/// "other" pattern is, which always has "{0}"; noSign is whether
/// GetNumberFormatPattern chose the zero pattern. Every other style returns
/// signParts. Returns the null-prototype record { parts, outerBefore,
/// outerAfter }: the parts, and the number of code units of the text of
/// the pattern before and after the number (all of it before for a pattern
/// without a number; both 0 without a pattern).
function wrapInPluralPattern(
  data,
  x,
  formattedString,
  exponent,
  noSign,
  signParts,
) {
  // 1. Let plural be the plural-dependent patterns of the style, and
  //    partType the type of their text. If there are none, return
  //    signParts.
  var plural = data.plural;
  var partType = data.pluralPartType;
  if (plural === undefined)
    return {__proto__: null, parts: signParts, outerBefore: 0, outerAfter: 0};
  // 2. Let category be the index of the plural category of the displayed
  //    number, "other" for NaN and the infinities.
  var category;
  if (x === 'nan' || x === 'posInf' || x === 'negInf') category = PLURAL_OTHER;
  else
    category =
      PLURAL_CATEGORY_INDEX[
        numberFormatPlural(data, formattedString, exponent)
      ];
  // 3. Let pattern be the category's pattern. If it has no number and the
  //    sign pattern shows a sign, let pattern be the "other" pattern.
  var pattern = plural[category];
  if (!noSign && stringIndexOf(pattern, '{0}', 0) === -1)
    pattern = plural[PLURAL_OTHER];
  // 4. Let result be the parts of pattern, with signParts in place of the
  //    number. A literal of one next to a literal of the other is one
  //    literal, as one pattern would have it.
  //    Count the code units of the pattern's text before and after the
  //    number.
  var pieces = cachedSplitPluralPattern(data, pattern, partType);
  var result = [];
  var outerBefore = 0;
  var outerAfter = 0;
  var afterNumber = false;
  for (var j = 0; j < pieces.length; j++) {
    if (pieces[j].type === 'number') {
      afterNumber = true;
      for (var k = 0; k < signParts.length; k++) {
        if (signParts[k].type === 'literal')
          appendLiteral(result, signParts[k].value);
        else listPush(result, signParts[k]);
      }
      continue;
    }
    if (afterNumber) outerAfter += pieces[j].value.length;
    else outerBefore += pieces[j].value.length;
    if (pieces[j].type === 'literal') appendLiteral(result, pieces[j].value);
    else listPush(result, pieces[j]);
  }
  // 5. Return result, with the lengths of the pattern's text.
  return {
    __proto__: null,
    parts: result,
    outerBefore: outerBefore,
    outerAfter: outerAfter,
  };
}

// PartitionNotationSubPattern ( numberFormat, x, formattedString, exponent )
// noSign is whether the number pattern GetNumberFormatPattern chose shows
// no sign, which decides whether a compact pattern without a number may be
// used (decision 10).
function PartitionNotationSubPattern(
  numberFormat,
  x,
  formattedString,
  exponent,
  noSign,
) {
  // 1. Let result be a new empty List.
  var result = [];
  if (x === 'nan') {
    // 2. If x is not-a-number, then
    //    a. Append the Record { [[Type]]: "nan", [[Value]]: formattedString
    //       } to result.
    appendPart(result, 'nan', formattedString);
  } else if (x === 'posInf' || x === 'negInf') {
    // 3. Else if x is positive-infinity or negative-infinity, then
    //    a. Append the Record { [[Type]]: "infinity", [[Value]]:
    //       formattedString } to result.
    appendPart(result, 'infinity', formattedString);
  } else {
    // 4. Else,
    //    a. Let notationSubPattern be GetNotationSubPattern(numberFormat,
    //       exponent).
    var notationSubPattern = GetNotationSubPattern(
      numberFormat,
      exponent,
      x,
      formattedString,
      noSign,
    );
    //    b. Let patternParts be PartitionPattern(notationSubPattern).
    //       A compact pattern is in CLDR form, split by splitPluralPattern
    //       (decision 10): its "number" record is the number, and its text
    //       records of type "compact" already have their values, the
    //       compactSymbol or compactName of steps iv and v.
    var compact =
      exponent !== 0 &&
      NumberFormatSlots.get(numberFormat, 'notation') === 'compact';
    var data = NumberFormatSlots.get(numberFormat, 'data');
    var patternParts = compact
      ? cachedSplitPluralPattern(data, notationSubPattern, 'compact')
      : cachedPartitionPattern(data, notationSubPattern);
    //    c. For each Record { [[Type]], [[Value]] } patternPart of
    //       patternParts, do
    for (var i = 0; i < patternParts.length; i++) {
      var patternPart = patternParts[i];
      // i. Let partType be patternPart.[[Type]].
      var partType = patternPart.type;
      if (partType === 'literal') {
        // ii. If partType is "literal", then
        //     1. Append the Record { [[Type]]: "literal", [[Value]]:
        //        patternPart.[[Value]] } to result.
        //     It joins a literal before it, from the bidi marks of a
        //     compact pattern's text.
        appendLiteral(result, patternPart.value);
      } else if (partType === 'number') {
        // iii. Else if partType is "number", then
        partitionNumberDigits(numberFormat, formattedString, result);
      } else if (partType === 'compact') {
        // iv. Else if partType is "compactSymbol", then
        //     1. Let compactSymbol be an ILD string representing exponent in
        //        short form, which may depend on x in languages having
        //        different plural forms. ...
        //     2. Append the Record { [[Type]]: "compact", [[Value]]:
        //        compactSymbol } to result.
        // v. Else if partType is "compactName", then
        //    1-2. The same in long form, as compactName.
        //    The text of the pattern compactSubPattern chose for x. The bidi
        //    marks at its ends are literal parts, as in ICU4C.
        appendMarkedText(result, 'compact', patternPart.value);
      } else if (partType === 'scientificSeparator') {
        // vi. Else if partType is "scientificSeparator", then
        //     1. Let scientificSeparator be the ILND String representing the
        //        exponent separator.
        //     2. Append the Record { [[Type]]: "exponentSeparator",
        //        [[Value]]: scientificSeparator } to result.
        appendPart(result, 'exponentSeparator', data.symbols[SYM_EXPONENT]);
      } else if (partType === 'scientificExponent') {
        // vii. Else if partType is "scientificExponent", then
        //      1. If exponent < 0, then
        if (exponent < 0) {
          // a. Let minusSignSymbol be the ILND String representing the
          //    minus sign.
          // b. Append the Record { [[Type]]: "exponentMinusSign",
          //    [[Value]]: minusSignSymbol } to result.
          //    CLDR's minus sign carries the bidi marks that native code
          //    put next to {minusSign} in the patterns; they are literal
          //    parts around it, as in ICU4C.
          var marks = data.exponentMinus;
          if (marks.before !== '') appendPart(result, 'literal', marks.before);
          appendPart(result, 'exponentMinusSign', data.symbols[SYM_MINUS]);
          if (marks.after !== '') appendPart(result, 'literal', marks.after);
          // c. Set exponent to -exponent.
          exponent = -exponent;
        }
        // 2. If exponent = 0, let exponentResult be "0"; else let
        //    exponentResult be the String consisting of the digits of the
        //    decimal representation of exponent (in order, with no leading
        //    zeroes).
        var exponentResult = `${exponent}`;
        // 3. Append the Record { [[Type]]: "exponentInteger", [[Value]]:
        //    exponentResult } to result.
        //    Its digits are the numbering system's, like the number's
        //    (decision 12).
        appendPart(
          result,
          'exponentInteger',
          transliterate(exponentResult, data.digits),
        );
      } else {
        // viii. Else,
        //       1. Let unknown be an ILND String based on x and partType.
        //       2. Append the Record { [[Type]]: "unknown", [[Value]]:
        //          unknown } to result.
        //       The sub-patterns have no other placeholder.
        appendPart(result, 'unknown', '');
      }
    }
  }
  // 5. Return result.
  return result;
}

/// Step 4.c.iii of PartitionNotationSubPattern: append to result the parts
/// of the ASCII digits and "." of formattedString. The steps are numbered
/// as the substeps of 4.c.iii.
///
/// The draft transliterates formattedString first and then finds the
/// groups in the transliterated integer digits. Here the groups are found
/// in the ASCII digits (decision 9) and each group is transliterated, which
/// gives the same parts: transliteration maps each ASCII digit to one
/// digit and leaves "." alone, and grouping by digit count on the
/// transliterated String could split a supplementary digit (mathbold) in
/// two.
function partitionNumberDigits(numberFormat, formattedString, result) {
  var data = NumberFormatSlots.get(numberFormat, 'data');
  // 1. If the numberFormat.[[NumberingSystem]] matches one of the values in
  //    the Numbering System column of the table, then
  //    a. Let digits be a List whose elements are the code points specified
  //       in the Digits column of the matching row.
  //    b. Assert: The length of digits is 10.
  //    c-h. Transliterate formattedString: transliterate does it below, for
  //         each piece.
  //    data.digits is undefined only for latn, whose digits are ASCII.
  // 2. Else,
  //    a. Use an implementation dependent algorithm to map formattedString
  //       to the appropriate representation of formattedString in the given
  //       numbering system. Every numbering system the constructor can
  //       resolve has a row, so this is not reached.
  var digits = data.digits;
  // 3. Let decimalSepIndex be StringIndexOf(formattedString, ".", 0).
  var decimalSepIndex = stringIndexOf(formattedString, '.', 0);
  var intDigits, fractionDigits;
  if (decimalSepIndex !== -1 && decimalSepIndex > 0) {
    // 4. If decimalSepIndex is not not-found and decimalSepIndex > 0, then
    //    a. Let intDigits be the substring of formattedString from 0 to
    //       decimalSepIndex.
    intDigits = stringSlice(formattedString, 0, decimalSepIndex);
    //    b. Let fractionDigits be the substring of formattedString from
    //       decimalSepIndex + 1.
    fractionDigits = stringSlice(formattedString, decimalSepIndex + 1);
  } else {
    // 5. Else,
    //    a. Let intDigits be formattedString.
    intDigits = formattedString;
    //    b. Let fractionDigits be undefined.
    fractionDigits = undefined;
  }
  var useGrouping = NumberFormatSlots.get(numberFormat, 'useGrouping');
  if (useGrouping === false) {
    // 6. If the numberFormat.[[UseGrouping]] is false, then
    //    a. Append the Record { [[Type]]: "integer", [[Value]]: intDigits }
    //       to result.
    appendPart(result, 'integer', transliterate(intDigits, digits));
  } else {
    // 7. Else,
    //    a. Let groupSepSymbol be the ILND String representing the grouping
    //       separator.
    var groupSepSymbol = data.groupSeparator;
    //    b. Let groups be a List whose elements are, in left to right
    //       order, the substrings defined by ILND set of locations within
    //       intDigits, which may depend on the value of
    //       numberFormat.[[UseGrouping]].
    var groups = numberGroups(intDigits, useGrouping, data);
    //    c. Assert: The number of elements in groups List is greater than 0.
    //    d. Repeat, while groups List is not empty,
    for (var i = 0; i < groups.length; i++) {
      // i. Remove the first element from groups and let intGroup be the
      //    value of that element.
      // ii. Append the Record { [[Type]]: "integer", [[Value]]: intGroup }
      //     to result.
      appendPart(result, 'integer', transliterate(groups[i], digits));
      // iii. If groups List is not empty, then
      //      1. Append the Record { [[Type]]: "group", [[Value]]:
      //         groupSepSymbol } to result.
      if (i + 1 < groups.length) appendPart(result, 'group', groupSepSymbol);
    }
  }
  // 8. If fractionDigits is not undefined, then
  if (fractionDigits !== undefined) {
    // a. Let decimalSepSymbol be the ILND String representing the decimal
    //    separator.
    // b. Append the Record { [[Type]]: "decimal", [[Value]]:
    //    decimalSepSymbol } to result.
    appendPart(result, 'decimal', data.decimalSeparator);
    // c. Append the Record { [[Type]]: "fraction", [[Value]]: fractionDigits
    //    } to result.
    appendPart(result, 'fraction', transliterate(fractionDigits, digits));
  }
}

// FormatNumeric ( numberFormat, x )
function FormatNumeric(numberFormat, x) {
  // 1. Let parts be PartitionNumberPattern(numberFormat, x).
  var parts = PartitionNumberPattern(numberFormat, x).parts;
  // 2. Let result be the empty String.
  var result = '';
  // 3. For each Record { [[Type]], [[Value]] } part of parts, do
  //    a. Set result to the string-concatenation of result and
  //       part.[[Value]].
  for (var i = 0; i < parts.length; i++) result += parts[i].value;
  // 4. Return result.
  return result;
}

// FormatNumericToParts ( numberFormat, x )
function FormatNumericToParts(numberFormat, x) {
  // 1. Let parts be PartitionNumberPattern(numberFormat, x).
  var parts = PartitionNumberPattern(numberFormat, x).parts;
  // 2. Let result be ! ArrayCreate(0).
  var result = [];
  // 3. Let n be 0.
  // 4. For each Record { [[Type]], [[Value]] } part of parts, do
  for (var i = 0; i < parts.length; i++) {
    // a. Let partObj be OrdinaryObjectCreate(%Object.prototype%).
    var partObj = {};
    // b. Perform ! CreateDataPropertyOrThrow(partObj, "type",
    //    part.[[Type]]).
    CreateDataPropertyOrThrow(partObj, 'type', parts[i].type);
    // c. Perform ! CreateDataPropertyOrThrow(partObj, "value",
    //    part.[[Value]]).
    CreateDataPropertyOrThrow(partObj, 'value', parts[i].value);
    // d. Perform ! CreateDataPropertyOrThrow(result, ! ToString(𝔽(n)),
    //    partObj).
    // e. Set n to n + 1.
    listPush(result, partObj);
  }
  // 5. Return result.
  return result;
}

/// The types of the parts that are a side's numbers in a range (decision
/// 15): the parts of PartitionNotationSubPattern other than the literal and
/// compact ones. Every other part of a side is its prefix or suffix.
var RANGE_NUMBER_PART_TYPES = {
  __proto__: null,
  integer: true,
  group: true,
  decimal: true,
  fraction: true,
  exponentSeparator: true,
  exponentMinusSign: true,
  exponentInteger: true,
  nan: true,
  infinity: true,
};

/// The types of the parts whose text depends on the plural form of the
/// number. In a range, two such parts match when their types do, whatever
/// their values (decision 15): "1 mile" and "5 miles" share their unit.
var RANGE_PLURAL_PART_TYPES = {
  __proto__: null,
  unit: true,
  currency: true,
  compact: true,
};

/// The record { type, value, source } of a range part.
function rangePart(type, value, source) {
  // 1. Return the Record { [[Type]]: type, [[Value]]: value, [[Source]]:
  //    source }.
  return {__proto__: null, type: type, value: value, source: source};
}

/// The concatenation of the values of the parts of the List parts.
function partsValue(parts) {
  // 1. Let result be the empty String.
  var result = '';
  // 2. For each part of parts, set result to the string-concatenation of
  //    result and part.[[Value]].
  for (var i = 0; i < parts.length; i++) result += parts[i].value;
  // 3. Return result.
  return result;
}

/// The number of code points of the String s: its code units, less the
/// low surrogates that follow a high surrogate.
function codePointCount(s) {
  // 1. Let count be the length of s.
  var count = s.length;
  // 2. For each low surrogate of s that follows a high surrogate, subtract
  //    1 from count.
  for (var i = 1; i < s.length; i++) {
    var c = charCodeAt(s, i);
    var previous = charCodeAt(s, i - 1);
    if (c >= 0xdc00 && c <= 0xdfff && previous >= 0xd800 && previous <= 0xdbff)
      count--;
  }
  // 3. Return count.
  return count;
}

/// The range separator of the range pattern pattern (numberSymbols entry
/// 18, "{0}–{1}"): the text between "{0}" and "{1}" (decision 15).
function rangeSeparator(pattern) {
  // 1. Let start be the index in pattern after "{0}", and end the index of
  //    "{1}" after it.
  var first = stringIndexOf(pattern, '{0}', 0);
  var end = first === -1 ? -1 : stringIndexOf(pattern, '{1}', first + 3);
  if (end === -1)
    throw new _TypeError('Intl internal error: bad range pattern ' + pattern);
  // 2. Return the substring of pattern from start to end.
  return stringSlice(pattern, first + 3, end);
}

/// The parts of the List parts with the part { approximatelySign,
/// approximatelySign } at index: before parts[index] when replace is false,
/// in its place when replace is true. parts[index] is a minusSign or
/// plusSign part, and marks the signMarks of its sign, or undefined. The
/// bidi marks of the sign at the end of the literal before it (marks.before)
/// are CLDR's sign symbol with the sign, so the approximately sign goes
/// before them; when the sign is replaced, its marks go with it, and so do
/// the marks after it (marks.after) at the start of the literal after it.
function insertApproximatelySign(
  parts,
  index,
  approximatelySign,
  marks,
  replace,
) {
  var result = [];
  // 1. Let before and after be the sign's bidi marks, or empty Strings.
  var before = marks === undefined ? '' : marks.before;
  var after = marks === undefined ? '' : marks.after;
  // 2. Append the parts before index to result, except the last of them if
  //    it is a literal that ends in before: append that one without before,
  //    if anything is left. Otherwise no marks precede the sign: set before
  //    to the empty String.
  var last = index - 1;
  for (var i = 0; i < last; i++) listPush(result, parts[i]);
  var value = last >= 0 ? parts[last].value : '';
  if (
    last >= 0 &&
    before !== '' &&
    parts[last].type === 'literal' &&
    value.length >= before.length &&
    stringSlice(value, value.length - before.length) === before
  ) {
    if (value.length > before.length)
      appendPart(
        result,
        'literal',
        stringSlice(value, 0, value.length - before.length),
      );
  } else {
    if (last >= 0) listPush(result, parts[last]);
    before = '';
  }
  // 3. Append the approximately sign.
  appendPart(result, 'approximatelySign', approximatelySign);
  // 4. If replace is false, append the literal before, if it is not
  //    empty, and the parts from index on, and return result.
  if (!replace) {
    if (before !== '') appendPart(result, 'literal', before);
    for (var j = index; j < parts.length; j++) listPush(result, parts[j]);
    return result;
  }
  // 5. Otherwise skip the sign. If the part after it is a literal that
  //    starts with after, append it without after, if anything is left.
  var next = index + 1;
  if (
    after !== '' &&
    next < parts.length &&
    parts[next].type === 'literal' &&
    stringSlice(parts[next].value, 0, after.length) === after
  ) {
    if (parts[next].value.length > after.length)
      appendPart(
        result,
        'literal',
        stringSlice(parts[next].value, after.length),
      );
    next++;
  }
  // 6. Append the rest of the parts, and return result.
  for (var k = next; k < parts.length; k++) listPush(result, parts[k]);
  return result;
}

// PartitionNumberRangePattern ( numberFormat, x, y )
// Returns a List of null-prototype records { type, value, source }.
function PartitionNumberRangePattern(numberFormat, x, y) {
  // 1. If x is not-a-number or y is not-a-number, throw a RangeError
  //    exception.
  if (x === 'nan' || y === 'nan')
    throw new _RangeError('Intl.NumberFormat: a range end is NaN');
  // 2. Let xResult be PartitionNumberPattern(numberFormat, x).
  var xResult = PartitionNumberPattern(numberFormat, x);
  // 3. Let yResult be PartitionNumberPattern(numberFormat, y).
  var yResult = PartitionNumberPattern(numberFormat, y);
  // 4. If FormatNumeric(numberFormat, x) is FormatNumeric(numberFormat, y),
  //    then
  //    FormatNumeric is the concatenation of the values of the parts of
  //    PartitionNumberPattern, which xResult and yResult hold.
  if (partsValue(xResult.parts) === partsValue(yResult.parts)) {
    // a. Let approximateResult be FormatApproximately(numberFormat,
    //    xResult).
    //    It takes the parts of xResult, and also x and xResult's noSign,
    //    which decide where the approximately sign goes (decision 15).
    var approximateResult = FormatApproximately(
      numberFormat,
      xResult.parts,
      x,
      xResult.noSign,
    );
    // b. For each element part of approximateResult, do
    //    i. Set part.[[Source]] to "shared".
    //    The parts are copied with the field, since FormatApproximately
    //    may return parts of xResult.
    var shared = [];
    for (var i = 0; i < approximateResult.length; i++) {
      var part = approximateResult[i];
      listPush(shared, rangePart(part.type, part.value, 'shared'));
    }
    // c. Return approximateResult.
    return shared;
  }
  // 5. Let result be a new empty List.
  var result = [];
  // 6. For each element part of xResult, do
  //    a. Append the Record { [[Type]]: part.[[Type]], [[Value]]:
  //       part.[[Value]], [[Source]]: "startRange" } to result.
  var xParts = xResult.parts;
  for (var j = 0; j < xParts.length; j++)
    listPush(result, rangePart(xParts[j].type, xParts[j].value, 'startRange'));
  // 7. Let rangeSeparator be an ILND String value used to separate two
  //    numbers.
  //    It is the text between "{0}" and "{1}" of the locale's range pattern
  //    (decision 15).
  var data = NumberFormatSlots.get(numberFormat, 'data');
  var separator = rangeSeparator(data.symbols[SYM_RANGE_PATTERN]);
  // 8. Append the Record { [[Type]]: "literal", [[Value]]: rangeSeparator,
  //    [[Source]]: "shared" } to result.
  listPush(result, rangePart('literal', separator, 'shared'));
  // 9. For each element part of yResult, do
  //    a. Append the Record { [[Type]]: part.[[Type]], [[Value]]:
  //       part.[[Value]], [[Source]]: "endRange" } to result.
  var yParts = yResult.parts;
  for (var k = 0; k < yParts.length; k++)
    listPush(result, rangePart(yParts[k].type, yParts[k].value, 'endRange'));
  // 10. Return CollapseNumberRange(numberFormat, result).
  //     It also takes xResult and yResult, for the exponents of x and y
  //     and the lengths of their outer text.
  return CollapseNumberRange(numberFormat, result, xResult, yResult);
}

// FormatApproximately ( numberFormat, result )
// result is the parts of PartitionNumberPattern(numberFormat, x); x and
// noSign, whether that used the zero pattern, are taken too. Returns a new
// List (decision 15).
function FormatApproximately(numberFormat, result, x, noSign) {
  var symbols = NumberFormatSlots.get(numberFormat, 'data').symbols;
  // 1. Let approximatelySign be an ILND String value used to signify that a
  //    number is approximate.
  //    It is CLDR's approximatelySign (numberSymbols entry 8).
  var approximatelySign = symbols[SYM_APPROXIMATELY];
  // 2. If approximatelySign is not empty, insert the Record { [[Type]]:
  //    "approximatelySign", [[Value]]: approximatelySign } at an ILND index
  //    in result.
  if (approximatelySign === '') return result;
  //    a. The index of the first minusSign or plusSign part, before the
  //       bidi marks of its sign (signMarks, from the decimal negative or
  //       positive pattern).
  for (var i = 0; i < result.length; i++) {
    var type = result[i].type;
    if (type === 'minusSign' || type === 'plusSign') {
      var pattern =
        symbols[SYM_DECIMAL_PATTERNS + (type === 'minusSign' ? 2 : 1)];
      return insertApproximatelySign(
        result,
        i,
        approximatelySign,
        signMarks(pattern, type),
        false,
      );
    }
  }
  //    b. Otherwise, if the zero pattern was used (a value that is not
  //       negative, or a negative one under signDisplay "never" or
  //       "negative"), where "{plusSign}" stands in the formatter's
  //       positive pattern: in place of the plusSign part, and its bidi
  //       marks, of the parts of x with that pattern ("~$3", "~3 €",
  //       "~$3.00" for accounting, and "\u200f~3.00 ج.م.\u200f" in ar
  //       inside the pattern's own mark, for 3 and, under "never", -3).
  if (noSign) {
    var positive = PartitionNumberPattern(numberFormat, x, true).parts;
    for (var j = 0; j < positive.length; j++) {
      if (positive[j].type === 'plusSign')
        return insertApproximatelySign(
          positive,
          j,
          approximatelySign,
          signMarks(symbols[SYM_DECIMAL_PATTERNS + 1], 'plusSign'),
          true,
        );
    }
  }
  //    c. Otherwise, before the first part: an accounting negative, whose
  //       negative pattern shows no sign ("~($3.00)"), or a pattern without
  //       the number, which shows no sign (decision 10).
  return insertApproximatelySign(
    result,
    0,
    approximatelySign,
    undefined,
    false,
  );
}

/// The bounds of the numbers of the List parts, the parts of one side of a
/// range: the null-prototype record { first, last } of the indices of its
/// first and last parts of a type of RANGE_NUMBER_PART_TYPES, or undefined
/// if it has none (a pattern without the number, decision 10). The numbers
/// run from first to last whatever lies between them, so a literal inside
/// the number (the bidi mark of ar-EG's exponent minus sign) is part of it.
function rangeNumberBounds(parts) {
  // 1. Let first and last be -1.
  var first = -1;
  var last = -1;
  // 2. For each part of parts with a type of RANGE_NUMBER_PART_TYPES, set
  //    last to its index, and first too if it is -1.
  for (var i = 0; i < parts.length; i++) {
    if (RANGE_NUMBER_PART_TYPES[parts[i].type] !== true) continue;
    if (first === -1) first = i;
    last = i;
  }
  // 3. If first is -1, return undefined; else return { first, last }.
  if (first === -1) return undefined;
  return {__proto__: null, first: first, last: last};
}

/// Whether the parts of a from aStart to aEnd match those of b from bStart
/// to bEnd: there are as many, and each pair has the same type and, except
/// for the types of RANGE_PLURAL_PART_TYPES, the same value (decision 15).
function rangePartsMatch(a, aStart, aEnd, b, bStart, bEnd) {
  // 1. If the counts differ, return false.
  if (aEnd - aStart !== bEnd - bStart) return false;
  // 2. For each pair of parts, if their types differ, or their values
  //    differ and their type is not plural-dependent, return false.
  for (var i = 0; i < aEnd - aStart; i++) {
    var p = a[aStart + i];
    var q = b[bStart + i];
    if (p.type !== q.type) return false;
    if (RANGE_PLURAL_PART_TYPES[p.type] !== true && p.value !== q.value)
      return false;
  }
  // 3. Return true.
  return true;
}

/// Whether the parts of the List parts from start to end, a middle affix
/// that collapsing would drop, have a part whose text the formatter shows
/// for every value: a currency or percentSign part (CollapseNumberRange).
function rangeAffixAlwaysShown(parts, start, end) {
  // 1. Return whether a part from start to end has the type "currency" or
  //    "percentSign".
  for (var i = start; i < end; i++) {
    var type = parts[i].type;
    if (type === 'currency' || type === 'percentSign') return true;
  }
  return false;
}

/// Whether the List parts has an exponentSeparator part.
function hasExponentPart(parts) {
  // 1. Return whether a part of parts has the type "exponentSeparator".
  for (var i = 0; i < parts.length; i++)
    if (parts[i].type === 'exponentSeparator') return true;
  return false;
}

/// separator with a space added at each end that is not whitespace
/// (isStrWhiteSpaceChar), as ICU4C adds U+0020.
function spaceRangeSeparator(separator) {
  // 1. If separator is empty or does not start with whitespace, prepend a
  //    space.
  if (separator === '' || !isStrWhiteSpaceChar(charCodeAt(separator, 0)))
    separator = ' ' + separator;
  // 2. If it does not end with whitespace, append a space.
  if (!isStrWhiteSpaceChar(charCodeAt(separator, separator.length - 1)))
    separator = separator + ' ';
  // 3. Return separator.
  return separator;
}

/// Append the part { type, value, source } to the List list, joined to the
/// last part if both are literals with the same source.
function appendRangePart(list, type, value, source) {
  // 1. If the last part of list is a literal with source source and type
  //    is "literal", replace it with a literal of its value and value.
  var last = list.length - 1;
  if (
    type === 'literal' &&
    last >= 0 &&
    list[last].type === 'literal' &&
    list[last].source === source
  ) {
    listSet(list, last, rangePart('literal', list[last].value + value, source));
    return;
  }
  // 2. Otherwise append the part.
  listPush(list, rangePart(type, value, source));
}

/// Append the parts of the List parts from start to end to the List list
/// (appendRangePart), with the source source, or their own if source is
/// undefined.
function appendRangeParts(list, parts, start, end, source) {
  // 1. For each part from start to end, append it with its source.
  for (var i = start; i < end; i++) {
    var part = parts[i];
    appendRangePart(
      list,
      part.type,
      part.value,
      source === undefined ? part.source : source,
    );
  }
}

/// One side of a range, the List parts, split into its levels: the
/// null-prototype record { outerPrefix, middle, outerSuffix } of three
/// Lists of parts. outerPrefix holds the first outerBefore code units of
/// the parts' text, outerSuffix the last outerAfter, and middle the rest:
/// the text of the unit or currency name pattern, and everything inside it
/// (PartitionNumberPattern's outerBefore and outerAfter). A literal across
/// a boundary is split there; only literals are joined, so no other part
/// crosses one.
function splitRangeSide(parts, outerBefore, outerAfter) {
  var outerPrefix = [];
  var middle = [];
  var outerSuffix = [];
  // 1. Let total be the length of the parts' text, and middleEnd the
  //    offset where the outer suffix starts.
  var total = 0;
  for (var i = 0; i < parts.length; i++) total += parts[i].value.length;
  var middleEnd = total - outerAfter;
  // 2. For each part, at offset offset in the text, append the piece of
  //    its value before outerBefore to outerPrefix, the piece from
  //    outerBefore to middleEnd to middle, and the piece from middleEnd on
  //    to outerSuffix, each that is not empty.
  var offset = 0;
  for (var j = 0; j < parts.length; j++) {
    var part = parts[j];
    var value = part.value;
    var end = offset + value.length;
    var a = MathMin(MathMax(outerBefore - offset, 0), value.length);
    var b = MathMin(MathMax(middleEnd - offset, a), value.length);
    if (a > 0)
      listPush(
        outerPrefix,
        rangePart(part.type, stringSlice(value, 0, a), part.source),
      );
    if (b > a)
      listPush(
        middle,
        rangePart(part.type, stringSlice(value, a, b), part.source),
      );
    if (value.length > b)
      listPush(
        outerSuffix,
        rangePart(part.type, stringSlice(value, b), part.source),
      );
    offset = end;
  }
  // 3. Return the three Lists.
  return {
    __proto__: null,
    outerPrefix: outerPrefix,
    middle: middle,
    outerSuffix: outerSuffix,
  };
}

/// The number of code points of the text of the parts of the List parts
/// from start to end.
function rangeTextCodePoints(parts, start, end) {
  // 1. Return the codePointCount of the concatenation of their values.
  var text = '';
  for (var i = start; i < end; i++) text += parts[i].value;
  return codePointCount(text);
}

// CollapseNumberRange ( numberFormat, result )
// result is PartitionNumberRangePattern's: the parts of x with the source
// "startRange", the separator with the source "shared", and the parts of y
// with the source "endRange". It also takes xResult and yResult, the
// records of PartitionNumberPattern for x and y, for their exponents and
// the lengths of their outer text. The rules are ICU4C's for its default
// collapse mode, on two levels:
// - the outer affixes, the text of a unit or currency name pattern (and
//   its literals), collapse when the two sides' match;
// - the middle affixes, every other part outside the numbers (signs, the
//   currency symbol or code, the percent sign, compact text, and their
//   patterns' literals), collapse only when the outer ones did, the two
//   sides' match, x and y have the same exponent, their text has more than
//   one code point, neither side shows an exponent (scientific or
//   engineering), and one of the affixes the collapse drops (x's middle
//   suffix and y's middle prefix) has the currency or the percent sign.
// The last two conditions keep the draft's rule that collapsing must not
// make two ranges equal that are not equal in full (step 4 below):
// ICU4C's "3–5 thousand" is also its range from 3 to 5000, and its
// "\u200e-3E0 – 5E0" in ar both -3 to -5 and -3 to 5. The outer affixes
// are shown for every value, so they collapse without that check.
// Affixes match as rangePartsMatch says. A side without a number never
// collapses (decision 10).
function CollapseNumberRange(numberFormat, result, xResult, yResult) {
  // 1. Let start be the parts of result before the separator, separator
  //    its value, and end the parts after it.
  var start = [];
  var end = [];
  var separator = undefined;
  for (var i = 0; i < result.length; i++) {
    var part = result[i];
    if (part.source === 'startRange') listPush(start, part);
    else if (part.source === 'endRange') listPush(end, part);
    else separator = part.value;
  }
  // 2. Split each side into its outer prefix, its middle and its outer
  //    suffix (splitRangeSide), and let a and b be the bounds of the
  //    numbers of the two middles (rangeNumberBounds). The parts of a
  //    middle before its numbers are its middle prefix, and those after
  //    them its middle suffix.
  var x = splitRangeSide(start, xResult.outerBefore, xResult.outerAfter);
  var y = splitRangeSide(end, yResult.outerBefore, yResult.outerAfter);
  var xm = x.middle;
  var ym = y.middle;
  var a = rangeNumberBounds(xm);
  var b = rangeNumberBounds(ym);
  // 3. Let outer be true if both sides have numbers, and the outer
  //    prefixes and the outer suffixes match.
  var outer =
    a !== undefined &&
    b !== undefined &&
    rangePartsMatch(
      x.outerPrefix,
      0,
      x.outerPrefix.length,
      y.outerPrefix,
      0,
      y.outerPrefix.length,
    ) &&
    rangePartsMatch(
      x.outerSuffix,
      0,
      x.outerSuffix.length,
      y.outerSuffix,
      0,
      y.outerSuffix.length,
    );
  // 4. Let middle be true if outer is true, x and y have the same exponent
  //    (decision 12; 0 for the standard notation), neither side has an
  //    exponent part, the middle prefixes and the middle suffixes match,
  //    the text of x's middle prefix and suffix has more than one code
  //    point, and x's middle suffix or y's middle prefix, the affixes the
  //    collapse drops, has a part shown for every value
  //    (rangeAffixAlwaysShown).
  //    That last condition keeps the collapse unambiguous, as the draft
  //    requires, by counting: every value of the formatter shows that
  //    text, so the range of any pair that does not collapse shows it once
  //    for each side, and a collapsed range, which drops it from one side,
  //    shows it one time fewer; the two can never be equal. Two collapsed
  //    ranges that are equal have equal affixes and equal numbers, since
  //    the affixes have no digits and sides with an exponent never
  //    collapse, so they are the range of one pair. A sign or compact text
  //    alone gives no such count, since another value can lack it or show
  //    a different one ("3–5 thousand" is also the range from 3 to 5000),
  //    so such a middle level is repeated.
  var middle =
    outer &&
    xResult.exponent === yResult.exponent &&
    !hasExponentPart(start) &&
    !hasExponentPart(end) &&
    (rangeAffixAlwaysShown(xm, a.last + 1, xm.length) ||
      rangeAffixAlwaysShown(ym, 0, b.first)) &&
    rangePartsMatch(xm, 0, a.first, ym, 0, b.first) &&
    rangePartsMatch(xm, a.last + 1, xm.length, ym, b.last + 1, ym.length) &&
    rangeTextCodePoints(xm, 0, a.first) +
      rangeTextCodePoints(xm, a.last + 1, xm.length) >
      1;
  // 5. Add a space at each end of separator that has none when the start
  //    side repeats an affix, at either level, or either side has an
  //    exponent, which is never collapsed. These are ICU4C's conditions: an
  //    affix only the end side has adds none ("5–-5"), and an exponent does
  //    ("3E3 – 5E3", and ar-EG's 0.001 to 0.005 in scientific notation). A
  //    start side without a number is all affix. The middle level never
  //    collapses when the outer one does not.
  var outerAffixes = x.outerPrefix.length > 0 || x.outerSuffix.length > 0;
  var middleAffixes =
    a === undefined || a.first > 0 || a.last + 1 < xm.length;
  if (
    (!outer && outerAffixes) ||
    (!middle && middleAffixes) ||
    hasExponentPart(start) ||
    hasExponentPart(end)
  )
    separator = spaceRangeSeparator(separator);
  var collapsed = [];
  // 6. If outer is false, return start, the separator and end.
  if (!outer) {
    appendRangeParts(collapsed, start, 0, start.length, undefined);
    appendRangePart(collapsed, 'literal', separator, 'shared');
    appendRangeParts(collapsed, end, 0, end.length, undefined);
    return collapsed;
  }
  // 7. Otherwise return x's outer prefix, shared; x's middle prefix,
  //    shared if middle is true; x's numbers; unless middle is true, x's
  //    middle suffix; the separator; unless middle is true, y's middle
  //    prefix; y's numbers; y's middle suffix, shared if middle is true;
  //    and y's outer suffix, shared. A collapsed level keeps the start's
  //    prefix and the end's suffix.
  var middleSource = middle ? 'shared' : undefined;
  appendRangeParts(
    collapsed,
    x.outerPrefix,
    0,
    x.outerPrefix.length,
    'shared',
  );
  appendRangeParts(collapsed, xm, 0, a.first, middleSource);
  appendRangeParts(collapsed, xm, a.first, a.last + 1, undefined);
  if (!middle)
    appendRangeParts(collapsed, xm, a.last + 1, xm.length, undefined);
  appendRangePart(collapsed, 'literal', separator, 'shared');
  if (!middle) appendRangeParts(collapsed, ym, 0, b.first, undefined);
  appendRangeParts(collapsed, ym, b.first, b.last + 1, undefined);
  appendRangeParts(collapsed, ym, b.last + 1, ym.length, middleSource);
  appendRangeParts(
    collapsed,
    y.outerSuffix,
    0,
    y.outerSuffix.length,
    'shared',
  );
  return collapsed;
}

// FormatNumericRange ( numberFormat, x, y )
function FormatNumericRange(numberFormat, x, y) {
  // 1. Let parts be ? PartitionNumberRangePattern(numberFormat, x, y).
  var parts = PartitionNumberRangePattern(numberFormat, x, y);
  // 2. Let result be the empty String.
  // 3. For each element part of parts, do
  //    a. Set result to the string-concatenation of result and
  //       part.[[Value]].
  // 4. Return result.
  return partsValue(parts);
}

// FormatNumericRangeToParts ( numberFormat, x, y )
function FormatNumericRangeToParts(numberFormat, x, y) {
  // 1. Let parts be ? PartitionNumberRangePattern(numberFormat, x, y).
  var parts = PartitionNumberRangePattern(numberFormat, x, y);
  // 2. Let result be ! ArrayCreate(0).
  var result = [];
  // 3. Let n be 0.
  // 4. For each element part of parts, do
  for (var i = 0; i < parts.length; i++) {
    // a. Let partObj be OrdinaryObjectCreate(%Object.prototype%).
    var partObj = {};
    // b. Perform ! CreateDataPropertyOrThrow(partObj, "type",
    //    part.[[Type]]).
    CreateDataPropertyOrThrow(partObj, 'type', parts[i].type);
    // c. Perform ! CreateDataPropertyOrThrow(partObj, "value",
    //    part.[[Value]]).
    CreateDataPropertyOrThrow(partObj, 'value', parts[i].value);
    // d. Perform ! CreateDataPropertyOrThrow(partObj, "source",
    //    part.[[Source]]).
    CreateDataPropertyOrThrow(partObj, 'source', parts[i].source);
    // e. Perform ! CreateDataPropertyOrThrow(result, ! ToString(𝔽(n)),
    //    partObj).
    // f. Set n to n + 1.
    listPush(result, partObj);
  }
  // 5. Return result.
  return result;
}
