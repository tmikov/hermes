/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// ECMA-402 "Locale Objects": the Intl.Locale constructor, UpdateLanguageId,
// MakeLocaleRecord, the abstract operations of the locale information
// methods and the Intl.Locale prototype, following the draft
// https://tc39.es/ecma402/ dated October 24, 2026. The other Locale abstract
// operations they use are in 05-tags.js.

/// %Intl.Locale%.[[LocaleExtensionKeys]] ("Internal slots" under
/// "Properties of the Intl.Locale Constructor"): the required keys, and "kf"
/// and "kn", which %Intl.Collator%.[[RelevantExtensionKeys]] contains.
var LOCALE_EXTENSION_KEYS = ['ca', 'co', 'fw', 'hc', 'kf', 'kn', 'nu'];

/// The internal slots of an Intl.Locale instance ("Properties of Intl.Locale
/// Instances"), as private fields of the object passed to the constructor.
/// [[InitializedLocale]] is the presence of #locale.
class LocaleSlots extends ReturnOverride {
  /// [[Locale]]
  #locale;
  /// [[Calendar]]
  #calendar;
  /// [[Collation]]
  #collation;
  /// [[FirstDayOfWeek]]
  #firstDayOfWeek;
  /// [[HourCycle]]
  #hourCycle;
  /// [[CaseFirst]]
  #caseFirst;
  /// [[Numeric]]
  #numeric;
  /// [[NumberingSystem]]
  #numberingSystem;

  /// Attach the slots to o, from a null-prototype record whose fields are
  /// named after the slots.
  constructor(o, record) {
    // 1. Add the private fields to o (ReturnOverride returns o).
    super(o);
    // 2. Set each slot from the field of the same name.
    this.#locale = record.locale;
    this.#calendar = record.calendar;
    this.#collation = record.collation;
    this.#firstDayOfWeek = record.firstDayOfWeek;
    this.#hourCycle = record.hourCycle;
    this.#caseFirst = record.caseFirst;
    this.#numeric = record.numeric;
    this.#numberingSystem = record.numberingSystem;
  }

  /// Whether o is an Object with an [[InitializedLocale]] internal slot.
  static is(o) {
    // 1. If o is not an Object, return false.
    if ((typeof o !== 'object' || o === null) && typeof o !== 'function')
      return false;
    // 2. Return whether o has the private fields.
    return #locale in o;
  }

  /// The slot name of o, after ECMA-262 RequireInternalSlot(o,
  /// [[InitializedLocale]]).
  static get(o, name) {
    // 1. Perform ? RequireInternalSlot(o, [[InitializedLocale]]).
    requireInitializedLocale(o);
    // 2. Return the slot.
    switch (name) {
      case 'locale':
        return o.#locale;
      case 'calendar':
        return o.#calendar;
      case 'collation':
        return o.#collation;
      case 'firstDayOfWeek':
        return o.#firstDayOfWeek;
      case 'hourCycle':
        return o.#hourCycle;
      case 'caseFirst':
        return o.#caseFirst;
      case 'numeric':
        return o.#numeric;
      case 'numberingSystem':
        return o.#numberingSystem;
    }
    throw new _TypeError('Intl internal error: no Locale slot ' + name);
  }
}

/// ECMA-262 RequireInternalSlot(o, [[InitializedLocale]]).
function requireInitializedLocale(o) {
  // 1. If o is not an Object, or has no [[InitializedLocale]] internal slot,
  //    throw a TypeError exception.
  if (!LocaleSlots.is(o))
    throw new _TypeError('Receiver is not an Intl.Locale object');
}

/// Whether o is an Object with an [[InitializedLocale]] internal slot; used
/// by CanonicalizeLocaleList.
function isLocaleObject(o) {
  // 1. Return whether o has the Intl.Locale private fields.
  return LocaleSlots.is(o);
}

/// o.[[Locale]] for an Intl.Locale object o.
function localeObjectTag(o) {
  // 1. Return o.[[Locale]].
  return LocaleSlots.get(o, 'locale');
}

/// CanonicalizeUnicodeLocaleId(tag) for a tag known to be well-formed.
function canonicalizeWellFormed(tag) {
  // 1. Let canonical be the native canonicalization of tag.
  var canonical = native.canonicalizeLocale(tag);
  // 2. Assert: canonical is not undefined, because tag is well-formed.
  if (canonical === undefined)
    throw new _TypeError('Intl internal error: ill-formed locale ' + tag);
  // 3. Return canonical.
  return canonical;
}

