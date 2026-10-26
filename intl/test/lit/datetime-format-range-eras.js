/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: cat %S/Inputs/escape.js %s > %t.js
// RUN: %hermes -Xintl-default-locale=en-US -Xintl-default-time-zone=UTC %t.js | %FileCheck --match-full-lines %s

// Intl.DateTimeFormat's formatRange and formatRangeToParts across two
// eras: decision 11's [[Era]] for a record R that shows no era, in a
// calendar with eras (a refinement of decision 11). The draft's loop
// checks [[Era]] first; with no [[Era]] pattern it took [[Default]], and
// with FB(P) the pattern itself, neither of which shows an era ("1 – 1"
// for {year} from 1 BC to 1 AD). [[Era]] is now FB of the best fit for R
// plus era: "short" and the year, month and day it needs, so both ends
// show their era. A time-only R whose [[Default]] already shows the era
// (japanese's "M/d/y GGGGG") keeps it, and a weekday-only R has no
// [[Year]] and no [[Era]], and keeps its two weekdays. The strings are
// Node 24's (ICU 78.2) but for {month: "long", day}, where ICU4C shows
// no era at all ("June 1 – June 1"), and, in japanese, en {weekday:
// "short", hour}, where ICU4C keeps the narrow era of its GyMEd pattern
// ("Tue, 4/30/31 H, 12 AM"), and de {day}, where it writes "Heisei
// 31-04-30". Each line is the range, then its parts as
// type:value:source; for the ranges across two eras, the line also says
// whether both ends have an era part ("eras"), or no end has one and
// both have a weekday ("no eras").

var BC = Date.parse('0000-06-01T00:00:00Z');
var AD = Date.parse('0001-06-01T00:00:00Z');
var HEISEI = Date.parse('2019-04-30T00:00:00Z');
var REIWA = Date.parse('2019-05-01T00:00:00Z');

/// The parts of formatRangeToParts(x, y) of f as type:value:source joined
/// by "|", escaped.
function partsOf(f, x, y) {
  var p = f.formatRangeToParts(x, y);
  return escape(p.map(q => q.type + ':' + q.value + ':' + q.source).join('|'));
}

/// "eras" if the range parts of f have an era part from each end, "no
/// eras" if they have none and a weekday from each end, else "BAD".
function erasOf(f, x, y) {
  var p = f.formatRangeToParts(x, y);
  function has(type, source) {
    return p.some(q => q.type === type && q.source === source);
  }
  if (has('era', 'startRange') && has('era', 'endRange')) return 'eras';
  if (
    !p.some(q => q.type === 'era') &&
    has('weekday', 'startRange') &&
    has('weekday', 'endRange')
  )
    return 'no eras';
  return 'BAD';
}

/// Print the range of locales and bag, with the time zone UTC, from x to
/// y, its parts and, if checkEras, erasOf, from a formatter made twice
/// (the second from the cached record), with "cache differs" if the two
/// disagree.
function show(locales, bag, x, y, checkEras = false) {
  var options = Object.assign({timeZone: 'UTC'}, bag);
  var f = new Intl.DateTimeFormat(locales, options);
  var g = new Intl.DateTimeFormat(locales, options);
  var same =
    f.formatRange(x, y) === g.formatRange(x, y) &&
    partsOf(f, x, y) === partsOf(g, x, y);
  print(
    locales + ' ' + JSON.stringify(bag) + ': ' +
      escape(f.formatRange(x, y)) +
      (checkEras ? ' [' + erasOf(f, x, y) + ']' : '') +
      (same ? '' : ' cache differs'),
  );
  print(partsOf(f, x, y));
}

