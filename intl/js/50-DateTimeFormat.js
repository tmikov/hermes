/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// ECMA-402 "DateTimeFormat Objects": the Intl.DateTimeFormat constructor
// with ChainDateTimeFormat, CreateDateTimeFormat and
// FormatOffsetTimeZoneIdentifier, its supportedLocalesOf, the prototype's
// resolvedOptions, format, formatRange, formatRangeToParts, formatToParts
// and @@toStringTag, and UnwrapDateTimeFormat, following the draft
// https://tc39.es/ecma402/ dated October 24, 2026; and the time zone
// operations they use: GetAvailableNamedTimeZoneIdentifier (ECMA-402
// "Use of the IANA Time Zone Database"), and IsTimeZoneOffsetString and
// ParseTimeZoneOffsetString (ECMA-262 21.4.1.33, "Time Zone Offset String
// Format", in the https://tc39.es/ecma262/ draft of the same date).
// 51-datetime-patterns.js and 53-datetime-ranges.js make the format
// records, 54-datetime-names.js the name symbols and day periods;
// 52-datetime-format.js formats with them.

/// %Intl.DateTimeFormat.prototype%; set by installDateTimeFormat.
var DateTimeFormatPrototype;

/// The internal slots of an Intl.DateTimeFormat instance ("Properties of
/// Intl.DateTimeFormat Instances"), as private fields of the object passed
/// to the constructor. [[InitializedDateTimeFormat]] is the presence of
/// #locale. #data is not a slot of the draft: it is the null-prototype
/// record of what the formatter formats with, which initializeDateTimeFormat
/// describes.
class DateTimeFormatSlots extends ReturnOverride {
  /// [[Locale]]
  #locale;
  /// [[Calendar]]
  #calendar;
  /// [[NumberingSystem]]
  #numberingSystem;
  /// [[TimeZone]]
  #timeZone;
  /// [[HourCycle]]
  #hourCycle;
  /// [[DateStyle]]
  #dateStyle;
  /// [[TimeStyle]]
  #timeStyle;
  /// [[DateTimeFormat]]
  #dateTimeFormat;
  /// [[BoundFormat]]
  #boundFormat;
  /// The formatter's data.
  #data;

  /// Attach the slots to o, from the null-prototype record slots whose
  /// fields are named after the slots (a field that is absent is an
  /// undefined slot), with the record data. [[BoundFormat]] starts
  /// undefined.
  constructor(o, slots, data) {
    // 1. Add the private fields to o (ReturnOverride returns o).
    super(o);
    // 2. Set each slot from the field of the same name.
    this.#locale = slots.locale;
    this.#calendar = slots.calendar;
    this.#numberingSystem = slots.numberingSystem;
    this.#timeZone = slots.timeZone;
    this.#hourCycle = slots.hourCycle;
    this.#dateStyle = slots.dateStyle;
    this.#timeStyle = slots.timeStyle;
    this.#dateTimeFormat = slots.dateTimeFormat;
    this.#boundFormat = undefined;
    // 3. Keep the data.
    this.#data = data;
  }

  /// Whether o is an Object with an [[InitializedDateTimeFormat]] internal
  /// slot.
  static is(o) {
    // 1. If o is not an Object, return false.
    if (!isObject(o)) return false;
    // 2. Return whether o has the private fields.
    return #locale in o;
  }

  /// The slot name of o, after ECMA-262 RequireInternalSlot(o,
  /// [[InitializedDateTimeFormat]]). name is the slot's name in
  /// lowerCamelCase, or "data".
  static get(o, name) {
    // 1. Perform ? RequireInternalSlot(o, [[InitializedDateTimeFormat]]).
    requireInitializedDateTimeFormat(o);
    // 2. Return the slot.
    switch (name) {
      case 'locale':
        return o.#locale;
      case 'calendar':
        return o.#calendar;
      case 'numberingSystem':
        return o.#numberingSystem;
      case 'timeZone':
        return o.#timeZone;
      case 'hourCycle':
        return o.#hourCycle;
      case 'dateStyle':
        return o.#dateStyle;
      case 'timeStyle':
        return o.#timeStyle;
      case 'dateTimeFormat':
        return o.#dateTimeFormat;
      case 'boundFormat':
        return o.#boundFormat;
      case 'data':
        return o.#data;
    }
    throw new _TypeError(
      'Intl internal error: no DateTimeFormat slot ' + name,
    );
  }

  /// Set o.[[BoundFormat]] to f, after ECMA-262 RequireInternalSlot(o,
  /// [[InitializedDateTimeFormat]]).
  static setBoundFormat(o, f) {
    // 1. Perform ? RequireInternalSlot(o, [[InitializedDateTimeFormat]]).
    requireInitializedDateTimeFormat(o);
    // 2. Set o.[[BoundFormat]] to f.
    o.#boundFormat = f;
  }
}

/// ECMA-262 RequireInternalSlot(o, [[InitializedDateTimeFormat]]).
function requireInitializedDateTimeFormat(o) {
  // 1. If o is not an Object, or has no [[InitializedDateTimeFormat]]
  //    internal slot, throw a TypeError exception.
  if (!DateTimeFormatSlots.is(o))
    throw new _TypeError('Receiver is not an Intl.DateTimeFormat object');
}

// ---------------------------------------------------------------------------
// Time zones.

/// Whether the code unit at index i of s and the next one are a
/// MinuteSecond of ECMA-262's UTCOffset grammar: 0-5, then a digit.
function isMinuteSecond(s, i) {
  // 1. MinuteSecond ::: `0` DecimalDigit | ... | `5` DecimalDigit.
  var c = charCodeAt(s, i);
  return c >= 0x30 && c <= 0x35 && isAsciiDigit(charCodeAt(s, i + 1));
}

