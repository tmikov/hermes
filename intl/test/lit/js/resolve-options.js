/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: cat %S/Inputs/strict.js %intl_js/01-intrinsics.js %intl_js/02-shapes.js %intl_js/03-locales.js %intl_js/04-options.js %intl_js/05-tags.js %s > %t.js
// RUN: %hermes_default -Xintl-plugin=false %t.js | %FileCheck --match-full-lines %s
// RUN: %hermesc -O -emit-binary -out=%t.hbc %t.js && %hermes_default -Xintl-plugin=false -b %t.hbc | %FileCheck --match-full-lines %s

// ResolveOptions (ECMA-402 9.2.8) in isolation, with stub natives and a fake
// service that has the Collator's resolution option descriptors.

var available = ['en', 'de', 'fr'];
var native = {
  canonicalizeLocale: function (tag) {
    if (!/^[a-z]{2,3}(-[A-Za-z0-9]{1,8})*$/.test(tag)) return undefined;
    return tag;
  },
  isAvailableLocale: function (tag) {
    return available.indexOf(tag) !== -1;
  },
};
var hostDefaults = {locales: ['fr'], timeZone: 'UTC'};
function isLocaleObject(o) {
  return false;
}
function localeObjectTag(o) {
  throw new Error('not a Locale');
}
function thrown(f) {
  try {
    f();
    return 'none';
  } catch (e) {
    return e.constructor.name;
  }
}

var ctor = {
  __proto__: null,
  availableLocales: isAvailableLocale,
  relevantExtensionKeys: ['co', 'kn', 'kf'],
  resolutionOptionDescriptors: [
    {__proto__: null, key: 'co', property: 'collation'},
    {__proto__: null, key: 'kn', property: 'numeric', type: 'boolean'},
    {
      __proto__: null,
      key: 'kf',
      property: 'caseFirst',
      values: ['upper', 'lower', 'false'],
    },
  ],
};
function localeData(locale) {
  return {
    __proto__: null,
    co: [null, 'phonebk', 'eor'],
    kn: ['false', 'true'],
    kf: ['false', 'upper', 'lower'],
  };
}
function resolve(locales, options, special, modify) {
  return ResolveOptions(ctor, localeData, locales, options, special, modify);
}
/// Reads logged through a Proxy.
function logged(target, log) {
  return new Proxy(target, {
    get: function (t, k) {
      log.push(k);
      return t[k];
    },
  });
}

// Descriptors are read in order, after localeMatcher.
var log = [];
var r = resolve(['en'], logged({}, log));
print(log.join(' '));
// CHECK: localeMatcher collation numeric caseFirst
print(JSON.stringify(r.resolutionOptions));
// CHECK-NEXT: {"localeMatcher":"best fit"}
print(Reflect.ownKeys(r.resolutionOptions).join(' '));
// CHECK-NEXT: localeMatcher co kn kf
print(
  r.resolvedLocale.locale,
  r.resolvedLocale.co,
  r.resolvedLocale.kn,
  r.resolvedLocale.kf,
);
// CHECK-NEXT: en null false false

// A boolean option becomes the string "true" or "false".
var kn = resolve(['en'], {numeric: true}).resolutionOptions.kn;
print(typeof kn, kn);
// CHECK-NEXT: string true
kn = resolve(['en'], {numeric: false}).resolutionOptions.kn;
print(typeof kn, kn);
// CHECK-NEXT: string false
print(resolve(['en'], {numeric: 0}).resolvedLocale.kn);
// CHECK-NEXT: false
print(resolve(['en'], {numeric: 1}).resolvedLocale.kn);
// CHECK-NEXT: true

// The type check: not a Unicode type is a RangeError.
print(thrown(() => resolve(['en'], {collation: 'a b'})));
// CHECK-NEXT: RangeError
print(thrown(() => resolve(['en'], {collation: ''})));
// CHECK-NEXT: RangeError
print(thrown(() => resolve(['en'], {collation: 'abcdefghi'})));
// CHECK-NEXT: RangeError
print(resolve(['en'], {collation: 'eor'}).resolvedLocale.co);
// CHECK-NEXT: eor
// GetOption's own list check.
print(thrown(() => resolve(['en'], {caseFirst: 'x'})));
// CHECK-NEXT: RangeError
print(thrown(() => resolve(['en'], {localeMatcher: 'x'})));
// CHECK-NEXT: RangeError
print(resolve(['en'], {caseFirst: 'upper'}).resolvedLocale.kf);
// CHECK-NEXT: upper

// Option handling.
print(thrown(() => resolve(['en'], 5)));
// CHECK-NEXT: TypeError
print(resolve(['en'], 5, ['coerce-options']).options instanceof Number);
// CHECK-NEXT: true
print(thrown(() => resolve(['en'], undefined, ['require-options'])));
// CHECK-NEXT: TypeError
print(thrown(() => resolve(['en'], undefined, ['coerce-options'])));
// CHECK-NEXT: none
print(Object.getPrototypeOf(resolve(['en'], undefined).options));
// CHECK-NEXT: null
// require-options does not reject an object.
print(thrown(() => resolve(['en'], {}, ['require-options'])));
// CHECK-NEXT: none
// Locales are canonicalized first: a bad tag fails before options are read.
log = [];
print(thrown(() => resolve(['x y'], logged({}, log))), log.length);
// CHECK-NEXT: RangeError 0

// modifyResolutionOptions sees opt before ResolveLocale.
var seen, seenKeys;
r = resolve(['en'], {collation: 'eor'}, undefined, function (opt) {
  seen = JSON.stringify(opt);
  seenKeys = Reflect.ownKeys(opt).join(' ');
  opt.kn = 'true';
});
print(seen);
// CHECK-NEXT: {"localeMatcher":"best fit","co":"eor"}
print(seenKeys);
// CHECK-NEXT: localeMatcher co kn kf
print(r.resolvedLocale.kn, r.resolvedLocale.co);
// CHECK-NEXT: true eor

// A requested extension is kept when the locale data supports it.
print(resolve(['de-u-co-phonebk']).resolvedLocale.locale);
// CHECK-NEXT: de-u-co-phonebk
print(resolve(['de-u-co-nope']).resolvedLocale.locale);
// CHECK-NEXT: de
