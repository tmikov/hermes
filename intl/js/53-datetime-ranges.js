/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// ECMA-402 "DateTimeFormat Objects": the DateTime Range Pattern Records of
// a DateTime Format Record ([[rangePatterns]] and [[rangePatterns12]]),
// following the draft https://tc39.es/ecma402/ dated October 24, 2026,
// built by decision 11 of the Phase 4 plan from CLDR's intervalFormats and
// intervalFormatFallback. The records, [[formats]] and the matchers they
// use are in 51-datetime-patterns.js; PartitionDateTimeRangePattern, which
// reads them, is in 52-datetime-format.js.
//
// A DateTime Range Pattern Record is a null-prototype object with the
// fields [[Default]] to [[FractionalSecondDigits]] (RANGE_FIELDS) that
// have a pattern; a field with none is not a property. Each value is a
// DateTime Range Pattern Format Record: the draft's fields and the
// contexts of the DateTime Format Record it is for, and [[PatternParts]],
// a List of { [[Source]], [[Pattern]] }. Fields that take another field's
// pattern hold the same Record.

/// The fields of a DateTime Range Pattern Record, in the draft's order
/// (the row order of PartitionDateTimeRangePattern's loop).
var RANGE_FIELDS = [
  'Default',
  'Era',
  'Year',
  'Month',
  'Day',
  'AmPm',
  'DayPeriod',
  'Hour',
  'Minute',
  'Second',
  'FractionalSecondDigits',
];
/// The range fields of the date ([[Era]] to [[Day]]) and the placeholder
/// each stands for; [[Year]] stands for any of {year}, {relatedYear} and
/// {yearName}.
var RANGE_DATE_FIELDS = ['Era', 'Year', 'Month', 'Day'];
var RANGE_DATE_PLACEHOLDERS = ['era', 'year', 'month', 'day'];
/// The range fields of the time ([[AmPm]] down) and their placeholders.
var RANGE_TIME_FIELDS = [
  'AmPm',
  'DayPeriod',
  'Hour',
  'Minute',
  'Second',
  'FractionalSecondDigits',
];
var RANGE_TIME_PLACEHOLDERS = [
  'ampm',
  'dayPeriod',
  'hour',
  'minute',
  'second',
  'fractionalSecondDigits',
];
/// The range field an intervalFormats greatest-difference letter is for
/// (decision 11).
var RANGE_FIELD_OF_LETTER = {
  __proto__: null,
  G: 'Era',
  y: 'Year',
  U: 'Year',
  r: 'Year',
  M: 'Month',
  L: 'Month',
  d: 'Day',
  a: 'AmPm',
  b: 'AmPm',
  B: 'DayPeriod',
  h: 'Hour',
  H: 'Hour',
  K: 'Hour',
  k: 'Hour',
  m: 'Minute',
};

/// The year placeholders pattern has, as a string ("y", "r" and "n" for
/// {year}, {relatedYear} and {yearName}), to compare two patterns by
/// (decision 11).
function yearPlaceholdersOf(pattern) {
  // 1. Return the letters of the year placeholders pattern has.
  return (
    (hasPlaceholder(pattern, 'year') ? 'y' : '') +
    (hasPlaceholder(pattern, 'relatedYear') ? 'r' : '') +
    (hasPlaceholder(pattern, 'yearName') ? 'n' : '')
  );
}

/// Append a DateTime Range Pattern Part Record { [[Source]]: source,
/// [[Pattern]]: pattern } to parts, merged into the last part when it has
/// the same source. An empty pattern adds nothing.
function pushRangePart(parts, source, pattern) {
  // 1. If pattern is empty, return.
  if (pattern === '') return;
  // 2. Merge into the last part, which this function made, if it has the
  //    same source; else append a new part.
  var last = parts.length > 0 ? parts[parts.length - 1] : undefined;
  if (last !== undefined && last.Source === source) {
    last.Pattern += pattern;
    return;
  }
  listPush(parts, {__proto__: null, Source: source, Pattern: pattern});
}

/// Decision 11's FB(p): the parts « startRange p, shared separator,
/// endRange p », with the separator the text between {0} and {1} of
/// intervalFormatFallback (text around them is shared).
function fallbackRangeParts(data, pattern) {
  // 1. Find {0} (the start) and {1} (the end) in the fallback.
  var fallback = data.intervalFallback;
  var i0 = stringIndexOf(fallback, '{0}', 0);
  var i1 = stringIndexOf(fallback, '{1}', 0);
  var first = i0 < i1 ? i0 : i1;
  var second = i0 < i1 ? i1 : i0;
  // 2. Let parts be the text before the first, the first, the text
  //    between, the second and the text after.
  var parts = [];
  pushRangePart(parts, 'shared', stringSlice(fallback, 0, first));
  pushRangePart(parts, first === i0 ? 'startRange' : 'endRange', pattern);
  pushRangePart(parts, 'shared', stringSlice(fallback, first + 3, second));
  pushRangePart(parts, second === i0 ? 'startRange' : 'endRange', pattern);
  pushRangePart(parts, 'shared', stringSlice(fallback, second + 3));
  // 3. Return parts.
  return parts;
}

