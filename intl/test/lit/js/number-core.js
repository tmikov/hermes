/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: cat %S/Inputs/strict.js %intl_js/01-intrinsics.js %intl_js/02-shapes.js %intl_js/03-locales.js %intl_js/04-options.js %intl_js/05-tags.js %intl_js/07-number.js %s > %t.js
// RUN: %hermes_default -Xintl-plugin=false %t.js | %FileCheck --match-full-lines %s
// RUN: %hermesc -O -emit-binary -out=%t.hbc %t.js && %hermes_default -Xintl-plugin=false -b %t.hbc | %FileCheck --match-full-lines %s

// Intl mathematical values and rounding (07-number.js) in isolation:
// ToIntlMathematicalValue, ToRawPrecision, ToRawFixed,
// FormatNumericToString and ComputeExponent.

/// JSON of v, with BigInts as strings.
function show(v) {
  return JSON.stringify(v, function (k, x) {
    return typeof x === 'bigint' ? String(x) : x;
  });
}
/// The class of the exception f throws, or 'none'.
function thrown(f) {
  try {
    f();
    return 'none';
  } catch (e) {
    return e.constructor.name;
  }
}
function mv(v) {
  return show(ToIntlMathematicalValue(v));
}

// ToIntlMathematicalValue of Numbers.
print(mv(1.5));
// CHECK: {"neg":false,"n":"15","e":-1}
print(mv(-1.5));
// CHECK-NEXT: {"neg":true,"n":"15","e":-1}
print(mv(-0));
// CHECK-NEXT: "negZero"
print(mv(0));
// CHECK-NEXT: {"neg":false,"n":"0","e":0}
print(mv(1e21));
// CHECK-NEXT: {"neg":false,"n":"1","e":21}
print(mv(5e-324));
// CHECK-NEXT: {"neg":false,"n":"5","e":-324}
print(mv(1e308));
// CHECK-NEXT: {"neg":false,"n":"1","e":308}
print(mv(1200));
// CHECK-NEXT: {"neg":false,"n":"12","e":2}
print(mv(0.1));
// CHECK-NEXT: {"neg":false,"n":"1","e":-1}
print(mv(NaN), mv(Infinity), mv(-Infinity));
// CHECK-NEXT: "nan" "posInf" "negInf"
print(mv(true), mv(false), mv(null), mv(undefined));
// CHECK-NEXT: {"neg":false,"n":"1","e":0} {"neg":false,"n":"0","e":0} {"neg":false,"n":"0","e":0} "nan"

// BigInts are exact.
print(mv(123n));
// CHECK-NEXT: {"neg":false,"n":"123","e":0}
print(mv(-(2n ** 70n)));
// CHECK-NEXT: {"neg":true,"n":"1180591620717411303424","e":0}
print(mv(1000n), mv(0n));
// CHECK-NEXT: {"neg":false,"n":"1","e":3} {"neg":false,"n":"0","e":0}
print(mv(10n ** 400n));
// CHECK-NEXT: {"neg":false,"n":"1","e":400}

