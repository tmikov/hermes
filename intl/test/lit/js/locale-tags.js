/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: cat %S/Inputs/strict.js %intl_js/01-intrinsics.js %intl_js/02-shapes.js %intl_js/03-locales.js %intl_js/04-options.js %intl_js/05-tags.js %s > %t.js
// RUN: %hermes_default -Xintl-plugin=false %t.js | %FileCheck --match-full-lines %s
// RUN: %hermesc -O -emit-binary -out=%t.hbc %t.js && %hermes_default -Xintl-plugin=false -b %t.hbc | %FileCheck --match-full-lines %s

// The Unicode locale grammar checks and the Locale abstract operations
// (ECMA-402 "Abstract Operations for Locale Objects") in isolation, with
// stub natives.

// Stubs for what the plugin's C++ passes to setup. The canonicalizer
// lowercases the language, uppercases regions and title-cases scripts.
var hostDefaults = {locales: ['en-US'], timeZone: 'UTC'};
// 20-Locale.js is not included, so no object is an Intl.Locale.
function isLocaleObject(o) {
  return false;
}
function localeObjectTag(o) {
  throw new Error('not a Locale');
}
var native = {
  canonicalizeLocale: function (tag) {
    var parts = tag.split('-');
    for (var i = 0; i < parts.length; i++) {
      var p = parts[i];
      if (i === 0) parts[i] = p.toLowerCase();
      else if (p.length === 2 && /^[A-Za-z]+$/.test(p)) parts[i] = p.toUpperCase();
      else if (p.length === 4 && /^[A-Za-z]+$/.test(p))
        parts[i] = p[0].toUpperCase() + p.slice(1).toLowerCase();
      else parts[i] = p.toLowerCase();
    }
    return parts.join('-');
  },
  addLikelySubtags: function (tag) {
    if (tag === 'en' || tag === 'und') return 'en-Latn-US';
    if (tag.indexOf('en-u-') === 0) return 'en-Latn-US' + tag.slice(2);
    return tag;
  },
};

function check(label, got, want) {
  print(label + ': ' + (got === want ? 'ok' : 'FAIL got ' + got + ' want ' + want));
}
function accepts(f, s) {
  return f(s) === true;
}

// Grammar: each check against its own inputs.
function grammar(name, f, yes, no) {
  for (var i = 0; i < yes.length; i++) check(name + ' accepts ' + yes[i], f(yes[i]), true);
  for (var i = 0; i < no.length; i++) check(name + ' rejects ' + no[i], f(no[i]), false);
}
grammar('language', isUnicodeLanguageSubtag, ['en', 'haw', 'posix'], ['e', 'engl', 'english12']);
// CHECK: language accepts en: ok
// CHECK-NEXT: language accepts haw: ok
// CHECK-NEXT: language accepts posix: ok
// CHECK-NEXT: language rejects e: ok
// CHECK-NEXT: language rejects engl: ok
// CHECK-NEXT: language rejects english12: ok
grammar('script', isUnicodeScriptSubtag, ['Latn'], ['Lat', 'Latin']);
// CHECK-NEXT: script accepts Latn: ok
// CHECK-NEXT: script rejects Lat: ok
// CHECK-NEXT: script rejects Latin: ok
grammar('region', isUnicodeRegionSubtag, ['US', '419'], ['U', 'USA', '4190']);
// CHECK-NEXT: region accepts US: ok
// CHECK-NEXT: region accepts 419: ok
// CHECK-NEXT: region rejects U: ok
// CHECK-NEXT: region rejects USA: ok
// CHECK-NEXT: region rejects 4190: ok
grammar('variant', isUnicodeVariantSubtag, ['fonipa', '1994'], ['abcd', 'fonipa123']);
// CHECK-NEXT: variant accepts fonipa: ok
// CHECK-NEXT: variant accepts 1994: ok
// CHECK-NEXT: variant rejects abcd: ok
// CHECK-NEXT: variant rejects fonipa123: ok
grammar('type', isUnicodeType, ['abc', 'buddhist', 'islamic-civil'], ['ab', 'abc-', '-abc', 'abc--abc']);
// CHECK-NEXT: type accepts abc: ok
// CHECK-NEXT: type accepts buddhist: ok
// CHECK-NEXT: type accepts islamic-civil: ok
// CHECK-NEXT: type rejects ab: ok
// CHECK-NEXT: type rejects abc-: ok
// CHECK-NEXT: type rejects -abc: ok
// CHECK-NEXT: type rejects abc--abc: ok
grammar('subdivision', isUnicodeSubdivisionId, ['gbzzzz', 'usca'], ['gb', 'gbzzzzz', 'u']);
// CHECK-NEXT: subdivision accepts gbzzzz: ok
// CHECK-NEXT: subdivision accepts usca: ok
// CHECK-NEXT: subdivision rejects gb: ok
// CHECK-NEXT: subdivision rejects gbzzzzz: ok
// CHECK-NEXT: subdivision rejects u: ok

// Non-ASCII look-alikes and a lone surrogate match nothing.
var checks = [
  isUnicodeLanguageSubtag, isUnicodeScriptSubtag, isUnicodeRegionSubtag,
  isUnicodeVariantSubtag, isUnicodeType, isUnicodeSubdivisionId,
];
var hostile = ['ı', 'ſ', '\ud800', 'abı', 'abſſ', 'ab\ud800', 'ſabc', 'abcı'];
var any = 0;
for (var i = 0; i < checks.length; i++)
  for (var j = 0; j < hostile.length; j++) if (checks[i](hostile[j])) any++;
check('non-ASCII matches', any, 0);
// CHECK-NEXT: non-ASCII matches: ok
// A case-insensitive match for ASCII only.
check('upper case', isUnicodeScriptSubtag('LATN') && isUnicodeRegionSubtag('us'), true);
// CHECK-NEXT: upper case: ok

