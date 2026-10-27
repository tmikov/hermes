/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// ECMA-402 "RelativeTimeFormat Objects": the Intl.RelativeTimeFormat
// constructor, its supportedLocalesOf, the prototype's resolvedOptions,
// format, formatToParts and @@toStringTag, SingularRelativeTimeUnit,
// PartitionRelativeTimePattern, MakePartsList, FormatRelativeTime and
// FormatRelativeTimeToParts, following the draft https://tc39.es/ecma402/
// dated October 24, 2026. An instance has no native state: its patterns
// and relative names come from relativeTimePatterns, and its numbers and
// plural categories from an internal NumberFormat and PluralRules, made
// with CreateNumberFormat and CreatePluralRules (Phase 5 plan decisions 4
// and 8), never through Intl.NumberFormat or Intl.PluralRules.

/// %Intl.RelativeTimeFormat.prototype%; set by installRelativeTimeFormat.
var RelativeTimeFormatPrototype;

/// The internal slots of an Intl.RelativeTimeFormat instance ("Properties
/// of Intl.RelativeTimeFormat Instances"), as private fields of the object
/// passed to the constructor. [[InitializedRelativeTimeFormat]] is the
/// presence of #locale. #localeData holds what ResolveOptions gives as
/// [[LocaleData]]: NumberFormat's locale-data record, { nu }, the only part
/// of the draft's [[LocaleData]] that ResolveLocale reads. #fields stands in
/// for the rest of the draft's [[LocaleData]].[[<locale>]], the fields
/// "second" to "year" (or "second-short" and so on) that
/// PartitionRelativeTimePattern reads: those of [[Style]], from
/// relativeTimeFields.
class RelativeTimeFormatSlots extends ReturnOverride {
  /// [[Locale]]
  #locale;
  /// [[LocaleData]], as ResolveOptions gives it: NumberFormat's record.
  #localeData;
  /// [[Style]]
  #style;
  /// [[Numeric]]
  #numeric;
  /// [[NumberFormat]]
  #numberFormat;
  /// [[NumberingSystem]]
  #numberingSystem;
  /// [[PluralRules]]
  #pluralRules;
  /// The fields of [[LocaleData]].[[<locale>]] for the language
  /// identifier of [[Locale]] and [[Style]].
  #fields;

  /// Attach the slots to o, from the null-prototype record slots whose
  /// fields are named after the slots, and the fields record fields.
  constructor(o, slots, fields) {
    // 1. Add the private fields to o (ReturnOverride returns o).
    super(o);
    // 2. Set each slot from the field of the same name.
    this.#locale = slots.locale;
    this.#localeData = slots.localeData;
    this.#style = slots.style;
    this.#numeric = slots.numeric;
    this.#numberFormat = slots.numberFormat;
    this.#numberingSystem = slots.numberingSystem;
    this.#pluralRules = slots.pluralRules;
    // 3. Keep the fields of the locale data.
    this.#fields = fields;
  }

  /// Whether o is an Object with an [[InitializedRelativeTimeFormat]]
  /// internal slot.
  static is(o) {
    // 1. If o is not an Object, return false.
    if (!isObject(o)) return false;
    // 2. Return whether o has the private fields.
    return #locale in o;
  }

  /// The slot name of o, after ECMA-262 RequireInternalSlot(o,
  /// [[InitializedRelativeTimeFormat]]). name is the slot's name in
  /// lowerCamelCase, or "fields".
  static get(o, name) {
    // 1. Perform ? RequireInternalSlot(o, [[InitializedRelativeTimeFormat]]).
    requireInitializedRelativeTimeFormat(o);
    // 2. Return the slot.
    switch (name) {
      case 'locale':
        return o.#locale;
      case 'localeData':
        return o.#localeData;
      case 'style':
        return o.#style;
      case 'numeric':
        return o.#numeric;
      case 'numberFormat':
        return o.#numberFormat;
      case 'numberingSystem':
        return o.#numberingSystem;
      case 'pluralRules':
        return o.#pluralRules;
      case 'fields':
        return o.#fields;
    }
    throw new _TypeError(
      'Intl internal error: no RelativeTimeFormat slot ' + name,
    );
  }
}

