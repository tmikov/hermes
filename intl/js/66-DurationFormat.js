/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// ECMA-402 "DurationFormat Objects": the Intl.DurationFormat constructor,
// its supportedLocalesOf, the prototype's resolvedOptions, format,
// formatToParts and @@toStringTag, and the abstract operations from
// ToIntegerIfIntegral to PartitionDurationFormatPattern, following the
// draft https://tc39.es/ecma402/ dated October 24, 2026. An instance has no
// native state. Its numbers come from internal NumberFormats made with
// CreateNumberFormat and its list from an internal ListFormat made with
// CreateListFormat (Phase 5 plan decisions 7 and 8), never through
// Intl.NumberFormat or Intl.ListFormat. Durations are formatted with their
// exact values: integral fields as BigInts, and the sums of IsValidDuration
// and ComputeFractionalDigits in BigInt nanoseconds (decision 7).

/// %Intl.DurationFormat.prototype%; set by installDurationFormat.
var DurationFormatPrototype;

/// The internal slots of an Intl.DurationFormat instance ("Properties of
/// Intl.DurationFormat Instances"), as private fields of the object passed
/// to the constructor. [[InitializedDurationFormat]] is the presence of
/// #locale. #unitOptions holds the ten slots [[YearsOptions]] to
/// [[NanosecondsOptions]], a List in the order of DURATION_FORMAT_UNITS.
/// #numberFormats and #listFormat are not slots of the draft: they keep the
/// NumberFormats and the ListFormat that format makes, each made on first
/// use (decision 7). Making them is not observable, so keeping them is.
class DurationFormatSlots extends ReturnOverride {
  /// [[Locale]]
  #locale;
  /// [[NumberingSystem]]
  #numberingSystem;
  /// [[Style]]
  #style;
  /// [[YearsOptions]] to [[NanosecondsOptions]], in table order.
  #unitOptions;
  /// [[HourMinuteSeparator]]
  #hourMinuteSeparator;
  /// [[MinuteSecondSeparator]]
  #minuteSecondSeparator;
  /// [[FractionalDigits]]
  #fractionalDigits;
  /// The NumberFormats made so far, a null-prototype record keyed by
  /// durationNumberFormat's key of their options.
  #numberFormats;
  /// The ListFormat of ListFormatParts, or undefined before its first use.
  #listFormat;

  /// Attach the slots to o, from the null-prototype record slots whose
  /// fields are named after the slots.
  constructor(o, slots) {
    // 1. Add the private fields to o (ReturnOverride returns o).
    super(o);
    // 2. Set each slot from the field of the same name.
    this.#locale = slots.locale;
    this.#numberingSystem = slots.numberingSystem;
    this.#style = slots.style;
    this.#unitOptions = slots.unitOptions;
    this.#hourMinuteSeparator = slots.hourMinuteSeparator;
    this.#minuteSecondSeparator = slots.minuteSecondSeparator;
    this.#fractionalDigits = slots.fractionalDigits;
    // 3. No NumberFormat and no ListFormat is made yet.
    this.#numberFormats = {__proto__: null};
    this.#listFormat = undefined;
  }

  /// Whether o is an Object with an [[InitializedDurationFormat]] internal
  /// slot.
  static is(o) {
    // 1. If o is not an Object, return false.
    if (!isObject(o)) return false;
    // 2. Return whether o has the private fields.
    return #locale in o;
  }

  /// The slot name of o, after ECMA-262 RequireInternalSlot(o,
  /// [[InitializedDurationFormat]]). name is the slot's name in
  /// lowerCamelCase, "unitOptions", "numberFormats" or "listFormat".
  static get(o, name) {
    // 1. Perform ? RequireInternalSlot(o, [[InitializedDurationFormat]]).
    requireInitializedDurationFormat(o);
    // 2. Return the slot.
    switch (name) {
      case 'locale':
        return o.#locale;
      case 'numberingSystem':
        return o.#numberingSystem;
      case 'style':
        return o.#style;
      case 'unitOptions':
        return o.#unitOptions;
      case 'hourMinuteSeparator':
        return o.#hourMinuteSeparator;
      case 'minuteSecondSeparator':
        return o.#minuteSecondSeparator;
      case 'fractionalDigits':
        return o.#fractionalDigits;
      case 'numberFormats':
        return o.#numberFormats;
      case 'listFormat':
        return o.#listFormat;
    }
    throw new _TypeError(
      'Intl internal error: no DurationFormat slot ' + name,
    );
  }

  /// Keep listFormat as the ListFormat of the DurationFormat o.
  static setListFormat(o, listFormat) {
    // 1. Perform ? RequireInternalSlot(o, [[InitializedDurationFormat]]).
    requireInitializedDurationFormat(o);
    // 2. Keep listFormat.
    o.#listFormat = listFormat;
  }
}

/// ECMA-262 RequireInternalSlot(o, [[InitializedDurationFormat]]).
function requireInitializedDurationFormat(o) {
  // 1. If o is not an Object, or has no [[InitializedDurationFormat]]
  //    internal slot, throw a TypeError exception.
  if (!DurationFormatSlots.is(o))
    throw new _TypeError('Receiver is not an Intl.DurationFormat object');
}

/// The fields of %Intl.DurationFormat% that ResolveOptions reads
/// ("Internal slots"): [[AvailableLocales]], the plugin's one list,
/// [[RelevantExtensionKeys]] « "nu" » and [[ResolutionOptionDescriptors]].
/// [[LocaleData]].[[<locale>]].[[nu]] is NumberFormat's list, so
/// numberFormatLocaleData serves as [[LocaleData]]; it has no "native",
/// "traditio" or "finance". [[LocaleData]].[[<locale>]].[[DigitalFormat]]
/// is durationDigitalFormat's.
var DURATION_FORMAT_SERVICE = {
  __proto__: null,
  availableLocales: isAvailableLocale,
  relevantExtensionKeys: ['nu'],
  resolutionOptionDescriptors: [
    {__proto__: null, key: 'nu', property: 'numberingSystem'},
  ],
};

/// The Styles of the rows years to days.
var DURATION_DATE_STYLES = ['long', 'short', 'narrow'];
/// The Styles of the rows hours to seconds.
var DURATION_TIME_STYLES = ['long', 'short', 'narrow', 'numeric', '2-digit'];
/// The Styles of the rows milliseconds to nanoseconds.
var DURATION_SUBSECOND_STYLES = ['long', 'short', 'narrow', 'numeric'];

/// The rows of Table "Internal slots and property names of DurationFormat
/// instances" and of Table "DurationFormat instance internal slots and
/// properties relevant to PartitionDurationFormatPattern", which have the
/// same units in the same order: null-prototype records { unit, styles,
/// digitalBase, numberFormatUnit }. A row's Internal Slot is its index in
/// #unitOptions, and its Value Field the Duration Record field named unit.
var DURATION_FORMAT_UNITS = [
  {
    __proto__: null,
    unit: 'years',
    styles: DURATION_DATE_STYLES,
    digitalBase: 'short',
    numberFormatUnit: 'year',
  },
  {
    __proto__: null,
    unit: 'months',
    styles: DURATION_DATE_STYLES,
    digitalBase: 'short',
    numberFormatUnit: 'month',
  },
  {
    __proto__: null,
    unit: 'weeks',
    styles: DURATION_DATE_STYLES,
    digitalBase: 'short',
    numberFormatUnit: 'week',
  },
  {
    __proto__: null,
    unit: 'days',
    styles: DURATION_DATE_STYLES,
    digitalBase: 'short',
    numberFormatUnit: 'day',
  },
  {
    __proto__: null,
    unit: 'hours',
    styles: DURATION_TIME_STYLES,
    digitalBase: 'numeric',
    numberFormatUnit: 'hour',
  },
  {
    __proto__: null,
    unit: 'minutes',
    styles: DURATION_TIME_STYLES,
    digitalBase: 'numeric',
    numberFormatUnit: 'minute',
  },
  {
    __proto__: null,
    unit: 'seconds',
    styles: DURATION_TIME_STYLES,
    digitalBase: 'numeric',
    numberFormatUnit: 'second',
  },
  {
    __proto__: null,
    unit: 'milliseconds',
    styles: DURATION_SUBSECOND_STYLES,
    digitalBase: 'numeric',
    numberFormatUnit: 'millisecond',
  },
  {
    __proto__: null,
    unit: 'microseconds',
    styles: DURATION_SUBSECOND_STYLES,
    digitalBase: 'numeric',
    numberFormatUnit: 'microsecond',
  },
  {
    __proto__: null,
    unit: 'nanoseconds',
    styles: DURATION_SUBSECOND_STYLES,
    digitalBase: 'numeric',
    numberFormatUnit: 'nanosecond',
  },
];

