/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// ECMA-402 "PluralRules Objects": the Intl.PluralRules constructor, its
// supportedLocalesOf, the prototype's resolvedOptions, select, selectRange
// and @@toStringTag, ResolvePlural, PluralRuleSelect, ResolvePluralRange
// and PluralRuleSelectRange, following the draft https://tc39.es/ecma402/
// dated October 24, 2026. Each instance has native plural rules of its own,
// attached by pluralRulesInit (plan decision 13).

/// %Intl.PluralRules.prototype%; set by installPluralRules.
var PluralRulesPrototype;

/// The internal slots of an Intl.PluralRules instance ("Properties of
/// Intl.PluralRules Instances"), as private fields of the object passed to
/// the constructor. [[InitializedPluralRules]] is the presence of #locale.
/// Two fields are not slots of the draft:
/// - #rounding, the null-prototype record that FormatNumericToString and
///   ComputeExponentForMagnitude read (07-number.js): the slots
///   SetNumberFormatDigitOptions sets, as lowerCamelCase fields, and
///   notation;
/// - #compactExponent, for [[Notation]] "compact", the function from a
///   magnitude to the exponent of the locale's compact patterns of
///   [[CompactDisplay]] (compactExponentOfMagnitude); otherwise undefined.
class PluralRulesSlots extends ReturnOverride {
  /// [[Locale]]
  #locale;
  /// [[Type]]
  #type;
  /// [[Notation]]
  #notation;
  /// [[CompactDisplay]]
  #compactDisplay;
  /// [[MinimumIntegerDigits]]
  #minimumIntegerDigits;
  /// [[MinimumFractionDigits]]
  #minimumFractionDigits;
  /// [[MaximumFractionDigits]]
  #maximumFractionDigits;
  /// [[MinimumSignificantDigits]]
  #minimumSignificantDigits;
  /// [[MaximumSignificantDigits]]
  #maximumSignificantDigits;
  /// [[RoundingType]]
  #roundingType;
  /// [[RoundingIncrement]]
  #roundingIncrement;
  /// [[RoundingMode]]
  #roundingMode;
  /// [[ComputedRoundingPriority]]
  #computedRoundingPriority;
  /// [[TrailingZeroDisplay]]
  #trailingZeroDisplay;
  /// The record of the digit options and notation, for rounding.
  #rounding;
  /// The compact exponent function, for the compact notation.
  #compactExponent;

  /// Attach the slots to o, from the null-prototype record slots whose
  /// fields are named after the slots (a field that is absent is an
  /// undefined slot), with the record rounding and the function
  /// compactExponent.
  constructor(o, slots, rounding, compactExponent) {
    // 1. Add the private fields to o (ReturnOverride returns o).
    super(o);
    // 2. Set each slot from the field of the same name.
    this.#locale = slots.locale;
    this.#type = slots.type;
    this.#notation = slots.notation;
    this.#compactDisplay = slots.compactDisplay;
    this.#minimumIntegerDigits = slots.minimumIntegerDigits;
    this.#minimumFractionDigits = slots.minimumFractionDigits;
    this.#maximumFractionDigits = slots.maximumFractionDigits;
    this.#minimumSignificantDigits = slots.minimumSignificantDigits;
    this.#maximumSignificantDigits = slots.maximumSignificantDigits;
    this.#roundingType = slots.roundingType;
    this.#roundingIncrement = slots.roundingIncrement;
    this.#roundingMode = slots.roundingMode;
    this.#computedRoundingPriority = slots.computedRoundingPriority;
    this.#trailingZeroDisplay = slots.trailingZeroDisplay;
    // 3. Keep the rounding record and the compact exponent function.
    this.#rounding = rounding;
    this.#compactExponent = compactExponent;
  }

  /// Whether o is an Object with an [[InitializedPluralRules]] internal
  /// slot.
  static is(o) {
    // 1. If o is not an Object, return false.
    if (!isObject(o)) return false;
    // 2. Return whether o has the private fields.
    return #locale in o;
  }

