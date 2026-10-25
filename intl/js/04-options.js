/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// ECMA-402 options processing (9.2.10 to 9.2.14, 16.1.2), following the
// draft https://tc39.es/ecma402/ dated October 24, 2026. GetOptionsObject is
// ECMA-262 7.3.36 in the https://tc39.es/ecma262/ draft of the same date.

/// The `default` argument of GetOption that makes the option required.
var REQUIRED = {__proto__: null};

// ECMA-262 7.3.36 GetOptionsObject ( options )
function GetOptionsObject(options) {
  // 1. If options is undefined, then
  //    a. Return OrdinaryObjectCreate(null).
  if (options === undefined) return ObjectCreate(null);
  // 2. If options is an Object, then
  //    a. Return options.
  if (
    (typeof options === 'object' && options !== null) ||
    typeof options === 'function'
  )
    return options;
  // 3. Throw a TypeError exception.
  throw new _TypeError('Options must be an object');
}

// 9.2.10 CoerceOptionsToObject ( options )
function CoerceOptionsToObject(options) {
  // 1. If options is undefined, then
  //    a. Return OrdinaryObjectCreate(null).
  if (options === undefined) return ObjectCreate(null);
  // 2. Return ? ToObject(options).
  return ToObject(options);
}

// 9.2.11 GetOption ( options, propertyKey, type, values, default )
// type is 'boolean' or 'string'; values is a List, or undefined for empty;
// default may be REQUIRED.
function GetOption(options, property, type, values, defaultValue) {
  // 1. Let value be ? Get(options, propertyKey).
  var value = options[property];
  // 2. If value is undefined, then
  if (value === undefined) {
    // a. If default is required, throw a TypeError exception.
    if (defaultValue === REQUIRED)
      throw new _TypeError(property + ' is required');
    // b. Return default.
    return defaultValue;
  }
  if (type === 'boolean') {
    // 3. If type is boolean, then
    //    a. Set value to ToBoolean(value).
    value = !!value;
  } else {
    // 4. Else,
    //    a. Assert: type is string.
    //    b. Set value to ? ToString(value).
    value = ToString(value);
  }
  // 5. If values is not empty and values does not contain value, throw a
  //    RangeError exception.
  if (values !== undefined && !listContains(values, value))
    throw new _RangeError(
      'Value ' + ToString(value) + ' out of range for ' + property,
    );
  // 6. Return value.
  return value;
}

// 9.2.12 GetBooleanOrStringNumberFormatOption ( options, propertyKey,
//                                              stringValues, fallback )
function GetBooleanOrStringNumberFormatOption(
  options,
  property,
  stringValues,
  fallback,
) {
  // 1. Let value be ? Get(options, propertyKey).
  var value = options[property];
  // 2. If value is undefined, return fallback.
  if (value === undefined) return fallback;
  // 3. If value is true, return true.
  if (value === true) return true;
  // 4. If ToBoolean(value) is false, return false.
  if (!value) return false;
  // 5. Set value to ? ToString(value).
  value = ToString(value);
  // 6. If stringValues does not contain value, throw a RangeError exception.
  if (!listContains(stringValues, value))
    throw new _RangeError('Value ' + value + ' out of range for ' + property);
  // 7. Return value.
  return value;
}

// 9.2.13 DefaultNumberOption ( value, minimum, maximum, fallback )
// property only names the option in the error message.
function DefaultNumberOption(value, minimum, maximum, fallback, property) {
  // 1. If value is undefined, return fallback.
  if (value === undefined) return fallback;
  // 2. Set value to ? ToNumber(value).
  value = ToNumber(value);
  // 3. If value is not a finite Number or ℝ(value) < minimum or
  //    ℝ(value) > maximum, throw a RangeError exception.
  if (!NumberIsFinite(value) || value < minimum || value > maximum)
    throw new _RangeError(
      (property === undefined ? 'Value' : property) + ' value is out of range',
    );
  // 4. Return floor(ℝ(value)). ℝ(-0𝔽) is the mathematical 0, so the result
  //    is +0 even where Math.floor would return -0; adding +0 turns -0 into
  //    +0 and leaves every other value unchanged.
  return MathFloor(value) + 0;
}