/// Decision 11: the parts of a CLDR interval pattern in calendar, or
/// undefined if no field repeats in it. The pattern is split at the first
/// field that repeats (UTS 35 Part 4, "Interval Formats"), and the sources
/// follow ICU4C: the fields in both halves are the overlap; in each half,
/// from the first to the last overlap field is startRange or endRange, and
/// the rest is shared ("Jan 5 – 9, 2026": the month shared, "5"
/// startRange, " – " shared, "9" endRange, ", 2026" shared).
function intervalRangeParts(pattern, calendar) {
  // 1. Let names and texts be each token's placeholder name (undefined for
  //    literal text) and its text in a Pattern String.
  var tokens = cldrPatternTokens(pattern);
  var names = [];
  var texts = [];
  for (var i = 0; i < tokens.length; i++) {
    var token = tokens[i];
    if (token.letter === undefined) {
      listPush(names, undefined);
      listPush(texts, token.text);
    } else {
      var name = cldrFieldOf(token.letter, token.count, calendar).name;
      listPush(names, name);
      listPush(texts, '{' + name + '}');
    }
  }
  // 2. Let split be the index of the first field whose placeholder came
  //    before it. If there is none, return undefined.
  var seen = [];
  var split = -1;
  for (var j = 0; j < names.length && split === -1; j++) {
    if (names[j] === undefined) continue;
    if (listContains(seen, names[j])) split = j;
    else listPush(seen, names[j]);
  }
  if (split === -1) return undefined;
  // 3. Let overlap be the placeholders in both halves.
  var overlap = [];
  for (var k = split; k < names.length; k++) {
    if (names[k] !== undefined && listContains(seen, names[k]))
      listPush(overlap, names[k]);
  }
  // 4. Append each half's parts, and return them.
  var parts = [];
  pushIntervalHalf(parts, names, texts, 0, split, overlap, 'startRange');
  pushIntervalHalf(
    parts,
    names,
    texts,
    split,
    names.length,
    overlap,
    'endRange',
  );
  return parts;
}

/// Append to parts the tokens from index from to index to of an interval
/// pattern (intervalRangeParts): from its first to its last overlap field
/// with source, the rest shared.
function pushIntervalHalf(parts, names, texts, from, to, overlap, source) {
  // 1. Find the first and the last overlap field of the half.
  var first = -1;
  var last = -1;
  for (var i = from; i < to; i++) {
    if (names[i] !== undefined && listContains(overlap, names[i])) {
      if (first === -1) first = i;
      last = i;
    }
  }
  // 2. Append each token with its source.
  for (var j = from; j < to; j++) {
    var inside = first !== -1 && j >= first && j <= last;
    pushRangePart(parts, inside ? source : 'shared', texts[j]);
  }
}

/// Whether parts, the parts of an interval pattern (intervalRangeParts),
/// show an era in a startRange part and in an endRange part.
function halvesShowEra(parts) {
  // 1. Look for {era} in a part of each source.
  var start = false;
  var end = false;
  for (var i = 0; i < parts.length; i++) {
    if (!hasPlaceholder(parts[i].Pattern, 'era')) continue;
    if (parts[i].Source === 'startRange') start = true;
    else if (parts[i].Source === 'endRange') end = true;
  }
  // 2. Return whether both have one.
  return start && end;
}

/// Whether width, a field's value in a DateTime Format Record, is numeric
/// ("numeric" or "2-digit") rather than text.
function isNumericWidth(width) {
  // 1. Return whether width is "numeric" or "2-digit".
  return width === 'numeric' || width === '2-digit';
}

/// The fields pattern, a CLDR pattern in calendar, shows in more than one
/// width, as a null-prototype object from each such field to true.
function mixedWidthFieldsOf(pattern, calendar) {
  // 1. Record each field's first width, and mark a field whose later
  //    occurrence has another.
  var widths = {__proto__: null};
  var mixed = {__proto__: null};
  var tokens = cldrPatternTokens(pattern);
  for (var i = 0; i < tokens.length; i++) {
    if (tokens[i].letter === undefined) continue;
    var f = cldrFieldOf(tokens[i].letter, tokens[i].count, calendar);
    if (f.field === undefined) continue;
    if (widths[f.field] === undefined) widths[f.field] = f.width;
    else if (widths[f.field] !== f.width) mixed[f.field] = true;
  }
  // 2. Return the marked fields.
  return mixed;
}

/// The number of CLDR letters record's pattern writes the field name
/// with, as best fit adjusted it (patternLetterCounts), which ICU4C's
/// interval width adjustment compares with the interval skeleton's
/// (intervalRangeFormat); Node behaves as if its interval skeleton came
/// from the formatter's pattern (ja's yMMMd "y年M月d日" takes the yMd
/// interval "y/MM/dd～y/MM/dd", "2026/01/05～2026/01/06"). For a record
/// without counts, the count of its width (WIDTH_LETTER_COUNTS).
function patternLetterCount(record, name) {
  // 1. Return the pattern's count if record has one, else its width's.
  var counts = record.patternLetterCounts;
  if (counts !== undefined && counts[name] !== undefined) return counts[name];
  return WIDTH_LETTER_COUNTS[record[name]];
}

