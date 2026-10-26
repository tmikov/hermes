/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// ECMA-402 "Locale Sensitive Functions of the ECMAScript Language
// Specification": the versions of the locale-sensitive built-ins that
// replace ECMA-262's, following the draft https://tc39.es/ecma402/ dated
// October 24, 2026. setup installs them (design spec "What setup does");
// later phases add theirs here.

/// The collator localeCompare uses when locales and options are both
/// undefined, created on first use (design spec "Resistance to tampering").
var defaultCollator;

/// The locale TransformCase maps with when locales is undefined, found on
/// first use. It depends only on DefaultLocale(), which is fixed for the
/// runtime's lifetime (design spec "Non-goals").
var defaultCaseMappingLocale;

/// Whether tag is in the available locales of TransformCase: the languages
/// whose upper and lower case mappings ICU4X's CaseMapper tailors (design
/// spec, "Data", Case mapping). nl is not one: it tailors only titlecase.
function isCaseMappingLocale(tag) {
  // 1. Return whether tag is one of « "az", "el", "hy", "lt", "tr" ».
  return (
    tag === 'az' || tag === 'el' || tag === 'hy' || tag === 'lt' || tag === 'tr'
  );
}

// TransformCase ( string, locales, targetCase ), with targetCase 'lower' or
// 'upper'.
function TransformCase(string, locales, targetCase) {
  var locale;
  if (locales === undefined && defaultCaseMappingLocale !== undefined) {
    //    Steps 1 to 6 have no observable effect when locales is undefined,
    //    and their result depends only on DefaultLocale(), so it is reused.
    locale = defaultCaseMappingLocale;
  } else {
    // 1. Let requestedLocales be ? CanonicalizeLocaleList(locales).
    var requestedLocales = CanonicalizeLocaleList(locales);
    var requestedLocale;
    if (requestedLocales.length !== 0) {
      // 2. If requestedLocales is not an empty List, then
      //    a. Let requestedLocale be requestedLocales[0].
      requestedLocale = requestedLocales[0];
    } else {
      // 3. Else,
      //    a. Let requestedLocale be DefaultLocale().
      requestedLocale = DefaultLocale();
    }
    // 4. Let availableLocales be an Available Locales List which includes
    //    the language tags for which the Unicode Character Database
    //    contains language-sensitive case mappings. If the implementation
    //    supports additional locale-sensitive case mappings,
    //    availableLocales should also include their corresponding language
    //    tags.
    //    These are the tags isCaseMappingLocale accepts.
    // 5. Let match be LookupMatchingLocaleByPrefix(availableLocales,
    //    « requestedLocale »).
    var match = LookupMatchingLocaleByPrefix(isCaseMappingLocale, [
      requestedLocale,
    ]);
    // 6. If match is not undefined, let locale be match.[[locale]]; else
    //    let locale be "und".
    locale = match !== undefined ? match.locale : 'und';
    //    Keep the result of steps 1 to 6 for undefined locales.
    if (locales === undefined) defaultCaseMappingLocale = locale;
  }
  // 7. Let codePoints be StringToCodePoints(string).
  // 8. If targetCase is lower, then
  //    a. Let newCodePoints be a List whose elements are the result of a
  //       lowercase transformation of codePoints according to an
  //       implementation-derived algorithm using locale or the Unicode
  //       Default Case Conversion algorithm.
  // 9. Else,
  //    a. Assert: targetCase is upper.
  //    b. Let newCodePoints be a List whose elements are the result of an
  //       uppercase transformation of codePoints according to an
  //       implementation-derived algorithm using locale or the Unicode
  //       Default Case Conversion algorithm.
  // 10. Return CodePointsToString(newCodePoints).
  //     ICU4X's CaseMapper with the tailoring of locale does steps 7 to 10.
  //     It maps each well-formed run of string and copies each unpaired
  //     surrogate through unchanged (design spec, "Strings").
  return native.caseMap(string, locale, targetCase === 'upper');
}

/// The NumberFormat that Number.prototype.toLocaleString and
/// BigInt.prototype.toLocaleString use when locales and options are both
/// undefined, created on first use (design spec "Resistance to tampering").
var defaultNumberFormat;

