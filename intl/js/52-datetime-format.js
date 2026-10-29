/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// ECMA-402 "Abstract Operations for DateTimeFormat Objects": the formatting
// operations of an Intl.DateTimeFormat, following the draft
// https://tc39.es/ecma402/ dated October 24, 2026: ToLocalTime,
// FormatDateTimePattern, PartitionDateTimePattern, FormatDateTime,
// FormatDateTimeToParts, PartitionDateTimeRangePattern, FormatDateTimeRange
// and FormatDateTimeRangeToParts, with ECMA-262's TimeClip (21.4.1.31 in
// the https://tc39.es/ecma262/ draft of the same date). 50-DateTimeFormat.js
// calls them from the format getter's bound function, formatToParts,
// formatRange and formatRangeToParts, after their receiver checks and
// conversions. The range patterns PartitionDateTimeRangePattern reads are
// built by 53-datetime-ranges.js.
//
// The draft passes an instant as epoch nanoseconds, a BigInt. The plugin
// passes the time value TimeClip made, a Number of milliseconds, which
// holds the same instant (the draft multiplies an integer by 10^6), so no
// BigInt is made.

// ECMA-262 21.4.1.31 TimeClip ( time )
function TimeClip(time) {
  // 1. If time is not finite, return NaN.
  if (!NumberIsFinite(time)) return NaN;
  // 2. If abs(ℝ(time)) > 8.64 × 10^15, return NaN.
  if (time > 8.64e15 || time < -8.64e15) return NaN;
  // 3. Return 𝔽(! ToIntegerOrInfinity(time)). -0 becomes +0.
  return ToIntegerOrInfinity(time);
}

/// The indices of a dateTimeFormatLocalTime list
/// (intl/jsi/NativeHelpers.def).
var LT_WEEKDAY = 0;
var LT_ERA = 1;
var LT_YEAR = 2;
var LT_RELATED_YEAR = 3;
var LT_MONTH = 4;
var LT_MONTH_CODE = 5;
var LT_LEAP = 6;
var LT_DAY = 7;
var LT_HOUR = 8;
var LT_MINUTE = 9;
var LT_SECOND = 10;
var LT_MILLISECOND = 11;
var LT_IN_DST = 12;
var LT_OFFSET = 13;
var LT_NAMES = 14;

/// The CLDR name symbols dateTimeFormatInit was given for data, the
/// formatter's data record, as a List in their order: data.symbols split at
/// its spaces. Made on first use and kept in data.
function nameSymbolListOf(data) {
  // 1. If data has the List, return it.
  if (data.symbolList !== undefined) return data.symbolList;
  // 2. Split data.symbols at each space, without String.prototype.split
  //    (rule 2 of intl/doc/Development.md).
  var list = [];
  var symbols = data.symbols;
  var start = 0;
  while (start < symbols.length) {
    var end = stringIndexOf(symbols, ' ', start);
    if (end === -1) end = symbols.length;
    listPush(list, stringSlice(symbols, start, end));
    start = end + 1;
  }
  // 3. Keep it in data, and return it.
  data.symbolList = list;
  return list;
}

// ToLocalTime ( epochNs, calendar, timeZoneIdentifier )
// Decision 3 of the Phase 4 plan: native code computes the record for the
// time value epochMs (the draft's epochNs / 10^6) in dateTimeFormat's
// [[Calendar]] and [[TimeZone]], which its native state holds, and the
// names of the instant for the CLDR symbols the formatter was initialized
// with (decision 4). Returns the null-prototype ToLocalTime Record
// { weekday, era (ICU4X's era code, "" for chinese, dangi and iso8601),
// year, relatedYear, yearName, month (zero-based), monthCode, leapMonth,
// day, hour, minute, second, millisecond, inDST, offsetSeconds, names (a
// null-prototype object from each symbol to its text) }.
function ToLocalTime(dateTimeFormat, epochMs) {
  var data = DateTimeFormatSlots.get(dateTimeFormat, 'data');
  var symbols = nameSymbolListOf(data);
  // 1. If IsTimeZoneOffsetString(timeZoneIdentifier) is true, then
  //    a. Let offsetNs be ParseTimeZoneOffsetString(timeZoneIdentifier).
  // 2. Else,
  //    a. Assert: GetAvailableNamedTimeZoneIdentifier(timeZoneIdentifier)
  //       is not empty.
  //    b. Let offsetNs be GetNamedTimeZoneOffsetNanoseconds(
  //       timeZoneIdentifier, epochNs).
  // 3. Let tz be ℝ(epochNs) + offsetNs.
  // 4. If calendar is "gregory", then
  //    a. Return a ToLocalTime Record with fields calculated from tz
  //       according to Table "Record returned by ToLocalTime".
  // 5. Else,
  //    a. Return a ToLocalTime Record with the fields calculated from tz
  //       for the given calendar.
  //    Native code performs every step, for an epochMs that TimeClip made.
  var fields = native.dateTimeFormatLocalTime(dateTimeFormat, epochMs);
  var names = {__proto__: null};
  for (var i = 0; i < symbols.length; i++)
    names[symbols[i]] = fields[LT_NAMES + i];
  var relatedYear = fields[LT_RELATED_YEAR];
  return {
    __proto__: null,
    weekday: +fields[LT_WEEKDAY],
    era: fields[LT_ERA],
    year: +fields[LT_YEAR],
    relatedYear: relatedYear === '' ? undefined : +relatedYear,
    yearName: names.U,
    // [[Month]] is zero-based; native code gives the number shown.
    month: +fields[LT_MONTH] - 1,
    monthCode: fields[LT_MONTH_CODE],
    leapMonth: fields[LT_LEAP] === '1',
    day: +fields[LT_DAY],
    hour: +fields[LT_HOUR],
    minute: +fields[LT_MINUTE],
    second: +fields[LT_SECOND],
    millisecond: +fields[LT_MILLISECOND],
    inDST: fields[LT_IN_DST] === '1',
    offsetSeconds: +fields[LT_OFFSET],
    names: names,
  };
}