// Intl.Locale ( tag [ , options ] )
function Locale(tag, options) {
  // 1. If NewTarget is undefined, throw a TypeError exception.
  if (new.target === undefined)
    throw new _TypeError('Intl.Locale must be called with new');
  // 2. Let localeExtensionKeys be %Intl.Locale%.[[LocaleExtensionKeys]].
  var localeExtensionKeys = LOCALE_EXTENSION_KEYS;
  // 3. Let internalSlotsList be « [[InitializedLocale]], [[Locale]],
  //    [[Calendar]], [[Collation]], [[FirstDayOfWeek]], [[HourCycle]],
  //    [[NumberingSystem]] ».
  // 4. If localeExtensionKeys contains "kf", then
  //    a. Append [[CaseFirst]] to internalSlotsList.
  // 5. If localeExtensionKeys contains "kn", then
  //    a. Append [[Numeric]] to internalSlotsList.
  //    LocaleSlots has all of them, since localeExtensionKeys contains both.
  // 6. Let locale be ? OrdinaryCreateFromConstructor(NewTarget,
  //    "%Intl.Locale.prototype%", internalSlotsList).
  //    Hermes created this from NewTarget before the call (design spec
  //    "Constructors"); steps 36 to 43 attach the slots.
  var locale = this;
  // 7. If tag is not a String and tag is not an Object, throw a TypeError
  //    exception.
  var type = typeof tag;
  if (
    type !== 'string' &&
    (tag === null || (type !== 'object' && type !== 'function'))
  ) {
    throw new _TypeError('Intl.Locale tag must be a string or object');
  }
  if (isLocaleObject(tag)) {
    // 8. If tag is an Object and tag has an [[InitializedLocale]] internal
    //    slot, then
    //    a. Set tag to tag.[[Locale]].
    tag = localeObjectTag(tag);
  } else {
    // 9. Else,
    //    a. Set tag to ? ToString(tag).
    tag = ToString(tag);
  }
  // 10. Set options to ? CoerceOptionsToObject(options).
  options = CoerceOptionsToObject(options);
  // 11. If IsWellFormedLanguageTag(tag) is false, throw a RangeError
  //     exception.
  // 12. NOTE: Because LanguageId canonicalization can alter tag in arbitrary
  //     ways according to Alias Rules from supplementalMetadata.xml, it is
  //     necessary to perform such canonicalization before applying overrides
  //     from options.
  // 13. Set tag to CanonicalizeUnicodeLocaleId(tag).
  //     The native canonicalizer does both: undefined means ill-formed.
  var canonical = native.canonicalizeLocale(tag);
  if (canonical === undefined)
    throw new _RangeError('Incorrect locale information provided');
  tag = canonical;
  // 14. Set tag to ? UpdateLanguageId(tag, options).
  tag = UpdateLanguageId(tag, options);
  // 15. Let opt be a new Record.
  var opt = {__proto__: null};
  // 16. Let calendar be ? GetOption(options, "calendar", string, empty,
  //     undefined).
  var calendar = GetOption(options, 'calendar', 'string', undefined, undefined);
  // 17. If calendar is not undefined, then
  //     a. If calendar cannot be matched by the type Unicode locale
  //        nonterminal, throw a RangeError exception.
  if (calendar !== undefined && !isUnicodeType(calendar))
    throw new _RangeError('Invalid calendar: ' + calendar);
  // 18. Set opt.[[ca]] to calendar.
  opt.ca = calendar;
  // 19. Let collation be ? GetOption(options, "collation", string, empty,
  //     undefined).
  var collation = GetOption(
    options,
    'collation',
    'string',
    undefined,
    undefined,
  );
  // 20. If collation is not undefined, then
  //     a. If collation cannot be matched by the type Unicode locale
  //        nonterminal, throw a RangeError exception.
  if (collation !== undefined && !isUnicodeType(collation))
    throw new _RangeError('Invalid collation: ' + collation);
  // 21. Set opt.[[co]] to collation.
  opt.co = collation;
  // 22. Let fw be ? GetOption(options, "firstDayOfWeek", string, empty,
  //     undefined).
  var fw = GetOption(options, 'firstDayOfWeek', 'string', undefined, undefined);
  // 23. If fw is not undefined, then
  if (fw !== undefined) {
    // a. Set fw to WeekdayToUValue(fw).
    fw = WeekdayToUValue(fw);
    // b. If fw cannot be matched by the type Unicode locale nonterminal,
    //    throw a RangeError exception.
    if (!isUnicodeType(fw))
      throw new _RangeError('Invalid firstDayOfWeek: ' + fw);
  }
  // 24. Set opt.[[fw]] to fw.
  opt.fw = fw;
  // 25. Let hc be ? GetOption(options, "hourCycle", string, « "h11", "h12",
  //     "h23", "h24" », undefined).
  // 26. Set opt.[[hc]] to hc.
  opt.hc = GetOption(
    options,
    'hourCycle',
    'string',
    ['h11', 'h12', 'h23', 'h24'],
    undefined,
  );
  // 27. Let kf be ? GetOption(options, "caseFirst", string, « "upper",
  //     "lower", "false" », undefined).
  // 28. Set opt.[[kf]] to kf.
  opt.kf = GetOption(
    options,
    'caseFirst',
    'string',
    ['upper', 'lower', 'false'],
    undefined,
  );
  // 29. Let kn be ? GetOption(options, "numeric", boolean, empty, undefined).
  var kn = GetOption(options, 'numeric', 'boolean', undefined, undefined);
  // 30. If kn is not undefined, set kn to ! ToString(kn).
  if (kn !== undefined) kn = ToString(kn);
  // 31. Set opt.[[kn]] to kn.
  opt.kn = kn;
  // 32. Let numberingSystem be ? GetOption(options, "numberingSystem",
  //     string, empty, undefined).
  var numberingSystem = GetOption(
    options,
    'numberingSystem',
    'string',
    undefined,
    undefined,
  );
  // 33. If numberingSystem is not undefined, then
  //     a. If numberingSystem cannot be matched by the type Unicode locale
  //        nonterminal, throw a RangeError exception.
  if (numberingSystem !== undefined && !isUnicodeType(numberingSystem))
    throw new _RangeError('Invalid numberingSystem: ' + numberingSystem);
  // 34. Set opt.[[nu]] to numberingSystem.
  opt.nu = numberingSystem;
  // 35. Let localeRecord be MakeLocaleRecord(tag, opt, localeExtensionKeys).
  var localeRecord = MakeLocaleRecord(tag, opt, localeExtensionKeys);
  // 36. Set locale.[[Locale]] to localeRecord.[[locale]].
  // 37. Set locale.[[Calendar]] to localeRecord.[[ca]].
  // 38. Set locale.[[Collation]] to localeRecord.[[co]].
  // 39. Set locale.[[FirstDayOfWeek]] to localeRecord.[[fw]].
  // 40. Set locale.[[HourCycle]] to localeRecord.[[hc]].
  // 41. If localeExtensionKeys contains "kf", then
  //     a. Set locale.[[CaseFirst]] to localeRecord.[[kf]].
  // 42. If localeExtensionKeys contains "kn", then
  //     a. If localeRecord.[[kn]] is either "true" or the empty String, then
  //        i. Set locale.[[Numeric]] to true.
  //     b. Else,
  //        i. Set locale.[[Numeric]] to false.
  // 43. Set locale.[[NumberingSystem]] to localeRecord.[[nu]].
  new LocaleSlots(locale, {
    __proto__: null,
    locale: localeRecord.locale,
    calendar: localeRecord.ca,
    collation: localeRecord.co,
    firstDayOfWeek: localeRecord.fw,
    hourCycle: localeRecord.hc,
    caseFirst: localeRecord.kf,
    numeric: localeRecord.kn === 'true' || localeRecord.kn === '',
    numberingSystem: localeRecord.nu,
  });
  // 44. Return locale.
  return locale;
}

