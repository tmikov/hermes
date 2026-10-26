/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US -Xintl-default-time-zone=UTC %s | %FileCheck --match-full-lines %s

// The timeZone option of Intl.DateTimeFormat: named time zones resolve to
// their primary identifier, matched without regard to case (plan decision
// 10); offset time zones follow ECMA-262's UTCOffset grammar, and resolve
// to FormatOffsetTimeZoneIdentifier's "±HH:MM".

/// The resolved time zone of timeZone, or the class of the exception.
function tz(timeZone) {
  try {
    return new Intl.DateTimeFormat('en', {timeZone: timeZone}).resolvedOptions().timeZone;
  } catch (e) {
    return e.constructor.name;
  }
}

// The default is the time zone the loader resolved.
print(new Intl.DateTimeFormat().resolvedOptions().timeZone, tz(undefined));
// CHECK: UTC UTC

// Named time zones: the primary identifier, whatever the case.
print(tz('utc'), tz('Etc/UTC'), tz('Etc/GMT'), tz('GMT'), tz('etc/universal'));
// CHECK-NEXT: UTC UTC UTC UTC UTC
print(tz('asia/calcutta'), tz('Asia/Kolkata'), tz('ASIA/KOLKATA'));
// CHECK-NEXT: Asia/Kolkata Asia/Kolkata Asia/Kolkata
print(tz('Pacific/Truk'), tz('Atlantic/Jan_Mayen'), tz('America/Buenos_Aires'));
// CHECK-NEXT: Pacific/Chuuk Arctic/Longyearbyen America/Argentina/Buenos_Aires
print(tz('Etc/GMT+5'), tz('etc/gmt-14'));
// CHECK-NEXT: Etc/GMT+5 Etc/GMT-14
// ToString of the option.
print(tz({toString: () => 'Europe/Paris'}), tz(null), tz(5));
// CHECK-NEXT: Europe/Paris RangeError RangeError
print(tz(Symbol('UTC')));
// CHECK-NEXT: TypeError

// Offset time zones: hours, or hours and minutes, with or without the
// colon; ASCII signs only.
print(tz('+05:30'), tz('+0530'), tz('+05'), tz('-05:00'), tz('-0800'));
// CHECK-NEXT: +05:30 +05:30 +05:00 -05:00 -08:00
print(tz('-00:00'), tz('+00'), tz('-00'), tz('+23:59'), tz('-23:59'));
// CHECK-NEXT: +00:00 +00:00 +00:00 +23:59 -23:59
// Seconds (with or without a fraction) are a RangeError, as are a Unicode
// minus sign, one-digit hours or minutes, hours beyond 23, minutes beyond
// 59, a mixed separator, and anything after the offset.
print(tz('+05:30:00'), tz('+053000'), tz('+05:30:00.5'), tz('-00:00:00'));
// CHECK-NEXT: RangeError RangeError RangeError RangeError
print(tz('−05:00'), tz('+5'), tz('+05:3'), tz('+24:00'), tz('+05:60'));
// CHECK-NEXT: RangeError RangeError RangeError RangeError RangeError
print(tz('+05:30x'), tz('+0530:00'), tz('+05:3000'), tz('05:30'), tz('+'), tz(''));
// CHECK-NEXT: RangeError RangeError RangeError RangeError RangeError RangeError
print(tz('+05.5'), tz('+05:30.5'), tz(' +05:30'), tz('+０５'));
// CHECK-NEXT: RangeError RangeError RangeError RangeError

// The non-IANA names of test262's timezone-legacy-non-iana.js, and other
// names that are not time zones.
var legacy = [
  'ACT', 'AET', 'AGT', 'ART', 'AST', 'BET', 'BST', 'CAT', 'CNT', 'CST',
  'CTT', 'EAT', 'ECT', 'IET', 'IST', 'JST', 'MIT', 'NET', 'NST', 'PLT',
  'PNT', 'PRT', 'PST', 'SST', 'VST',
];
print(legacy.length, JSON.stringify(legacy.filter(name => tz(name) !== 'RangeError')));
// CHECK-NEXT: 25 []
print(tz('Europe/Nowhere'), tz('Asia/Kolkata '), tz('Asia\u0000/Kolkata'), tz('\ud800'));
// CHECK-NEXT: RangeError RangeError RangeError RangeError

// Every value of Intl.supportedValuesOf("timeZone") is accepted and
// resolves to itself, and so does its upper-case form.
var zones = Intl.supportedValuesOf('timeZone');
var notItself = zones.filter(zone => tz(zone) !== zone);
var notUpper = zones.filter(zone => tz(zone.toUpperCase()) !== zone);
print(zones.length > 400, JSON.stringify(notItself), JSON.stringify(notUpper));
// CHECK-NEXT: true [] []
