/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// ECMA-402 "DateTimeFormat Objects": the DateTime Format Records of a
// locale and calendar ([[LocaleData]].[[<locale>]].[[formats]] and
// [[styles]]), BasicFormatMatcher, the best-fit matcher,
// and DateTimeStyleFormat, following the draft https://tc39.es/ecma402/
// dated October 24, 2026. The CLDR patterns come from native code as lists
// of strings (plan decision 5); they become records here by decisions 6
// and 7 of the Phase 4 plan. The range patterns (decision 11) are in
// 53-datetime-ranges.js, and the names and day periods (decision 4) in
// 54-datetime-names.js.
//
// A DateTime Format Record is a null-prototype object with the draft's
// fields (a field it lacks reads as undefined), [[pattern]] and
// [[pattern12]] (the same string, decision 6), and these internal fields:
// monthStandalone and weekdayStandalone (the context of a text month or
// weekday), ampmWidth (the width of {ampm}: "abbreviated", "wide" or
// "narrow"), dayPeriodWidth (the width of {dayPeriod}, which a pattern may
// show without a [[dayPeriod]] field), hourLetter (the CLDR letter of the
// hour), skeletonWidths (the fields and widths of the CLDR skeleton an
// availableFormats record is for, or of its parts for a glued record,
// which best fit scores; undefined when they are the record's own),
// patternLetterCounts (a null-prototype object from each field to the
// number of CLDR letters its pattern writes it with, as best fit adjusted
// them, which the range patterns' interval widths compare; undefined for
// a record glued by glueFormatRecords, whose parts have their own; a
// date and time style record from DateTimeStyleFormat has its time
// part's, which addFieldsExcept copies with the time part's other fields,
// and addTimeRangeFields compares), and, for a record glued from a date
// and a time record (by best fit or DateTimeStyleFormat), datePart and
// timePart; withDayPeriod is, for a best-fit record whose unrequested B
// shows as {ampm} (showDayPeriodAsAmPm), the record as it was with the B
// and with the fields best fit appended to it (appendMissingFields), and,
// for a glued record, its time part's withDayPeriod (glueFormatRecords).
// rangePatternsOf (53-datetime-ranges.js) adds [[rangePatterns]] and
// [[rangePatterns12]] on first use.

/// The Property column of the draft's table "Components of date and time
/// formats", in table order. 0-4 are the date fields, 5-10 the time fields
/// (decision 6).
var DATE_TIME_COMPONENTS = [
  'weekday',
  'era',
  'year',
  'month',
  'day',
  'dayPeriod',
  'hour',
  'minute',
  'second',
  'fractionalSecondDigits',
  'timeZoneName',
];
var FIRST_TIME_COMPONENT = 5;

/// The indices of a dateTimePatterns list (intl/jsi/NativeHelpers.def).
/// Each style group has four entries, full to short.
var DTP_DATE_FORMATS = 0;
var DTP_TIME_FORMATS = 4;
var DTP_DATE_TIME_FORMATS = 8;
var DTP_AT_TIME_FORMATS = 12;
var DTP_INTERVAL_FALLBACK = 16;
var DTP_LEAP_MONTH = 17;
var DTP_TIME_ZONE_APPEND = 18;
var DTP_AVAILABLE = 19;

/// The date and time styles, in the order of the dateTimePatterns groups.
var DATE_TIME_STYLES = ['full', 'long', 'medium', 'short'];

/// The CLDR pattern of the one-field record decision 6 adds for a field
/// that has no one-field availableFormats entry. hour's depends on the
/// family; fractionalSecondDigits gets one of each width, and so does
/// timeZoneName, a refinement of decision 6: a timeZoneName of another
/// kind is not adjusted to the width asked for, so {timeZoneName:
/// "shortOffset"} would otherwise give "z".
var ONE_FIELD_PATTERNS = {
  __proto__: null,
  weekday: 'ccc',
  era: 'G',
  year: 'y',
  month: 'L',
  day: 'd',
  dayPeriod: 'B',
  minute: 'm',
  second: 's',
};
var FRACTION_PATTERNS = ['S', 'SS', 'SSS'];
var TIME_ZONE_NAME_PATTERNS = ['z', 'zzzz', 'O', 'OOOO', 'v', 'vvvv'];
/// The timeZoneName widths, in the order of TIME_ZONE_NAME_PATTERNS.
var TIME_ZONE_NAME_WIDTHS = [
  'short',
  'long',
  'shortOffset',
  'longOffset',
  'shortGeneric',
  'longGeneric',
];

/// The number of CLDR letters a skeleton for each requested width has, as
/// ICU's generator compares them with a pattern's skeleton.
var WIDTH_LETTER_COUNTS = {
  __proto__: null,
  numeric: 1,
  '2-digit': 2,
  short: 3,
  long: 4,
  narrow: 5,
};

/// The values BasicFormatMatcher's step 10.b.vii.2.a indexes.
var FORMAT_WIDTH_VALUES = ['2-digit', 'numeric', 'narrow', 'short', 'long'];

/// Whether calendar has cyclic year names, the only calendars with the U
/// symbol (decision 4), which write y as the related ISO year (decision
/// 6).
function calendarIsCyclic(calendar) {
  // 1. Return true for chinese and dangi.
  return calendar === 'chinese' || calendar === 'dangi';
}

/// Whether pattern, a Pattern String, has the placeholder {name}.
function hasPlaceholder(pattern, name) {
  // 1. Return whether "{" + name + "}" is a substring of pattern.
  return stringIndexOf(pattern, '{' + name + '}', 0) !== -1;
}

/// pattern with its first "{0}" replaced by v0 and its first "{1}" by v1
/// (v1 is undefined when pattern has no "{1}"). Both are replaced in one
/// pass, so the text of v0 and v1 is never searched.
function substitutePlaceholders(pattern, v0, v1) {
  // 1. Let i0 and i1 be the indices of "{0}" and "{1}" in pattern.
  var i0 = stringIndexOf(pattern, '{0}', 0);
  var i1 = v1 === undefined ? -1 : stringIndexOf(pattern, '{1}', 0);
  // 2. If pattern has only one of them, replace it.
  if (i1 === -1) {
    if (i0 === -1) return pattern;
    return stringSlice(pattern, 0, i0) + v0 + stringSlice(pattern, i0 + 3);
  }
  if (i0 === -1)
    return stringSlice(pattern, 0, i1) + v1 + stringSlice(pattern, i1 + 3);
  // 3. Otherwise replace both, in the order they appear.
  var first = i0 < i1 ? i0 : i1;
  var second = i0 < i1 ? i1 : i0;
  return (
    stringSlice(pattern, 0, first) +
    (first === i0 ? v0 : v1) +
    stringSlice(pattern, first + 3, second) +
    (second === i0 ? v0 : v1) +
    stringSlice(pattern, second + 3)
  );
}

// ---------------------------------------------------------------------------
// CLDR patterns (decision 6).

/// The tokens of a CLDR date pattern (UTS 35 Part 4, "Date Format
/// Patterns"), read by code unit, with no regular expression: a run of one
/// ASCII letter outside quotes is a field, { [[Letter]], [[Count]] }; all
/// other text is literal, { [[Text]] }, without its quotes. Two quotes are
/// a quote, inside or outside quoted text.
function cldrPatternTokens(pattern) {
  // 1. Let tokens be a new empty List, literal the empty String and quoted
  //    false.
  var tokens = [];
  var literal = '';
  var quoted = false;
  var i = 0;
  // 2. Repeat, while i < the length of pattern,
  while (i < pattern.length) {
    var c = charCodeAt(pattern, i);
    // a. Two quotes are a quote; one starts or ends quoted text.
    if (c === 0x27) {
      if (i + 1 < pattern.length && charCodeAt(pattern, i + 1) === 0x27) {
        literal += "'";
        i += 2;
      } else {
        quoted = !quoted;
        i++;
      }
      continue;
    }
    // b. A run of one ASCII letter outside quotes is a field.
    if (!quoted && ((c >= 0x41 && c <= 0x5a) || (c >= 0x61 && c <= 0x7a))) {
      var j = i + 1;
      while (j < pattern.length && charCodeAt(pattern, j) === c) j++;
      if (literal !== '') {
        listPush(tokens, {__proto__: null, letter: undefined, text: literal});
        literal = '';
      }
      listPush(tokens, {
        __proto__: null,
        letter: StringFromCharCode(c),
        count: j - i,
      });
      i = j;
      continue;
    }
    // c. Any other code unit is literal text.
    literal += stringSlice(pattern, i, i + 1);
    i++;
  }
  // 3. Append the remaining literal text, and return tokens.
  if (literal !== '')
    listPush(tokens, {__proto__: null, letter: undefined, text: literal});
  return tokens;
}

/// The text of a CLDR pattern that has no fields but {0} and {1}
/// (dateTimeFormats, intervalFormatFallback, the Timezone append item, the
/// numeric leap month pattern): quotes are removed as cldrPatternTokens
/// removes them, and every other code unit, letters included, is kept (fr
/// "du {0} au {1}").
function cldrLiteralPattern(pattern) {
  // 1. Let result be the empty String.
  var result = '';
  var i = 0;
  // 2. For each code unit of pattern, drop a quote and keep two as one;
  //    keep any other code unit.
  while (i < pattern.length) {
    if (charCodeAt(pattern, i) === 0x27) {
      if (i + 1 < pattern.length && charCodeAt(pattern, i + 1) === 0x27) {
        result += "'";
        i++;
      }
    } else {
      result += stringSlice(pattern, i, i + 1);
    }
    i++;
  }
  // 3. Return result.
  return result;
}