// UpdateLanguageId ( tag, options )
function UpdateLanguageId(tag, options) {
  // 1. Let baseName be GetLocaleBaseName(tag).
  var baseName = GetLocaleBaseName(tag);
  // 2. Let language be ? GetOption(options, "language", string, empty,
  //    GetLocaleLanguage(baseName)).
  var language = GetOption(
    options,
    'language',
    'string',
    undefined,
    GetLocaleLanguage(baseName),
  );
  // 3. If language cannot be matched by the unicode_language_subtag Unicode
  //    locale nonterminal, throw a RangeError exception.
  if (!isUnicodeLanguageSubtag(language))
    throw new _RangeError('Invalid language: ' + language);
  // 4. Let script be ? GetOption(options, "script", string, empty,
  //    GetLocaleScript(baseName)).
  var script = GetOption(
    options,
    'script',
    'string',
    undefined,
    GetLocaleScript(baseName),
  );
  // 5. If script is not undefined, then
  //    a. If script cannot be matched by the unicode_script_subtag Unicode
  //       locale nonterminal, throw a RangeError exception.
  if (script !== undefined && !isUnicodeScriptSubtag(script))
    throw new _RangeError('Invalid script: ' + script);
  // 6. Let region be ? GetOption(options, "region", string, empty,
  //    GetLocaleRegion(baseName)).
  var region = GetOption(
    options,
    'region',
    'string',
    undefined,
    GetLocaleRegion(baseName),
  );
  // 7. If region is not undefined, then
  //    a. If region cannot be matched by the unicode_region_subtag Unicode
  //       locale nonterminal, throw a RangeError exception.
  if (region !== undefined && !isUnicodeRegionSubtag(region))
    throw new _RangeError('Invalid region: ' + region);
  // 8. Let variants be ? GetOption(options, "variants", string, empty,
  //    GetLocaleVariants(baseName)).
  var variants = GetOption(
    options,
    'variants',
    'string',
    undefined,
    GetLocaleVariants(baseName),
  );
  // 9. If variants is not undefined, then
  if (variants !== undefined) {
    // a. If variants is the empty String, throw a RangeError exception.
    if (variants === '') throw new _RangeError('Invalid variants: ""');
    // b. Let lowerVariants be the ASCII-lowercase of variants.
    var lowerVariants = asciiLowercase(variants);
    // c. Let variantSubtags be StringSplitToList(lowerVariants, "-").
    var variantSubtags = StringSplitToList(lowerVariants, '-');
    // d. For each element variant of variantSubtags, do
    //    i. If variant cannot be matched by the unicode_variant_subtag
    //       Unicode locale nonterminal, throw a RangeError exception.
    // e. If variantSubtags contains any duplicate elements, throw a
    //    RangeError exception.
    //    Both checks are made in one pass: each variant is compared with
    //    the ones before it.
    var seen = [];
    for (var i = 0; i < variantSubtags.length; i++) {
      var variant = variantSubtags[i];
      if (!isUnicodeVariantSubtag(variant) || listContains(seen, variant))
        throw new _RangeError('Invalid variants: ' + variants);
      listPush(seen, variant);
    }
  }
  // 10. Let allExtensions be the suffix of tag following baseName.
  var allExtensions = stringSlice(tag, baseName.length);
  // 11. Let newTag be language.
  var newTag = language;
  // 12. If script is not undefined, set newTag to the string-concatenation
  //     of newTag, "-", and script.
  if (script !== undefined) newTag = newTag + '-' + script;
  // 13. If region is not undefined, set newTag to the string-concatenation
  //     of newTag, "-", and region.
  if (region !== undefined) newTag = newTag + '-' + region;
  // 14. If variants is not undefined, set newTag to the string-concatenation
  //     of newTag, "-", and variants.
  if (variants !== undefined) newTag = newTag + '-' + variants;
  // 15. Set newTag to the string-concatenation of newTag and allExtensions.
  newTag = newTag + allExtensions;
  // 16. Return newTag.
  return newTag;
}

