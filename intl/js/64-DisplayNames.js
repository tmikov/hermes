/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// ECMA-402 "DisplayNames Objects": the Intl.DisplayNames constructor, its
// supportedLocalesOf, the prototype's resolvedOptions, of and
// @@toStringTag, CanonicalCodeForDisplayNames and IsValidDateTimeFieldCode,
// following the draft https://tc39.es/ecma402/ dated October 24, 2026. An
// instance has no native state. Its [[Fields]] is what the native lookups
// read: of makes one native.displayName call per name, and composes the
// name of a language code from the names of its parts by UTS 35's
// algorithm, with the locale's patterns from native.localeDisplayPatterns
// (Phase 5 plan decision 5).

/// %Intl.DisplayNames.prototype%; set by installDisplayNames.
var DisplayNamesPrototype;

/// The internal slots of an Intl.DisplayNames instance ("Properties of
/// Intl.DisplayNames Instances"), as private fields of the object passed to
/// the constructor. [[InitializedDisplayNames]] is the presence of #locale.
/// [[Fields]] is not a field: the record from code to name that it holds is
/// what the native lookups give for the language identifier of [[Locale]]
/// (#languageId), [[Type]], [[Style]] and [[LanguageDisplay]] (decision 5).
class DisplayNamesSlots extends ReturnOverride {
  /// [[Locale]]
  #locale;
  /// [[Style]]
  #style;
  /// [[Type]]
  #type;
  /// [[Fallback]]
  #fallback;
  /// [[LanguageDisplay]]; undefined unless [[Type]] is "language".
  #languageDisplay;
  /// The language identifier of [[Locale]], which the native lookups that
  /// stand in for [[Fields]] take.
  #languageId;

  /// Attach the slots to o, from the null-prototype record slots whose
  /// fields are named after the slots.
  constructor(o, slots) {
    // 1. Add the private fields to o (ReturnOverride returns o).
    super(o);
    // 2. Set each slot from the field of the same name.
    this.#locale = slots.locale;
    this.#style = slots.style;
    this.#type = slots.type;
    this.#fallback = slots.fallback;
    this.#languageDisplay = slots.languageDisplay;
    this.#languageId = slots.languageId;
  }

  /// Whether o is an Object with an [[InitializedDisplayNames]] internal
  /// slot.
  static is(o) {
    // 1. If o is not an Object, return false.
    if (!isObject(o)) return false;
    // 2. Return whether o has the private fields.
    return #locale in o;
  }

  /// The slot name of o, after ECMA-262 RequireInternalSlot(o,
  /// [[InitializedDisplayNames]]). name is the slot's name in
  /// lowerCamelCase, or "languageId".
  static get(o, name) {
    // 1. Perform ? RequireInternalSlot(o, [[InitializedDisplayNames]]).
    requireInitializedDisplayNames(o);
    // 2. Return the slot.
    switch (name) {
      case 'locale':
        return o.#locale;
      case 'style':
        return o.#style;
      case 'type':
        return o.#type;
      case 'fallback':
        return o.#fallback;
      case 'languageDisplay':
        return o.#languageDisplay;
      case 'languageId':
        return o.#languageId;
    }
    throw new _TypeError('Intl internal error: no DisplayNames slot ' + name);
  }
}

/// ECMA-262 RequireInternalSlot(o, [[InitializedDisplayNames]]).
function requireInitializedDisplayNames(o) {
  // 1. If o is not an Object, or has no [[InitializedDisplayNames]]
  //    internal slot, throw a TypeError exception.
  if (!DisplayNamesSlots.is(o))
    throw new _TypeError('Receiver is not an Intl.DisplayNames object');
}

/// The one record of %Intl.DisplayNames%.[[LocaleData]].[[<locale>]] that
/// ResolveLocale sees: there are no relevant extension keys, so it reads no
/// field of it. Its [[types]] are the native lookups (decision 5).
var DISPLAY_NAMES_LOCALE_DATA = {__proto__: null};

/// %Intl.DisplayNames%.[[LocaleData]].[[<locale>]] for an available locale.
function displayNamesLocaleData(locale) {
  // 1. Return the empty record.
  return DISPLAY_NAMES_LOCALE_DATA;
}

/// The fields of %Intl.DisplayNames% that ResolveOptions reads ("Internal
/// slots"): [[AvailableLocales]], the plugin's one list,
/// [[RelevantExtensionKeys]] « » and [[ResolutionOptionDescriptors]] « ».
var DISPLAY_NAMES_SERVICE = {
  __proto__: null,
  availableLocales: isAvailableLocale,
  relevantExtensionKeys: [],
  resolutionOptionDescriptors: [],
};