/// A DateTime Range Pattern Format Record with the fields and contexts of
/// source, a DateTime Format Record, and parts as its [[PatternParts]].
function rangeFormatRecord(source, parts) {
  // 1. Let record be a new Record with source's fields and contexts.
  var record = {__proto__: null};
  for (var i = 0; i < DATE_TIME_COMPONENTS.length; i++) {
    var name = DATE_TIME_COMPONENTS[i];
    if (source[name] !== undefined) record[name] = source[name];
  }
  record.monthStandalone = source.monthStandalone;
  record.weekdayStandalone = source.weekdayStandalone;
  record.ampmWidth = source.ampmWidth;
  record.dayPeriodWidth = source.dayPeriodWidth;
  // 2. Set record.[[PatternParts]] to parts, and return record.
  record.PatternParts = parts;
  return record;
}

/// Whether records a and b have the same fields, widths aside (so z, O and
/// v are all timeZoneName).
function haveSameFields(a, b) {
  // 1. Compare the presence of each field.
  for (var i = 0; i < DATE_TIME_COMPONENTS.length; i++) {
    var name = DATE_TIME_COMPONENTS[i];
    if ((a[name] === undefined) !== (b[name] === undefined)) return false;
  }
  return true;
}

/// Decision 11: the skeleton of data's intervalFormats entries for part, a
/// DateTime Format Record, or undefined: of the skeletons with the same
/// fields as part and an hour of part's family, the one whose widths
/// part's match best by BasicFormatMatcher's penalties, the first on ties.
/// It scores as best fit does (basicFormatScore with bestFit true, a
/// refinement of decisions 6 and 11): a numeric field shown as text or the
/// other way costs KIND_MISMATCH_PENALTY more, since the entry's patterns
/// get part's widths and a text-month skeleton for a numeric month would
/// put numbers into a text layout ("MMM d – d, y"); and a timeZoneName of
/// another kind is a small width difference, so a z record uses CLDR's v
/// intervals. For a part without a skeleton (a style record, or a best-fit
/// withDayPeriod with appended fields) whose pattern shows a B it has no
/// [[dayPeriod]] field for, the B counts as a dayPeriod field, so its
/// intervals are CLDR's for the B (zh-TW timeStyle "short" "Bh:mm" takes
/// Bhm's "上午9:07–10:07", not hm's "上午9:07至10:07"; a refinement of
/// decision 11); if no skeleton has those fields (no CLDR skeleton has a
/// B and a zone), its own fields choose.
function intervalSkeletonOf(data, part) {
  // 1. If part has no skeleton and shows an unrequested B, try its fields
  //    with the B's dayPeriod.
  if (
    part.skeletonWidths === undefined &&
    part.dayPeriod === undefined &&
    part.dayPeriodWidth !== undefined &&
    hasPlaceholder(part.pattern, 'dayPeriod')
  ) {
    var withDayPeriod = requestOf(part);
    withDayPeriod.dayPeriod = part.dayPeriodWidth;
    withDayPeriod.hourLetter = part.hourLetter;
    var skeleton = intervalSkeletonOfFields(data, withDayPeriod);
    if (skeleton !== undefined) return skeleton;
  }
  // 2. Else choose by part's own fields.
  return intervalSkeletonOfFields(data, part);
}

/// intervalSkeletonOf's choice for fields, a DateTime Format Record or a
/// request with an hourLetter.
function intervalSkeletonOfFields(data, part) {
  // 1. Score each entry whose skeleton qualifies.
  var best = undefined;
  var bestScore = -Infinity;
  var family = hourLetterFamily(part.hourLetter);
  for (var i = 0; i < data.intervals.length; i++) {
    var entry = data.intervals[i];
    if (
      !haveSameFields(entry.fields, part) ||
      hourLetterFamily(entry.fields.hourLetter) !== family
    )
      continue;
    var score = basicFormatScore(part, entry.fields, true);
    if (score > bestScore) {
      best = entry.skeleton;
      bestScore = score;
    }
  }
  // 2. Return the best skeleton.
  return best;
}