// MakeLocaleRecord ( tag, options, localeExtensionKeys )
function MakeLocaleRecord(tag, options, localeExtensionKeys) {
  // The extensions of tag come from a canonical tag (UpdateLanguageId only
  // replaces the base name), so its Unicode extension is lowercase.
  var parts = splitUnicodeExtension(tag);
  var attributes;
  var keywords;
  if (parts.extension !== undefined) {
    // 1. If tag contains a substring that is a Unicode locale extension
    //    sequence, then
    //    a. Let extension be the String value consisting of the substring of
    //       the Unicode locale extension sequence within tag.
    //    b. Let components be UnicodeExtensionComponents(extension).
    var components = UnicodeExtensionComponents(parts.extension);
    //    c. Let attributes be components.[[Attributes]].
    attributes = components.attributes;
    //    d. Let keywords be components.[[Keywords]].
    keywords = components.keywords;
  } else {
    // 2. Else,
    //    a. Let attributes be a new empty List.
    attributes = [];
    //    b. Let keywords be a new empty List.
    keywords = [];
  }
  // 3. Let result be a new Record.
  var result = {__proto__: null};
  // 4. For each element key of localeExtensionKeys, do
  for (var i = 0; i < localeExtensionKeys.length; i++) {
    var key = localeExtensionKeys[i];
    // a. If keywords contains an element whose [[Key]] is key, then
    //    i. Let entry be the element of keywords whose [[Key]] is key.
    //    ii. Let value be entry.[[Value]].
    // b. Else,
    //    i. Let entry be empty.
    //    ii. Let value be undefined.
    var entry = findKeyword(keywords, key);
    var value = entry === undefined ? undefined : entry.value;
    // c. Assert: options has a field [[<key>]].
    // d. Let overrideValue be options.[[<key>]].
    var overrideValue = options[key];
    // e. If overrideValue is not undefined, then
    if (overrideValue !== undefined) {
      // i. Set value to CanonicalizeUValue(key, overrideValue).
      value = CanonicalizeUValue(key, overrideValue);
      if (entry !== undefined) {
        // ii. If entry is not empty, then
        //     1. Set entry.[[Value]] to value.
        entry.value = value;
      } else {
        // iii. Else,
        //      1. Append the Record { [[Key]]: key, [[Value]]: value } to
        //         keywords.
        listPush(keywords, {__proto__: null, key: key, value: value});
      }
    }
    // f. Set result.[[<key>]] to value.
    result[key] = value;
  }
  // 5. Let locale be the String value that is tag with any Unicode locale
  //    extension sequences removed.
  var locale = parts.base;
  if (attributes.length > 0 || keywords.length > 0) {
    // 6. If attributes is not empty or keywords is not empty, then
    //    a. Set result.[[locale]] to
    //       InsertUnicodeExtensionAndCanonicalize(locale, attributes,
    //       keywords).
    result.locale = InsertUnicodeExtensionAndCanonicalize(
      locale,
      attributes,
      keywords,
    );
  } else {
    // 7. Else,
    //    a. Set result.[[locale]] to CanonicalizeUnicodeLocaleId(locale).
    result.locale = canonicalizeWellFormed(locale);
  }
  // 8. Return result.
  return result;
}

/// ECMA-262 CreateDataPropertyOrThrow(o, key, v) on an ordinary extensible
/// object with no property key, which cannot throw: a writable, enumerable,
/// configurable data property, defined without consulting setters on the
/// prototype chain.
function createDataProperty(o, key, v) {
  // 1. Perform ! DefinePropertyOrThrow(o, key, PropertyDescriptor {
  //    [[Value]]: v, [[Writable]]: true, [[Enumerable]]: true,
  //    [[Configurable]]: true }).
  ObjectDefineProperty(o, key, {
    __proto__: null,
    value: v,
    writable: true,
    enumerable: true,
    configurable: true,
  });
}

// CalendarsOfLocale ( loc )
function CalendarsOfLocale(loc) {
  // 1. If loc.[[Calendar]] is not undefined, then
  //    a. Return CreateArrayFromList(« loc.[[Calendar]] »).
  var calendar = LocaleSlots.get(loc, 'calendar');
  if (calendar !== undefined) return CreateArrayFromList([calendar]);
  // 2. Let preference be RegionPreference(loc.[[Locale]]).
  var preference = RegionPreference(LocaleSlots.get(loc, 'locale'));
  // 3. If preference.[[RegionOverride]] is not undefined, let
  //    preferredRegions be « preference.[[RegionOverride]],
  //    preference.[[Region]] »; else let preferredRegions be
  //    « preference.[[Region]] ».
  var preferredRegions =
    preference.regionOverride !== undefined
      ? [preference.regionOverride, preference.region]
      : [preference.region];
  // 4. Let calendarsInUse be a new empty List.
  var calendarsInUse = [];
  // 5. Let language be GetLocaleLanguage(loc.[[Locale]]).
  //    Only step 6.b uses it, and that step never applies (see there).
  // 6. For each String region of preferredRegions, do
  for (var i = 0; i < preferredRegions.length; i++) {
    var region = preferredRegions[i];
    // a. Let locale be the string-concatenation of language, "-", and
    //    region.
    // b. If calendarsInUse is empty and calendar preference data for locale
    //    locale are available, then
    //    i. Set calendarsInUse to a List of unique calendar types in common
    //       use for date and time formatting in locale locale, sorted by
    //       descending preference of use in locale.
    //    CLDR's calendarPreferenceData is keyed by region only, so there
    //    are no such data for any locale.
    // c. If calendarsInUse is empty and calendar preference data for region
    //    region are available, then
    //    i. Set calendarsInUse to a List of unique calendar types in common
    //       use for date and time formatting in region region, sorted by
    //       descending preference of use in region.
    if (calendarsInUse.length === 0) {
      var preferences = native.calendarPreferences(region);
      if (preferences !== undefined) calendarsInUse = splitSpaces(preferences);
    }
  }
  // 7. Let list be a new empty List.
  var list = [];
  // 8. Let availableCalendars be AvailableCalendars().
  var availableCalendars = AvailableCalendars();
  // 9. For each element identifier of calendarsInUse, do
  for (var j = 0; j < calendarsInUse.length; j++) {
    // a. Let canonical be CanonicalizeUValue("ca", identifier).
    var canonical = CanonicalizeUValue('ca', calendarsInUse[j]);
    // b. If availableCalendars contains canonical and list does not contain
    //    canonical, then
    //    i. Append canonical to list.
    if (
      listContains(availableCalendars, canonical) &&
      !listContains(list, canonical)
    ) {
      listPush(list, canonical);
    }
  }
  // 10. If list is empty, set list to « "gregory" ».
  if (list.length === 0) list = ['gregory'];
  // 11. Return CreateArrayFromList(list).
  return CreateArrayFromList(list);
}

