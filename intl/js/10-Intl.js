/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// ECMA-402 8: The Intl Object, following the draft
// https://tc39.es/ecma402/ dated October 22, 2026. ECMA-262 section numbers
// are from the https://tc39.es/ecma262/ draft of the same date. The object is
// created when the bytecode runs, so its [[Prototype]] is the intrinsic
// %Object.prototype%.
var IntlObject = {};

/// The unit identifiers of ECMA-402 "Table 2: Single units sanctioned for
/// use in ECMAScript", sorted by code unit, without duplicates.
var SANCTIONED_UNITS = [
  'acre', 'bit', 'byte', 'celsius', 'centimeter', 'day', 'degree',
  'fahrenheit', 'fluid-ounce', 'foot', 'gallon', 'gigabit', 'gigabyte',
  'gram', 'hectare', 'hour', 'inch', 'kilobit', 'kilobyte', 'kilogram',
  'kilometer', 'liter', 'megabit', 'megabyte', 'meter', 'microsecond',
  'mile', 'mile-scandinavian', 'milliliter', 'millimeter', 'millisecond',
  'minute', 'month', 'nanosecond', 'ounce', 'percent', 'petabyte', 'pound',
  'second', 'stone', 'terabit', 'terabyte', 'week', 'yard', 'year',
];

// 6.6.3 AvailableCanonicalUnits ( )
function AvailableCanonicalUnits() {
  // 1. Return the List, sorted by code unit, of the unique simple unit
  //    identifiers in every row of Table 2, except the header row.
  return SANCTIONED_UNITS;
}

/// Split a space-separated list from native code into a List. Empty items
/// (from leading, trailing or repeated spaces) are skipped.
function splitSpaces(s) {
  // 1. Let list be a new empty List.
  var list = [];
  // 2. Let start be 0.
  var start = 0;
  // 3. Repeat, while start < the length of s,
  while (start < s.length) {
    // a. Let end be StringIndexOf(s, " ", start); if it is not-found, let
    //    end be the length of s.
    var end = stringIndexOf(s, ' ', start);
    if (end === -1) end = s.length;
    // b. If end > start, append the substring of s from start to end to
    //    list.
    if (end > start) listPush(list, stringSlice(s, start, end));
    // c. Set start to end + 1.
    start = end + 1;
  }
  // 4. Return list.
  return list;
}

/// Install Intl on the global object; called once by setup.
function installIntl() {
  // 1. ECMA-402 8.3: the function properties of Intl, with the shape the
  //    design spec "Function shapes" gives static methods.
  defineMethods(IntlObject, {
    // 8.3.1 Intl.getCanonicalLocales ( locales )
    getCanonicalLocales(locales) {
      // 1. Let localeList be ? CanonicalizeLocaleList(locales).
      var localeList = CanonicalizeLocaleList(locales);
      // 2. Return CreateArrayFromList(localeList).
      return CreateArrayFromList(localeList);
    },
    // 8.3.2 Intl.supportedValuesOf ( key )
    supportedValuesOf(key) {
      // 1. Set key to ? ToString(key).
      key = ToString(key);
      var list;
      if (key === 'calendar') {
        // 2. If key is "calendar", then
        //    a. Let list be a new empty List.
        list = [];
        //    b. For each element identifier of AvailableCalendars(), do
        var calendars = splitSpaces(native.supportedValues(key));
        for (var i = 0; i < calendars.length; i++) {
          var identifier = calendars[i];
          //    i. Let canonical be CanonicalizeUValue("ca", identifier).
          var canonical = CanonicalizeUValue('ca', identifier);
          //    ii. If identifier is canonical, then
          //        1. Append identifier to list.
          if (identifier === canonical) listPush(list, identifier);
        }
      } else if (key === 'collation') {
        // 3. Else if key is "collation", then
        //    a. Let list be AvailableCanonicalCollations( ).
        list = splitSpaces(native.supportedValues(key));
      } else if (key === 'currency') {
        // 4. Else if key is "currency", then
        //    a. Let list be AvailableCanonicalCurrencies( ).
        list = splitSpaces(native.supportedValues(key));
      } else if (key === 'numberingSystem') {
        // 5. Else if key is "numberingSystem", then
        //    a. Let list be AvailableCanonicalNumberingSystems( ).
        list = splitSpaces(native.supportedValues(key));
      } else if (key === 'timeZone') {
        // 6. Else if key is "timeZone", then
        //    a. Let list be AvailablePrimaryTimeZoneIdentifiers( ).
        list = splitSpaces(native.supportedValues(key));
      } else if (key === 'unit') {
        // 7. Else if key is "unit", then
        //    a. Let list be AvailableCanonicalUnits( ).
        list = AvailableCanonicalUnits();
      } else {
        // 8. Else,
        //    a. Throw a RangeError exception.
        throw new _RangeError('Invalid key: ' + key);
      }
      // 9. Return CreateArrayFromList(list).
      return CreateArrayFromList(list);
    },
  });
  // 2. ECMA-402 8.2: the constructor properties of Intl.
  installLocale(IntlObject);
  // 3. ECMA-402 8.1.1 Intl [ %Symbol.toStringTag% ]: the String value
  //    "Intl", { [[Writable]]: false, [[Enumerable]]: false,
  //    [[Configurable]]: true }.
  setToStringTag(IntlObject, 'Intl');
  // 4. ECMA-402 8: Intl is the initial value of the "Intl" property of the
  //    global object. ECMA-262 18 gives such data properties the attributes
  //    { [[Writable]]: true, [[Enumerable]]: false, [[Configurable]]: true }.
  ObjectDefineProperty(global, 'Intl', {
    __proto__: null,
    value: IntlObject,
    writable: true,
    enumerable: false,
    configurable: true,
  });
}