/// Decision 12: an internal Intl.NumberFormat of dateTimeFormat's
/// [[Locale]] and [[NumberingSystem]] without grouping, with
/// minimumIntegerDigits when it is not undefined: the draft's
/// Construct(%Intl.NumberFormat%, « locale, options ») with a
/// null-prototype options object, through the internal creation function,
/// so user code never sees it.
function dateTimeNumberFormat(dateTimeFormat, minimumIntegerDigits) {
  // 1. Let options be OrdinaryObjectCreate(null).
  var options = {__proto__: null};
  // 2. If minimumIntegerDigits is not undefined, perform !
  //    CreateDataPropertyOrThrow(options, "minimumIntegerDigits",
  //    𝔽(minimumIntegerDigits)).
  if (minimumIntegerDigits !== undefined)
    options.minimumIntegerDigits = minimumIntegerDigits;
  // 3. Perform ! CreateDataPropertyOrThrow(options, "numberingSystem",
  //    dateTimeFormat.[[NumberingSystem]]).
  options.numberingSystem = DateTimeFormatSlots.get(
    dateTimeFormat,
    'numberingSystem',
  );
  // 4. Perform ! CreateDataPropertyOrThrow(options, "useGrouping", false).
  options.useGrouping = false;
  // 5. Return ! Construct(%Intl.NumberFormat%, « dateTimeFormat.[[Locale]],
  //    options »).
  return CreateNumberFormat(
    DateTimeFormatSlots.get(dateTimeFormat, 'locale'),
    options,
  );
}

/// FormatNumeric(nf, value) for the integer Number value, where nf is
/// data's "nf" or "nf2", or nf3 of digits ("nf3"), made by
/// dateTimeNumberFormat the first time dateTimeFormat needs it and kept in
/// data (decision 12). data.nf3 is a null-prototype object from the
/// number of digits to its NumberFormat.
function formatDateTimeNumber(dateTimeFormat, data, which, digits, value) {
  var nf;
  if (which === 'nf') {
    // 1. nf: no minimum.
    if (data.nf === undefined)
      data.nf = dateTimeNumberFormat(dateTimeFormat, undefined);
    nf = data.nf;
  } else if (which === 'nf2') {
    // 2. nf2: at least two digits.
    if (data.nf2 === undefined)
      data.nf2 = dateTimeNumberFormat(dateTimeFormat, 2);
    nf = data.nf2;
  } else {
    // 3. nf3: at least digits digits.
    if (data.nf3 === undefined) data.nf3 = {__proto__: null};
    if (data.nf3[digits] === undefined)
      data.nf3[digits] = dateTimeNumberFormat(dateTimeFormat, digits);
    nf = data.nf3[digits];
  }
  // 4. Return FormatNumeric(nf, value).
  return FormatNumeric(nf, IntlMVFromNumber(value));
}