// Strings, by the StringNumericLiteral grammar.
print(mv(' 12 '));
// CHECK-NEXT: {"neg":false,"n":"12","e":0}
print(mv('+1.50'));
// CHECK-NEXT: {"neg":false,"n":"15","e":-1}
print(mv('.5'));
// CHECK-NEXT: {"neg":false,"n":"5","e":-1}
print(mv('5.'));
// CHECK-NEXT: {"neg":false,"n":"5","e":0}
print(mv('1e400'));
// CHECK-NEXT: "posInf"
print(mv('-1e400'));
// CHECK-NEXT: "negInf"
print(mv('-1e-400'));
// CHECK-NEXT: "negZero"
print(mv('1e-400'));
// CHECK-NEXT: {"neg":false,"n":"0","e":0}
print(mv('-Infinity'), mv('+Infinity'), mv('Infinity'), mv('infinity'));
// CHECK-NEXT: "negInf" "posInf" "posInf" "nan"
print(mv('0x1F'), mv('0X1f'));
// CHECK-NEXT: {"neg":false,"n":"31","e":0} {"neg":false,"n":"31","e":0}
print(mv('-0x1F'), mv('+0x1F'));
// CHECK-NEXT: "nan" "nan"
print(mv('0b101'), mv('0o17'), mv('0x64'));
// CHECK-NEXT: {"neg":false,"n":"5","e":0} {"neg":false,"n":"15","e":0} {"neg":false,"n":"1","e":2}
print(mv('0x'), mv('0b102'), mv('0x1.5'), mv('0x' + 'F'.repeat(300)));
// CHECK-NEXT: "nan" "nan" "nan" "posInf"
print(mv('1_000'));
// CHECK-NEXT: "nan"
print(mv(''), mv('   '));
// CHECK-NEXT: {"neg":false,"n":"0","e":0} {"neg":false,"n":"0","e":0}
print(mv('  7 ﻿'));
// CHECK-NEXT: {"neg":false,"n":"7","e":0}
// Every StrWhiteSpaceChar: TAB, VT, FF, ZWNBSP, the Zs code points, and
// the line terminators. U+180E is not white space.
print(mv('\t\v\f﻿       　\n\r  9'));
// CHECK-NEXT: {"neg":false,"n":"9","e":0}
print(mv('᠎9'), mv('9​'));
// CHECK-NEXT: "nan" "nan"
print(mv('-0'), mv('-0.000'), mv('0.000'), mv('-0e5'));
// CHECK-NEXT: "negZero" "negZero" {"neg":false,"n":"0","e":0} "negZero"
print(mv('00012.3400'), mv('1.5e3'), mv('-1.50E-3'), mv('1e+2'), mv('12e-0'));
// CHECK-NEXT: {"neg":false,"n":"1234","e":-2} {"neg":false,"n":"15","e":2} {"neg":true,"n":"15","e":-4} {"neg":false,"n":"1","e":2} {"neg":false,"n":"12","e":0}
print(mv('123456789012345678901234567890.123456789'));
// CHECK-NEXT: {"neg":false,"n":"123456789012345678901234567890123456789","e":-9}
print(mv('1e'), mv('1e+'), mv('.'), mv('+'), mv('-'), mv('.e1'), mv('1 2'), mv('--1'));
// CHECK-NEXT: "nan" "nan" "nan" "nan" "nan" "nan" "nan" "nan"
print(mv('1e99999999999999999999'), mv('0e99999999999999999999'));
// CHECK-NEXT: "posInf" {"neg":false,"n":"0","e":0}
print(mv('0.' + '0'.repeat(1000) + '1'));
// CHECK-NEXT: {"neg":false,"n":"0","e":0}

// Objects go through ToPrimitive with hint number.
var hints = [];
print(mv({
  [Symbol.toPrimitive](hint) {
    hints.push(hint);
    return '2.5';
  },
}), hints.join());
// CHECK-NEXT: {"neg":false,"n":"25","e":-1} number
print(mv({
  valueOf() {
    return 3n;
  },
}));
// CHECK-NEXT: {"neg":false,"n":"3","e":0}
var calls = [];
print(mv({
  valueOf() {
    calls.push('valueOf');
    return {};
  },
  toString() {
    calls.push('toString');
    return '-7';
  },
}), calls.join());
// CHECK-NEXT: {"neg":true,"n":"7","e":0} valueOf,toString
print(mv({[Symbol.toPrimitive]: null, valueOf: () => 4}));
// CHECK-NEXT: {"neg":false,"n":"4","e":0}
print(mv(new Number(-0)), mv(Object(5n)));
// CHECK-NEXT: "negZero" {"neg":false,"n":"5","e":0}
print(thrown(() => ToIntlMathematicalValue({[Symbol.toPrimitive]: () => ({})})));
// CHECK-NEXT: TypeError
print(thrown(() => ToIntlMathematicalValue({[Symbol.toPrimitive]: 5})));
// CHECK-NEXT: TypeError
print(thrown(() => ToIntlMathematicalValue({valueOf: () => ({}), toString: () => ({})})));
// CHECK-NEXT: TypeError
print(thrown(() => ToIntlMathematicalValue(Object.create(null))));
// CHECK-NEXT: TypeError
print(thrown(() => ToIntlMathematicalValue(Symbol())));
// CHECK-NEXT: TypeError
print(thrown(() => ToIntlMathematicalValue({valueOf: () => Symbol()})));
// CHECK-NEXT: TypeError
print(ToPrimitiveNumber('x'), ToPrimitiveNumber(1n), typeof ToPrimitiveNumber({}));
// CHECK-NEXT: x 1 string