/// Construct(%Intl.NumberFormat%, « locales, options ») for the Number and
/// BigInt toLocaleString overrides. CreateNumberFormat performs every
/// observable step of the constructor without going through
/// %Intl.NumberFormat% or any public property. When locales and options are
/// both undefined, those steps have no observable effect and always give
/// the same NumberFormat, so one default NumberFormat is created on first
/// use and reused without them.
function toLocaleStringNumberFormat(locales, options) {
  // 1. If locales and options are both undefined, return the default
  //    NumberFormat, creating it first if there is none.
  if (locales === undefined && options === undefined) {
    if (defaultNumberFormat === undefined)
      defaultNumberFormat = CreateNumberFormat(undefined, undefined);
    return defaultNumberFormat;
  }
  // 2. Return CreateNumberFormat(locales, options).
  return CreateNumberFormat(locales, options);
}

// ECMA-262 21.1.3.7.1 ThisNumberValue ( value )
function ThisNumberValue(value) {
  // 1. If value is a Number, return value.
  // 2. If value is an Object and value has a [[NumberData]] internal slot,
  //    then
  //    a. Let n be value.[[NumberData]].
  //    b. Assert: n is a Number.
  //    c. Return n.
  // 3. Throw a TypeError exception.
  //    The captured Number.prototype.valueOf is exactly these steps
  //    (ECMA-262 21.1.3.7: "Return ? ThisNumberValue(this value)").
  return ReflectApply(NumberPrototypeValueOf, value, []);
}

// ECMA-262 21.2.3.4.1 ThisBigIntValue ( value )
function ThisBigIntValue(value) {
  // 1. If value is a BigInt, return value.
  // 2. If value is an Object and value has a [[BigIntData]] internal slot,
  //    then
  //    a. Assert: value.[[BigIntData]] is a BigInt.
  //    b. Return value.[[BigIntData]].
  // 3. Throw a TypeError exception.
  //    The captured BigInt.prototype.valueOf is exactly these steps
  //    (ECMA-262 21.2.3.4: "Return ? ThisBigIntValue(this value)").
  return ReflectApply(BigIntPrototypeValueOf, value, []);
}

/// The DateTimeFormats that Date.prototype.toLocaleString,
/// toLocaleDateString and toLocaleTimeString use when locales and options
/// are both undefined, keyed by the required argument each passes to
/// CreateDateTimeFormat ("any", "date", "time"), each created on first use
/// (design spec "Resistance to tampering").
var defaultDateTimeFormats = {
  __proto__: null,
  any: undefined,
  date: undefined,
  time: undefined,
};

/// CreateDateTimeFormat(%Intl.DateTimeFormat%, locales, options, required,
/// defaults) for the Date overrides. CreateDateTimeFormat performs every
/// observable step without going through %Intl.DateTimeFormat% or any
/// public property. When locales and options are both undefined, those
/// steps have no observable effect and always give the same DateTimeFormat
/// for the same required and defaults (the default locale and time zone
/// are fixed for the runtime's lifetime), so each override's default
/// DateTimeFormat is created on first use and reused without them. Each
/// override passes one required with one defaults, so required alone keys
/// the default.
function toLocaleDateTimeFormat(locales, options, required, defaults) {
  // 1. If locales and options are both undefined, return the default
  //    DateTimeFormat for required, creating it first if there is none.
  if (locales === undefined && options === undefined) {
    var dateTimeFormat = defaultDateTimeFormats[required];
    if (dateTimeFormat === undefined) {
      dateTimeFormat = CreateDateTimeFormat(
        undefined,
        undefined,
        required,
        defaults,
      );
      defaultDateTimeFormats[required] = dateTimeFormat;
    }
    return dateTimeFormat;
  }
  // 2. Return ? CreateDateTimeFormat(locales, options, required, defaults).
  return CreateDateTimeFormat(locales, options, required, defaults);
}

/// Steps 1 to 3 of the Date overrides: the [[DateValue]] of the this value
/// dateObj, or a TypeError naming method, the override called, if it has
/// none.
function thisDateValue(dateObj, method) {
  // 1. Let dateObj be the this value. (The caller passes it.)
  // 2. Perform ? RequireInternalSlot(dateObj, [[DateValue]]).
  // 3. Let x be dateObj.[[DateValue]].
  //    The captured Date.prototype.getTime is exactly these steps
  //    (ECMA-262 21.4.4.10: 1. Let dateObj be the this value. 2. Perform ?
  //    RequireInternalSlot(dateObj, [[DateValue]]). 3. Return
  //    dateObj.[[DateValue]].), so it throws a TypeError for an object
  //    without the slot, such as Date.prototype or a proxy of a Date, and
  //    nothing else: it runs no user code. Its TypeError names getTime, so
  //    it is replaced by one that names method.
  try {
    return ReflectApply(DatePrototypeGetTime, dateObj, []);
  } catch (e) {
    throw new _TypeError(
      'Date.prototype.' + method + ' called on non-Date object',
    );
  }
}