var bags = [
  {year: 'numeric'},
  {month: 'long', day: 'numeric'},
  {day: 'numeric'},
  {weekday: 'long'},
  {hour: 'numeric'},
  {weekday: 'short', hour: 'numeric'},
  {dateStyle: 'long'},
  {dateStyle: 'full', timeStyle: 'short'},
];
var cases = [
  ['en', BC, AD],
  ['de', BC, AD],
  ['en-u-ca-japanese', HEISEI, REIWA],
  ['de-u-ca-japanese', HEISEI, REIWA],
];
for (var ci = 0; ci < cases.length; ci++) {
  for (var bi = 0; bi < bags.length; bi++) {
    show(cases[ci][0], bags[bi], cases[ci][1], cases[ci][2], true);
  }
}
// CHECK: en {"year":"numeric"}: 1 BC\u2009–\u20091 AD [eras]
// CHECK-NEXT: year:1:startRange|literal: :startRange|era:BC:startRange|literal:\u2009–\u2009:shared|year:1:endRange|literal: :endRange|era:AD:endRange
// CHECK-NEXT: en {"month":"long","day":"numeric"}: June 1, 1 BC\u2009–\u2009June 1, 1 AD [eras]
// CHECK-NEXT: month:June:startRange|literal: :startRange|day:1:startRange|literal:, :startRange|year:1:startRange|literal: :startRange|era:BC:startRange|literal:\u2009–\u2009:shared|month:June:endRange|literal: :endRange|day:1:endRange|literal:, :endRange|year:1:endRange|literal: :endRange|era:AD:endRange
// CHECK-NEXT: en {"day":"numeric"}: 6/1/1 BC\u2009–\u20096/1/1 AD [eras]
// CHECK-NEXT: month:6:startRange|literal:/:startRange|day:1:startRange|literal:/:startRange|year:1:startRange|literal: :startRange|era:BC:startRange|literal:\u2009–\u2009:shared|month:6:endRange|literal:/:endRange|day:1:endRange|literal:/:endRange|year:1:endRange|literal: :endRange|era:AD:endRange
// CHECK-NEXT: en {"weekday":"long"}: Thursday\u2009–\u2009Friday [no eras]
// CHECK-NEXT: weekday:Thursday:startRange|literal:\u2009–\u2009:shared|weekday:Friday:endRange
// CHECK-NEXT: en {"hour":"numeric"}: 6/1/1 BC, 12\u202fAM\u2009–\u20096/1/1 AD, 12\u202fAM [eras]
// CHECK-NEXT: month:6:startRange|literal:/:startRange|day:1:startRange|literal:/:startRange|year:1:startRange|literal: :startRange|era:BC:startRange|literal:, :startRange|hour:12:startRange|literal:\u202f:startRange|dayPeriod:AM:startRange|literal:\u2009–\u2009:shared|month:6:endRange|literal:/:endRange|day:1:endRange|literal:/:endRange|year:1:endRange|literal: :endRange|era:AD:endRange|literal:, :endRange|hour:12:endRange|literal:\u202f:endRange|dayPeriod:AM:endRange
// CHECK-NEXT: en {"weekday":"short","hour":"numeric"}: Thu, 6/1/1 BC, 12\u202fAM\u2009–\u2009Fri, 6/1/1 AD, 12\u202fAM [eras]
// CHECK-NEXT: weekday:Thu:startRange|literal:, :startRange|month:6:startRange|literal:/:startRange|day:1:startRange|literal:/:startRange|year:1:startRange|literal: :startRange|era:BC:startRange|literal:, :startRange|hour:12:startRange|literal:\u202f:startRange|dayPeriod:AM:startRange|literal:\u2009–\u2009:shared|weekday:Fri:endRange|literal:, :endRange|month:6:endRange|literal:/:endRange|day:1:endRange|literal:/:endRange|year:1:endRange|literal: :endRange|era:AD:endRange|literal:, :endRange|hour:12:endRange|literal:\u202f:endRange|dayPeriod:AM:endRange
// CHECK-NEXT: en {"dateStyle":"long"}: June 1, 1 BC\u2009–\u2009June 1, 1 AD [eras]
// CHECK-NEXT: month:June:startRange|literal: :startRange|day:1:startRange|literal:, :startRange|year:1:startRange|literal: :startRange|era:BC:startRange|literal:\u2009–\u2009:shared|month:June:endRange|literal: :endRange|day:1:endRange|literal:, :endRange|year:1:endRange|literal: :endRange|era:AD:endRange
// CHECK-NEXT: en {"dateStyle":"full","timeStyle":"short"}: Thursday, June 1, 1 BC at 12:00\u202fAM\u2009–\u2009Friday, June 1, 1 AD at 12:00\u202fAM [eras]
// CHECK-NEXT: weekday:Thursday:startRange|literal:, :startRange|month:June:startRange|literal: :startRange|day:1:startRange|literal:, :startRange|year:1:startRange|literal: :startRange|era:BC:startRange|literal: at :startRange|hour:12:startRange|literal:::startRange|minute:00:startRange|literal:\u202f:startRange|dayPeriod:AM:startRange|literal:\u2009–\u2009:shared|weekday:Friday:endRange|literal:, :endRange|month:June:endRange|literal: :endRange|day:1:endRange|literal:, :endRange|year:1:endRange|literal: :endRange|era:AD:endRange|literal: at :endRange|hour:12:endRange|literal:::endRange|minute:00:endRange|literal:\u202f:endRange|dayPeriod:AM:endRange
// CHECK-NEXT: de {"year":"numeric"}: 1 v. Chr.\u2009–\u20091 n. Chr. [eras]
// CHECK-NEXT: year:1:startRange|literal: :startRange|era:v. Chr.:startRange|literal:\u2009–\u2009:shared|year:1:endRange|literal: :endRange|era:n. Chr.:endRange
// CHECK-NEXT: de {"month":"long","day":"numeric"}: 1. Juni 1 v. Chr.\u2009–\u20091. Juni 1 n. Chr. [eras]
// CHECK-NEXT: day:1:startRange|literal:. :startRange|month:Juni:startRange|literal: :startRange|year:1:startRange|literal: :startRange|era:v. Chr.:startRange|literal:\u2009–\u2009:shared|day:1:endRange|literal:. :endRange|month:Juni:endRange|literal: :endRange|year:1:endRange|literal: :endRange|era:n. Chr.:endRange
// CHECK-NEXT: de {"day":"numeric"}: 01.06.1 v. Chr.\u2009–\u200901.06.1 n. Chr. [eras]
// CHECK-NEXT: day:01:startRange|literal:.:startRange|month:06:startRange|literal:.:startRange|year:1:startRange|literal: :startRange|era:v. Chr.:startRange|literal:\u2009–\u2009:shared|day:01:endRange|literal:.:endRange|month:06:endRange|literal:.:endRange|year:1:endRange|literal: :endRange|era:n. Chr.:endRange
// CHECK-NEXT: de {"weekday":"long"}: Donnerstag\u2009–\u2009Freitag [no eras]
// CHECK-NEXT: weekday:Donnerstag:startRange|literal:\u2009–\u2009:shared|weekday:Freitag:endRange
// CHECK-NEXT: de {"hour":"numeric"}: 01.06.1 v. Chr., 00 Uhr\u2009–\u200901.06.1 n. Chr., 00 Uhr [eras]
// CHECK-NEXT: day:01:startRange|literal:.:startRange|month:06:startRange|literal:.:startRange|year:1:startRange|literal: :startRange|era:v. Chr.:startRange|literal:, :startRange|hour:00:startRange|literal: Uhr:startRange|literal:\u2009–\u2009:shared|day:01:endRange|literal:.:endRange|month:06:endRange|literal:.:endRange|year:1:endRange|literal: :endRange|era:n. Chr.:endRange|literal:, :endRange|hour:00:endRange|literal: Uhr:endRange
// CHECK-NEXT: de {"weekday":"short","hour":"numeric"}: Do., 01.06.1 v. Chr., 00 Uhr\u2009–\u2009Fr., 01.06.1 n. Chr., 00 Uhr [eras]
// CHECK-NEXT: weekday:Do.:startRange|literal:, :startRange|day:01:startRange|literal:.:startRange|month:06:startRange|literal:.:startRange|year:1:startRange|literal: :startRange|era:v. Chr.:startRange|literal:, :startRange|hour:00:startRange|literal: Uhr:startRange|literal:\u2009–\u2009:shared|weekday:Fr.:endRange|literal:, :endRange|day:01:endRange|literal:.:endRange|month:06:endRange|literal:.:endRange|year:1:endRange|literal: :endRange|era:n. Chr.:endRange|literal:, :endRange|hour:00:endRange|literal: Uhr:endRange
// CHECK-NEXT: de {"dateStyle":"long"}: 1. Juni 1 v. Chr.\u2009–\u20091. Juni 1 n. Chr. [eras]
// CHECK-NEXT: day:1:startRange|literal:. :startRange|month:Juni:startRange|literal: :startRange|year:1:startRange|literal: :startRange|era:v. Chr.:startRange|literal:\u2009–\u2009:shared|day:1:endRange|literal:. :endRange|month:Juni:endRange|literal: :endRange|year:1:endRange|literal: :endRange|era:n. Chr.:endRange
// CHECK-NEXT: de {"dateStyle":"full","timeStyle":"short"}: Donnerstag, 1. Juni 1 v. Chr. um 00:00\u2009–\u2009Freitag, 1. Juni 1 n. Chr. um 00:00 [eras]
// CHECK-NEXT: weekday:Donnerstag:startRange|literal:, :startRange|day:1:startRange|literal:. :startRange|month:Juni:startRange|literal: :startRange|year:1:startRange|literal: :startRange|era:v. Chr.:startRange|literal: um :startRange|hour:00:startRange|literal:::startRange|minute:00:startRange|literal:\u2009–\u2009:shared|weekday:Freitag:endRange|literal:, :endRange|day:1:endRange|literal:. :endRange|month:Juni:endRange|literal: :endRange|year:1:endRange|literal: :endRange|era:n. Chr.:endRange|literal: um :endRange|hour:00:endRange|literal:::endRange|minute:00:endRange
// CHECK-NEXT: en-u-ca-japanese {"year":"numeric"}: 31 Heisei\u2009–\u20091 Reiwa [eras]
// CHECK-NEXT: year:31:startRange|literal: :startRange|era:Heisei:startRange|literal:\u2009–\u2009:shared|year:1:endRange|literal: :endRange|era:Reiwa:endRange
// CHECK-NEXT: en-u-ca-japanese {"month":"long","day":"numeric"}: April 30, 31 Heisei\u2009–\u2009May 1, 1 Reiwa [eras]
// CHECK-NEXT: month:April:startRange|literal: :startRange|day:30:startRange|literal:, :startRange|year:31:startRange|literal: :startRange|era:Heisei:startRange|literal:\u2009–\u2009:shared|month:May:endRange|literal: :endRange|day:1:endRange|literal:, :endRange|year:1:endRange|literal: :endRange|era:Reiwa:endRange
// CHECK-NEXT: en-u-ca-japanese {"day":"numeric"}: 4/30/31 Heisei\u2009–\u20095/1/1 Reiwa [eras]
// CHECK-NEXT: month:4:startRange|literal:/:startRange|day:30:startRange|literal:/:startRange|year:31:startRange|literal: :startRange|era:Heisei:startRange|literal:\u2009–\u2009:shared|month:5:endRange|literal:/:endRange|day:1:endRange|literal:/:endRange|year:1:endRange|literal: :endRange|era:Reiwa:endRange
// CHECK-NEXT: en-u-ca-japanese {"weekday":"long"}: Tuesday\u2009–\u2009Wednesday [no eras]
// CHECK-NEXT: weekday:Tuesday:startRange|literal:\u2009–\u2009:shared|weekday:Wednesday:endRange
// CHECK-NEXT: en-u-ca-japanese {"hour":"numeric"}: 4/30/31 H, 12\u202fAM\u2009–\u20095/1/1 R, 12\u202fAM [eras]
// CHECK-NEXT: month:4:startRange|literal:/:startRange|day:30:startRange|literal:/:startRange|year:31:startRange|literal: :startRange|era:H:startRange|literal:, :startRange|hour:12:startRange|literal:\u202f:startRange|dayPeriod:AM:startRange|literal:\u2009–\u2009:shared|month:5:endRange|literal:/:endRange|day:1:endRange|literal:/:endRange|year:1:endRange|literal: :endRange|era:R:endRange|literal:, :endRange|hour:12:endRange|literal:\u202f:endRange|dayPeriod:AM:endRange
// CHECK-NEXT: en-u-ca-japanese {"weekday":"short","hour":"numeric"}: Tue, 4/30/31 Heisei, 12\u202fAM\u2009–\u2009Wed, 5/1/1 Reiwa, 12\u202fAM [eras]
// CHECK-NEXT: weekday:Tue:startRange|literal:, :startRange|month:4:startRange|literal:/:startRange|day:30:startRange|literal:/:startRange|year:31:startRange|literal: :startRange|era:Heisei:startRange|literal:, :startRange|hour:12:startRange|literal:\u202f:startRange|dayPeriod:AM:startRange|literal:\u2009–\u2009:shared|weekday:Wed:endRange|literal:, :endRange|month:5:endRange|literal:/:endRange|day:1:endRange|literal:/:endRange|year:1:endRange|literal: :endRange|era:Reiwa:endRange|literal:, :endRange|hour:12:endRange|literal:\u202f:endRange|dayPeriod:AM:endRange
// CHECK-NEXT: en-u-ca-japanese {"dateStyle":"long"}: April 30, 31 Heisei\u2009–\u2009May 1, 1 Reiwa [eras]
// CHECK-NEXT: month:April:startRange|literal: :startRange|day:30:startRange|literal:, :startRange|year:31:startRange|literal: :startRange|era:Heisei:startRange|literal:\u2009–\u2009:shared|month:May:endRange|literal: :endRange|day:1:endRange|literal:, :endRange|year:1:endRange|literal: :endRange|era:Reiwa:endRange
// CHECK-NEXT: en-u-ca-japanese {"dateStyle":"full","timeStyle":"short"}: Tuesday, April 30, 31 Heisei at 12:00\u202fAM\u2009–\u2009Wednesday, May 1, 1 Reiwa at 12:00\u202fAM [eras]
// CHECK-NEXT: weekday:Tuesday:startRange|literal:, :startRange|month:April:startRange|literal: :startRange|day:30:startRange|literal:, :startRange|year:31:startRange|literal: :startRange|era:Heisei:startRange|literal: at :startRange|hour:12:startRange|literal:::startRange|minute:00:startRange|literal:\u202f:startRange|dayPeriod:AM:startRange|literal:\u2009–\u2009:shared|weekday:Wednesday:endRange|literal:, :endRange|month:May:endRange|literal: :endRange|day:1:endRange|literal:, :endRange|year:1:endRange|literal: :endRange|era:Reiwa:endRange|literal: at :endRange|hour:12:endRange|literal:::endRange|minute:00:endRange|literal:\u202f:endRange|dayPeriod:AM:endRange
// CHECK-NEXT: de-u-ca-japanese {"year":"numeric"}: Heisei 31\u2009–\u2009Reiwa 1 [eras]
// CHECK-NEXT: era:Heisei:startRange|literal: :startRange|year:31:startRange|literal:\u2009–\u2009:shared|era:Reiwa:endRange|literal: :endRange|year:1:endRange
// CHECK-NEXT: de-u-ca-japanese {"month":"long","day":"numeric"}: 30. April 31 Heisei\u2009–\u20091. Mai 1 Reiwa [eras]
// CHECK-NEXT: day:30:startRange|literal:. :startRange|month:April:startRange|literal: :startRange|year:31:startRange|literal: :startRange|era:Heisei:startRange|literal:\u2009–\u2009:shared|day:1:endRange|literal:. :endRange|month:Mai:endRange|literal: :endRange|year:1:endRange|literal: :endRange|era:Reiwa:endRange
// CHECK-NEXT: de-u-ca-japanese {"day":"numeric"}: 30.4.31 Heisei\u2009–\u20091.5.1 Reiwa [eras]
// CHECK-NEXT: day:30:startRange|literal:.:startRange|month:4:startRange|literal:.:startRange|year:31:startRange|literal: :startRange|era:Heisei:startRange|literal:\u2009–\u2009:shared|day:1:endRange|literal:.:endRange|month:5:endRange|literal:.:endRange|year:1:endRange|literal: :endRange|era:Reiwa:endRange
// CHECK-NEXT: de-u-ca-japanese {"weekday":"long"}: Dienstag\u2009–\u2009Mittwoch [no eras]
// CHECK-NEXT: weekday:Dienstag:startRange|literal:\u2009–\u2009:shared|weekday:Mittwoch:endRange
// CHECK-NEXT: de-u-ca-japanese {"hour":"numeric"}: 30.4.31 H, 00 Uhr\u2009–\u20091.5.1 R, 00 Uhr [eras]
// CHECK-NEXT: day:30:startRange|literal:.:startRange|month:4:startRange|literal:.:startRange|year:31:startRange|literal: :startRange|era:H:startRange|literal:, :startRange|hour:00:startRange|literal: Uhr:startRange|literal:\u2009–\u2009:shared|day:1:endRange|literal:.:endRange|month:5:endRange|literal:.:endRange|year:1:endRange|literal: :endRange|era:R:endRange|literal:, :endRange|hour:00:endRange|literal: Uhr:endRange
// CHECK-NEXT: de-u-ca-japanese {"weekday":"short","hour":"numeric"}: Di., 30.4.31 Heisei, 00 Uhr\u2009–\u2009Mi., 1.5.1 Reiwa, 00 Uhr [eras]
// CHECK-NEXT: weekday:Di.:startRange|literal:, :startRange|day:30:startRange|literal:.:startRange|month:4:startRange|literal:.:startRange|year:31:startRange|literal: :startRange|era:Heisei:startRange|literal:, :startRange|hour:00:startRange|literal: Uhr:startRange|literal:\u2009–\u2009:shared|weekday:Mi.:endRange|literal:, :endRange|day:1:endRange|literal:.:endRange|month:5:endRange|literal:.:endRange|year:1:endRange|literal: :endRange|era:Reiwa:endRange|literal:, :endRange|hour:00:endRange|literal: Uhr:endRange
// CHECK-NEXT: de-u-ca-japanese {"dateStyle":"long"}: Heisei 31 April 30\u2009–\u2009Reiwa 1 Mai 1 [eras]
// CHECK-NEXT: era:Heisei:startRange|literal: :startRange|year:31:startRange|literal: :startRange|month:April:startRange|literal: :startRange|day:30:startRange|literal:\u2009–\u2009:shared|era:Reiwa:endRange|literal: :endRange|year:1:endRange|literal: :endRange|month:Mai:endRange|literal: :endRange|day:1:endRange
// CHECK-NEXT: de-u-ca-japanese {"dateStyle":"full","timeStyle":"short"}: Dienstag, 30. April 31 Heisei um 00:00\u2009–\u2009Mittwoch, 1. Mai 1 Reiwa um 00:00 [eras]
// CHECK-NEXT: weekday:Dienstag:startRange|literal:, :startRange|day:30:startRange|literal:. :startRange|month:April:startRange|literal: :startRange|year:31:startRange|literal: :startRange|era:Heisei:startRange|literal: um :startRange|hour:00:startRange|literal:::startRange|minute:00:startRange|literal:\u2009–\u2009:shared|weekday:Mittwoch:endRange|literal:, :endRange|day:1:endRange|literal:. :endRange|month:Mai:endRange|literal: :endRange|year:1:endRange|literal: :endRange|era:Reiwa:endRange|literal: um :endRange|hour:00:endRange|literal:::endRange|minute:00:endRange