/// ECMA-262 RequireInternalSlot(o, [[InitializedRelativeTimeFormat]]).
function requireInitializedRelativeTimeFormat(o) {
  // 1. If o is not an Object, or has no [[InitializedRelativeTimeFormat]]
  //    internal slot, throw a TypeError exception.
  if (!RelativeTimeFormatSlots.is(o))
    throw new _TypeError('Receiver is not an Intl.RelativeTimeFormat object');
}

/// The fields of %Intl.RelativeTimeFormat% that ResolveOptions reads
/// ("Internal slots"): [[AvailableLocales]], the plugin's one list,
/// [[RelevantExtensionKeys]] « "nu" » and [[ResolutionOptionDescriptors]].
/// [[LocaleData]].[[<locale>]].[[nu]] is NumberFormat's list, so
/// numberFormatLocaleData serves as [[LocaleData]] (decision 4); it has no
/// "native", "traditio" or "finance".
var RELATIVE_TIME_FORMAT_SERVICE = {
  __proto__: null,
  availableLocales: isAvailableLocale,
  relevantExtensionKeys: ['nu'],
  resolutionOptionDescriptors: [
    {__proto__: null, key: 'nu', property: 'numberingSystem'},
  ],
};

/// The units of relativeTimePatterns, in its order.
var RELATIVE_TIME_UNITS = [
  'second',
  'minute',
  'hour',
  'day',
  'week',
  'month',
  'quarter',
  'year',
];

/// The cache of relativeTimeFields results, keyed by "languageId;style".
var relativeTimeFieldsCache = newDataCache();

/// The fields of %Intl.RelativeTimeFormat%.[[LocaleData]] that
/// PartitionRelativeTimePattern reads for the language identifier
/// languageId and the style style, from native.relativeTimePatterns,
/// through its cache: a null-prototype record with a field for each unit
/// (second to year), each a null-prototype record { future, past,
/// relatives }. future and past are Lists of the patterns of the plural
/// categories in PLURAL_CATEGORY_INDEX's order, and relatives is a
/// null-prototype record from ToString of an offset ("-1") to its name
/// ("yesterday"). For style "long" these are the draft's fields "second"
/// to "year"; for "short" and "narrow", its fields "second-short" and so
/// on, which ICU4X has for every locale and unit.
function relativeTimeFields(languageId, style) {
  // 1. If the cache has the record, return it.
  var key = languageId + ';' + style;
  var fields = dataCacheGet(relativeTimeFieldsCache, key);
  if (fields !== undefined) return fields;
  // 2. Otherwise parse the native list: for each unit, 6 future patterns,
  //    6 past patterns, the number n of relative names, and n pairs of
  //    offset and name.
  var list = native.relativeTimePatterns(languageId, style);
  fields = {__proto__: null};
  var i = 0;
  for (var u = 0; u < RELATIVE_TIME_UNITS.length; u++) {
    var future = [];
    var past = [];
    for (var c = 0; c < 6; c++) {
      listPush(future, list[i + c]);
      listPush(past, list[i + 6 + c]);
    }
    var n = ToNumber(list[i + 12]);
    i += 13;
    var relatives = {__proto__: null};
    for (var k = 0; k < n; k++) {
      relatives[list[i]] = list[i + 1];
      i += 2;
    }
    fields[RELATIVE_TIME_UNITS[u]] = {
      __proto__: null,
      future: future,
      past: past,
      relatives: relatives,
    };
  }
  // 3. Cache and return the record.
  dataCachePut(relativeTimeFieldsCache, key, fields);
  return fields;
}