/// The width of count letters of a text field: 1-3 short, 4 long, 5
/// narrow (decision 6's table); undefined for any other count.
function textWidthOf(count, short, long, narrow) {
  // 1. Return the width for count.
  if (count >= 1 && count <= 3) return short;
  if (count === 4) return long;
  if (count === 5) return narrow;
  return undefined;
}

/// Decision 6's table: for count letters letter of a CLDR pattern in
/// calendar, the Record { [[Name]]: the placeholder, [[Field]]: the
/// draft's field (undefined for {ampm}), [[Width]], [[Standalone]] }. Any
/// other letter or count is an internal error: the generator leaves none
/// in the data (decision 5).
function cldrFieldOf(letter, count, calendar) {
  // 1. Let name, field and width be undefined, and standalone false.
  var name;
  var field;
  var width;
  var standalone = false;
  // 2. Look letter and count up in the table.
  if (letter === 'G') {
    name = field = 'era';
    width = textWidthOf(count, 'short', 'long', 'narrow');
  } else if (letter === 'y') {
    // In chinese and dangi, y is the related ISO year.
    name = calendarIsCyclic(calendar) ? 'relatedYear' : 'year';
    field = 'year';
    width = count === 2 ? '2-digit' : 'numeric';
  } else if (letter === 'U' || letter === 'r') {
    name = letter === 'U' ? 'yearName' : 'relatedYear';
    field = 'year';
    width = 'numeric';
  } else if (letter === 'M' || letter === 'L') {
    name = field = 'month';
    width =
      count === 1
        ? 'numeric'
        : count === 2
        ? '2-digit'
        : textWidthOf(count, 'short', 'long', 'narrow');
    standalone = letter === 'L';
  } else if (letter === 'E' || letter === 'c') {
    // E (1-3, 6), EEEE, EEEEE; c (3, 6), cccc, ccccc.
    name = field = 'weekday';
    if (count === 6) width = 'short';
    else if (letter === 'E' || count >= 3)
      width = textWidthOf(count, 'short', 'long', 'narrow');
    standalone = letter === 'c';
  } else if (letter === 'a' || letter === 'b') {
    name = 'ampm';
    width = textWidthOf(count, 'abbreviated', 'wide', 'narrow');
  } else if (letter === 'B') {
    name = field = 'dayPeriod';
    width = textWidthOf(count, 'short', 'long', 'narrow');
  } else if (
    letter === 'd' ||
    letter === 'h' ||
    letter === 'H' ||
    letter === 'K' ||
    letter === 'k' ||
    letter === 'm' ||
    letter === 's'
  ) {
    name = field =
      letter === 'd'
        ? 'day'
        : letter === 'm'
        ? 'minute'
        : letter === 's'
        ? 'second'
        : 'hour';
    width = count === 1 ? 'numeric' : count === 2 ? '2-digit' : undefined;
  } else if (letter === 'S') {
    name = field = 'fractionalSecondDigits';
    width = MathMin(count, 3);
  } else if (letter === 'z') {
    name = field = 'timeZoneName';
    width = count <= 3 ? 'short' : count === 4 ? 'long' : undefined;
  } else if (letter === 'O' || letter === 'v') {
    name = field = 'timeZoneName';
    var generic = letter === 'v';
    if (count === 1) width = generic ? 'shortGeneric' : 'shortOffset';
    else if (count === 4) width = generic ? 'longGeneric' : 'longOffset';
  }
  // 3. If the table has no entry, it is an internal error.
  if (width === undefined) {
    throw new _TypeError(
      'Intl internal error: bad date pattern field ' + letter + count,
    );
  }
  // 4. Return the Record.
  return {
    __proto__: null,
    name: name,
    field: field,
    width: width,
    standalone: standalone,
  };
}

/// A new DateTime Format Record with no fields and an empty pattern.
function newFormatRecord() {
  // 1. Return the record, with every internal field (see the file header)
  //    at its empty value.
  return {
    __proto__: null,
    pattern: '',
    pattern12: '',
    monthStandalone: false,
    weekdayStandalone: false,
    ampmWidth: undefined,
    dayPeriodWidth: undefined,
    hourLetter: undefined,
    skeletonWidths: undefined,
    patternLetterCounts: undefined,
    withDayPeriod: undefined,
    datePart: undefined,
    timePart: undefined,
  };
}

/// The fields and widths best fit scores record by: its skeleton's, or its
/// own when it has no skeleton (a refinement of decision 6: ICU matches on
/// the skeleton, so zh's yMMMMd "y年M月d日" is the record for a long month).
function scoringFieldsOf(record) {
  // 1. Return the skeleton's widths if record has them, else record.
  return record.skeletonWidths !== undefined ? record.skeletonWidths : record;
}

/// A copy of record, without its range patterns, which belong to its
/// widths, and without its [[dayPeriod]] field when keepDayPeriod is
/// false: a B the request did not ask for formats as the day period but
/// adds no field (decision 6).
function copyFormatRecord(record, keepDayPeriod = true) {
  // 1. Let result be a new record with record's fields.
  var result = newFormatRecord();
  for (var i = 0; i < DATE_TIME_COMPONENTS.length; i++) {
    var name = DATE_TIME_COMPONENTS[i];
    if (record[name] !== undefined && (keepDayPeriod || name !== 'dayPeriod'))
      result[name] = record[name];
  }
  // 2. Copy the patterns and the internal fields, and return result.
  result.pattern = record.pattern;
  result.pattern12 = record.pattern12;
  result.monthStandalone = record.monthStandalone;
  result.weekdayStandalone = record.weekdayStandalone;
  result.ampmWidth = record.ampmWidth;
  result.dayPeriodWidth = record.dayPeriodWidth;
  result.hourLetter = record.hourLetter;
  result.skeletonWidths = record.skeletonWidths;
  result.patternLetterCounts = record.patternLetterCounts;
  result.withDayPeriod = record.withDayPeriod;
  result.datePart = record.datePart;
  result.timePart = record.timePart;
  return result;
}

/// Decision 6: the DateTime Format Record of a CLDR pattern in calendar.
/// skeleton is the availableFormats skeleton the pattern is for: a B in
/// the pattern gives a [[dayPeriod]] field only if skeleton has one (or is
/// undefined), so zh-TW's "Bh時" for h shows the day period without adding
/// the field. A style pattern passes "", since nothing asked for its B. A
/// non-empty skeleton's fields and widths are kept as skeletonWidths, which
/// best fit scores by (a refinement of decision 6; yyyy is a numeric year,
/// U a numeric year).
function cldrPatternToRecord(
  pattern,
  calendar = undefined,
  skeleton = undefined,
) {
  // 1. Let record be a new DateTime Format Record.
  var record = newFormatRecord();
  // 2. Let dayPeriodAsked be whether skeleton is undefined or has a B.
  var dayPeriodAsked =
    skeleton === undefined || stringIndexOf(skeleton, 'B', 0) !== -1;
  // 3. For each token of pattern, append its placeholder or its literal
  //    text, and give record the field, width and letter count of a
  //    placeholder. A field shown twice keeps its first width.
  var tokens = cldrPatternTokens(pattern);
  var result = '';
  var counts = {__proto__: null};
  for (var i = 0; i < tokens.length; i++) {
    var token = tokens[i];
    if (token.letter === undefined) {
      result += token.text;
      continue;
    }
    var f = cldrFieldOf(token.letter, token.count, calendar);
    result += '{' + f.name + '}';
    if (f.name === 'ampm') {
      if (record.ampmWidth === undefined) record.ampmWidth = f.width;
      continue;
    }
    if (f.name === 'dayPeriod') {
      if (record.dayPeriodWidth === undefined) record.dayPeriodWidth = f.width;
      if (!dayPeriodAsked) continue;
    }
    if (record[f.field] !== undefined) continue;
    record[f.field] = f.width;
    counts[f.field] = token.count;
    if (f.field === 'hour') record.hourLetter = token.letter;
    else if (f.field === 'month') record.monthStandalone = f.standalone;
    else if (f.field === 'weekday') record.weekdayStandalone = f.standalone;
  }
  // 4. Set record.[[pattern]] and record.[[pattern12]] to the result, and
  //    keep the letter counts.
  record.pattern = result;
  record.pattern12 = result;
  record.patternLetterCounts = counts;
  // 5. Keep the fields and widths of a non-empty skeleton, the skeleton,
  //    and the number of letters of each field as letterCounts.
  if (skeleton !== undefined && skeleton !== '') {
    var widths = requestOf(cldrPatternToRecord(skeleton, calendar));
    widths.skeleton = skeleton;
    widths.letterCounts = {__proto__: null};
    var skeletonTokens = cldrPatternTokens(skeleton);
    for (var j = 0; j < skeletonTokens.length; j++) {
      var skeletonToken = skeletonTokens[j];
      if (skeletonToken.letter === undefined) continue;
      var skeletonField = cldrFieldOf(
        skeletonToken.letter,
        skeletonToken.count,
        calendar,
      ).field;
      if (skeletonField !== undefined)
        widths.letterCounts[skeletonField] = skeletonToken.count;
    }
    record.skeletonWidths = widths;
  }
  // 6. Return record.
  return record;
}

/// Whether record (a DateTime Format Record or a request) has a field of
/// DATE_TIME_COMPONENTS from index from to index to.
function hasFieldIn(record, from, to) {
  // 1. Return true if any of those fields is present.
  for (var i = from; i < to; i++) {
    if (record[DATE_TIME_COMPONENTS[i]] !== undefined) return true;
  }
  return false;
}

/// Whether record has a date field (weekday, era, year, month, day).
function hasDateField(record) {
  // 1. Look at the date fields.
  return hasFieldIn(record, 0, FIRST_TIME_COMPONENT);
}

