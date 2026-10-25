/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// Intl mathematical values and their rounding, shared by Intl.NumberFormat,
// Intl.PluralRules and the toLocaleString overrides: ECMA-402 16.5.2
// FormatNumericToString, 16.5.7 ToRawPrecision, 16.5.8 ToRawFixed, 16.5.12
// ComputeExponent, 16.5.13 ComputeExponentForMagnitude, 16.5.14
// StringIntlMV, 16.5.15 ToIntlMathematicalValue, 16.5.16
// GetUnsignedRoundingMode and 16.5.17 ApplyUnsignedRoundingMode, following
// the draft https://tc39.es/ecma402/ dated October 24, 2026; and ECMA-262
// 7.1.1 ToPrimitive with hint number, from the https://tc39.es/ecma262/
// draft of the same date.
//
// An Intl mathematical value is one of the strings 'nan', 'posInf',
// 'negInf' and 'negZero' (the draft's not-a-number, positive-infinity,
// negative-infinity and negative-zero), or a record
// {__proto__: null, neg, n, e} for the mathematical value
// (-1)^neg × n × 10^e: neg a boolean, n a non-negative BigInt and e an
// integer Number. A record is normalized: n has no trailing decimal zero
// (e absorbs them), and 0 is {neg: false, n: 0n, e: 0}. Records are never
// changed after they are made.
//
// Rounding is exact: x is divided by the rounding unit with one BigInt
// division, and the quotient and remainder decide the result.

/// A string of k code units 0x0030 (DIGIT ZERO), for an integer k >= 0.
function zeroString(k) {
  // 1. Append doubling runs of zeros for the bits of k, so a long string
  //    takes O(log k) concatenations.
  var out = '';
  var run = '0';
  while (k > 0) {
    if (k % 2 === 1) out += run;
    k = MathFloor(k / 2);
    if (k > 0) run += run;
  }
  return out;
}

/// The powers of ten bigPow10 has computed: entry k is 10n ** k.
var BIG_POWERS_OF_TEN = [1n];

/// 10^k as a BigInt, for an integer k >= 0. Callers pass k < 100. The
/// powers are kept in a table, because Hermes's ** is slow (a native call,
/// about 17 microseconds in the ASan build, on the formatting path) and
/// squares past the result, so 10n ** k fails well below the BigInt size
/// limit (for k above about 16384).
function bigPow10(k) {
  // 1. Extend the table up to k, each entry ten times the one before.
  var table = BIG_POWERS_OF_TEN;
  while (table.length <= k) listPush(table, table[table.length - 1] * 10n);
  // 2. Return its entry k.
  return table[k];
}

/// 10^k as a Number, for k from 0 to 4: the rounding increments all divide
/// 10^4 (isMultipleOfUnit).
var POWERS_OF_TEN_TO_4 = [1, 10, 100, 1000, 10000];

/// The Intl mathematical value 0.
function intlMVZero() {
  // 1. Return the normalized record of 0.
  return {__proto__: null, neg: false, n: 0n, e: 0};
}

/// The normalized record of (-1)^neg × n × 10^e, for a non-negative BigInt
/// n and an integer e.
function makeIntlMV(neg, n, e) {
  // 1. If n is 0, return 0, which has no sign.
  if (n === 0n) return intlMVZero();
  // 2. Move the trailing zeros of n into e.
  var digits = `${n}`;
  var end = digits.length;
  while (charCodeAt(digits, end - 1) === 0x30) end--;
  if (end < digits.length) {
    e += digits.length - end;
    n = _BigInt(stringSlice(digits, 0, end));
  }
  // 3. Return the record.
  return {__proto__: null, neg: neg, n: n, e: e};
}

/// The number of digits below a value's leading digit, or below 10^0 if it
/// is smaller, that intlMVFromDigits keeps. Every rounding of
/// FormatNumericToString is at a position at least min(magnitude, 0) - 102:
/// ToRawPrecision rounds at magnitude - 20 at the lowest (21 significant
/// digits), and ToRawFixed at -100 (100 fraction digits) of a value that
/// ComputeExponent scaled by 10^-exponent, where exponent >= magnitude - 2
/// for scientific and engineering and exponent >= 0 for standard and
/// compact, and that the percent style scaled by 100 first. 110 leaves a
/// margin.
var KEPT_DIGITS_BELOW = 110;

/// The largest magnitude intlMVFromDigits keeps all the digits of. A value
/// of a larger magnitude is ±∞ after RoundMVResult, and of a magnitude
/// below -324 is 0, so StringToIntlMV discards the record.
var KEPT_MAGNITUDE_MAX = 400;

/// The normalized record of (-1)^neg × D × 10^e, where D is the integer
/// whose decimal digits are digits, a string of ASCII digits that may have
/// leading and trailing zeros; StringToIntlMV's records come from here.
/// Digits more than KEPT_DIGITS_BELOW places below the leading digit, or
/// below 10^0 if that is lower, are replaced by one sticky digit 1, so
/// that a long decimal string does not make a BigInt beyond the size limit.
/// No rounding sees the difference (KEPT_DIGITS_BELOW).
function intlMVFromDigits(neg, digits, e) {
  // 1. Skip the leading zeros. If nothing is left, return 0.
  var start = 0;
  var end = digits.length;
  while (start < end && charCodeAt(digits, start) === 0x30) start++;
  if (start === end) return intlMVZero();
  // 2. Move the trailing zeros into e.
  while (charCodeAt(digits, end - 1) === 0x30) end--;
  e += digits.length - end;
  // 3. Let magnitude be the position of the leading digit, and keep be the
  //    number of digits from it down to 10^(min(magnitude, 0) -
  //    KEPT_DIGITS_BELOW).
  var magnitude = e + (end - start) - 1;
  var keep =
    MathMin(MathMax(magnitude, 0), KEPT_MAGNITUDE_MAX) + KEPT_DIGITS_BELOW + 1;
  // 4. If there are more digits, replace the rest by the digit 1 one place
  //    below the last digit kept. The rest ends in a non-zero digit, so it
  //    is a value t with 0 < t < 10^p, where 10^p is the place of the last
  //    digit kept, and so is 10^(p-1). Every rounding unit is a multiple of
  //    10^(p+1) whose half is a multiple of 10^p, so the remainder of a
  //    division by it is zero for neither, and less than half, equal to
  //    half or more than half for both alike: every rounding mode gives the
  //    same result.
  if (end - start > keep) {
    return {
      __proto__: null,
      neg: neg,
      n: _BigInt(stringSlice(digits, start, start + keep) + '1'),
      e: magnitude - keep,
    };
  }
  // 5. Return the record.
  return {
    __proto__: null,
    neg: neg,
    n: _BigInt(stringSlice(digits, start, end)),
    e: e,
  };
}