/// The Code column of Table "Codes For Date Time Field of DisplayNames", in
/// table order.
var DATE_TIME_FIELD_CODES = [
  'era',
  'year',
  'quarter',
  'month',
  'weekOfYear',
  'weekday',
  'day',
  'dayPeriod',
  'hour',
  'minute',
  'second',
  'timeZoneName',
];

/// The cache of partitioned native.localeDisplayPatterns results, keyed by
/// the language identifier.
var localeDisplayPatternsCache = newDataCache();

/// PartitionPattern(pattern) for a locale display pattern in CLDR form,
/// after checking that its only placeholders are {0} and {1}.
function partitionDisplayPattern(pattern) {
  // 1. Let patternParts be PartitionPattern(pattern).
  var patternParts = PartitionPattern(pattern);
  // 2. If a placeholder is not "0" or "1", throw an internal error.
  for (var i = 0; i < patternParts.length; i++) {
    var type = patternParts[i].type;
    if (type !== 'literal' && type !== '0' && type !== '1')
      throw new _TypeError('Intl internal error: bad pattern ' + pattern);
  }
  // 3. Return patternParts.
  return patternParts;
}

/// native.localeDisplayPatterns(languageId), partitioned, through its
/// cache: the List « PartitionPattern(localePattern),
/// PartitionPattern(localeSeparator) » of the language identifier
/// languageId, from CLDR's "{0} ({1})" and "{0}, {1}". The list is shared
/// and never modified.
function cachedLocaleDisplayPatterns(languageId) {
  // 1. If the cache has the list, return it.
  var patterns = dataCacheGet(localeDisplayPatternsCache, languageId);
  if (patterns !== undefined) return patterns;
  // 2. Otherwise ask native code and partition both patterns.
  var strings = native.localeDisplayPatterns(languageId);
  patterns = [];
  listPush(patterns, partitionDisplayPattern(strings[0]));
  listPush(patterns, partitionDisplayPattern(strings[1]));
  // 3. Cache the list and return it.
  dataCachePut(localeDisplayPatternsCache, languageId, patterns);
  return patterns;
}

/// The partitioned locale display pattern patternParts (from
/// partitionDisplayPattern) with {0} replaced by a and {1} by b.
function formatDisplayPattern(patternParts, a, b) {
  // 1. Let result be the empty String.
  var result = '';
  // 2. For each Record { [[Type]], [[Value]] } patternPart of patternParts,
  //    append to result its literal text, a for "0" or b for "1".
  for (var i = 0; i < patternParts.length; i++) {
    var patternPart = patternParts[i];
    if (patternPart.type === 'literal') result += patternPart.value;
    else if (patternPart.type === '0') result += a;
    else result += b;
  }
  // 3. Return result.
  return result;
}

/// The name name as a qualifier of a language name (UTS 35 Part 2,
/// "Locale Display Name Algorithm"): with "(" and ")" replaced by "[" and
/// "]", and the fullwidth "（" and "）" by "［" and "］".
function displayNameQualifier(name) {
  // 1. Let result be the empty String, and start be 0.
  var result = '';
  var start = 0;
  // 2. For each code unit c of name, at index i, do
  for (var i = 0; i < name.length; i++) {
    // a. Let replacement be the bracket that replaces c, or skip c if it
    //    is not a parenthesis.
    var c = charCodeAt(name, i);
    var replacement;
    if (c === 0x28) replacement = '[';
    else if (c === 0x29) replacement = ']';
    else if (c === 0xff08) replacement = '［';
    else if (c === 0xff09) replacement = '］';
    else continue;
    // b. Append the code units from start to i, and replacement, to result.
    result += stringSlice(name, start, i) + replacement;
    // c. Set start to i + 1.
    start = i + 1;
  }
  // 3. Return result and the code units from start.
  return result + stringSlice(name, start);
}