/// Whether record has a time field (dayPeriod, hour, minute, second,
/// fractionalSecondDigits, timeZoneName).
function hasTimeField(record) {
  // 1. Look at the time fields.
  return hasFieldIn(
    record,
    FIRST_TIME_COMPONENT,
    DATE_TIME_COMPONENTS.length,
  );
}

/// A request (the draft's formatOptions) with the fields of record from
/// index from to index to of DATE_TIME_COMPONENTS, at record's widths.
function requestOf(record, from = 0, to = DATE_TIME_COMPONENTS.length) {
  // 1. Let request be a new Record; copy each present field.
  var request = {__proto__: null};
  for (var i = from; i < to; i++) {
    var name = DATE_TIME_COMPONENTS[i];
    if (record[name] !== undefined) request[name] = record[name];
  }
  // 2. Return request.
  return request;
}

/// The hour-cycle family of an hour letter: "h12" for h and K, "h24" for H
/// and k, undefined for none.
function hourLetterFamily(letter) {
  // 1. Return the family of letter.
  if (letter === 'h' || letter === 'K') return 'h12';
  if (letter === 'H' || letter === 'k') return 'h24';
  return undefined;
}

/// Decision 6's hour-cycle families: "h12" for a record with an hour whose
/// letter is h or K or whose pattern shows a day period, "h24" for H or k,
/// undefined (both families) for a record without an hour.
function hourFamilyOf(record) {
  // 1. A record without an hour is in both families.
  if (record.hour === undefined) return undefined;
  // 2. A pattern with B is 12-hour; otherwise the hour letter decides.
  if (record.dayPeriodWidth !== undefined) return 'h12';
  return hourLetterFamily(record.hourLetter);
}

// ---------------------------------------------------------------------------
// [[LocaleData]] of a locale and calendar (decisions 6, 7 and 13).

/// Decision 6: the format records, styles and patterns of a locale and
/// calendar, from patterns, a dateTimePatterns list, and decimalSeparator,
/// the decimal separator of [[NumberingSystem]] a fraction of a second is
/// written with. The result is { [[Calendar]], [[DecimalSeparator]],
/// [[Available]]: for "h12" and "h24", the records best fit scores (the
/// availableFormats and one-field records), [[Formats]]: for each family,
/// its [[formats]], made by formatsOf on first use (only tests need the
/// whole List), [[Unglued]]: for each family, the records of [[formats]]
/// that are not glued (ungluedFormatsOf, on first use), [[Glued]]: the
/// glued records made so far (gluedFormatRecord), [[Styles]]: the DateTime
/// Styles Record of each family (decision 7), [[DateTimeFormats]] and
/// [[AtTimeFormats]]: the four standard and atTime dateTimeFormats,
/// [[IntervalFallback]], [[LeapMonthPattern]], [[TimeZoneAppend]] (the
/// Timezone append item), [[Intervals]]: the intervalFormats entries, and
/// [[Matches]]: the cache of cachedFormatMatch }.
function dateTimeLocaleData(patterns, calendar, decimalSeparator) {
  // 1. Let data be the Record, with the patterns that have no fields.
  var data = {
    __proto__: null,
    calendar: calendar,
    decimalSeparator: decimalSeparator,
    available: {__proto__: null, h12: [], h24: []},
    formats: {__proto__: null, h12: undefined, h24: undefined},
    unglued: {__proto__: null, h12: undefined, h24: undefined},
    glued: {__proto__: null},
    styles: {__proto__: null, h12: undefined, h24: undefined},
    dateTimeFormats: [],
    atTimeFormats: [],
    intervalFallback: cldrLiteralPattern(patterns[DTP_INTERVAL_FALLBACK]),
    leapMonthPattern: cldrLiteralPattern(patterns[DTP_LEAP_MONTH]),
    timeZoneAppend: cldrLiteralPattern(patterns[DTP_TIME_ZONE_APPEND]),
    intervals: [],
    matches: newDataCache(FORMAT_MATCH_CACHE_SIZE),
  };
  for (var i = 0; i < 4; i++) {
    listPush(
      data.dateTimeFormats,
      cldrLiteralPattern(patterns[DTP_DATE_TIME_FORMATS + i]),
    );
    listPush(
      data.atTimeFormats,
      cldrLiteralPattern(patterns[DTP_AT_TIME_FORMATS + i]),
    );
  }
  // 2. For each availableFormats entry, in CLDR's order, add its record to
  //    the families it belongs to.
  var count = +patterns[DTP_AVAILABLE];
  var k = DTP_AVAILABLE + 1;
  for (var n = 0; n < count; n++, k += 2) {
    var record = cldrPatternToRecord(patterns[k + 1], calendar, patterns[k]);
    var family = hourFamilyOf(record);
    if (family !== 'h24') listPush(data.available.h12, record);
    if (family !== 'h12') listPush(data.available.h24, record);
  }
  // 3. For each intervalFormats entry, add { [[Skeleton]], [[Fields]]: the
  //    record of its skeleton, [[Letter]], [[Pattern]] }.
  count = +patterns[k];
  k++;
  for (var m = 0; m < count; m++, k += 3) {
    listPush(data.intervals, {
      __proto__: null,
      skeleton: patterns[k],
      fields: cldrPatternToRecord(patterns[k], calendar),
      letter: patterns[k + 1],
      pattern: patterns[k + 2],
    });
  }
  // 4. Add the one-field records to each family.
  addOneFieldRecords(data.available.h12, 'h12', calendar);
  addOneFieldRecords(data.available.h24, 'h24', calendar);
  // 5. Make the styles of each family, for which best fit needs the
  //    records.
  data.styles.h12 = dateTimeStyles(data, patterns, 'h12');
  data.styles.h24 = dateTimeStyles(data, patterns, 'h24');
  // 6. Return data.
  return data;
}

/// Whether list has a record whose only field is name, of width width
/// when width is not undefined. The skeleton's fields count, not the
/// pattern's (a refinement of decision 6): en japanese's y "y G" is the
/// one-field record for the year, so no {year} is added.
function hasOneFieldRecord(list, name, width) {
  // 1. Look for a record with name and no other field.
  for (var i = 0; i < list.length; i++) {
    var r = scoringFieldsOf(list[i]);
    if (r[name] === undefined || (width !== undefined && r[name] !== width))
      continue;
    var only = true;
    for (var j = 0; j < DATE_TIME_COMPONENTS.length && only; j++) {
      var other = DATE_TIME_COMPONENTS[j];
      only = other === name || r[other] === undefined;
    }
    if (only) return true;
  }
  return false;
}

/// Decision 6: add to list, the records of family, a one-field record for
/// each field that has no one-field entry ({minute} numeric, {second},
/// {dayPeriod}, {era}...), and {fractionalSecondDigits} of each width 1, 2
/// and 3, in the draft's table order; and, a refinement of decision 6,
/// {timeZoneName} of each of its six widths (TIME_ZONE_NAME_PATTERNS).
function addOneFieldRecords(list, family, calendar) {
  // 1. For each field, in table order, add the records it lacks.
  for (var i = 0; i < DATE_TIME_COMPONENTS.length; i++) {
    var name = DATE_TIME_COMPONENTS[i];
    if (name === 'fractionalSecondDigits' || name === 'timeZoneName') {
      var widths =
        name === 'timeZoneName' ? TIME_ZONE_NAME_PATTERNS : FRACTION_PATTERNS;
      for (var w = 0; w < widths.length; w++) {
        var record = cldrPatternToRecord(widths[w]);
        if (!hasOneFieldRecord(list, name, record[name]))
          listPush(list, record);
      }
    } else if (!hasOneFieldRecord(list, name, undefined)) {
      var pattern = ONE_FIELD_PATTERNS[name];
      if (name === 'hour') pattern = family === 'h12' ? 'h a' : 'HH';
      listPush(list, cldrPatternToRecord(pattern, calendar));
    }
  }
}

/// Decision 6: record with {fractionalSecondDigits} of width digits right
/// after its {second}, preceded by decimalSeparator. record is a copy the
/// caller owns; it is changed and returned.
function insertFraction(record, digits, decimalSeparator) {
  // 1. Replace the first "{second}" with itself, the separator and
  //    "{fractionalSecondDigits}".
  var i = stringIndexOf(record.pattern, '{second}', 0);
  if (i === -1) return record;
  record.pattern =
    stringSlice(record.pattern, 0, i + 8) +
    decimalSeparator +
    '{fractionalSecondDigits}' +
    stringSlice(record.pattern, i + 8);
  record.pattern12 = record.pattern;
  // 2. Set the field, and return record. Its fields are now its own.
  record.fractionalSecondDigits = digits;
  record.skeletonWidths = undefined;
  return record;
}

/// UTS 35 Part 4, "Missing Skeleton Fields" (decision 6): the index (0
/// full to 3 short) of the dateTimeFormat that joins dateRecord to a time:
/// full for a long month and a weekday, long for a long month, medium for
/// a short month, else short.
function connectorStyleIndex(dateRecord) {
  // 1. Return the index by the month's width and the weekday.
  if (dateRecord.month === 'long')
    return dateRecord.weekday !== undefined ? 0 : 1;
  return dateRecord.month === 'short' ? 2 : 3;
}