/// Whether x, an Intl mathematical value, is the mathematical value 0.
/// negative-zero is not.
function intlMVIsZero(x) {
  // 1. Return whether x is a record with n = 0.
  return typeof x === 'object' && x.n === 0n;
}

/// Whether x, a record, is an integer (x modulo 1 = 0). A normalized record
/// is an integer exactly when e >= 0.
function intlMVIsInteger(x) {
  // 1. Return whether x.e >= 0.
  return x.e >= 0;
}

/// -x for an Intl mathematical value x, as FormatNumericToString negates:
/// 0 and negative-zero swap, and so do the two infinities; NaN stays.
function intlMVNegate(x) {
  // 1. Swap the special values.
  if (x === 'negZero') return intlMVZero();
  if (x === 'posInf') return 'negInf';
  if (x === 'negInf') return 'posInf';
  if (x === 'nan') return x;
  // 2. If x is 0, return negative-zero.
  if (x.n === 0n) return 'negZero';
  // 3. Return x with the other sign.
  return {__proto__: null, neg: !x.neg, n: x.n, e: x.e};
}

/// x × 10^k for an Intl mathematical value x and an integer k. Only
/// non-zero records change.
function intlMVScale(x, k) {
  // 1. If x is not a record, or is 0, return x.
  if (typeof x !== 'object' || x.n === 0n) return x;
  // 2. Return x with k added to its exponent.
  return {__proto__: null, neg: x.neg, n: x.n, e: x.e + k};
}

/// floor(log10(abs(x))) for a non-zero record x: the power of ten of its
/// most significant digit.
function intlMVMagnitude(x) {
  // 1. n has length(n) digits, so abs(x) is in
  //    [10^(length(n) - 1 + e), 10^(length(n) + e)).
  return `${x.n}`.length - 1 + x.e;
}

/// ℝ(b) for a BigInt b, as ToIntlMathematicalValue step 2 returns it.
function IntlMVFromBigInt(b) {
  // 1. If b is 0n, return 0.
  if (b === 0n) return intlMVZero();
  // 2. Return the record of the decimal digits of b, without the sign and
  //    with the trailing zeros moved into e. -b is not computed: for a b at
  //    Hermes's size limit it exceeds the limit (the note before
  //    roundToUnit).
  var digits = `${b}`;
  var start = b < 0n ? 1 : 0;
  var end = digits.length;
  while (charCodeAt(digits, end - 1) === 0x30) end--;
  return {
    __proto__: null,
    neg: start === 1,
    n: _BigInt(stringSlice(digits, start, end)),
    e: digits.length - end,
  };
}

/// ToIntlMathematicalValue steps 4.b to 10 for a Number x: the Intl
/// mathematical value of x, exactly.
function IntlMVFromNumber(x) {
  // 4.b. If x is -0𝔽, return negative-zero.
  if (x === 0 && 1 / x < 0) return 'negZero';
  // 4.c. Let str be Number::toString(x, 10). A template literal on a Number
  //      primitive is exactly that, and runs no user code.
  // 5-8. Let intlMV be the StringIntlMV of str. For NaN and the infinities,
  //      str is "NaN", which does not parse (not-a-number), or "Infinity"
  //      with an optional "-".
  if (x !== x) return 'nan';
  if (!NumberIsFinite(x)) return x > 0 ? 'posInf' : 'negInf';
  //      Otherwise str is an optional "-", DecimalDigits, an optional "."
  //      and DecimalDigits, and an optional "e", sign and DecimalDigits
  //      (ECMA-262 6.1.6.1.20): a StrDecimalLiteral whose value is
  //      D × 10^(exponent - n), with D the integer of all its digits and n
  //      the number of digits after the ".". It is read with two searches
  //      rather than code unit by code unit, which is the slow part of
  //      formatting a Number.
  var str = `${x}`;
  var start = x < 0 ? 1 : 0;
  var end = stringIndexOf(str, 'e', start);
  var exponent = 0;
  if (end === -1) end = str.length;
  else exponent = ToNumber(stringSlice(str, end + 1));
  var dot = stringIndexOf(str, '.', start);
  var digits, fractionLength;
  if (dot === -1 || dot > end) {
    digits = stringSlice(str, start, end);
    fractionLength = 0;
  } else {
    digits = stringSlice(str, start, dot) + stringSlice(str, dot + 1, end);
    fractionLength = end - dot - 1;
  }
  var intlMV = intlMVFromDigits(start === 1, digits, exponent - fractionLength);
  // 9. Step 9's RoundMVResult(abs(intlMV)) is abs(x), because ToNumber of
  //    Number::toString(x, 10) is x. x is finite, and +0 only for a zero x,
  //    whose intlMV is already 0, so none of step 9's returns applies.
  // 10. Return intlMV.
  return intlMV;
}

/// Whether v is an Object.
function isObject(v) {
  // 1. Return whether v is an Object: typeof gives "object" for a non-null
  //    object and "function" for a callable one.
  return (typeof v === 'object' && v !== null) || typeof v === 'function';
}

// ECMA-262 7.3.10 GetMethod ( V, P )
function GetMethod(V, P) {
  // 1. Let func be ? GetV(V, P).
  var func = V[P];
  // 2. If func is either undefined or null, return undefined.
  if (func === undefined || func === null) return undefined;
  // 3. If IsCallable(func) is false, throw a TypeError exception.
  if (typeof func !== 'function')
    throw new _TypeError('Property is not a function');
  // 4. Return func.
  return func;
}

// ECMA-262 7.1.1.1 OrdinaryToPrimitive ( O, hint ), with hint number.
function OrdinaryToPrimitiveNumber(O) {
  // 1. If hint is string, then ... 2. Else,
  //    a. Let methodNames be « "valueOf", "toString" ».
  // 3. For each element name of methodNames, do
  //    a. Let method be ? Get(O, name).
  //    b. If IsCallable(method) is true, then
  //       i. Let result be ? Call(method, O).
  //       ii. If result is not an Object, return result.
  var method = O.valueOf;
  if (typeof method === 'function') {
    var result = ReflectApply(method, O, []);
    if (!isObject(result)) return result;
  }
  method = O.toString;
  if (typeof method === 'function') {
    result = ReflectApply(method, O, []);
    if (!isObject(result)) return result;
  }
  // 4. Throw a TypeError exception.
  throw new _TypeError('Cannot convert object to primitive value');
}