/// Steps 3 to 18 of Intl.RelativeTimeFormat ( [ locales [ , options ] ] ),
/// on the object relativeTimeFormat that steps 1 and 2 made. The slots are
/// collected in a null-prototype record and attached at the end, once
/// every value is known; nothing between can observe them. Returns
/// relativeTimeFormat.
function initializeRelativeTimeFormat(relativeTimeFormat, locales, options) {
  var slots = {__proto__: null};
  // 3. Let optionsResolution be ? ResolveOptions(%Intl.RelativeTimeFormat%,
  //    %Intl.RelativeTimeFormat%.[[LocaleData]], locales, options,
  //    « coerce-options »).
  var optionsResolution = ResolveOptions(
    RELATIVE_TIME_FORMAT_SERVICE,
    numberFormatLocaleData,
    locales,
    options,
    ['coerce-options'],
  );
  // 4. Set options to optionsResolution.[[Options]].
  options = optionsResolution.options;
  // 5. Let resolvedLocale be optionsResolution.[[ResolvedLocale]].
  var resolvedLocale = optionsResolution.resolvedLocale;
  // 6. Let locale be resolvedLocale.[[Locale]].
  var locale = resolvedLocale.locale;
  // 7. Set relativeTimeFormat.[[Locale]] to locale.
  slots.locale = locale;
  // 8. Set relativeTimeFormat.[[LocaleData]] to
  //    resolvedLocale.[[LocaleData]]. This is NumberFormat's record { nu };
  //    the fields PartitionRelativeTimePattern reads are attached below as
  //    #fields.
  slots.localeData = resolvedLocale.localeData;
  // 9. Set relativeTimeFormat.[[NumberingSystem]] to resolvedLocale.[[nu]].
  slots.numberingSystem = resolvedLocale.nu;
  // 10. Let style be ? GetOption(options, "style", string, « "long",
  //     "short", "narrow" », "long").
  var style = GetOption(
    options,
    'style',
    'string',
    ['long', 'short', 'narrow'],
    'long',
  );
  // 11. Set relativeTimeFormat.[[Style]] to style.
  slots.style = style;
  // 12. Let numeric be ? GetOption(options, "numeric", string,
  //     « "always", "auto" », "always").
  var numeric = GetOption(
    options,
    'numeric',
    'string',
    ['always', 'auto'],
    'always',
  );
  // 13. Set relativeTimeFormat.[[Numeric]] to numeric.
  slots.numeric = numeric;
  // 14. Let nfOptions be OrdinaryObjectCreate(null).
  var nfOptions = {__proto__: null};
  // 15. Perform ! CreateDataPropertyOrThrow(nfOptions, "numberingSystem",
  //     relativeTimeFormat.[[NumberingSystem]]).
  nfOptions.numberingSystem = slots.numberingSystem;
  // 16. Let relativeTimeFormat.[[NumberFormat]] be !
  //     Construct(%Intl.NumberFormat%, « locale, nfOptions »).
  slots.numberFormat = CreateNumberFormat(locale, nfOptions);
  // 17. Let relativeTimeFormat.[[PluralRules]] be !
  //     Construct(%Intl.PluralRules%, « locale »).
  slots.pluralRules = CreatePluralRules(locale, undefined);
  //     The fields PartitionRelativeTimePattern reads, requested now (as
  //     NumberFormat requests its data in its constructor), and the slots.
  new RelativeTimeFormatSlots(
    relativeTimeFormat,
    slots,
    relativeTimeFields(GetLocaleBaseName(locale), style),
  );
  // 18. Return relativeTimeFormat.
  return relativeTimeFormat;
}