/// The name of the canonical language code code (the result of
/// CanonicalCodeForDisplayNames) for the language identifier languageId,
/// the style style and the language display languageDisplay, or undefined
/// if code has no name: the field [[<code>]] of [[Fields]]. The name is
/// composed by UTS 35's algorithm (Part 2, "Locale Display Name
/// Algorithm"), as decision 5 says.
function languageDisplayName(languageId, code, style, languageDisplay) {
  // 1. Split code into its language, script, region and variant subtags.
  //    code is a canonical unicode_language_id that starts with a language
  //    subtag (IsWellFormedLanguageTag rejects the others).
  var subtags = StringSplitToList(code, '-');
  var language = subtags[0];
  var k = 1;
  var script;
  if (k < subtags.length && isUnicodeScriptSubtag(subtags[k])) {
    script = subtags[k];
    k++;
  }
  var region;
  if (k < subtags.length && isUnicodeRegionSubtag(subtags[k])) {
    region = subtags[k];
    k++;
  }
  // 2. If languageDisplay is "dialect" and code has exactly one of a script
  //    and a region, let name be the dialect name of language-script or
  //    language-region; on a hit, that subtag is used up. With both, or
  //    "standard", there is no dialect lookup.
  var name;
  if (
    languageDisplay === 'dialect' &&
    (script === undefined) !== (region === undefined)
  ) {
    name = native.displayName(
      languageId,
      'dialect',
      language + '-' + (script !== undefined ? script : region),
      style,
    );
    if (name !== undefined) {
      script = undefined;
      region = undefined;
    }
  }
  // 3. Otherwise let name be the name of language.
  if (name === undefined)
    name = native.displayName(languageId, 'language', language, style);
  // 4. If there is no name, code has no name.
  if (name === undefined) return undefined;
  // 5. Let qualifiers be the names of the script, the region and each
  //    variant, in this order and in style style, as qualifiers. If one has
  //    no name, code has no name.
  var qualifiers = [];
  var qualifier;
  if (script !== undefined) {
    qualifier = native.displayName(languageId, 'script', script, style);
    if (qualifier === undefined) return undefined;
    listPush(qualifiers, displayNameQualifier(qualifier));
  }
  if (region !== undefined) {
    qualifier = native.displayName(languageId, 'region', region, style);
    if (qualifier === undefined) return undefined;
    listPush(qualifiers, displayNameQualifier(qualifier));
  }
  for (; k < subtags.length; k++) {
    qualifier = native.displayName(languageId, 'variant', subtags[k], style);
    if (qualifier === undefined) return undefined;
    listPush(qualifiers, displayNameQualifier(qualifier));
  }
  // 6. If qualifiers is empty, return name.
  if (qualifiers.length === 0) return name;
  // 7. Join the qualifiers with the locale's localeSeparator, and name and
  //    them with its localePattern.
  var patterns = cachedLocaleDisplayPatterns(languageId);
  var joined = qualifiers[0];
  for (var i = 1; i < qualifiers.length; i++)
    joined = formatDisplayPattern(patterns[1], joined, qualifiers[i]);
  return formatDisplayPattern(patterns[0], name, joined);
}

/// The field [[<code>]] of displayNames.[[Fields]] for the String code that
/// CanonicalCodeForDisplayNames returned, or undefined if [[Fields]] has no
/// such field.
function displayNamesField(displayNames, code) {
  // 1. Let languageId, type and style be the language identifier of
  //    displayNames.[[Locale]], displayNames.[[Type]] and
  //    displayNames.[[Style]].
  var languageId = DisplayNamesSlots.get(displayNames, 'languageId');
  var type = DisplayNamesSlots.get(displayNames, 'type');
  var style = DisplayNamesSlots.get(displayNames, 'style');
  // 2. If type is "language", return the composed name of code.
  if (type === 'language')
    return languageDisplayName(
      languageId,
      code,
      style,
      DisplayNamesSlots.get(displayNames, 'languageDisplay'),
    );
  // 3. Return the native name of code; the native kinds have the names of
  //    the other types.
  return native.displayName(languageId, type, code, style);
}