// The overrides' conversions.
print(show(IntlMVFromNumber(-0)), show(IntlMVFromNumber(-2.5e-7)), show(IntlMVFromNumber(NaN)));
// CHECK-NEXT: "negZero" {"neg":true,"n":"25","e":-8} "nan"
print(show(IntlMVFromBigInt(-500n)), show(IntlMVFromBigInt(0n)));
// CHECK-NEXT: {"neg":true,"n":"5","e":2} {"neg":false,"n":"0","e":0}

// The record helpers.
var X = ToIntlMathematicalValue;
print(intlMVMagnitude(X(123)), intlMVMagnitude(X(0.00123)), intlMVMagnitude(X(1)),
      intlMVMagnitude(X(9.99)), intlMVMagnitude(X(-10)), intlMVMagnitude(X(2n ** 1000n)));
// CHECK-NEXT: 2 -3 0 0 1 301
print(show(intlMVScale(X(1.5), 3)), show(intlMVScale(X(1.5), -3)), show(intlMVScale(X(0), 5)));
// CHECK-NEXT: {"neg":false,"n":"15","e":2} {"neg":false,"n":"15","e":-4} {"neg":false,"n":"0","e":0}
print(intlMVIsZero(X(0)), intlMVIsZero(X(-0)), intlMVIsZero(X(1)), intlMVIsZero(X(NaN)));
// CHECK-NEXT: true false false false
print(show(intlMVNegate(X(1.5))), show(intlMVNegate(X(-1.5))), show(intlMVNegate(X(0))),
      show(intlMVNegate(X(-0))));
// CHECK-NEXT: {"neg":true,"n":"15","e":-1} {"neg":false,"n":"15","e":-1} "negZero" {"neg":false,"n":"0","e":0}
print(intlMVNegate('posInf'), intlMVNegate('negInf'), intlMVNegate('nan'));
// CHECK-NEXT: negInf posInf nan

// GetUnsignedRoundingMode: the draft's table.
var modes = ['ceil', 'floor', 'expand', 'trunc', 'halfCeil', 'halfFloor',
             'halfExpand', 'halfTrunc', 'halfEven'];
for (var i = 0; i < modes.length; i++) {
  print(modes[i], GetUnsignedRoundingMode(modes[i], 'positive'),
        GetUnsignedRoundingMode(modes[i], 'negative'));
}
// CHECK-NEXT: ceil infinity zero
// CHECK-NEXT: floor zero infinity
// CHECK-NEXT: expand infinity infinity
// CHECK-NEXT: trunc zero zero
// CHECK-NEXT: halfCeil half-infinity half-zero
// CHECK-NEXT: halfFloor half-zero half-infinity
// CHECK-NEXT: halfExpand half-infinity half-infinity
// CHECK-NEXT: halfTrunc half-zero half-zero
// CHECK-NEXT: halfEven half-even half-even