// Intl.RelativeTimeFormat ( [ locales [ , options ] ] )
function RelativeTimeFormat(locales = undefined, options = undefined) {
  // 1. If NewTarget is undefined, throw a TypeError exception.
  if (new.target === undefined)
    throw new _TypeError('Intl.RelativeTimeFormat must be called with new');
  // 2. Let relativeTimeFormat be ? OrdinaryCreateFromConstructor(NewTarget,
  //    "%Intl.RelativeTimeFormat.prototype%",
  //    « [[InitializedRelativeTimeFormat]], [[Locale]], [[LocaleData]],
  //    [[Style]], [[Numeric]], [[NumberFormat]], [[NumberingSystem]],
  //    [[PluralRules]] »).
  //    Hermes created this from NewTarget before the call (design spec
  //    "Constructors"); initializeRelativeTimeFormat attaches the slots.
  // 3-18. The steps that resolve the slots, numbered there.
  return initializeRelativeTimeFormat(this, locales, options);
}

// SingularRelativeTimeUnit ( unit )
function SingularRelativeTimeUnit(unit) {
  switch (unit) {
    // 1. If unit is "seconds", return "second".
    case 'seconds':
      return 'second';
    // 2. If unit is "minutes", return "minute".
    case 'minutes':
      return 'minute';
    // 3. If unit is "hours", return "hour".
    case 'hours':
      return 'hour';
    // 4. If unit is "days", return "day".
    case 'days':
      return 'day';
    // 5. If unit is "weeks", return "week".
    case 'weeks':
      return 'week';
    // 6. If unit is "months", return "month".
    case 'months':
      return 'month';
    // 7. If unit is "quarters", return "quarter".
    case 'quarters':
      return 'quarter';
    // 8. If unit is "years", return "year".
    case 'years':
      return 'year';
    // 9. If unit is not one of "second", "minute", "hour", "day", "week",
    //    "month", "quarter", or "year", throw a RangeError exception.
    // 10. Return unit.
    case 'second':
    case 'minute':
    case 'hour':
    case 'day':
    case 'week':
    case 'month':
    case 'quarter':
    case 'year':
      return unit;
  }
  throw new _RangeError('Invalid unit argument: ' + unit);
}