/// Decision 6's gluing: the record whose pattern is connector with "{1}"
/// replaced by dateRecord's pattern and "{0}" by timeRecord's, with the
/// date fields and contexts of dateRecord and the time fields of
/// timeRecord. It keeps both parts, for its range patterns (decision 11).
function glueFormatRecords(dateRecord, timeRecord, connector) {
  // 1. Let record be a new record with the fields of both.
  var record = newFormatRecord();
  for (var i = 0; i < DATE_TIME_COMPONENTS.length; i++) {
    var name = DATE_TIME_COMPONENTS[i];
    var from = i < FIRST_TIME_COMPONENT ? dateRecord : timeRecord;
    if (from[name] !== undefined) record[name] = from[name];
  }
  // 2. Take the contexts from the date, and the AM/PM, day period and hour
  //    letter from the time.
  record.monthStandalone = dateRecord.monthStandalone;
  record.weekdayStandalone = dateRecord.weekdayStandalone;
  record.ampmWidth = timeRecord.ampmWidth;
  record.dayPeriodWidth = timeRecord.dayPeriodWidth;
  record.hourLetter = timeRecord.hourLetter;
  record.withDayPeriod = timeRecord.withDayPeriod;
  // 3. Combine the parts' skeleton widths, if either has them.
  if (
    dateRecord.skeletonWidths !== undefined ||
    timeRecord.skeletonWidths !== undefined
  ) {
    var widths = requestOf(
      scoringFieldsOf(dateRecord),
      0,
      FIRST_TIME_COMPONENT,
    );
    var timeWidths = scoringFieldsOf(timeRecord);
    for (var j = FIRST_TIME_COMPONENT; j < DATE_TIME_COMPONENTS.length; j++) {
      var timeName = DATE_TIME_COMPONENTS[j];
      if (timeWidths[timeName] !== undefined)
        widths[timeName] = timeWidths[timeName];
    }
    record.skeletonWidths = widths;
  }
  // 4. Join the patterns, keep the parts, and return record.
  record.pattern = substitutePlaceholders(
    connector,
    timeRecord.pattern,
    dateRecord.pattern,
  );
  record.pattern12 = record.pattern;
  record.datePart = dateRecord;
  record.timePart = timeRecord;
  return record;
}

/// Decision 6: the records of [[formats]] of data and family that are not
/// glued: the availableFormats and one-field records; for every record
/// with {second} and no fraction, three more with {fractionalSecondDigits}
/// of width 1, 2 and 3; and for every time record without a timeZoneName,
/// one more for each timeZoneName width, appended by the Timezone append
/// item as decision 6's step 4 does (a refinement of decision 6, so that
/// formatMatcher "basic" keeps the hour of {hour, timeZoneName} in family
/// "h24"). The result is { [[List]]: those records, in that order,
/// [[Dates]] and [[Times]]: the records of the List with only date fields
/// and with only time fields, in its order }, the date and time records
/// formatsOf glues. Made on first use and kept in data.
function ungluedFormatsOf(data, family) {
  // 1. If data has the record, return it.
  var unglued = data.unglued[family];
  if (unglued !== undefined) return unglued;
  // 2. Let formats be the availableFormats and one-field records.
  var formats = [];
  var available = data.available[family];
  for (var i = 0; i < available.length; i++) listPush(formats, available[i]);
  // 3. Add the records with a fraction of a second.
  var count = formats.length;
  for (var j = 0; j < count; j++) {
    var r = formats[j];
    if (r.second === undefined || r.fractionalSecondDigits !== undefined)
      continue;
    for (var w = 1; w <= 3; w++) {
      listPush(
        formats,
        insertFraction(copyFormatRecord(r), w, data.decimalSeparator),
      );
    }
  }
  // 4. Add the time records with a time zone name.
  count = formats.length;
  for (var z = 0; z < count; z++) {
    var timeOnly = formats[z];
    if (
      hasDateField(timeOnly) ||
      !hasTimeField(timeOnly) ||
      timeOnly.timeZoneName !== undefined
    )
      continue;
    for (var n = 0; n < TIME_ZONE_NAME_WIDTHS.length; n++) {
      listPush(
        formats,
        appendTimeZoneName(
          copyFormatRecord(timeOnly),
          data,
          TIME_ZONE_NAME_WIDTHS[n],
        ),
      );
    }
  }
  // 5. Find the date and the time records.
  var dates = [];
  var times = [];
  count = formats.length;
  for (var k = 0; k < count; k++) {
    var date = hasDateField(formats[k]);
    var time = hasTimeField(formats[k]);
    if (date && !time) listPush(dates, formats[k]);
    else if (time && !date) listPush(times, formats[k]);
  }
  // 6. Keep the record in data, and return it.
  unglued = {__proto__: null, list: formats, dates: dates, times: times};
  data.unglued[family] = unglued;
  return unglued;
}

/// Decision 6: [[formats]] of data and family: the records of
/// ungluedFormatsOf, then every date record glued to every time record,
/// date by date. Made on first use and kept in data. Only tests use the
/// whole List: CreateDateTimeFormat matches with basicFormatMatch, which
/// gives the same record without gluing every pair.
function formatsOf(data, family) {
  // 1. If data has the List, return it.
  var formats = data.formats[family];
  if (formats !== undefined) return formats;
  // 2. Let formats be the records that are not glued.
  var unglued = ungluedFormatsOf(data, family);
  formats = [];
  for (var i = 0; i < unglued.list.length; i++)
    listPush(formats, unglued.list[i]);
  // 3. Glue every date record to every time record.
  for (var d = 0; d < unglued.dates.length; d++) {
    for (var t = 0; t < unglued.times.length; t++)
      listPush(formats, gluedFormatRecord(data, family, d, t));
  }
  // 4. Keep formats in data, and return it.
  data.formats[family] = formats;
  return formats;
}

/// The record of [[formats]] of data and family that glues date record d to
/// time record t of ungluedFormatsOf (decision 6's gluing, with the
/// connector of the date's month and weekday). Made on first use and kept
/// in data, so formatsOf and basicFormatMatch give the same object, whose
/// range patterns are kept on it.
function gluedFormatRecord(data, family, d, t) {
  // 1. If data has the record, return it.
  var key = family + ';' + d + ';' + t;
  var record = data.glued[key];
  if (record !== undefined) return record;
  // 2. Otherwise glue the two records, keep the result, and return it.
  var unglued = ungluedFormatsOf(data, family);
  var date = unglued.dates[d];
  record = glueFormatRecords(
    date,
    unglued.times[t],
    data.atTimeFormats[connectorStyleIndex(date)],
  );
  data.glued[key] = record;
  return record;
}

/// Decision 7: the DateTime Styles Record of data's family: [[Date]] and
/// [[Time]] from dateFormats and timeFormats, [[Connector]] from the
/// atTime dateTimeFormats, and [[DateTimeRangeFormat]], whose records
/// DateTimeStyleFormat makes on first use. A time style whose hour is of
/// the other family is replaced by the best-fit record for its fields and
/// widths in family. The Record also keeps data and family, for the range
/// patterns.
function dateTimeStyles(data, patterns, family) {
  // 1. Let styles be the Record.
  var styles = {
    __proto__: null,
    Date: {__proto__: null},
    Time: {__proto__: null},
    Connector: {__proto__: null},
    DateTimeRangeFormat: {__proto__: null},
    localeData: data,
    family: family,
  };
  // 2. For each style, make its records.
  for (var i = 0; i < DATE_TIME_STYLES.length; i++) {
    var style = DATE_TIME_STYLES[i];
    styles.Date[style] = cldrPatternToRecord(
      patterns[DTP_DATE_FORMATS + i],
      data.calendar,
      '',
    );
    var time = cldrPatternToRecord(
      patterns[DTP_TIME_FORMATS + i],
      data.calendar,
      '',
    );
    var timeFamily = hourFamilyOf(time);
    if (timeFamily !== undefined && timeFamily !== family)
      time = BestFitFormatMatcher(requestOf(time), data, family, true);
    styles.Time[style] = time;
    styles.Connector[style] = data.atTimeFormats[i];
    styles.DateTimeRangeFormat[style] = {
      __proto__: null,
      full: undefined,
      long: undefined,
      medium: undefined,
      short: undefined,
    };
  }
  // 3. Return styles.
  return styles;
}

/// The cache of dateTimePatterns results (decision 13: 64 entries,
/// emptied when full).
var dateTimePatternsCache = newDataCache();

/// native.dateTimePatterns(languageId, calendar), through its cache.
/// languageId is the language identifier of [[Locale]]. The list is shared
/// and never modified.
function cachedDateTimePatterns(languageId, calendar) {
  // 1. Let key be languageId and calendar, joined by ";".
  var key = languageId + ';' + calendar;
  // 2. If the cache has a result for key, return it.
  var patterns = dataCacheGet(dateTimePatternsCache, key);
  if (patterns !== undefined) return patterns;
  // 3. Otherwise ask native code, and cache the result.
  patterns = native.dateTimePatterns(languageId, calendar);
  dataCachePut(dateTimePatternsCache, key, patterns);
  return patterns;
}

/// How many dateTimeLocaleData results their cache keeps (decision 13).
var DATE_TIME_LOCALE_DATA_CACHE_SIZE = 32;
var dateTimeLocaleDataCache = newDataCache(DATE_TIME_LOCALE_DATA_CACHE_SIZE);

/// dateTimeLocaleData of languageId, calendar and decimalSeparator,
/// through its cache, keyed by all three, since the fraction's separator
/// depends on [[NumberingSystem]] (decision 13). One entry has both
/// hour-cycle families.
function cachedDateTimeLocaleData(languageId, calendar, decimalSeparator) {
  // 1. Let key be the three, joined by ";".
  var key = languageId + ';' + calendar + ';' + decimalSeparator;
  // 2. If the cache has a result for key, return it.
  var data = dataCacheGet(dateTimeLocaleDataCache, key);
  if (data !== undefined) return data;
  // 3. Otherwise make it, and cache it.
  data = dateTimeLocaleData(
    cachedDateTimePatterns(languageId, calendar),
    calendar,
    decimalSeparator,
  );
  dataCachePut(dateTimeLocaleDataCache, key, data);
  return data;
}