/// Decision 11: the range record of part for rangeField, from data's
/// intervalFormats entry for skeleton; undefined if there is none, if no
/// field repeats in it, or if it does not show the same year placeholders
/// as part (en chinese "U – U" would drop the related years of "r(U)").
/// Its widths follow ICU4C's interval width adjustment (a refinement of
/// decision 11, which gave the patterns part's widths). A field both show
/// takes part's width only when the letter count the format's pattern
/// writes it with (patternLetterCount of countsRecord, which is part
/// unless the caller passes the format's record) differs from the
/// interval skeleton's, the interval pattern writes it with the
/// skeleton's count, and both widths are of the same kind (numeric or
/// 2-digit, or text); else it keeps the interval pattern's own width. So
/// de's yMd "dd.–dd.MM.y" stays "05.–06.01.2026" for "d.M.y", and he's
/// MMMEd "EEEE, d בMMM – …" keeps EEEE for "E, d בMMM", as ICU4C gives;
/// en's MMMd "MMM d – d" takes MMMM for "MMMM d"; and ko's yMMMd
/// "y년 M월 d일~d일", whose numeric month is written for its literal 월,
/// stays numeric for "MMM" (which showed "1월월"; a refinement of
/// decision 11). timeZoneName and fractionalSecondDigits keep part's; a
/// field the interval pattern shows in two widths, which has no single
/// width of its own, takes part's if the kinds agree (fi's MMMM
/// "LLL–LLLL"; ICU4C gives "tammikuu–helmikuu"), else keeps its first
/// (fi's yM "LLL–LLLL y" for "L.y", "tammi–helmi 2026", CLDR's own
/// interval). A field the interval pattern shows that part lacks (hebrew
/// "y – y G" for "y", de "h–h a" for "h 'Uhr' B") takes the interval
/// pattern's own width, stand-alone flag or AM/PM or day period width,
/// as in ICU4C. An interval for an era difference ([[Era]]) is not used
/// either unless both its halves show the era (a refinement of decision
/// 11): CLDR 48.2's ko GyMMMd and GyMd intervals for G, "G y년 M월 d일 ~
/// y년 M월 d일", lack the second era, which ICU4C shows ("헤이세이 31년 4월
/// 30일 ~ 1년 5월 1일"); FB(P) gives "헤이세이 31년 4월 30일 ~ 레이와 1년 5월
/// 1일".
function intervalRangeFormat(
  data,
  skeleton,
  rangeField,
  part,
  countsRecord = part,
) {
  // 1. Find the entry for skeleton whose greatest difference is rangeField.
  if (skeleton === undefined) return undefined;
  for (var i = 0; i < data.intervals.length; i++) {
    var entry = data.intervals[i];
    if (
      entry.skeleton !== skeleton ||
      RANGE_FIELD_OF_LETTER[entry.letter] !== rangeField
    )
      continue;
    // 2. Convert it, and check it: its year placeholders, and, for an era
    //    difference, the era of each half.
    var parts = intervalRangeParts(entry.pattern, data.calendar);
    if (parts === undefined) return undefined;
    var shown = '';
    for (var j = 0; j < parts.length; j++) shown += parts[j].Pattern;
    if (yearPlaceholdersOf(shown) !== yearPlaceholdersOf(part.pattern))
      return undefined;
    if (rangeField === 'Era' && !halvesShowEra(parts)) return undefined;
    // 3. Let record have the widths the adjustment above gives each
    //    field, and the interval pattern's for the fields part lacks;
    //    return it. The pattern is read with its skeleton, so a B the
    //    skeleton does not ask for sets only the day period width, as in
    //    cldrPatternToRecord (decision 6).
    var record = rangeFormatRecord(part, parts);
    var own = cldrPatternToRecord(
      entry.pattern,
      data.calendar,
      entry.skeleton,
    );
    var mixed = mixedWidthFieldsOf(entry.pattern, data.calendar);
    var skeletonCounts = own.skeletonWidths.letterCounts;
    for (var k = 0; k < DATE_TIME_COMPONENTS.length; k++) {
      var name = DATE_TIME_COMPONENTS[k];
      if (own[name] === undefined) continue;
      if (
        record[name] !== undefined &&
        (name === 'timeZoneName' ||
          name === 'fractionalSecondDigits' ||
          ((mixed[name] === true ||
            (patternLetterCount(countsRecord, name) !== skeletonCounts[name] &&
              own.patternLetterCounts[name] === skeletonCounts[name])) &&
            isNumericWidth(record[name]) === isNumericWidth(own[name])))
      )
        continue;
      record[name] = own[name];
      if (name === 'month') record.monthStandalone = own.monthStandalone;
      else if (name === 'weekday')
        record.weekdayStandalone = own.weekdayStandalone;
    }
    if (record.ampmWidth === undefined) record.ampmWidth = own.ampmWidth;
    if (record.dayPeriodWidth === undefined)
      record.dayPeriodWidth = own.dayPeriodWidth;
    return record;
  }
  return undefined;
}

/// FB of the best-fit pattern for record's fields plus each field of
/// names, a List of "era", "year", "month" and "day", the era "short" and
/// the others numeric (decision 11's widened ranges: addDateRangeFields,
/// addWidenedDateTimeRangeFields and rangePatternsOf), with that best-fit
/// record's fields. The request is new, so record, which may be a cached
/// match (cachedFormatMatch), is not changed.
function widenedFallbackRange(record, names, data, family) {
  // 1. Let request be record's fields, the era of names "short" and each
  //    other field of names numeric.
  var request = requestOf(record);
  for (var i = 0; i < names.length; i++)
    request[names[i]] = names[i] === 'era' ? 'short' : 'numeric';
  // 2. Return FB of its best-fit pattern.
  var best = BestFitFormatMatcher(
    request,
    data,
    family,
    keepsDayPeriod(record),
  );
  return rangeFormatRecord(best, fallbackRangeParts(data, best.pattern));
}