  /// The slot name of o, after ECMA-262 RequireInternalSlot(o,
  /// [[InitializedPluralRules]]). name is the slot's name in
  /// lowerCamelCase, or "rounding" or "compactExponent".
  static get(o, name) {
    // 1. Perform ? RequireInternalSlot(o, [[InitializedPluralRules]]).
    requireInitializedPluralRules(o);
    // 2. Return the slot.
    switch (name) {
      case 'locale':
        return o.#locale;
      case 'type':
        return o.#type;
      case 'notation':
        return o.#notation;
      case 'compactDisplay':
        return o.#compactDisplay;
      case 'minimumIntegerDigits':
        return o.#minimumIntegerDigits;
      case 'minimumFractionDigits':
        return o.#minimumFractionDigits;
      case 'maximumFractionDigits':
        return o.#maximumFractionDigits;
      case 'minimumSignificantDigits':
        return o.#minimumSignificantDigits;
      case 'maximumSignificantDigits':
        return o.#maximumSignificantDigits;
      case 'roundingType':
        return o.#roundingType;
      case 'roundingIncrement':
        return o.#roundingIncrement;
      case 'roundingMode':
        return o.#roundingMode;
      case 'computedRoundingPriority':
        return o.#computedRoundingPriority;
      case 'trailingZeroDisplay':
        return o.#trailingZeroDisplay;
      case 'rounding':
        return o.#rounding;
      case 'compactExponent':
        return o.#compactExponent;
    }
    throw new _TypeError('Intl internal error: no PluralRules slot ' + name);
  }
}

/// ECMA-262 RequireInternalSlot(o, [[InitializedPluralRules]]).
function requireInitializedPluralRules(o) {
  // 1. If o is not an Object, or has no [[InitializedPluralRules]] internal
  //    slot, throw a TypeError exception.
  if (!PluralRulesSlots.is(o))
    throw new _TypeError('Receiver is not an Intl.PluralRules object');
}

/// The one record of %Intl.PluralRules%.[[LocaleData]].[[<locale>]]: there
/// are no relevant extension keys, so ResolveLocale reads no field of it.
var PLURAL_RULES_LOCALE_DATA = {__proto__: null};

/// %Intl.PluralRules%.[[LocaleData]].[[<locale>]] for an available locale.
function pluralRulesLocaleData(locale) {
  // 1. Return the empty record.
  return PLURAL_RULES_LOCALE_DATA;
}

/// The fields of %Intl.PluralRules% that ResolveOptions reads ("Internal
/// slots"): [[AvailableLocales]], the plugin's one list (decision 17),
/// [[RelevantExtensionKeys]] « » and [[ResolutionOptionDescriptors]] « ».
var PLURAL_RULES_SERVICE = {
  __proto__: null,
  availableLocales: isAvailableLocale,
  relevantExtensionKeys: [],
  resolutionOptionDescriptors: [],
};

/// The compact exponent function of the compact notation for [[Locale]]
/// locale and [[CompactDisplay]] compactDisplay: compactExponentOfMagnitude
/// of the locale's compact table of that display. PluralRules has no
/// numbering system, so the table is the one of the locale's default
/// numbering system; the magnitudes and exponents of compact patterns
/// depend only on the locale and the display.
function pluralRulesCompactExponent(locale, compactDisplay) {
  // 1. Let languageId be GetLocaleBaseName(locale), the language identifier
  //    native code looks the data up by (decision 7).
  var languageId = GetLocaleBaseName(locale);
  // 2. Let nu be the locale's default numbering system: the first entry of
  //    %Intl.NumberFormat%.[[LocaleData]].[[<locale>]].[[nu]] (decision 17),
  //    which is cached. locale is an available locale, since PluralRules has
  //    no relevant extension keys.
  var nu = numberFormatLocaleData(locale).nu[0];
  // 3. Return the exponent function of the compact table of languageId, nu
  //    and compactDisplay.
  return compactExponentOfMagnitude(
    cachedCompactTable(languageId, nu, compactDisplay === 'long'),
  );
}