/// The index of the row "hours" in DURATION_FORMAT_UNITS.
var DURATION_HOURS = 4;
/// The index of the row "minutes" in DURATION_FORMAT_UNITS.
var DURATION_MINUTES = 5;
/// The index of the row "seconds" in DURATION_FORMAT_UNITS.
var DURATION_SECONDS = 6;
/// The index of the row "milliseconds" in DURATION_FORMAT_UNITS.
var DURATION_MILLISECONDS = 7;
/// The index of the row "microseconds" in DURATION_FORMAT_UNITS.
var DURATION_MICROSECONDS = 8;
/// The index of the row "nanoseconds" in DURATION_FORMAT_UNITS.
var DURATION_NANOSECONDS = 9;

/// %Intl.DurationFormat%.[[LocaleData]].[[<locale>]].[[DigitalFormat]]
/// for the numbering system nu: a null-prototype record
/// { hourMinuteSeparator, minuteSecondSeparator, twoDigitHours }. The
/// draft keys [[DigitalFormat]] by numbering system but its constructor
/// never picks an entry; the plugin reads the entry of the resolved
/// numbering system (decision 7). Both separators are CLDR's
/// timeSeparator for the language identifier of locale and nu, entry 21
/// of numberSymbols. [[TwoDigitHours]] is false for every locale: the
/// draft's GetDurationUnitOptions step 8 would make the hours "2-digit"
/// in every style, so a true value would break the other styles
/// (decision 7).
function durationDigitalFormat(locale, nu) {
  // 1. Let separator be the time separator of locale and nu.
  var separator = cachedNumberSymbols(GetLocaleBaseName(locale), nu)[21];
  // 2. Return the record.
  return {
    __proto__: null,
    hourMinuteSeparator: separator,
    minuteSecondSeparator: separator,
    twoDigitHours: false,
  };
}

/// Steps 3 to 17 of Intl.DurationFormat ( [ locales [ , options ] ] ), on
/// the object durationFormat that steps 1 and 2 made. The slots are
/// collected in a null-prototype record and attached at the end, once
/// every value is known; nothing between can observe them. Returns
/// durationFormat.
function initializeDurationFormat(durationFormat, locales, options) {
  var slots = {__proto__: null};
  // 3. Let optionsResolution be ? ResolveOptions(%Intl.DurationFormat%,
  //    %Intl.DurationFormat%.[[LocaleData]], locales, options).
  var optionsResolution = ResolveOptions(
    DURATION_FORMAT_SERVICE,
    numberFormatLocaleData,
    locales,
    options,
  );
  // 4. Set options to optionsResolution.[[Options]].
  options = optionsResolution.options;
  // 5. Let resolvedLocale be optionsResolution.[[ResolvedLocale]].
  var resolvedLocale = optionsResolution.resolvedLocale;
  // 6. Set durationFormat.[[Locale]] to resolvedLocale.[[Locale]].
  slots.locale = resolvedLocale.locale;
  // 7. Let resolvedLocaleData be resolvedLocale.[[LocaleData]].
  // 8. Let digitalFormat be resolvedLocaleData.[[DigitalFormat]].
  //    The entry of resolvedLocale.[[nu]] (durationDigitalFormat).
  var digitalFormat = durationDigitalFormat(
    resolvedLocale.locale,
    resolvedLocale.nu,
  );
  // 9. Set durationFormat.[[HourMinuteSeparator]] to
  //    digitalFormat.[[HourMinuteSeparator]].
  slots.hourMinuteSeparator = digitalFormat.hourMinuteSeparator;
  // 10. Set durationFormat.[[MinuteSecondSeparator]] to
  //     digitalFormat.[[MinuteSecondSeparator]].
  slots.minuteSecondSeparator = digitalFormat.minuteSecondSeparator;
  // 11. Set durationFormat.[[NumberingSystem]] to resolvedLocale.[[nu]].
  slots.numberingSystem = resolvedLocale.nu;
  // 12. Let style be ? GetOption(options, "style", string, « "long",
  //     "short", "narrow", "digital" », "short").
  var style = GetOption(
    options,
    'style',
    'string',
    ['long', 'short', 'narrow', 'digital'],
    'short',
  );
  // 13. Set durationFormat.[[Style]] to style.
  slots.style = style;
  // 14. Let prevStyle be the empty String.
  var prevStyle = '';
  // 15. For each row of Table "Internal slots and property names of
  //     DurationFormat instances", except the header row, in table order,
  //     do
  var unitOptionsList = [];
  for (var i = 0; i < DURATION_FORMAT_UNITS.length; i++) {
    var row = DURATION_FORMAT_UNITS[i];
    // a. Let slot be the Internal Slot value of the current row.
    // b. Let unit be the Unit value of the current row.
    var unit = row.unit;
    // c. Let styles be the Styles value of the current row.
    // d. Let digitalBase be the Digital Default value of the current row.
    // e. Let unitOptions be ? GetDurationUnitOptions(unit, options, style,
    //    styles, digitalBase, prevStyle, digitalFormat.[[TwoDigitHours]]).
    var unitOptions = GetDurationUnitOptions(
      unit,
      options,
      style,
      row.styles,
      row.digitalBase,
      prevStyle,
      digitalFormat.twoDigitHours,
    );
    // f. Set the value of durationFormat's internal slot whose name is slot
    //    to unitOptions.
    listPush(unitOptionsList, unitOptions);
    // g. If unit is one of "hours", "minutes", "seconds", "milliseconds",
    //    or "microseconds", then
    //    i. Set prevStyle to unitOptions.[[Style]].
    if (
      unit === 'hours' ||
      unit === 'minutes' ||
      unit === 'seconds' ||
      unit === 'milliseconds' ||
      unit === 'microseconds'
    )
      prevStyle = unitOptions.style;
  }
  slots.unitOptions = unitOptionsList;
  // 16. Set durationFormat.[[FractionalDigits]] to ? GetNumberOption(
  //     options, "fractionalDigits", 0, 9, undefined).
  slots.fractionalDigits = GetNumberOption(
    options,
    'fractionalDigits',
    0,
    9,
    undefined,
  );
  new DurationFormatSlots(durationFormat, slots);
  // 17. Return durationFormat.
  return durationFormat;
}

// Intl.DurationFormat ( [ locales [ , options ] ] )
function DurationFormat(locales = undefined, options = undefined) {
  // 1. If NewTarget is undefined, throw a TypeError exception.
  if (new.target === undefined)
    throw new _TypeError('Intl.DurationFormat must be called with new');
  // 2. Let durationFormat be ? OrdinaryCreateFromConstructor(NewTarget,
  //    "%Intl.DurationFormatPrototype%", « [[InitializedDurationFormat]],
  //    [[Locale]], [[NumberingSystem]], [[Style]], [[YearsOptions]],
  //    [[MonthsOptions]], [[WeeksOptions]], [[DaysOptions]],
  //    [[HoursOptions]], [[MinutesOptions]], [[SecondsOptions]],
  //    [[MillisecondsOptions]], [[MicrosecondsOptions]],
  //    [[NanosecondsOptions]], [[HourMinuteSeparator]],
  //    [[MinuteSecondSeparator]], [[FractionalDigits]] »).
  //    Hermes created this from NewTarget before the call (design spec
  //    "Constructors"); initializeDurationFormat attaches the slots.
  // 3-17. The steps that resolve the slots, numbered there.
  return initializeDurationFormat(this, locales, options);
}

// ToIntegerIfIntegral ( arg )
function ToIntegerIfIntegral(arg) {
  // 1. Let number be ? ToNumber(arg).
  var number = ToNumber(arg);
  // 2. If number is not an integral Number, throw a RangeError exception.
  if (!NumberIsFinite(number) || MathTrunc(number) !== number)
    throw new _RangeError('Invalid duration value: ' + ToString(number));
  // 3. Return ℝ(number). ℝ(-0𝔽) is the mathematical 0: adding +0 turns -0
  //    into +0 and leaves every other value unchanged (decision 7). This
  //    is not observable today, since BigInt(-0) is 0n and every consumer
  //    of a field compares it with 0; it is kept as the literal
  //    transcription of ℝ.
  return number + 0;
}