// ECMA-262 7.1.1 ToPrimitive ( input [ , preferredType ] ), with
// preferredType number.
function ToPrimitiveNumber(input) {
  // 1. If input is an Object, then
  if (isObject(input)) {
    // a. Let exoticToPrim be ? GetMethod(input, %Symbol.toPrimitive%).
    var exoticToPrim = GetMethod(input, SymbolToPrimitive);
    // b. If exoticToPrim is not undefined, then
    if (exoticToPrim !== undefined) {
      // i. If preferredType is not present, ... Else if preferredType is
      //    string, ... iii. Else, let hint be "number".
      // iv. Let result be ? Call(exoticToPrim, input, « hint »).
      var result = ReflectApply(exoticToPrim, input, ['number']);
      // v. If result is not an Object, return result.
      if (!isObject(result)) return result;
      // vi. Throw a TypeError exception.
      throw new _TypeError('Cannot convert object to primitive value');
    }
    // c. If preferredType is not present, let preferredType be number.
    // d. Return ? OrdinaryToPrimitive(input, preferredType).
    return OrdinaryToPrimitiveNumber(input);
  }
  // 2. Return input.
  return input;
}

/// Whether c, a code unit, is a StrWhiteSpaceChar (ECMA-262 7.1.4.1): a
/// WhiteSpace (12.2: TAB, VT, FF, ZWNBSP and the Zs code points) or a
/// LineTerminator (12.3: LF, CR, LS, PS). All of them are BMP code points.
function isStrWhiteSpaceChar(c) {
  // 1. Return whether c is one of those code points.
  return (
    c === 0x09 ||
    c === 0x0a ||
    c === 0x0b ||
    c === 0x0c ||
    c === 0x0d ||
    c === 0x20 ||
    c === 0xa0 ||
    c === 0x1680 ||
    (c >= 0x2000 && c <= 0x200a) ||
    c === 0x2028 ||
    c === 0x2029 ||
    c === 0x202f ||
    c === 0x205f ||
    c === 0x3000 ||
    c === 0xfeff
  );
}

/// The value of c, a code unit, as a digit in bases up to 16, or 16 if it
/// is not one.
function hexDigitValue(c) {
  // 1. Return 0-9 for 0-9, and 10-15 for a-f and A-F.
  if (c >= 0x30 && c <= 0x39) return c - 0x30;
  if (c >= 0x61 && c <= 0x66) return c - 0x61 + 10;
  if (c >= 0x41 && c <= 0x46) return c - 0x41 + 10;
  return 16;
}

/// The StringIntlMV of the StrUnsignedDecimalLiteral that is the code units
/// of str from start to end: 'posInf' or a record. undefined if they are
/// not one.
function unsignedDecimalIntlMV(str, start, end) {
  // StrUnsignedDecimalLiteral ::: `Infinity`
  //   1. Return positive-infinity.
  if (end - start === 8 && stringSlice(str, start, end) === 'Infinity')
    return 'posInf';
  // StrUnsignedDecimalLiteral :::
  //   DecimalDigits `.` DecimalDigits? ExponentPart?
  //   `.` DecimalDigits ExponentPart?
  //   DecimalDigits ExponentPart?
  // Scan the digits, the `.`, the digits after it and the ExponentPart
  // once; the end of the function computes the value for the production
  // they match. DecimalDigits[~Sep] has no numeric separators.
  var i = start;
  while (i < end && isAsciiDigit(charCodeAt(str, i))) i++;
  var intEnd = i;
  var fracStart = i;
  var fracEnd = i;
  if (i < end && charCodeAt(str, i) === 0x2e) {
    fracStart = ++i;
    while (i < end && isAsciiDigit(charCodeAt(str, i))) i++;
    fracEnd = i;
  }
  // There are digits before or after the `.`.
  if (intEnd === start && fracEnd === fracStart) return undefined;
  // ExponentPart ::: ExponentIndicator SignedInteger, where
  // ExponentIndicator is `e` or `E` and SignedInteger is DecimalDigits with
  // an optional `+` or `-`.
  var e = 0;
  if (i < end && (charCodeAt(str, i) | 0x20) === 0x65) {
    i++;
    var expNeg = false;
    if (i < end && (charCodeAt(str, i) === 0x2b || charCodeAt(str, i) === 0x2d))
      expNeg = charCodeAt(str, i++) === 0x2d;
    var expStart = i;
    while (i < end && isAsciiDigit(charCodeAt(str, i))) i++;
    if (i === expStart) return undefined;
    // The MV of ExponentPart. Beyond 2^53 it is not exact, but such a value
    // is ±∞ or 0 after RoundMVResult, and ToIntlMathematicalValue returns
    // that instead.
    e = ToNumber(stringSlice(str, expStart, i));
    if (expNeg) e = -e;
  }
  if (i !== end) return undefined;
  // In each production below, the digits of a followed by those of b form
  // one integer D, and intlMVFromDigits returns D × 10^k exactly.
  var hasDot = fracStart !== intEnd;
  if (intEnd !== start && hasDot) {
    // StrUnsignedDecimalLiteral ::: DecimalDigits `.` DecimalDigits?
    //                               ExponentPart?
    //   1. Let a be MV of the first DecimalDigits.
    //   2. If the second DecimalDigits is present, then
    //      a. Let b be MV of the second DecimalDigits.
    //      b. Let n be the number of code points in the second
    //         DecimalDigits.
    //   3. Else,
    //      a. Let b be 0.
    //      b. Let n be 0.
    //      (An absent second DecimalDigits is the empty string here, with
    //      n = 0.)
    //   4. If ExponentPart is present, let e be MV of ExponentPart.
    //      Otherwise, let e be 0.
    //   5. Return (a + (b × 10^-n)) × 10^e, which is D × 10^(e - n).
    return intlMVFromDigits(
      false,
      stringSlice(str, start, intEnd) + stringSlice(str, fracStart, fracEnd),
      e - (fracEnd - fracStart),
    );
  }
  if (hasDot) {
    // StrUnsignedDecimalLiteral ::: `.` DecimalDigits ExponentPart?
    //   1. Let b be MV of DecimalDigits.
    //   2. If ExponentPart is present, let e be MV of ExponentPart.
    //      Otherwise, let e be 0.
    //   3. Let n be the number of code points in DecimalDigits.
    //   4. Return b × 10^(e - n).
    return intlMVFromDigits(
      false,
      stringSlice(str, fracStart, fracEnd),
      e - (fracEnd - fracStart),
    );
  }
  // StrUnsignedDecimalLiteral ::: DecimalDigits ExponentPart?
  //   1. Let a be MV of DecimalDigits.
  //   2. If ExponentPart is present, let e be MV of ExponentPart. Otherwise,
  //      let e be 0.
  //   3. Return a × 10^e.
  return intlMVFromDigits(false, stringSlice(str, start, intEnd), e);
}