/// ParseText(offsetString, UTCOffset) of ECMA-262 21.4.1.33: the
/// null-prototype record { sign, hour, minuteSeconds, fraction } of the
/// source text the ASCIISign, Hour, MinuteSecond (a List of one String per
/// node, in order) and TemporalDecimalFraction (or undefined) Parse Nodes
/// match, or undefined if s does not conform to the grammar:
///
///   UTCOffset ::: ASCIISign Hour
///     | ASCIISign Hour HourSubcomponents[+Extended]
///     | ASCIISign Hour HourSubcomponents[~Extended]
///   ASCIISign ::: one of + -
///   Hour ::: 0 DecimalDigit | 1 DecimalDigit | 20 | 21 | 22 | 23
///   HourSubcomponents[Extended] ::: TimeSeparator[?Extended] MinuteSecond
///     | TimeSeparator[?Extended] MinuteSecond TimeSeparator[?Extended]
///       MinuteSecond TemporalDecimalFraction?
///   TimeSeparator[Extended] ::: [+Extended] : | [~Extended] [empty]
///   TemporalDecimalFraction ::: TemporalDecimalSeparator followed by one
///     to nine DecimalDigits
///   TemporalDecimalSeparator ::: one of . ,
function parseUTCOffset(s) {
  // 1. ASCIISign: the first code unit is + or - (ASCII only, so U+2212
  //    MINUS SIGN does not match).
  var sign = charCodeAt(s, 0);
  if (sign !== 0x2b && sign !== 0x2d) return undefined;
  // 2. Hour: 00 to 19, or 20 to 23.
  var h0 = charCodeAt(s, 1);
  var h1 = charCodeAt(s, 2);
  if (!isAsciiDigit(h1)) return undefined;
  if (!(h0 === 0x30 || h0 === 0x31 || (h0 === 0x32 && h1 <= 0x33)))
    return undefined;
  var result = {
    __proto__: null,
    sign: stringSlice(s, 0, 1),
    hour: stringSlice(s, 1, 3),
    minuteSeconds: [],
    fraction: undefined,
  };
  // 3. ASCIISign Hour alone.
  var i = 3;
  if (i === s.length) return result;
  // 4. HourSubcomponents: [+Extended] if a colon follows the hour, else
  //    [~Extended]; every TimeSeparator in it is the same.
  var extended = charCodeAt(s, i) === 0x3a;
  // 5. One or two MinuteSecond nodes, each after its TimeSeparator.
  for (var n = 0; n < 2; n++) {
    if (n === 1 && i === s.length) return result;
    if (extended) {
      if (charCodeAt(s, i) !== 0x3a) return undefined;
      i++;
    }
    if (!isMinuteSecond(s, i)) return undefined;
    listPush(result.minuteSeconds, stringSlice(s, i, i + 2));
    i += 2;
  }
  if (i === s.length) return result;
  // 6. TemporalDecimalFraction, after the second MinuteSecond only: a
  //    separator and one to nine digits, ending the string.
  var c = charCodeAt(s, i);
  if (c !== 0x2e && c !== 0x2c) return undefined;
  var start = i;
  i++;
  while (i < s.length && isAsciiDigit(charCodeAt(s, i))) i++;
  if (i - start < 2 || i - start > 10 || i !== s.length) return undefined;
  result.fraction = stringSlice(s, start, i);
  return result;
}

// ECMA-262 21.4.1.33.1 IsTimeZoneOffsetString ( offsetString )
function IsTimeZoneOffsetString(offsetString) {
  // 1. Let parseResult be ParseText(offsetString, UTCOffset).
  // 2. If parseResult is a List of errors, return false.
  // 3. Return true.
  return parseUTCOffset(offsetString) !== undefined;
}

// ECMA-262 21.4.1.33.2 ParseTimeZoneOffsetString ( offsetString )
function ParseTimeZoneOffsetString(offsetString) {
  // 1. Let parseResult be ParseText(offsetString, UTCOffset).
  var parseResult = parseUTCOffset(offsetString);
  // 2. Assert: parseResult is not a List of errors.
  // 3. Assert: parseResult contains a ASCIISign Parse Node.
  // 4. Let parsedSign be the source text matched by the ASCIISign Parse
  //    Node contained within parseResult.
  // 5. If parsedSign is the single code point U+002D (HYPHEN-MINUS), then
  //    a. Let sign be -1.
  // 6. Else,
  //    a. Let sign be 1.
  var sign = parseResult.sign === '-' ? -1 : 1;
  // 7. NOTE: Applications of StringToNumber below do not lose precision,
  //    since each of the parsed values is guaranteed to be a sufficiently
  //    short string of decimal digits.
  // 8. Assert: parseResult contains an Hour Parse Node.
  // 9. Let parsedHours be the source text matched by the Hour Parse Node
  //    contained within parseResult.
  // 10. Let hours be ℝ(StringToNumber(CodePointsToString(parsedHours))).
  var hours = ToNumber(parseResult.hour);
  var minuteSeconds = parseResult.minuteSeconds;
  var minutes;
  if (minuteSeconds.length === 0) {
    // 11. If parseResult does not contain a MinuteSecond Parse Node, then
    //     a. Let minutes be 0.
    minutes = 0;
  } else {
    // 12. Else,
    //     a. Let parsedMinutes be the source text matched by the first
    //        MinuteSecond Parse Node contained within parseResult.
    //     b. Let minutes be ℝ(StringToNumber(CodePointsToString(
    //        parsedMinutes))).
    minutes = ToNumber(minuteSeconds[0]);
  }
  var seconds;
  if (minuteSeconds.length < 2) {
    // 13. If parseResult does not contain two MinuteSecond Parse Nodes,
    //     then
    //     a. Let seconds be 0.
    seconds = 0;
  } else {
    // 14. Else,
    //     a. Let parsedSeconds be the source text matched by the second
    //        MinuteSecond Parse Node contained within parseResult.
    //     b. Let seconds be ℝ(StringToNumber(CodePointsToString(
    //        parsedSeconds))).
    seconds = ToNumber(minuteSeconds[1]);
  }
  var nanoseconds;
  if (parseResult.fraction === undefined) {
    // 15. If parseResult does not contain a TemporalDecimalFraction Parse
    //     Node, then
    //     a. Let nanoseconds be 0.
    nanoseconds = 0;
  } else {
    // 16. Else,
    //     a. Let parsedFraction be the source text matched by the
    //        TemporalDecimalFraction Parse Node contained within
    //        parseResult.
    //     b. Let fraction be the string-concatenation of
    //        CodePointsToString(parsedFraction) and "000000000".
    var fraction = parseResult.fraction + '000000000';
    //     c. Let nanosecondsString be the substring of fraction from 1 to
    //        10.
    //     d. Let nanoseconds be ℝ(StringToNumber(nanosecondsString)).
    nanoseconds = ToNumber(stringSlice(fraction, 1, 10));
  }
  // 17. Return sign × (((hours × MinutesPerHour + minutes) ×
  //     SecondsPerMinute + seconds) × NanosecondsPerSecond + nanoseconds).
  //     Every intermediate value is an integer below 2^53, so the Number
  //     arithmetic is exact.
  return sign * (((hours * 60 + minutes) * 60 + seconds) * 1e9 + nanoseconds);
}

