/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// Every intrinsic the implementation uses, captured when the plugin's
// bytecode runs, before any user code; and the ECMA-262 conversions built on
// them. Plugin code calls only these. ECMA-402 draft of October 24, 2026;
// ECMA-262 section numbers are from the https://tc39.es/ecma262/ draft of
// the same date.

var global = globalThis;
var _Object = Object;
var ObjectCreate = Object.create;
var ObjectDefineProperty = Object.defineProperty;
var ObjectGetOwnPropertyDescriptor = Object.getOwnPropertyDescriptor;
var ReflectApply = Reflect.apply;
var ReflectOwnKeys = Reflect.ownKeys;
var _TypeError = TypeError;
var _RangeError = RangeError;
var SymbolToStringTag = Symbol.toStringTag;
var MathFloor = Math.floor;
var MathMax = Math.max;
var MathMin = Math.min;
var MathTrunc = Math.trunc;
var NumberIsFinite = Number.isFinite;
var StringFromCharCode = String.fromCharCode;
var StringPrototypeCharCodeAt = String.prototype.charCodeAt;
var StringPrototypeIndexOf = String.prototype.indexOf;
var StringPrototypeSlice = String.prototype.slice;

/// s.indexOf(search, from) through the captured method. This is ECMA-262
/// 6.1.4.1 StringIndexOf, with -1 for not-found.
function stringIndexOf(s, search, from) {
  // 1. Return ! Call(%String.prototype.indexOf%, s, « search, from »).
  return ReflectApply(StringPrototypeIndexOf, s, [search, from]);
}

/// s.slice(start, end) through the captured method: the substring of s from
/// start to end, or to the end of s when end is undefined.
function stringSlice(s, start, end) {
  // 1. Return ! Call(%String.prototype.slice%, s, « start, end »).
  return ReflectApply(StringPrototypeSlice, s, [start, end]);
}

/// s.charCodeAt(i) through the captured method: the code unit at index i.
function charCodeAt(s, i) {
  // 1. Return ! Call(%String.prototype.charCodeAt%, s, « i »).
  return ReflectApply(StringPrototypeCharCodeAt, s, [i]);
}

// ECMA-262 7.1.19 ToString ( arg )
function ToString(v) {
  // 1-12. A template literal performs exactly ToString, including the
  //       TypeError for symbols (step 2) and ToPrimitive with hint string
  //       for objects (step 10). '' + v would use hint default instead.
  return `${v}`;
}

// ECMA-262 7.1.20 ToObject ( arg )
function ToObject(v) {
  // 1. If arg is either undefined or null, throw a TypeError exception.
  if (v === undefined || v === null)
    throw new _TypeError('Cannot convert undefined or null to object');
  // 2-8. Wrap a primitive in a new wrapper object; return an object as is.
  return _Object(v);
}

// ECMA-262 7.1.4 ToNumber ( arg )
function ToNumber(v) {
  // 1-8. Unary plus performs exactly ToNumber, including the TypeError for
  //      BigInts and symbols.
  return +v;
}

// ECMA-262 7.1.5 ToIntegerOrInfinity ( arg )
function ToIntegerOrInfinity(v) {
  // 1. Let number be ? ToNumber(arg).
  var number = ToNumber(v);
  // 2. If number is one of NaN, +0𝔽, or -0𝔽, return 0.
  if (number !== number || number === 0) return 0;
  // 3-4. If number is +∞𝔽 or -∞𝔽, return +∞ or -∞.
  if (!NumberIsFinite(number)) return number;
  // 5. Return truncate(ℝ(number)).
  return MathTrunc(number);
}

// ECMA-262 7.1.22 ToLength ( arg )
function ToLength(v) {
  // 1. Let length be ? ToIntegerOrInfinity(arg).
  var length = ToIntegerOrInfinity(v);
  // 2. Let clampedLength be the result of clamping length between 0 and
  //    2^53 - 1.
  // 3. Return 𝔽(clampedLength).
  if (length <= 0) return 0;
  return MathMin(length, 9007199254740991);
}

/// The ASCII-lowercase of s (ECMA-402 6.1): only the code units A-Z change.
function asciiLowercase(s) {
  // 1. Replace each code unit from 0x41 to 0x5A with the one 0x20 above it,
  //    and keep every other code unit.
  var out = '';
  for (var i = 0; i < s.length; i++) {
    var c = charCodeAt(s, i);
    out += StringFromCharCode(c >= 0x41 && c <= 0x5a ? c + 0x20 : c);
  }
  return out;
}