/// Whether record R, in calendar, needs an [[Era]] range pattern that
/// shows the eras (a refinement of decision 11; addDateRangeFields,
/// addWidenedDateTimeRangeFields and rangePatternsOf): R's pattern shows
/// no era, and calendar has eras (not chinese, dangi or iso8601,
/// calendarHasNoEras). An R that shows an era has its patterns from
/// decision 11's other rules.
function needsRangeEra(record, calendar) {
  // 1. Return whether R shows no era and calendar has eras.
  return (
    !hasPlaceholder(record.pattern, 'era') && !calendarHasNoEras(calendar)
  );
}

/// Whether range, a DateTime Range Pattern Format Record, shows an era.
function rangeShowsEra(range) {
  // 1. Return whether a part of range's [[PatternParts]] has {era}.
  var parts = range.PatternParts;
  for (var i = 0; i < parts.length; i++) {
    if (hasPlaceholder(parts[i].Pattern, 'era')) return true;
  }
  return false;
}

/// Decision 11's [[Era]] to [[Day]] of record R, with date fields and no
/// time fields, added to result: CLDR's pattern for G, y, M or d; a field
/// R shows without its own pattern takes the nearest larger shown field's,
/// else fallback, FB(P). A weekday counts as a day for [[Day]] (a
/// refinement of decision 11), so an R that shows a weekday has a [[Day]]
/// and the loop stops at the absent [[AmPm]]: en {weekday: "long"} an hour
/// apart is "Monday", as ICU4C gives, not [[Default]]'s "Monday – Monday".
/// [[Year]] of an R with a month or day but no year is FB of the best fit
/// for R plus year: "numeric" (widenedFallbackRange with « "year" », and
/// "month" too for a day without a month); and, a refinement of decision
/// 11, [[Month]] of an R with a day but no month is, likewise, FB of the
/// best fit for R plus month: "numeric" (en {day} gives "1/5 – 2/9"), so
/// that the loop does not stop at [[Month]] and hide a month difference.
/// [[Era]] of an R that shows no era, in a calendar with eras
/// (needsRangeEra), is, a refinement of decision 11, FB of the best fit
/// for R plus era: "short" and the fields [[Year]]'s widening adds, if R
/// has a [[Year]]: the loop checks [[Era]] first, and without it a range
/// across two eras took [[Default]], which shows no era either (en {year}
/// from 1 BC to 1 AD gave "1 – 1"; it is "1 BC – 1 AD", and {day} "6/1/1
/// BC – 6/1/1 AD", as ICU4C gives). It is set after the loop, so no later
/// field takes it as the nearest, and a range within an era is as before.
/// An R without a [[Year]] (a weekday alone) has no [[Era]]: {weekday}
/// across two eras stays "Thursday – Friday", as in ICU4C. Other fields R
/// does not show are absent.
function addDateRangeFields(result, record, data, family, fallback) {
  // 1. Find the skeleton of R's intervals.
  var pattern = record.pattern;
  var skeleton = intervalSkeletonOf(data, record);
  var shownMonth = hasPlaceholder(pattern, 'month');
  var shownDay = hasPlaceholder(pattern, 'day');
  var nearest = undefined;
  // 2. For each field from [[Era]] to [[Day]], set its pattern.
  for (var i = 0; i < RANGE_DATE_FIELDS.length; i++) {
    var rangeField = RANGE_DATE_FIELDS[i];
    var shown =
      rangeField === 'Year'
        ? yearPlaceholdersOf(pattern) !== ''
        : rangeField === 'Day'
        ? shownDay || hasPlaceholder(pattern, 'weekday')
        : hasPlaceholder(pattern, RANGE_DATE_PLACEHOLDERS[i]);
    var entry = undefined;
    if (shown) {
      entry = intervalRangeFormat(data, skeleton, rangeField, record);
      if (entry === undefined)
        entry = nearest !== undefined ? nearest : fallback;
      nearest = entry;
    } else if (rangeField === 'Year' && (shownMonth || shownDay)) {
      // A year added to a record with a day but no month brings a numeric
      // month too, as [[Month]]'s does (a refinement of decision 11): best
      // fit would otherwise add a text month, en-GB {day} giving "5 Jan
      // 2026 – 9 Feb 2027" where ICU4C gives "05/01/2026 – 09/02/2027".
      var names = ['year'];
      if (shownDay && !shownMonth) listPush(names, 'month');
      entry = widenedFallbackRange(record, names, data, family);
    } else if (rangeField === 'Month' && shownDay) {
      entry = widenedFallbackRange(record, ['month'], data, family);
    }
    if (entry !== undefined) result[rangeField] = entry;
  }
  // 3. If R needs an [[Era]] of its own and has a [[Year]], [[Era]] is FB
  //    of the best fit for R plus era: "short" and the fields [[Year]]'s
  //    widening adds. It is set after the loop, so it is never nearest.
  if (needsRangeEra(record, data.calendar) && result.Year !== undefined) {
    var eraNames = ['era'];
    if (yearPlaceholdersOf(pattern) === '') {
      listPush(eraNames, 'year');
      if (shownDay && !shownMonth) listPush(eraNames, 'month');
    }
    result.Era = widenedFallbackRange(record, eraNames, data, family);
  }
}

