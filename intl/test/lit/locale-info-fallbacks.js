/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes %s | %FileCheck --match-full-lines %s

// The Intl.Locale information methods on locales with little or no data.
// Each expected line is worked out from the steps of the ECMA-402 draft of
// October 24, 2026 ("Abstract Operations for Locale Objects") and the
// CLDR 48.2 data of intl/core/data and ICU4X; the comments name the step
// that gives it. RegionPreference turns a missing region into the likely
// one, or "001", which has calendar, hour-cycle and week data, so the
// last-resort fallbacks apply less often than one might think. No method
// throws.

/// JSON for v, or "undefined".
function show(v) {
  return v === undefined ? 'undefined' : JSON.stringify(v);
}

/// Print each information method's result for tag, one line each.
/// getTextInfo prints its own keys, then the direction.
function run(tag) {
  var locale = new Intl.Locale(tag);
  var names = [
    'getCalendars',
    'getCollations',
    'getHourCycles',
    'getNumberingSystems',
    'getTimeZones',
    'getTextInfo',
    'getWeekInfo',
  ];
  for (var i = 0; i < names.length; i++) {
    var line;
    try {
      var r = locale[names[i]]();
      line =
        names[i] === 'getTextInfo'
          ? Object.keys(r).join(',') + '=' + r.direction
          : show(r);
    } catch (e) {
      line = 'threw ' + e;
    }
    print(tag, names[i], line);
  }
}

// "und": Add Likely Subtags gives en-Latn-US, so RegionPreference's region
// is "US" (step 2.b.iii). "und" is an available locale.
run('und');
// CalendarsOfLocale: neither "und-US" nor "US" has a calendarPreferenceData
// line (step 6), so step 10 gives "gregory".
// CHECK: und getCalendars ["gregory"]
// CollationsOfLocale step 3: "und" is found; the root's tailorings.
// CHECK-NEXT: und getCollations ["emoji","eor"]
// HourCyclesOfLocale step 6.c: the "US" timeData line.
// CHECK-NEXT: und getHourCycles ["h12","h23"]
// NumberingSystemsOfLocale step 3: "und" is found; its decimal data.
// CHECK-NEXT: und getNumberingSystems ["latn"]
// TimeZonesOfLocale step 2: the tag has no region.
// CHECK-NEXT: und getTimeZones undefined
// TextDirectionOfLocale step 3.b: the likely script Latn; step 5.
// CHECK-NEXT: und getTextInfo direction=ltr
// WeekInfoOfLocale step 5: the region "US".
// CHECK-NEXT: und getWeekInfo {"firstDay":7,"weekend":[6,7]}

// "zxx" has no likely subtags, so the region is "001" (step 2.b.iv).
run('zxx');
// CalendarsOfLocale step 6.c: the "001" line.
// CHECK-NEXT: zxx getCalendars ["gregory"]
// CollationsOfLocale step 4: no prefix of "zxx" is available.
// CHECK-NEXT: zxx getCollations ["emoji","eor"]
// HourCyclesOfLocale step 6.c: the "001" line.
// CHECK-NEXT: zxx getHourCycles ["h23","h12"]
// NumberingSystemsOfLocale step 4: no prefix of "zxx" is available.
// CHECK-NEXT: zxx getNumberingSystems ["latn"]
// TimeZonesOfLocale step 2.
// CHECK-NEXT: zxx getTimeZones undefined
// TextDirectionOfLocale step 3.c: no likely script. The key is still
// present.
// CHECK-NEXT: zxx getTextInfo direction=undefined
// WeekInfoOfLocale step 5: the "001" data.
// CHECK-NEXT: zxx getWeekInfo {"firstDay":1,"weekend":[6,7]}