/// The last two code points of s, or s if it has fewer
/// (FormatDateTimePattern step 15.f.ix.2-4): a digit outside the BMP is
/// two code units, so the code points are counted from the end of s.
function lastTwoCodePoints(s) {
  // 1. Let start be the length of s; step back over two code points, a
  //    trail surrogate after a lead surrogate being one.
  var start = s.length;
  for (var n = 0; n < 2 && start > 0; n++) {
    start--;
    var c = charCodeAt(s, start);
    if (c >= 0xdc00 && c <= 0xdfff && start > 0) {
      var lead = charCodeAt(s, start - 1);
      if (lead >= 0xd800 && lead <= 0xdbff) start--;
    }
  }
  // 2. Return the substring of s from start.
  return stringSlice(s, start);
}

/// PartitionPattern(pattern) through dateTimeFormat's cache of parsed
/// patterns, data.patternParts (decision 13). The List is shared and never
/// modified.
function dateTimePatternParts(data, pattern) {
  // 1. If the cache has the parts of pattern, return them.
  var parts = data.patternParts[pattern];
  if (parts !== undefined) return parts;
  // 2. Otherwise partition pattern, cache the parts, and return them.
  parts = PartitionPattern(pattern);
  data.patternParts[pattern] = parts;
  return parts;
}

/// The text native code gave localTime for the name of the placeholder
/// partType with the widths and contexts of format, in calendar (decision
/// 4). A name dateTimeFormatInit was not asked for is an internal error.
function localTimeName(localTime, partType, format, calendar) {
  // 1. Let symbol be the CLDR symbol of the placeholder at format's width.
  var symbol = nameSymbolOf(partType, format, calendar, undefined);
  // 2. Return its text.
  var name = symbol === undefined ? undefined : localTime.names[symbol];
  if (name === undefined) {
    throw new _TypeError(
      'Intl internal error: no DateTimeFormat name for ' + partType,
    );
  }
  return name;
}

/// Append the Record { [[Type]]: type, [[Value]]: value } to result.
function appendDateTimePart(result, type, value) {
  // 1. Append the null-prototype record.
  listPush(result, {__proto__: null, type: type, value: value});
}

