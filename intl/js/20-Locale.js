/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// ECMA-402 "Locale Objects": the Intl.Locale constructor, UpdateLanguageId,
// MakeLocaleRecord and the Intl.Locale prototype, following the draft
// https://tc39.es/ecma402/ dated October 24, 2026. The Locale abstract
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
    if (!LocaleSlots.is(o))
      throw new _TypeError('Receiver is not an Intl.Locale object');
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