// FormatOffsetTimeZoneIdentifier ( offsetMinutes )
function FormatOffsetTimeZoneIdentifier(offsetMinutes) {
  // 1. If offsetMinutes ≥ 0, let sign be the code unit 0x002B (PLUS SIGN);
  //    otherwise, let sign be the code unit 0x002D (HYPHEN-MINUS). -0 is
  //    ≥ 0, as the mathematical 0 it stands for is.
  var sign = offsetMinutes >= 0 ? '+' : '-';
  // 2. Let absoluteMinutes be abs(offsetMinutes).
  var absoluteMinutes = offsetMinutes < 0 ? -offsetMinutes : offsetMinutes;
  // 3. Let hours be floor(absoluteMinutes / 60).
  var hours = MathFloor(absoluteMinutes / 60);
  // 4. Let minutes be absoluteMinutes modulo 60.
  var minutes = absoluteMinutes - hours * 60;
  // 5. Return the string-concatenation of sign, ToZeroPaddedDecimalString(
  //    hours, 2), the code unit 0x003A (COLON), and
  //    ToZeroPaddedDecimalString(minutes, 2).
  return (
    sign +
    (hours < 10 ? '0' : '') +
    hours +
    ':' +
    (minutes < 10 ? '0' : '') +
    minutes
  );
}

// GetAvailableNamedTimeZoneIdentifier ( timeZoneIdentifier )
// The plugin's AvailableNamedTimeZoneIdentifiers are ICU4X's zone names and
// their aliases (Phase 4 decision 2); the native helper looks the name up
// without regard to case. Returns undefined for empty, or the
// null-prototype record { primaryIdentifier }: the record's [[Identifier]]
// is the name as AvailableNamedTimeZoneIdentifiers spells it, which no
// caller uses, so it is left out.
function GetAvailableNamedTimeZoneIdentifier(timeZoneIdentifier) {
  // 1. For each element record of AvailableNamedTimeZoneIdentifiers(), do
  //    a. If record.[[Identifier]] is an ASCII-case-insensitive match for
  //       timeZoneIdentifier, return record.
  var primary = native.canonicalizeTimeZone(timeZoneIdentifier);
  if (primary !== undefined)
    return {__proto__: null, primaryIdentifier: primary};
  // 2. Return empty.
  return undefined;
}

// ECMA-262 21.4.1.24 SystemTimeZoneIdentifier ( ): the primary identifier
// the loader resolved from the host or the IntlDefaultTimeZone option
// (intl/jsi/Defaults.cpp).
function SystemTimeZoneIdentifier() {
  // 1-3. Return the host's time zone, a primary time zone identifier.
  return hostDefaults.timeZone;
}

// ---------------------------------------------------------------------------
// [[LocaleData]] and the constructor.

/// [[LocaleData]].[[<locale>]].[[hc]], the same for every locale.
var DATE_TIME_FORMAT_HOUR_CYCLES = [null, 'h11', 'h12', 'h23', 'h24'];

/// The cache of %Intl.DateTimeFormat%.[[LocaleData]].[[<locale>]] records.
var dateTimeFormatLocaleDataCache = newDataCache();

/// %Intl.DateTimeFormat%.[[LocaleData]].[[<locale>]] for an available
/// locale (a tag with no Unicode extension): the record { ca, nu, hc,
/// hourCycle, hourCycle12, hourCycle24 }.
/// - ca (decision 9): the first calendar of the locale's calendarsOfTag
///   that Intl.supportedValuesOf("calendar") lists, then that list's other
///   calendars, in its order;
/// - nu: NumberFormat's list (Phase 3, decision 17);
/// - hc: « null, "h11", "h12", "h23", "h24" »;
/// - hourCycle, hourCycle12, hourCycle24 (decision 8): the first of the
///   locale's hourCyclesOfTag, its first 12-hour one (else "h12"), and its
///   first 24-hour one (else "h23").
/// [[formats]] and [[styles]] depend on the calendar and the numbering
/// system the constructor resolves, so it asks for them then
/// (cachedDateTimeLocaleData, decision 13).
function dateTimeFormatLocaleData(locale) {
  // 1. If the cache has the record of locale, return it.
  var data = dataCacheGet(dateTimeFormatLocaleDataCache, locale);
  if (data !== undefined) return data;
  // 2. Let first be the first calendar of calendarsOfTag(locale) that is
  //    supported; "gregory" if there is none.
  var supported = canonicalCalendars();
  var inUse = calendarsOfTag(locale);
  var first = 'gregory';
  for (var i = 0; i < inUse.length; i++) {
    if (listContains(supported, inUse[i])) {
      first = inUse[i];
      break;
    }
  }
  // 3. Let ca be « first », followed by every other supported calendar.
  var ca = [first];
  for (var j = 0; j < supported.length; j++) {
    if (supported[j] !== first) listPush(ca, supported[j]);
  }
  // 4. Let hourCycles be hourCyclesOfTag(locale); find its first 12-hour
  //    and first 24-hour cycles.
  var hourCycles = hourCyclesOfTag(locale);
  var hourCycle12 = undefined;
  var hourCycle24 = undefined;
  for (var k = 0; k < hourCycles.length; k++) {
    var hc = hourCycles[k];
    if (hourCycle12 === undefined && (hc === 'h11' || hc === 'h12'))
      hourCycle12 = hc;
    if (hourCycle24 === undefined && (hc === 'h23' || hc === 'h24'))
      hourCycle24 = hc;
  }
  // 5. Make, cache and return the record.
  data = {
    __proto__: null,
    ca: ca,
    nu: numberFormatLocaleData(locale).nu,
    hc: DATE_TIME_FORMAT_HOUR_CYCLES,
    hourCycle: hourCycles[0],
    hourCycle12: hourCycle12 === undefined ? 'h12' : hourCycle12,
    hourCycle24: hourCycle24 === undefined ? 'h23' : hourCycle24,
  };
  dataCachePut(dateTimeFormatLocaleDataCache, locale, data);
  return data;
}

/// The fields of %Intl.DateTimeFormat% that ResolveOptions reads ("Internal
/// slots"): [[AvailableLocales]], the plugin's one list,
/// [[RelevantExtensionKeys]] « "ca", "hc", "nu" », and
/// [[ResolutionOptionDescriptors]].
var DATE_TIME_FORMAT_SERVICE = {
  __proto__: null,
  availableLocales: isAvailableLocale,
  relevantExtensionKeys: ['ca', 'hc', 'nu'],
  resolutionOptionDescriptors: [
    {__proto__: null, key: 'ca', property: 'calendar'},
    {__proto__: null, key: 'nu', property: 'numberingSystem'},
    {__proto__: null, key: 'hour12', property: 'hour12', type: 'boolean'},
    {
      __proto__: null,
      key: 'hc',
      property: 'hourCycle',
      values: ['h11', 'h12', 'h23', 'h24'],
    },
  ],
};