// FormatDateTimePattern ( dateTimeFormat, format, pattern, epochNanoseconds )
// epochMs is the time value TimeClip made (see the file header).
// PartitionDateTimeRangePattern passes the ToLocalTime Record it already
// has for epochMs as localTime; ToLocalTime gives the same record for the
// same instant, so step 12 reuses it.
function FormatDateTimePattern(
  dateTimeFormat,
  format,
  pattern,
  epochMs,
  localTime = undefined,
) {
  var data = DateTimeFormatSlots.get(dateTimeFormat, 'data');
  var calendar = DateTimeFormatSlots.get(dateTimeFormat, 'calendar');
  var hourCycle = DateTimeFormatSlots.get(dateTimeFormat, 'hourCycle');
  // 1. Let locale be dateTimeFormat.[[Locale]].
  // 2-10. Let nf be a NumberFormat of locale with numberingSystem
  //       dateTimeFormat.[[NumberingSystem]] and useGrouping false, and nf2
  //       the same with minimumIntegerDigits 2.
  // 11. If format has a field [[fractionalSecondDigits]], then
  //     a. Let fractionalSecondDigits be format.[[fractionalSecondDigits]].
  //     b-f. Let nf3 be the same with minimumIntegerDigits
  //          fractionalSecondDigits.
  //     Decision 12: each is made on its first use and kept
  //     (formatDateTimeNumber).
  var fractionalSecondDigits = format.fractionalSecondDigits;
  // 12. Let localTime be ToLocalTime(epochNanoseconds,
  //     dateTimeFormat.[[Calendar]], dateTimeFormat.[[TimeZone]]).
  if (localTime === undefined)
    localTime = ToLocalTime(dateTimeFormat, epochMs);
  // 13. Let patternParts be PartitionPattern(pattern).
  var patternParts = dateTimePatternParts(data, pattern);
  // 14. Let result be a new empty List.
  var result = [];
  // 15. For each Record { [[Type]], [[Value]] } patternPart of
  //     patternParts, do
  for (var i = 0; i < patternParts.length; i++) {
    var patternPart = patternParts[i];
    // a. Let partType be patternPart.[[Type]].
    var partType = patternPart.type;
    var fieldFormat;
    var value;
    var formattedValue;
    if (partType === 'literal') {
      // b. If partType is "literal", then
      //    i. Append the Record { [[Type]]: "literal", [[Value]]:
      //       patternPart.[[Value]] } to result.
      appendDateTimePart(result, 'literal', patternPart.value);
    } else if (partType === 'fractionalSecondDigits') {
      // c. Else if partType is "fractionalSecondDigits", then
      //    i. Assert: format has a field [[fractionalSecondDigits]].
      //    ii. Let value be localTime.[[Millisecond]].
      //    iii. Set value to floor(value × 10^(fractionalSecondDigits - 3)).
      //         Dividing the integer by 10^(3 - fractionalSecondDigits)
      //         gives the same value without a rounded power of ten.
      value = MathFloor(
        localTime.millisecond /
          (fractionalSecondDigits === 1
            ? 100
            : fractionalSecondDigits === 2
            ? 10
            : 1),
      );
      //    iv. Let formattedValue be FormatNumeric(nf3, value).
      formattedValue = formatDateTimeNumber(
        dateTimeFormat,
        data,
        'nf3',
        fractionalSecondDigits,
        value,
      );
      //    v. Append the Record { [[Type]]: "fractionalSecond", [[Value]]:
      //       formattedValue } to result.
      appendDateTimePart(result, 'fractionalSecond', formattedValue);
    } else if (partType === 'dayPeriod') {
      // d. Else if partType is "dayPeriod", then
      //    i. Assert: format has a field [[dayPeriod]]. A B the request did
      //       not ask for has no field (decision 6), so the width is the
      //       one {dayPeriod} shows, format.dayPeriodWidth, which is
      //       [[dayPeriod]] when the field is there.
      //    ii. Let fieldFormat be format.[[dayPeriod]].
      fieldFormat = format.dayPeriodWidth;
      //    iii. Let formattedValue be an ILD String representing the day
      //         period of localTime in the form given by fieldFormat:
      //         CLDR's flexible day period (decision 4), or the hour's
      //         AM/PM name in the same width where CLDR has no name.
      var id = dayPeriodOf(
        data.dayPeriods,
        localTime.hour,
        localTime.minute,
        localTime.second,
        format.minute !== undefined,
        format.second !== undefined,
      );
      formattedValue = dayPeriodName(data.dayPeriods, id, fieldFormat);
      if (formattedValue === '') {
        formattedValue =
          localTime.names[DAY_PERIOD_AMPM_SYMBOLS[fieldFormat]];
        if (formattedValue === undefined) {
          throw new _TypeError(
            'Intl internal error: no DateTimeFormat name for ampm',
          );
        }
      }
      //    iv. Append the Record { [[Type]]: partType, [[Value]]:
      //        formattedValue } to result.
      appendDateTimePart(result, partType, formattedValue);
    } else if (partType === 'timeZoneName') {
      // e. Else if partType is "timeZoneName", then
      //    i. Assert: format has a field [[timeZoneName]].
      //    ii. Let fieldFormat be format.[[timeZoneName]].
      //    iii. Let value be dateTimeFormat.[[TimeZone]].
      //    iv. Let formattedValue be an ILD String representing value in
      //        the form given by fieldFormat: ICU4X's name (decision 4),
      //        whose ASCII digits are written in [[NumberingSystem]]'s.
      var numberingSystem = DateTimeFormatSlots.get(
        dateTimeFormat,
        'numberingSystem',
      );
      formattedValue = transliterate(
        localTimeName(localTime, partType, format, calendar),
        numberingSystem === 'latn'
          ? undefined
          : NUMBERING_SYSTEM_DIGITS[numberingSystem],
      );
      //    v. Append the Record { [[Type]]: partType, [[Value]]:
      //       formattedValue } to result.
      appendDateTimePart(result, partType, formattedValue);
    } else if (
      partType === 'weekday' ||
      partType === 'era' ||
      partType === 'year' ||
      partType === 'month' ||
      partType === 'day' ||
      partType === 'hour' ||
      partType === 'minute' ||
      partType === 'second'
    ) {
      // f. Else if partType matches a Property column of the row in Table
      //    "Components of date and time formats", then
      //    {era} gives no part in a calendar without eras (decision 6).
      if (partType === 'era' && calendarHasNoEras(calendar)) continue;
      //    i. Assert: format has a field [[<partType>]].
      //    ii. Let fieldFormat be format.[[<partType>]].
      fieldFormat = format[partType];
      //    iii. Let value be the value of localTime's field whose name is
      //         the Internal Slot column of the matching row.
      value = localTime[partType];
      //    iv. If partType is "year" and value ≤ 0, set value to 1 - value.
      if (partType === 'year' && value <= 0) value = 1 - value;
      //    v. If partType is "month", set value to value + 1.
      if (partType === 'month') value = value + 1;
      //    vi. If partType is "hour" and dateTimeFormat.[[HourCycle]] is
      //        "h11" or "h12", then
      if (
        partType === 'hour' &&
        (hourCycle === 'h11' || hourCycle === 'h12')
      ) {
        // 1. Set value to value modulo 12.
        value = value % 12;
        // 2. If value = 0 and dateTimeFormat.[[HourCycle]] is "h12", set
        //    value to 12.
        if (value === 0 && hourCycle === 'h12') value = 12;
      }
      //    vii. If partType is "hour" and dateTimeFormat.[[HourCycle]] is
      //         "h24", then
      //         1. If value = 0, set value to 24.
      if (partType === 'hour' && hourCycle === 'h24' && value === 0)
        value = 24;
      if (fieldFormat === 'numeric') {
        // viii. If fieldFormat is "numeric", then
        //       1. Let formattedValue be FormatNumeric(nf, value).
        formattedValue = formatDateTimeNumber(
          dateTimeFormat,
          data,
          'nf',
          undefined,
          value,
        );
      } else if (fieldFormat === '2-digit') {
        // ix. Else if fieldFormat is "2-digit", then
        //     1. Let formattedValue be FormatNumeric(nf2, value).
        //     2. Let codePoints be StringToCodePoints(formattedValue).
        //     3. Let count be the number of elements in codePoints.
        //     4. If count > 2, then
        //        a. Let tens be codePoints[count - 2].
        //        b. Let ones be codePoints[count - 1].
        //        c. Set formattedValue to CodePointsToString(« tens, ones »).
        formattedValue = lastTwoCodePoints(
          formatDateTimeNumber(dateTimeFormat, data, 'nf2', undefined, value),
        );
      } else {
        // x. Else if fieldFormat is "narrow", "short", or "long", then
        //    1. Let formattedValue be an ILD String representing value in
        //       the form given by fieldFormat: ICU4X's name of the instant
        //       (decision 4), in the month's or weekday's context.
        formattedValue = localTimeName(localTime, partType, format, calendar);
      }
      //    A numeric leap month takes the locale's numeric leap month
      //    pattern ("闰{0}", decision 5). Only chinese and dangi have one; a
      //    Hebrew M05L has a name, not a pattern.
      if (
        partType === 'month' &&
        localTime.leapMonth &&
        (fieldFormat === 'numeric' || fieldFormat === '2-digit') &&
        data.localeData.leapMonthPattern !== ''
      ) {
        formattedValue = substitutePlaceholders(
          data.localeData.leapMonthPattern,
          formattedValue,
          undefined,
        );
      }
      //    xi. Append the Record { [[Type]]: partType, [[Value]]:
      //        formattedValue } to result.
      appendDateTimePart(result, partType, formattedValue);
    } else if (partType === 'ampm') {
      // g. Else if partType is "ampm", then
      //    i. Let value be localTime.[[Hour]].
      //    ii. If value > 11, let formattedValue be an ILD String
      //        representing "post meridiem"; else let formattedValue be an
      //        ILD String representing "ante meridiem". Native code gave
      //        the name for the instant's hour.
      formattedValue = localTimeName(localTime, partType, format, calendar);
      //    iii. Append the Record { [[Type]]: "dayPeriod", [[Value]]:
      //         formattedValue } to result.
      appendDateTimePart(result, 'dayPeriod', formattedValue);
    } else if (partType === 'relatedYear') {
      // h. Else if partType is "relatedYear", then
      //    i. Let value be localTime.[[RelatedYear]].
      //    ii. Let formattedValue be FormatNumeric(nf, value).
      formattedValue = formatDateTimeNumber(
        dateTimeFormat,
        data,
        'nf',
        undefined,
        localTime.relatedYear,
      );
      //    iii. Append the Record { [[Type]]: "relatedYear", [[Value]]:
      //         formattedValue } to result.
      appendDateTimePart(result, 'relatedYear', formattedValue);
    } else if (partType === 'yearName') {
      // i. Else if partType is "yearName", then
      //    i. Let value be localTime.[[YearName]].
      //    ii. Let formattedValue be an ILD String representing value.
      formattedValue = localTimeName(localTime, partType, format, calendar);
      //    iii. Append the Record { [[Type]]: "yearName", [[Value]]:
      //         formattedValue } to result.
      appendDateTimePart(result, 'yearName', formattedValue);
    } else {
      // j. Else,
      //    i. Let unknown be an ILND String based on epochNanoseconds and
      //       partType: the placeholder's name.
      //    ii. Append the Record { [[Type]]: "unknown", [[Value]]: unknown }
      //        to result.
      appendDateTimePart(result, 'unknown', partType);
    }
  }
  // 16. Return result.
  return result;
}