// ---------------------------------------------------------------------------
// The format matchers (decision 6).

/// The index of v in values, or -1.
function indexInList(values, v) {
  // 1. Return the first index whose element is v.
  for (var i = 0; i < values.length; i++) if (values[i] === v) return i;
  return -1;
}

/// BasicFormatMatcher's steps 1-7 and 10.a-b for one format: its score
/// for options. Best fit scores with it too (decision 6), with bestFit
/// true, which adds two refinements of decision 6: KIND_MISMATCH_PENALTY
/// for a numeric field shown as text or the other way, and a timeZoneName
/// of another kind (specific, offset, generic) costs shortMorePenalty, a
/// small width difference, not a removal. BasicFormatMatcher passes false.
function basicFormatScore(options, format, bestFit = false) {
  // 1-7. The penalties.
  var removalPenalty = 120;
  var additionPenalty = 20;
  var longLessPenalty = 8;
  var longMorePenalty = 6;
  var shortLessPenalty = 6;
  var shortMorePenalty = 3;
  var offsetPenalty = 1;
  // 10.a. Let score be 0.
  var score = 0;
  // 10.b. For each row of the table, except the header row, in table
  //       order, do
  for (var i = 0; i < DATE_TIME_COMPONENTS.length; i++) {
    // i. Let property be the name given in the Property column of the
    //    current row.
    var property = DATE_TIME_COMPONENTS[i];
    // ii. If options has a field [[<property>]], let optionsProp be
    //     options.[[<property>]]; else let optionsProp be undefined.
    var optionsProp = options[property];
    // iii. If format has a field [[<property>]], let formatProp be
    //      format.[[<property>]]; else let formatProp be undefined.
    var formatProp = format[property];
    if (optionsProp === undefined && formatProp !== undefined) {
      // iv. If optionsProp is undefined and formatProp is not undefined,
      //     then
      //     1. Set score to score - additionPenalty.
      score -= additionPenalty;
    } else if (optionsProp !== undefined && formatProp === undefined) {
      // v. Else if optionsProp is not undefined and formatProp is
      //    undefined, then
      //    1. Set score to score - removalPenalty.
      score -= removalPenalty;
    } else if (property === 'timeZoneName') {
      // vi. Else if property is "timeZoneName", then
      var penalty = timeZoneNamePenalty(optionsProp, formatProp);
      if (
        bestFit &&
        timeZoneNameKind(optionsProp) !== timeZoneNameKind(formatProp)
      )
        penalty = MathMin(penalty, shortMorePenalty);
      score -= penalty;
    } else if (optionsProp !== formatProp) {
      // vii. Else if optionsProp ≠ formatProp, then
      // 1. If property is "fractionalSecondDigits", then
      //    a. Let values be « 1, 2, 3 ».
      // 2. Else,
      //    a. Let values be « "2-digit", "numeric", "narrow", "short",
      //       "long" ».
      // 3. Let optionsPropIndex be the index of optionsProp within values.
      // 4. Let formatPropIndex be the index of formatProp within values.
      //    (In « 1, 2, 3 », the index of a value is the value less 1.)
      var optionsPropIndex;
      var formatPropIndex;
      if (property === 'fractionalSecondDigits') {
        optionsPropIndex = optionsProp - 1;
        formatPropIndex = formatProp - 1;
      } else {
        optionsPropIndex = indexInList(FORMAT_WIDTH_VALUES, optionsProp);
        formatPropIndex = indexInList(FORMAT_WIDTH_VALUES, formatProp);
      }
      // 5. Let delta be max(min(formatPropIndex - optionsPropIndex, 2),
      //    -2).
      var delta = MathMax(MathMin(formatPropIndex - optionsPropIndex, 2), -2);
      // 6. If delta = 2, set score to score - longMorePenalty.
      // 7. Else if delta = 1, set score to score - shortMorePenalty.
      // 8. Else if delta = -1, set score to score - shortLessPenalty.
      // 9. Else if delta = -2, set score to score - longLessPenalty.
      if (delta === 2) score -= longMorePenalty;
      else if (delta === 1) score -= shortMorePenalty;
      else if (delta === -1) score -= shortLessPenalty;
      else if (delta === -2) score -= longLessPenalty;
      // A refinement of decision 6, best fit only: a field asked for as
      // numeric (numeric or 2-digit) and shown as text (narrow, short or
      // long), or the other way, costs KIND_MISMATCH_PENALTY more.
      if (
        bestFit &&
        property !== 'fractionalSecondDigits' &&
        (optionsPropIndex <= 1) !== (formatPropIndex <= 1)
      )
        score -= KIND_MISMATCH_PENALTY;
    }
  }
  return score;
}

/// The extra cost in best fit's scoring of a field whose requested and
/// skeleton values are of different kinds, numeric and text (a refinement
/// of decision 6). Without it, numeric to 2-digit costs as much as numeric
/// to short (6 each). With the draft's width penalty, a kind mismatch
/// costs 16 (numeric to narrow), 19 (numeric or 2-digit to short or long,
/// 2-digit to narrow, narrow to numeric) or 21 (short or long to numeric,
/// short, long or narrow to 2-digit): more than two numeric width changes
/// (12), and, but for the 21s, less than one extra field (the addition
/// penalty of 20), the order in which ICU's skeleton distance ranks them.
/// So for en-GB's {year, month, day: "numeric"}, yMd "dd/MM/y" (its
/// skeleton scores 0) beats yMMMd "d MMM y" (-19).
var KIND_MISMATCH_PENALTY = 13;

/// BasicFormatMatcher's step 10.b.vi: the penalty for a timeZoneName of
/// width formatProp where optionsProp was asked for, both present.
function timeZoneNamePenalty(optionsProp, formatProp) {
  var removalPenalty = 120;
  var longLessPenalty = 8;
  var shortMorePenalty = 3;
  var offsetPenalty = 1;
  // 1. If optionsProp is "short" or "shortGeneric", then
  if (optionsProp === 'short' || optionsProp === 'shortGeneric') {
    // a. If formatProp is "shortOffset", set score to score -
    //    offsetPenalty.
    if (formatProp === 'shortOffset') return offsetPenalty;
    // b. Else if formatProp is "longOffset", set score to score -
    //    (offsetPenalty + shortMorePenalty).
    if (formatProp === 'longOffset') return offsetPenalty + shortMorePenalty;
    // c. Else if optionsProp is "short" and formatProp is "long", set score
    //    to score - shortMorePenalty.
    if (optionsProp === 'short' && formatProp === 'long')
      return shortMorePenalty;
    // d. Else if optionsProp is "shortGeneric" and formatProp is
    //    "longGeneric", set score to score - shortMorePenalty.
    if (optionsProp === 'shortGeneric' && formatProp === 'longGeneric')
      return shortMorePenalty;
    // e. Else if optionsProp ≠ formatProp, set score to score -
    //    removalPenalty.
    return optionsProp !== formatProp ? removalPenalty : 0;
  }
  // 2. Else if optionsProp is "shortOffset" and formatProp is "longOffset",
  //    then
  //    a. Set score to score - shortMorePenalty.
  if (optionsProp === 'shortOffset' && formatProp === 'longOffset')
    return shortMorePenalty;
  // 3. Else if optionsProp is "long" or "longGeneric", then
  if (optionsProp === 'long' || optionsProp === 'longGeneric') {
    // a. If formatProp is "longOffset", set score to score -
    //    offsetPenalty.
    if (formatProp === 'longOffset') return offsetPenalty;
    // b. Else if formatProp is "shortOffset", set score to score -
    //    (offsetPenalty + longLessPenalty).
    if (formatProp === 'shortOffset') return offsetPenalty + longLessPenalty;
    // c. Else if optionsProp is "long" and formatProp is "short", set score
    //    to score - longLessPenalty.
    if (optionsProp === 'long' && formatProp === 'short')
      return longLessPenalty;
    // d. Else if optionsProp is "longGeneric" and formatProp is
    //    "shortGeneric", set score to score - longLessPenalty.
    if (optionsProp === 'longGeneric' && formatProp === 'shortGeneric')
      return longLessPenalty;
    // e. Else if optionsProp ≠ formatProp, set score to score -
    //    removalPenalty.
    return optionsProp !== formatProp ? removalPenalty : 0;
  }
  // 4. Else if optionsProp is "longOffset" and formatProp is "shortOffset",
  //    then
  //    a. Set score to score - longLessPenalty.
  if (optionsProp === 'longOffset' && formatProp === 'shortOffset')
    return longLessPenalty;
  // 5. Else if optionsProp ≠ formatProp, then
  //    a. Set score to score - removalPenalty.
  return optionsProp !== formatProp ? removalPenalty : 0;
}

// BasicFormatMatcher ( options, formats )
function BasicFormatMatcher(options, formats) {
  // 1-7. The penalties are in basicFormatScore.
  // 8. Let bestScore be -∞.
  var bestScore = -Infinity;
  // 9. Let bestFormat be undefined.
  var bestFormat = undefined;
  // 10. For each element format of formats, do
  for (var i = 0; i < formats.length; i++) {
    var format = formats[i];
    // a-b. Let score be the score of format (basicFormatScore).
    var score = basicFormatScore(options, format);
    // c. If score > bestScore, then
    if (score > bestScore) {
      // i. Set bestScore to score.
      bestScore = score;
      // ii. Set bestFormat to format.
      bestFormat = format;
    }
  }
  // 11. Return bestFormat.
  return bestFormat;
}