/// The Values column of the draft's table "Components of date and time
/// formats", for each Property but fractionalSecondDigits, whose values
/// GetNumberOption checks.
var DATE_TIME_COMPONENT_VALUES = {
  __proto__: null,
  weekday: ['narrow', 'short', 'long'],
  era: ['narrow', 'short', 'long'],
  year: ['2-digit', 'numeric'],
  month: ['2-digit', 'numeric', 'narrow', 'short', 'long'],
  day: ['2-digit', 'numeric'],
  dayPeriod: ['narrow', 'short', 'long'],
  hour: ['2-digit', 'numeric'],
  minute: ['2-digit', 'numeric'],
  second: ['2-digit', 'numeric'],
  timeZoneName: [
    'short',
    'long',
    'shortOffset',
    'longOffset',
    'shortGeneric',
    'longGeneric',
  ],
};

/// The hour-cycle family of [[formats]] for the hour cycle hc (decision
/// 6): "h12" for h11 and h12, "h24" for h23 and h24.
function hourCycleFamily(hc) {
  // 1. Return the family.
  return hc === 'h11' || hc === 'h12' ? 'h12' : 'h24';
}

/// [[LocaleData]].[[<locale>]]'s [[formats]] and [[styles]] for the
/// calendar and numbering system of slots, the null-prototype record of
/// the resolved slots: cachedDateTimeLocaleData of [[Locale]]'s language
/// identifier, [[Calendar]], and the decimal separator of
/// [[NumberingSystem]], which a fraction of a second is written with
/// (entry 0 of Phase 3's numberSymbols, decision 13).
function dateTimeFormatsOf(slots, languageId) {
  // 1. Let decimalSeparator be the decimal separator of languageId and
  //    [[NumberingSystem]].
  var decimalSeparator = cachedNumberSymbols(
    languageId,
    slots.numberingSystem,
  )[SYM_DECIMAL];
  // 2. Return the records of languageId, [[Calendar]] and
  //    decimalSeparator.
  return cachedDateTimeLocaleData(
    languageId,
    slots.calendar,
    decimalSeparator,
  );
}