/// Steps 3 to 23 of Intl.DisplayNames ( locales, options ), on the object
/// displayNames that steps 1 and 2 made. The slots are collected in a
/// null-prototype record and attached at the end, once every value is
/// known; nothing between can observe them. Returns displayNames.
function initializeDisplayNames(displayNames, locales, options) {
  var slots = {__proto__: null};
  // 3. Let optionsResolution be ? ResolveOptions(%Intl.DisplayNames%,
  //    %Intl.DisplayNames%.[[LocaleData]], locales, options,
  //    « require-options »).
  var optionsResolution = ResolveOptions(
    DISPLAY_NAMES_SERVICE,
    displayNamesLocaleData,
    locales,
    options,
    ['require-options'],
  );
  // 4. Set options to optionsResolution.[[Options]].
  options = optionsResolution.options;
  // 5. Let resolvedLocale be optionsResolution.[[ResolvedLocale]].
  var resolvedLocale = optionsResolution.resolvedLocale;
  // 6. Let style be ? GetOption(options, "style", string, « "narrow",
  //    "short", "long" », "long").
  var style = GetOption(
    options,
    'style',
    'string',
    ['narrow', 'short', 'long'],
    'long',
  );
  // 7. Set displayNames.[[Style]] to style.
  slots.style = style;
  // 8. Let type be ? GetOption(options, "type", string, « "language",
  //    "region", "script", "currency", "calendar", "dateTimeField" »,
  //    required).
  var type = GetOption(
    options,
    'type',
    'string',
    ['language', 'region', 'script', 'currency', 'calendar', 'dateTimeField'],
    REQUIRED,
  );
  // 9. Set displayNames.[[Type]] to type.
  slots.type = type;
  // 10. Let fallback be ? GetOption(options, "fallback", string, « "code",
  //     "none" », "code").
  var fallback = GetOption(
    options,
    'fallback',
    'string',
    ['code', 'none'],
    'code',
  );
  // 11. Set displayNames.[[Fallback]] to fallback.
  slots.fallback = fallback;
  // 12. Set displayNames.[[Locale]] to resolvedLocale.[[Locale]].
  slots.locale = resolvedLocale.locale;
  // 13. Let resolvedLocaleData be resolvedLocale.[[LocaleData]].
  // 14. Let types be resolvedLocaleData.[[types]].
  // 15. Assert: types is a Record.
  //     types is the native lookups for the language identifier of
  //     [[Locale]] (decision 5).
  slots.languageId = GetLocaleBaseName(slots.locale);
  // 16. Let languageDisplay be ? GetOption(options, "languageDisplay",
  //     string, « "dialect", "standard" », "dialect").
  var languageDisplay = GetOption(
    options,
    'languageDisplay',
    'string',
    ['dialect', 'standard'],
    'dialect',
  );
  // 17. Let typeFields be types.[[<type>]].
  // 18. Assert: typeFields is a Record.
  // 19. If type is "language", then
  //     a. Set displayNames.[[LanguageDisplay]] to languageDisplay.
  //     b. Set typeFields to typeFields.[[<languageDisplay>]].
  //     c. Assert: typeFields is a Record.
  slots.languageDisplay = type === 'language' ? languageDisplay : undefined;
  // 20. Let styleFields be typeFields.[[<style>]].
  // 21. Assert: styleFields is a Record.
  // 22. Set displayNames.[[Fields]] to styleFields.
  //     [[Fields]] is the native lookups for the language identifier,
  //     [[Type]], [[Style]] and [[LanguageDisplay]] (displayNamesField).
  new DisplayNamesSlots(displayNames, slots);
  // 23. Return displayNames.
  return displayNames;
}

// Intl.DisplayNames ( locales, options )
function DisplayNames(locales, options) {
  // 1. If NewTarget is undefined, throw a TypeError exception.
  if (new.target === undefined)
    throw new _TypeError('Intl.DisplayNames must be called with new');
  // 2. Let displayNames be ? OrdinaryCreateFromConstructor(NewTarget,
  //    "%Intl.DisplayNames.prototype%", « [[InitializedDisplayNames]],
  //    [[Locale]], [[Style]], [[Type]], [[Fallback]], [[LanguageDisplay]],
  //    [[Fields]] »).
  //    Hermes created this from NewTarget before the call (design spec
  //    "Constructors"); initializeDisplayNames attaches the slots.
  // 3-23. The steps that resolve the slots, numbered there.
  return initializeDisplayNames(this, locales, options);
}

/// Whether the String code is matched by the unicode_language_id Unicode
/// locale nonterminal (UTS 35 3.2), with "-" as the separator: "_", which
/// the grammar also allows, is the backwards compatibility syntax that
/// IsWellFormedLanguageTag rejects anyway.
function isUnicodeLanguageId(code) {
  // 1. Let subtags be StringSplitToList(code, "-"), and n its length.
  var subtags = StringSplitToList(code, '-');
  var n = subtags.length;
  // 2. Skip a unicode_language_subtag followed by an optional
  //    unicode_script_subtag, or a unicode_script_subtag alone (which
  //    covers "root"); otherwise return false.
  var k;
  if (isUnicodeLanguageSubtag(subtags[0])) {
    k = 1;
    if (k < n && isUnicodeScriptSubtag(subtags[k])) k++;
  } else if (isUnicodeScriptSubtag(subtags[0])) {
    k = 1;
  } else {
    return false;
  }
  // 3. Skip an optional unicode_region_subtag.
  if (k < n && isUnicodeRegionSubtag(subtags[k])) k++;
  // 4. Skip any unicode_variant_subtags.
  while (k < n && isUnicodeVariantSubtag(subtags[k])) k++;
  // 5. Return whether every subtag was skipped.
  return k === n;
}