// ECMA-262 7.3.18 LengthOfArrayLike ( obj )
function LengthOfArrayLike(obj) {
  // 1. Return ℝ(? ToLength(? Get(obj, "length"))).
  return ToLength(obj.length);
}

// ECMA-262 7.3.20 Invoke ( V, P [ , argumentsList ] ), with argumentsList
// present.
function Invoke(V, P, argumentsList) {
  // 1. If argumentsList is not present, set argumentsList to a new empty
  //    List. Every caller passes it.
  // 2. Let func be ? GetV(V, P).
  //    A property access is GetV (7.3.3): ToObject(V).[[Get]](P, V), with V
  //    itself as the receiver, and the TypeError for undefined and null.
  var func = V[P];
  // 3. Return ? Call(func, V, argumentsList).
  //    Call (7.3.13): 1. If IsCallable(F) is false, throw a TypeError
  //    exception. 2. Return ? F.[[Call]](V, argumentsList).
  if (typeof func !== 'function')
    throw new _TypeError(ToString(P) + ' is not a function');
  return ReflectApply(func, V, argumentsList);
}

// ECMA-262 23.1.3.18.1 JoinIndexedProperties ( obj, length, separator [ ,
// methodName [ , methodArgs ] ] ), with methodName and methodArgs present:
// the only form ECMA-402 uses.
function JoinIndexedProperties(obj, length, separator, methodName, methodArgs) {
  // 1. If methodName is present and methodArgs is not present, set
  //    methodArgs to a new empty List. Every caller passes methodArgs.
  // 2. Let result be the empty String.
  var result = '';
  // 3. Let k be 0.
  // 4. Repeat, while k < length,
  for (var k = 0; k < length; k++) {
    // a. If k > 0, set result to the string-concatenation of result and
    //    separator.
    if (k > 0) result += separator;
    // b. NOTE: If obj is a TypedArray, the following invocation of Get will
    //    return a normal completion.
    // c. Let element be ? Get(obj, ! ToString(𝔽(k))).
    //    A Number key is converted to that String.
    var element = obj[k];
    // d. If element is neither undefined nor null, then
    if (element !== undefined && element !== null) {
      // i. If methodName is present, then
      //    1. Let elementString be ? ToString(? Invoke(element, methodName,
      //       methodArgs)).
      // ii. (Else: methodName is always present.)
      var elementString = ToString(Invoke(element, methodName, methodArgs));
      // iii. Set result to the string-concatenation of result and
      //      elementString.
      result += elementString;
    }
    // e. Set k to k + 1.
  }
  // 5. Return result.
  return result;
}

/// The objects that calls of the Array.prototype.toLocaleString override
/// are joining, outermost first: the cycle check Hermes's own method has
/// (see the override). Compared by identity.
var arraysBeingJoined = [];

// ECMA-262 23.1.3.32.1 LocalizedListSeparator ( )
function LocalizedListSeparator() {
  // 1. Return an implementation-defined String appropriate for use as a
  //    list separator in the host environment's current locale.
  //    The plugin uses ",", as Hermes's own Array.prototype.toLocaleString
  //    does.
  return ',';
}