// A very long type value is linear time.
var long = 'abc';
while (long.length < 10000) long += '-abc';
var t0 = Date.now();
var typeOk = isUnicodeType(long);
var others = isUnicodeLanguageSubtag(long) || isUnicodeScriptSubtag(long) ||
  isUnicodeRegionSubtag(long) || isUnicodeVariantSubtag(long) ||
  isUnicodeSubdivisionId(long);
check('long type', typeOk, true);
check('long others', others, false);
check('long fast', Date.now() - t0 < 1000, true);
// CHECK-NEXT: long type: ok
// CHECK-NEXT: long others: ok
// CHECK-NEXT: long fast: ok

function show(v) {
  return v === undefined ? 'undefined' : '"' + v + '"';
}

// StringSplitToList.
var parts = StringSplitToList('a--b', '-');
print('split ' + parts.length + ' ' + show(parts[0]) + show(parts[1]) + show(parts[2]));
// CHECK-NEXT: split 3 "a""""b"
print('split ' + StringSplitToList('', '-').length + ' ' + StringSplitToList('abc', '-').length);
// CHECK-NEXT: split 1 1
print('split ' + StringSplitToList('a-b-', '-').length + ' ' + StringSplitToList('a<>b', '<>').length);
// CHECK-NEXT: split 3 2

// GetLocaleBaseName and friends.
print(GetLocaleBaseName('en-Latn-US-fonipa-u-ca-x-a'));
// CHECK-NEXT: en-Latn-US-fonipa
print(GetLocaleBaseName('en-x-a'));
// CHECK-NEXT: en
print(GetLocaleBaseName('en-t-ja'));
// CHECK-NEXT: en
print(GetLocaleBaseName('en-Latn-US'));
// CHECK-NEXT: en-Latn-US
print(GetLocaleLanguage('posix-u-ca-buddhist') + ' ' + GetLocaleLanguage('en-US'));
// CHECK-NEXT: posix en
print(show(GetLocaleScript('en-US')) + ' ' + show(GetLocaleScript('sr-Cyrl-RS')));
// CHECK-NEXT: undefined "Cyrl"
print(show(GetLocaleRegion('en-Latn')) + ' ' + show(GetLocaleRegion('es-419')) + ' ' + show(GetLocaleRegion('en-Latn-US')));
// CHECK-NEXT: undefined "419" "US"
print(show(GetLocaleRegion('en-u-rg-gbzzzz')));
// CHECK-NEXT: undefined
print(show(GetLocaleVariants('sl-rozaj-biske')) + ' ' + show(GetLocaleVariants('sl-IT-rozaj')) + ' ' + show(GetLocaleVariants('en')));
// CHECK-NEXT: "rozaj-biske" "rozaj" undefined
print(show(GetLocaleVariants('sl-Latn-IT-rozaj-1994-u-ca-gregory')));
// CHECK-NEXT: "rozaj-1994"
print(show(GetLocaleVariants('en-US-1994')) + ' ' + show(GetLocaleVariants('sl-rozaj')));
// CHECK-NEXT: "1994" "rozaj"
print(GetLocaleBaseName('en-US-1994-u-ca-gregory') + ' ' + GetLocaleBaseName('sl-rozaj-t-ja'));
// CHECK-NEXT: en-US-1994 sl-rozaj

// UnicodeExtensionValue: undefined stands for ~empty~.
print(show(UnicodeExtensionValue('en-u-ca-gregory', 'ca')) + ' ' + show(UnicodeExtensionValue('en-u-kn', 'kn')) + ' ' + show(UnicodeExtensionValue('en', 'ca')));
// CHECK-NEXT: "gregory" "" undefined
print(show(UnicodeExtensionValue('en-u-ca-gregory-nu-latn', 'nu')) + ' ' + show(UnicodeExtensionValue('en-x-u-ca-gregory', 'ca')));
// CHECK-NEXT: "latn" undefined

// CanonicalUnicodeSubdivision.
print(show(CanonicalUnicodeSubdivision('en-u-rg-gbzzzz', 'rg')) + ' ' + show(CanonicalUnicodeSubdivision('en-u-rg-gb', 'rg')) + ' ' + show(CanonicalUnicodeSubdivision('en-u-sd-419abc', 'sd')));
// CHECK-NEXT: "GB" undefined "419"
print(show(CanonicalUnicodeSubdivision('en', 'rg')) + ' ' + show(CanonicalUnicodeSubdivision('en-u-rg-gbzzzzz', 'rg')));
// CHECK-NEXT: undefined undefined

// RegionPreference.
function pref(tag) {
  var p = RegionPreference(tag);
  return show(p.region) + ' ' + show(p.regionOverride) + ' ' + (Object.getPrototypeOf(p) === null);
}
print(pref('en'));
// CHECK-NEXT: "US" undefined true
print(pref('zxx'));
// CHECK-NEXT: "001" undefined true
print(pref('en-u-rg-gbzzzz'));
// CHECK-NEXT: "US" "GB" true
print(pref('en-u-sd-gbsct'));
// CHECK-NEXT: "GB" undefined true
print(pref('en-AU'));
// CHECK-NEXT: "AU" undefined true

// WeekdayToUValue and WeekdayUValueToNumber.
print(['0', '7', '1', '6', 'mon', 'x'].map(WeekdayToUValue).join(' '));
// CHECK-NEXT: sun sun mon sat mon x
print(['sun', 'mon', 'sat', '0', 'x'].map(function (s) { return show(WeekdayUValueToNumber(s)); }).join(' '));
// CHECK-NEXT: "7" "1" "6" undefined undefined