// PartitionRelativeTimePattern ( relativeTimeFormat, value, unit )
// Returns a List of null-prototype records { type, value, unit }, unit
// undefined for ~empty~.
function PartitionRelativeTimePattern(relativeTimeFormat, value, unit) {
  // 1. If value is NaN, +∞𝔽, or -∞𝔽, throw a RangeError exception.
  if (!NumberIsFinite(value))
    throw new _RangeError('Invalid value: must be a finite number');
  // 2. Set unit to ? SingularRelativeTimeUnit(unit).
  unit = SingularRelativeTimeUnit(unit);
  // 3. Let fields be relativeTimeFormat.[[LocaleData]].
  // 4. Let patterns be fields.[[<unit>]].
  // 5. Let style be relativeTimeFormat.[[Style]].
  // 6. If style is "short" or "narrow", then
  //    a. Let key be the string-concatenation of unit, "-", and style.
  //    b. If fields has a field [[<key>]], set patterns to fields.[[<key>]].
  //    fields is #fields, which stands in for [[LocaleData]].[[<locale>]]
  //    and holds only the fields of [[Style]] (relativeTimeFields): the
  //    field of unit is fields.[[<key>]] for "short" and "narrow", which
  //    exists for every locale and unit, and fields.[[<unit>]] for "long".
  var fields = RelativeTimeFormatSlots.get(relativeTimeFormat, 'fields');
  var patterns = fields[unit];
  // 7. If relativeTimeFormat.[[Numeric]] is "auto", then
  if (RelativeTimeFormatSlots.get(relativeTimeFormat, 'numeric') === 'auto') {
    // a. Let valueString be ! ToString(value).
    var valueString = ToString(value);
    // b. If patterns has a field [[<valueString>]], then
    //    i. Let result be patterns.[[<valueString>]].
    //    ii. Return a List containing the Record { [[Type]]: "literal",
    //        [[Value]]: result, [[Unit]]: empty }.
    //    The relative names are the field relatives of patterns.
    var result = patterns.relatives[valueString];
    if (result !== undefined) {
      var named = [];
      listPush(named, {
        __proto__: null,
        type: 'literal',
        value: result,
        unit: undefined,
      });
      return named;
    }
  }
  // 8. If value is -0𝔽 or value < -0𝔽, then
  //    a. Let tense be "past".
  // 9. Else,
  //    a. Let tense be "future".
  var past = value < 0 || (value === 0 && 1 / value < 0);
  // 10. Let tensePatterns be patterns.[[<tense>]].
  var tensePatterns = past ? patterns.past : patterns.future;
  // 11. Let formattedValue be PartitionNumberPattern(
  //     relativeTimeFormat.[[NumberFormat]], ℝ(value)).
  //     Two deliberate deviations:
  //     - the magnitude (decision 4): the draft formats the value with its
  //       sign, which would give "-1 day ago"; test262 and every engine
  //       format its magnitude, and the past pattern carries the sense.
  //       The magnitude of -0 is +0 (0 - -0), never negative-zero, which
  //       would show "-0";
  //     - the conversion: ℝ(value) is the exact value of the Number, but
  //       the plugin converts it as NumberFormat does
  //       (ToIntlMathematicalValue, through Number::toString), so
  //       RelativeTimeFormat shows the same digits as the plugin's own
  //       NumberFormat. Engines agree on large integers:
  //       1000000000000000100 (exactly 1000000000000000128) shows as
  //       1,000,000,000,000,000,100, as in Node. They differ on some
  //       fractions: 1.0005 shows as 1.001, where Node shows 1.
  var formattedValue = PartitionNumberPattern(
    RelativeTimeFormatSlots.get(relativeTimeFormat, 'numberFormat'),
    IntlMVFromNumber(value <= 0 ? 0 - value : value),
  ).parts;
  // 12. Let pluralCategory be ResolvePlural(
  //     relativeTimeFormat.[[PluralRules]], value).[[PluralCategory]].
  //     The sign does not change the category.
  var pluralCategory = ResolvePlural(
    RelativeTimeFormatSlots.get(relativeTimeFormat, 'pluralRules'),
    IntlMVFromNumber(value),
  ).pluralCategory;
  // 13. Let pattern be tensePatterns.[[<pluralCategory>]].
  var pattern = tensePatterns[PLURAL_CATEGORY_INDEX[pluralCategory]];
  // 14. Return MakePartsList(pattern, unit, formattedValue).
  return MakePartsList(pattern, unit, formattedValue);
}

// MakePartsList ( pattern, unit, parts )
// Returns a List of null-prototype records { type, value, unit }, unit
// undefined for ~empty~.
function MakePartsList(pattern, unit, parts) {
  // 1. Let patternParts be PartitionPattern(pattern).
  var patternParts = PartitionPattern(pattern);
  // 2. Let result be a new empty List.
  var result = [];
  // 3. For each Record { [[Type]], [[Value]] } patternPart of patternParts,
  //    do
  for (var i = 0; i < patternParts.length; i++) {
    var patternPart = patternParts[i];
    if (patternPart.type === 'literal') {
      // a. If patternPart.[[Type]] is "literal", then
      //    i. Append the Record { [[Type]]: "literal", [[Value]]:
      //       patternPart.[[Value]], [[Unit]]: empty } to result.
      listPush(result, {
        __proto__: null,
        type: 'literal',
        value: patternPart.value,
        unit: undefined,
      });
    } else {
      // b. Else,
      //    i. Assert: patternPart.[[Type]] is "0".
      if (patternPart.type !== '0')
        throw new _TypeError('Intl internal error: bad pattern ' + pattern);
      //    ii. For each Record { [[Type]], [[Value]] } part of parts, do
      //        1. Append the Record { [[Type]]: part.[[Type]], [[Value]]:
      //           part.[[Value]], [[Unit]]: unit } to result.
      for (var j = 0; j < parts.length; j++) {
        listPush(result, {
          __proto__: null,
          type: parts[j].type,
          value: parts[j].value,
          unit: unit,
        });
      }
    }
  }
  // 4. Return result.
  return result;
}