// 16.5.14 Runtime Semantics: StringIntlMV, of the StrNumericLiteral that is
// the code units of str from start to end, with start < end. Returns
// undefined if they are not a StrNumericLiteral (ParseText fails).
function StringIntlMV(str, start, end) {
  var c = charCodeAt(str, start);
  // StrNumericLiteral ::: NonDecimalIntegerLiteral[~Sep]: `0b`, `0o` or
  // `0x`, in either case, and at least one digit of that base. It has no
  // sign.
  if (c === 0x30 && end - start >= 2) {
    var p = charCodeAt(str, start + 1) | 0x20;
    var radix = p === 0x62 ? 2 : p === 0x6f ? 8 : p === 0x78 ? 16 : 0;
    if (radix !== 0) {
      if (end - start === 2) return undefined;
      for (var i = start + 2; i < end; i++) {
        if (hexDigitValue(charCodeAt(str, i)) >= radix) return undefined;
      }
      // 1. Return MV of NonDecimalIntegerLiteral. The captured BigInt
      //    parses the literal exactly (ECMA-262 7.1.16 StringToBigInt).
      //    A literal whose value is beyond the range of a Number becomes
      //    positive-infinity in StringToIntlMV's step 9 (it has no sign),
      //    so that is returned here before the BigInt is made: one with
      //    more digits than a BigInt holds would fail. ToNumber of the
      //    literal is RoundMVResult of its MV (ECMA-262 7.1.4.1.2), as
      //    step 9 computes it. A finite value has at most 1024 bits.
      var literal = stringSlice(str, start, end);
      if (!NumberIsFinite(ToNumber(literal))) return 'posInf';
      return IntlMVFromBigInt(_BigInt(literal));
    }
  }
  // StrNumericLiteral ::: StrDecimalLiteral
  // StrDecimalLiteral ::: StrUnsignedDecimalLiteral
  //                       `+` StrUnsignedDecimalLiteral
  //                       `-` StrUnsignedDecimalLiteral
  var negative = c === 0x2d;
  if (c === 0x2b || c === 0x2d) start++;
  var a = unsignedDecimalIntlMV(str, start, end);
  if (a === undefined || !negative) return a;
  // StrDecimalLiteral ::: `-` StrUnsignedDecimalLiteral
  //   1. Let a be StringIntlMV of StrUnsignedDecimalLiteral.
  //   2. If a is 0, return negative-zero.
  //   3. If a is positive-infinity, return negative-infinity.
  //   4. Return -a.
  return intlMVNegate(a);
}

/// ToIntlMathematicalValue steps 5 to 10 for a String str.
function StringToIntlMV(str) {
  // 5. Let text be StringToCodePoints(str).
  // 6. Let literal be ParseText(text, StringNumericLiteral). Every code
  //    point of the grammar is in the BMP, so the parse reads code units; a
  //    surrogate never matches.
  //    StringNumericLiteral ::: StrWhiteSpace?
  //                             StrWhiteSpace? StrNumericLiteral StrWhiteSpace?
  var start = 0;
  var end = str.length;
  while (start < end && isStrWhiteSpaceChar(charCodeAt(str, start))) start++;
  while (end > start && isStrWhiteSpaceChar(charCodeAt(str, end - 1))) end--;
  // StringNumericLiteral ::: StrWhiteSpace?
  //   1. Return 0.
  if (start === end) return intlMVZero();
  // 8. Let intlMV be the StringIntlMV of literal.
  var intlMV = StringIntlMV(str, start, end);
  // 7. If literal is a List of errors, return not-a-number.
  if (intlMV === undefined) return 'nan';
  // 9. If intlMV is a mathematical value, then
  if (typeof intlMV === 'object') {
    // a. Let rounded be RoundMVResult(abs(intlMV)). ToNumber of the same
    //    literal is RoundMVResult of its MV, with its sign (ECMA-262
    //    7.1.4.1.2), so its absolute value is rounded.
    var rounded = ToNumber(stringSlice(str, start, end));
    // b. If rounded is +∞𝔽 and intlMV < 0, return negative-infinity.
    // c. If rounded is +∞𝔽, return positive-infinity.
    if (!NumberIsFinite(rounded)) return intlMV.neg ? 'negInf' : 'posInf';
    // d. If rounded is +0𝔽 and intlMV < 0, return negative-zero.
    // e. If rounded is +0𝔽, return 0.
    if (rounded === 0) return intlMV.neg ? 'negZero' : intlMVZero();
  }
  // 10. Return intlMV.
  return intlMV;
}

// 16.5.15 ToIntlMathematicalValue ( value )
function ToIntlMathematicalValue(value) {
  // 1. Let primValue be ? ToPrimitive(value, number).
  var primValue = ToPrimitiveNumber(value);
  // 2. If primValue is a BigInt, return ℝ(primValue).
  if (typeof primValue === 'bigint') return IntlMVFromBigInt(primValue);
  // 3. If primValue is a String, then
  //    a. Let str be primValue.
  // 5-10. Convert str.
  if (typeof primValue === 'string') return StringToIntlMV(primValue);
  // 4. Else,
  //    a. Let x be ? ToNumber(primValue).
  //    b-c, 5-10. Convert x.
  return IntlMVFromNumber(ToNumber(primValue));
}

/// The Unsigned Rounding Mode column of the table in 16.5.16, by Identifier
/// and Sign.
var UNSIGNED_ROUNDING_MODES = {
  __proto__: null,
  ceil: {__proto__: null, positive: 'infinity', negative: 'zero'},
  floor: {__proto__: null, positive: 'zero', negative: 'infinity'},
  expand: {__proto__: null, positive: 'infinity', negative: 'infinity'},
  trunc: {__proto__: null, positive: 'zero', negative: 'zero'},
  halfCeil: {__proto__: null, positive: 'half-infinity', negative: 'half-zero'},
  halfFloor: {
    __proto__: null,
    positive: 'half-zero',
    negative: 'half-infinity',
  },
  halfExpand: {
    __proto__: null,
    positive: 'half-infinity',
    negative: 'half-infinity',
  },
  halfTrunc: {__proto__: null, positive: 'half-zero', negative: 'half-zero'},
  halfEven: {__proto__: null, positive: 'half-even', negative: 'half-even'},
};

// 16.5.16 GetUnsignedRoundingMode ( roundingMode, sign ). sign is
// 'negative' or 'positive'; the result is one of 'infinity', 'zero',
// 'half-infinity', 'half-zero' and 'half-even'.
function GetUnsignedRoundingMode(roundingMode, sign) {
  // 1. Return the specification type in the Unsigned Rounding Mode column
  //    of the table for the row where the value in the Identifier column is
  //    roundingMode and the value in the Sign column is sign.
  return UNSIGNED_ROUNDING_MODES[roundingMode][sign];
}