/// Replace the locale-sensitive built-ins with their ECMA-402 versions;
/// step 3 of setup.
function installBuiltinOverrides() {
  // 1. "Properties of the String Prototype Object": the String.prototype
  //    methods.
  defineMethods(StringPrototype, {
    // String.prototype.localeCompare ( that [ , locales [ , options ] ] )
    localeCompare(that, locales = undefined, options = undefined) {
      // 1. Let thisValue be ? RequireObjectCoercible(this value).
      //    ECMA-262's RequireObjectCoercible now returns unused, so
      //    thisValue is the this value.
      RequireObjectCoercible(this, 'String.prototype.localeCompare');
      var thisValue = this;
      // 2. Let string be ? ToString(thisValue).
      var string = ToString(thisValue);
      // 3. Let thatValue be ? ToString(that).
      var thatValue = ToString(that);
      // 4. Let collator be ? Construct(%Intl.Collator%, « locales, options »).
      //    internalCollator performs every observable step of the
      //    constructor without going through %Intl.Collator% or any public
      //    property, and shares one collator among the calls that read
      //    the same values (design spec "Resistance to tampering"). When
      //    locales and options are both undefined, those steps have no
      //    observable effect and always give the same collator, so one
      //    default collator is found on first use and reused without them.
      var collator;
      if (locales === undefined && options === undefined) {
        if (defaultCollator === undefined)
          defaultCollator = internalCollator(undefined, undefined);
        collator = defaultCollator;
      } else {
        collator = internalCollator(locales, options);
      }
      // 5. Return CompareStrings(collator, string, thatValue).
      return CompareStrings(collator, string, thatValue);
    },

    // String.prototype.toLocaleLowerCase ( [ locales ] )
    toLocaleLowerCase(locales = undefined) {
      // 1. Let thisValue be ? RequireObjectCoercible(this value).
      RequireObjectCoercible(this, 'String.prototype.toLocaleLowerCase');
      var thisValue = this;
      // 2. Let string be ? ToString(thisValue).
      var string = ToString(thisValue);
      // 3. Return ? TransformCase(string, locales, lower).
      return TransformCase(string, locales, 'lower');
    },

    // String.prototype.toLocaleUpperCase ( [ locales ] )
    toLocaleUpperCase(locales = undefined) {
      // 1. Let thisValue be ? RequireObjectCoercible(this value).
      RequireObjectCoercible(this, 'String.prototype.toLocaleUpperCase');
      var thisValue = this;
      // 2. Let string be ? ToString(thisValue).
      var string = ToString(thisValue);
      // 3. Return ? TransformCase(string, locales, upper).
      return TransformCase(string, locales, 'upper');
    },
  });

  // 2. "Properties of the Number Prototype Object".
  defineMethods(NumberPrototype, {
    // Number.prototype.toLocaleString ( [ locales [ , options ] ] )
    toLocaleString(locales = undefined, options = undefined) {
      // 1. Let x be ? ThisNumberValue(this value).
      var x = ThisNumberValue(this);
      // 2. Let numberFormat be ? Construct(%Intl.NumberFormat%, « locales,
      //    options »).
      var numberFormat = toLocaleStringNumberFormat(locales, options);
      // 3. Return FormatNumeric(numberFormat, ! ToIntlMathematicalValue(x)).
      //    x is a Number, so ToIntlMathematicalValue is its steps 4.b to
      //    10, which IntlMVFromNumber performs.
      return FormatNumeric(numberFormat, IntlMVFromNumber(x));
    },
  });

  // 3. "Properties of the BigInt Prototype Object".
  defineMethods(BigIntPrototype, {
    // BigInt.prototype.toLocaleString ( [ locales [ , options ] ] )
    toLocaleString(locales = undefined, options = undefined) {
      // 1. Let x be ? ThisBigIntValue(this value).
      var x = ThisBigIntValue(this);
      // 2. Let numberFormat be ? Construct(%Intl.NumberFormat%, « locales,
      //    options »).
      var numberFormat = toLocaleStringNumberFormat(locales, options);
      // 3. Return FormatNumeric(numberFormat, ℝ(x)).
      return FormatNumeric(numberFormat, IntlMVFromBigInt(x));
    },
  });

  // 4. "Properties of the Date Prototype Object".
  defineMethods(DatePrototype, {
    // Date.prototype.toLocaleString ( [ locales [ , options ] ] )
    toLocaleString(locales = undefined, options = undefined) {
      // 1. Let dateObj be the this value.
      // 2. Perform ? RequireInternalSlot(dateObj, [[DateValue]]).
      // 3. Let x be dateObj.[[DateValue]].
      var x = thisDateValue(this, 'toLocaleString');
      // 4. If x is NaN, return "Invalid Date".
      if (x !== x) return 'Invalid Date';
      // 5. Let dateFormat be ? CreateDateTimeFormat(%Intl.DateTimeFormat%,
      //    locales, options, any, all).
      var dateFormat = toLocaleDateTimeFormat(locales, options, 'any', 'all');
      // 6. Return ! FormatDateTime(dateFormat, x).
      return FormatDateTime(dateFormat, x);
    },

    // Date.prototype.toLocaleDateString ( [ locales [ , options ] ] )
    toLocaleDateString(locales = undefined, options = undefined) {
      // 1. Let dateObj be the this value.
      // 2. Perform ? RequireInternalSlot(dateObj, [[DateValue]]).
      // 3. Let x be dateObj.[[DateValue]].
      var x = thisDateValue(this, 'toLocaleDateString');
      // 4. If x is NaN, return "Invalid Date".
      if (x !== x) return 'Invalid Date';
      // 5. Let dateFormat be ? CreateDateTimeFormat(%Intl.DateTimeFormat%,
      //    locales, options, date, date).
      var dateFormat = toLocaleDateTimeFormat(locales, options, 'date', 'date');
      // 6. Return ! FormatDateTime(dateFormat, x).
      return FormatDateTime(dateFormat, x);
    },

    // Date.prototype.toLocaleTimeString ( [ locales [ , options ] ] )
    toLocaleTimeString(locales = undefined, options = undefined) {
      // 1. Let dateObj be the this value.
      // 2. Perform ? RequireInternalSlot(dateObj, [[DateValue]]).
      // 3. Let x be dateObj.[[DateValue]].
      var x = thisDateValue(this, 'toLocaleTimeString');
      // 4. If x is NaN, return "Invalid Date".
      if (x !== x) return 'Invalid Date';
      // 5. Let timeFormat be ? CreateDateTimeFormat(%Intl.DateTimeFormat%,
      //    locales, options, time, time).
      var timeFormat = toLocaleDateTimeFormat(locales, options, 'time', 'time');
      // 6. Return ! FormatDateTime(timeFormat, x).
      return FormatDateTime(timeFormat, x);
    },
  });

  // 5. "Properties of the Array Prototype Object".
  defineMethods(ArrayPrototype, {
    // Array.prototype.toLocaleString ( [ locales [ , options ] ] )
    toLocaleString(locales = undefined, options = undefined) {
      // 1. Let array be ? ToObject(this value).
      var array = ToObject(this);
      //    Not in the spec: an object this method is already joining gives
      //    the empty string, as in Hermes's own Array.prototype.toLocaleString.
      //    The spec's steps would recurse on a cyclic array until the stack
      //    overflows. The check covers only the objects this method is
      //    joining. Hermes's built-in shares one set with join, which the
      //    plugin cannot reach, so a cycle through both methods is cut one
      //    level later: with a = [1] and a.push({toString() { return
      //    a.toLocaleString(); }}), a.join() is "1,1," here and "1," in
      //    Hermes and V8. The recursion still ends.
      if (listContains(arraysBeingJoined, array)) return '';
      // 2. Let length be ? LengthOfArrayLike(array).
      //    A length above 2^32 - 1 is joined like any other, where Hermes's
      //    own method throws a RangeError before reading an element. The
      //    loop still ends early: each element adds at least the separator
      //    to the result, so Hermes's string length limit throws a
      //    RangeError within 2^28 elements.
      var length = LengthOfArrayLike(array);
      //    Mark array as being joined until JoinIndexedProperties returns
      //    or throws, so a throw cannot leave it marked.
      var depth = arraysBeingJoined.length;
      listPush(arraysBeingJoined, array);
      try {
        // 3. Return ? JoinIndexedProperties(array, length,
        //    LocalizedListSeparator(), "toLocaleString",
        //    « locales, options »).
        return JoinIndexedProperties(
          array,
          length,
          LocalizedListSeparator(),
          'toLocaleString',
          [locales, options],
        );
      } finally {
        // Unmark it. arraysBeingJoined is the plugin's own array, so
        // setting its length consults nothing on Array.prototype.
        arraysBeingJoined.length = depth;
      }
    },
  });

  // 6. "Properties of the %TypedArray% Prototype Object".
  defineMethods(TypedArrayPrototype, {
    // %TypedArray%.prototype.toLocaleString ( [ locales [ , options ] ] )
    toLocaleString(locales = undefined, options = undefined) {
      // 1. Let obj be the this value.
      var obj = this;
      // 2. Let taRecord be ? ValidateTypedArray(obj, seq-cst).
      //    The captured %TypedArray%.prototype.at performs ValidateTypedArray
      //    (obj, seq-cst) first (ECMA-262 23.2.3.1 step 2); with index 0 the
      //    rest of it has no observable effect.
      ReflectApply(TypedArrayPrototypeAt, obj, [0]);
      // 3. Let length be TypedArrayLength(taRecord).
      //    obj is valid and in bounds, so the captured length getter
      //    returns TypedArrayLength (ECMA-262 23.2.3.21 step 6), never a
      //    "length" property of obj.
      var length = ReflectApply(TypedArrayPrototypeGetLength, obj, []);
      // 4. Return ? JoinIndexedProperties(obj, length,
      //    LocalizedListSeparator(), "toLocaleString", « locales, options »).
      return JoinIndexedProperties(
        obj,
        length,
        LocalizedListSeparator(),
        'toLocaleString',
        [locales, options],
      );
    },
  });
}