// FormatRelativeTime ( relativeTimeFormat, value, unit )
function FormatRelativeTime(relativeTimeFormat, value, unit) {
  // 1. Let parts be ? PartitionRelativeTimePattern(relativeTimeFormat,
  //    value, unit).
  var parts = PartitionRelativeTimePattern(relativeTimeFormat, value, unit);
  // 2. Let result be the empty String.
  var result = '';
  // 3. For each Record { [[Type]], [[Value]], [[Unit]] } part of parts, do
  //    a. Set result to the string-concatenation of result and
  //       part.[[Value]].
  for (var i = 0; i < parts.length; i++) result += parts[i].value;
  // 4. Return result.
  return result;
}

// FormatRelativeTimeToParts ( relativeTimeFormat, value, unit )
function FormatRelativeTimeToParts(relativeTimeFormat, value, unit) {
  // 1. Let parts be ? PartitionRelativeTimePattern(relativeTimeFormat,
  //    value, unit).
  var parts = PartitionRelativeTimePattern(relativeTimeFormat, value, unit);
  // 2. Let result be ! ArrayCreate(0).
  var result = [];
  // 3. Let n be 0.
  // 4. For each Record { [[Type]], [[Value]], [[Unit]] } part of parts, do
  for (var n = 0; n < parts.length; n++) {
    var part = parts[n];
    // a. Let partObj be OrdinaryObjectCreate(%Object.prototype%).
    // b. Perform ! CreateDataPropertyOrThrow(partObj, "type",
    //    part.[[Type]]).
    // c. Perform ! CreateDataPropertyOrThrow(partObj, "value",
    //    part.[[Value]]).
    // d. If part.[[Unit]] is not empty, then
    //    i. Perform ! CreateDataPropertyOrThrow(partObj, "unit",
    //       part.[[Unit]]).
    //    An object literal defines the properties, in this order.
    // e. Perform ! CreateDataPropertyOrThrow(result, ! ToString(𝔽(n)),
    //    partObj).
    // f. Set n to n + 1.
    listPush(
      result,
      part.unit === undefined
        ? {type: part.type, value: part.value}
        : {type: part.type, value: part.value, unit: part.unit},
    );
  }
  // 5. Return result.
  return result;
}

/// The rows of the table "Resolved Options of RelativeTimeFormat
/// Instances", in table order: [Property, slot name].
var RELATIVE_TIME_FORMAT_RESOLVED_OPTIONS = [
  ['locale', 'locale'],
  ['style', 'style'],
  ['numeric', 'numeric'],
  ['numberingSystem', 'numberingSystem'],
];

