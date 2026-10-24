/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes %s | %FileCheck --match-full-lines %s

// User code that replaces intrinsics before its first call to Intl does not
// change what Intl returns or throws. Nothing calls Intl before the
// tampering, so a value the plugin caches on a first call cannot hide a use
// of a replaced intrinsic; the results are compared with literal values.

// Everything this test uses after the tampering is captured first.
var define = Object.defineProperty;
var ObjectPrototype = Object.prototype;
var OriginalRangeError = RangeError;
var OriginalTypeError = TypeError;
var stringify = JSON.stringify;
var aSymbol = Symbol('x');
var boom = function () {
  throw new Error('tampered intrinsic was used');
};
/// "ok" if f throws an instance of the original constructor expected (not
/// whatever the global now holds), or what went wrong.
function thrown(f, expected) {
  try {
    f();
    return 'none';
  } catch (e) {
    return e.constructor === expected ? 'ok' : 'wrong error: ' + e;
  }
}

Array.prototype[Symbol.iterator] = boom;
Array.prototype.push = boom;
Array.prototype.indexOf = boom;
Array.prototype.slice = boom;
String.prototype[Symbol.split] = boom;
// A string separator does not consult @@split (ECMA-262 22.1.3.23 step 3),
// so split itself is replaced too.
String.prototype.split = boom;
String.prototype.slice = boom;
String.prototype.indexOf = boom;
String.prototype.charCodeAt = boom;
String.prototype.toLowerCase = boom;
String.fromCharCode = boom;
Function.prototype.call = boom;
Function.prototype.apply = boom;
Reflect.apply = boom;
Reflect.ownKeys = boom;
Object.create = boom;
Object.defineProperty = boom;
Object.getOwnPropertyDescriptor = boom;
Math.min = boom;
Math.trunc = boom;
Number.isFinite = boom;
define(Array.prototype, '0', {__proto__: null, set: boom, get: boom, configurable: true});
define(Array.prototype, '1', {__proto__: null, set: boom, get: boom, configurable: true});
// The globals 01-intrinsics.js captures from, except String, whose
// functions are replaced above.
var globals = ['Object', 'RangeError', 'TypeError', 'Reflect', 'Math', 'Number', 'Symbol', 'Array'];
for (var i = 0; i < globals.length; i++) globalThis[globals[i]] = boom;
// Last, because it breaks ordinary descriptor objects: accessors for the
// descriptor fields on Object.prototype.
var fields = ['value', 'writable', 'enumerable', 'configurable', 'get', 'set'];
for (var j = 0; j < fields.length; j++)
  define(ObjectPrototype, fields[j], {__proto__: null, get: boom, configurable: true});

print(stringify(Intl.getCanonicalLocales(['EN-us', 'de', 'EN-us'])));
// CHECK: ["en-US","de"]
// ToObject wraps a primitive with the captured Object.
print(stringify(Intl.getCanonicalLocales(5)));
// CHECK-NEXT: []
print(stringify(Intl.supportedValuesOf('calendar')));
// CHECK-NEXT: ["buddhist","chinese","coptic","dangi","ethioaa","ethiopic","gregory","hebrew","indian","islamic-civil","islamic-tbla","islamic-umalqura","iso8601","japanese","persian","roc"]
print(stringify(Intl.supportedValuesOf('collation')));
// CHECK-NEXT: ["compat","dict","emoji","eor","phonebk","pinyin","stroke","trad","unihan","zhuyin"]
var units = Intl.supportedValuesOf('unit');
print(units.length, units[0], units[1], units[44]);
// CHECK-NEXT: 45 acre bit year

// The errors come from the intrinsics, not from the replaced globals.
print(thrown(() => Intl.supportedValuesOf('x'), OriginalRangeError));
// CHECK-NEXT: ok
print(thrown(() => Intl.supportedValuesOf(aSymbol), OriginalTypeError));
// CHECK-NEXT: ok
print(thrown(() => Intl.getCanonicalLocales(['']), OriginalRangeError));
// CHECK-NEXT: ok
print(thrown(() => Intl.getCanonicalLocales([null]), OriginalTypeError));
// CHECK-NEXT: ok
print(thrown(() => Intl.getCanonicalLocales(null), OriginalTypeError));
// CHECK-NEXT: ok