// 16.5.17 ApplyUnsignedRoundingMode ( x, r1, r2, unsignedRoundingMode ), on
// the quotient and remainder of x divided by the rounding unit u > 0:
// x = (q + r / d) × u with BigInts q >= 0 and 0 <= r < d, r1 = q × u and
// r2 = (q + 1) × u. Returns q for r1 and q + 1n for r2.
function ApplyUnsignedRoundingMode(q, r, d, unsignedRoundingMode) {
  // 1. If x is r1, return r1.
  if (r === 0n) return q;
  // 2. Assert: r1 < x < r2.
  // 3. If unsignedRoundingMode is zero, return r1.
  if (unsignedRoundingMode === 'zero') return q;
  // 4. If unsignedRoundingMode is infinity, return r2.
  if (unsignedRoundingMode === 'infinity') return q + 1n;
  // 5. Let d1 be x – r1, which is r / d × u.
  // 6. Let d2 be r2 – x, which is (d - r) / d × u.
  //    So d1 < d2 exactly when 2r < d.
  var twice = 2n * r;
  // 7. If d1 < d2, return r1.
  if (twice < d) return q;
  // 8. If d2 < d1, return r2.
  if (twice > d) return q + 1n;
  // 9. Assert: d1 is d2.
  // 10. If unsignedRoundingMode is half-zero, return r1.
  if (unsignedRoundingMode === 'half-zero') return q;
  // 11. If unsignedRoundingMode is half-infinity, return r2.
  if (unsignedRoundingMode === 'half-infinity') return q + 1n;
  // 12. Assert: unsignedRoundingMode is half-even.
  // 13. Let cardinality be (r1 / (r2 – r1)) modulo 2, which is q modulo 2.
  // 14. If cardinality is 0, return r1.
  if (q % 2n === 0n) return q;
  // 15. Return r2.
  return q + 1n;
}

// Hermes limits a BigInt to 1024 64-bit digits, and its BigInt addition,
// subtraction, multiplication and division reserve one or two digits more
// than their operands have before they compute: a BigInt that uses the top
// 64 to 128 bits of the limit cannot be incremented, multiplied or divided
// at all, even by 1n. A record's n is as large as the BigInt it came from,
// so the rounding below works on the decimal digits of n and does BigInt
// arithmetic only on numbers at least 10^40 smaller.

/// The number of low decimal digits of n that roundToUnit rounds with
/// BigInt arithmetic; the digits above them only have a carry added.
var ROUNDING_LOW_DIGITS = 46;

/// The decimal digits of N, where N × 10^u is the multiple of
/// increment × 10^u that ApplyUnsignedRoundingMode chooses for a
/// non-negative record x and unsignedRoundingMode: x / (increment × 10^u)
/// rounded to an integer, times increment. "0" if N is 0.
function roundToUnit(x, u, increment, unsignedRoundingMode) {
  // 1. x = n × 10^shift × 10^u, with n the digits of x.n.
  var n = `${x.n}`;
  var shift = x.e - u;
  // 2. If x has digits below 10^(u-1), replace them by one digit 1 at
  //    10^(u-2), as intlMVFromDigits does: a multiple of the unit is a
  //    multiple of 10^u and half of it a multiple of 10^(u-1), so every
  //    rounding mode gives the same result (intlMVFromDigits step 4). The
  //    digits dropped end in a non-zero digit, since x is normalized. If
  //    all of n is dropped, n becomes "1".
  if (shift < -1) {
    var drop = -1 - shift;
    n = drop < n.length ? stringSlice(n, 0, n.length - drop) + '1' : '1';
    shift = -2;
  }
  // 3. Let s be max(shift, 0) and t be max(-shift, 0), so x / (increment ×
  //    10^u) = n × 10^s / d with d = increment × 10^t.
  var s = MathMax(shift, 0);
  var t = MathMax(0 - shift, 0);
  var d = _BigInt(increment) * bigPow10(t);
  // 4. Split n into its high digits a and its ROUNDING_LOW_DIGITS low
  //    digits b: n = a × 10^K + b with K = ROUNDING_LOW_DIGITS. Every
  //    increment divides 10^4 and t <= 2, so a × 10^K × 10^s / d is a × E
  //    with E = 10^(K+s) / d an even integer. The quotient q of n × 10^s / d
  //    is therefore a × E + qLow, where qLow and the remainder are those of
  //    b × 10^s / d, and q has the parity of qLow.
  var split = MathMax(n.length - ROUNDING_LOW_DIGITS, 0);
  var a = stringSlice(n, 0, split);
  var num = _BigInt(stringSlice(n, split, n.length)) * bigPow10(s);
  // 5. ApplyUnsignedRoundingMode chooses q or q + 1, which is a × E plus
  //    qLow or qLow + 1; times increment, N = a × 10^L + low with
  //    L = K + shift and low the chosen low quotient times increment.
  var low =
    ApplyUnsignedRoundingMode(num / d, num % d, d, unsignedRoundingMode) *
    _BigInt(increment);
  if (a === '') return `${low}`;
  // 6. low is less than 10^L + 10^4, so it carries at most 1 into a.
  var lowLength = ROUNDING_LOW_DIGITS + shift;
  var tenToL = bigPow10(lowLength);
  if (low >= tenToL) {
    a = `${_BigInt(a) + 1n}`;
    low -= tenToL;
  }
  // 7. Return the digits of a followed by those of low, padded to L digits.
  var lowDigits = `${low}`;
  return a + zeroString(lowLength - lowDigits.length) + lowDigits;
}

/// Whether a non-negative record x is a multiple of increment × 10^u, for
/// a roundingIncrement that SetNumberFormatDigitOptions allows: the case
/// where x is r1 in ApplyUnsignedRoundingMode, decided from the last digits
/// of x.n without BigInt arithmetic.
function isMultipleOfUnit(x, u, increment) {
  // 1. x / (increment × 10^u) = x.n × 10^shift / increment.
  var shift = x.e - u;
  // 2. If shift < 0, x is a multiple only if it is 0: increment × 10^-shift
  //    is a multiple of 10, and a non-zero x.n has no trailing zero.
  if (shift < 0) return x.n === 0n;
  // 3. Every increment divides 10^4, so x.n × 10^shift modulo increment
  //    depends only on the last four digits of x.n and on 10^min(shift, 4).
  //    Both are below 10^4, so the product is exact as a Number.
  var digits = `${x.n}`;
  var last = ToNumber(stringSlice(digits, MathMax(digits.length - 4, 0)));
  return (last * POWERS_OF_TEN_TO_4[MathMin(shift, 4)]) % increment === 0;
}