// Intl.PluralRules ( [ locales [ , options ] ] )
function PluralRules(locales = undefined, options = undefined) {
  // 1. If NewTarget is undefined, throw a TypeError exception.
  if (new.target === undefined)
    throw new _TypeError('Intl.PluralRules must be called with new');
  // 2. Let pluralRules be ? OrdinaryCreateFromConstructor(NewTarget,
  //    "%Intl.PluralRules.prototype%", « [[InitializedPluralRules]],
  //    [[Locale]], [[Type]], [[Notation]], [[CompactDisplay]],
  //    [[MinimumIntegerDigits]], [[MinimumFractionDigits]],
  //    [[MaximumFractionDigits]], [[MinimumSignificantDigits]],
  //    [[MaximumSignificantDigits]], [[RoundingType]],
  //    [[RoundingIncrement]], [[RoundingMode]],
  //    [[ComputedRoundingPriority]], [[TrailingZeroDisplay]] »).
  //    Hermes created this from NewTarget before the call (design spec
  //    "Constructors"). The slots are collected in a null-prototype record
  //    and attached at the end, with the native plural rules, once every
  //    value is known; nothing between can observe them.
  var pluralRules = this;
  var slots = {__proto__: null};
  // 3. Let optionsResolution be ? ResolveOptions(%Intl.PluralRules%,
  //    %Intl.PluralRules%.[[LocaleData]], locales, options,
  //    « coerce-options »).
  var optionsResolution = ResolveOptions(
    PLURAL_RULES_SERVICE,
    pluralRulesLocaleData,
    locales,
    options,
    ['coerce-options'],
  );
  // 4. Set options to optionsResolution.[[Options]].
  options = optionsResolution.options;
  // 5. Let resolvedLocale be optionsResolution.[[ResolvedLocale]].
  var resolvedLocale = optionsResolution.resolvedLocale;
  // 6. Set pluralRules.[[Locale]] to resolvedLocale.[[Locale]].
  slots.locale = resolvedLocale.locale;
  // 7. Let type be ? GetOption(options, "type", string, « "cardinal",
  //    "ordinal" », "cardinal").
  var type = GetOption(
    options,
    'type',
    'string',
    ['cardinal', 'ordinal'],
    'cardinal',
  );
  // 8. Set pluralRules.[[Type]] to type.
  slots.type = type;
  // 9. Let notation be ? GetOption(options, "notation", string,
  //    « "standard", "scientific", "engineering", "compact" »,
  //    "standard").
  var notation = GetOption(
    options,
    'notation',
    'string',
    ['standard', 'scientific', 'engineering', 'compact'],
    'standard',
  );
  // 10. Set pluralRules.[[Notation]] to notation.
  slots.notation = notation;
  // 11. Let compactDisplay be ? GetOption(options, "compactDisplay",
  //     string, « "short", "long" », "short").
  var compactDisplay = GetOption(
    options,
    'compactDisplay',
    'string',
    ['short', 'long'],
    'short',
  );
  // 12. If notation is "compact", then
  //     a. Set pluralRules.[[CompactDisplay]] to compactDisplay.
  if (notation === 'compact') slots.compactDisplay = compactDisplay;
  // 13. Perform ? SetNumberFormatDigitOptions(pluralRules, options, 0, 3,
  //     notation).
  SetNumberFormatDigitOptions(slots, options, 0, 3, notation);
  //     Attach the slots, the rounding record and, for the compact
  //     notation, the compact exponent function; then the native plural
  //     rules of the language identifier of [[Locale]] and [[Type]].
  //     pluralRules is a new object, so it has no native state yet.
  new PluralRulesSlots(
    pluralRules,
    slots,
    numberFormatRounding(slots),
    notation === 'compact'
      ? pluralRulesCompactExponent(slots.locale, compactDisplay)
      : undefined,
  );
  native.pluralRulesInit(
    pluralRules,
    GetLocaleBaseName(slots.locale),
    type === 'ordinal',
  );
  // 14. Return pluralRules.
  return pluralRules;
}

/// The magnitude of the value of decimalString, ASCII digits with at most
/// one "." between digits (FormatNumericToString's [[FormattedString]]):
/// floor(log10(value)), from the position of its first non-zero digit
/// relative to the decimal point. undefined if the value is 0.
function decimalStringMagnitude(decimalString) {
  // 1. Let point be the index of "." in decimalString, or its length if it
  //    has none.
  var point = stringIndexOf(decimalString, '.', 0);
  if (point === -1) point = decimalString.length;
  // 2. Find the first digit that is not "0".
  for (var i = 0; i < decimalString.length; i++) {
    var c = charCodeAt(decimalString, i);
    if (c === 0x30 || c === 0x2e) continue;
    // 3. A digit before the point at index i has the magnitude
    //    point - i - 1, one after it point - i.
    return i < point ? point - i - 1 : point - i;
  }
  // 4. Every digit is "0": the value is 0.
  return undefined;
}