// "qaa-Qaaa-QM": private-use language, script and region; the region is
// "QM" (RegionPreference step 1).
run('qaa-Qaaa-QM');
// CalendarsOfLocale: no "QM" line (step 6), so step 10.
// CHECK-NEXT: qaa-Qaaa-QM getCalendars ["gregory"]
// CollationsOfLocale step 4: no prefix is available.
// CHECK-NEXT: qaa-Qaaa-QM getCollations ["emoji","eor"]
// HourCyclesOfLocale: no "qaa-QM" or "QM" line (step 6), so step 7.
// CHECK-NEXT: qaa-Qaaa-QM getHourCycles ["h23"]
// NumberingSystemsOfLocale step 4.
// CHECK-NEXT: qaa-Qaaa-QM getNumberingSystems ["latn"]
// TimeZonesOfLocale step 3: "QM" has no zones.
// CHECK-NEXT: qaa-Qaaa-QM getTimeZones []
// TextDirectionOfLocale step 6: Qaaa has no known direction.
// CHECK-NEXT: qaa-Qaaa-QM getTextInfo direction=undefined
// WeekInfoOfLocale step 5: "QM" gets the "001" data by fallback.
// CHECK-NEXT: qaa-Qaaa-QM getWeekInfo {"firstDay":1,"weekend":[6,7]}

// "posix", a 5-letter language with no likely subtags: as "zxx".
run('posix');
// CHECK-NEXT: posix getCalendars ["gregory"]
// CHECK-NEXT: posix getCollations ["emoji","eor"]
// CHECK-NEXT: posix getHourCycles ["h23","h12"]
// CHECK-NEXT: posix getNumberingSystems ["latn"]
// CHECK-NEXT: posix getTimeZones undefined
// CHECK-NEXT: posix getTextInfo direction=undefined
// CHECK-NEXT: posix getWeekInfo {"firstDay":1,"weekend":[6,7]}

// "en-ZZ": the unknown region is the tag's own, so RegionPreference keeps it
// (step 1). "en" is the available prefix, and Add Likely Subtags gives Latn.
run('en-ZZ');
// CalendarsOfLocale: no "ZZ" line (step 6), so step 10.
// CHECK-NEXT: en-ZZ getCalendars ["gregory"]
// CollationsOfLocale step 3: "en", whose tailorings are the root's.
// CHECK-NEXT: en-ZZ getCollations ["emoji","eor"]
// HourCyclesOfLocale: no "en-ZZ" or "ZZ" line (step 6), so step 7.
// CHECK-NEXT: en-ZZ getHourCycles ["h23"]
// NumberingSystemsOfLocale step 3: "en".
// CHECK-NEXT: en-ZZ getNumberingSystems ["latn"]
// TimeZonesOfLocale step 3: "ZZ" has no zones.
// CHECK-NEXT: en-ZZ getTimeZones []
// TextDirectionOfLocale steps 3.b and 5.
// CHECK-NEXT: en-ZZ getTextInfo direction=ltr
// WeekInfoOfLocale step 5: "ZZ" gets the "001" data by fallback.
// CHECK-NEXT: en-ZZ getWeekInfo {"firstDay":1,"weekend":[6,7]}

// "en-QO" (Outlying Oceania): as "en-ZZ".
run('en-QO');
// CHECK-NEXT: en-QO getCalendars ["gregory"]
// CHECK-NEXT: en-QO getCollations ["emoji","eor"]
// CHECK-NEXT: en-QO getHourCycles ["h23"]
// CHECK-NEXT: en-QO getNumberingSystems ["latn"]
// CHECK-NEXT: en-QO getTimeZones []
// CHECK-NEXT: en-QO getTextInfo direction=ltr
// CHECK-NEXT: en-QO getWeekInfo {"firstDay":1,"weekend":[6,7]}

// "en-AQ": neither "en-AQ" nor "AQ" has calendar or time data, but "AQ" has
// time zones.
run('en-AQ');
// CalendarsOfLocale step 10.
// CHECK-NEXT: en-AQ getCalendars ["gregory"]
// CHECK-NEXT: en-AQ getCollations ["emoji","eor"]
// HourCyclesOfLocale step 7.
// CHECK-NEXT: en-AQ getHourCycles ["h23"]
// CHECK-NEXT: en-AQ getNumberingSystems ["latn"]
// TimeZonesOfLocale step 3: the "AQ" zones, sorted.
// CHECK-NEXT: en-AQ getTimeZones ["Antarctica/Casey","Antarctica/Davis","Antarctica/DumontDUrville","Antarctica/Mawson","Antarctica/McMurdo","Antarctica/Palmer","Antarctica/Rothera","Antarctica/Syowa","Antarctica/Troll","Antarctica/Vostok"]
// CHECK-NEXT: en-AQ getTextInfo direction=ltr
// WeekInfoOfLocale step 5: "AQ" equals "001".
// CHECK-NEXT: en-AQ getWeekInfo {"firstDay":1,"weekend":[6,7]}