/// Install Intl.RelativeTimeFormat on intl; called by installIntl.
function installRelativeTimeFormat(intl) {
  // 1. Let proto be %Intl.RelativeTimeFormat.prototype%, an ordinary
  //    object whose [[Prototype]] is %Object.prototype% ("Properties of
  //    the Intl.RelativeTimeFormat Prototype Object").
  var proto = {};
  RelativeTimeFormatPrototype = proto;
  // 2. "Properties of the Intl.RelativeTimeFormat Constructor":
  //    Intl.RelativeTimeFormat.prototype, with its length 0, and
  //    Intl.RelativeTimeFormat.prototype.constructor.
  defineConstructor(RelativeTimeFormat, 0, proto);
  // 3. The method properties of the constructor.
  defineMethods(RelativeTimeFormat, {
    // Intl.RelativeTimeFormat.supportedLocalesOf ( locales [ , options ] )
    supportedLocalesOf(locales, options = undefined) {
      // 1. Let availableLocales be
      //    %Intl.RelativeTimeFormat%.[[AvailableLocales]].
      // 2. Let requestedLocales be ? CanonicalizeLocaleList(locales).
      var requestedLocales = CanonicalizeLocaleList(locales);
      // 3. Return ? FilterLocales(availableLocales, requestedLocales,
      //    options).
      return FilterLocales(isAvailableLocale, requestedLocales, options);
    },
  });
  // 4. The method properties of the prototype, in the draft's order.
  defineMethods(proto, {
    // Intl.RelativeTimeFormat.prototype.resolvedOptions ( )
    resolvedOptions() {
      // 1. Let relativeTimeFormat be the this value.
      var relativeTimeFormat = this;
      // 2. Perform ? RequireInternalSlot(relativeTimeFormat,
      //    [[InitializedRelativeTimeFormat]]).
      requireInitializedRelativeTimeFormat(relativeTimeFormat);
      // 3. Let options be OrdinaryObjectCreate(%Object.prototype%).
      var options = {};
      // 4. For each row of Table "Resolved Options of RelativeTimeFormat
      //    Instances", except the header row, in table order, do
      for (var i = 0; i < RELATIVE_TIME_FORMAT_RESOLVED_OPTIONS.length; i++) {
        var row = RELATIVE_TIME_FORMAT_RESOLVED_OPTIONS[i];
        // a. Let propertyKey be the Property value of the current row.
        // b. Let value be the value of relativeTimeFormat's internal slot
        //    whose name is the Internal Slot value of the current row.
        // c. Assert: value is not undefined.
        // d. Perform ! CreateDataPropertyOrThrow(options, propertyKey,
        //    value).
        CreateDataPropertyOrThrow(
          options,
          row[0],
          RelativeTimeFormatSlots.get(relativeTimeFormat, row[1]),
        );
      }
      // 5. Return options.
      return options;
    },
    // Intl.RelativeTimeFormat.prototype.format ( value, unit )
    format(value, unit) {
      // 1. Let relativeTimeFormat be the this value.
      var relativeTimeFormat = this;
      // 2. Perform ? RequireInternalSlot(relativeTimeFormat,
      //    [[InitializedRelativeTimeFormat]]).
      requireInitializedRelativeTimeFormat(relativeTimeFormat);
      // 3. Set value to ? ToNumber(value).
      value = ToNumber(value);
      // 4. Set unit to ? ToString(unit).
      unit = ToString(unit);
      // 5. Return ? FormatRelativeTime(relativeTimeFormat, value, unit).
      return FormatRelativeTime(relativeTimeFormat, value, unit);
    },
    // Intl.RelativeTimeFormat.prototype.formatToParts ( value, unit )
    formatToParts(value, unit) {
      // 1. Let relativeTimeFormat be the this value.
      var relativeTimeFormat = this;
      // 2. Perform ? RequireInternalSlot(relativeTimeFormat,
      //    [[InitializedRelativeTimeFormat]]).
      requireInitializedRelativeTimeFormat(relativeTimeFormat);
      // 3. Set value to ? ToNumber(value).
      value = ToNumber(value);
      // 4. Set unit to ? ToString(unit).
      unit = ToString(unit);
      // 5. Return ? FormatRelativeTimeToParts(relativeTimeFormat, value,
      //    unit).
      return FormatRelativeTimeToParts(relativeTimeFormat, value, unit);
    },
  });
  // 5. Intl.RelativeTimeFormat.prototype [ %Symbol.toStringTag% ]:
  //    "Intl.RelativeTimeFormat", { [[Writable]]: false, [[Enumerable]]:
  //    false, [[Configurable]]: true }.
  setToStringTag(proto, 'Intl.RelativeTimeFormat');
  // 6. "The Intl.RelativeTimeFormat Constructor": the initial value of the
  //    "RelativeTimeFormat" property of the Intl object. ECMA-262 18 gives
  //    such data properties the attributes { [[Writable]]: true,
  //    [[Enumerable]]: false, [[Configurable]]: true }.
  ObjectDefineProperty(intl, 'RelativeTimeFormat', {
    __proto__: null,
    value: RelativeTimeFormat,
    writable: true,
    enumerable: false,
    configurable: true,
  });
}
