/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: cat %S/Inputs/strict.js %intl_js/01-intrinsics.js %intl_js/02-shapes.js %intl_js/03-locales.js %intl_js/04-options.js %s > %t.js
// RUN: %hermes_default -Xintl-plugin=false %t.js | %FileCheck --match-full-lines %s
// RUN: %hermesc -O -emit-binary -out=%t.hbc %t.js && %hermes_default -Xintl-plugin=false -b %t.hbc | %FileCheck --match-full-lines %s

// ResolveLocale, DefaultLocale and FilterLocales (ECMA-402 6.2.3, 9.2.3,
// 9.2.7, 9.2.9) in isolation, with stub natives.

// Stubs for what the plugin's C++ passes to setup.
var available = ['en', 'en-US', 'de', 'fr'];
var native = {
  canonicalizeLocale: function (tag) {
    if (!/^[a-z]{2,3}(-[A-Za-z0-9]{1,8})*$/.test(tag)) return undefined;
    // One UTS 35 value alias, so that callers which read a value back from
    // the canonical tag see it change.
    return tag.replace(/-u-ca-islamicc(?=-|$)/, '-u-ca-islamic-civil');
  },
  isAvailableLocale: function (tag) {
    return available.indexOf(tag) !== -1;
  },
};
// The loader passes the defaults already reduced to available locales,
// most preferred first; DefaultLocale takes the first.
var hostDefaults = {locales: ['fr', 'de'], timeZone: 'UTC'};
/// The class of the exception f throws, or 'none'.
function thrown(f) {
  try {
    f();
    return 'none';
  } catch (e) {
    return e.constructor.name;
  }
}

function localeData(locale) {
  return {
    __proto__: null,
    ca: ['gregory', 'buddhist'],
    kn: ['false', 'true'],
    tag: 'data for ' + locale,
  };
}
// Records which prefixes lookup asks about.
var asked = [];
function isAvailableLogged(tag) {
  asked.push(tag);
  return isAvailableLocale(tag);
}
function resolve(locales, options) {
  var r = ResolveLocale(isAvailableLogged, locales, options, ['ca', 'kn'], localeData);
  return r.locale + ' ca=' + r.ca + ' kn=' + r.kn + ' (' + r.localeData.tag + ')';
}
var none = {__proto__: null, ca: undefined, kn: undefined};

print(resolve(['en-GB'], none));
// CHECK: en ca=gregory kn=false (data for en)
print(resolve(['xx'], none));
// CHECK-NEXT: fr ca=gregory kn=false (data for fr)
print(resolve(['de-u-ca-buddhist'], none));
// CHECK-NEXT: de-u-ca-buddhist ca=buddhist kn=false (data for de)
print(resolve(['de-u-ca-chinese'], none));
// CHECK-NEXT: de ca=gregory kn=false (data for de)
print(resolve(['de-u-ca-buddhist'], {__proto__: null, ca: 'gregory', kn: undefined}));
// CHECK-NEXT: de ca=gregory kn=false (data for de)
print(resolve(['en-u-kn'], none));
// CHECK-NEXT: en-u-kn ca=gregory kn=true (data for en)
// 12.j.ii: an option value goes through CanonicalizeUValue.
print(resolve(['de'], {__proto__: null, ca: 'BUDDHIST', kn: undefined}));
// CHECK-NEXT: de ca=buddhist kn=false (data for de)
// 12.j.iii: an empty option value means "true".
print(resolve(['en'], {__proto__: null, ca: undefined, kn: ''}));
// CHECK-NEXT: en ca=gregory kn=true (data for en)
// 12.k: an option equal to the requested keyword keeps the keyword.
print(resolve(['de-u-ca-buddhist'], {__proto__: null, ca: 'buddhist', kn: undefined}));
// CHECK-NEXT: de-u-ca-buddhist ca=buddhist kn=false (data for de)
asked = [];
print(resolve(['en-US-x-foo'], none));
// CHECK-NEXT: en-US ca=gregory kn=false (data for en-US)
// Lookup skips the dangling singleton: it never asks about "en-US-x".
print(asked.join(' '));
// CHECK-NEXT: en-US-x-foo en-US
print(resolve(['xx', 'de-AT'], none));
// CHECK-NEXT: de ca=gregory kn=false (data for de)
var noData = ResolveLocale(isAvailableLocale, ['de'], none, [], undefined);
print(noData.locale, noData.localeData);
// CHECK-NEXT: de undefined

print(DefaultLocale());
// CHECK-NEXT: fr

print(JSON.stringify(FilterLocales(isAvailableLocale, ['de-AT', 'xx', 'en-u-ca-gregory'], undefined)));
// CHECK-NEXT: ["de-AT","en-u-ca-gregory"]
print(JSON.stringify(FilterLocales(isAvailableLocale, ['de-AT', 'xx'], {localeMatcher: 'lookup'})));
// CHECK-NEXT: ["de-AT"]
print(thrown(() => FilterLocales(isAvailableLocale, [], {localeMatcher: 'nope'})));
// CHECK-NEXT: RangeError
print(thrown(() => FilterLocales(isAvailableLocale, [], null)));
// CHECK-NEXT: TypeError
