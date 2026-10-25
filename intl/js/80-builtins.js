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
}