// ApplyUnsignedRoundingMode on x = (q + r / d) units: q or q + 1.
var urms = ['zero', 'infinity', 'half-zero', 'half-infinity', 'half-even'];
function aurm(q, r, d) {
  var out = [];
  for (var i = 0; i < urms.length; i++)
    out.push(ApplyUnsignedRoundingMode(q, r, d, urms[i]));
  return out.join(' ');
}
print(aurm(4n, 0n, 10n));
// CHECK-NEXT: 4 4 4 4 4
print(aurm(4n, 3n, 10n));
// CHECK-NEXT: 4 5 4 4 4
print(aurm(4n, 7n, 10n));
// CHECK-NEXT: 4 5 5 5 5
print(aurm(4n, 5n, 10n));
// CHECK-NEXT: 4 5 4 5 4
print(aurm(5n, 5n, 10n));
// CHECK-NEXT: 5 6 5 6 6

// ToRawFixed and ToRawPrecision, directly.
print(show(ToRawFixed(X('1.25'), 0, 1, 1, 'half-even')));
// CHECK-NEXT: {"formattedString":"1.2","roundedNumber":{"neg":false,"n":"12","e":-1},"integerDigitsCount":1,"roundingMagnitude":-1}
print(show(ToRawFixed(X(0), 2, 2, 1, 'half-infinity')));
// CHECK-NEXT: {"formattedString":"0.00","roundedNumber":{"neg":false,"n":"0","e":0},"integerDigitsCount":1,"roundingMagnitude":-2}
print(show(ToRawFixed(X(123.456), 0, 0, 1, 'half-infinity')));
// CHECK-NEXT: {"formattedString":"123","roundedNumber":{"neg":false,"n":"123","e":0},"integerDigitsCount":3,"roundingMagnitude":0}
print(show(ToRawFixed(X(0.5), 0, 0, 1, 'half-even')), show(ToRawFixed(X(0.5), 0, 0, 1, 'half-infinity')));
// CHECK-NEXT: {"formattedString":"0","roundedNumber":{"neg":false,"n":"0","e":0},"integerDigitsCount":1,"roundingMagnitude":0} {"formattedString":"1","roundedNumber":{"neg":false,"n":"1","e":0},"integerDigitsCount":1,"roundingMagnitude":0}
print(show(ToRawFixed(X(0.0012), 1, 3, 1, 'half-infinity')));
// CHECK-NEXT: {"formattedString":"0.001","roundedNumber":{"neg":false,"n":"1","e":-3},"integerDigitsCount":1,"roundingMagnitude":-3}
print(show(ToRawFixed(X(1.5), 1, 4, 1, 'half-infinity')));
// CHECK-NEXT: {"formattedString":"1.5","roundedNumber":{"neg":false,"n":"15","e":-1},"integerDigitsCount":1,"roundingMagnitude":-4}
print(show(ToRawFixed(X(1200), 0, 2, 1, 'zero')));
// CHECK-NEXT: {"formattedString":"1200","roundedNumber":{"neg":false,"n":"12","e":2},"integerDigitsCount":4,"roundingMagnitude":-2}
print(show(ToRawFixed(X('1.3125'), 2, 2, 25, 'half-infinity')));
// CHECK-NEXT: {"formattedString":"1.25","roundedNumber":{"neg":false,"n":"125","e":-2},"integerDigitsCount":1,"roundingMagnitude":-2}
print(show(ToRawFixed(X(1.75), 4, 4, 5000, 'half-infinity')));
// CHECK-NEXT: {"formattedString":"2.0000","roundedNumber":{"neg":false,"n":"2","e":0},"integerDigitsCount":1,"roundingMagnitude":-4}
print(show(ToRawPrecision(X(0), 1, 3, 'half-infinity')));
// CHECK-NEXT: {"formattedString":"0","roundedNumber":{"neg":false,"n":"0","e":0},"integerDigitsCount":1,"roundingMagnitude":-2}
print(show(ToRawPrecision(X(0), 3, 3, 'half-infinity')));
// CHECK-NEXT: {"formattedString":"0.00","roundedNumber":{"neg":false,"n":"0","e":0},"integerDigitsCount":1,"roundingMagnitude":-2}
print(show(ToRawPrecision(X(123456), 1, 3, 'half-infinity')));
// CHECK-NEXT: {"formattedString":"123000","roundedNumber":{"neg":false,"n":"123","e":3},"integerDigitsCount":6,"roundingMagnitude":3}
print(show(ToRawPrecision(X(0.000123456), 2, 3, 'zero')));
// CHECK-NEXT: {"formattedString":"0.000123","roundedNumber":{"neg":false,"n":"123","e":-6},"integerDigitsCount":1,"roundingMagnitude":-6}
print(show(ToRawPrecision(X(999.5), 1, 3, 'half-infinity')));
// CHECK-NEXT: {"formattedString":"1000","roundedNumber":{"neg":false,"n":"1","e":3},"integerDigitsCount":4,"roundingMagnitude":1}
print(show(ToRawPrecision(X(1.5), 3, 3, 'zero')));
// CHECK-NEXT: {"formattedString":"1.50","roundedNumber":{"neg":false,"n":"15","e":-1},"integerDigitsCount":1,"roundingMagnitude":-2}
print(show(ToRawPrecision(X(12.5), 1, 2, 'half-even')), show(ToRawPrecision(X(13.5), 1, 2, 'half-even')));
// CHECK-NEXT: {"formattedString":"12","roundedNumber":{"neg":false,"n":"12","e":0},"integerDigitsCount":2,"roundingMagnitude":0} {"formattedString":"14","roundedNumber":{"neg":false,"n":"14","e":0},"integerDigitsCount":2,"roundingMagnitude":0}
print(show(ToRawPrecision(X(1.2), 1, 4, 'zero')));
// CHECK-NEXT: {"formattedString":"1.2","roundedNumber":{"neg":false,"n":"12","e":-1},"integerDigitsCount":1,"roundingMagnitude":-3}