/// BasicFormatMatcher(options, formatsOf(data, family)), without gluing
/// every date record to every time record. A glued record has the date
/// fields of its date record and the time fields of its time record, and
/// BasicFormatMatcher's score is a sum over the fields, so the score of
/// the record gluing date D to time T is score(D) + score(T) - score(∅),
/// where ∅ has no field: the best glued record glues the first date record
/// with the best score to the first time record with the best score. It
/// comes after every record that is not glued, so it is chosen only if it
/// scores more than all of them, as in BasicFormatMatcher's step 10.c.
function basicFormatMatch(options, data, family) {
  var unglued = ungluedFormatsOf(data, family);
  // 1. Let bestFormat be BasicFormatMatcher(options, the records that are
  //    not glued), and bestScore its score.
  var bestFormat = BasicFormatMatcher(options, unglued.list);
  var bestScore = basicFormatScore(options, bestFormat);
  // 2. If there are no date or no time records, return bestFormat.
  if (unglued.dates.length === 0 || unglued.times.length === 0)
    return bestFormat;
  // 3. Let d and t be the first date and time records with the best score.
  var d = bestRecordIndex(options, unglued.dates);
  var t = bestRecordIndex(options, unglued.times);
  // 4. If their glued record scores more than bestScore, return it; else
  //    return bestFormat.
  var gluedScore =
    basicFormatScore(options, unglued.dates[d]) +
    basicFormatScore(options, unglued.times[t]) -
    basicFormatScore(options, {__proto__: null});
  if (gluedScore > bestScore) return gluedFormatRecord(data, family, d, t);
  return bestFormat;
}

/// The index of the first record of formats with the best
/// BasicFormatMatcher score for options; formats is not empty.
function bestRecordIndex(options, formats) {
  // 1. Keep the first index of the highest score.
  var best = 0;
  var bestScore = basicFormatScore(options, formats[0]);
  for (var i = 1; i < formats.length; i++) {
    var score = basicFormatScore(options, formats[i]);
    if (score > bestScore) {
      bestScore = score;
      best = i;
    }
  }
  // 2. Return it.
  return best;
}

/// Decision 6: the request a family's records are matched against. CLDR's
/// patterns with an hour and a day period are all 12-hour, so in the
/// "h24" family a request with an hour drops its dayPeriod, as ICU4C does
/// (en {dayPeriod: "short", hour: "numeric", hourCycle: "h23"} is "09").
function familyFormatOptions(options, family) {
  // 1. If family is "h24" and options has an hour and a dayPeriod, return
  //    options without its dayPeriod.
  if (
    family === 'h24' &&
    options.hour !== undefined &&
    options.dayPeriod !== undefined
  ) {
    var result = requestOf(options);
    result.dayPeriod = undefined;
    return result;
  }
  // 2. Otherwise return options.
  return options;
}

/// The hour-cycle family whose records a request options is matched in,
/// for the family of the resolved hour cycle: "h12" for a request with a
/// dayPeriod and no hour, since CLDR's records with a day period and
/// another time field are all 12-hour, and the "h24" family has none with
/// a minute or a second (a refinement of decision 6: en {dayPeriod:
/// "short", minute: "numeric", hour12: false} is "13:07 in the afternoon",
/// not "in the afternoon"); else family. The hour, if the record has one,
/// is still shown by the resolved hour cycle.
function formatsFamilyOf(options, family) {
  // 1. Return "h12" for a dayPeriod without an hour, else family.
  if (options.dayPeriod !== undefined && options.hour === undefined)
    return 'h12';
  return family;
}

/// The number of fields options asks for that record (a DateTime Format
/// Record or its skeleton's widths) lacks and best fit's step 6 cannot
/// append: those other than timeZoneName and fractionalSecondDigits.
function unappendableMissingCount(options, record) {
  // 1. Count the missing fields that cannot be appended.
  var count = 0;
  for (var i = 0; i < DATE_TIME_COMPONENTS.length; i++) {
    var name = DATE_TIME_COMPONENTS[i];
    if (name === 'timeZoneName' || name === 'fractionalSecondDigits') continue;
    if (options[name] !== undefined && record[name] === undefined) count++;
  }
  return count;
}

/// CreateDateTimeFormat steps 31.g-h, with [[formats]] keyed by calendar
/// and hour-cycle family (decision 6): "Let formats be
/// resolvedLocaleData.[[formats]].[[<resolvedCalendar>]]. If formatMatcher
/// is "basic", let bestFormat be BasicFormatMatcher(formatOptions,
/// formats); else let bestFormat be BestFitFormatMatcher(formatOptions,
/// formats)." Both match familyFormatOptions of formatOptions.
function matchDateTimeFormat(formatOptions, data, family, formatMatcher) {
  // g. Let formats be data's [[formats]] of family.
  // h. Match with the matcher formatMatcher names.
  if (formatMatcher === 'basic') {
    return basicFormatMatch(
      familyFormatOptions(formatOptions, family),
      data,
      family,
    );
  }
  return BestFitFormatMatcher(formatOptions, data, family);
}

/// How many matchDateTimeFormat results each dateTimeLocaleData record
/// keeps (decision 13's caches, a refinement: 32 entries, emptied when
/// full).
var FORMAT_MATCH_CACHE_SIZE = 32;

/// matchDateTimeFormat(formatOptions, data, family, formatMatcher), through
/// data's cache of matches, keyed by the matcher, the family and each
/// requested field's width (formatOptions' [[hourCycle]] does not choose
/// the record). The record is shared by every DateTimeFormat that asks for
/// the same fields, with its range patterns (rangePatternsOf keeps them on
/// it), and is never modified after rangePatternsOf; nothing can observe
/// the sharing.
function cachedFormatMatch(formatOptions, data, family, formatMatcher) {
  // 1. Let key be the matcher, the family and each field's value.
  var key = formatMatcher + ';' + family;
  for (var i = 0; i < DATE_TIME_COMPONENTS.length; i++) {
    var value = formatOptions[DATE_TIME_COMPONENTS[i]];
    key += ';' + (value === undefined ? '' : value);
  }
  // 2. If the cache has a record for key, return it.
  var record = dataCacheGet(data.matches, key);
  if (record !== undefined) return record;
  // 3. Otherwise match, and cache the result.
  record = matchDateTimeFormat(formatOptions, data, family, formatMatcher);
  dataCachePut(data.matches, key, record);
  return record;
}

/// Whether record has every field options asks for.
function hasEveryField(record, options) {
  // 1. Return false if a requested field is missing.
  for (var i = 0; i < DATE_TIME_COMPONENTS.length; i++) {
    var name = DATE_TIME_COMPONENTS[i];
    if (options[name] !== undefined && record[name] === undefined) return false;
  }
  return true;
}

/// The kind of a timeZoneName width: specific, offset or generic.
function timeZoneNameKind(width) {
  // 1. Return the kind of width.
  if (width === 'short' || width === 'long') return 'specific';
  if (width === 'shortOffset' || width === 'longOffset') return 'offset';
  return 'generic';
}

/// Decision 6's width adjustment (best fit only), based on the matched
/// skeleton as ICU's is (a refinement of decision 6): a copy of record in
/// which a field both options and record have is adjusted only when the
/// requested width differs from record's skeleton for it, compared as ICU
/// compares them, by the number of letters (WIDTH_LETTER_COUNTS for the
/// request, the skeleton's own count); then the pattern's field takes the
/// requested width if the pattern's is of the same kind (numeric or
/// 2-digit, or text), else keeps the pattern's. So a 2-digit hour makes
/// skeleton h's "h" "hh", but a numeric hour keeps skeleton H's "HH"; cs's
/// yMMM "LLLL y" stays long for {month: "short"} ("MMM"); and eu's Ed "d,
/// EEEE" becomes short for {weekday: "short"} ("EEE", not Ed's "E").
/// timeZoneName takes the requested one, of any kind (best fit scores
/// another kind as a small width difference, a refinement of decision 6),
/// and fractionalSecondDigits the requested number. A [[dayPeriod]]
/// options does not ask for is dropped, and the width {dayPeriod} shows
/// (dayPeriodWidth) is set to the adjusted [[dayPeriod]]. Unless
/// keepDayPeriod (true for the best fits of a style record, which keeps
/// its B, and of its range patterns, keepsDayPeriod), a B that options
/// does not ask for then shows as AM/PM (showDayPeriodAsAmPm, step 4),
/// which keeps the record with the B as withDayPeriod.
function adjustFormatWidths(options, record, keepDayPeriod = false) {
  // 1. Let result be a copy of record, and counts the letter counts of its
  //    skeleton, if it has one.
  var result = copyFormatRecord(record, options.dayPeriod !== undefined);
  var counts =
    record.skeletonWidths !== undefined
      ? record.skeletonWidths.letterCounts
      : undefined;
  // 2. Adjust each field both have; an adjusted field's letter count
  //    becomes the requested width's, as ICU's adjusted pattern writes it.
  var adjustedCounts = {__proto__: null};
  if (record.patternLetterCounts !== undefined) {
    for (var c = 0; c < DATE_TIME_COMPONENTS.length; c++) {
      var countName = DATE_TIME_COMPONENTS[c];
      if (record.patternLetterCounts[countName] !== undefined)
        adjustedCounts[countName] = record.patternLetterCounts[countName];
    }
  }
  result.patternLetterCounts = adjustedCounts;
  for (var i = 0; i < DATE_TIME_COMPONENTS.length; i++) {
    var name = DATE_TIME_COMPONENTS[i];
    var wanted = options[name];
    var width = result[name];
    if (wanted === undefined || width === undefined) continue;
    if (name === 'fractionalSecondDigits' || name === 'timeZoneName') {
      result[name] = wanted;
    } else if (
      counts !== undefined && counts[name] !== undefined
        ? WIDTH_LETTER_COUNTS[wanted] !== counts[name]
        : wanted !== width
    ) {
      var wantedNumeric = wanted === 'numeric' || wanted === '2-digit';
      var widthNumeric = width === 'numeric' || width === '2-digit';
      if (wantedNumeric === widthNumeric) {
        result[name] = wanted;
        adjustedCounts[name] = WIDTH_LETTER_COUNTS[wanted];
      }
    }
  }
  // 3. {dayPeriod} shows the [[dayPeriod]] width asked for.
  if (result.dayPeriod !== undefined) result.dayPeriodWidth = result.dayPeriod;
  // 4. Unless keepDayPeriod, a B the request did not ask for shows as AM/PM.
  if (!keepDayPeriod && options.dayPeriod === undefined)
    showDayPeriodAsAmPm(result);
  // 5. Return result.
  return result;
}

