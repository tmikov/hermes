/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// ECMA-402 "DateTimeFormat Objects": the CLDR name symbols a DateTimeFormat's
// patterns need from native code, and the flexible day periods
// FormatDateTimePattern and PartitionDateTimeRangePattern show, following
// the draft https://tc39.es/ecma402/ dated October 24, 2026, by decision 4
// of the Phase 4 plan. The records they read are made in
// 51-datetime-patterns.js and 53-datetime-ranges.js.

/// The CLDR name symbol of each width (decision 4; the symbols native code
/// accepts).
var ERA_SYMBOLS = {
  __proto__: null,
  short: 'G',
  long: 'GGGG',
  narrow: 'GGGGG',
};
var FORMAT_MONTH_SYMBOLS = {
  __proto__: null,
  short: 'MMM',
  long: 'MMMM',
  narrow: 'MMMMM',
};
var STANDALONE_MONTH_SYMBOLS = {
  __proto__: null,
  short: 'LLL',
  long: 'LLLL',
  narrow: 'LLLLL',
};
var FORMAT_WEEKDAY_SYMBOLS = {
  __proto__: null,
  short: 'EEE',
  long: 'EEEE',
  narrow: 'EEEEE',
};
var STANDALONE_WEEKDAY_SYMBOLS = {
  __proto__: null,
  short: 'ccc',
  long: 'cccc',
  narrow: 'ccccc',
};
var AMPM_SYMBOLS = {
  __proto__: null,
  abbreviated: 'a',
  wide: 'aaaa',
  narrow: 'aaaaa',
};
/// The AM/PM symbol a day period of each width needs when a period has no
/// name in that width (JS then shows the hour's AM/PM name).
var DAY_PERIOD_AMPM_SYMBOLS = {
  __proto__: null,
  short: 'a',
  long: 'aaaa',
  narrow: 'aaaaa',
};
var TIME_ZONE_NAME_SYMBOLS = {
  __proto__: null,
  short: 'z',
  long: 'zzzz',
  shortOffset: 'O',
  longOffset: 'OOOO',
  shortGeneric: 'v',
  longGeneric: 'vvvv',
};
/// The column of a dayPeriods group's names (after id, start and end) for
/// each width: CLDR's abbreviated, wide and narrow names.
var DAY_PERIOD_NAME_COLUMN = {__proto__: null, short: 0, long: 1, narrow: 2};

/// Whether calendar has no eras, so {era} gives no part and asks for no
/// name (decision 6).
function calendarHasNoEras(calendar) {
  // 1. Return true for chinese, dangi and iso8601.
  return (
    calendar === 'chinese' || calendar === 'dangi' || calendar === 'iso8601'
  );
}

/// The cache of dayPeriods results (decision 13: 64 entries, emptied when
/// full).
var dayPeriodsCache = newDataCache();

/// native.dayPeriods(languageId), through its cache.
function cachedDayPeriods(languageId) {
  // 1. If the cache has a result for languageId, return it.
  var periods = dataCacheGet(dayPeriodsCache, languageId);
  if (periods !== undefined) return periods;
  // 2. Otherwise ask native code, and cache the result.
  periods = native.dayPeriods(languageId);
  dataCachePut(dayPeriodsCache, languageId, periods);
  return periods;
}

/// Whether dayPeriods, a dayPeriods list, has the names of noon: they are
/// all "" when the rules have no noon.
function dayPeriodsHaveNoon(dayPeriods) {
  // 1. Look at noon's three names, after the periods.
  var noon = 1 + 6 * +dayPeriods[0];
  return (
    dayPeriods[noon] !== '' ||
    dayPeriods[noon + 1] !== '' ||
    dayPeriods[noon + 2] !== ''
  );
}

/// Whether a day period of width has no name in dayPeriods, a dayPeriods
/// list, for some period (noon included if the rules have it), so that JS
/// shows the hour's AM/PM name for it (decision 4).
function dayPeriodLacksName(dayPeriods, width) {
  // 1. Let column be the name column of width.
  var column = DAY_PERIOD_NAME_COLUMN[width];
  var count = +dayPeriods[0];
  // 2. Look at each period's name.
  for (var i = 0; i < count; i++) {
    if (dayPeriods[1 + 6 * i + 3 + column] === '') return true;
  }
  // 3. Look at noon's.
  return (
    dayPeriodsHaveNoon(dayPeriods) && dayPeriods[1 + 6 * count + column] === ''
  );
}

/// The CLDR name symbol of the placeholder name with the widths and
/// contexts of widths, a DateTime Format Record or a DateTime Range Pattern
/// Format Record, in calendar, or undefined if it needs none (decision 4).
function nameSymbolOf(name, widths, calendar, dayPeriods) {
  // 1. Return the symbol of name at its width, if calendar has the names.
  if (name === 'era')
    return calendarHasNoEras(calendar) ? undefined : ERA_SYMBOLS[widths.era];
  if (name === 'month') {
    var months = widths.monthStandalone
      ? STANDALONE_MONTH_SYMBOLS
      : FORMAT_MONTH_SYMBOLS;
    return months[widths.month];
  }
  if (name === 'weekday') {
    var weekdays = widths.weekdayStandalone
      ? STANDALONE_WEEKDAY_SYMBOLS
      : FORMAT_WEEKDAY_SYMBOLS;
    return weekdays[widths.weekday];
  }
  if (name === 'ampm') return AMPM_SYMBOLS[widths.ampmWidth];
  if (name === 'yearName') return calendarIsCyclic(calendar) ? 'U' : undefined;
  if (name === 'timeZoneName')
    return TIME_ZONE_NAME_SYMBOLS[widths.timeZoneName];
  // 2. A day period with no name in its width shows the AM/PM name.
  if (
    name === 'dayPeriod' &&
    dayPeriods !== undefined &&
    widths.dayPeriodWidth !== undefined &&
    dayPeriodLacksName(dayPeriods, widths.dayPeriodWidth)
  )
    return DAY_PERIOD_AMPM_SYMBOLS[widths.dayPeriodWidth];
  return undefined;
}

