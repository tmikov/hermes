/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes %s | %FileCheck --match-full-lines %s

// Hostile option values for the Intl.Locale constructor, each a RangeError,
// never a crash or an ill-formed tag reaching native code; and maximize and
// minimize round trips on tags with variants, -t- and -x- sequences, and a
// long language.

/// The class of the exception f throws, or 'none'.
function thrown(f) {
  try {
    f();
    return 'none';
  } catch (e) {
    return e.constructor.name;
  }
}

var bad = [
  // A lone surrogate.
  ['lone surrogate', {calendar: '\uD800'}],
  // U+017F LATIN SMALL LETTER LONG S, which some case mappings turn into
  // "s".
  ['long s', {calendar: 'ſtr'}],
  // 10,000 characters with no "-": one subtag far longer than 8.
  ['long subtag', {calendar: 'abcdefghi'.repeat(1112).slice(0, 10000)}],
  // Four letters is not a language subtag.
  ['root', {language: 'root'}],
  ['region 1234', {region: '1234'}],
  ['empty variants', {variants: ''}],
  ['trailing dash', {variants: 'fonipa-'}],
  ['repeated variant', {variants: 'fonipa-fonipa'}],
];
for (var i = 0; i < bad.length; i++)
  print(bad[i][0] + ':', thrown(() => new Intl.Locale('en', bad[i][1])));
// CHECK: lone surrogate: RangeError
// CHECK-NEXT: long s: RangeError
// CHECK-NEXT: long subtag: RangeError
// CHECK-NEXT: root: RangeError
// CHECK-NEXT: region 1234: RangeError
// CHECK-NEXT: empty variants: RangeError
// CHECK-NEXT: trailing dash: RangeError
// CHECK-NEXT: repeated variant: RangeError

// A valid calendar of more than 10,000 characters: 2501 subtags "abc".
var longCalendar = 'abc-'.repeat(2500) + 'abc';
var long = new Intl.Locale('en', {calendar: longCalendar});
print(longCalendar.length, long.calendar === longCalendar);
// CHECK-NEXT: 10003 true
print(long.toString() === 'en-u-ca-' + longCalendar);
// CHECK-NEXT: true

var roundTrips = ['sl-rozaj-biske-1994', 'en-t-ja-x-a', 'posix-u-ca-buddhist'];
for (var j = 0; j < roundTrips.length; j++) {
  var loc = new Intl.Locale(roundTrips[j]);
  print(roundTrips[j], loc.maximize().toString(), loc.minimize().toString());
}
// CHECK-NEXT: sl-rozaj-biske-1994 sl-Latn-SI-1994-biske-rozaj sl-1994-biske-rozaj
// CHECK-NEXT: en-t-ja-x-a en-Latn-US-t-ja-x-a en-t-ja-x-a
// CHECK-NEXT: posix-u-ca-buddhist posix-u-ca-buddhist posix-u-ca-buddhist

// A -t- tlang starts with a unicode_language_subtag, which may have 5-8
// letters, like the language of the tag itself.
var longTlangs = ['en-t-posix', 'fr-t-abcdefgh-latn', 'de-t-Posix-AT'];
for (var k = 0; k < longTlangs.length; k++) {
  var tl = new Intl.Locale(longTlangs[k]);
  print(
    Intl.getCanonicalLocales(longTlangs[k])[0],
    tl.toString(),
    tl.maximize().toString(),
    tl.minimize().toString(),
  );
}
// CHECK-NEXT: en-t-posix en-t-posix en-Latn-US-t-posix en-t-posix
// CHECK-NEXT: fr-t-abcdefgh-latn fr-t-abcdefgh-latn fr-Latn-FR-t-abcdefgh-latn fr-t-abcdefgh-latn
// CHECK-NEXT: de-t-posix-at de-t-posix-at de-Latn-DE-t-posix-at de-t-posix-at