// 9.2.14 GetNumberOption ( options, propertyKey, minimum, maximum,
//                          fallback )
function GetNumberOption(options, property, minimum, maximum, fallback) {
  // 1. Let value be ? Get(options, propertyKey).
  var value = options[property];
  // 2. Return ? DefaultNumberOption(value, minimum, maximum, fallback).
  return DefaultNumberOption(value, minimum, maximum, fallback, property);
}

/// The roundingIncrement values ECMA-402 16.1.2 step 8 allows.
var ROUNDING_INCREMENTS = [
  1, 2, 5, 10, 20, 25, 50, 100, 200, 250, 500, 1000, 2000, 2500, 5000,
];

/// The roundingMode values ECMA-402 16.1.2 step 9 allows.
var ROUNDING_MODES = [
  'ceil',
  'floor',
  'expand',
  'trunc',
  'halfCeil',
  'halfFloor',
  'halfExpand',
  'halfTrunc',
  'halfEven',
];

// 16.1.2 SetNumberFormatDigitOptions ( intlObj, options, minFDDefault,
//                                      maxFDDefault, notation )
// intlObj is a null-prototype record; its slots are lowerCamelCase fields.
// The [[RoundingType]] enums more-precision, less-precision,
// significant-digits and fraction-digits are the strings "morePrecision",
// "lessPrecision", "significantDigits" and "fractionDigits".
function SetNumberFormatDigitOptions(
  intlObj,
  options,
  minFDDefault,
  maxFDDefault,
  notation,
) {
  // 1. Let minIntegerDigits be ? GetNumberOption(options,
  //    "minimumIntegerDigits", 1, 21, 1).
  var minIntegerDigits = GetNumberOption(
    options,
    'minimumIntegerDigits',
    1,
    21,
    1,
  );
  // 2. Let minFD be ? Get(options, "minimumFractionDigits").
  var minFD = options.minimumFractionDigits;
  // 3. Let maxFD be ? Get(options, "maximumFractionDigits").
  var maxFD = options.maximumFractionDigits;
  // 4. Let minSD be ? Get(options, "minimumSignificantDigits").
  var minSD = options.minimumSignificantDigits;
  // 5. Let maxSD be ? Get(options, "maximumSignificantDigits").
  var maxSD = options.maximumSignificantDigits;
  // 6. Let roundingIncrement be ? GetNumberOption(options,
  //    "roundingIncrement", 1, 5000, 1).
  var roundingIncrement = GetNumberOption(
    options,
    'roundingIncrement',
    1,
    5000,
    1,
  );
  // 7. If roundingIncrement is not in « 1, 2, 5, 10, 20, 25, 50, 100, 200,
  //    250, 500, 1000, 2000, 2500, 5000 », throw a RangeError exception.
  if (!listContains(ROUNDING_INCREMENTS, roundingIncrement))
    throw new _RangeError('roundingIncrement value is out of range');
  // 8. Let roundingMode be ? GetOption(options, "roundingMode", string,
  //    « "ceil", "floor", "expand", "trunc", "halfCeil", "halfFloor",
  //    "halfExpand", "halfTrunc", "halfEven" », "halfExpand").
  var roundingMode = GetOption(
    options,
    'roundingMode',
    'string',
    ROUNDING_MODES,
    'halfExpand',
  );
  // 9. Let roundingPriority be ? GetOption(options, "roundingPriority",
  //    string, « "auto", "morePrecision", "lessPrecision" », "auto").
  var roundingPriority = GetOption(
    options,
    'roundingPriority',
    'string',
    ['auto', 'morePrecision', 'lessPrecision'],
    'auto',
  );
  // 10. Let trailingZeroDisplay be ? GetOption(options,
  //     "trailingZeroDisplay", string, « "auto", "stripIfInteger" »,
  //     "auto").
  var trailingZeroDisplay = GetOption(
    options,
    'trailingZeroDisplay',
    'string',
    ['auto', 'stripIfInteger'],
    'auto',
  );
  // 11. NOTE: The remaining steps perform no further interaction with
  //     options.
  // 12. If roundingIncrement is not 1, set maxFDDefault to minFDDefault.
  if (roundingIncrement !== 1) maxFDDefault = minFDDefault;
  // 13. Set intlObj.[[MinimumIntegerDigits]] to minIntegerDigits.
  intlObj.minimumIntegerDigits = minIntegerDigits;
  // 14. Set intlObj.[[RoundingIncrement]] to roundingIncrement.
  intlObj.roundingIncrement = roundingIncrement;
  // 15. Set intlObj.[[RoundingMode]] to roundingMode.
  intlObj.roundingMode = roundingMode;
  // 16. Set intlObj.[[TrailingZeroDisplay]] to trailingZeroDisplay.
  intlObj.trailingZeroDisplay = trailingZeroDisplay;
  // 17. If minSD is undefined and maxSD is undefined, let hasSD be false.
  //     Otherwise, let hasSD be true.
  var hasSD = !(minSD === undefined && maxSD === undefined);
  // 18. If minFD is undefined and maxFD is undefined, let hasFD be false.
  //     Otherwise, let hasFD be true.
  var hasFD = !(minFD === undefined && maxFD === undefined);
  var needSD, needFD;
  if (roundingPriority === 'auto') {
    // 19. If roundingPriority is "auto", then
    //     a. Let needSD be hasSD.
    needSD = hasSD;
    //     b. If needSD is true, let needFD be false.
    //     c. Else if hasFD is false and notation is "compact", let needFD
    //        be false.
    //     d. Else, let needFD be true.
    needFD = !(needSD || (!hasFD && notation === 'compact'));
  } else {
    // 20. Else,
    //     a. Let needSD be true.
    needSD = true;
    //     b. Let needFD be true.
    needFD = true;
  }
  // 21. If needSD is true, then
  if (needSD) {
    if (hasSD) {
      // a. If hasSD is true, then
      //    i. Let resolvedMinSD be ? DefaultNumberOption(minSD, 1, 21, 1).
      var resolvedMinSD = DefaultNumberOption(
        minSD,
        1,
        21,
        1,
        'minimumSignificantDigits',
      );
      //    ii. Let resolvedMaxSD be ? DefaultNumberOption(maxSD,
      //        resolvedMinSD, 21, 21).
      var resolvedMaxSD = DefaultNumberOption(
        maxSD,
        resolvedMinSD,
        21,
        21,
        'maximumSignificantDigits',
      );
      //    iii. Set intlObj.[[MinimumSignificantDigits]] to resolvedMinSD.
      intlObj.minimumSignificantDigits = resolvedMinSD;
      //    iv. Set intlObj.[[MaximumSignificantDigits]] to resolvedMaxSD.
      intlObj.maximumSignificantDigits = resolvedMaxSD;
    } else {
      // b. Else,
      //    i. Set intlObj.[[MinimumSignificantDigits]] to 1.
      intlObj.minimumSignificantDigits = 1;
      //    ii. Set intlObj.[[MaximumSignificantDigits]] to 21.
      intlObj.maximumSignificantDigits = 21;
    }
  }
  // 22. If needFD is true, then
  if (needFD) {
    if (hasFD) {
      // a. If hasFD is true, then
      //    i. Let resolvedMinFD be ? DefaultNumberOption(minFD, 0, 100,
      //       undefined).
      var resolvedMinFD = DefaultNumberOption(
        minFD,
        0,
        100,
        undefined,
        'minimumFractionDigits',
      );
      //    ii. Let resolvedMaxFD be ? DefaultNumberOption(maxFD, 0, 100,
      //        undefined).
      var resolvedMaxFD = DefaultNumberOption(
        maxFD,
        0,
        100,
        undefined,
        'maximumFractionDigits',
      );
      if (resolvedMinFD === undefined) {
        //  iii. If resolvedMinFD is undefined, set resolvedMinFD to
        //       min(minFDDefault, resolvedMaxFD).
        resolvedMinFD = MathMin(minFDDefault, resolvedMaxFD);
      } else if (resolvedMaxFD === undefined) {
        //  iv. Else if resolvedMaxFD is undefined, set resolvedMaxFD to
        //      max(maxFDDefault, resolvedMinFD).
        resolvedMaxFD = MathMax(maxFDDefault, resolvedMinFD);
      } else if (resolvedMinFD > resolvedMaxFD) {
        //  v. Else if resolvedMinFD > resolvedMaxFD, throw a RangeError
        //     exception.
        throw new _RangeError(
          'minimumFractionDigits is greater than maximumFractionDigits',
        );
      }
      //    vi. Set intlObj.[[MinimumFractionDigits]] to resolvedMinFD.
      intlObj.minimumFractionDigits = resolvedMinFD;
      //    vii. Set intlObj.[[MaximumFractionDigits]] to resolvedMaxFD.
      intlObj.maximumFractionDigits = resolvedMaxFD;
    } else {
      // b. Else,
      //    i. Set intlObj.[[MinimumFractionDigits]] to minFDDefault.
      intlObj.minimumFractionDigits = minFDDefault;
      //    ii. Set intlObj.[[MaximumFractionDigits]] to maxFDDefault.
      intlObj.maximumFractionDigits = maxFDDefault;
    }
  }
  if (!needSD && !needFD) {
    // 23. If needSD is false and needFD is false, then
    //     a. Set intlObj.[[MinimumFractionDigits]] to 0.
    intlObj.minimumFractionDigits = 0;
    //     b. Set intlObj.[[MaximumFractionDigits]] to 0.
    intlObj.maximumFractionDigits = 0;
    //     c. Set intlObj.[[MinimumSignificantDigits]] to 1.
    intlObj.minimumSignificantDigits = 1;
    //     d. Set intlObj.[[MaximumSignificantDigits]] to 2.
    intlObj.maximumSignificantDigits = 2;
    //     e. Set intlObj.[[RoundingType]] to more-precision.
    intlObj.roundingType = 'morePrecision';
    //     f. Set intlObj.[[ComputedRoundingPriority]] to "morePrecision".
    intlObj.computedRoundingPriority = 'morePrecision';
  } else if (roundingPriority === 'morePrecision') {
    // 24. Else if roundingPriority is "morePrecision", then
    //     a. Set intlObj.[[RoundingType]] to more-precision.
    intlObj.roundingType = 'morePrecision';
    //     b. Set intlObj.[[ComputedRoundingPriority]] to "morePrecision".
    intlObj.computedRoundingPriority = 'morePrecision';
  } else if (roundingPriority === 'lessPrecision') {
    // 25. Else if roundingPriority is "lessPrecision", then
    //     a. Set intlObj.[[RoundingType]] to less-precision.
    intlObj.roundingType = 'lessPrecision';
    //     b. Set intlObj.[[ComputedRoundingPriority]] to "lessPrecision".
    intlObj.computedRoundingPriority = 'lessPrecision';
  } else if (hasSD) {
    // 26. Else if hasSD is true, then
    //     a. Set intlObj.[[RoundingType]] to significant-digits.
    intlObj.roundingType = 'significantDigits';
    //     b. Set intlObj.[[ComputedRoundingPriority]] to "auto".
    intlObj.computedRoundingPriority = 'auto';
  } else {
    // 27. Else,
    //     a. Set intlObj.[[RoundingType]] to fraction-digits.
    intlObj.roundingType = 'fractionDigits';
    //     b. Set intlObj.[[ComputedRoundingPriority]] to "auto".
    intlObj.computedRoundingPriority = 'auto';
  }
  // 28. If roundingIncrement is not 1, then
  if (roundingIncrement !== 1) {
    // a. If intlObj.[[RoundingType]] is not fraction-digits, throw a
    //    TypeError exception.
    if (intlObj.roundingType !== 'fractionDigits')
      throw new _TypeError(
        'roundingIncrement requires fraction-digit rounding',
      );
    // b. If intlObj.[[MaximumFractionDigits]] is not
    //    intlObj.[[MinimumFractionDigits]], throw a RangeError exception.
    if (intlObj.maximumFractionDigits !== intlObj.minimumFractionDigits)
      throw new _RangeError(
        'roundingIncrement requires equal minimum and maximum fraction digits',
      );
  }
  // 29. Return unused.
}