// CanonicalCodeForDisplayNames ( type, code )
function CanonicalCodeForDisplayNames(type, code) {
  // 1. If type is "language", then
  if (type === 'language') {
    // a. If code cannot be matched by the unicode_language_id Unicode
    //    locale nonterminal, throw a RangeError exception.
    if (!isUnicodeLanguageId(code))
      throw new _RangeError('Invalid language code: ' + code);
    // b. If IsWellFormedLanguageTag(code) is false, throw a RangeError
    //    exception.
    // c. Return CanonicalizeUnicodeLocaleId(code).
    //    canonicalizeTag does both: it returns undefined for a tag that is
    //    not well-formed.
    var canonical = canonicalizeTag(code);
    if (canonical === undefined)
      throw new _RangeError('Invalid language code: ' + code);
    return canonical;
  }
  // 2. If type is "region", then
  if (type === 'region') {
    // a. If code cannot be matched by the unicode_region_subtag Unicode
    //    locale nonterminal, throw a RangeError exception.
    if (!isUnicodeRegionSubtag(code))
      throw new _RangeError('Invalid region code: ' + code);
    // b. Return the ASCII-uppercase of code.
    return asciiUppercase(code);
  }
  // 3. If type is "script", then
  if (type === 'script') {
    // a. If code cannot be matched by the unicode_script_subtag Unicode
    //    locale nonterminal, throw a RangeError exception.
    if (!isUnicodeScriptSubtag(code))
      throw new _RangeError('Invalid script code: ' + code);
    // b. Assert: The length of code is 4, and every code unit of code
    //    represents an ASCII letter.
    // c. Let first be the ASCII-uppercase of the substring of code from 0
    //    to 1.
    var first = asciiUppercase(stringSlice(code, 0, 1));
    // d. Let rest be the ASCII-lowercase of the substring of code from 1.
    var rest = asciiLowercase(stringSlice(code, 1));
    // e. Return the string-concatenation of first and rest.
    return first + rest;
  }
  // 4. If type is "calendar", then
  if (type === 'calendar') {
    // a. If code cannot be matched by the type Unicode locale nonterminal,
    //    throw a RangeError exception.
    // b. If code uses any of the backwards compatibility syntax described
    //    in Unicode Technical Standard #35 Part 1 Core, Section 3.3 BCP 47
    //    Conformance, throw a RangeError exception.
    //    For a type, that syntax is "_" as a separator, which isUnicodeType
    //    already rejects.
    if (!isUnicodeType(code))
      throw new _RangeError('Invalid calendar code: ' + code);
    // c. Return the ASCII-lowercase of code.
    return asciiLowercase(code);
  }
  // 5. If type is "dateTimeField", then
  if (type === 'dateTimeField') {
    // a. If the result of IsValidDateTimeFieldCode(code) is false, throw a
    //    RangeError exception.
    if (!IsValidDateTimeFieldCode(code))
      throw new _RangeError('Invalid dateTimeField code: ' + code);
    // b. Return code.
    return code;
  }
  // 6. Assert: type is "currency".
  // 7. If IsWellFormedCurrencyCode(code) is false, throw a RangeError
  //    exception.
  if (!IsWellFormedCurrencyCode(code))
    throw new _RangeError('Invalid currency code: ' + code);
  // 8. Return the ASCII-uppercase of code.
  return asciiUppercase(code);
}

// IsValidDateTimeFieldCode ( field )
function IsValidDateTimeFieldCode(field) {
  // 1. If field is listed in the Code column of Table "Codes For Date Time
  //    Field of DisplayNames", return true.
  // 2. Return false.
  return listContains(DATE_TIME_FIELD_CODES, field);
}

/// The rows of the table "Resolved Options of DisplayNames Instances", in
/// table order: [Property, slot name].
var DISPLAY_NAMES_RESOLVED_OPTIONS = [
  ['locale', 'locale'],
  ['style', 'style'],
  ['type', 'type'],
  ['fallback', 'fallback'],
  ['languageDisplay', 'languageDisplay'],
];

