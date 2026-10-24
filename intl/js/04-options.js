/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// ECMA-402 options processing (9.2.10 to 9.2.14, 16.1.2), following the
// draft https://tc39.es/ecma402/ dated October 22, 2026. GetOptionsObject is
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

// 16.1.2 SetNumberFormatDigitOptions ( intlObj, options, mnfdDefault,
//                                      mxfdDefault, notation )
// intlObj is a null-prototype record; its slots are lowerCamelCase fields.
// The [[RoundingType]] enums more-precision, less-precision,
// significant-digits and fraction-digits are the strings "morePrecision",
// "lessPrecision", "significantDigits" and "fractionDigits".
function SetNumberFormatDigitOptions(
  intlObj,
  options,
  mnfdDefault,
  mxfdDefault,
  notation,
) {
  // 1. Let mnid be ? GetNumberOption(options, "minimumIntegerDigits", 1, 21,
  //    1).
  var mnid = GetNumberOption(options, 'minimumIntegerDigits', 1, 21, 1);
  // 2. Let mnfd be ? Get(options, "minimumFractionDigits").
  var mnfd = options.minimumFractionDigits;
  // 3. Let mxfd be ? Get(options, "maximumFractionDigits").
  var mxfd = options.maximumFractionDigits;
  // 4. Let mnsd be ? Get(options, "minimumSignificantDigits").
  var mnsd = options.minimumSignificantDigits;
  // 5. Let mxsd be ? Get(options, "maximumSignificantDigits").
  var mxsd = options.maximumSignificantDigits;
  // 6. Set intlObj.[[MinimumIntegerDigits]] to mnid.
  intlObj.minimumIntegerDigits = mnid;
  // 7. Let roundingIncrement be ? GetNumberOption(options,
  //    "roundingIncrement", 1, 5000, 1).
  var roundingIncrement = GetNumberOption(
    options,
    'roundingIncrement',
    1,
    5000,
    1,
  );
  // 8. If roundingIncrement is not in « 1, 2, 5, 10, 20, 25, 50, 100, 200,
  //    250, 500, 1000, 2000, 2500, 5000 », throw a RangeError exception.
  if (!listContains(ROUNDING_INCREMENTS, roundingIncrement))
    throw new _RangeError('roundingIncrement value is out of range');
  // 9. Let roundingMode be ? GetOption(options, "roundingMode", string,
  //    « "ceil", "floor", "expand", "trunc", "halfCeil", "halfFloor",
  //    "halfExpand", "halfTrunc", "halfEven" », "halfExpand").
  var roundingMode = GetOption(
    options,
    'roundingMode',
    'string',
    ROUNDING_MODES,
    'halfExpand',
  );
  // 10. Let roundingPriority be ? GetOption(options, "roundingPriority",
  //     string, « "auto", "morePrecision", "lessPrecision" », "auto").
  var roundingPriority = GetOption(
    options,
    'roundingPriority',
    'string',
    ['auto', 'morePrecision', 'lessPrecision'],
    'auto',
  );
  // 11. Let trailingZeroDisplay be ? GetOption(options,
  //     "trailingZeroDisplay", string, « "auto", "stripIfInteger" »,
  //     "auto").
  var trailingZeroDisplay = GetOption(
    options,
    'trailingZeroDisplay',
    'string',
    ['auto', 'stripIfInteger'],
    'auto',
  );
  // 12. NOTE: All fields required by SetNumberFormatDigitOptions have now
  //     been read from options. The remainder of this AO interprets the
  //     options and may throw exceptions.
  // 13. If roundingIncrement is not 1, set mxfdDefault to mnfdDefault.
  if (roundingIncrement !== 1) mxfdDefault = mnfdDefault;
  // 14. Set intlObj.[[RoundingIncrement]] to roundingIncrement.
  intlObj.roundingIncrement = roundingIncrement;
  // 15. Set intlObj.[[RoundingMode]] to roundingMode.
  intlObj.roundingMode = roundingMode;
  // 16. Set intlObj.[[TrailingZeroDisplay]] to trailingZeroDisplay.
  intlObj.trailingZeroDisplay = trailingZeroDisplay;
  // 17. If mnsd is undefined and mxsd is undefined, let hasSd be false.
  //     Otherwise, let hasSd be true.
  var hasSd = !(mnsd === undefined && mxsd === undefined);
  // 18. If mnfd is undefined and mxfd is undefined, let hasFd be false.
  //     Otherwise, let hasFd be true.
  var hasFd = !(mnfd === undefined && mxfd === undefined);
  // 19. Let needSd be true.
  var needSd = true;
  // 20. Let needFd be true.
  var needFd = true;
  // 21. If roundingPriority is "auto", then
  if (roundingPriority === 'auto') {
    // a. Set needSd to hasSd.
    needSd = hasSd;
    // b. If needSd is true, or hasFd is false and notation is "compact",
    //    then
    //    i. Set needFd to false.
    if (needSd || (!hasFd && notation === 'compact')) needFd = false;
  }
  // 22. If needSd is true, then
  if (needSd) {
    if (hasSd) {
      // a. If hasSd is true, then
      //    i. Set intlObj.[[MinimumSignificantDigits]] to
      //       ? DefaultNumberOption(mnsd, 1, 21, 1).
      intlObj.minimumSignificantDigits = DefaultNumberOption(
        mnsd,
        1,
        21,
        1,
        'minimumSignificantDigits',
      );
      //    ii. Set intlObj.[[MaximumSignificantDigits]] to
      //        ? DefaultNumberOption(mxsd,
      //        intlObj.[[MinimumSignificantDigits]], 21, 21).
      intlObj.maximumSignificantDigits = DefaultNumberOption(
        mxsd,
        intlObj.minimumSignificantDigits,
        21,
        21,
        'maximumSignificantDigits',
      );
    } else {
      // b. Else,
      //    i. Set intlObj.[[MinimumSignificantDigits]] to 1.
      intlObj.minimumSignificantDigits = 1;
      //    ii. Set intlObj.[[MaximumSignificantDigits]] to 21.
      intlObj.maximumSignificantDigits = 21;
    }
  }
  // 23. If needFd is true, then
  if (needFd) {
    if (hasFd) {
      // a. If hasFd is true, then
      //    i. Set mnfd to ? DefaultNumberOption(mnfd, 0, 100, undefined).
      mnfd = DefaultNumberOption(
        mnfd,
        0,
        100,
        undefined,
        'minimumFractionDigits',
      );
      //    ii. Set mxfd to ? DefaultNumberOption(mxfd, 0, 100, undefined).
      mxfd = DefaultNumberOption(
        mxfd,
        0,
        100,
        undefined,
        'maximumFractionDigits',
      );
      if (mnfd === undefined) {
        //  iii. If mnfd is undefined, set mnfd to min(mnfdDefault, mxfd).
        mnfd = MathMin(mnfdDefault, mxfd);
      } else if (mxfd === undefined) {
        //  iv. Else if mxfd is undefined, set mxfd to max(mxfdDefault,
        //      mnfd).
        mxfd = MathMax(mxfdDefault, mnfd);
      } else if (mnfd > mxfd) {
        //  v. Else if mnfd is greater than mxfd, throw a RangeError
        //     exception.
        throw new _RangeError(
          'minimumFractionDigits is greater than maximumFractionDigits',
        );
      }
      //    vi. Set intlObj.[[MinimumFractionDigits]] to mnfd.
      intlObj.minimumFractionDigits = mnfd;
      //    vii. Set intlObj.[[MaximumFractionDigits]] to mxfd.
      intlObj.maximumFractionDigits = mxfd;
    } else {
      // b. Else,
      //    i. Set intlObj.[[MinimumFractionDigits]] to mnfdDefault.
      intlObj.minimumFractionDigits = mnfdDefault;
      //    ii. Set intlObj.[[MaximumFractionDigits]] to mxfdDefault.
      intlObj.maximumFractionDigits = mxfdDefault;
    }
  }
  if (!needSd && !needFd) {
    // 24. If needSd is false and needFd is false, then
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
    // 25. Else if roundingPriority is "morePrecision", then
    //     a. Set intlObj.[[RoundingType]] to more-precision.
    intlObj.roundingType = 'morePrecision';
    //     b. Set intlObj.[[ComputedRoundingPriority]] to "morePrecision".
    intlObj.computedRoundingPriority = 'morePrecision';
  } else if (roundingPriority === 'lessPrecision') {
    // 26. Else if roundingPriority is "lessPrecision", then
    //     a. Set intlObj.[[RoundingType]] to less-precision.
    intlObj.roundingType = 'lessPrecision';
    //     b. Set intlObj.[[ComputedRoundingPriority]] to "lessPrecision".
    intlObj.computedRoundingPriority = 'lessPrecision';
  } else if (hasSd) {
    // 27. Else if hasSd is true, then
    //     a. Set intlObj.[[RoundingType]] to significant-digits.
    intlObj.roundingType = 'significantDigits';
    //     b. Set intlObj.[[ComputedRoundingPriority]] to "auto".
    intlObj.computedRoundingPriority = 'auto';
  } else {
    // 28. Else,
    //     a. Set intlObj.[[RoundingType]] to fraction-digits.
    intlObj.roundingType = 'fractionDigits';
    //     b. Set intlObj.[[ComputedRoundingPriority]] to "auto".
    intlObj.computedRoundingPriority = 'auto';
  }
  // 29. If roundingIncrement is not 1, then
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
  // 30. Return unused.
}