// ToDurationRecord ( input )
// Returns a null-prototype record whose fields are named after the units
// (years to nanoseconds), each an integral Number.
function ToDurationRecord(input) {
  // 1. If input is not an Object, then
  if (!isObject(input)) {
    // a. If input is a String, throw a RangeError exception.
    if (typeof input === 'string')
      throw new _RangeError('Duration strings are not supported');
    // b. Throw a TypeError exception.
    throw new _TypeError('Duration must be an object');
  }
  // 2. Let result be a new Duration Record with each field set to 0.
  var result = {
    __proto__: null,
    years: 0,
    months: 0,
    weeks: 0,
    days: 0,
    hours: 0,
    minutes: 0,
    seconds: 0,
    milliseconds: 0,
    microseconds: 0,
    nanoseconds: 0,
  };
  // 3. Let days be ? Get(input, "days").
  var days = input.days;
  // 4. If days is not undefined, set result.[[Days]] to ?
  //    ToIntegerIfIntegral(days).
  if (days !== undefined) result.days = ToIntegerIfIntegral(days);
  // 5. Let hours be ? Get(input, "hours").
  var hours = input.hours;
  // 6. If hours is not undefined, set result.[[Hours]] to ?
  //    ToIntegerIfIntegral(hours).
  if (hours !== undefined) result.hours = ToIntegerIfIntegral(hours);
  // 7. Let microseconds be ? Get(input, "microseconds").
  var microseconds = input.microseconds;
  // 8. If microseconds is not undefined, set result.[[Microseconds]] to ?
  //    ToIntegerIfIntegral(microseconds).
  if (microseconds !== undefined)
    result.microseconds = ToIntegerIfIntegral(microseconds);
  // 9. Let milliseconds be ? Get(input, "milliseconds").
  var milliseconds = input.milliseconds;
  // 10. If milliseconds is not undefined, set result.[[Milliseconds]] to ?
  //     ToIntegerIfIntegral(milliseconds).
  if (milliseconds !== undefined)
    result.milliseconds = ToIntegerIfIntegral(milliseconds);
  // 11. Let minutes be ? Get(input, "minutes").
  var minutes = input.minutes;
  // 12. If minutes is not undefined, set result.[[Minutes]] to ?
  //     ToIntegerIfIntegral(minutes).
  if (minutes !== undefined) result.minutes = ToIntegerIfIntegral(minutes);
  // 13. Let months be ? Get(input, "months").
  var months = input.months;
  // 14. If months is not undefined, set result.[[Months]] to ?
  //     ToIntegerIfIntegral(months).
  if (months !== undefined) result.months = ToIntegerIfIntegral(months);
  // 15. Let nanoseconds be ? Get(input, "nanoseconds").
  var nanoseconds = input.nanoseconds;
  // 16. If nanoseconds is not undefined, set result.[[Nanoseconds]] to ?
  //     ToIntegerIfIntegral(nanoseconds).
  if (nanoseconds !== undefined)
    result.nanoseconds = ToIntegerIfIntegral(nanoseconds);
  // 17. Let seconds be ? Get(input, "seconds").
  var seconds = input.seconds;
  // 18. If seconds is not undefined, set result.[[Seconds]] to ?
  //     ToIntegerIfIntegral(seconds).
  if (seconds !== undefined) result.seconds = ToIntegerIfIntegral(seconds);
  // 19. Let weeks be ? Get(input, "weeks").
  var weeks = input.weeks;
  // 20. If weeks is not undefined, set result.[[Weeks]] to ?
  //     ToIntegerIfIntegral(weeks).
  if (weeks !== undefined) result.weeks = ToIntegerIfIntegral(weeks);
  // 21. Let years be ? Get(input, "years").
  var years = input.years;
  // 22. If years is not undefined, set result.[[Years]] to ?
  //     ToIntegerIfIntegral(years).
  if (years !== undefined) result.years = ToIntegerIfIntegral(years);
  // 23. If years, months, weeks, days, hours, minutes, seconds,
  //     milliseconds, microseconds, and nanoseconds are all undefined,
  //     throw a TypeError exception.
  if (
    years === undefined &&
    months === undefined &&
    weeks === undefined &&
    days === undefined &&
    hours === undefined &&
    minutes === undefined &&
    seconds === undefined &&
    milliseconds === undefined &&
    microseconds === undefined &&
    nanoseconds === undefined
  )
    throw new _TypeError('Duration must have at least one unit');
  // 24. If IsValidDuration(result.[[Years]], result.[[Months]],
  //     result.[[Weeks]], result.[[Days]], result.[[Hours]],
  //     result.[[Minutes]], result.[[Seconds]], result.[[Milliseconds]],
  //     result.[[Microseconds]], result.[[Nanoseconds]]) is false, then
  //     a. Throw a RangeError exception.
  if (
    !IsValidDuration(
      result.years,
      result.months,
      result.weeks,
      result.days,
      result.hours,
      result.minutes,
      result.seconds,
      result.milliseconds,
      result.microseconds,
      result.nanoseconds,
    )
  )
    throw new _RangeError('Invalid duration');
  // 25. Return result.
  return result;
}

// DurationSign ( duration )
function DurationSign(duration) {
  // 1. For each value v of « duration.[[Years]], duration.[[Months]],
  //    duration.[[Weeks]], duration.[[Days]], duration.[[Hours]],
  //    duration.[[Minutes]], duration.[[Seconds]],
  //    duration.[[Milliseconds]], duration.[[Microseconds]],
  //    duration.[[Nanoseconds]] », do
  for (var i = 0; i < DURATION_FORMAT_UNITS.length; i++) {
    var v = duration[DURATION_FORMAT_UNITS[i].unit];
    // a. If v < 0, return -1.
    if (v < 0) return -1;
    // b. If v > 0, return 1.
    if (v > 0) return 1;
  }
  // 2. Return 0.
  return 0;
}

/// 2^53 × 10^9: the bound of IsValidDuration on the normalized seconds, in
/// nanoseconds.
var DURATION_MAX_NANOSECONDS = 9007199254740992000000000n;

// IsValidDuration ( years, months, weeks, days, hours, minutes, seconds,
//                   milliseconds, microseconds, nanoseconds )
// Each argument is an integral Number.
function IsValidDuration(
  years,
  months,
  weeks,
  days,
  hours,
  minutes,
  seconds,
  milliseconds,
  microseconds,
  nanoseconds,
) {
  // 1. Let sign be 0.
  var sign = 0;
  // 2. For each value v of « years, months, weeks, days, hours, minutes,
  //    seconds, milliseconds, microseconds, nanoseconds », do
  var values = [
    years,
    months,
    weeks,
    days,
    hours,
    minutes,
    seconds,
    milliseconds,
    microseconds,
    nanoseconds,
  ];
  for (var i = 0; i < values.length; i++) {
    var v = values[i];
    // a. If 𝔽(v) is not a finite Number, return false.
    if (!NumberIsFinite(v)) return false;
    if (v < 0) {
      // b. If v < 0, then
      //    i. If sign > 0, return false.
      if (sign > 0) return false;
      //    ii. Set sign to -1.
      sign = -1;
    } else if (v > 0) {
      // c. Else if v > 0, then
      //    i. If sign < 0, return false.
      if (sign < 0) return false;
      //    ii. Set sign to 1.
      sign = 1;
    }
  }
  // 3. If abs(years) ≥ 2^32, return false.
  if (years >= 4294967296 || years <= -4294967296) return false;
  // 4. If abs(months) ≥ 2^32, return false.
  if (months >= 4294967296 || months <= -4294967296) return false;
  // 5. If abs(weeks) ≥ 2^32, return false.
  if (weeks >= 4294967296 || weeks <= -4294967296) return false;
  // 6. Let normalizedSeconds be days × 86,400 + hours × 3600 + minutes × 60
  //    + seconds + ℝ(𝔽(milliseconds)) × 10^-3 + ℝ(𝔽(microseconds)) ×
  //    10^-6 + ℝ(𝔽(nanoseconds)) × 10^-9.
  // 7. NOTE: The above step cannot be implemented directly using
  //    floating-point arithmetic.
  //    normalizedNanoseconds is normalizedSeconds × 10^9, exactly: each
  //    argument is an integral Number, which BigInt converts exactly.
  var normalizedNanoseconds =
    _BigInt(days) * 86400000000000n +
    _BigInt(hours) * 3600000000000n +
    _BigInt(minutes) * 60000000000n +
    _BigInt(seconds) * 1000000000n +
    _BigInt(milliseconds) * 1000000n +
    _BigInt(microseconds) * 1000n +
    _BigInt(nanoseconds);
  // 8. If abs(normalizedSeconds) ≥ 2^53, return false.
  if (
    normalizedNanoseconds >= DURATION_MAX_NANOSECONDS ||
    normalizedNanoseconds <= -DURATION_MAX_NANOSECONDS
  )
    return false;
  // 9. Return true.
  return true;
}