// PartitionDateTimePattern ( dateTimeFormat, x )
function PartitionDateTimePattern(dateTimeFormat, x) {
  // 1. Set x to TimeClip(x).
  x = TimeClip(x);
  // 2. If x is NaN, throw a RangeError exception.
  if (x !== x) throw new _RangeError('Invalid time value');
  // 3. Let epochNanoseconds be ℤ(ℝ(x) × 10^6). The plugin keeps x, the
  //    same instant in milliseconds (see the file header).
  // 4. Let format be dateTimeFormat.[[DateTimeFormat]].
  var format = DateTimeFormatSlots.get(dateTimeFormat, 'dateTimeFormat');
  var hourCycle = DateTimeFormatSlots.get(dateTimeFormat, 'hourCycle');
  var pattern;
  if (hourCycle === 'h11' || hourCycle === 'h12') {
    // 5. If dateTimeFormat.[[HourCycle]] is "h11" or "h12", then
    //    a. Let pattern be format.[[pattern12]].
    pattern = format.pattern12;
  } else {
    // 6. Else,
    //    a. Let pattern be format.[[pattern]].
    pattern = format.pattern;
  }
  // 7. Let result be FormatDateTimePattern(dateTimeFormat, format, pattern,
  //    epochNanoseconds).
  // 8. Return result.
  return FormatDateTimePattern(dateTimeFormat, format, pattern, x);
}

