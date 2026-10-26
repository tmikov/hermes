/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The locales and the formatMatcher "basic" check that
// datetime-format-locales*.js share: their RUN lines put this file in front
// of the test. The check is split over four tests, a quarter of the
// locales each, so that lit runs the parts in parallel.

/// 40 locales across scripts and regions.
var LOCALES = [
  'en', 'en-GB', 'de', 'fr', 'es', 'pt-BR', 'it', 'nl', 'pl', 'ru', 'uk',
  'cs', 'tr', 'el', 'he', 'ar', 'ar-EG', 'fa', 'hi', 'bn', 'ta', 'th', 'vi',
  'id', 'zh', 'zh-Hant', 'zh-HK', 'ja', 'ko', 'sw', 'am', 'my', 'km', 'ka',
  'hy', 'kk', 'mn', 'sr-Latn', 'bal-Latn', 'ff-Adlm',
];
/// The draft's 14 required field sets ("Internal slots").
var SETS = [
  ['weekday', 'year', 'month', 'day', 'hour', 'minute', 'second', 'fractionalSecondDigits'],
  ['weekday', 'year', 'month', 'day', 'hour', 'minute', 'second'],
  ['weekday', 'year', 'month', 'day'],
  ['year', 'month', 'day'],
  ['year', 'month'],
  ['month', 'day'],
  ['month'],
  ['hour', 'minute', 'second', 'fractionalSecondDigits'],
  ['hour', 'minute', 'second'],
  ['hour', 'minute'],
  ['dayPeriod', 'hour', 'minute', 'second', 'fractionalSecondDigits'],
  ['dayPeriod', 'hour', 'minute', 'second'],
  ['dayPeriod', 'hour', 'minute'],
  ['dayPeriod', 'hour'],
];
var FIELDS = [
  'weekday', 'era', 'year', 'month', 'day', 'dayPeriod', 'hour', 'minute',
  'second', 'fractionalSecondDigits', 'timeZoneName',
];
var NO_ERAS = ['chinese', 'dangi', 'iso8601'];

/// For LOCALES[from] to LOCALES[to - 1] and every supported calendar, each
/// field set with formatMatcher "basic": numeric fields, weekday "short",
/// dayPeriod "short", fractionalSecondDigits 3, and hour12 true for the
/// sets with dayPeriod. resolvedOptions must have every field of the set
/// and no other, except an era in a calendar with eras. Prints the number
/// of cases and "ok", or the problems, and whether some case added an era.
function checkBasicSets(from, to) {
  var calendars = Intl.supportedValuesOf('calendar');
  var cases = 0;
  var withEra = 0;
  var problems = [];
  for (var l = from; l < to; l++) {
    for (var c = 0; c < calendars.length; c++) {
      for (var k = 0; k < SETS.length; k++) {
        var set = SETS[k];
        var options = {formatMatcher: 'basic', calendar: calendars[c]};
        for (var f = 0; f < set.length; f++) {
          var name = set[f];
          options[name] =
            name === 'weekday' || name === 'dayPeriod'
              ? 'short'
              : name === 'fractionalSecondDigits'
              ? 3
              : 'numeric';
        }
        if (options.dayPeriod !== undefined) options.hour12 = true;
        cases++;
        var label = LOCALES[l] + ' ' + calendars[c] + ' ' + set.join();
        try {
          var resolved = new Intl.DateTimeFormat(LOCALES[l], options).resolvedOptions();
          for (var g = 0; g < FIELDS.length; g++) {
            var field = FIELDS[g];
            var wanted = set.indexOf(field) !== -1;
            var has = resolved[field] !== undefined;
            if (wanted && !has) problems.push(label + ': no ' + field);
            if (!wanted && has) {
              if (field === 'era' && NO_ERAS.indexOf(calendars[c]) === -1) withEra++;
              else problems.push(label + ': extra ' + field);
            }
          }
        } catch (e) {
          problems.push(label + ': ' + e);
        }
      }
    }
  }
  print(cases, 'basic cases:', problems.length === 0 ? 'ok' : problems.join('; '), withEra > 0);
}