/// CreateDateTimeFormat ( newTarget, locales, options, required, defaults )
/// from step 2 on, on the object dateTimeFormat that step 1 made. required
/// is "date", "time" or "any", defaults "date", "time" or "all". The slots
/// are collected in a null-prototype record and attached at the end, with
/// the formatter's data, once every value is known; nothing between can
/// observe them. The data is the null-prototype record { languageId
/// (GetLocaleBaseName of [[Locale]], the language identifier native code
/// looks the data up by), localeData (cachedDateTimeLocaleData's record of
/// [[Locale]], [[Calendar]] and [[NumberingSystem]]), family (the
/// hour-cycle family whose records [[DateTimeFormat]] was matched in: the
/// resolved hour cycle's, or "h12" for a dayPeriod without an hour,
/// formatsFamilyOf),
/// dayPeriods (the dayPeriods list of languageId), rangePatterns (the
/// DateTime Range Pattern Record of [[DateTimeFormat]], decision 11),
/// symbols (the CLDR name symbols native code was initialized with,
/// decision 4), symbolList (those symbols as a List, made on first use),
/// patternParts (an empty null-prototype object in which the formatting
/// keeps the parsed parts of each pattern it has used), and nf, nf2 and nf3
/// (the internal NumberFormats of decision 12, undefined until the first
/// use; nf3 becomes a null-prototype object from the number of fraction
/// digits to its NumberFormat) }. Native code keeps the calendar, the time
/// zone and the names (dateTimeFormatInit). Returns dateTimeFormat.
function initializeDateTimeFormat(
  dateTimeFormat,
  locales,
  options,
  required,
  defaults,
) {
  // 2. Let specialOptions be the Record { [[Hour12]]: undefined }.
  var specialOptions = {__proto__: null, hour12: undefined};
  // 3. Let modifyResolutionOptions be a new Abstract Closure with
  //    parameters (options) that captures specialOptions and performs the
  //    following steps when called:
  var modifyResolutionOptions = opt => {
    // a. Set specialOptions.[[Hour12]] to options.[[hour12]].
    specialOptions.hour12 = opt.hour12;
    // b. Remove field [[hour12]] from options.
    delete opt.hour12;
    // c. If specialOptions.[[Hour12]] is not undefined, set
    //    options.[[hc]] to null.
    if (specialOptions.hour12 !== undefined) opt.hc = null;
  };
  // 4. Let optionsResolution be ? ResolveOptions(%Intl.DateTimeFormat%,
  //    %Intl.DateTimeFormat%.[[LocaleData]], locales, options,
  //    « coerce-options », modifyResolutionOptions).
  var optionsResolution = ResolveOptions(
    DATE_TIME_FORMAT_SERVICE,
    dateTimeFormatLocaleData,
    locales,
    options,
    ['coerce-options'],
    modifyResolutionOptions,
  );
  // 5. Let hour12 be specialOptions.[[Hour12]]. ResolveOptions's step
  //    6.d.i turned the Boolean option into the String "true" or "false".
  var hour12 = specialOptions.hour12;
  // 6. Set options to optionsResolution.[[Options]].
  options = optionsResolution.options;
  // 7. Let resolvedLocale be optionsResolution.[[ResolvedLocale]].
  var resolvedLocale = optionsResolution.resolvedLocale;
  var slots = {__proto__: null};
  // 8. Set dateTimeFormat.[[Locale]] to resolvedLocale.[[Locale]].
  slots.locale = resolvedLocale.locale;
  // 9. Let resolvedCalendar be resolvedLocale.[[ca]].
  var resolvedCalendar = resolvedLocale.ca;
  // 10. Set dateTimeFormat.[[Calendar]] to resolvedCalendar.
  slots.calendar = resolvedCalendar;
  // 11. Set dateTimeFormat.[[NumberingSystem]] to resolvedLocale.[[nu]].
  slots.numberingSystem = resolvedLocale.nu;
  // 12. Let resolvedLocaleData be resolvedLocale.[[LocaleData]].
  var resolvedLocaleData = resolvedLocale.localeData;
  // 13. Let hc be resolvedLocale.[[hc]].
  var hc = resolvedLocale.hc;
  if (hour12 === 'true') {
    // 14. If hour12 is true, then
    //     a. Set hc to resolvedLocaleData.[[hourCycle12]].
    hc = resolvedLocaleData.hourCycle12;
  } else if (hour12 === 'false') {
    // 15. Else if hour12 is false, then
    //     a. Set hc to resolvedLocaleData.[[hourCycle24]].
    hc = resolvedLocaleData.hourCycle24;
  } else if (hc === null) {
    // 16. Else if hc is null, then
    //     a. Set hc to resolvedLocaleData.[[hourCycle]].
    hc = resolvedLocaleData.hourCycle;
  }
  // 17. Let timeZone be ? GetOption(options, "timeZone", string, empty,
  //     SystemTimeZoneIdentifier()).
  var timeZone = GetOption(
    options,
    'timeZone',
    'string',
    undefined,
    SystemTimeZoneIdentifier(),
  );
  if (IsTimeZoneOffsetString(timeZone)) {
    // 18. If IsTimeZoneOffsetString(timeZone) is true, then
    //     a. Let parseResult be ParseText(StringToCodePoints(timeZone),
    //        UTCOffset).
    //     b. Assert: parseResult is a Parse Node.
    var parseResult = parseUTCOffset(timeZone);
    //     c. If parseResult contains more than one MinuteSecond Parse Node,
    //        throw a RangeError exception.
    if (parseResult.minuteSeconds.length > 1)
      throw new _RangeError('Invalid time zone specified: ' + timeZone);
    //     d. Let offsetNanoseconds be ParseTimeZoneOffsetString(timeZone).
    var offsetNanoseconds = ParseTimeZoneOffsetString(timeZone);
    //     e. Let offsetMinutes be offsetNanoseconds / (6 × 10^10).
    //     f. Assert: offsetMinutes is an integer.
    var offsetMinutes = offsetNanoseconds / 6e10;
    //     g. Set timeZone to FormatOffsetTimeZoneIdentifier(offsetMinutes).
    timeZone = FormatOffsetTimeZoneIdentifier(offsetMinutes);
  } else {
    // 19. Else,
    //     a. Let timeZoneIdentifierRecord be
    //        GetAvailableNamedTimeZoneIdentifier(timeZone).
    var timeZoneIdentifierRecord =
      GetAvailableNamedTimeZoneIdentifier(timeZone);
    //     b. If timeZoneIdentifierRecord is empty, throw a RangeError
    //        exception.
    if (timeZoneIdentifierRecord === undefined)
      throw new _RangeError('Invalid time zone specified: ' + timeZone);
    //     c. Set timeZone to timeZoneIdentifierRecord.[[PrimaryIdentifier]].
    timeZone = timeZoneIdentifierRecord.primaryIdentifier;
  }
  // 20. Set dateTimeFormat.[[TimeZone]] to timeZone.
  slots.timeZone = timeZone;
  // 21. Let formatOptions be a new Record.
  var formatOptions = {__proto__: null};
  // 22. Set formatOptions.[[hourCycle]] to hc.
  formatOptions.hourCycle = hc;
  // 23. Let hasExplicitFormatComponents be false.
  var hasExplicitFormatComponents = false;
  // 24. For each row of Table "Components of date and time formats", except
  //     the header row, in table order, do
  for (var i = 0; i < DATE_TIME_COMPONENTS.length; i++) {
    // a. Let propertyKey be the name given in the Property column of the
    //    current row.
    var propertyKey = DATE_TIME_COMPONENTS[i];
    var value;
    if (propertyKey === 'fractionalSecondDigits') {
      // b. If propertyKey is "fractionalSecondDigits", then
      //    i. Let value be ? GetNumberOption(options,
      //       "fractionalSecondDigits", 1, 3, undefined).
      value = GetNumberOption(
        options,
        'fractionalSecondDigits',
        1,
        3,
        undefined,
      );
    } else {
      // c. Else,
      //    i. Let values be a List whose elements are the strings given in
      //       the Values column of the current row.
      //    ii. Let value be ? GetOption(options, propertyKey, string,
      //        values, undefined).
      value = GetOption(
        options,
        propertyKey,
        'string',
        DATE_TIME_COMPONENT_VALUES[propertyKey],
        undefined,
      );
    }
    // d. Set formatOptions.[[<propertyKey>]] to value.
    formatOptions[propertyKey] = value;
    // e. If value is not undefined, set hasExplicitFormatComponents to
    //    true.
    if (value !== undefined) hasExplicitFormatComponents = true;
  }
  // 25. Let formatMatcher be ? GetOption(options, "formatMatcher", string,
  //     « "basic", "best fit" », "best fit").
  var formatMatcher = GetOption(
    options,
    'formatMatcher',
    'string',
    ['basic', 'best fit'],
    'best fit',
  );
  // 26. Let dateStyle be ? GetOption(options, "dateStyle", string,
  //     « "full", "long", "medium", "short" », undefined).
  var dateStyle = GetOption(
    options,
    'dateStyle',
    'string',
    DATE_TIME_STYLES,
    undefined,
  );
  // 27. Set dateTimeFormat.[[DateStyle]] to dateStyle.
  slots.dateStyle = dateStyle;
  // 28. Let timeStyle be ? GetOption(options, "timeStyle", string,
  //     « "full", "long", "medium", "short" », undefined).
  var timeStyle = GetOption(
    options,
    'timeStyle',
    'string',
    DATE_TIME_STYLES,
    undefined,
  );
  // 29. Set dateTimeFormat.[[TimeStyle]] to timeStyle.
  slots.timeStyle = timeStyle;
  //     The records are keyed by calendar and hour-cycle family (decision
  //     6), and by the decimal separator of [[NumberingSystem]] (decision
  //     13); they are requested where the draft reads them.
  var languageId = GetLocaleBaseName(slots.locale);
  var family = hourCycleFamily(hc);
  var localeData;
  var bestFormat;
  if (dateStyle !== undefined || timeStyle !== undefined) {
    // 30. If dateStyle is not undefined or timeStyle is not undefined, then
    //     a. If hasExplicitFormatComponents is true, throw a TypeError
    //        exception.
    if (hasExplicitFormatComponents)
      throw new _TypeError(
        'dateStyle and timeStyle cannot be used with date or time ' +
          'component options',
      );
    //     b. If required is date and timeStyle is not undefined, throw a
    //        TypeError exception.
    if (required === 'date' && timeStyle !== undefined)
      throw new _TypeError('timeStyle cannot be used here');
    //     c. If required is time and dateStyle is not undefined, throw a
    //        TypeError exception.
    if (required === 'time' && dateStyle !== undefined)
      throw new _TypeError('dateStyle cannot be used here');
    //     d. Let styles be resolvedLocaleData.[[styles]].[[<
    //        resolvedCalendar>]], here of hc's family.
    localeData = dateTimeFormatsOf(slots, languageId);
    var styles = localeData.styles[family];
    //     e. Let bestFormat be DateTimeStyleFormat(dateStyle, timeStyle,
    //        styles).
    bestFormat = DateTimeStyleFormat(dateStyle, timeStyle, styles);
  } else {
    // 31. Else,
    //     a. Let needDefaults be true.
    var needDefaults = true;
    //     b. Let propertyKeys be a new empty List.
    var propertyKeys = [];
    //     c. If required is date or any, set propertyKeys to the
    //        list-concatenation of propertyKeys and « "weekday", "year",
    //        "month", "day" ».
    if (required === 'date' || required === 'any') {
      listPush(propertyKeys, 'weekday');
      listPush(propertyKeys, 'year');
      listPush(propertyKeys, 'month');
      listPush(propertyKeys, 'day');
    }
    //     d. If required is time or any, set propertyKeys to the
    //        list-concatenation of propertyKeys and « "dayPeriod", "hour",
    //        "minute", "second", "fractionalSecondDigits" ».
    if (required === 'time' || required === 'any') {
      listPush(propertyKeys, 'dayPeriod');
      listPush(propertyKeys, 'hour');
      listPush(propertyKeys, 'minute');
      listPush(propertyKeys, 'second');
      listPush(propertyKeys, 'fractionalSecondDigits');
    }
    //     e. For each property name propertyKey of propertyKeys, do
    for (var j = 0; j < propertyKeys.length; j++) {
      // i. Let value be formatOptions.[[<propertyKey>]].
      // ii. If value is not undefined, set needDefaults to false.
      if (formatOptions[propertyKeys[j]] !== undefined) needDefaults = false;
    }
    //     f. If needDefaults is true, then
    if (needDefaults) {
      // i. Let numericProperties be a new empty List.
      var numericProperties = [];
      // ii. If defaults is either date or all, set numericProperties to
      //     the list-concatenation of numericProperties and « "year",
      //     "month", "day" ».
      if (defaults === 'date' || defaults === 'all') {
        listPush(numericProperties, 'year');
        listPush(numericProperties, 'month');
        listPush(numericProperties, 'day');
      }
      // iii. If defaults is either time or all, set numericProperties to
      //      the list-concatenation of numericProperties and « "hour",
      //      "minute", "second" ».
      if (defaults === 'time' || defaults === 'all') {
        listPush(numericProperties, 'hour');
        listPush(numericProperties, 'minute');
        listPush(numericProperties, 'second');
      }
      // iv. For each property name propertyKey of numericProperties, do
      //     1. Set formatOptions.[[<propertyKey>]] to "numeric".
      for (var k = 0; k < numericProperties.length; k++)
        formatOptions[numericProperties[k]] = 'numeric';
    }
    //     g. Let formats be resolvedLocaleData.[[formats]].[[<
    //        resolvedCalendar>]], here of hc's family, or of "h12" for a
    //        request with a dayPeriod and no hour (formatsFamilyOf).
    family = formatsFamilyOf(formatOptions, family);
    localeData = dateTimeFormatsOf(slots, languageId);
    //     h. If formatMatcher is "basic", let bestFormat be
    //        BasicFormatMatcher(formatOptions, formats); else let
    //        bestFormat be BestFitFormatMatcher(formatOptions, formats).
    //        The result is cached in localeData (cachedFormatMatch).
    bestFormat = cachedFormatMatch(
      formatOptions,
      localeData,
      family,
      formatMatcher,
    );
  }
  // 32. Set dateTimeFormat.[[DateTimeFormat]] to bestFormat.
  slots.dateTimeFormat = bestFormat;
  // 33. If bestFormat has a field [[hour]], set
  //     dateTimeFormat.[[HourCycle]] to hc.
  if (bestFormat.hour !== undefined) slots.hourCycle = hc;
  //     Initialize the native state with the calendar, the time zone and
  //     the names of bestFormat and its range patterns (decision 4), then
  //     attach the slots and the data.
  var rangePatterns = rangePatternsOf(bestFormat, localeData, family);
  var dayPeriods = cachedDayPeriods(languageId);
  var symbols = nameSymbolsOf(
    bestFormat,
    rangePatterns,
    resolvedCalendar,
    dayPeriods,
  );
  native.dateTimeFormatInit(
    dateTimeFormat,
    languageId,
    resolvedCalendar,
    timeZone,
    symbols,
  );
  new DateTimeFormatSlots(dateTimeFormat, slots, {
    __proto__: null,
    languageId: languageId,
    localeData: localeData,
    family: family,
    dayPeriods: dayPeriods,
    rangePatterns: rangePatterns,
    symbols: symbols,
    symbolList: undefined,
    patternParts: {__proto__: null},
    nf: undefined,
    nf2: undefined,
    nf3: undefined,
  });
  // 34. Return dateTimeFormat.
  return dateTimeFormat;
}