// FormatDateTime ( dateTimeFormat, x )
function FormatDateTime(dateTimeFormat, x) {
  // 1. Let parts be ? PartitionDateTimePattern(dateTimeFormat, x).
  var parts = PartitionDateTimePattern(dateTimeFormat, x);
  // 2. Let result be the empty String.
  var result = '';
  // 3. For each Record { [[Type]], [[Value]] } part of parts, do
  //    a. Set result to the string-concatenation of result and
  //       part.[[Value]].
  for (var i = 0; i < parts.length; i++) result += parts[i].value;
  // 4. Return result.
  return result;
}

// FormatDateTimeToParts ( dateTimeFormat, x )
function FormatDateTimeToParts(dateTimeFormat, x) {
  // 1. Let parts be ? PartitionDateTimePattern(dateTimeFormat, x).
  var parts = PartitionDateTimePattern(dateTimeFormat, x);
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

/// The value PartitionDateTimeRangePattern compares for the range field
/// fieldName (step 15.d.ii-v) of localTime, a ToLocalTime Record of
/// dateTimeFormat, whose [[DateTimeFormat]] is format. Decision 3 refines
/// two of the draft's fields, whose values repeat where the dates differ:
/// [[Month]] is compared by month code, so a leap month and its base month
/// differ (chinese M04 and M04L are both month 4), and the [[Year]] of
/// chinese and dangi by [[RelatedYear]], because the cyclic year repeats
/// every 60 years.
function rangeFieldValue(dateTimeFormat, format, fieldName, localTime) {
  if (fieldName === 'AmPm') {
    // ii. If fieldName is [[AmPm]], then
    //     1-2. If localTime.[[Hour]] is less than 12, let v be "am"; else
    //          let v be "pm".
    return localTime.hour < 12 ? 'am' : 'pm';
  }
  if (fieldName === 'DayPeriod') {
    // iii. Else if fieldName is [[DayPeriod]], then
    //      1-2. Let v be an ILD String representing the day period of
    //           localTime: the day period id FormatDateTimePattern would
    //           show for it with format's fields (decision 4).
    var data = DateTimeFormatSlots.get(dateTimeFormat, 'data');
    return dayPeriodOf(
      data.dayPeriods,
      localTime.hour,
      localTime.minute,
      localTime.second,
      format.minute !== undefined,
      format.second !== undefined,
    );
  }
  if (fieldName === 'FractionalSecondDigits') {
    // iv. Else if fieldName is [[FractionalSecondDigits]], then
    //     1. If format has a field [[fractionalSecondDigits]], then
    //        a. Let fractionalSecondDigits be
    //           format.[[fractionalSecondDigits]].
    //     2. Else,
    //        a. Let fractionalSecondDigits be 3.
    var fractionalSecondDigits =
      format.fractionalSecondDigits !== undefined
        ? format.fractionalSecondDigits
        : 3;
    //     3. Let exp be fractionalSecondDigits - 3.
    //     4-5. Let v be floor(localTime.[[Millisecond]] × 10^exp). Dividing
    //          the integer by 10^(-exp) gives the same value without a
    //          rounded power of ten, as in FormatDateTimePattern.
    return MathFloor(
      localTime.millisecond /
        (fractionalSecondDigits === 1
          ? 100
          : fractionalSecondDigits === 2
          ? 10
          : 1),
    );
  }
  // v. Else,
  //    1-2. Let v be localTime's field whose name is fieldName, with
  //         decision 3's refinements of [[Month]] and [[Year]].
  if (fieldName === 'Era') return localTime.era;
  if (fieldName === 'Year') {
    var calendar = DateTimeFormatSlots.get(dateTimeFormat, 'calendar');
    return calendar === 'chinese' || calendar === 'dangi'
      ? localTime.relatedYear
      : localTime.year;
  }
  if (fieldName === 'Month') return localTime.monthCode;
  if (fieldName === 'Day') return localTime.day;
  if (fieldName === 'Hour') return localTime.hour;
  if (fieldName === 'Minute') return localTime.minute;
  // fieldName is [[Second]]; [[Default]] is never compared.
  return localTime.second;
}

// PartitionDateTimeRangePattern ( dateTimeFormat, x, y )
function PartitionDateTimeRangePattern(dateTimeFormat, x, y) {
  // 1. Set x to TimeClip(x).
  x = TimeClip(x);
  // 2. If x is NaN, throw a RangeError exception.
  if (x !== x) throw new _RangeError('Invalid time value');
  // 3. Set y to TimeClip(y).
  y = TimeClip(y);
  // 4. If y is NaN, throw a RangeError exception.
  if (y !== y) throw new _RangeError('Invalid time value');
  // 5. Let xEpochNanoseconds be ℤ(ℝ(x) × 10^6).
  // 6. Let yEpochNanoseconds be ℤ(ℝ(y) × 10^6).
  //    The plugin keeps x and y, the same instants in milliseconds (see
  //    the file header).
  // 7. Let localTime1 be ToLocalTime(xEpochNanoseconds,
  //    dateTimeFormat.[[Calendar]], dateTimeFormat.[[TimeZone]]).
  var localTime1 = ToLocalTime(dateTimeFormat, x);
  // 8. Let localTime2 be ToLocalTime(yEpochNanoseconds,
  //    dateTimeFormat.[[Calendar]], dateTimeFormat.[[TimeZone]]).
  var localTime2 = ToLocalTime(dateTimeFormat, y);
  // 9. Let format be dateTimeFormat.[[DateTimeFormat]].
  var format = DateTimeFormatSlots.get(dateTimeFormat, 'dateTimeFormat');
  var hourCycle = DateTimeFormatSlots.get(dateTimeFormat, 'hourCycle');
  var pattern;
  var rangePatterns;
  if (hourCycle === 'h11' || hourCycle === 'h12') {
    // 10. If dateTimeFormat.[[HourCycle]] is "h11" or "h12", then
    //     a. Let pattern be format.[[pattern12]].
    //     b. Let rangePatterns be format.[[rangePatterns12]].
    pattern = format.pattern12;
    rangePatterns = format.rangePatterns12;
  } else {
    // 11. Else,
    //     a. Let pattern be format.[[pattern]].
    //     b. Let rangePatterns be format.[[rangePatterns]].
    pattern = format.pattern;
    rangePatterns = format.rangePatterns;
  }
  // 12. Let selectedRangePattern be undefined.
  var selectedRangePattern = undefined;
  // 13. Let relevantFieldsEqual be true.
  var relevantFieldsEqual = true;
  // 14. Let checkMoreFields be true.
  var checkMoreFields = true;
  // 15. For each row of Table "DateTime Range Pattern Record", except the
  //     header row, in table order, do
  for (var i = 0; i < RANGE_FIELDS.length; i++) {
    // a. Let fieldName be the name given in the Field Name column of the
    //    row.
    var fieldName = RANGE_FIELDS[i];
    // b. If rangePatterns has a field whose name is fieldName, let
    //    rangePattern be rangePatterns' field whose name is fieldName; else
    //    let rangePattern be undefined. A field with no pattern is not a
    //    property of the null-prototype record (53-datetime-ranges.js).
    var rangePattern = rangePatterns[fieldName];
    // c. If selectedRangePattern is not undefined and rangePattern is
    //    undefined, then
    //    i. NOTE: Because there is no range pattern for differences at or
    //       below this field, no further checks will be performed.
    //    ii. Set checkMoreFields to false.
    if (selectedRangePattern !== undefined && rangePattern === undefined)
      checkMoreFields = false;
    // d. If fieldName is not equal to [[Default]] and relevantFieldsEqual
    //    is true and checkMoreFields is true, then
    if (fieldName !== 'Default' && relevantFieldsEqual && checkMoreFields) {
      // i. Set selectedRangePattern to rangePattern.
      selectedRangePattern = rangePattern;
      // ii-v. Let v1 and v2 be the values of localTime1 and localTime2
      //       compared for fieldName.
      var v1 = rangeFieldValue(dateTimeFormat, format, fieldName, localTime1);
      var v2 = rangeFieldValue(dateTimeFormat, format, fieldName, localTime2);
      // vi. If v1 is not equal to v2, then
      //     1. Set relevantFieldsEqual to false.
      if (v1 !== v2) relevantFieldsEqual = false;
    }
  }
  // 16. If relevantFieldsEqual is true, then
  if (relevantFieldsEqual) {
    // a. Let collapsedResult be a new empty List.
    var collapsedResult = [];
    // b. Let resultParts be FormatDateTimePattern(dateTimeFormat, format,
    //    pattern, xEpochNanoseconds).
    var resultParts = FormatDateTimePattern(
      dateTimeFormat,
      format,
      pattern,
      x,
      localTime1,
    );
    // c. For each Record { [[Type]], [[Value]] } part of resultParts, do
    //    i. Append the Record { [[Type]]: part.[[Type]], [[Value]]:
    //       part.[[Value]], [[Source]]: "shared" } to collapsedResult.
    for (var j = 0; j < resultParts.length; j++)
      appendDateTimeRangePart(collapsedResult, resultParts[j], 'shared');
    // d. Return collapsedResult.
    return collapsedResult;
  }
  // 17. Let rangeResult be a new empty List.
  var rangeResult = [];
  // 18. If selectedRangePattern is undefined, then
  //     a. Set selectedRangePattern to rangePatterns.[[Default]].
  if (selectedRangePattern === undefined)
    selectedRangePattern = rangePatterns.Default;
  // 19. For each Record { [[Pattern]], [[Source]] } rangePatternPart of
  //     selectedRangePattern.[[PatternParts]], do
  var patternParts = selectedRangePattern.PatternParts;
  for (var k = 0; k < patternParts.length; k++) {
    // a. Let pattern be rangePatternPart.[[Pattern]].
    var partPattern = patternParts[k].Pattern;
    // b. Let source be rangePatternPart.[[Source]].
    var source = patternParts[k].Source;
    var z;
    var localTime;
    if (source === 'startRange' || source === 'shared') {
      // c. If source is "startRange" or "shared", then
      //    i. Let z be xEpochNanoseconds.
      z = x;
      localTime = localTime1;
    } else {
      // d. Else,
      //    i. Let z be yEpochNanoseconds.
      z = y;
      localTime = localTime2;
    }
    // e. Let resultParts be FormatDateTimePattern(dateTimeFormat,
    //    selectedRangePattern, pattern, z).
    var partResults = FormatDateTimePattern(
      dateTimeFormat,
      selectedRangePattern,
      partPattern,
      z,
      localTime,
    );
    // f. For each Record { [[Type]], [[Value]] } part of resultParts, do
    //    i. Append the Record { [[Type]]: part.[[Type]], [[Value]]:
    //       part.[[Value]], [[Source]]: source } to rangeResult.
    for (var n = 0; n < partResults.length; n++)
      appendDateTimeRangePart(rangeResult, partResults[n], source);
  }
  // 20. Return rangeResult.
  return rangeResult;
}

/// Append the Record { [[Type]]: part.[[Type]], [[Value]]: part.[[Value]],
/// [[Source]]: source } to result.
function appendDateTimeRangePart(result, part, source) {
  // 1. Append the null-prototype record.
  listPush(result, {
    __proto__: null,
    type: part.type,
    value: part.value,
    source: source,
  });
}

// FormatDateTimeRange ( dateTimeFormat, x, y )
function FormatDateTimeRange(dateTimeFormat, x, y) {
  // 1. Let parts be ? PartitionDateTimeRangePattern(dateTimeFormat, x, y).
  var parts = PartitionDateTimeRangePattern(dateTimeFormat, x, y);
  // 2. Let result be the empty String.
  var result = '';
  // 3. For each Record { [[Type]], [[Value]], [[Source]] } part of parts, do
  //    a. Set result to the string-concatenation of result and
  //       part.[[Value]].
  for (var i = 0; i < parts.length; i++) result += parts[i].value;
  // 4. Return result.
  return result;
}

// FormatDateTimeRangeToParts ( dateTimeFormat, x, y )
function FormatDateTimeRangeToParts(dateTimeFormat, x, y) {
  // 1. Let parts be ? PartitionDateTimeRangePattern(dateTimeFormat, x, y).
  var parts = PartitionDateTimeRangePattern(dateTimeFormat, x, y);
  // 2. Let result be ! ArrayCreate(0).
  var result = [];
  // 3. Let n be 0.
  // 4. For each Record { [[Type]], [[Value]], [[Source]] } part of parts, do
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