/// m without up to cut trailing zeros, and then without a trailing ".", as
/// ToRawPrecision and ToRawFixed end.
function cutTrailingZeros(m, cut) {
  var last = m.length;
  // 1. Repeat, while cut > 0 and the last code unit of m is 0x0030
  //    (DIGIT ZERO),
  //    a. Remove the last code unit from m.
  //    b. Set cut to cut - 1.
  while (cut > 0 && charCodeAt(m, last - 1) === 0x30) {
    last--;
    cut--;
  }
  // 2. If the last code unit of m is 0x002E (FULL STOP), then
  //    a. Remove the last code unit from m.
  if (charCodeAt(m, last - 1) === 0x2e) last--;
  return stringSlice(m, 0, last);
}

// 16.5.7 ToRawPrecision ( x, minPrecision, maxPrecision,
//                         unsignedRoundingMode )
// x is a non-negative record. The result's roundedNumber is a record.
function ToRawPrecision(x, minPrecision, maxPrecision, unsignedRoundingMode) {
  // 1. Let p be maxPrecision.
  var p = maxPrecision;
  var m, e, xFinal;
  if (x.n === 0n) {
    // 2. If x = 0, then
    //    a. Let m be the String consisting of p occurrences of the code unit
    //       0x0030 (DIGIT ZERO).
    m = zeroString(p);
    //    b. Let e be 0.
    e = 0;
    //    c. Let xFinal be 0.
    xFinal = intlMVZero();
  } else {
    // 3. Else,
    //    a. Let n1 and e1 each be an integer and r1 a mathematical value,
    //       with r1 = n1 × 10^(e1–p+1), 10^(p–1) ≤ n1 < 10^p, such that
    //       r1 ≤ x and r1 is maximized. Then e1 is the magnitude of x, and
    //       n1 is x / 10^(e1–p+1) rounded down.
    //    b. Let n2 and e2 each be an integer and r2 a mathematical value,
    //       with r2 = n2 × 10^(e2–p+1), 10^(p–1) ≤ n2 < 10^p, such that
    //       r2 ≥ x and r2 is minimized. Then r2 is (n1 + 1) × 10^(e1–p+1)
    //       when x is not r1, written with n2 = 10^(p–1) and e2 = e1 + 1
    //       if n1 + 1 is 10^p.
    //    c. Let xFinal be ApplyUnsignedRoundingMode(x, r1, r2,
    //       unsignedRoundingMode).
    var e1 = intlMVMagnitude(x);
    //       n has at most p + 1 digits.
    var n = _BigInt(roundToUnit(x, e1 - p + 1, 1, unsignedRoundingMode));
    //    d. If xFinal is r1, then
    //       i. Let n be n1.
    //       ii. Let e be e1.
    //    e. Else,
    //       i. Let n be n2.
    //       ii. Let e be e2.
    e = e1;
    if (n === bigPow10(p)) {
      n = bigPow10(p - 1);
      e = e1 + 1;
    }
    xFinal = makeIntlMV(false, n, e - p + 1);
    //    f. Let m be the String consisting of the digits of the decimal
    //       representation of n (in order, with no leading zeroes).
    m = `${n}`;
  }
  var int;
  if (e >= p - 1) {
    // 4. If e ≥ (p - 1), then
    //    a. Set m to the string-concatenation of m and e - p + 1
    //       occurrences of the code unit 0x0030 (DIGIT ZERO).
    m += zeroString(e - p + 1);
    //    b. Let int be e + 1.
    int = e + 1;
  } else if (e >= 0) {
    // 5. Else if e ≥ 0, then
    //    a. Set m to the string-concatenation of the first e + 1 code units
    //       of m, the code unit 0x002E (FULL STOP), and the remaining
    //       p - (e + 1) code units of m.
    m = stringSlice(m, 0, e + 1) + '.' + stringSlice(m, e + 1);
    //    b. Let int be e + 1.
    int = e + 1;
  } else {
    // 6. Else,
    //    a. Assert: e < 0.
    //    b. Set m to the string-concatenation of "0.", -(e + 1) occurrences
    //       of the code unit 0x0030 (DIGIT ZERO), and m.
    m = '0.' + zeroString(-(e + 1)) + m;
    //    c. Let int be 1.
    int = 1;
  }
  // 7. If m contains the code unit 0x002E (FULL STOP) and maxPrecision >
  //    minPrecision, then
  //    a. Let cut be maxPrecision - minPrecision.
  //    b-c. Remove up to cut trailing zeros, then a trailing ".".
  if (stringIndexOf(m, '.', 0) !== -1 && maxPrecision > minPrecision)
    m = cutTrailingZeros(m, maxPrecision - minPrecision);
  // 8. Return the Record { [[FormattedString]]: m, [[RoundedNumber]]:
  //    xFinal, [[IntegerDigitsCount]]: int, [[RoundingMagnitude]]: e–p+1 }.
  return {
    __proto__: null,
    formattedString: m,
    roundedNumber: xFinal,
    integerDigitsCount: int,
    roundingMagnitude: e - p + 1,
  };
}