// Ranges within an era are as before. A time-only R in a calendar with
// eras has an [[Era]], so its [[Year]], [[Month]] and [[Day]] are
// [[Default]]'s: without them the loop would stop at the absent [[Year]]
// and collapse two days to one.
var AD9 = Date.parse('0001-06-01T09:00:00Z');
show('en', {hour: 'numeric'}, AD9, Date.parse('0001-06-01T17:00:00Z'));
// CHECK-NEXT: en {"hour":"numeric"}: 9\u202fAM\u2009–\u20095\u202fPM
// CHECK-NEXT: hour:9:startRange|literal:\u202f:startRange|dayPeriod:AM:startRange|literal:\u2009–\u2009:shared|hour:5:endRange|literal:\u202f:endRange|dayPeriod:PM:endRange
show('en', {hour: 'numeric'}, AD9, Date.parse('0001-06-02T09:00:00Z'));
// CHECK-NEXT: en {"hour":"numeric"}: 6/1/1, 9\u202fAM\u2009–\u20096/2/1, 9\u202fAM
// CHECK-NEXT: month:6:startRange|literal:/:startRange|day:1:startRange|literal:/:startRange|year:1:startRange|literal:, :startRange|hour:9:startRange|literal:\u202f:startRange|dayPeriod:AM:startRange|literal:\u2009–\u2009:shared|month:6:endRange|literal:/:endRange|day:2:endRange|literal:/:endRange|year:1:endRange|literal:, :endRange|hour:9:endRange|literal:\u202f:endRange|dayPeriod:AM:endRange
show('de', {hour: 'numeric'}, AD9, Date.parse('0001-06-02T09:00:00Z'));
// CHECK-NEXT: de {"hour":"numeric"}: 1.6.1, 09 Uhr\u2009–\u20092.6.1, 09 Uhr
// CHECK-NEXT: day:1:startRange|literal:.:startRange|month:6:startRange|literal:.:startRange|year:1:startRange|literal:, :startRange|hour:09:startRange|literal: Uhr:startRange|literal:\u2009–\u2009:shared|day:2:endRange|literal:.:endRange|month:6:endRange|literal:.:endRange|year:1:endRange|literal:, :endRange|hour:09:endRange|literal: Uhr:endRange
show('en', {hour: 'numeric'}, AD9, Date.parse('0002-06-01T09:00:00Z'));
// CHECK-NEXT: en {"hour":"numeric"}: 6/1/1, 9\u202fAM\u2009–\u20096/1/2, 9\u202fAM
// CHECK-NEXT: month:6:startRange|literal:/:startRange|day:1:startRange|literal:/:startRange|year:1:startRange|literal:, :startRange|hour:9:startRange|literal:\u202f:startRange|dayPeriod:AM:startRange|literal:\u2009–\u2009:shared|month:6:endRange|literal:/:endRange|day:1:endRange|literal:/:endRange|year:2:endRange|literal:, :endRange|hour:9:endRange|literal:\u202f:endRange|dayPeriod:AM:endRange
show('en', {year: 'numeric'}, AD, Date.parse('0002-06-01T00:00:00Z'));
// CHECK-NEXT: en {"year":"numeric"}: 1\u2009–\u20092
// CHECK-NEXT: year:1:startRange|literal:\u2009–\u2009:shared|year:2:endRange
show('en', {day: 'numeric'}, AD, Date.parse('0001-07-09T00:00:00Z'));
// CHECK-NEXT: en {"day":"numeric"}: 6/1\u2009–\u20097/9
// CHECK-NEXT: month:6:startRange|literal:/:startRange|day:1:startRange|literal:\u2009–\u2009:shared|month:7:endRange|literal:/:endRange|day:9:endRange
show('en', {weekday: 'long'}, AD, Date.parse('0001-06-02T00:00:00Z'));
// CHECK-NEXT: en {"weekday":"long"}: Friday\u2009–\u2009Saturday
// CHECK-NEXT: weekday:Friday:startRange|literal:\u2009–\u2009:shared|weekday:Saturday:endRange
show('en', {weekday: 'short', hour: 'numeric'}, AD9, Date.parse('0001-06-02T09:00:00Z'));
// CHECK-NEXT: en {"weekday":"short","hour":"numeric"}: 1 Fri, 9\u202fAM\u2009–\u20092 Sat, 9\u202fAM
// CHECK-NEXT: day:1:startRange|literal: :startRange|weekday:Fri:startRange|literal:, :startRange|hour:9:startRange|literal:\u202f:startRange|dayPeriod:AM:startRange|literal:\u2009–\u2009:shared|day:2:endRange|literal: :endRange|weekday:Sat:endRange|literal:, :endRange|hour:9:endRange|literal:\u202f:endRange|dayPeriod:AM:endRange
show('en', {dateStyle: 'long'}, AD, Date.parse('0001-06-09T00:00:00Z'));
// CHECK-NEXT: en {"dateStyle":"long"}: June 1\u2009–\u20099, 1
// CHECK-NEXT: month:June:shared|literal: :shared|day:1:startRange|literal:\u2009–\u2009:shared|day:9:endRange|literal:, :shared|year:1:shared
show('en-u-ca-japanese', {day: 'numeric'}, REIWA, Date.parse('2019-06-09T00:00:00Z'));
// CHECK-NEXT: en-u-ca-japanese {"day":"numeric"}: 5/1\u2009–\u20096/9
// CHECK-NEXT: month:5:startRange|literal:/:startRange|day:1:startRange|literal:\u2009–\u2009:shared|month:6:endRange|literal:/:endRange|day:9:endRange