// FormatNumericToString with the record SetNumberFormatDigitOptions fills.
function digitRecord(options, notation) {
  var r = {__proto__: null};
  SetNumberFormatDigitOptions(r, options, 0, 3, notation || 'standard');
  r.notation = notation || 'standard';
  return r;
}
/// The signed string FormatNumericToString gives for value under options.
function fnts(options, value) {
  var res = FormatNumericToString(digitRecord(options), X(value));
  var r = res.roundedNumber;
  return (r === 'negZero' || r.neg ? '-' : '') + res.formattedString;
}
function table(options, values) {
  var out = [];
  for (var i = 0; i < values.length; i++) out.push(fnts(options, values[i]));
  return out.join(' ');
}

// test262's format-rounding-mode-*.js rows, at maximumSignificantDigits 2.
var sdValues = ['1.101', '1.15', '1.1999', '1.25', '0', '-0', '-1.101',
                '-1.15', '-1.1999', '-1.25'];
print(table({roundingMode: 'halfEven', maximumSignificantDigits: 2}, sdValues));
// CHECK-NEXT: 1.1 1.2 1.2 1.2 0 -0 -1.1 -1.2 -1.2 -1.2
print(table({roundingMode: 'halfExpand', maximumSignificantDigits: 2}, sdValues));
// CHECK-NEXT: 1.1 1.2 1.2 1.3 0 -0 -1.1 -1.2 -1.2 -1.3
print(table({roundingMode: 'ceil', maximumSignificantDigits: 2}, sdValues));
// CHECK-NEXT: 1.2 1.2 1.2 1.3 0 -0 -1.1 -1.1 -1.1 -1.2
// The same modes at maximumFractionDigits 1, with Numbers.
var fdValues = [1.1, 1.15, 1.2, 1.25, 1.3, 1.35, -1.1, -1.15, -1.2, -1.25,
                -1.3, -1.35];