// GetDurationUnitOptions ( unit, options, baseStyle, stylesList,
//                          digitalBase, prevStyle, twoDigitHours )
// Returns the Duration Unit Options Record, a null-prototype record
// { style, display }.
function GetDurationUnitOptions(
  unit,
  options,
  baseStyle,
  stylesList,
  digitalBase,
  prevStyle,
  twoDigitHours,
) {
  // 1. Let style be ? GetOption(options, unit, string, stylesList,
  //    undefined).
  var style = GetOption(options, unit, 'string', stylesList, undefined);
  // 2. Let displayDefault be "always".
  var displayDefault = 'always';
  // 3. If style is undefined, then
  if (style === undefined) {
    if (baseStyle === 'digital') {
      // a. If baseStyle is "digital", then
      //    i. Set style to digitalBase.
      style = digitalBase;
      //    ii. If unit is not one of "hours", "minutes", or "seconds", set
      //        displayDefault to "auto".
      if (unit !== 'hours' && unit !== 'minutes' && unit !== 'seconds')
        displayDefault = 'auto';
    } else if (
      prevStyle === 'fractional' ||
      prevStyle === 'numeric' ||
      prevStyle === '2-digit'
    ) {
      // b. Else if prevStyle is one of "fractional", "numeric" or
      //    "2-digit", then
      //    i. Set style to "numeric".
      style = 'numeric';
      //    ii. If unit is not "minutes" or "seconds", set displayDefault to
      //        "auto".
      if (unit !== 'minutes' && unit !== 'seconds') displayDefault = 'auto';
    } else {
      // c. Else,
      //    i. Set style to baseStyle.
      style = baseStyle;
      //    ii. Set displayDefault to "auto".
      displayDefault = 'auto';
    }
  }
  // 4. If style is "numeric" and IsFractionalSecondUnitName(unit) is true,
  //    then
  if (style === 'numeric' && IsFractionalSecondUnitName(unit)) {
    // a. Set style to "fractional".
    style = 'fractional';
    // b. Set displayDefault to "auto".
    displayDefault = 'auto';
  }
  // 5. Let displayField be the string-concatenation of unit and "Display".
  var displayField = unit + 'Display';
  // 6. Let display be ? GetOption(options, displayField, string,
  //    « "auto", "always" », displayDefault).
  var display = GetOption(
    options,
    displayField,
    'string',
    ['auto', 'always'],
    displayDefault,
  );
  // 7. Perform ? ValidateDurationUnitStyle(unit, style, display,
  //    prevStyle).
  ValidateDurationUnitStyle(unit, style, display, prevStyle);
  // 8. If unit is "hours" and twoDigitHours is true, set style to
  //    "2-digit".
  //    twoDigitHours is always false (durationDigitalFormat).
  if (unit === 'hours' && twoDigitHours) style = '2-digit';
  // 9. If unit is "minutes" or "seconds" and prevStyle is "numeric" or
  //    "2-digit", set style to "2-digit".
  if (
    (unit === 'minutes' || unit === 'seconds') &&
    (prevStyle === 'numeric' || prevStyle === '2-digit')
  )
    style = '2-digit';
  // 10. Return the Duration Unit Options Record { [[Style]]: style,
  //     [[Display]]: display }.
  return {__proto__: null, style: style, display: display};
}

// ValidateDurationUnitStyle ( unit, style, display, prevStyle )
function ValidateDurationUnitStyle(unit, style, display, prevStyle) {
  // 1. If display is "always" and style is "fractional", throw a RangeError
  //    exception.
  if (display === 'always' && style === 'fractional')
    throw new _RangeError(
      unit + ' cannot be displayed always with a numeric style',
    );
  // 2. If prevStyle is "fractional" and style is not "fractional", throw a
  //    RangeError exception.
  if (prevStyle === 'fractional' && style !== 'fractional')
    throw new _RangeError(unit + ' must be numeric after a numeric unit');
  // 3. If prevStyle is "numeric" or "2-digit" and style is not one of
  //    "fractional", "numeric" or "2-digit", throw a RangeError exception.
  if (
    (prevStyle === 'numeric' || prevStyle === '2-digit') &&
    style !== 'fractional' &&
    style !== 'numeric' &&
    style !== '2-digit'
  )
    throw new _RangeError(unit + ' must be numeric after a numeric unit');
  // 4. Return unused.
}

/// 10^(9 - exponent) for the exponents 3, 6 and 9 of ComputeFractionalDigits,
/// in that order.
var DURATION_FRACTION_SCALES = [1000000n, 1000n, 1n];

// ComputeFractionalDigits ( durationFormat, duration )
// Returns the draft's result × 10^9, a BigInt: every term is a multiple of
// 10^-9, so the result is exact.
function ComputeFractionalDigits(durationFormat, duration) {
  var unitOptions = DurationFormatSlots.get(durationFormat, 'unitOptions');
  // 1. Let result be 0.
  var result = 0n;
  // 2. Let exponent be 3.
  //    k is (exponent - 3) / 3, the index of 10^(9 - exponent) in
  //    DURATION_FRACTION_SCALES.
  var k = 0;
  // 3. For each row of Table "Internal slots and property names of
  //    DurationFormat instances", except the header row, in table order,
  //    do
  for (var i = 0; i < DURATION_FORMAT_UNITS.length; i++) {
    // a. Let unitOptions be the value of durationFormat's internal slot
    //    whose name is the Internal Slot value of the current row.
    // b. If unitOptions.[[Style]] is "fractional", then
    if (unitOptions[i].style === 'fractional') {
      //  i. Let unit be the Unit value of the current row.
      var unit = DURATION_FORMAT_UNITS[i].unit;
      //  ii. Assert: IsFractionalSecondUnitName(unit) is true.
      //  iii. Let value be the value of duration's field whose name is the
      //       Value Field value of the current row.
      var value = duration[unit];
      //  iv. Set result to result + (value / 10^exponent).
      result += _BigInt(value) * DURATION_FRACTION_SCALES[k];
      //  v. Set exponent to exponent + 3.
      k++;
    }
  }
  // 4. Return result.
  return result;
}

// NextUnitFractional ( durationFormat, unit )
function NextUnitFractional(durationFormat, unit) {
  var unitOptions = DurationFormatSlots.get(durationFormat, 'unitOptions');
  // 1. If unit is "seconds" and durationFormat.[[MillisecondsOptions]].
  //    [[Style]] is "fractional", return true.
  if (
    unit === 'seconds' &&
    unitOptions[DURATION_MILLISECONDS].style === 'fractional'
  )
    return true;
  // 2. If unit is "milliseconds" and durationFormat.[[MicrosecondsOptions]].
  //    [[Style]] is "fractional", return true.
  if (
    unit === 'milliseconds' &&
    unitOptions[DURATION_MICROSECONDS].style === 'fractional'
  )
    return true;
  // 3. If unit is "microseconds" and durationFormat.[[NanosecondsOptions]].
  //    [[Style]] is "fractional", return true.
  if (
    unit === 'microseconds' &&
    unitOptions[DURATION_NANOSECONDS].style === 'fractional'
  )
    return true;
  // 4. Return false.
  return false;
}

/// The Intl mathematical value of value + fraction × 10^-9, exactly, for
/// an integral Number value and a BigInt fraction of the same sign (a
/// result of ComputeFractionalDigits). With no fraction it is
/// makeIntlMV of value's BigInt magnitude, never IntlMVFromNumber, which
/// reads the shortest decimal string: 4503599627370497000 is exactly
/// 4503599627370497024 (decision 7).
function durationValueMV(value, fraction) {
  // 1. If fraction is 0, return the integer value.
  if (fraction === 0n) {
    var negative = value < 0;
    return makeIntlMV(negative, _BigInt(negative ? -value : value), 0);
  }
  // 2. Otherwise return value × 10^9 + fraction, scaled by 10^-9.
  var scaled = _BigInt(value) * 1000000000n + fraction;
  var negativeScaled = scaled < 0n;
  return makeIntlMV(negativeScaled, negativeScaled ? -scaled : scaled, -9);
}

/// The NumberFormat options durationNumberFormat keys its cache by, every
/// option the draft's nfOptions records have.
var DURATION_NUMBER_FORMAT_OPTIONS = [
  'numberingSystem',
  'style',
  'unit',
  'unitDisplay',
  'minimumIntegerDigits',
  'signDisplay',
  'useGrouping',
  'maximumFractionDigits',
  'minimumFractionDigits',
  'roundingMode',
];

/// ! Construct(%Intl.NumberFormat%, « durationFormat.[[Locale]],
/// nfOptions ») for the null-prototype record nfOptions: a NumberFormat
/// made with CreateNumberFormat on the first request for these options and
/// kept in durationFormat's #numberFormats (decision 7). The options are a
/// null-prototype record of primitives, so making the NumberFormat runs no
/// user code, and one made earlier formats as a new one would.
function durationNumberFormat(durationFormat, nfOptions) {
  // 1. Let key be the options' values, each followed by ";".
  var key = '';
  for (var i = 0; i < DURATION_NUMBER_FORMAT_OPTIONS.length; i++)
    key += ToString(nfOptions[DURATION_NUMBER_FORMAT_OPTIONS[i]]) + ';';
  // 2. If durationFormat has a NumberFormat for key, return it.
  var numberFormats = DurationFormatSlots.get(durationFormat, 'numberFormats');
  var nf = numberFormats[key];
  if (nf !== undefined) return nf;
  // 3. Otherwise make it, keep it and return it.
  nf = CreateNumberFormat(
    DurationFormatSlots.get(durationFormat, 'locale'),
    nfOptions,
  );
  numberFormats[key] = nf;
  return nf;
}