/// Decision 11's [[Era]] to [[Day]] of record R, with date and time
/// fields, added to result: FB(P), fallback, except that each of [[Year]],
/// [[Month]] and [[Day]] whose field R lacks widens R: FB of the best fit
/// for R plus that field and each smaller one of year, month and day R
/// lacks, numeric (refinements of decision 11, as ICU4C adds them: FB(P)
/// would hide the difference, "Jan 5, 9 AM – Jan 5, 9 AM" a year apart,
/// where ICU4C gives "Jan 5, 2026, 9 AM – Jan 5, 2027, 9 AM"; en {weekday,
/// hour} is "5 Monday, 9 AM – 6 Tuesday, 9 AM" a day apart and "Monday,
/// 1/5, 9 AM – Thursday, 2/5, 9 AM" a month apart; {year, hour} a year
/// apart stays "2026, 9 AM – 2027, 9 AM"). [[Era]] of an R that shows no
/// era, in a calendar with eras (needsRangeEra), is, a refinement of
/// decision 11, FB of the best fit for R plus era: "short" and each of
/// year, month and day R lacks, numeric, so the eras show (en {weekday:
/// "short", hour} from 1 BC to 1 AD is "Thu, 6/1/1 BC, 12 AM – Fri, 6/1/1
/// AD, 12 AM", as ICU4C gives, not "Thu 12 AM – Fri 12 AM"); otherwise
/// [[Era]] is FB(P).
function addWidenedDateTimeRangeFields(result, record, data, family, fallback) {
  // 1. Let shown be whether R shows a year, a month and a day.
  var pattern = record.pattern;
  var shown = [
    yearPlaceholdersOf(pattern) !== '',
    hasPlaceholder(pattern, 'month'),
    hasPlaceholder(pattern, 'day'),
  ];
  var names = ['year', 'month', 'day'];
  // 2. [[Era]] is FB(P), unless R needs an [[Era]] of its own: then it is
  //    FB of the best fit for R plus era: "short" and each of year, month
  //    and day R lacks, numeric.
  if (!needsRangeEra(record, data.calendar)) {
    result.Era = fallback;
  } else {
    var eraNames = ['era'];
    for (var e = 0; e < 3; e++) {
      if (!shown[e]) listPush(eraNames, names[e]);
    }
    result.Era = widenedFallbackRange(record, eraNames, data, family);
  }
  // 3. For [[Year]], [[Month]] and [[Day]], take FB(P) if R shows the
  //    field; else widen R by it and the smaller fields R lacks.
  for (var i = 0; i < 3; i++) {
    if (shown[i]) {
      result[RANGE_DATE_FIELDS[i + 1]] = fallback;
      continue;
    }
    var missing = [];
    for (var j = i; j < 3; j++) {
      if (!shown[j]) listPush(missing, names[j]);
    }
    result[RANGE_DATE_FIELDS[i + 1]] = widenedFallbackRange(
      record,
      missing,
      data,
      family,
    );
  }
}

/// Decision 11: timeRange joined to datePart for a record R with date
/// fields: the date part's pattern, shared, joined to timeRange's parts by
/// the medium standard dateTimeFormat for every R ("Jan 5, 2026, 9:07 –
/// 10:07 AM"), as ICU4C's interval formats join them (a refinement of
/// decision 11, which chose the date style's or UTS 35's connector: it
/// with long and short styles is "5 gennaio 2026, 09:07–11:07", as Node
/// gives, though its long connector is "{1} {0}"). The result has the
/// date fields and contexts of datePart and the time fields and widths of
/// timeRange, whose AM/PM, day period and hour widths may be the interval
/// pattern's own (rulings 79, 127 and 132: zh-TW {weekday: "short", hour}
/// "週一 上午9時至下午2時", zh {month, day, hour} "1/5 09–11").
/// nameSymbolsOf reads these widths for the names it asks for.
function dateWrappedRange(data, record, datePart, timeRange) {
  // 1. Let connector be the medium standard dateTimeFormat, and find {0}
  //    (the time) and {1} (the date) in it.
  var connector = data.dateTimeFormats[2];
  var i0 = stringIndexOf(connector, '{0}', 0);
  var i1 = stringIndexOf(connector, '{1}', 0);
  var first = i0 < i1 ? i0 : i1;
  var second = i0 < i1 ? i1 : i0;
  // 2. Let parts be the connector's text, shared, with the date pattern
  //    for {1} and the time parts for {0}.
  var parts = [];
  pushRangePart(parts, 'shared', stringSlice(connector, 0, first));
  pushWrappedPlaceholder(parts, first === i1, datePart, timeRange);
  pushRangePart(parts, 'shared', stringSlice(connector, first + 3, second));
  pushWrappedPlaceholder(parts, second === i1, datePart, timeRange);
  pushRangePart(parts, 'shared', stringSlice(connector, second + 3));
  // 3. Let result have timeRange's time fields and widths, and datePart's
  //    date fields and contexts; return it.
  var result = rangeFormatRecord(timeRange, parts);
  for (var i = 0; i < FIRST_TIME_COMPONENT; i++) {
    var name = DATE_TIME_COMPONENTS[i];
    result[name] = datePart[name];
  }
  result.monthStandalone = datePart.monthStandalone;
  result.weekdayStandalone = datePart.weekdayStandalone;
  return result;
}