/// decimalString with its decimal point moved k places left, for an
/// integer k > 0 no greater than the magnitude of its value, so that the
/// point stays after the first digit (decision 13: an exact digit shift,
/// which keeps every digit and the value's digit count after the point
/// once the exponent k moves it back). decimalString is as for
/// decimalStringMagnitude.
function shiftDecimalPointLeft(decimalString, k) {
  // 1. Let intDigits be the digits before the point, and digits all the
  //    digits.
  var dot = stringIndexOf(decimalString, '.', 0);
  var intDigits =
    dot === -1 ? decimalString : stringSlice(decimalString, 0, dot);
  var digits =
    dot === -1
      ? decimalString
      : intDigits + stringSlice(decimalString, dot + 1);
  // 2. Let point be the new position of the point: k places left of the
  //    old one. k is at most the magnitude, so 1 <= point <
  //    intDigits.length.
  var point = intDigits.length - k;
  // 3. Return the digits with the point at point.
  return stringSlice(digits, 0, point) + '.' + stringSlice(digits, point);
}

// PluralRuleSelect ( locale, type, notation, compactDisplay, decimalString )
// Implementation-defined (decision 13). The native plural rules attached
// to pluralRules are those of its [[Locale]] (locale) and [[Type]] (type),
// its rounding record has its [[Notation]] (notation), and its compact
// exponent function is the one of [[CompactDisplay]] (compactDisplay), so
// this takes pluralRules for those four.
function PluralRuleSelect(pluralRules, decimalString) {
  // 1. Let exponent be the exponent of notation for the value of
  //    decimalString: 0 if the value is 0; otherwise
  //    ComputeExponentForMagnitude for its magnitude (the magnitude for
  //    "scientific", the magnitude rounded down to a multiple of 3 for
  //    "engineering", the compact patterns' exponent for "compact", and 0
  //    for "standard").
  var magnitude = decimalStringMagnitude(decimalString);
  var exponent =
    magnitude === undefined
      ? 0
      : ComputeExponentForMagnitude(
          PluralRulesSlots.get(pluralRules, 'rounding'),
          magnitude,
          PluralRulesSlots.get(pluralRules, 'compactExponent'),
        );
  // 2. If exponent is not positive, the category is that of decimalString
  //    itself: CLDR's c (and e) operand is 0 then. Moving the point right
  //    by an engineering exponent below the magnitude ("0.0001" and -6)
  //    would need zeros the value does not have, which the operands would
  //    count as fraction digits.
  if (exponent <= 0) return native.pluralSelect(pluralRules, decimalString, 0);
  // 3. Otherwise return the category of decimalString with its point moved
  //    exponent places left, times 10^exponent: the same value, with c
  //    (and e) = exponent.
  return native.pluralSelect(
    pluralRules,
    shiftDecimalPointLeft(decimalString, exponent),
    exponent,
  );
}

// ResolvePlural ( pluralRules, n )
// n is an Intl mathematical value. Returns the null-prototype record
// { pluralCategory, formattedString }.
function ResolvePlural(pluralRules, n) {
  // 1. If n is not-a-number, then
  //    a. Let formattedString be an ILD String value indicating the NaN
  //       value.
  //    b. Return the Record { [[PluralCategory]]: "other",
  //       [[FormattedString]]: formattedString }.
  if (n === 'nan')
    return {__proto__: null, pluralCategory: 'other', formattedString: 'NaN'};
  // 2. If n is positive-infinity, then
  //    a. Let formattedString be an ILD String value indicating positive
  //       infinity.
  //    b. Return the Record { [[PluralCategory]]: "other",
  //       [[FormattedString]]: formattedString }.
  if (n === 'posInf')
    return {
      __proto__: null,
      pluralCategory: 'other',
      formattedString: 'Infinity',
    };
  // 3. If n is negative-infinity, then
  //    a. Let formattedString be an ILD String value indicating negative
  //       infinity.
  //    b. Return the Record { [[PluralCategory]]: "other",
  //       [[FormattedString]]: formattedString }.
  if (n === 'negInf')
    return {
      __proto__: null,
      pluralCategory: 'other',
      formattedString: '-Infinity',
    };
  // 4. Let res be FormatNumericToString(pluralRules, n).
  var res = FormatNumericToString(
    PluralRulesSlots.get(pluralRules, 'rounding'),
    n,
  );
  // 5. Let formattedString be res.[[FormattedString]].
  var formattedString = res.formattedString;
  // 6. Let locale be pluralRules.[[Locale]].
  // 7. Let type be pluralRules.[[Type]].
  // 8. Let notation be pluralRules.[[Notation]].
  // 9. Let compactDisplay be pluralRules.[[CompactDisplay]].
  //    PluralRuleSelect reads locale, type, notation and compactDisplay
  //    through pluralRules.
  // 10. Let pluralCategory be PluralRuleSelect(locale, type, notation,
  //     compactDisplay, formattedString).
  var pluralCategory = PluralRuleSelect(pluralRules, formattedString);
  // 11. Return the Record { [[PluralCategory]]: pluralCategory,
  //     [[FormattedString]]: formattedString }.
  return {
    __proto__: null,
    pluralCategory: pluralCategory,
    formattedString: formattedString,
  };
}