/// A refinement of decision 6, superseding for best fit its "a B the
/// request did not ask for formats as the day period": as ICU does,
/// record's {dayPeriod} becomes {ampm} of the B's width (abbreviated for B,
/// wide for BBBB, narrow for BBBBB), so zh-TW {hour} is "下午8時", not
/// "晚上8時". record, a copy the caller owns, is changed, and keeps a copy
/// of itself with the B as withDayPeriod: its range patterns take CLDR's
/// intervals as for the B, as ICU4C's do (addTimeRangeFields). Style
/// records and basic keep the B.
function showDayPeriodAsAmPm(record) {
  // 1. If the pattern shows no day period, there is nothing to change.
  if (
    record.dayPeriodWidth === undefined ||
    !hasPlaceholder(record.pattern, 'dayPeriod')
  )
    return;
  // 2. Keep the record with the B, and replace each {dayPeriod} with
  //    {ampm}, of the day period's width.
  record.withDayPeriod = copyFormatRecord(record);
  record.pattern = replaceDayPeriodPlaceholders(record.pattern);
  record.pattern12 = record.pattern;
  if (record.ampmWidth === undefined)
    record.ampmWidth = DAY_PERIOD_AMPM_WIDTHS[record.dayPeriodWidth];
  record.dayPeriodWidth = undefined;
}

/// The AM/PM width that shows a B of each day period width.
var DAY_PERIOD_AMPM_WIDTHS = {
  __proto__: null,
  short: 'abbreviated',
  long: 'wide',
  narrow: 'narrow',
};

/// pattern, a Pattern String, with every {dayPeriod} replaced by {ampm}.
function replaceDayPeriodPlaceholders(pattern) {
  // 1. Copy pattern, writing {ampm} for each {dayPeriod}.
  var result = '';
  var from = 0;
  var i = stringIndexOf(pattern, '{dayPeriod}', 0);
  while (i !== -1) {
    result += stringSlice(pattern, from, i) + '{ampm}';
    from = i + 11;
    i = stringIndexOf(pattern, '{dayPeriod}', from);
  }
  // 2. Return it with the rest.
  return result + stringSlice(pattern, from);
}

/// Decision 6's best-fit step 4 (BestFitFormatMatcher's step 6): record, a
/// copy the caller owns, with the fields options asks for that it lacks
/// appended: fractionalSecondDigits right after {second}, preceded by the
/// decimal separator; timeZoneName by the Timezone append item ("{0}
/// {1}"). A record whose B shows as AM/PM (showDayPeriodAsAmPm) gets them
/// in its withDayPeriod too, whose intervals its range patterns take
/// (addTimeRangeFields), so a range keeps the appended fields.
function appendMissingFields(options, record, data) {
  // 1. Append the fraction of a second.
  if (
    options.fractionalSecondDigits !== undefined &&
    record.fractionalSecondDigits === undefined
  ) {
    insertFraction(
      record,
      options.fractionalSecondDigits,
      data.decimalSeparator,
    );
  }
  // 2. Append the time zone name.
  if (options.timeZoneName !== undefined && record.timeZoneName === undefined)
    appendTimeZoneName(record, data, options.timeZoneName);
  // 3. Append them to a copy of withDayPeriod, and return record.
  if (record.withDayPeriod !== undefined) {
    record.withDayPeriod = appendMissingFields(
      options,
      copyFormatRecord(record.withDayPeriod),
      data,
    );
  }
  return record;
}

/// Decision 6: record, a copy the caller owns, with {timeZoneName} of
/// width appended by data's Timezone append item ("{0} {1}"); it is
/// changed and returned.
function appendTimeZoneName(record, data, width) {
  // 1. Replace {0} with the pattern and {1} with "{timeZoneName}".
  record.pattern = substitutePlaceholders(
    data.timeZoneAppend,
    record.pattern,
    '{timeZoneName}',
  );
  record.pattern12 = record.pattern;
  // 2. Set the field, and return record. Its fields are now its own.
  record.timeZoneName = width;
  record.skeletonWidths = undefined;
  return record;
}

/// Whether record, a DateTime Format Record or a request, has a field and
/// all its fields are ones best fit appends (timeZoneName,
/// fractionalSecondDigits).
function isAppendableOnly(record) {
  // 1. Return whether one of the two is present and no other field is.
  var appendable = false;
  for (var i = 0; i < DATE_TIME_COMPONENTS.length; i++) {
    var name = DATE_TIME_COMPONENTS[i];
    if (record[name] === undefined) continue;
    if (name !== 'timeZoneName' && name !== 'fractionalSecondDigits')
      return false;
    appendable = true;
  }
  return appendable;
}

/// Best fit's finer width distance of record for options (a refinement of
/// decision 6): the sum, over the fields options asks for with a width of
/// WIDTH_LETTER_COUNTS (not timeZoneName, whose "short" and "long" are not
/// letter counts), of the absolute difference between the requested
/// letter count and the record's skeleton's (its own width's for a record
/// without a skeleton), not clamped as the draft's delta is, so that short
/// is closer to numeric than long is, as in ICU's type distance. Fields the
/// record lacks count nothing; basicFormatScore has scored them.
function fineWidthDistance(options, record) {
  // 1. Let counts be the letter counts of record's skeleton, if any.
  var counts =
    record.skeletonWidths !== undefined
      ? record.skeletonWidths.letterCounts
      : undefined;
  // 2. Sum the differences.
  var distance = 0;
  for (var i = 0; i < DATE_TIME_COMPONENTS.length; i++) {
    var name = DATE_TIME_COMPONENTS[i];
    if (name === 'timeZoneName') continue;
    var wanted = WIDTH_LETTER_COUNTS[options[name]];
    if (wanted === undefined) continue;
    var have =
      counts !== undefined ? counts[name] : WIDTH_LETTER_COUNTS[record[name]];
    if (have === undefined) continue;
    distance += MathMax(wanted - have, have - wanted);
  }
  // 3. Return the sum.
  return distance;
}

/// Whether skeleton a comes before skeleton b in code point order; a
/// record without a skeleton (undefined) comes after every skeleton.
function skeletonBefore(a, b) {
  // 1. Compare the two.
  if (a === undefined) return false;
  if (b === undefined) return true;
  return a < b;
}