// 16.5.8 ToRawFixed ( x, minFraction, maxFraction, roundingIncrement,
//                     unsignedRoundingMode )
// x is a non-negative record. The result's roundedNumber is a record.
function ToRawFixed(
  x,
  minFraction,
  maxFraction,
  roundingIncrement,
  unsignedRoundingMode,
) {
  // 1. Let f be maxFraction.
  var f = maxFraction;
  // 2. Let n1 be an integer and r1 a mathematical value, with
  //    r1 = n1 × 10^–f, such that n1 modulo roundingIncrement = 0, r1 ≤ x,
  //    and r1 is maximized.
  // 3. Let n2 be an integer and r2 a mathematical value, with
  //    r2 = n2 × 10^–f, such that n2 modulo roundingIncrement = 0, r2 ≥ x,
  //    and r2 is minimized.
  //    With the unit u = roundingIncrement × 10^–f, r1 is x / u rounded
  //    down, times u, and r2 is r1 + u when x is not r1.
  // 4. Let xFinal be ApplyUnsignedRoundingMode(x, r1, r2,
  //    unsignedRoundingMode).
  // 5. If xFinal is r1, let n be n1. Otherwise, let n be n2.
  // 6. If n = 0, let m be "0". Otherwise, let m be the String consisting of
  //    the digits of the decimal representation of n (in order, with no
  //    leading zeroes).
  var m, xFinal;
  if (isMultipleOfUnit(x, 0 - f, roundingIncrement)) {
    // x is r1 (ApplyUnsignedRoundingMode step 1), so xFinal is x, and n is
    // x.n × 10^(x.e + f): the digits of x.n and x.e + f zeros. This needs
    // no rounding, and no power of ten as large as 10^(x.e + f): a BigInt
    // like 10n ** 19000n has x.n = 1n and x.e = 19000.
    xFinal = x;
    m = x.n === 0n ? '0' : `${x.n}` + zeroString(x.e + f);
  } else {
    m = roundToUnit(x, 0 - f, roundingIncrement, unsignedRoundingMode);
    // xFinal is n × 10^-f, normalized from the digits of n, so that a
    // BigInt is made only of the digits without the trailing zeros. With
    // f = 0, the exponent is 0 - 0, +0.
    var end = m.length;
    while (end > 1 && charCodeAt(m, end - 1) === 0x30) end--;
    xFinal =
      m === '0'
        ? intlMVZero()
        : {
            __proto__: null,
            neg: false,
            n: _BigInt(stringSlice(m, 0, end)),
            e: m.length - end - f,
          };
  }
  var int;
  if (f !== 0) {
    // 7. If f ≠ 0, then
    //    a. Let k be the length of m.
    var k = m.length;
    //    b. If k ≤ f, then
    if (k <= f) {
      //   i. Let z be the String value consisting of f + 1 - k occurrences
      //      of the code unit 0x0030 (DIGIT ZERO).
      //   ii. Set m to the string-concatenation of z and m.
      m = zeroString(f + 1 - k) + m;
      //   iii. Set k to f + 1.
      k = f + 1;
    }
    //    c. Let a be the first k - f code units of m, and let b be the
    //       remaining f code units of m.
    var a = stringSlice(m, 0, k - f);
    var b = stringSlice(m, k - f);
    //    d. Set m to the string-concatenation of a, ".", and b.
    m = a + '.' + b;
    //    e. Let int be the length of a.
    int = a.length;
  } else {
    // 8. Else,
    //    a. Let int be the length of m.
    int = m.length;
  }
  // 9. Let cut be maxFraction - minFraction.
  // 10-11. Remove up to cut trailing zeros, then a trailing ".".
  m = cutTrailingZeros(m, maxFraction - minFraction);
  // 12. Return the Record { [[FormattedString]]: m, [[RoundedNumber]]:
  //     xFinal, [[IntegerDigitsCount]]: int, [[RoundingMagnitude]]: –f }.
  return {
    __proto__: null,
    formattedString: m,
    roundedNumber: xFinal,
    integerDigitsCount: int,
    roundingMagnitude: 0 - f,
  };
}

// 16.5.2 FormatNumericToString ( intlObj, x )
// intlObj is a record with the fields SetNumberFormatDigitOptions sets; x is
// a record or 'negZero'. Returns {__proto__: null, roundedNumber,
// formattedString}: roundedNumber is a record or 'negZero', and
// formattedString has ASCII digits and "." and no sign.
function FormatNumericToString(intlObj, x) {
  // 1. Assert: intlObj has [[RoundingMode]], [[RoundingType]],
  //    [[MinimumSignificantDigits]], [[MaximumSignificantDigits]],
  //    [[MinimumIntegerDigits]], [[MinimumFractionDigits]],
  //    [[MaximumFractionDigits]], [[RoundingIncrement]], and
  //    [[TrailingZeroDisplay]] internal slots.
  var sign;
  if (x === 'negZero') {
    // 2. If x is negative-zero, then
    //    a. Let sign be negative.
    sign = 'negative';
    //    b. Set x to 0.
    x = intlMVZero();
  } else {
    // 3. Else,
    //    a. Assert: x is a mathematical value.
    //    b. If x < 0, let sign be negative; else let sign be positive.
    sign = x.neg ? 'negative' : 'positive';
    //    c. If sign is negative, then
    //       i. Set x to -x.
    if (x.neg) x = intlMVNegate(x);
  }
  // 4. Let unsignedRoundingMode be
  //    GetUnsignedRoundingMode(intlObj.[[RoundingMode]], sign).
  var unsignedRoundingMode = GetUnsignedRoundingMode(
    intlObj.roundingMode,
    sign,
  );
  var result;
  if (intlObj.roundingType === 'significantDigits') {
    // 5. If intlObj.[[RoundingType]] is significant-digits, then
    //    a. Let result be ToRawPrecision(x,
    //       intlObj.[[MinimumSignificantDigits]],
    //       intlObj.[[MaximumSignificantDigits]], unsignedRoundingMode).
    result = ToRawPrecision(
      x,
      intlObj.minimumSignificantDigits,
      intlObj.maximumSignificantDigits,
      unsignedRoundingMode,
    );
  } else if (intlObj.roundingType === 'fractionDigits') {
    // 6. Else if intlObj.[[RoundingType]] is fraction-digits, then
    //    a. Let result be ToRawFixed(x, intlObj.[[MinimumFractionDigits]],
    //       intlObj.[[MaximumFractionDigits]],
    //       intlObj.[[RoundingIncrement]], unsignedRoundingMode).
    result = ToRawFixed(
      x,
      intlObj.minimumFractionDigits,
      intlObj.maximumFractionDigits,
      intlObj.roundingIncrement,
      unsignedRoundingMode,
    );
  } else {
    // 7. Else,
    //    a. Let significantResult be ToRawPrecision(x,
    //       intlObj.[[MinimumSignificantDigits]],
    //       intlObj.[[MaximumSignificantDigits]], unsignedRoundingMode).
    var significantResult = ToRawPrecision(
      x,
      intlObj.minimumSignificantDigits,
      intlObj.maximumSignificantDigits,
      unsignedRoundingMode,
    );
    //    b. Let fractionResult be ToRawFixed(x,
    //       intlObj.[[MinimumFractionDigits]],
    //       intlObj.[[MaximumFractionDigits]],
    //       intlObj.[[RoundingIncrement]], unsignedRoundingMode).
    var fractionResult = ToRawFixed(
      x,
      intlObj.minimumFractionDigits,
      intlObj.maximumFractionDigits,
      intlObj.roundingIncrement,
      unsignedRoundingMode,
    );
    //    c. If fractionResult.[[RoundingMagnitude]] <
    //       significantResult.[[RoundingMagnitude]], let fixedIsMorePrecise
    //       be true; else let fixedIsMorePrecise be false.
    var fixedIsMorePrecise =
      fractionResult.roundingMagnitude < significantResult.roundingMagnitude;
    if (intlObj.roundingType === 'morePrecision' && fixedIsMorePrecise) {
      //  d. If intlObj.[[RoundingType]] is more-precision and
      //     fixedIsMorePrecise is true, then
      //     i. Let result be fractionResult.
      result = fractionResult;
    } else if (
      intlObj.roundingType === 'lessPrecision' &&
      !fixedIsMorePrecise
    ) {
      //  e. Else if intlObj.[[RoundingType]] is less-precision and
      //     fixedIsMorePrecise is false, then
      //     i. Let result be fractionResult.
      result = fractionResult;
    } else {
      //  f. Else,
      //     i. Let result be significantResult.
      result = significantResult;
    }
  }
  // 8. Set x to result.[[RoundedNumber]].
  x = result.roundedNumber;
  // 9. Let string be result.[[FormattedString]].
  var string = result.formattedString;
  // 10. If intlObj.[[TrailingZeroDisplay]] is "stripIfInteger" and x modulo
  //     1 = 0, then
  if (intlObj.trailingZeroDisplay === 'stripIfInteger' && intlMVIsInteger(x)) {
    // a. Let i be StringIndexOf(string, ".", 0).
    var i = stringIndexOf(string, '.', 0);
    // b. If i is not not-found, set string to the substring of string from
    //    0 to i.
    if (i !== -1) string = stringSlice(string, 0, i);
  }
  // 11. Let int be result.[[IntegerDigitsCount]].
  var int = result.integerDigitsCount;
  // 12. Let minIntDigits be intlObj.[[MinimumIntegerDigits]].
  var minIntDigits = intlObj.minimumIntegerDigits;
  // 13. If int < minIntDigits, then
  if (int < minIntDigits) {
    // a. Let forwardZeros be the String consisting of minIntDigits - int
    //    occurrences of the code unit 0x0030 (DIGIT ZERO).
    // b. Set string to the string-concatenation of forwardZeros and string.
    string = zeroString(minIntDigits - int) + string;
  }
  // 14. If sign is negative, then
  //     a. If x is 0, set x to negative-zero. Otherwise, set x to -x.
  if (sign === 'negative') x = intlMVNegate(x);
  // 15. Return the Record { [[RoundedNumber]]: x, [[FormattedString]]:
  //     string }.
  return {__proto__: null, roundedNumber: x, formattedString: string};
}