// PluralRuleSelectRange ( locale, type, notation, compactDisplay, xp, yp )
// Implementation-defined: CLDR's plural ranges of locale, which ICU4X keeps
// with the native plural rules of pluralRules (locale and type). They do
// not depend on notation or compactDisplay.
function PluralRuleSelectRange(pluralRules, xp, yp) {
  // 1. Return the category CLDR's plural ranges give for xp to yp.
  return native.pluralSelectRange(pluralRules, xp, yp);
}

// ResolvePluralRange ( pluralRules, x, y )
function ResolvePluralRange(pluralRules, x, y) {
  // 1. If x is not-a-number or y is not-a-number, throw a RangeError
  //    exception.
  if (x === 'nan' || y === 'nan')
    throw new _RangeError('selectRange: start and end must not be NaN');
  // 2. Let xResult be ResolvePlural(pluralRules, x).
  var xResult = ResolvePlural(pluralRules, x);
  // 3. Let yResult be ResolvePlural(pluralRules, y).
  var yResult = ResolvePlural(pluralRules, y);
  // 4. If xResult.[[FormattedString]] is yResult.[[FormattedString]], then
  //    a. Return xResult.[[PluralCategory]].
  if (xResult.formattedString === yResult.formattedString)
    return xResult.pluralCategory;
  // 5. Let locale be pluralRules.[[Locale]].
  // 6. Let type be pluralRules.[[Type]].
  // 7. Let notation be pluralRules.[[Notation]].
  // 8. Let compactDisplay be pluralRules.[[CompactDisplay]].
  //    PluralRuleSelectRange reads locale and type through pluralRules.
  // 9. Return PluralRuleSelectRange(locale, type, notation, compactDisplay,
  //    xResult.[[PluralCategory]], yResult.[[PluralCategory]]).
  return PluralRuleSelectRange(
    pluralRules,
    xResult.pluralCategory,
    yResult.pluralCategory,
  );
}

/// The rows of the table "Resolved Options of PluralRules Instances", in
/// table order: [Property, slot name], the slot name undefined for
/// "pluralCategories". Every slot with the Conversion number already holds
/// a Number.
var PLURAL_RULES_RESOLVED_OPTIONS = [
  ['locale', 'locale'],
  ['type', 'type'],
  ['notation', 'notation'],
  ['compactDisplay', 'compactDisplay'],
  ['minimumIntegerDigits', 'minimumIntegerDigits'],
  ['minimumFractionDigits', 'minimumFractionDigits'],
  ['maximumFractionDigits', 'maximumFractionDigits'],
  ['minimumSignificantDigits', 'minimumSignificantDigits'],
  ['maximumSignificantDigits', 'maximumSignificantDigits'],
  ['pluralCategories', undefined],
  ['roundingIncrement', 'roundingIncrement'],
  ['roundingMode', 'roundingMode'],
  ['roundingPriority', 'computedRoundingPriority'],
  ['trailingZeroDisplay', 'trailingZeroDisplay'],
];