/// Decision 6's best-fit matcher (the draft leaves BestFitFormatMatcher to
/// the implementation), over data's records of family. The result is a
/// new record. Unless keepDayPeriod (a style record's), a B the request
/// did not ask for shows as AM/PM (showDayPeriodAsAmPm).
function BestFitFormatMatcher(
  options,
  data,
  family,
  keepDayPeriod = false,
) {
  // 1. Let family be formatsFamilyOf(options, family), and options
  //    familyFormatOptions(options, family). A request with
  //    fractionalSecondDigits and an hour or minute is matched as if it
  //    also asked for second: "numeric", as ICU4C does.
  family = formatsFamilyOf(options, family);
  options = familyFormatOptions(options, family);
  if (
    options.fractionalSecondDigits !== undefined &&
    options.second === undefined &&
    (options.hour !== undefined || options.minute !== undefined)
  ) {
    options = requestOf(options);
    options.second = 'numeric';
  }
  // 2. Score the availableFormats and one-field records with
  //    BasicFormatMatcher's penalties as basicFormatScore refines them for
  //    best fit, by their skeletons' fields and widths (scoringFieldsOf);
  //    ties go, in chinese and dangi, to a record with {relatedYear}; then
  //    to the smaller fineWidthDistance, which ranks widths as ICU's type
  //    distance does where the draft's clamped delta ties them (en
  //    chinese's GyMMMd beats GyMMMMd for a numeric month, en hebrew's yMd
  //    beats yyyyMd, ko dangi's yyyyMMMEEEEd "... MMM d일 EEEE" beats
  //    yyyyMMMMEd "... MMMM d일(E)" for a long month and weekday); then to
  //    the skeleton first in code point order, and to an availableFormats
  //    record before a one-field one (refinements of decision 6, which said
  //    CLDR's order). The code point step matches ICU only for the ties
  //    ICU's distance also has, where ICU keeps the first match in the
  //    order it loads them, its resource tables' key order. A
  //    refinement of decision 6: the one-field records of the two fields
  //    step 6 appends, {timeZoneName} and {fractionalSecondDigits}, are
  //    scored only for a request without other fields: otherwise
  //    {timeZoneName} (-120 for the missing hour) would beat "HH" (-126:
  //    -6 for the width, -120 for the time zone name) for {hour:
  //    "numeric", timeZoneName: "short"} in family "h24", and the hour
  //    would be lost. Before the score, a record ranks by the number of
  //    requested fields it lacks that step 6 cannot append (all but
  //    timeZoneName and fractionalSecondDigits; unappendableMissingCount),
  //    fewer first, so a missing appendable field never costs one that
  //    cannot be appended: th's {hour, minute, timeZoneName} takes Hm "HH:mm"
  //    (-126) and appends the zone, not Hv "HH v" (-123), which drops the
  //    minute (a refinement of decision 6).
  var records = data.available[family];
  var preferRelatedYear = calendarIsCyclic(data.calendar);
  var appendableOnly = isAppendableOnly(options);
  var best = undefined;
  var bestLacking = Infinity;
  var bestScore = -Infinity;
  var bestRelated = false;
  var bestDistance = 0;
  for (var i = 0; i < records.length; i++) {
    var fields = scoringFieldsOf(records[i]);
    if (!appendableOnly && isAppendableOnly(fields)) continue;
    var lacking = unappendableMissingCount(options, fields);
    if (lacking > bestLacking) continue;
    var score = basicFormatScore(options, fields, true);
    if (lacking === bestLacking && score < bestScore) continue;
    var related =
      preferRelatedYear && hasPlaceholder(records[i].pattern, 'relatedYear');
    var distance = fineWidthDistance(options, records[i]);
    if (
      lacking < bestLacking ||
      score > bestScore ||
      (related !== bestRelated
        ? related
        : distance !== bestDistance
        ? distance < bestDistance
        : skeletonBefore(fields.skeleton, scoringFieldsOf(best).skeleton))
    ) {
      best = records[i];
      bestLacking = lacking;
      bestScore = score;
      bestRelated = related;
      bestDistance = distance;
    }
  }
  // 3. If the best record's skeleton has every requested field, return it,
  //    with widths adjusted, unless it also has a field the request lacks
  //    and the request has date and time fields (a refinement of decision
  //    6: ICU then splits, so en-GB {weekday: "long", hour} is "Wednesday,
  //    23", not EHm's "Wednesday 23:37").
  var bestFields = scoringFieldsOf(best);
  var dateAndTime = hasDateField(options) && hasTimeField(options);
  var hasExtraField = !hasEveryField(options, bestFields);
  if (hasEveryField(bestFields, options) && !(dateAndTime && hasExtraField))
    return adjustFormatWidths(options, best, keepDayPeriod);
  // 4. Else, if the request has both date fields and time fields, match
  //    each half by this algorithm and glue them.
  if (dateAndTime) {
    var date = BestFitFormatMatcher(
      requestOf(options, 0, FIRST_TIME_COMPONENT),
      data,
      family,
      keepDayPeriod,
    );
    var time = BestFitFormatMatcher(
      requestOf(options, FIRST_TIME_COMPONENT),
      data,
      family,
      keepDayPeriod,
    );
    // The connector is chosen by the requested date fields (UTS 35's
    // "requested date fields include wide month"), as ICU does.
    return glueFormatRecords(
      date,
      time,
      data.atTimeFormats[
        connectorStyleIndex(requestOf(options, 0, FIRST_TIME_COMPONENT))
      ],
    );
  }
  // 5. Else, if the request asks for fractionalSecondDigits with no hour or
  //    minute and the best record has no {second} to put it after, return
  //    the one-field {fractionalSecondDigits} record joined by a space to
  //    the best record for the other fields, the fraction first, as ICU4C
  //    gives (en {dayPeriod, fractionalSecondDigits: 2} "04 in the
  //    morning"); a refinement of decision 6.
  if (
    options.fractionalSecondDigits !== undefined &&
    options.hour === undefined &&
    options.minute === undefined &&
    best.second === undefined &&
    best.fractionalSecondDigits === undefined
  ) {
    var rest = requestOf(options);
    rest.fractionalSecondDigits = undefined;
    var result = copyFormatRecord(
      BestFitFormatMatcher(rest, data, family, keepDayPeriod),
    );
    result.fractionalSecondDigits = options.fractionalSecondDigits;
    result.pattern = '{fractionalSecondDigits} ' + result.pattern;
    result.pattern12 = result.pattern;
    return result;
  }
  // 6. Else append the missing fields.
  return appendMissingFields(
    options,
    adjustFormatWidths(options, best, keepDayPeriod),
    data,
  );
}

// ---------------------------------------------------------------------------
// Styles (decision 7).

/// Add to target every field of source except those in excluded, as
/// DateTimeStyleFormat's steps 4.b-c say ("Add to format all fields from
/// ... except ..."). A field source lacks (undefined) is not added.
function addFieldsExcept(target, source, excluded) {
  // 1. For each own property key of source not in excluded, copy it.
  var keys = ReflectOwnKeys(source);
  for (var i = 0; i < keys.length; i++) {
    var key = keys[i];
    if (source[key] !== undefined && !listContains(excluded, key))
      target[key] = source[key];
  }
}

// DateTimeStyleFormat ( dateStyle, timeStyle, styles ), with decision 7's
// records. styles is the DateTime Styles Record of an hour-cycle family
// (dateTimeLocaleData's [[Styles]]).
function DateTimeStyleFormat(dateStyle, timeStyle, styles) {
  // 1. Assert: dateStyle is not undefined or timeStyle is not undefined.
  var timeFormat;
  var dateFormat;
  // 2. If timeStyle is not undefined, then
  if (timeStyle !== undefined) {
    // a. Assert: timeStyle is one of "full", "long", "medium", or "short".
    // b. Let timeFormat be styles.[[Time]].[[<timeStyle>]].
    timeFormat = styles.Time[timeStyle];
  }
  // 3. If dateStyle is not undefined, then
  if (dateStyle !== undefined) {
    // a. Assert: dateStyle is one of "full", "long", "medium", or "short".
    // b. Let dateFormat be styles.[[Date]].[[<dateStyle>]].
    dateFormat = styles.Date[dateStyle];
  }
  // 4. If dateStyle is not undefined and timeStyle is not undefined, then
  if (dateStyle !== undefined && timeStyle !== undefined) {
    // a. Let format be a new DateTime Format Record.
    var format = newFormatRecord();
    // b. Add to format all fields from dateFormat except [[pattern]] and
    //    [[rangePatterns]].
    addFieldsExcept(format, dateFormat, ['pattern', 'rangePatterns']);
    // c. Add to format all fields from timeFormat except [[pattern]],
    //    [[rangePatterns]], [[pattern12]], and [[rangePatterns12]], if
    //    present. The internal fields come too, so format has the time
    //    part's patternLetterCounts, which its range patterns' interval
    //    widths compare (addTimeRangeFields), its hourLetter, AM/PM and
    //    day period widths, and its withDayPeriod.
    addFieldsExcept(format, timeFormat, [
      'pattern',
      'rangePatterns',
      'pattern12',
      'rangePatterns12',
    ]);
    //    The contexts are the date's, and format keeps both parts for its
    //    range patterns (decisions 6 and 11).
    format.monthStandalone = dateFormat.monthStandalone;
    format.weekdayStandalone = dateFormat.weekdayStandalone;
    format.datePart = dateFormat;
    format.timePart = timeFormat;
    // d. Let connector be styles.[[Connector]].[[<dateStyle>]].
    var connector = styles.Connector[dateStyle];
    // e. Let pattern be the string connector with the substring "{0}"
    //    replaced with timeFormat.[[pattern]] and the substring "{1}"
    //    replaced with dateFormat.[[pattern]].
    var pattern = substitutePlaceholders(
      connector,
      timeFormat.pattern,
      dateFormat.pattern,
    );
    // f. Set format.[[pattern]] to pattern.
    format.pattern = pattern;
    // g. If timeFormat has a [[pattern12]] field, then
    //    i. Let pattern12 be the string connector with the substring "{0}"
    //       replaced with timeFormat.[[pattern12]] and the substring "{1}"
    //       replaced with dateFormat.[[pattern]].
    //    ii. Set format.[[pattern12]] to pattern12.
    format.pattern12 = substitutePlaceholders(
      connector,
      timeFormat.pattern12,
      dateFormat.pattern,
    );
    // h. Let dateTimeRangeFormat be
    //    styles.[[DateTimeRangeFormat]].[[<dateStyle>]].[[<timeStyle>]]:
    //    a DateTime Style Range Record made from format on first use.
    var rangeFormats = styles.DateTimeRangeFormat[dateStyle];
    var dateTimeRangeFormat = rangeFormats[timeStyle];
    if (dateTimeRangeFormat === undefined) {
      var rangePatterns = rangePatternsOf(
        format,
        styles.localeData,
        styles.family,
      );
      dateTimeRangeFormat = {
        __proto__: null,
        rangePatterns: rangePatterns,
        rangePatterns12: rangePatterns,
      };
      rangeFormats[timeStyle] = dateTimeRangeFormat;
    }
    // i. Set format.[[rangePatterns]] to
    //    dateTimeRangeFormat.[[rangePatterns]].
    format.rangePatterns = dateTimeRangeFormat.rangePatterns;
    // j. If dateTimeRangeFormat has a [[rangePatterns12]] field, then
    //    i. Set format.[[rangePatterns12]] to
    //       dateTimeRangeFormat.[[rangePatterns12]].
    format.rangePatterns12 = dateTimeRangeFormat.rangePatterns12;
    // k. Return format.
    return format;
  }
  // 5. If timeStyle is not undefined, then
  //    a. Return timeFormat.
  if (timeStyle !== undefined) return timeFormat;
  // 6. Assert: dateStyle is not undefined.
  // 7. Return dateFormat.
  return dateFormat;
}