// CollationsOfLocale ( loc )
function CollationsOfLocale(loc) {
  // 1. If loc.[[Collation]] is not undefined, then
  //    a. Return CreateArrayFromList(« loc.[[Collation]] »).
  var collation = LocaleSlots.get(loc, 'collation');
  if (collation !== undefined) return CreateArrayFromList([collation]);
  // 2. Let match be LookupMatchingLocaleByPrefix(%Intl.Collator%.
  //    [[AvailableLocales]], « loc.[[Locale]] »). That is the plugin's one
  //    [[AvailableLocales]] list (isAvailableLocale).
  var match = LookupMatchingLocaleByPrefix(isAvailableLocale, [
    LocaleSlots.get(loc, 'locale'),
  ]);
  var list;
  if (match !== undefined) {
    // 3. If match is not undefined, then
    //    a. Let foundLocale be match.[[locale]].
    //    b. Let foundLocaleData be %Intl.Collator%.[[SortLocaleData]].
    //       [[<foundLocale>]].
    //    c. Let list be a copy of foundLocaleData.[[co]].
    //    d. Assert: list[0] is null.
    //    e. Remove the first element from list.
    //    The native list is foundLocale's [[co]] without its leading null:
    //    the collation types it has tailorings for.
    list = splitSpaces(native.collationsOfLocale(match.locale));
  } else {
    // 4. Else,
    //    a. Let list be « "emoji", "eor" ».
    list = ['emoji', 'eor'];
  }
  // 5. Let sorted be a copy of list, sorted according to lexicographic code
  //    unit order. Both lists are sorted already: the native one by its
  //    contract, and step 4's as written.
  // 6. Return CreateArrayFromList(sorted).
  return CreateArrayFromList(list);
}

// HourCyclesOfLocale ( loc )
function HourCyclesOfLocale(loc) {
  // 1. If loc.[[HourCycle]] is not undefined, then
  //    a. Return CreateArrayFromList(« loc.[[HourCycle]] »).
  var hourCycle = LocaleSlots.get(loc, 'hourCycle');
  if (hourCycle !== undefined) return CreateArrayFromList([hourCycle]);
  // 2. Let preference be RegionPreference(loc.[[Locale]]).
  var preference = RegionPreference(LocaleSlots.get(loc, 'locale'));
  // 3. If preference.[[RegionOverride]] is not undefined, let
  //    preferredRegions be « preference.[[RegionOverride]],
  //    preference.[[Region]] »; else let preferredRegions be
  //    « preference.[[Region]] ».
  var preferredRegions =
    preference.regionOverride !== undefined
      ? [preference.regionOverride, preference.region]
      : [preference.region];
  // 4. Let hourCycles be a new empty List.
  var hourCycles = [];
  // 5. Let language be GetLocaleLanguage(loc.[[Locale]]).
  var language = GetLocaleLanguage(LocaleSlots.get(loc, 'locale'));
  // 6. For each String region of preferredRegions, do
  for (var i = 0; i < preferredRegions.length; i++) {
    var region = preferredRegions[i];
    // a. Let locale be the string-concatenation of language, "-", and
    //    region.
    var locale = language + '-' + region;
    var data;
    // b. If hourCycles is empty and time data for locale locale are
    //    available, then
    //    i. Set hourCycles to a List of unique hour cycle identifiers, which
    //       must be lower case Strings indicating either the 12-hour format
    //       ("h11", "h12") or the 24-hour format ("h23", "h24"), sorted in
    //       descending preference of those in common use for date and time
    //       formatting in locale locale.
    if (hourCycles.length === 0) {
      data = native.hourCycles(locale);
      if (data !== undefined) hourCycles = splitSpaces(data);
    }
    // c. If hourCycles is empty and time data for region region are
    //    available, then
    //    i. Set hourCycles to a List of unique hour cycle identifiers, which
    //       must be lower case Strings indicating either the 12-hour format
    //       ("h11", "h12") or the 24-hour format ("h23", "h24"), sorted in
    //       descending preference of those in common use for date and time
    //       formatting in region region.
    if (hourCycles.length === 0) {
      data = native.hourCycles(region);
      if (data !== undefined) hourCycles = splitSpaces(data);
    }
  }
  // 7. If hourCycles is empty, set hourCycles to « "h23" ».
  if (hourCycles.length === 0) hourCycles = ['h23'];
  // 8. Return CreateArrayFromList(hourCycles).
  return CreateArrayFromList(hourCycles);
}