// ChainDateTimeFormat ( dateTimeFormat, newTarget, this )
function ChainDateTimeFormat(dateTimeFormat, newTarget, thisValue) {
  // 1. If newTarget is undefined and ? OrdinaryHasInstance(
  //    %Intl.DateTimeFormat%, this) is true, then
  if (
    newTarget === undefined &&
    OrdinaryHasInstance(DateTimeFormat, thisValue)
  ) {
    // a. Perform ? DefinePropertyOrThrow(this, %Intl%.[[FallbackSymbol]],
    //    PropertyDescriptor { [[Value]]: dateTimeFormat, [[Writable]]:
    //    false, [[Enumerable]]: false, [[Configurable]]: false }).
    //    The captured Object.defineProperty throws the TypeError when the
    //    definition fails.
    ObjectDefineProperty(thisValue, IntlFallbackSymbol, {
      __proto__: null,
      value: dateTimeFormat,
      writable: false,
      enumerable: false,
      configurable: false,
    });
    // b. Return this.
    return thisValue;
  }
  // 2. Return dateTimeFormat.
  return dateTimeFormat;
}

// Intl.DateTimeFormat ( [ locales [ , options ] ] )
function DateTimeFormat(locales = undefined, options = undefined) {
  // 1. If NewTarget is undefined, let newTarget be the active function
  //    object, else let newTarget be NewTarget.
  // 2. Let dateTimeFormat be ? CreateDateTimeFormat(newTarget, locales,
  //    options, any, date).
  //    CreateDateTimeFormat's step 1, OrdinaryCreateFromConstructor: with
  //    new, Hermes created this from NewTarget before the call, reading
  //    its "prototype" once (design spec "Constructors"). Without new,
  //    newTarget is %Intl.DateTimeFormat%, whose "prototype" is the
  //    non-writable, non-configurable %Intl.DateTimeFormat.prototype%, so
  //    the object is made from that. initializeDateTimeFormat performs the
  //    other steps and attaches the slots.
  var dateTimeFormat =
    new.target === undefined ? ObjectCreate(DateTimeFormatPrototype) : this;
  initializeDateTimeFormat(dateTimeFormat, locales, options, 'any', 'date');
  // 3. If the implementation supports the normative optional constructor
  //    mode of 4.3 Note 3, then
  //    a. Let this be the this value.
  //    b. Return ? ChainDateTimeFormat(dateTimeFormat, NewTarget, this).
  //    The plugin supports it (design spec "Legacy constructor
  //    behavior"); step 4, "Return dateTimeFormat", is not reached.
  return ChainDateTimeFormat(dateTimeFormat, new.target, this);
}