print(table({roundingMode: 'halfEven', maximumFractionDigits: 1}, fdValues));
// CHECK-NEXT: 1.1 1.2 1.2 1.2 1.3 1.4 -1.1 -1.2 -1.2 -1.2 -1.3 -1.4
print(table({roundingMode: 'halfExpand', maximumFractionDigits: 1}, fdValues));
// CHECK-NEXT: 1.1 1.2 1.2 1.3 1.3 1.4 -1.1 -1.2 -1.2 -1.3 -1.3 -1.4
print(table({roundingMode: 'ceil', maximumFractionDigits: 1}, fdValues));
// CHECK-NEXT: 1.1 1.2 1.2 1.3 1.3 1.4 -1.1 -1.1 -1.2 -1.2 -1.3 -1.3
print(table({roundingMode: 'floor', maximumFractionDigits: 1}, fdValues));
// CHECK-NEXT: 1.1 1.1 1.2 1.2 1.3 1.3 -1.1 -1.2 -1.2 -1.3 -1.3 -1.4
print(table({roundingMode: 'expand', maximumFractionDigits: 1}, fdValues));
// CHECK-NEXT: 1.1 1.2 1.2 1.3 1.3 1.4 -1.1 -1.2 -1.2 -1.3 -1.3 -1.4
print(table({roundingMode: 'trunc', maximumFractionDigits: 1}, fdValues));
// CHECK-NEXT: 1.1 1.1 1.2 1.2 1.3 1.3 -1.1 -1.1 -1.2 -1.2 -1.3 -1.3
print(table({roundingMode: 'halfCeil', maximumFractionDigits: 1}, fdValues));
// CHECK-NEXT: 1.1 1.2 1.2 1.3 1.3 1.4 -1.1 -1.1 -1.2 -1.2 -1.3 -1.3
print(table({roundingMode: 'halfFloor', maximumFractionDigits: 1}, fdValues));
// CHECK-NEXT: 1.1 1.1 1.2 1.2 1.3 1.3 -1.1 -1.2 -1.2 -1.3 -1.3 -1.4
print(table({roundingMode: 'halfTrunc', maximumFractionDigits: 1}, fdValues));
// CHECK-NEXT: 1.1 1.1 1.2 1.2 1.3 1.3 -1.1 -1.1 -1.2 -1.2 -1.3 -1.3

// test262's format-rounding-increment-25.js and -5000.js.
print(table({roundingIncrement: 25, maximumFractionDigits: 2, minimumFractionDigits: 2},
            ['1.2500', '1.3125', '1.3750', '1.4375', '1.5000']));
// CHECK-NEXT: 1.25 1.25 1.50 1.50 1.50
print(table({roundingIncrement: 25, maximumFractionDigits: 3, minimumFractionDigits: 3},
            ['1.02500', '1.03125', '1.03750', '1.04375', '1.05000']));
// CHECK-NEXT: 1.025 1.025 1.050 1.050 1.050
// halfEven with an even increment: the parity is that of the quotient by
// the increment (0, 1, 2 units of 0.50), not of n, which is always even.
print(table({roundingIncrement: 50, roundingMode: 'halfEven', maximumFractionDigits: 2,
             minimumFractionDigits: 2}, [0.25, 0.75, 1.25]));
// CHECK-NEXT: 0.00 1.00 1.00
print(table({roundingIncrement: 5000, maximumFractionDigits: 4, minimumFractionDigits: 4},
            ['1.500', '1.625', '1.750', '1.875', '2.000']));
// CHECK-NEXT: 1.5000 1.5000 2.0000 2.0000 2.0000
print(table({roundingIncrement: 5000, maximumFractionDigits: 5, minimumFractionDigits: 5},
            ['1.0500', '1.0625', '1.0750', '1.0875', '1.1000']));
// CHECK-NEXT: 1.05000 1.05000 1.10000 1.10000 1.10000