/// Append to parts the date pattern, shared, if isDate; else the parts of
/// timeRange (dateWrappedRange).
function pushWrappedPlaceholder(parts, isDate, datePart, timeRange) {
  // 1. Append the date or the time.
  if (isDate) {
    pushRangePart(parts, 'shared', datePart.pattern);
    return;
  }
  var timeParts = timeRange.PatternParts;
  for (var i = 0; i < timeParts.length; i++)
    pushRangePart(parts, timeParts[i].Source, timeParts[i].Pattern);
}

/// Decision 11's [[AmPm]] to [[FractionalSecondDigits]] of record R, whose
/// time part is timePart (R itself when it has no date fields), added to
/// result: CLDR's patterns for a, B, h/H and m; a field the time part
/// shows without one takes FB of the time part's pattern; a field from
/// [[AmPm]] down that it does not show, but that is larger than the
/// smallest field it shows, takes the pattern of the next smaller field it
/// shows; fields smaller than the smallest one it shows are absent. If R
/// has date fields, each pattern is joined to datePart (dateWrappedRange).
function addTimeRangeFields(result, record, timePart, datePart, data) {
  // 1. Let shown be whether the time part shows each field, and smallest
  //    the index of the smallest one it shows. If none, return.
  var shown = [];
  var smallest = -1;
  for (var i = 0; i < RANGE_TIME_FIELDS.length; i++) {
    var s = hasPlaceholder(timePart.pattern, RANGE_TIME_PLACEHOLDERS[i]);
    listPush(shown, s);
    if (s) smallest = i;
  }
  if (smallest === -1) return;
  // A time part whose unrequested B shows as {ampm} (showDayPeriodAsAmPm)
  // takes CLDR's intervals as for its B, as ICU4C's interval formats do:
  // its [[AmPm]] takes the B interval, if there is one (zh-TW {hour} is
  // "上午9時至晚上8時", where an AM/PM difference takes h's B interval), and
  // its other fields the intervals of the record with the B; its
  // fallbacks show its own AM/PM. (A refinement of decisions 6 and 11.)
  var intervalPart =
    timePart.withDayPeriod !== undefined ? timePart.withDayPeriod : timePart;
  // 2. For each shown field down to the smallest, take CLDR's pattern,
  //    else FB of the time part's pattern. The interval widths compare
  //    the letter counts of R's own pattern (ruling 132): R's, when R has
  //    its own pattern with a date (an availableFormats or style record;
  //    be's {weekday, hour} "EEEE, h a" against a separately fitted time
  //    part's "hh a", where ICU4C gives "9–11 AM"), else the time part's,
  //    which is R's own time (R itself, or a glued R's part).
  var countsRecord =
    datePart !== undefined && record.patternLetterCounts !== undefined
      ? record
      : timePart;
  var skeleton = intervalSkeletonOf(data, intervalPart);
  var fallback = undefined;
  var entries = [];
  for (var j = 0; j <= smallest; j++) {
    var entry = undefined;
    if (shown[j]) {
      var field = RANGE_TIME_FIELDS[j];
      if (field === 'AmPm' && intervalPart !== timePart) {
        entry = intervalRangeFormat(
          data,
          skeleton,
          'DayPeriod',
          intervalPart,
          countsRecord,
        );
      }
      if (entry === undefined) {
        entry = intervalRangeFormat(
          data,
          skeleton,
          field,
          intervalPart,
          countsRecord,
        );
      }
      if (entry === undefined) {
        if (fallback === undefined) {
          fallback = rangeFormatRecord(
            timePart,
            fallbackRangeParts(data, timePart.pattern),
          );
        }
        entry = fallback;
      }
    }
    listPush(entries, entry);
  }
  // 3. A field not shown takes the next smaller shown field's pattern.
  for (var k = smallest - 1; k >= 0; k--) {
    if (!shown[k]) listSet(entries, k, entries[k + 1]);
  }
  // 4. Join each pattern to the date part, keeping equal patterns the same
  //    record, and set the fields.
  var previous = undefined;
  var wrapped = undefined;
  for (var n = 0; n <= smallest; n++) {
    var value = entries[n];
    if (datePart !== undefined) {
      if (value !== previous)
        wrapped = dateWrappedRange(data, record, datePart, value);
      previous = value;
      value = wrapped;
    }
    result[RANGE_TIME_FIELDS[n]] = value;
  }
}

