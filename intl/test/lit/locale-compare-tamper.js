/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US %s | %FileCheck --match-full-lines %s

// Tampering after the String overrides have filled their caches changes
// none of their results. tamper.js tampers before the first call; this
// test calls first, so the later calls take the paths that reuse what the
// first calls cached (the collators shared by localeCompare, the locale
// data of each locale, the default collator and the default case-mapping
// locale), and then the paths that add to those caches and empty a full
// one.

// Everything this test uses after the tampering is captured first.
var define = Object.defineProperty;
var ObjectPrototype = Object.prototype;
var apply = Reflect.apply;
var OriginalRangeError = RangeError;
var OriginalCollator = Intl.Collator;
var CollatorPrototype = Intl.Collator.prototype;
var getCompare = Object.getOwnPropertyDescriptor(CollatorPrototype, 'compare').get;
var boom = function () {
  throw new Error('tampered intrinsic was used');
};
/// "ok" if f throws an instance of the original constructor expected, or
/// what went wrong.
function thrown(f, expected) {
  try {
    f();
    return 'none';
  } catch (e) {
    return e.constructor === expected ? 'ok' : 'wrong error: ' + e;
  }
}
/// The bound compare of a new collator for locale and options, through the
/// references captured before the tampering.
function compareOf(locale, options) {
  return apply(getCompare, new OriginalCollator(locale, options), []);
}

/// The pairs each option set compares, and their results under compare
/// (one of localeCompare's or a collator's), as one string.
var pairs = [['a', 'A'], ['a', 'á'], ['2', '10'], ['a-b', 'ab'], ['ä', 'z'], ['å', 'z'], ['b', 'A']];
function results(compare) {
  var s = '';
  for (var i = 0; i < pairs.length; i++) s += (i === 0 ? '' : ' ') + compare(pairs[i][0], pairs[i][1]);
  return s;
}
/// An options object with every option localeCompare reads, so that none
/// is read from the tampered Object.prototype.
function fullOptions(sensitivity, caseFirst, numeric, ignorePunctuation) {
  return {
    localeMatcher: 'lookup',
    usage: 'sort',
    collation: 'default',
    numeric: numeric,
    caseFirst: caseFirst,
    sensitivity: sensitivity,
    ignorePunctuation: ignorePunctuation,
  };
}

// The calls that fill the caches.
var deOptions = fullOptions('base', 'false', true, false);
var enOptions = fullOptions('variant', 'upper', false, true);
var before =
  results((x, y) => x.localeCompare(y, 'de', deOptions)) +
  ' / ' +
  results((x, y) => x.localeCompare(y, ['en'], enOptions)) +
  ' / ' +
  results((x, y) => x.localeCompare(y)) +
  ' / ' +
  'i'.toLocaleUpperCase() +
  'I'.toLocaleLowerCase();
new OriginalCollator('de');
print(before);
// CHECK: 0 0 -1 -1 -1 -1 1 / 1 -1 1 0 -1 -1 1 / -1 -1 1 -1 -1 -1 1 / Ii

// The tampering, as in tamper.js.
Array.prototype[Symbol.iterator] = boom;
Array.prototype.push = boom;
Array.prototype.indexOf = boom;
Array.prototype.slice = boom;
Array.prototype.join = boom;
String.prototype[Symbol.split] = boom;
String.prototype.split = boom;
String.prototype.slice = boom;
String.prototype.indexOf = boom;
String.prototype.charCodeAt = boom;
String.prototype.toLowerCase = boom;
String.prototype.toUpperCase = boom;
String.prototype.concat = boom;
Function.prototype.call = boom;
Function.prototype.apply = boom;
Reflect.apply = boom;
Reflect.ownKeys = boom;
Object.create = boom;
Object.defineProperty = boom;
Object.getOwnPropertyDescriptor = boom;
define(Array.prototype, '0', {__proto__: null, set: boom, get: boom, configurable: true});
define(Array.prototype, '1', {__proto__: null, set: boom, get: boom, configurable: true});
CollatorPrototype.resolvedOptions = boom;
define(CollatorPrototype, 'compare', {__proto__: null, get: boom, configurable: true});
// The fields of the internal records (as in tamper.js), and the locales
// first used below: a cache keyed by locale must not find them on
// Object.prototype.
var tamperedNames = [
  'locale', 'key', 'base', 'extension', 'localeMatcher', 'co', 'kn', 'kf',
  'sensitivity', 'ignorePunctuation', 'property', 'type', 'values',
  'options', 'resolvedLocale', 'resolutionOptions', 'localeData',
  'attributes', 'keywords', 'fr', 'sv', 'da', 'nb', 'fi',
];
for (var k = 0; k < tamperedNames.length; k++)
  define(ObjectPrototype, tamperedNames[k], {__proto__: null, get: boom, set: boom, configurable: true});
var globals = ['Object', 'RangeError', 'TypeError', 'Reflect', 'Math', 'Number', 'Symbol', 'Array'];
for (var g = 0; g < globals.length; g++) globalThis[globals[g]] = boom;
Intl.Collator = boom;
var fields = ['value', 'writable', 'enumerable', 'configurable', 'get', 'set'];
for (var j = 0; j < fields.length; j++)
  define(ObjectPrototype, fields[j], {__proto__: null, get: boom, configurable: true});

// The same calls give the same results, now from the caches.
var after =
  results((x, y) => x.localeCompare(y, 'de', deOptions)) +
  ' / ' +
  results((x, y) => x.localeCompare(y, ['en'], enOptions)) +
  ' / ' +
  results((x, y) => x.localeCompare(y)) +
  ' / ' +
  'i'.toLocaleUpperCase() +
  'I'.toLocaleLowerCase();
print(after === before);
// CHECK-NEXT: true

// Locales first used now, and more option sets than the cache of shared
// collators holds, so it is filled and emptied again. Each localeCompare
// call comes before the collator made to check it, so localeCompare is the
// first to need the locale data of the new locales.
var newLocales = ['fr', 'sv', 'da', 'nb', 'fi'];
var sensitivities = ['base', 'accent', 'case', 'variant'];
var caseFirsts = ['upper', 'lower', 'false'];
var sets = 0;
var wrong = 0;
for (var l = 0; l < newLocales.length; l++) {
  for (var s = 0; s < sensitivities.length; s++) {
    for (var c = 0; c < caseFirsts.length; c++) {
      var options = fullOptions(sensitivities[s], caseFirsts[c], s % 2 === 0, c % 2 === 0);
      var locale = newLocales[l];
      var actual = results((x, y) => x.localeCompare(y, locale, options));
      if (actual !== results(compareOf(locale, options))) wrong++;
      sets++;
    }
  }
}
print(sets, 'option sets,', wrong, 'wrong');
// CHECK-NEXT: 60 option sets, 0 wrong
// Two of them with literal results: Swedish sorts "ä" and "å" after "z".
print(results((x, y) => x.localeCompare(y, 'sv', fullOptions('base', 'false', false, false))));
// CHECK-NEXT: 0 0 1 -1 1 1 1
print(results((x, y) => x.localeCompare(y, 'fr', fullOptions('variant', 'upper', true, true))));
// CHECK-NEXT: 1 -1 -1 0 -1 -1 1
// The errors still come from the intrinsics.
print(thrown(() => 'a'.localeCompare('b', 'de', fullOptions('x', 'false', false, false)), OriginalRangeError));
// CHECK-NEXT: ok