// NumberingSystemsOfLocale ( loc )
function NumberingSystemsOfLocale(loc) {
  // 1. If loc.[[NumberingSystem]] is not undefined, then
  //    a. Return CreateArrayFromList(« loc.[[NumberingSystem]] »).
  var numberingSystem = LocaleSlots.get(loc, 'numberingSystem');
  if (numberingSystem !== undefined)
    return CreateArrayFromList([numberingSystem]);
  // 2. Let match be LookupMatchingLocaleByPrefix(%Intl.NumberFormat%.
  //    [[AvailableLocales]], « loc.[[Locale]] »). That is the plugin's one
  //    [[AvailableLocales]] list (isAvailableLocale).
  var match = LookupMatchingLocaleByPrefix(isAvailableLocale, [
    LocaleSlots.get(loc, 'locale'),
  ]);
  var list;
  if (match !== undefined) {
    // 3. If match is not undefined, then
    //    a. Let foundLocale be match.[[locale]].
    //    b. Let foundLocaleData be %Intl.NumberFormat%.[[LocaleData]].
    //       [[<foundLocale>]].
    //    c. Let numberingSystems be foundLocaleData.[[nu]].
    //    d. Let list be « numberingSystems[0] ».
    //    numberingSystems[0] is the numbering system of foundLocale's
    //    decimal data, which the native helper gives.
    list = [native.defaultNumberingSystem(match.locale)];
  } else {
    // 4. Else,
    //    a. Let list be « "latn" ».
    list = ['latn'];
  }
  // 5. Return CreateArrayFromList(list).
  return CreateArrayFromList(list);
}

// TimeZonesOfLocale ( loc )
function TimeZonesOfLocale(loc) {
  // 1. Let region be GetLocaleRegion(loc.[[Locale]]).
  var region = GetLocaleRegion(LocaleSlots.get(loc, 'locale'));
  // 2. If region is undefined, return undefined.
  if (region === undefined) return undefined;
  // 3. Let list be a List of unique canonical time zone identifiers, which
  //    must be String values indicating a canonical Zone name of the IANA
  //    Time Zone Database, of those in common use in region. The list is
  //    empty if no time zones are commonly used in region. The list is
  //    sorted according to lexicographic code unit order.
  //    The native list is exactly that: primary identifiers, sorted.
  var list = splitSpaces(native.timeZonesOfRegion(region));
  // 4. Return CreateArrayFromList(list).
  return CreateArrayFromList(list);
}

// TextDirectionOfLocale ( loc )
function TextDirectionOfLocale(loc) {
  // 1. Let locale be loc.[[Locale]].
  var locale = LocaleSlots.get(loc, 'locale');
  // 2. Let script be GetLocaleScript(locale).
  var script = GetLocaleScript(locale);
  // 3. If script is undefined, then
  if (script === undefined) {
    // a. Let maximal be the result of the Add Likely Subtags algorithm
    //    applied to locale. If an error is signaled, return undefined.
    //    (locale is canonical, so the native helper cannot fail.)
    var maximal = native.addLikelySubtags(locale);
    // b. Set script to GetLocaleScript(maximal).
    script = GetLocaleScript(maximal);
    // c. If script is undefined, return undefined.
    if (script === undefined) return undefined;
  }
  var direction = native.scriptDirection(script);
  // 4. If the default general ordering of characters within a line in
  //    script is right-to-left, return "rtl".
  if (direction === 'rtl') return 'rtl';
  // 5. If the default general ordering of characters within a line in
  //    script is left-to-right, return "ltr".
  if (direction === 'ltr') return 'ltr';
  // 6. Return undefined.
  return undefined;
}

// WeekInfoOfLocale ( loc ). The WeekInfo Record is a null-prototype record
// with fields firstDay and weekend.
function WeekInfoOfLocale(loc) {
  // 1. Let preference be RegionPreference(loc.[[Locale]]).
  var preference = RegionPreference(LocaleSlots.get(loc, 'locale'));
  // 2. Let region be preference.[[Region]].
  var region = preference.region;
  // 3. Let regionOverride be preference.[[RegionOverride]].
  var regionOverride = preference.regionOverride;
  // 4. If regionOverride is not undefined and week data for region
  //    regionOverride are available, then
  //    a. Let lookupRegion be regionOverride.
  // 5. Else if week data for region region are available, then
  //    a. Let lookupRegion be region.
  // 6. Else,
  //    a. Let lookupRegion be "001".
  //    Week data are available for every region: CLDR's "001" data apply
  //    to each region it does not list, and the native helper always
  //    loads with that fallback. (ICU4X drops the regions whose data equal
  //    "001", so an exact load would wrongly skip them.) Steps 5 and 6.a
  //    therefore reduce to the override, if any, and else the region.
  var lookupRegion = regionOverride !== undefined ? regionOverride : region;
  // 7. Let weekInfo be a Record whose fields are defined by the WeekInfo
  //    Record Fields table, with values based on region lookupRegion.
  //    The native data are the first day, then the weekend days in
  //    ascending order.
  var data = splitSpaces(native.weekInfo(lookupRegion));
  var weekend = [];
  for (var i = 1; i < data.length; i++) listPush(weekend, ToNumber(data[i]));
  var weekInfo = {
    __proto__: null,
    firstDay: ToNumber(data[0]),
    weekend: weekend,
  };
  // 8. Let firstDayString be loc.[[FirstDayOfWeek]].
  var firstDayString = LocaleSlots.get(loc, 'firstDayOfWeek');
  // 9. If firstDayString is not undefined, then
  if (firstDayString !== undefined) {
    // a. Let firstDay be WeekdayUValueToNumber(firstDayString).
    var firstDay = WeekdayUValueToNumber(firstDayString);
    // b. If firstDay is not undefined, set weekInfo.[[FirstDay]] to
    //    firstDay.
    if (firstDay !== undefined) weekInfo.firstDay = firstDay;
  }
  // 10. Return weekInfo.
  return weekInfo;
}