// An R that shows its era takes CLDR's interval for an era difference
// only if both halves show the era. CLDR 48.2's ko GyMMMd and GyMd
// intervals for G, "G y년 M월 d일 ~ y년 M월 d일", lack the second era (as
// ICU4C shows, "헤이세이 31년 4월 30일 ~ 1년 5월 1일"), so [[Era]] is
// FB(P), with both eras.
var MINGUO1 = Date.parse('1911-06-01T00:00:00Z');
var MINGUO2 = Date.parse('1913-06-01T00:00:00Z');
show('ko-u-ca-japanese', {year: 'numeric', month: 'short', day: 'numeric'}, HEISEI, REIWA, true);
// CHECK-NEXT: ko-u-ca-japanese {"year":"numeric","month":"short","day":"numeric"}: 헤이세이 31년 4월 30일 ~ 레이와 1년 5월 1일 [eras]
// CHECK-NEXT: era:헤이세이:startRange|literal: :startRange|year:31:startRange|literal:년 :startRange|month:4월:startRange|literal: :startRange|day:30:startRange|literal:일:startRange|literal: ~ :shared|era:레이와:endRange|literal: :endRange|year:1:endRange|literal:년 :endRange|month:5월:endRange|literal: :endRange|day:1:endRange|literal:일:endRange
show('ko-u-ca-japanese', {year: 'numeric', day: 'numeric'}, HEISEI, REIWA, true);
// CHECK-NEXT: ko-u-ca-japanese {"year":"numeric","day":"numeric"}: 헤이세이 31년 4월 30일 ~ 레이와 1년 5월 1일 [eras]
// CHECK-NEXT: era:헤이세이:startRange|literal: :startRange|year:31:startRange|literal:년 :startRange|month:4월:startRange|literal: :startRange|day:30:startRange|literal:일:startRange|literal: ~ :shared|era:레이와:endRange|literal: :endRange|year:1:endRange|literal:년 :endRange|month:5월:endRange|literal: :endRange|day:1:endRange|literal:일:endRange
show('ko-u-ca-roc', {year: 'numeric', month: 'short', day: 'numeric'}, MINGUO1, MINGUO2, true);
// CHECK-NEXT: ko-u-ca-roc {"year":"numeric","month":"short","day":"numeric"}: 중화민국전 1년 6월 1일 ~ 중화민국 2년 6월 1일 [eras]
// CHECK-NEXT: era:중화민국전:startRange|literal: :startRange|year:1:startRange|literal:년 :startRange|month:6월:startRange|literal: :startRange|day:1:startRange|literal:일:startRange|literal: ~ :shared|era:중화민국:endRange|literal: :endRange|year:2:endRange|literal:년 :endRange|month:6월:endRange|literal: :endRange|day:1:endRange|literal:일:endRange
show('ko-u-ca-roc', {year: 'numeric', day: 'numeric'}, MINGUO1, MINGUO2, true);
// CHECK-NEXT: ko-u-ca-roc {"year":"numeric","day":"numeric"}: 중화민국전 1년 6월 1일 ~ 중화민국 2년 6월 1일 [eras]
// CHECK-NEXT: era:중화민국전:startRange|literal: :startRange|year:1:startRange|literal:년 :startRange|month:6월:startRange|literal: :startRange|day:1:startRange|literal:일:startRange|literal: ~ :shared|era:중화민국:endRange|literal: :endRange|year:2:endRange|literal:년 :endRange|month:6월:endRange|literal: :endRange|day:1:endRange|literal:일:endRange