/// Append to result the parts of PartitionNumberPattern(nf, x), each with
/// the unit unit: the loops "For each Record { [[Type]], [[Value]] } part
/// of parts, do: Append the Record { [[Type]]: part.[[Type]], [[Value]]:
/// part.[[Value]], [[Unit]]: unit } to result" of FormatNumericHours,
/// FormatNumericMinutes, FormatNumericSeconds and
/// PartitionDurationFormatPattern.
function appendUnitParts(result, nf, x, unit) {
  // 1. Let parts be PartitionNumberPattern(nf, x).[[Parts]].
  var parts = PartitionNumberPattern(nf, x).parts;
  // 2. For each part of parts, append a copy with the unit unit to result.
  for (var i = 0; i < parts.length; i++) {
    listPush(result, {
      __proto__: null,
      type: parts[i].type,
      value: parts[i].value,
      unit: unit,
    });
  }
}

// FormatNumericHours ( durationFormat, hoursValue, signDisplayed )
// hoursValue is the Intl mathematical value of the integer or
// negative-zero. Returns a List of null-prototype records { type, value,
// unit }, unit undefined for ~empty~, as do the operations below.
function FormatNumericHours(durationFormat, hoursValue, signDisplayed) {
  // 1. Let result be a new empty List.
  var result = [];
  // 2. Let hoursStyle be durationFormat.[[HoursOptions]].[[Style]].
  var hoursStyle = DurationFormatSlots.get(durationFormat, 'unitOptions')[
    DURATION_HOURS
  ].style;
  // 3. Assert: hoursStyle is "numeric" or hoursStyle is "2-digit".
  // 4. Let nfOptions be OrdinaryObjectCreate(null).
  var nfOptions = {__proto__: null};
  // 5. Let numberingSystem be durationFormat.[[NumberingSystem]].
  // 6. Perform ! CreateDataPropertyOrThrow(nfOptions, "numberingSystem",
  //    numberingSystem).
  nfOptions.numberingSystem = DurationFormatSlots.get(
    durationFormat,
    'numberingSystem',
  );
  // 7. If hoursStyle is "2-digit", then
  //    a. Perform ! CreateDataPropertyOrThrow(nfOptions,
  //       "minimumIntegerDigits", 2𝔽).
  if (hoursStyle === '2-digit') nfOptions.minimumIntegerDigits = 2;
  // 8. If signDisplayed is false, then
  //    a. Perform ! CreateDataPropertyOrThrow(nfOptions, "signDisplay",
  //       "never").
  if (!signDisplayed) nfOptions.signDisplay = 'never';
  // 9. Perform ! CreateDataPropertyOrThrow(nfOptions, "useGrouping",
  //    false).
  nfOptions.useGrouping = false;
  // 10. Let nf be ! Construct(%Intl.NumberFormat%,
  //     « durationFormat.[[Locale]], nfOptions »).
  var nf = durationNumberFormat(durationFormat, nfOptions);
  // 11. Let hoursParts be PartitionNumberPattern(nf, hoursValue).
  // 12. For each Record { [[Type]], [[Value]] } part of hoursParts, do
  //     a. Append the Record { [[Type]]: part.[[Type]], [[Value]]:
  //        part.[[Value]], [[Unit]]: "hour" } to result.
  appendUnitParts(result, nf, hoursValue, 'hour');
  // 13. Return result.
  return result;
}

// FormatNumericMinutes ( durationFormat, minutesValue, hoursDisplayed,
//                        signDisplayed )
// minutesValue is the Intl mathematical value of the integer or
// negative-zero.
function FormatNumericMinutes(
  durationFormat,
  minutesValue,
  hoursDisplayed,
  signDisplayed,
) {
  // 1. Let result be a new empty List.
  var result = [];
  // 2. If hoursDisplayed is true, then
  if (hoursDisplayed) {
    // a. Let separator be durationFormat.[[HourMinuteSeparator]].
    var separator = DurationFormatSlots.get(
      durationFormat,
      'hourMinuteSeparator',
    );
    // b. Append the Record { [[Type]]: "literal", [[Value]]: separator,
    //    [[Unit]]: empty } to result.
    listPush(result, {
      __proto__: null,
      type: 'literal',
      value: separator,
      unit: undefined,
    });
  }
  // 3. Let minutesStyle be durationFormat.[[MinutesOptions]].[[Style]].
  var minutesStyle = DurationFormatSlots.get(durationFormat, 'unitOptions')[
    DURATION_MINUTES
  ].style;
  // 4. Assert: minutesStyle is "numeric" or minutesStyle is "2-digit".
  // 5. Let nfOptions be OrdinaryObjectCreate(null).
  var nfOptions = {__proto__: null};
  // 6. Let numberingSystem be durationFormat.[[NumberingSystem]].
  // 7. Perform ! CreateDataPropertyOrThrow(nfOptions, "numberingSystem",
  //    numberingSystem).
  nfOptions.numberingSystem = DurationFormatSlots.get(
    durationFormat,
    'numberingSystem',
  );
  // 8. If minutesStyle is "2-digit", then
  //    a. Perform ! CreateDataPropertyOrThrow(nfOptions,
  //       "minimumIntegerDigits", 2𝔽).
  if (minutesStyle === '2-digit') nfOptions.minimumIntegerDigits = 2;
  // 9. If signDisplayed is false, then
  //    a. Perform ! CreateDataPropertyOrThrow(nfOptions, "signDisplay",
  //       "never").
  if (!signDisplayed) nfOptions.signDisplay = 'never';
  // 10. Perform ! CreateDataPropertyOrThrow(nfOptions, "useGrouping",
  //     false).
  nfOptions.useGrouping = false;
  // 11. Let nf be ! Construct(%Intl.NumberFormat%,
  //     « durationFormat.[[Locale]], nfOptions »).
  var nf = durationNumberFormat(durationFormat, nfOptions);
  // 12. Let minutesParts be PartitionNumberPattern(nf, minutesValue).
  // 13. For each Record { [[Type]], [[Value]] } part of minutesParts, do
  //     a. Append the Record { [[Type]]: part.[[Type]], [[Value]]:
  //        part.[[Value]], [[Unit]]: "minute" } to result.
  appendUnitParts(result, nf, minutesValue, 'minute');
  // 14. Return result.
  return result;
}

// FormatNumericSeconds ( durationFormat, secondsValue, minutesDisplayed,
//                        signDisplayed )
// secondsValue is an Intl mathematical value.
function FormatNumericSeconds(
  durationFormat,
  secondsValue,
  minutesDisplayed,
  signDisplayed,
) {
  // 1. Let result be a new empty List.
  var result = [];
  // 2. If minutesDisplayed is true, then
  if (minutesDisplayed) {
    // a. Let separator be durationFormat.[[MinuteSecondSeparator]].
    var separator = DurationFormatSlots.get(
      durationFormat,
      'minuteSecondSeparator',
    );
    // b. Append the Record { [[Type]]: "literal", [[Value]]: separator,
    //    [[Unit]]: empty } to result.
    listPush(result, {
      __proto__: null,
      type: 'literal',
      value: separator,
      unit: undefined,
    });
  }
  // 3. Let secondsStyle be durationFormat.[[SecondsOptions]].[[Style]].
  var secondsStyle = DurationFormatSlots.get(durationFormat, 'unitOptions')[
    DURATION_SECONDS
  ].style;
  // 4. Assert: secondsStyle is "numeric" or secondsStyle is "2-digit".
  // 5. Let nfOptions be OrdinaryObjectCreate(null).
  var nfOptions = {__proto__: null};
  // 6. Let numberingSystem be durationFormat.[[NumberingSystem]].
  // 7. Perform ! CreateDataPropertyOrThrow(nfOptions, "numberingSystem",
  //    numberingSystem).
  nfOptions.numberingSystem = DurationFormatSlots.get(
    durationFormat,
    'numberingSystem',
  );
  // 8. If secondsStyle is "2-digit", then
  //    a. Perform ! CreateDataPropertyOrThrow(nfOptions,
  //       "minimumIntegerDigits", 2𝔽).
  if (secondsStyle === '2-digit') nfOptions.minimumIntegerDigits = 2;
  // 9. If signDisplayed is false, then
  //    a. Perform ! CreateDataPropertyOrThrow(nfOptions, "signDisplay",
  //       "never").
  if (!signDisplayed) nfOptions.signDisplay = 'never';
  // 10. Perform ! CreateDataPropertyOrThrow(nfOptions, "useGrouping",
  //     false).
  nfOptions.useGrouping = false;
  // 11. Let fractionDigits be durationFormat.[[FractionalDigits]].
  var fractionDigits = DurationFormatSlots.get(
    durationFormat,
    'fractionalDigits',
  );
  if (fractionDigits === undefined) {
    // 12. If fractionDigits is undefined, then
    //     a. Perform ! CreateDataPropertyOrThrow(nfOptions,
    //        "maximumFractionDigits", 9𝔽).
    nfOptions.maximumFractionDigits = 9;
    //     b. Perform ! CreateDataPropertyOrThrow(nfOptions,
    //        "minimumFractionDigits", +0𝔽).
    nfOptions.minimumFractionDigits = 0;
  } else {
    // 13. Else,
    //     a. Perform ! CreateDataPropertyOrThrow(nfOptions,
    //        "maximumFractionDigits", fractionDigits).
    nfOptions.maximumFractionDigits = fractionDigits;
    //     b. Perform ! CreateDataPropertyOrThrow(nfOptions,
    //        "minimumFractionDigits", fractionDigits).
    nfOptions.minimumFractionDigits = fractionDigits;
  }
  // 14. Perform ! CreateDataPropertyOrThrow(nfOptions, "roundingMode",
  //     "trunc").
  nfOptions.roundingMode = 'trunc';
  // 15. Let nf be ! Construct(%Intl.NumberFormat%,
  //     « durationFormat.[[Locale]], nfOptions »).
  var nf = durationNumberFormat(durationFormat, nfOptions);
  // 16. Let secondsParts be PartitionNumberPattern(nf, secondsValue).
  // 17. For each Record { [[Type]], [[Value]] } part of secondsParts, do
  //     a. Append the Record { [[Type]]: part.[[Type]], [[Value]]:
  //        part.[[Value]], [[Unit]]: "second" } to result.
  appendUnitParts(result, nf, secondsValue, 'second');
  // 18. Return result.
  return result;
}