/// CreateDateTimeFormat(%Intl.DateTimeFormat%, locales, options, required,
/// defaults) for the plugin's own callers, such as the Date overrides
/// (design spec "Resistance to tampering"): every step, with newTarget
/// %Intl.DateTimeFormat%, without going through any public property.
function CreateDateTimeFormat(locales, options, required, defaults) {
  // 1. Let dateTimeFormat be OrdinaryCreateFromConstructor(
  //    %Intl.DateTimeFormat%, "%Intl.DateTimeFormat.prototype%", ...): an
  //    object made from %Intl.DateTimeFormat.prototype%.
  // 2-34. Initialize it, and return it.
  return initializeDateTimeFormat(
    ObjectCreate(DateTimeFormatPrototype),
    locales,
    options,
    required,
    defaults,
  );
}

// UnwrapDateTimeFormat ( dtf )
function UnwrapDateTimeFormat(dtf) {
  // 1. If dtf is not an Object, throw a TypeError exception.
  if (!isObject(dtf))
    throw new _TypeError('Receiver is not an Intl.DateTimeFormat object');
  // 2. If dtf does not have an [[InitializedDateTimeFormat]] internal slot
  //    and ? OrdinaryHasInstance(%Intl.DateTimeFormat%, dtf) is true, then
  //    a. Return ? Get(dtf, %Intl%.[[FallbackSymbol]]).
  if (
    !DateTimeFormatSlots.is(dtf) &&
    OrdinaryHasInstance(DateTimeFormat, dtf)
  )
    return dtf[IntlFallbackSymbol];
  // 3. Return dtf.
  return dtf;
}

/// The bound format function of dtf: step 4.a of get
/// Intl.DateTimeFormat.prototype.format. An arrow function returned from
/// here has the name "" and length 1, and is not a constructor.
function makeBoundDateTimeFormat(dtf) {
  // a. Let closure be a new Abstract Closure with parameters (date) that
  //    captures dtf and performs the following steps when called:
  // b. Set dtf.[[BoundFormat]] to CreateBuiltinFunction(closure, 1, "",
  //    « »). (The caller does this.)
  return date => {
    // i. If date is undefined, let x be ! Call(%Date.now%, undefined); else
    //    let x be ? ToNumber(date).
    var x = date === undefined ? DateNow() : ToNumber(date);
    // ii. Return ? FormatDateTime(dtf, x).
    return FormatDateTime(dtf, x);
  };
}

/// The rows of the table "Resolved Options of DateTimeFormat Instances", in
/// table order: [Property, Internal Slot (undefined for none), Conversion
/// (undefined for none)].
var DATE_TIME_FORMAT_RESOLVED_OPTIONS = [
  ['locale', 'locale', undefined],
  ['calendar', 'calendar', undefined],
  ['numberingSystem', 'numberingSystem', undefined],
  ['timeZone', 'timeZone', undefined],
  ['hourCycle', 'hourCycle', undefined],
  ['hour12', 'hourCycle', 'hour12'],
  ['weekday', undefined, undefined],
  ['era', undefined, undefined],
  ['year', undefined, undefined],
  ['month', undefined, undefined],
  ['day', undefined, undefined],
  ['dayPeriod', undefined, undefined],
  ['hour', undefined, undefined],
  ['minute', undefined, undefined],
  ['second', undefined, undefined],
  ['fractionalSecondDigits', undefined, 'number'],
  ['timeZoneName', undefined, undefined],
  ['dateStyle', 'dateStyle', undefined],
  ['timeStyle', 'timeStyle', undefined],
];