/// Install Intl.PluralRules on intl; called by installIntl.
function installPluralRules(intl) {
  // 1. Let proto be %Intl.PluralRules.prototype%, an ordinary object whose
  //    [[Prototype]] is %Object.prototype% ("Properties of the
  //    Intl.PluralRules Prototype Object").
  var proto = {};
  PluralRulesPrototype = proto;
  // 2. "Properties of the Intl.PluralRules Constructor":
  //    Intl.PluralRules.prototype, with its length 0, and
  //    Intl.PluralRules.prototype.constructor.
  defineConstructor(PluralRules, 0, proto);
  // 3. The method properties of the constructor.
  defineMethods(PluralRules, {
    // Intl.PluralRules.supportedLocalesOf ( locales [ , options ] )
    supportedLocalesOf(locales, options = undefined) {
      // 1. Let availableLocales be %Intl.PluralRules%.[[AvailableLocales]].
      // 2. Let requestedLocales be ? CanonicalizeLocaleList(locales).
      var requestedLocales = CanonicalizeLocaleList(locales);
      // 3. Return ? FilterLocales(availableLocales, requestedLocales,
      //    options).
      return FilterLocales(isAvailableLocale, requestedLocales, options);
    },
  });
  // 4. The method properties of the prototype, in the draft's order.
  defineMethods(proto, {
    // Intl.PluralRules.prototype.resolvedOptions ( )
    resolvedOptions() {
      // 1. Let pr be the this value.
      var pr = this;
      // 2. Perform ? RequireInternalSlot(pr, [[InitializedPluralRules]]).
      requireInitializedPluralRules(pr);
      // 3. Let options be OrdinaryObjectCreate(%Object.prototype%).
      var options = {};
      // 4. Let pluralCategories be a List of Strings containing all
      //    possible results of PluralRuleSelect for the selected locale
      //    pr.[[Locale]], sorted according to the following order: "zero",
      //    "one", "two", "few", "many", "other". The native plural rules
      //    of pr give them in that order.
      var pluralCategories = native.pluralCategories(pr);
      // 5. For each row of Table "Resolved Options of PluralRules
      //    Instances", except the header row, in table order, do
      for (var i = 0; i < PLURAL_RULES_RESOLVED_OPTIONS.length; i++) {
        var row = PLURAL_RULES_RESOLVED_OPTIONS[i];
        // a. Let propertyKey be the Property value of the current row.
        var propertyKey = row[0];
        var value;
        if (propertyKey === 'pluralCategories') {
          // b. If propertyKey is "pluralCategories", then
          //    i. Let value be CreateArrayFromList(pluralCategories).
          value = CreateArrayFromList(pluralCategories);
        } else {
          // c. Else,
          //    i. Let value be the value of pr's internal slot whose name
          //       is the Internal Slot value of the current row.
          value = PluralRulesSlots.get(pr, row[1]);
        }
        // d. If value is not undefined, then
        if (value !== undefined) {
          // i. If there is a Conversion value in the current row, then
          //    1. Assert: The Conversion value of the current row is
          //       number.
          //    2. Set value to 𝔽(value).
          // ii. Perform ! CreateDataPropertyOrThrow(options, propertyKey,
          //     value).
          CreateDataPropertyOrThrow(options, propertyKey, value);
        }
      }
      // 6. Return options.
      return options;
    },
    // Intl.PluralRules.prototype.select ( value )
    select(value) {
      // 1. Let pr be the this value.
      var pr = this;
      // 2. Perform ? RequireInternalSlot(pr, [[InitializedPluralRules]]).
      requireInitializedPluralRules(pr);
      // 3. Let n be ? ToIntlMathematicalValue(value).
      var n = ToIntlMathematicalValue(value);
      // 4. Return ResolvePlural(pr, n).[[PluralCategory]].
      return ResolvePlural(pr, n).pluralCategory;
    },
    // Intl.PluralRules.prototype.selectRange ( start, end )
    selectRange(start, end) {
      // 1. Let pr be the this value.
      var pr = this;
      // 2. Perform ? RequireInternalSlot(pr, [[InitializedPluralRules]]).
      requireInitializedPluralRules(pr);
      // 3. If start is undefined or end is undefined, throw a TypeError
      //    exception.
      if (start === undefined || end === undefined)
        throw new _TypeError('start and end are required');
      // 4. Let x be ? ToIntlMathematicalValue(start).
      var x = ToIntlMathematicalValue(start);
      // 5. Let y be ? ToIntlMathematicalValue(end).
      var y = ToIntlMathematicalValue(end);
      // 6. Return ? ResolvePluralRange(pr, x, y).
      return ResolvePluralRange(pr, x, y);
    },
  });
  // 5. Intl.PluralRules.prototype [ %Symbol.toStringTag% ]:
  //    "Intl.PluralRules", { [[Writable]]: false, [[Enumerable]]: false,
  //    [[Configurable]]: true }.
  setToStringTag(proto, 'Intl.PluralRules');
  // 6. "The Intl.PluralRules Constructor": the initial value of the
  //    "PluralRules" property of the Intl object. ECMA-262 18 gives such
  //    data properties the attributes { [[Writable]]: true,
  //    [[Enumerable]]: false, [[Configurable]]: true }.
  ObjectDefineProperty(intl, 'PluralRules', {
    __proto__: null,
    value: PluralRules,
    writable: true,
    enumerable: false,
    configurable: true,
  });
}