/// Install Intl.DisplayNames on intl; called by installIntl.
function installDisplayNames(intl) {
  // 1. Let proto be %Intl.DisplayNames.prototype%, an ordinary object whose
  //    [[Prototype]] is %Object.prototype% ("Properties of the
  //    Intl.DisplayNames Prototype Object").
  var proto = {};
  DisplayNamesPrototype = proto;
  // 2. "Properties of the Intl.DisplayNames Constructor":
  //    Intl.DisplayNames.prototype, with its length 2 (Intl.DisplayNames (
  //    locales, options )), and Intl.DisplayNames.prototype.constructor.
  defineConstructor(DisplayNames, 2, proto);
  // 3. The method properties of the constructor.
  defineMethods(DisplayNames, {
    // Intl.DisplayNames.supportedLocalesOf ( locales [ , options ] )
    supportedLocalesOf(locales, options = undefined) {
      // 1. Let availableLocales be %Intl.DisplayNames%.[[AvailableLocales]].
      // 2. Let requestedLocales be ? CanonicalizeLocaleList(locales).
      var requestedLocales = CanonicalizeLocaleList(locales);
      // 3. Return ? FilterLocales(availableLocales, requestedLocales,
      //    options).
      return FilterLocales(isAvailableLocale, requestedLocales, options);
    },
  });
  // 4. The method properties of the prototype, in the draft's order.
  defineMethods(proto, {
    // Intl.DisplayNames.prototype.resolvedOptions ( )
    resolvedOptions() {
      // 1. Let displayNames be this value.
      var displayNames = this;
      // 2. Perform ? RequireInternalSlot(displayNames,
      //    [[InitializedDisplayNames]]).
      requireInitializedDisplayNames(displayNames);
      // 3. Let options be OrdinaryObjectCreate(%Object.prototype%).
      var options = {};
      // 4. For each row of Table "Resolved Options of DisplayNames
      //    Instances", except the header row, in table order, do
      for (var i = 0; i < DISPLAY_NAMES_RESOLVED_OPTIONS.length; i++) {
        var row = DISPLAY_NAMES_RESOLVED_OPTIONS[i];
        // a. Let propertyKey be the Property value of the current row.
        // b. Let value be the value of displayNames's internal slot whose
        //    name is the Internal Slot value of the current row.
        var value = DisplayNamesSlots.get(displayNames, row[1]);
        // c. If value is not undefined, then
        //    i. Perform ! CreateDataPropertyOrThrow(options, propertyKey,
        //       value).
        if (value !== undefined)
          CreateDataPropertyOrThrow(options, row[0], value);
      }
      // 5. Return options.
      return options;
    },
    // Intl.DisplayNames.prototype.of ( code )
    of(code) {
      // 1. Let displayNames be this value.
      var displayNames = this;
      // 2. Perform ? RequireInternalSlot(displayNames,
      //    [[InitializedDisplayNames]]).
      requireInitializedDisplayNames(displayNames);
      // 3. Set code to ? ToString(code).
      code = ToString(code);
      // 4. Set code to ? CanonicalCodeForDisplayNames(displayNames.[[Type]],
      //    code).
      code = CanonicalCodeForDisplayNames(
        DisplayNamesSlots.get(displayNames, 'type'),
        code,
      );
      // 5. Let fields be displayNames.[[Fields]].
      // 6. If fields has a field [[<code>]], return fields.[[<code>]].
      var name = displayNamesField(displayNames, code);
      if (name !== undefined) return name;
      // 7. If displayNames.[[Fallback]] is "code", return code.
      if (DisplayNamesSlots.get(displayNames, 'fallback') === 'code')
        return code;
      // 8. Return undefined.
      return undefined;
    },
  });
  // 5. Intl.DisplayNames.prototype [ %Symbol.toStringTag% ]:
  //    "Intl.DisplayNames", { [[Writable]]: false, [[Enumerable]]: false,
  //    [[Configurable]]: true }.
  setToStringTag(proto, 'Intl.DisplayNames');
  // 6. "The Intl.DisplayNames Constructor": the initial value of the
  //    "DisplayNames" property of the Intl object. ECMA-262 18 gives such
  //    data properties the attributes { [[Writable]]: true,
  //    [[Enumerable]]: false, [[Configurable]]: true }.
  ObjectDefineProperty(intl, 'DisplayNames', {
    __proto__: null,
    value: DisplayNames,
    writable: true,
    enumerable: false,
    configurable: true,
  });
}