/// Whether the best fits rangePatternsOf makes for record keep a B the
/// request did not ask for (showDayPeriodAsAmPm): they do for a record
/// that still shows one, a style record's. A record whose B shows as
/// AM/PM keeps CLDR's interval patterns, B and all, as ICU4C does (zh-TW
/// {hour} "凌晨1時至3時"); its fallbacks show its own pattern's AM/PM.
function keepsDayPeriod(record) {
  // 1. Return whether record shows a day period.
  return record.dayPeriodWidth !== undefined;
}

/// Decision 11: the DateTime Range Pattern Record of record R, a DateTime
/// Format Record of data and family, with its fields in the draft's
/// order; a field with no pattern is absent. An R with time fields only,
/// in a calendar with eras (needsRangeEra), whose [[Default]] shows no era
/// (japanese's does, "4/30/31 H, 12 AM – 5/1/1 R, 12 AM", as ICU4C
/// gives), has, a refinement of decision 11, an [[Era]] that is FB of the
/// best fit for R plus era: "short" and year, month and day numeric (en
/// {hour} from 1 BC to 1 AD is "6/1/1 BC, 12 AM – 6/1/1 AD, 12 AM", as
/// ICU4C gives, not "6/1/1, 12 AM – 6/1/1, 12 AM"), and its [[Year]],
/// [[Month]] and [[Day]] are [[Default]]'s: once [[Era]] has a pattern,
/// the draft's loop would stop at an absent [[Year]] and show two days as
/// one, where without [[Era]] it took [[Default]]. The result is complete
/// before it is kept in record, as its [[rangePatterns]] and
/// [[rangePatterns12]], which are the same, as [[pattern]] and
/// [[pattern12]] are.
function rangePatternsOf(record, data, family) {
  // 1. If record has its range patterns, return them.
  if (record.rangePatterns !== undefined) return record.rangePatterns;
  var hasDate = hasDateField(record);
  var hasTime = hasTimeField(record);
  var result = {__proto__: null};
  // 2. [[Default]]: FB(P) if R has date fields; else FB of the best-fit
  //    pattern for year, month and day numeric plus R's time fields.
  var fallback = undefined;
  if (hasDate) {
    fallback = rangeFormatRecord(
      record,
      fallbackRangeParts(data, record.pattern),
    );
    result.Default = fallback;
  } else {
    var request = requestOf(record, FIRST_TIME_COMPONENT);
    request.year = 'numeric';
    request.month = 'numeric';
    request.day = 'numeric';
    var best = BestFitFormatMatcher(
      request,
      data,
      family,
      keepsDayPeriod(record),
    );
    result.Default = rangeFormatRecord(
      best,
      fallbackRangeParts(data, best.pattern),
    );
  }
  // 3. [[Era]] to [[Day]]: by addDateRangeFields if R has date fields and
  //    no time fields; by addWidenedDateTimeRangeFields if it has both. If
  //    it has no date fields, they are absent, unless R needs an [[Era]]
  //    of its own and [[Default]] shows no era: then [[Era]] is FB of the
  //    best fit for R plus era: "short" and year, month and day numeric,
  //    and [[Year]], [[Month]] and [[Day]] are [[Default]].
  if (hasDate && !hasTime) {
    addDateRangeFields(result, record, data, family, fallback);
  } else if (hasDate) {
    addWidenedDateTimeRangeFields(result, record, data, family, fallback);
  } else if (
    needsRangeEra(record, data.calendar) &&
    !rangeShowsEra(result.Default)
  ) {
    result.Era = widenedFallbackRange(
      record,
      ['era', 'year', 'month', 'day'],
      data,
      family,
    );
    result.Year = result.Default;
    result.Month = result.Default;
    result.Day = result.Default;
  }
  // 4. [[AmPm]] down: by addTimeRangeFields, for the time part of R: R
  //    itself if it has no date fields; else the parts of a glued R, or the
  //    best fit for R's date fields and for its time fields.
  if (hasTime) {
    var datePart = undefined;
    var timePart = record;
    if (hasDate && record.timePart !== undefined) {
      datePart = record.datePart;
      timePart = record.timePart;
    } else if (hasDate) {
      datePart = BestFitFormatMatcher(
        requestOf(record, 0, FIRST_TIME_COMPONENT),
        data,
        family,
      );
      timePart = BestFitFormatMatcher(
        requestOf(record, FIRST_TIME_COMPONENT),
        data,
        family,
        keepsDayPeriod(record),
      );
    }
    addTimeRangeFields(result, record, timePart, datePart, data);
  }
  // 5. Keep the result in record, and return it.
  record.rangePatterns = result;
  record.rangePatterns12 = result;
  return result;
}