/// Append each element of the List b to the List a: the draft's "Set a to
/// the list-concatenation of a and b".
function listAppendAll(a, b) {
  // 1. For each element e of b, in order, append e to a.
  for (var i = 0; i < b.length; i++) listPush(a, b[i]);
}

// FormatNumericUnits ( durationFormat, duration, firstNumericUnit,
//                      signDisplayed )
function FormatNumericUnits(
  durationFormat,
  duration,
  firstNumericUnit,
  signDisplayed,
) {
  var unitOptions = DurationFormatSlots.get(durationFormat, 'unitOptions');
  // 1. Assert: firstNumericUnit is "hours", "minutes", or "seconds".
  // 2. Let numericPartsList be a new empty List.
  var numericPartsList = [];
  // 3. Let hoursValue be duration.[[Hours]].
  var hoursValue = duration.hours;
  // 4. Let hoursDisplay be durationFormat.[[HoursOptions]].[[Display]].
  var hoursDisplay = unitOptions[DURATION_HOURS].display;
  // 5. Let minutesValue be duration.[[Minutes]].
  var minutesValue = duration.minutes;
  // 6. Let minutesDisplay be durationFormat.[[MinutesOptions]].[[Display]].
  var minutesDisplay = unitOptions[DURATION_MINUTES].display;
  // 7. Let secondsValue be duration.[[Seconds]].
  //    secondsFraction is what step 8 adds, × 10^9.
  var secondsValue = duration.seconds;
  var secondsFraction = 0n;
  // 8. If duration.[[Milliseconds]] is not 0 or duration.[[Microseconds]]
  //    is not 0 or duration.[[Nanoseconds]] is not 0, then
  //    a. Set secondsValue to secondsValue +
  //       ComputeFractionalDigits(durationFormat, duration).
  if (
    duration.milliseconds !== 0 ||
    duration.microseconds !== 0 ||
    duration.nanoseconds !== 0
  )
    secondsFraction = ComputeFractionalDigits(durationFormat, duration);
  // 9. Let secondsDisplay be durationFormat.[[SecondsOptions]].[[Display]].
  var secondsDisplay = unitOptions[DURATION_SECONDS].display;
  // 10. Let hoursFormatted be false.
  var hoursFormatted = false;
  // 11. If firstNumericUnit is "hours", then
  //     a. If hoursValue is not 0 or hoursDisplay is "always", then
  //        i. Set hoursFormatted to true.
  if (
    firstNumericUnit === 'hours' &&
    (hoursValue !== 0 || hoursDisplay === 'always')
  )
    hoursFormatted = true;
  // 12. If secondsValue is not 0 or secondsDisplay is "always", then
  //     a. Let secondsFormatted be true.
  // 13. Else,
  //     a. Let secondsFormatted be false.
  //     secondsValue is secondsValue + secondsFraction × 10^-9, which is 0
  //     exactly when both are: IsValidDuration guarantees that the integer
  //     part and the fraction never have opposite signs.
  var secondsFormatted =
    secondsValue !== 0 || secondsFraction !== 0n || secondsDisplay === 'always';
  // 14. Let minutesFormatted be false.
  var minutesFormatted = false;
  // 15. If firstNumericUnit is "hours" or firstNumericUnit is "minutes",
  //     then
  if (firstNumericUnit === 'hours' || firstNumericUnit === 'minutes') {
    if (hoursFormatted && secondsFormatted) {
      // a. If hoursFormatted is true and secondsFormatted is true, then
      //    i. Set minutesFormatted to true.
      minutesFormatted = true;
    } else if (minutesValue !== 0 || minutesDisplay === 'always') {
      // b. Else if minutesValue is not 0 or minutesDisplay is "always",
      //    then
      //    i. Set minutesFormatted to true.
      minutesFormatted = true;
    }
  }
  // 16. If hoursFormatted is true, then
  if (hoursFormatted) {
    // a. If signDisplayed is true, then
    //    i. If hoursValue is 0 and DurationSign(duration) is -1, then
    //       1. Set hoursValue to negative-zero.
    var hoursMV =
      signDisplayed && hoursValue === 0 && DurationSign(duration) === -1
        ? 'negZero'
        : durationValueMV(hoursValue, 0n);
    // b. Let hoursParts be FormatNumericHours(durationFormat, hoursValue,
    //    signDisplayed).
    var hoursParts = FormatNumericHours(durationFormat, hoursMV, signDisplayed);
    // c. Set numericPartsList to the list-concatenation of numericPartsList
    //    and hoursParts.
    listAppendAll(numericPartsList, hoursParts);
    // d. Set signDisplayed to false.
    signDisplayed = false;
  }
  // 17. If minutesFormatted is true, then
  if (minutesFormatted) {
    // a. If signDisplayed is true, then
    //    i. If minutesValue is 0 and DurationSign(duration) is -1, then
    //       1. Set minutesValue to negative-zero.
    var minutesMV =
      signDisplayed && minutesValue === 0 && DurationSign(duration) === -1
        ? 'negZero'
        : durationValueMV(minutesValue, 0n);
    // b. Let minutesParts be FormatNumericMinutes(durationFormat,
    //    minutesValue, hoursFormatted, signDisplayed).
    var minutesParts = FormatNumericMinutes(
      durationFormat,
      minutesMV,
      hoursFormatted,
      signDisplayed,
    );
    // c. Set numericPartsList to the list-concatenation of numericPartsList
    //    and minutesParts.
    listAppendAll(numericPartsList, minutesParts);
    // d. Set signDisplayed to false.
    signDisplayed = false;
  }
  // 18. If secondsFormatted is true, then
  if (secondsFormatted) {
    // a. Let secondsParts be FormatNumericSeconds(durationFormat,
    //    secondsValue, minutesFormatted, signDisplayed).
    var secondsParts = FormatNumericSeconds(
      durationFormat,
      durationValueMV(secondsValue, secondsFraction),
      minutesFormatted,
      signDisplayed,
    );
    // b. Set numericPartsList to the list-concatenation of numericPartsList
    //    and secondsParts.
    listAppendAll(numericPartsList, secondsParts);
  }
  // 19. Return numericPartsList.
  return numericPartsList;
}

// IsFractionalSecondUnitName ( unit )
function IsFractionalSecondUnitName(unit) {
  // 1. If unit is one of "milliseconds", "microseconds", or "nanoseconds",
  //    return true.
  // 2. Return false.
  return (
    unit === 'milliseconds' ||
    unit === 'microseconds' ||
    unit === 'nanoseconds'
  );
}