/// Install Intl.Locale and its prototype on intl; called by installIntl.
function installLocale(intl) {
  // 1. Let proto be %Intl.Locale.prototype%, an ordinary object whose
  //    [[Prototype]] is %Object.prototype% ("Properties of the Intl.Locale
  //    Prototype Object").
  var proto = {};
  // 2. "Properties of the Intl.Locale Constructor": Intl.Locale.prototype,
  //    with its length 1, and Intl.Locale.prototype.constructor.
  defineConstructor(Locale, 1, proto);
  // 3. The accessor properties of the prototype, with no set accessor.
  defineGetters(proto, {
    // get Intl.Locale.prototype.baseName
    get baseName() {
      // 1. Let loc be the this value.
      // 2. Perform ? RequireInternalSlot(loc, [[InitializedLocale]]).
      // 3. Return GetLocaleBaseName(loc.[[Locale]]).
      return GetLocaleBaseName(LocaleSlots.get(this, 'locale'));
    },
    // get Intl.Locale.prototype.calendar
    get calendar() {
      // 1. Let loc be the this value.
      // 2. Perform ? RequireInternalSlot(loc, [[InitializedLocale]]).
      // 3. Return loc.[[Calendar]].
      return LocaleSlots.get(this, 'calendar');
    },
    // get Intl.Locale.prototype.caseFirst
    get caseFirst() {
      // 1. Let loc be the this value.
      // 2. Perform ? RequireInternalSlot(loc, [[InitializedLocale]]).
      // 3. Return loc.[[CaseFirst]].
      return LocaleSlots.get(this, 'caseFirst');
    },
    // get Intl.Locale.prototype.collation
    get collation() {
      // 1. Let loc be the this value.
      // 2. Perform ? RequireInternalSlot(loc, [[InitializedLocale]]).
      // 3. Return loc.[[Collation]].
      return LocaleSlots.get(this, 'collation');
    },
    // get Intl.Locale.prototype.firstDayOfWeek
    get firstDayOfWeek() {
      // 1. Let loc be the this value.
      // 2. Perform ? RequireInternalSlot(loc, [[InitializedLocale]]).
      // 3. Return loc.[[FirstDayOfWeek]].
      return LocaleSlots.get(this, 'firstDayOfWeek');
    },
    // get Intl.Locale.prototype.hourCycle
    get hourCycle() {
      // 1. Let loc be the this value.
      // 2. Perform ? RequireInternalSlot(loc, [[InitializedLocale]]).
      // 3. Return loc.[[HourCycle]].
      return LocaleSlots.get(this, 'hourCycle');
    },
    // get Intl.Locale.prototype.language
    get language() {
      // 1. Let loc be the this value.
      // 2. Perform ? RequireInternalSlot(loc, [[InitializedLocale]]).
      // 3. Return GetLocaleLanguage(loc.[[Locale]]).
      return GetLocaleLanguage(LocaleSlots.get(this, 'locale'));
    },
    // get Intl.Locale.prototype.numberingSystem
    get numberingSystem() {
      // 1. Let loc be the this value.
      // 2. Perform ? RequireInternalSlot(loc, [[InitializedLocale]]).
      // 3. Return loc.[[NumberingSystem]].
      return LocaleSlots.get(this, 'numberingSystem');
    },
    // get Intl.Locale.prototype.numeric
    get numeric() {
      // 1. Let loc be the this value.
      // 2. Perform ? RequireInternalSlot(loc, [[InitializedLocale]]).
      // 3. Return loc.[[Numeric]].
      return LocaleSlots.get(this, 'numeric');
    },
    // get Intl.Locale.prototype.region
    get region() {
      // 1. Let loc be the this value.
      // 2. Perform ? RequireInternalSlot(loc, [[InitializedLocale]]).
      // 3. Return GetLocaleRegion(loc.[[Locale]]).
      return GetLocaleRegion(LocaleSlots.get(this, 'locale'));
    },
    // get Intl.Locale.prototype.script
    get script() {
      // 1. Let loc be the this value.
      // 2. Perform ? RequireInternalSlot(loc, [[InitializedLocale]]).
      // 3. Return GetLocaleScript(loc.[[Locale]]).
      return GetLocaleScript(LocaleSlots.get(this, 'locale'));
    },
    // get Intl.Locale.prototype.variants
    get variants() {
      // 1. Let loc be the this value.
      // 2. Perform ? RequireInternalSlot(loc, [[InitializedLocale]]).
      // 3. Return GetLocaleVariants(loc.[[Locale]]).
      return GetLocaleVariants(LocaleSlots.get(this, 'locale'));
    },
  });
  // 4. The method properties of the prototype.
  defineMethods(proto, {
    // Intl.Locale.prototype.getCalendars ( )
    getCalendars() {
      // 1. Let loc be the this value.
      var loc = this;
      // 2. Perform ? RequireInternalSlot(loc, [[InitializedLocale]]).
      requireInitializedLocale(loc);
      // 3. Return CalendarsOfLocale(loc).
      return CalendarsOfLocale(loc);
    },
    // Intl.Locale.prototype.getCollations ( )
    getCollations() {
      // 1. Let loc be the this value.
      var loc = this;
      // 2. Perform ? RequireInternalSlot(loc, [[InitializedLocale]]).
      requireInitializedLocale(loc);
      // 3. Return CollationsOfLocale(loc).
      return CollationsOfLocale(loc);
    },
    // Intl.Locale.prototype.getHourCycles ( )
    getHourCycles() {
      // 1. Let loc be the this value.
      var loc = this;
      // 2. Perform ? RequireInternalSlot(loc, [[InitializedLocale]]).
      requireInitializedLocale(loc);
      // 3. Return HourCyclesOfLocale(loc).
      return HourCyclesOfLocale(loc);
    },
    // Intl.Locale.prototype.getNumberingSystems ( )
    getNumberingSystems() {
      // 1. Let loc be the this value.
      var loc = this;
      // 2. Perform ? RequireInternalSlot(loc, [[InitializedLocale]]).
      requireInitializedLocale(loc);
      // 3. Return NumberingSystemsOfLocale(loc).
      return NumberingSystemsOfLocale(loc);
    },
    // Intl.Locale.prototype.getTextInfo ( )
    getTextInfo() {
      // 1. Let loc be the this value.
      var loc = this;
      // 2. Perform ? RequireInternalSlot(loc, [[InitializedLocale]]).
      requireInitializedLocale(loc);
      // 3. Let info be OrdinaryObjectCreate(%Object.prototype%).
      var info = {};
      // 4. Let dir be TextDirectionOfLocale(loc).
      var dir = TextDirectionOfLocale(loc);
      // 5. Perform ! CreateDataPropertyOrThrow(info, "direction", dir).
      createDataProperty(info, 'direction', dir);
      // 6. Return info.
      return info;
    },
    // Intl.Locale.prototype.getTimeZones ( )
    getTimeZones() {
      // 1. Let loc be the this value.
      var loc = this;
      // 2. Perform ? RequireInternalSlot(loc, [[InitializedLocale]]).
      requireInitializedLocale(loc);
      // 3. Return TimeZonesOfLocale(loc).
      return TimeZonesOfLocale(loc);
    },
    // Intl.Locale.prototype.getWeekInfo ( )
    getWeekInfo() {
      // 1. Let loc be the this value.
      var loc = this;
      // 2. Perform ? RequireInternalSlot(loc, [[InitializedLocale]]).
      requireInitializedLocale(loc);
      // 3. Let info be OrdinaryObjectCreate(%Object.prototype%).
      var info = {};
      // 4. Let weekInfo be WeekInfoOfLocale(loc).
      var weekInfo = WeekInfoOfLocale(loc);
      // 5. Perform ! CreateDataPropertyOrThrow(info, "firstDay",
      //    weekInfo.[[FirstDay]]).
      createDataProperty(info, 'firstDay', weekInfo.firstDay);
      // 6. Perform ! CreateDataPropertyOrThrow(info, "weekend",
      //    CreateArrayFromList(weekInfo.[[Weekend]])).
      createDataProperty(
        info,
        'weekend',
        CreateArrayFromList(weekInfo.weekend),
      );
      // 7. Return info.
      return info;
    },
    // Intl.Locale.prototype.maximize ( )
    maximize() {
      // 1. Let loc be the this value.
      // 2. Perform ? RequireInternalSlot(loc, [[InitializedLocale]]).
      // 3. Let maximal be the result of the Add Likely Subtags algorithm
      //    applied to loc.[[Locale]]. If an error is signaled, set maximal to
      //    loc.[[Locale]]. ([[Locale]] is canonical, so the native helper
      //    cannot fail.)
      var maximal = native.addLikelySubtags(LocaleSlots.get(this, 'locale'));
      // 4. Return ! Construct(%Intl.Locale%, « maximal »).
      return new Locale(maximal);
    },
    // Intl.Locale.prototype.minimize ( )
    minimize() {
      // 1. Let loc be the this value.
      // 2. Perform ? RequireInternalSlot(loc, [[InitializedLocale]]).
      // 3. Let minimal be the result of the Remove Likely Subtags algorithm
      //    applied to loc.[[Locale]]. If an error is signaled, set minimal to
      //    loc.[[Locale]]. ([[Locale]] is canonical, so the native helper
      //    cannot fail.)
      var minimal = native.removeLikelySubtags(LocaleSlots.get(this, 'locale'));
      // 4. Return ! Construct(%Intl.Locale%, « minimal »).
      return new Locale(minimal);
    },
    // Intl.Locale.prototype.toString ( )
    toString() {
      // 1. Let loc be the this value.
      // 2. Perform ? RequireInternalSlot(loc, [[InitializedLocale]]).
      // 3. Return loc.[[Locale]].
      return LocaleSlots.get(this, 'locale');
    },
  });
  // 5. Intl.Locale.prototype [ %Symbol.toStringTag% ]: "Intl.Locale",
  //    { [[Writable]]: false, [[Enumerable]]: false, [[Configurable]]: true }.
  setToStringTag(proto, 'Intl.Locale');
  // 6. "The Intl.Locale Constructor": the initial value of the "Locale"
  //    property of the Intl object. ECMA-262 18 gives such data properties
  //    the attributes { [[Writable]]: true, [[Enumerable]]: false,
  //    [[Configurable]]: true }.
  ObjectDefineProperty(intl, 'Locale', {
    __proto__: null,
    value: Locale,
    writable: true,
    enumerable: false,
    configurable: true,
  });
}