// test262's format-rounding-priority-*.js.
print(fnts({roundingPriority: 'morePrecision', minimumSignificantDigits: 2, minimumFractionDigits: 2}, 1),
      fnts({roundingPriority: 'morePrecision', minimumSignificantDigits: 3, minimumFractionDigits: 2}, 1),
      fnts({roundingPriority: 'morePrecision', maximumSignificantDigits: 2, maximumFractionDigits: 2}, 1.23),
      fnts({roundingPriority: 'morePrecision', maximumSignificantDigits: 3, maximumFractionDigits: 1}, 1.234));
// CHECK-NEXT: 1.0 1.00 1.23 1.23
print(fnts({roundingPriority: 'lessPrecision', minimumSignificantDigits: 2, minimumFractionDigits: 2}, 1),
      fnts({roundingPriority: 'lessPrecision', minimumSignificantDigits: 3, minimumFractionDigits: 1}, 1),
      fnts({roundingPriority: 'lessPrecision', maximumSignificantDigits: 2, maximumFractionDigits: 2}, 1.23),
      fnts({roundingPriority: 'lessPrecision', maximumSignificantDigits: 3, maximumFractionDigits: 1}, 1.234));
// CHECK-NEXT: 1.00 1.0 1.2 1.2
print(fnts({roundingPriority: 'auto', minimumSignificantDigits: 2}, 1),
      fnts({roundingPriority: 'auto', minimumSignificantDigits: 3}, 1),
      fnts({roundingPriority: 'auto', maximumSignificantDigits: 2}, 1.23),
      fnts({roundingPriority: 'auto', maximumSignificantDigits: 3}, 1.234));
// CHECK-NEXT: 1.0 1.00 1.2 1.23

// trailingZeroDisplay, minimumIntegerDigits and signs.
print(fnts({minimumFractionDigits: 2, trailingZeroDisplay: 'stripIfInteger'}, '1.00'),
      fnts({minimumFractionDigits: 2, trailingZeroDisplay: 'stripIfInteger'}, '1.50'),
      fnts({maximumFractionDigits: 2, minimumFractionDigits: 2, trailingZeroDisplay: 'stripIfInteger'}, 0.999),
      fnts({minimumFractionDigits: 2}, '1.00'));
// CHECK-NEXT: 1 1.50 1 1.00
print(fnts({minimumIntegerDigits: 3}, 5), fnts({minimumIntegerDigits: 3}, 0.5),
      fnts({minimumIntegerDigits: 3}, 1234), fnts({minimumIntegerDigits: 3, maximumSignificantDigits: 1}, 0.05));
// CHECK-NEXT: 005 000.5 1234 000.05
print(show(FormatNumericToString(digitRecord({}), X(-1.5))));
// CHECK-NEXT: {"roundedNumber":{"neg":true,"n":"15","e":-1},"formattedString":"1.5"}
print(show(FormatNumericToString(digitRecord({}), X(-0.0001))));
// CHECK-NEXT: {"roundedNumber":"negZero","formattedString":"0"}
print(show(FormatNumericToString(digitRecord({}), X(-0))));
// CHECK-NEXT: {"roundedNumber":"negZero","formattedString":"0"}
print(show(FormatNumericToString(digitRecord({}), X(0))));
// CHECK-NEXT: {"roundedNumber":{"neg":false,"n":"0","e":0},"formattedString":"0"}
print(fnts({}, 1234567.891), fnts({}, -999.9995), fnts({maximumSignificantDigits: 3}, -0.0012345));
// CHECK-NEXT: 1234567.891 -1000 -0.00123