// ListFormatParts ( durationFormat, partitionedPartsList )
function ListFormatParts(durationFormat, partitionedPartsList) {
  // 1. Let lfOptions be OrdinaryObjectCreate(null).
  // 2. Perform ! CreateDataPropertyOrThrow(lfOptions, "type", "unit").
  // 3. Let listStyle be durationFormat.[[Style]].
  // 4. If listStyle is "digital", then
  //    a. Set listStyle to "short".
  // 5. Perform ! CreateDataPropertyOrThrow(lfOptions, "style", listStyle).
  // 6. Let lf be ! Construct(%Intl.ListFormat%, « durationFormat.[[Locale]],
  //    lfOptions »).
  //    The ListFormat is made with CreateListFormat on the first call and
  //    kept (decision 7); it depends only on the slots.
  var lf = DurationFormatSlots.get(durationFormat, 'listFormat');
  if (lf === undefined) {
    var listStyle = DurationFormatSlots.get(durationFormat, 'style');
    if (listStyle === 'digital') listStyle = 'short';
    lf = CreateListFormat(DurationFormatSlots.get(durationFormat, 'locale'), {
      __proto__: null,
      type: 'unit',
      style: listStyle,
    });
    DurationFormatSlots.setListFormat(durationFormat, lf);
  }
  // 7. Let strings be a new empty List.
  var strings = [];
  // 8. For each element parts of partitionedPartsList, do
  for (var i = 0; i < partitionedPartsList.length; i++) {
    var parts = partitionedPartsList[i];
    // a. Let string be the empty String.
    var string = '';
    // b. For each Record { [[Type]], [[Value]], [[Unit]] } part of parts,
    //    do
    //    i. Set string to the string-concatenation of string and
    //       part.[[Value]].
    for (var j = 0; j < parts.length; j++) string += parts[j].value;
    // c. Append string to strings.
    listPush(strings, string);
  }
  // 9. Let formattedPartsList be CreatePartsFromList(lf, strings).
  var formattedPartsList = CreatePartsFromList(lf, strings);
  // 10. Let partitionedPartsIndex be 0.
  var partitionedPartsIndex = 0;
  // 11. Let partitionedLength be the number of elements in
  //     partitionedPartsList.
  var partitionedLength = partitionedPartsList.length;
  // 12. Let flattenedPartsList be a new empty List.
  var flattenedPartsList = [];
  // 13. For each Record { [[Type]], [[Value]] } listPart of
  //     formattedPartsList, do
  for (var k = 0; k < formattedPartsList.length; k++) {
    var listPart = formattedPartsList[k];
    if (listPart.type === 'element') {
      // a. If listPart.[[Type]] is "element", then
      //    i. Assert: partitionedPartsIndex < partitionedLength.
      if (partitionedPartsIndex >= partitionedLength)
        throw new _TypeError('Intl internal error: too many list elements');
      //    ii. Let parts be partitionedPartsList[partitionedPartsIndex].
      //    iii. For each Record { [[Type]], [[Value]], [[Unit]] } part of
      //         parts, do
      //         1. Append part to flattenedPartsList.
      listAppendAll(
        flattenedPartsList,
        partitionedPartsList[partitionedPartsIndex],
      );
      //    iv. Set partitionedPartsIndex to partitionedPartsIndex + 1.
      partitionedPartsIndex++;
    } else {
      // b. Else,
      //    i. Assert: listPart.[[Type]] is "literal".
      //    ii. Append the Record { [[Type]]: "literal", [[Value]]:
      //        listPart.[[Value]], [[Unit]]: empty } to flattenedPartsList.
      listPush(flattenedPartsList, {
        __proto__: null,
        type: 'literal',
        value: listPart.value,
        unit: undefined,
      });
    }
  }
  // 14. Return flattenedPartsList.
  return flattenedPartsList;
}

// PartitionDurationFormatPattern ( durationFormat, duration )
function PartitionDurationFormatPattern(durationFormat, duration) {
  var unitOptionsList = DurationFormatSlots.get(durationFormat, 'unitOptions');
  // 1. Let result be a new empty List.
  var result = [];
  // 2. Let signDisplayed be true.
  var signDisplayed = true;
  // 3. Let numericUnitFound be false.
  var numericUnitFound = false;
  // 4. While numericUnitFound is false, repeat for each row in Table
  //    "DurationFormat instance internal slots and properties relevant to
  //    PartitionDurationFormatPattern" in table order, except the header
  //    row:
  for (var i = 0; !numericUnitFound && i < DURATION_FORMAT_UNITS.length; i++) {
    var row = DURATION_FORMAT_UNITS[i];
    // a. Let value be the value of duration's field whose name is the Value
    //    Field value of the current row.
    //    fraction is what step 4.g.ii.1 adds, × 10^9.
    var value = duration[row.unit];
    var fraction = 0n;
    // b. Let unitOptions be the value of durationFormat's internal slot
    //    whose name is the Internal Slot value of the current row.
    var unitOptions = unitOptionsList[i];
    // c. Let style be unitOptions.[[Style]].
    var style = unitOptions.style;
    // d. Let display be unitOptions.[[Display]].
    var display = unitOptions.display;
    // e. Let unit be the Unit value of the current row.
    var unit = row.unit;
    // f. Let numberFormatUnit be the NumberFormat Unit value of the current
    //    row.
    var numberFormatUnit = row.numberFormatUnit;
    if (style === 'numeric' || style === '2-digit') {
      // g. If style is "numeric" or "2-digit", then
      //    i. Let numericPartsList be FormatNumericUnits(durationFormat,
      //       duration, unit, signDisplayed).
      var numericPartsList = FormatNumericUnits(
        durationFormat,
        duration,
        unit,
        signDisplayed,
      );
      //    ii. If numericPartsList is not empty, append numericPartsList to
      //        result.
      if (numericPartsList.length !== 0) listPush(result, numericPartsList);
      //    iii. Set numericUnitFound to true.
      numericUnitFound = true;
    } else {
      // h. Else,
      //    i. Let nfOptions be OrdinaryObjectCreate(null).
      var nfOptions = {__proto__: null};
      //    ii. If NextUnitFractional(durationFormat, unit) is true, then
      if (NextUnitFractional(durationFormat, unit)) {
        // 1. Set value to value + ComputeFractionalDigits(durationFormat,
        //    duration).
        fraction = ComputeFractionalDigits(durationFormat, duration);
        // 2. Let fractionDigits be durationFormat.[[FractionalDigits]].
        var fractionDigits = DurationFormatSlots.get(
          durationFormat,
          'fractionalDigits',
        );
        if (fractionDigits === undefined) {
          // 3. If fractionDigits is undefined, then
          //    a. Perform ! CreateDataPropertyOrThrow(nfOptions,
          //       "maximumFractionDigits", 9𝔽).
          nfOptions.maximumFractionDigits = 9;
          //    b. Perform ! CreateDataPropertyOrThrow(nfOptions,
          //       "minimumFractionDigits", +0𝔽).
          nfOptions.minimumFractionDigits = 0;
        } else {
          // 4. Else,
          //    a. Perform ! CreateDataPropertyOrThrow(nfOptions,
          //       "maximumFractionDigits", fractionDigits).
          nfOptions.maximumFractionDigits = fractionDigits;
          //    b. Perform ! CreateDataPropertyOrThrow(nfOptions,
          //       "minimumFractionDigits", fractionDigits).
          nfOptions.minimumFractionDigits = fractionDigits;
        }
        // 5. Perform ! CreateDataPropertyOrThrow(nfOptions, "roundingMode",
        //    "trunc").
        nfOptions.roundingMode = 'trunc';
        // 6. Set numericUnitFound to true.
        numericUnitFound = true;
      }
      //    iii. If display is "always" or value is not 0, then
      //         value is value + fraction × 10^-9, which is 0 exactly when
      //         both are: IsValidDuration guarantees that the integer part
      //         and the fraction never have opposite signs.
      if (display === 'always' || value !== 0 || fraction !== 0n) {
        // 1. Perform ! CreateDataPropertyOrThrow(nfOptions,
        //    "numberingSystem", durationFormat.[[NumberingSystem]]).
        nfOptions.numberingSystem = DurationFormatSlots.get(
          durationFormat,
          'numberingSystem',
        );
        var x;
        if (signDisplayed) {
          // 2. If signDisplayed is true, then
          //    a. Set signDisplayed to false.
          signDisplayed = false;
          //    b. If value is 0 and DurationSign(duration) is -1, set value
          //       to negative-zero. value + fraction × 10^-9 is 0 exactly
          //       when both are, as above.
          x =
            value === 0 && fraction === 0n && DurationSign(duration) === -1
              ? 'negZero'
              : durationValueMV(value, fraction);
        } else {
          // 3. Else,
          //    a. Perform ! CreateDataPropertyOrThrow(nfOptions,
          //       "signDisplay", "never").
          nfOptions.signDisplay = 'never';
          x = durationValueMV(value, fraction);
        }
        // 4. Perform ! CreateDataPropertyOrThrow(nfOptions, "style",
        //    "unit").
        nfOptions.style = 'unit';
        // 5. Perform ! CreateDataPropertyOrThrow(nfOptions, "unit",
        //    numberFormatUnit).
        nfOptions.unit = numberFormatUnit;
        // 6. Perform ! CreateDataPropertyOrThrow(nfOptions, "unitDisplay",
        //    style).
        nfOptions.unitDisplay = style;
        // 7. Let nf be ! Construct(%Intl.NumberFormat%,
        //    « durationFormat.[[Locale]], nfOptions »).
        var nf = durationNumberFormat(durationFormat, nfOptions);
        // 8. Let parts be PartitionNumberPattern(nf, value).
        // 9. Let list be a new empty List.
        // 10. For each Record { [[Type]], [[Value]] } part of parts, do
        //     a. Append the Record { [[Type]]: part.[[Type]], [[Value]]:
        //        part.[[Value]], [[Unit]]: numberFormatUnit } to list.
        var list = [];
        appendUnitParts(list, nf, x, numberFormatUnit);
        // 11. Append list to result.
        listPush(result, list);
      }
    }
  }
  // 5. Return ListFormatParts(durationFormat, result).
  return ListFormatParts(durationFormat, result);
}

