/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US -Xintl-default-time-zone=UTC %s | %FileCheck --match-full-lines %s

// CLDR 49 makes the POSIX-style zones cst6cdt, est5edt, mst7mdt and pst8pdt
// zones of their own in bcp47/timezone.xml, with no region. As -u-tz-
// values they are canonical (CLDR 48 aliased them to uschi, usnyc, usden
// and uslax; Node gives "en-u-tz-usnyc"), while a deprecated zone still
// goes to its preferred value. As time zone names they keep ECMA-402's
// behaviour: tzdb 2024b made them links, so they resolve to their targets
// and supportedValuesOf does not list them. No region lists them either.

print(JSON.stringify(Intl.getCanonicalLocales([
  'en-u-tz-est5edt', 'en-u-tz-PST8PDT', 'en-u-tz-aukns', 'en-u-tz-usnyc',
])));
// CHECK: ["en-u-tz-est5edt","en-u-tz-pst8pdt","en-u-tz-auhba","en-u-tz-usnyc"]
print(new Intl.Locale('en-u-tz-cst6cdt').toString(), new Intl.Locale('en-u-tz-mst7mdt').baseName);
// CHECK-NEXT: en-u-tz-cst6cdt en

var zones = Intl.supportedValuesOf('timeZone');
print(
  ['CST6CDT', 'EST5EDT', 'MST7MDT', 'PST8PDT'].map(z => zones.indexOf(z)).join(),
  zones.indexOf('America/New_York') >= 0,
);
// CHECK-NEXT: -1,-1,-1,-1 true

var t = Date.UTC(2026, 6, 1, 12, 0, 0);
var NAMES = ['EST5EDT', 'est5edt', 'CST6CDT', 'MST7MDT', 'PST8PDT'];
for (var i = 0; i < NAMES.length; i++) {
  var f = new Intl.DateTimeFormat('en', {
    timeZone: NAMES[i],
    hour: 'numeric',
    minute: 'numeric',
    timeZoneName: 'short',
  });
  print(NAMES[i], f.resolvedOptions().timeZone, f.format(t).replace(' ', ' '));
}
// CHECK-NEXT: EST5EDT America/New_York 8:00 AM EDT
// CHECK-NEXT: est5edt America/New_York 8:00 AM EDT
// CHECK-NEXT: CST6CDT America/Chicago 7:00 AM CDT
// CHECK-NEXT: MST7MDT America/Denver 6:00 AM MDT
// CHECK-NEXT: PST8PDT America/Los_Angeles 5:00 AM PDT

// The regions the first letters of their names would give (PS, ES, MS)
// have only their own zones, and the US has its zones, not these.
var REGIONS = ['und-PS', 'und-ES', 'und-MS'];
for (var j = 0; j < REGIONS.length; j++) {
  print(REGIONS[j], JSON.stringify(new Intl.Locale(REGIONS[j]).getTimeZones()));
}
// CHECK-NEXT: und-PS ["Asia/Gaza","Asia/Hebron"]
// CHECK-NEXT: und-ES ["Africa/Ceuta","Atlantic/Canary","Europe/Madrid"]
// CHECK-NEXT: und-MS ["America/Montserrat"]
var us = new Intl.Locale('und-US').getTimeZones();
print(us.indexOf('America/Los_Angeles') >= 0, us.some(z => /[0-9]/.test(z)));
// CHECK-NEXT: true false