/// Install Intl.DateTimeFormat on intl; called by installIntl.
function installDateTimeFormat(intl) {
  // 1. Let proto be %Intl.DateTimeFormat.prototype%, an ordinary object
  //    whose [[Prototype]] is %Object.prototype% ("Properties of the
  //    Intl.DateTimeFormat Prototype Object").
  var proto = {};
  DateTimeFormatPrototype = proto;
  // 2. "Properties of the Intl.DateTimeFormat Constructor":
  //    Intl.DateTimeFormat.prototype, with its length 0, and
  //    Intl.DateTimeFormat.prototype.constructor.
  defineConstructor(DateTimeFormat, 0, proto);
  // 3. The method properties of the constructor.
  defineMethods(DateTimeFormat, {
    // Intl.DateTimeFormat.supportedLocalesOf ( locales [ , options ] )
    supportedLocalesOf(locales, options = undefined) {
      // 1. Let availableLocales be
      //    %Intl.DateTimeFormat%.[[AvailableLocales]].
      // 2. Let requestedLocales be ? CanonicalizeLocaleList(locales).
      var requestedLocales = CanonicalizeLocaleList(locales);
      // 3. Return ? FilterLocales(availableLocales, requestedLocales,
      //    options).
      return FilterLocales(isAvailableLocale, requestedLocales, options);
    },
  });
  // 4. The properties of the prototype, in the draft's order.
  defineMethods(proto, {
    // Intl.DateTimeFormat.prototype.resolvedOptions ( )
    resolvedOptions() {
      // 1. Let dtf be the this value.
      // 2. If the implementation supports the normative optional
      //    constructor mode of 4.3 Note 3, then
      //    a. Set dtf to ? UnwrapDateTimeFormat(dtf).
      var dtf = UnwrapDateTimeFormat(this);
      // 3. Perform ? RequireInternalSlot(dtf, [[InitializedDateTimeFormat]]).
      requireInitializedDateTimeFormat(dtf);
      // 4. Let options be OrdinaryObjectCreate(%Object.prototype%).
      var options = {};
      var format = DateTimeFormatSlots.get(dtf, 'dateTimeFormat');
      var hasStyle =
        DateTimeFormatSlots.get(dtf, 'dateStyle') !== undefined ||
        DateTimeFormatSlots.get(dtf, 'timeStyle') !== undefined;
      // 5. For each row of Table "Resolved Options of DateTimeFormat
      //    Instances", except the header row, in table order, do
      for (var i = 0; i < DATE_TIME_FORMAT_RESOLVED_OPTIONS.length; i++) {
        var row = DATE_TIME_FORMAT_RESOLVED_OPTIONS[i];
        // a. Let propertyKey be the Property value of the current row.
        var propertyKey = row[0];
        var value;
        if (row[1] !== undefined) {
          // b. If there is an Internal Slot value in the current row, then
          //    i. Let value be the value of dtf's internal slot whose name
          //       is the Internal Slot value of the current row.
          value = DateTimeFormatSlots.get(dtf, row[1]);
        } else if (format[propertyKey] !== undefined && !hasStyle) {
          // c. Else,
          //    i. Let format be dtf.[[DateTimeFormat]].
          //    ii. If format has a field [[<propertyKey>]] and
          //        dtf.[[DateStyle]] is undefined and dtf.[[TimeStyle]] is
          //        undefined, then
          //        1. Let value be format.[[<propertyKey>]].
          value = format[propertyKey];
        } else {
          //    iii. Else,
          //         1. Let value be undefined.
          value = undefined;
        }
        // d. If value is not undefined, then
        if (value !== undefined) {
          // i. If there is a Conversion value in the current row, then
          //    1. Let conversion be the Conversion value of the current row.
          //    2. If conversion is hour12, then
          //       a. If value is "h11" or "h12", set value to true.
          //          Otherwise, set value to false.
          //    3. Else,
          //       a. Assert: conversion is number.
          //       b. Set value to 𝔽(value). The record holds a Number.
          if (row[2] === 'hour12') value = value === 'h11' || value === 'h12';
          // ii. Perform ! CreateDataPropertyOrThrow(options, propertyKey,
          //     value).
          CreateDataPropertyOrThrow(options, propertyKey, value);
        }
      }
      // 6. Return options.
      return options;
    },
  });
  defineGetters(proto, {
    // get Intl.DateTimeFormat.prototype.format
    get format() {
      // 1. Let dtf be the this value.
      // 2. If the implementation supports the normative optional
      //    constructor mode of 4.3 Note 3, then
      //    a. Set dtf to ? UnwrapDateTimeFormat(dtf).
      var dtf = UnwrapDateTimeFormat(this);
      // 3. Perform ? RequireInternalSlot(dtf, [[InitializedDateTimeFormat]]).
      requireInitializedDateTimeFormat(dtf);
      // 4. If dtf.[[BoundFormat]] is undefined, then
      //    a-b. Set dtf.[[BoundFormat]] to a new built-in function that
      //         formats with dtf (makeBoundDateTimeFormat).
      if (DateTimeFormatSlots.get(dtf, 'boundFormat') === undefined)
        DateTimeFormatSlots.setBoundFormat(dtf, makeBoundDateTimeFormat(dtf));
      // 5. Return dtf.[[BoundFormat]].
      return DateTimeFormatSlots.get(dtf, 'boundFormat');
    },
  });
  defineMethods(proto, {
    // Intl.DateTimeFormat.prototype.formatRange ( startDate, endDate )
    formatRange(startDate, endDate) {
      // 1. Let dtf be this value.
      var dtf = this;
      // 2. Perform ? RequireInternalSlot(dtf, [[InitializedDateTimeFormat]]).
      requireInitializedDateTimeFormat(dtf);
      // 3. If startDate is undefined or endDate is undefined, throw a
      //    TypeError exception.
      if (startDate === undefined || endDate === undefined)
        throw new _TypeError('startDate and endDate are required');
      // 4. Let x be ? ToNumber(startDate).
      var x = ToNumber(startDate);
      // 5. Let y be ? ToNumber(endDate).
      var y = ToNumber(endDate);
      // 6. Return ? FormatDateTimeRange(dtf, x, y).
      return FormatDateTimeRange(dtf, x, y);
    },
    // Intl.DateTimeFormat.prototype.formatRangeToParts ( startDate,
    //                                                    endDate )
    formatRangeToParts(startDate, endDate) {
      // 1. Let dtf be this value.
      var dtf = this;
      // 2. Perform ? RequireInternalSlot(dtf, [[InitializedDateTimeFormat]]).
      requireInitializedDateTimeFormat(dtf);
      // 3. If startDate is undefined or endDate is undefined, throw a
      //    TypeError exception.
      if (startDate === undefined || endDate === undefined)
        throw new _TypeError('startDate and endDate are required');
      // 4. Let x be ? ToNumber(startDate).
      var x = ToNumber(startDate);
      // 5. Let y be ? ToNumber(endDate).
      var y = ToNumber(endDate);
      // 6. Return ? FormatDateTimeRangeToParts(dtf, x, y).
      return FormatDateTimeRangeToParts(dtf, x, y);
    },
    // Intl.DateTimeFormat.prototype.formatToParts ( date )
    formatToParts(date) {
      // 1. Let dtf be the this value.
      var dtf = this;
      // 2. Perform ? RequireInternalSlot(dtf, [[InitializedDateTimeFormat]]).
      requireInitializedDateTimeFormat(dtf);
      var x;
      if (date === undefined) {
        // 3. If date is undefined, then
        //    a. Let x be ! Call(%Date.now%, undefined).
        x = DateNow();
      } else {
        // 4. Else,
        //    a. Let x be ? ToNumber(date).
        x = ToNumber(date);
      }
      // 5. Return ? FormatDateTimeToParts(dtf, x).
      return FormatDateTimeToParts(dtf, x);
    },
  });
  // 5. Intl.DateTimeFormat.prototype [ %Symbol.toStringTag% ]:
  //    "Intl.DateTimeFormat", { [[Writable]]: false, [[Enumerable]]:
  //    false, [[Configurable]]: true }.
  setToStringTag(proto, 'Intl.DateTimeFormat');
  // 6. "The Intl.DateTimeFormat Constructor": the initial value of the
  //    "DateTimeFormat" property of the Intl object. ECMA-262 18 gives such
  //    data properties the attributes { [[Writable]]: true,
  //    [[Enumerable]]: false, [[Configurable]]: true }.
  ObjectDefineProperty(intl, 'DateTimeFormat', {
    __proto__: null,
    value: DateTimeFormat,
    writable: true,
    enumerable: false,
    configurable: true,
  });
}