// 16.5.13 ComputeExponentForMagnitude ( numberFormat, magnitude )
// numberFormat is a record with the notation field and the fields
// SetNumberFormatDigitOptions sets. compactExponentOfMagnitude, a function
// from a magnitude to the exponent the locale's compact patterns use for
// it, is called only when the notation is "compact".
function ComputeExponentForMagnitude(
  numberFormat,
  magnitude,
  compactExponentOfMagnitude,
) {
  // 1. Let notation be numberFormat.[[Notation]].
  var notation = numberFormat.notation;
  // 2. If notation is "standard", then
  //    a. Return 0.
  if (notation === 'standard') return 0;
  // 3. Else if notation is "scientific", then
  //    a. Return magnitude.
  if (notation === 'scientific') return magnitude;
  // 4. Else if notation is "engineering", then
  if (notation === 'engineering') {
    // a. Let thousands be the greatest integer that is not greater than
    //    magnitude / 3.
    var thousands = MathFloor(magnitude / 3);
    // b. Return thousands × 3.
    return thousands * 3;
  }
  // 5. Else,
  //    a. Assert: notation is "compact".
  //    b. Let exponent be an ILD integer by which to scale a number of the
  //       given magnitude in compact notation for the current locale.
  //    c. Return exponent.
  return compactExponentOfMagnitude(magnitude);
}

// 16.5.12 ComputeExponent ( numberFormat, x )
// x is a record; numberFormat and compactExponentOfMagnitude are as in
// ComputeExponentForMagnitude.
//
// Deviation from the draft: step 2 is read as applying only to the
// magnitude (step 3). The trial rounding of step 6 uses the signed x, so a
// directed rounding mode (ceil, floor, halfCeil, halfFloor) rounds the way
// PartitionNumberPattern's own FormatNumericToString of the signed value
// does. Read literally, -950 in scientific notation with ceil and
// maximumFractionDigits 0 rounds 9.5 up to 10 and takes exponent 3, and
// the number then formats as -0.95 rounded toward zero: the mantissa "-0".
// With the signed value the exponent is 2 ("-9E2", as ICU gives).
function ComputeExponent(numberFormat, x, compactExponentOfMagnitude) {
  // 1. If x = 0, then
  //    a. Return 0.
  if (x.n === 0n) return 0;
  //    For the standard notation, ComputeExponentForMagnitude is 0 for
  //    every magnitude, so steps 4, 9 and 10 all return 0: return it
  //    without the trial rounding of step 6, which has no other effect.
  if (numberFormat.notation === 'standard') return 0;
  // 2. If x < 0, then
  //    a. Let x = -x.
  //    Only the magnitude uses abs(x): intlMVMagnitude ignores the sign, so
  //    x keeps it for step 6.
  // 3. Let magnitude be floor(log10(x)).
  var magnitude = intlMVMagnitude(x);
  // 4. Let exponent be ComputeExponentForMagnitude(numberFormat, magnitude).
  var exponent = ComputeExponentForMagnitude(
    numberFormat,
    magnitude,
    compactExponentOfMagnitude,
  );
  // 5. Set x to x × 10^-exponent.
  x = intlMVScale(x, -exponent);
  // 6. Let formatNumberResult be FormatNumericToString(numberFormat, x).
  var formatNumberResult = FormatNumericToString(numberFormat, x);
  // 7. If formatNumberResult.[[RoundedNumber]] = 0, then
  //    a. Return exponent.
  //    A negative x that rounds to 0 gives negative-zero.
  var rounded = formatNumberResult.roundedNumber;
  if (rounded === 'negZero' || intlMVIsZero(rounded)) return exponent;
  // 8. Let newMagnitude be
  //    floor(log10(formatNumberResult.[[RoundedNumber]])).
  //    With the sign kept (the step 2 deviation), this is the magnitude of
  //    abs(formatNumberResult.[[RoundedNumber]]), which intlMVMagnitude
  //    computes.
  var newMagnitude = intlMVMagnitude(rounded);
  // 9. If newMagnitude is magnitude - exponent, then
  //    a. Return exponent.
  if (newMagnitude === magnitude - exponent) return exponent;
  // 10. Return ComputeExponentForMagnitude(numberFormat, magnitude + 1).
  return ComputeExponentForMagnitude(
    numberFormat,
    magnitude + 1,
    compactExponentOfMagnitude,
  );
}