/// The rows of Table "Resolved Options of DurationFormat Instances", in
/// table order: [Property, slot name, Conversion]. The style+display rows
/// name the index of the unit's slot in #unitOptions instead of a slot.
var DURATION_FORMAT_RESOLVED_OPTIONS = [
  ['locale', 'locale', ''],
  ['numberingSystem', 'numberingSystem', ''],
  ['style', 'style', ''],
  ['years', 0, 'style+display'],
  ['months', 1, 'style+display'],
  ['weeks', 2, 'style+display'],
  ['days', 3, 'style+display'],
  ['hours', 4, 'style+display'],
  ['minutes', 5, 'style+display'],
  ['seconds', 6, 'style+display'],
  ['milliseconds', 7, 'style+display'],
  ['microseconds', 8, 'style+display'],
  ['nanoseconds', 9, 'style+display'],
  ['fractionalDigits', 'fractionalDigits', 'number'],
];

/// Install Intl.DurationFormat on intl; called by installIntl.
function installDurationFormat(intl) {
  // 1. Let proto be %Intl.DurationFormat.prototype%, an ordinary object
  //    whose [[Prototype]] is %Object.prototype% ("Properties of the
  //    Intl.DurationFormat Prototype Object").
  var proto = {};
  DurationFormatPrototype = proto;
  // 2. "Properties of the Intl.DurationFormat Constructor":
  //    Intl.DurationFormat.prototype, with its length 0, and
  //    Intl.DurationFormat.prototype.constructor.
  defineConstructor(DurationFormat, 0, proto);
  // 3. The method properties of the constructor.
  defineMethods(DurationFormat, {
    // Intl.DurationFormat.supportedLocalesOf ( locales [ , options ] )
    supportedLocalesOf(locales, options = undefined) {
      // 1. Let availableLocales be
      //    %Intl.DurationFormat%.[[AvailableLocales]].
      // 2. Let requestedLocales be ? CanonicalizeLocaleList(locales).
      var requestedLocales = CanonicalizeLocaleList(locales);
      // 3. Return ? FilterLocales(availableLocales, requestedLocales,
      //    options).
      return FilterLocales(isAvailableLocale, requestedLocales, options);
    },
  });
  // 4. The method properties of the prototype, in the draft's order.
  defineMethods(proto, {
    // Intl.DurationFormat.prototype.resolvedOptions ( )
    resolvedOptions() {
      // 1. Let df be the this value.
      var df = this;
      // 2. Perform ? RequireInternalSlot(df, [[InitializedDurationFormat]]).
      requireInitializedDurationFormat(df);
      var unitOptionsList = DurationFormatSlots.get(df, 'unitOptions');
      // 3. Let options be OrdinaryObjectCreate(%Object.prototype%).
      var options = {};
      // 4. For each row of Table "Resolved Options of DurationFormat
      //    Instances", except the header row, in table order, do
      for (var i = 0; i < DURATION_FORMAT_RESOLVED_OPTIONS.length; i++) {
        var row = DURATION_FORMAT_RESOLVED_OPTIONS[i];
        // a. Let propertyKey be the Property value of the current row.
        var propertyKey = row[0];
        // b. Let value be the value of df's internal slot whose name is the
        //    Internal Slot value of the current row.
        var value =
          row[2] === 'style+display'
            ? unitOptionsList[row[1]]
            : DurationFormatSlots.get(df, row[1]);
        // c. If value is not undefined, then
        if (value === undefined) continue;
        // i. If there is a Conversion value in the current row, let
        //    conversion be that value; else let conversion be empty.
        var conversion = row[2];
        if (conversion === 'number') {
          // ii. If conversion is number, then
          //     1. Set value to 𝔽(value).
          //     value is already a Number.
        } else if (conversion !== '') {
          // iii. Else if conversion is not empty, then
          //      1. Assert: conversion is style+display and value is a
          //         Duration Unit Options Record.
          //      2. NOTE: value.[[Style]] will be represented with a
          //         property named propertyKey (a plural Temporal unit),
          //         then value.[[Display]] will be represented with a
          //         property whose name suffixes propertyKey with
          //         "Display".
          //      3. Let style be value.[[Style]].
          var style = value.style;
          //      4. If style is "fractional", then
          //         a. Assert: IsFractionalSecondUnitName(propertyKey) is
          //            true.
          //         b. Set style to "numeric".
          if (style === 'fractional') style = 'numeric';
          //      5. Perform ! CreateDataPropertyOrThrow(options,
          //         propertyKey, style).
          CreateDataPropertyOrThrow(options, propertyKey, style);
          //      6. Set propertyKey to the string-concatenation of
          //         propertyKey and "Display".
          propertyKey = propertyKey + 'Display';
          //      7. Set value to value.[[Display]].
          value = value.display;
        }
        // iv. Perform ! CreateDataPropertyOrThrow(options, propertyKey,
        //     value).
        CreateDataPropertyOrThrow(options, propertyKey, value);
      }
      // 5. Return options.
      return options;
    },
    // Intl.DurationFormat.prototype.format ( duration )
    format(duration) {
      // 1. Let df be the this value.
      var df = this;
      // 2. Perform ? RequireInternalSlot(df, [[InitializedDurationFormat]]).
      requireInitializedDurationFormat(df);
      // 3. Let record be ? ToDurationRecord(duration).
      var record = ToDurationRecord(duration);
      // 4. Let parts be PartitionDurationFormatPattern(df, record).
      var parts = PartitionDurationFormatPattern(df, record);
      // 5. Let result be the empty String.
      var result = '';
      // 6. For each Record { [[Type]], [[Value]], [[Unit]] } part of parts,
      //    do
      //    a. Set result to the string-concatenation of result and
      //       part.[[Value]].
      for (var i = 0; i < parts.length; i++) result += parts[i].value;
      // 7. Return result.
      return result;
    },
    // Intl.DurationFormat.prototype.formatToParts ( duration )
    formatToParts(duration) {
      // 1. Let df be the this value.
      var df = this;
      // 2. Perform ? RequireInternalSlot(df, [[InitializedDurationFormat]]).
      requireInitializedDurationFormat(df);
      // 3. Let record be ? ToDurationRecord(duration).
      var record = ToDurationRecord(duration);
      // 4. Let parts be PartitionDurationFormatPattern(df, record).
      var parts = PartitionDurationFormatPattern(df, record);
      // 5. Let result be ! ArrayCreate(0).
      var result = [];
      // 6. Let n be 0.
      // 7. For each Record { [[Type]], [[Value]], [[Unit]] } part of parts,
      //    do
      for (var n = 0; n < parts.length; n++) {
        var part = parts[n];
        // a. Let obj be OrdinaryObjectCreate(%Object.prototype%).
        // b. Perform ! CreateDataPropertyOrThrow(obj, "type",
        //    part.[[Type]]).
        // c. Perform ! CreateDataPropertyOrThrow(obj, "value",
        //    part.[[Value]]).
        // d. If part.[[Unit]] is not empty, perform !
        //    CreateDataPropertyOrThrow(obj, "unit", part.[[Unit]]).
        //    An object literal defines the properties, in this order.
        // e. Perform ! CreateDataPropertyOrThrow(result, ! ToString(n),
        //    obj).
        // f. Set n to n + 1.
        listPush(
          result,
          part.unit === undefined
            ? {type: part.type, value: part.value}
            : {type: part.type, value: part.value, unit: part.unit},
        );
      }
      // 8. Return result.
      return result;
    },
  });
  // 5. Intl.DurationFormat.prototype [ %Symbol.toStringTag% ]:
  //    "Intl.DurationFormat", { [[Writable]]: false, [[Enumerable]]: false,
  //    [[Configurable]]: true }.
  setToStringTag(proto, 'Intl.DurationFormat');
  // 6. "The Intl.DurationFormat Constructor": the initial value of the
  //    "DurationFormat" property of the Intl object. ECMA-262 18 gives such
  //    data properties the attributes { [[Writable]]: true,
  //    [[Enumerable]]: false, [[Configurable]]: true }.
  ObjectDefineProperty(intl, 'DurationFormat', {
    __proto__: null,
    value: DurationFormat,
    writable: true,
    enumerable: false,
    configurable: true,
  });
}