// Review Focus 1: extreme values keep every digit.
var tiny = '0.' + '0'.repeat(99) + '1';
print(fnts({maximumFractionDigits: 100}, tiny) === tiny);
// CHECK-NEXT: true
var big = 2n ** 1000n;
var s = fnts({}, big);
print(s.length, s === String(big));
// CHECK-NEXT: 302 true
s = fnts({maximumSignificantDigits: 21}, big);
print(s.length, s.slice(0, 22), s.slice(21) === '0'.repeat(281));
// CHECK-NEXT: 302 1071508607186267320950 true
s = fnts({}, 1e308);
print(s.length, s === '1' + '0'.repeat(308));
// CHECK-NEXT: 309 true
s = fnts({maximumSignificantDigits: 3}, 5e-324);
print(s.length, s === '0.' + '0'.repeat(323) + '5');
// CHECK-NEXT: 326 true
print(fnts({maximumFractionDigits: 100}, 5e-324),
      fnts({maximumFractionDigits: 100, roundingMode: 'ceil'}, 5e-324) === tiny);
// CHECK-NEXT: 0 true
print(fnts({}, -(10n ** 400n)).length);
// CHECK-NEXT: 402

// ComputeExponentForMagnitude and ComputeExponent.
function compactStub(magnitude) {
  return magnitude < 3 ? 0 : magnitude < 6 ? 3 : 6;
}
function ce(notation, options, value) {
  return ComputeExponent(digitRecord(options, notation), X(value), compactStub);
}
print(ce('scientific', {}, 123456), ce('scientific', {}, 0.00123), ce('scientific', {}, -123456),
      ce('scientific', {}, 0), ce('scientific', {}, 1));
// CHECK-NEXT: 5 -3 5 0 0
print(ce('engineering', {}, 123456), ce('engineering', {}, 0.00123), ce('engineering', {}, 0.5));
// CHECK-NEXT: 3 -3 -3
print(ce('scientific', {maximumFractionDigits: 0}, 999.99), ce('scientific', {}, 999.99),
      ce('scientific', {}, 999.9));
// CHECK-NEXT: 3 3 2
// The trial rounding keeps the sign (a deliberate deviation from the
// draft's step 2; see ComputeExponent), so a directed rounding mode rounds
// as the final formatting does: -950 with ceil is "-9E2", with floor
// "-1E3"; 950 with ceil is "1E3", with floor "9E2" (V8 agrees).
function dir(mode, value) {
  return ce('scientific', {roundingMode: mode, maximumFractionDigits: 0}, value);
}
print(dir('ceil', -950), dir('floor', -950), dir('ceil', 950), dir('floor', 950),
      dir('halfCeil', -9.5), dir('halfFloor', -9.5));
// CHECK-NEXT: 2 3 3 2 0 1
// A negative value that rounds to 0 keeps the exponent (step 7).
print(ComputeExponent(digitRecord({maximumFractionDigits: 0}, 'compact'), X(-0.0001),
                      function (m) { return m === -4 ? 0 : 3; }));
// CHECK-NEXT: 0
print(ce('engineering', {maximumSignificantDigits: 2}, 999999), ce('engineering', {}, 999999));
// CHECK-NEXT: 6 3
print(ce('standard', {}, 123456), ce('compact', {}, 1234), ce('compact', {}, 999999), ce('compact', {}, 999));
// CHECK-NEXT: 0 3 6 0
// Step 7: x rounds to 0 after scaling, so the exponent stays.
print(ComputeExponent(digitRecord({maximumFractionDigits: 0}, 'compact'), X(0.0001),
                      function (m) { return m === -4 ? 0 : 3; }));
// CHECK-NEXT: 0
var r = digitRecord({}, 'engineering');
print(ComputeExponentForMagnitude(r, -1, undefined), ComputeExponentForMagnitude(r, -4, undefined),
      ComputeExponentForMagnitude(r, 5, undefined), ComputeExponentForMagnitude(digitRecord({}, 'compact'), 7, compactStub));
// CHECK-NEXT: -3 -6 3 6

// No record or magnitude holds -0.
var noFrac = ToRawFixed(X(5), 0, 0, 1, 'zero');
print(Object.is(noFrac.roundingMagnitude, 0), Object.is(noFrac.roundedNumber.e, 0),
      Object.is(X('5e-0').e, 0), Object.is(intlMVScale(X(5), -0).e, 0));
// CHECK-NEXT: true true true true