// Chinese and dangi have no eras, so their ranges have no [[Era]].
show('en-u-ca-chinese', {day: 'numeric'}, BC, AD);
// CHECK-NEXT: en-u-ca-chinese {"day":"numeric"}: 4/13/0\u2009–\u20094/24/1
// CHECK-NEXT: month:4:startRange|literal:/:startRange|day:13:startRange|literal:/:startRange|relatedYear:0:startRange|literal:\u2009–\u2009:shared|month:4:endRange|literal:/:endRange|day:24:endRange|literal:/:endRange|relatedYear:1:endRange
show('en-u-ca-chinese', {hour: 'numeric'}, BC, AD);
// CHECK-NEXT: en-u-ca-chinese {"hour":"numeric"}: 4/13/0, 12\u202fAM\u2009–\u20094/24/1, 12\u202fAM
// CHECK-NEXT: month:4:startRange|literal:/:startRange|day:13:startRange|literal:/:startRange|relatedYear:0:startRange|literal:, :startRange|hour:12:startRange|literal:\u202f:startRange|dayPeriod:AM:startRange|literal:\u2009–\u2009:shared|month:4:endRange|literal:/:endRange|day:24:endRange|literal:/:endRange|relatedYear:1:endRange|literal:, :endRange|hour:12:endRange|literal:\u202f:endRange|dayPeriod:AM:endRange
show('ko-u-ca-dangi', {dateStyle: 'long'}, BC, AD);
// CHECK-NEXT: ko-u-ca-dangi {"dateStyle":"long"}: 경신 4월 13\u2009–\u2009신유 4월 24
// CHECK-NEXT: yearName:경신:startRange|literal: :startRange|month:4월:startRange|literal: :startRange|day:13:startRange|literal:\u2009–\u2009:shared|yearName:신유:endRange|literal: :endRange|month:4월:endRange|literal: :endRange|day:24:endRange
show('ko-u-ca-dangi', {hour: 'numeric'}, HEISEI, REIWA);
// CHECK-NEXT: ko-u-ca-dangi {"hour":"numeric"}: 2019. 3. 26. 오전 12시\u2009–\u20092019. 3. 27. 오전 12시
// CHECK-NEXT: relatedYear:2019:startRange|literal:. :startRange|month:3:startRange|literal:. :startRange|day:26:startRange|literal:. :startRange|dayPeriod:오전:startRange|literal: :startRange|hour:12:startRange|literal:시:startRange|literal:\u2009–\u2009:shared|relatedYear:2019:endRange|literal:. :endRange|month:3:endRange|literal:. :endRange|day:27:endRange|literal:. :endRange|dayPeriod:오전:endRange|literal: :endRange|hour:12:endRange|literal:시:endRange