/// Append to symbols, without repeats, the name symbols of the
/// placeholders of pattern (nameSymbolOf).
function addNameSymbols(symbols, widths, pattern, calendar, dayPeriods) {
  // 1. For each placeholder of pattern, in order, append its symbol if it
  //    has one that symbols lacks.
  var start = stringIndexOf(pattern, '{', 0);
  while (start !== -1) {
    var end = stringIndexOf(pattern, '}', start);
    if (end === -1) break;
    var name = stringSlice(pattern, start + 1, end);
    var symbol = nameSymbolOf(name, widths, calendar, dayPeriods);
    if (symbol !== undefined && !listContains(symbols, symbol))
      listPush(symbols, symbol);
    start = stringIndexOf(pattern, '{', end);
  }
}

/// Decision 4: the CLDR name symbols that record and rangePatterns (its
/// DateTime Range Pattern Record, or undefined) need, space separated, in
/// the order they first appear, with no repeats; never one calendar does
/// not have (U only for chinese and dangi; G not for chinese, dangi and
/// iso8601). A {dayPeriod} whose width has a period with no name in
/// dayPeriods (a dayPeriods list) needs the AM/PM symbol of that width,
/// which JS shows instead.
function nameSymbolsOf(record, rangePatterns, calendar, dayPeriods) {
  // 1. Let symbols be the symbols of record's pattern.
  var symbols = [];
  addNameSymbols(symbols, record, record.pattern, calendar, dayPeriods);
  // 2. Add those of each range pattern, in the draft's field order.
  if (rangePatterns !== undefined) {
    for (var i = 0; i < RANGE_FIELDS.length; i++) {
      var range = rangePatterns[RANGE_FIELDS[i]];
      if (range === undefined) continue;
      for (var j = 0; j < range.PatternParts.length; j++) {
        addNameSymbols(
          symbols,
          range,
          range.PatternParts[j].Pattern,
          calendar,
          dayPeriods,
        );
      }
    }
  }
  // 3. Return them joined by spaces.
  var result = '';
  for (var k = 0; k < symbols.length; k++)
    result += (k === 0 ? '' : ' ') + symbols[k];
  return result;
}

/// Decision 4: the id of the flexible day period of a local time in
/// dayPeriods, a dayPeriods list: "noon" only when the hour is 12, the
/// rules have noon, and the formatter shows no non-zero minute or second
/// (showsMinute, showsSecond); else the period whose range holds the
/// minute of the day. "midnight" is never chosen, as ICU4C.
function dayPeriodOf(
  dayPeriods,
  hour,
  minute,
  second,
  showsMinute,
  showsSecond,
) {
  // 1. If the hour is 12, the rules have noon, and no non-zero minute or
  //    second is shown, return "noon".
  if (
    hour === 12 &&
    dayPeriodsHaveNoon(dayPeriods) &&
    !(showsMinute && minute !== 0) &&
    !(showsSecond && second !== 0)
  )
    return 'noon';
  // 2. Return the period whose range holds the minute of the day; a range
  //    whose start is after its end wraps midnight.
  var minuteOfDay = hour * 60 + minute;
  var count = +dayPeriods[0];
  for (var i = 0; i < count; i++) {
    var start = +dayPeriods[1 + 6 * i + 1];
    var end = +dayPeriods[1 + 6 * i + 2];
    var holds =
      start <= end
        ? minuteOfDay >= start && minuteOfDay < end
        : minuteOfDay >= start || minuteOfDay < end;
    if (holds) return dayPeriods[1 + 6 * i];
  }
  // 3. The rules cover the day (Task 3's Rust tests check it), so this is
  //    an internal error.
  throw new _TypeError('Intl internal error: no day period for ' + hour);
}

/// Decision 4: the name of the day period id at width ("short", "long" or
/// "narrow": CLDR's abbreviated, wide or narrow) in dayPeriods, a
/// dayPeriods list; "" where CLDR has none.
function dayPeriodName(dayPeriods, id, width) {
  // 1. Let column be the name column of width.
  var column = DAY_PERIOD_NAME_COLUMN[width];
  var count = +dayPeriods[0];
  // 2. Noon's names follow the periods.
  if (id === 'noon') return dayPeriods[1 + 6 * count + column];
  // 3. Otherwise return the name of the period id.
  for (var i = 0; i < count; i++) {
    if (dayPeriods[1 + 6 * i] === id)
      return dayPeriods[1 + 6 * i + 3 + column];
  }
  throw new _TypeError('Intl internal error: no day period ' + id);
}