// "es-419": the region is "419" (RegionPreference step 1), and "es-419" is
// an available locale.
run('es-419');
// CalendarsOfLocale: no "419" line (step 6), so step 10.
// CHECK-NEXT: es-419 getCalendars ["gregory"]
// CollationsOfLocale step 3: "es-419", whose fallback chain reaches "es"
// and its "trad" tailoring.
// CHECK-NEXT: es-419 getCollations ["emoji","eor","trad"]
// HourCyclesOfLocale step 6.c: no "es-419" line, but a "419" one.
// CHECK-NEXT: es-419 getHourCycles ["h12","h23"]
// NumberingSystemsOfLocale step 3: "es-419".
// CHECK-NEXT: es-419 getNumberingSystems ["latn"]
// TimeZonesOfLocale step 3: a macroregion has no zones of its own.
// CHECK-NEXT: es-419 getTimeZones []
// CHECK-NEXT: es-419 getTextInfo direction=ltr
// WeekInfoOfLocale step 5: "419" gets the "001" data by fallback.
// CHECK-NEXT: es-419 getWeekInfo {"firstDay":1,"weekend":[6,7]}

// "en-u-rg-xx": "xx" is not a unicode_subdivision_id, so there is no region
// override (CanonicalUnicodeSubdivision step 3), and the region is the
// likely "US" (RegionPreference step 2.b.iii).
run('en-u-rg-xx');
// CalendarsOfLocale step 10.
// CHECK-NEXT: en-u-rg-xx getCalendars ["gregory"]
// CollationsOfLocale step 3: "en", once the extension is removed.
// CHECK-NEXT: en-u-rg-xx getCollations ["emoji","eor"]
// HourCyclesOfLocale step 6.c: the "US" line.
// CHECK-NEXT: en-u-rg-xx getHourCycles ["h12","h23"]
// CHECK-NEXT: en-u-rg-xx getNumberingSystems ["latn"]
// TimeZonesOfLocale step 2: the tag itself has no region.
// CHECK-NEXT: en-u-rg-xx getTimeZones undefined
// CHECK-NEXT: en-u-rg-xx getTextInfo direction=ltr
// WeekInfoOfLocale step 5: "US".
// CHECK-NEXT: en-u-rg-xx getWeekInfo {"firstDay":7,"weekend":[6,7]}

// "en-u-sd-foobarx": "foobarx" is not a unicode_subdivision_id (five
// characters after the region), so RegionPreference step 2.b applies: as
// "en-u-rg-xx".
run('en-u-sd-foobarx');
// CHECK-NEXT: en-u-sd-foobarx getCalendars ["gregory"]
// CHECK-NEXT: en-u-sd-foobarx getCollations ["emoji","eor"]
// CHECK-NEXT: en-u-sd-foobarx getHourCycles ["h12","h23"]
// CHECK-NEXT: en-u-sd-foobarx getNumberingSystems ["latn"]
// CHECK-NEXT: en-u-sd-foobarx getTimeZones undefined
// CHECK-NEXT: en-u-sd-foobarx getTextInfo direction=ltr
// CHECK-NEXT: en-u-sd-foobarx getWeekInfo {"firstDay":7,"weekend":[6,7]}

// "en-u-fw-xyz": WeekdayUValueToNumber("xyz") is undefined, so the first
// day stays the region's (WeekInfoOfLocale step 9.b); the rest is as "en".
run('en-u-fw-xyz');
// CHECK-NEXT: en-u-fw-xyz getCalendars ["gregory"]
// CHECK-NEXT: en-u-fw-xyz getCollations ["emoji","eor"]
// CHECK-NEXT: en-u-fw-xyz getHourCycles ["h12","h23"]
// CHECK-NEXT: en-u-fw-xyz getNumberingSystems ["latn"]
// CHECK-NEXT: en-u-fw-xyz getTimeZones undefined
// CHECK-NEXT: en-u-fw-xyz getTextInfo direction=ltr
// CHECK-NEXT: en-u-fw-xyz getWeekInfo {"firstDay":7,"weekend":[6,7]}
