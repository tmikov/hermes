/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// ECMA-402 9: Locale and Parameter Negotiation, following the draft
// https://tc39.es/ecma402/ dated October 24, 2026. Locale tags here are
// canonical (they come from CanonicalizeLocaleList), so Unicode extensions
// are lowercase and their keywords sorted.

/// Whether the [[AvailableLocales]] list (ECMA-402 9.1) contains tag, a
/// canonical tag without a Unicode extension. The native list includes the
/// fallbacks 9.1 requires.
function isAvailableLocale(tag) {
  // 1. Return whether the locale data covers tag.
  return native.isAvailableLocale(tag);
}

// 9.2.1 CanonicalizeLocaleList ( locales )
function CanonicalizeLocaleList(locales) {
  // 1. If locales is undefined, then
  //    a. Return a new empty List.
  if (locales === undefined) return [];
  // 2. Let seen be a new empty List.
  var seen = [];
  var localesObj;
  // 3. If locales is a String or locales is an Object and locales has an
  //    [[InitializedLocale]] internal slot, then
  if (typeof locales === 'string' || isLocaleObject(locales)) {
    // a. Let localesObj be CreateArrayFromList(« locales »).
    localesObj = CreateArrayFromList([locales]);
  } else {
    // 4. Else,
    //    a. Let localesObj be ? ToObject(locales).
    localesObj = ToObject(locales);
  }
  // 5. Let length be ? LengthOfArrayLike(localesObj).
  var length = ToLength(localesObj.length);
  // 6. Let k be 0.
  // 7. Repeat, while k < length,
  for (var k = 0; k < length; k++) {
    // a. Let propertyKey be ! ToString(𝔽(k)).
    var propertyKey = ToString(k);
    // b. Let exists be ? HasProperty(localesObj, propertyKey).
    // c. If exists is true, then
    if (propertyKey in localesObj) {
      // i. Let element be ? Get(localesObj, propertyKey).
      var element = localesObj[propertyKey];
      // ii. If element is neither a String nor an Object, throw a TypeError
      //     exception.
      var type = typeof element;
      if (
        type !== 'string' &&
        (element === null || (type !== 'object' && type !== 'function'))
      ) {
        throw new _TypeError('Language ID should be string or object');
      }
      var tag;
      // iii. If element is an Object and element has an [[InitializedLocale]]
      //      internal slot, then
      if (isLocaleObject(element)) {
        // 1. Let tag be element.[[Locale]].
        tag = localeObjectTag(element);
      } else {
        // iv. Else,
        //     1. Let tag be ? ToString(element).
        tag = ToString(element);
      }
      // v. If IsWellFormedLanguageTag(tag) is false, throw a RangeError
      //    exception.
      // vi. Let canonicalizedTag be CanonicalizeUnicodeLocaleId(tag).
      //     The native canonicalizer does both: undefined means ill-formed.
      var canonicalizedTag = native.canonicalizeLocale(tag);
      if (canonicalizedTag === undefined)
        throw new _RangeError('Incorrect locale information provided');
      // vii. If seen does not contain canonicalizedTag, append
      //      canonicalizedTag to seen.
      if (!listContains(seen, canonicalizedTag))
        listPush(seen, canonicalizedTag);
    }
    // d. Set k to k + 1.
  }
  // 8. Return seen.
  return seen;
}

/// Split the Unicode locale extension sequence (ECMA-402 6.2: "-u-" and the
/// non-singleton subtags after it) off a canonical locale. Returns
/// {base, extension}, extension undefined if there is none. A "-u-" inside a
/// "-x-" private use sequence is not one.
function splitUnicodeExtension(locale) {
  var start = -1;
  var end = locale.length;
  var pos = 0;
  // 1. Walk the subtags; pos is the start of the current one.
  while (pos < locale.length) {
    var dash = stringIndexOf(locale, '-', pos);
    var next = dash === -1 ? locale.length : dash;
    // a. A singleton ends the extension, if one has started.
    if (next - pos === 1) {
      if (start !== -1) {
        end = pos - 1;
        break;
      }
      var c = charCodeAt(locale, pos);
      // b. Nothing after "x" (private use) is an extension.
      if (c === 0x78) break;
      // c. "u" starts the extension, at its preceding "-".
      if (c === 0x75) start = pos - 1;
    }
    pos = next + 1;
  }
  // 2. If there is no extension, the base is the whole locale.
  if (start === -1)
    return {__proto__: null, base: locale, extension: undefined};
  // 3. Else, remove the extension from the locale.
  return {
    __proto__: null,
    base: stringSlice(locale, 0, start) + stringSlice(locale, end),
    extension: stringSlice(locale, start, end),
  };
}

/// The element of keywords whose [[Key]] is key, or undefined.
function findKeyword(keywords, key) {
  // 1. For each Record keyword of keywords, if keyword.[[Key]] is key,
  //    return keyword.
  for (var i = 0; i < keywords.length; i++) {
    if (keywords[i].key === key) return keywords[i];
  }
  // 2. Return undefined.
  return undefined;
}

// 9.2.5 UnicodeExtensionComponents ( extension )
function UnicodeExtensionComponents(extension) {
  // 1. Assert: The ASCII-lowercase of extension is extension.
  // 2. Assert: The substring of extension from 0 to 3 is "-u-".
  // 3. Let attributes be a new empty List.
  var attributes = [];
  // 4. Let keywords be a new empty List.
  var keywords = [];
  // 5. Let keyword be undefined.
  var keyword = undefined;
  // 6. Let size be the length of extension.
  var size = extension.length;
  // 7. Let k be 3.
  var k = 3;
  // 8. Repeat, while k < size,
  while (k < size) {
    // a. Let e be StringIndexOf(extension, "-", k).
    var e = stringIndexOf(extension, '-', k);
    // b. If e is not-found, let length be size - k; else let length be
    //    e - k.
    var length = e === -1 ? size - k : e - k;
    // c. Let subtag be the substring of extension from k to k + length.
    var subtag = stringSlice(extension, k, k + length);
    // d. NOTE: A keyword is a key of length 2 followed by value subtags of
    //    length 3 to 8; an attribute is a subtag of length 3 to 8 that
    //    precedes all keywords.
    // e. Assert: length ≥ 2.
    if (keyword === undefined && length !== 2) {
      // f. If keyword is undefined and length ≠ 2, then
      //    i. If subtag is not an element of attributes, append subtag to
      //       attributes.
      if (!listContains(attributes, subtag)) listPush(attributes, subtag);
    } else if (length === 2) {
      // g. Else if length = 2, then
      //    i. Set keyword to the Record { [[Key]]: subtag, [[Value]]: the
      //       empty String }.
      keyword = {__proto__: null, key: subtag, value: ''};
      //    ii. If keywords does not contain an element whose [[Key]] is
      //        keyword.[[Key]], append keyword to keywords. A repeated key's
      //        record still collects its value below but is never appended.
      if (findKeyword(keywords, keyword.key) === undefined)
        listPush(keywords, keyword);
    } else if (keyword.value === '') {
      // h. Else if keyword.[[Value]] is the empty String, then
      //    i. Set keyword.[[Value]] to subtag.
      keyword.value = subtag;
    } else {
      // i. Else,
      //    i. Set keyword.[[Value]] to the string-concatenation of
      //       keyword.[[Value]], "-", and subtag.
      keyword.value = keyword.value + '-' + subtag;
    }
    // j. Set k to k + length + 1.
    k = k + length + 1;
  }
  // 9. Return the Record { [[Attributes]]: attributes, [[Keywords]]:
  //    keywords }.
  return {__proto__: null, attributes: attributes, keywords: keywords};
}

// 9.2.6 InsertUnicodeExtensionAndCanonicalize ( locale, attributes,
//                                               keywords )
function InsertUnicodeExtensionAndCanonicalize(locale, attributes, keywords) {
  // 1. Assert: locale does not contain a Unicode locale extension sequence.
  // 2. Let extension be "-u".
  var extension = '-u';
  // 3. For each element attr of attributes, do
  for (var i = 0; i < attributes.length; i++) {
    // a. Set extension to the string-concatenation of extension, "-", and
    //    attr.
    extension = extension + '-' + attributes[i];
  }
  // 4. For each Record { [[Key]], [[Value]] } keyword of keywords, do
  for (var j = 0; j < keywords.length; j++) {
    // a. Set extension to the string-concatenation of extension, "-", and
    //    keyword.[[Key]].
    extension = extension + '-' + keywords[j].key;
    // b. If keyword.[[Value]] is not the empty String, set extension to the
    //    string-concatenation of extension, "-", and keyword.[[Value]].
    if (keywords[j].value !== '')
      extension = extension + '-' + keywords[j].value;
  }
  var newLocale;
  if (extension === '-u') {
    // 5. If extension is "-u", return CanonicalizeUnicodeLocaleId(locale).
    newLocale = locale;
  } else {
    // 6. Let privateIndex be StringIndexOf(locale, "-x-", 0).
    var privateIndex = stringIndexOf(locale, '-x-', 0);
    if (privateIndex === -1) {
      // 7. If privateIndex is not-found, then
      //    a. Let newLocale be the string-concatenation of locale and
      //       extension.
      newLocale = locale + extension;
    } else {
      // 8. Else,
      //    a. Let preExtension be the substring of locale from 0 to
      //       privateIndex.
      //    b. Let postExtension be the substring of locale from privateIndex.
      //    c. Let newLocale be the string-concatenation of preExtension,
      //       extension, and postExtension.
      newLocale =
        stringSlice(locale, 0, privateIndex) +
        extension +
        stringSlice(locale, privateIndex);
    }
  }
  // 9. Assert: IsWellFormedLanguageTag(newLocale) is true.
  // 10. Return CanonicalizeUnicodeLocaleId(newLocale).
  var canonical = native.canonicalizeLocale(newLocale);
  if (canonical === undefined)
    throw new _TypeError('Intl internal error: ill-formed locale ' + newLocale);
  return canonical;
}

// 9.2.2 CanonicalizeUValue ( ukey, uvalue )
function CanonicalizeUValue(ukey, uvalue) {
  // 1. Let lowerValue be the ASCII-lowercase of uvalue.
  var lowerValue = asciiLowercase(uvalue);
  // 2. Let canonicalized be the String value resulting from canonicalizing
  //    lowerValue as a value of key ukey per UTS 35 Annex C. The native
  //    canonicalizer works on whole tags, so wrap the value in one and read
  //    it back. A value that cannot form a tag, or one whose canonical tag
  //    has no Unicode extension left, is returned lowercased.
  var tag = native.canonicalizeLocale('und-u-' + ukey + '-' + lowerValue);
  if (tag === undefined) return lowerValue;
  var extension = splitUnicodeExtension(tag).extension;
  if (extension === undefined) return lowerValue;
  var keyword = findKeyword(
    UnicodeExtensionComponents(extension).keywords,
    ukey,
  );
  // 3. NOTE: It is recommended that implementations use the 'u' extension
  //    data in common/bcp47 provided by CLDR.
  // 4. Return canonicalized.
  return keyword === undefined ? lowerValue : keyword.value;
}

// 9.2.3 LookupMatchingLocaleByPrefix ( availableLocales, requestedLocales )
// availableLocales is given as a membership predicate.
function LookupMatchingLocaleByPrefix(isAvailable, requestedLocales) {
  // 1. For each element locale of requestedLocales, do
  for (var i = 0; i < requestedLocales.length; i++) {
    // a. Let extension be empty.
    // b. If locale contains a Unicode locale extension sequence, then
    //    i. Set extension to the Unicode locale extension sequence of
    //       locale.
    //    ii. Set locale to the String value that is locale with any Unicode
    //        locale extension sequences removed.
    var parts = splitUnicodeExtension(requestedLocales[i]);
    // c. Let prefix be locale.
    var prefix = parts.base;
    // d. Repeat, while prefix is not the empty String,
    while (prefix !== '') {
      // i. If availableLocales contains prefix, return the Record
      //    { [[locale]]: prefix, [[extension]]: extension }.
      if (isAvailable(prefix))
        return {__proto__: null, locale: prefix, extension: parts.extension};
      // ii. If prefix contains "-" (code unit 0x002D HYPHEN-MINUS), let pos
      //     be the index into prefix of the last occurrence of "-"; else
      //     let pos be 0.
      var pos = 0;
      for (var p = prefix.length - 1; p >= 0; p--) {
        if (charCodeAt(prefix, p) === 0x2d) {
          pos = p;
          break;
        }
      }
      // iii. Repeat, while pos ≥ 2 and the substring of prefix from pos - 2
      //      to pos - 1 is "-",
      //      1. Set pos to pos - 2.
      while (pos >= 2 && charCodeAt(prefix, pos - 2) === 0x2d) pos = pos - 2;
      // iv. Set prefix to the substring of prefix from 0 to pos.
      prefix = stringSlice(prefix, 0, pos);
    }
  }
  // 2. Return undefined.
  return undefined;
}

// 6.2.3 DefaultLocale ( ): implementation-defined. It must be an available
// locale with no Unicode extensions. The loader's resolve_defaults
// (intl/jsi/Defaults.cpp) already reduces hostDefaults.locales to such tags,
// most preferred first, and never leaves the list empty (it falls back to
// "en-US"), so the default is its first element.
function DefaultLocale() {
  // 1. Return the first of the host's default locales.
  return hostDefaults.locales[0];
}

// 9.2.7 ResolveLocale ( availableLocales, requestedLocales, options,
//                       relevantExtensionKeys [ , localeData ] )
// availableLocales is a predicate. localeData is undefined (not present) or
// a function: localeData(locale) is the spec's localeData.[[<locale>]], a
// record with a list per relevant key, default first. options is a
// null-prototype record. "best fit" behaves as "lookup", which the spec
// permits (design spec "Locale matching").
function ResolveLocale(
  isAvailable,
  requestedLocales,
  options,
  relevantExtensionKeys,
  localeData,
) {
  // 1. Let matcher be options.[[localeMatcher]].
  // 2. If matcher is "lookup", then
  //    a. Let match be LookupMatchingLocaleByPrefix(availableLocales,
  //       requestedLocales).
  // 3. Else,
  //    a. Let match be LookupMatchingLocaleByBestFit(availableLocales,
  //       requestedLocales), which is LookupMatchingLocaleByPrefix here.
  var match = LookupMatchingLocaleByPrefix(isAvailable, requestedLocales);
  // 4. If match is undefined, set match to the Record { [[locale]]:
  //    DefaultLocale(), [[extension]]: empty }.
  if (match === undefined)
    match = {__proto__: null, locale: DefaultLocale(), extension: undefined};
  // 5. Let foundLocale be match.[[locale]].
  var foundLocale = match.locale;
  // 6. Let result be a new Record.
  var result = {__proto__: null};
  var foundLocaleData;
  if (localeData === undefined) {
    // 7. If localeData is not present, then
    //    a. Assert: relevantExtensionKeys is an empty List.
    //    b. Set result.[[LocaleData]] to undefined.
    result.localeData = undefined;
  } else {
    // 8. Else,
    //    a. Let foundLocaleData be localeData.[[<foundLocale>]].
    foundLocaleData = localeData(foundLocale);
    //    b. Assert: foundLocaleData is a Record.
    //    c. Set result.[[LocaleData]] to foundLocaleData.
    result.localeData = foundLocaleData;
  }
  var keywords;
  if (match.extension !== undefined) {
    // 9. If match.[[extension]] is not empty, then
    //    a. Let components be UnicodeExtensionComponents(match.[[extension]]).
    //    b. Let keywords be components.[[Keywords]].
    keywords = UnicodeExtensionComponents(match.extension).keywords;
  } else {
    // 10. Else,
    //     a. Let keywords be a new empty List.
    keywords = [];
  }
  // 11. Let supportedKeywords be a new empty List.
  var supportedKeywords = [];
  // 12. For each element key of relevantExtensionKeys, do
  for (var i = 0; i < relevantExtensionKeys.length; i++) {
    var key = relevantExtensionKeys[i];
    // a. Let keyLocaleData be foundLocaleData.[[<key>]].
    var keyLocaleData = foundLocaleData[key];
    // b. Assert: keyLocaleData is a List.
    // c. Let value be keyLocaleData[0].
    var value = keyLocaleData[0];
    // d. Assert: value is a String or value is null.
    // e. Let supportedKeyword be empty.
    var supportedKeyword = undefined;
    // f. If keywords contains an element whose [[Key]] is key, then
    //    i. Let entry be the element of keywords whose [[Key]] is key.
    var entry = findKeyword(keywords, key);
    if (entry !== undefined) {
      // ii. Let requestedValue be entry.[[Value]].
      var requestedValue = entry.value;
      if (requestedValue !== '') {
        // iii. If requestedValue is not the empty String, then
        //      1. If keyLocaleData contains requestedValue, then
        if (listContains(keyLocaleData, requestedValue)) {
          // a. Set value to requestedValue.
          value = requestedValue;
          // b. Set supportedKeyword to the Record { [[Key]]: key,
          //    [[Value]]: value }.
          supportedKeyword = {__proto__: null, key: key, value: value};
        }
      } else if (listContains(keyLocaleData, 'true')) {
        // iv. Else if keyLocaleData contains "true", then
        //     1. Set value to "true".
        value = 'true';
        //     2. Set supportedKeyword to the Record { [[Key]]: key,
        //        [[Value]]: the empty String }.
        supportedKeyword = {__proto__: null, key: key, value: ''};
      }
    }
    // g. Assert: options has a field [[<key>]].
    // h. Let optionsValue be options.[[<key>]].
    var optionsValue = options[key];
    // i. Assert: optionsValue is a String, or optionsValue is either
    //    undefined or null.
    // j. If optionsValue is a String, then
    if (typeof optionsValue === 'string') {
      // i. Let ukey be the ASCII-lowercase of key.
      // ii. Set optionsValue to CanonicalizeUValue(ukey, optionsValue).
      optionsValue = CanonicalizeUValue(asciiLowercase(key), optionsValue);
      // iii. If optionsValue is the empty String, then
      //      1. Set optionsValue to "true".
      if (optionsValue === '') optionsValue = 'true';
    }
    // k. If SameValue(optionsValue, value) is false and keyLocaleData
    //    contains optionsValue, then
    if (optionsValue !== value && listContains(keyLocaleData, optionsValue)) {
      // i. Set value to optionsValue.
      value = optionsValue;
      // ii. Set supportedKeyword to empty.
      supportedKeyword = undefined;
    }
    // l. If supportedKeyword is not empty, append supportedKeyword to
    //    supportedKeywords.
    if (supportedKeyword !== undefined)
      listPush(supportedKeywords, supportedKeyword);
    // m. Set result.[[<key>]] to value.
    result[key] = value;
  }
  // 13. If supportedKeywords is not empty, then
  if (supportedKeywords.length > 0) {
    // a. Let supportedAttributes be a new empty List.
    // b. Set foundLocale to InsertUnicodeExtensionAndCanonicalize(
    //    foundLocale, supportedAttributes, supportedKeywords).
    foundLocale = InsertUnicodeExtensionAndCanonicalize(
      foundLocale,
      [],
      supportedKeywords,
    );
  }
  // 14. Set result.[[Locale]] to foundLocale.
  result.locale = foundLocale;
  // 15. Return result.
  return result;
}

// 9.2.9 FilterLocales ( availableLocales, requestedLocales, options )
// availableLocales is given as a membership predicate.
function FilterLocales(isAvailable, requestedLocales, options) {
  // 1. Set options to ? CoerceOptionsToObject(options).
  options = CoerceOptionsToObject(options);
  // 2. Let matcher be ? GetOption(options, "localeMatcher", string,
  //    « "lookup", "best fit" », "best fit").
  GetOption(
    options,
    'localeMatcher',
    'string',
    ['lookup', 'best fit'],
    'best fit',
  );
  // 3. Let subset be a new empty List.
  var subset = [];
  // 4. For each element locale of requestedLocales, do
  for (var i = 0; i < requestedLocales.length; i++) {
    // a. If matcher is "lookup", then
    //    i. Let match be LookupMatchingLocaleByPrefix(availableLocales,
    //       « locale »).
    // b. Else,
    //    i. Let match be LookupMatchingLocaleByBestFit(availableLocales,
    //       « locale »), which is LookupMatchingLocaleByPrefix here.
    var match = LookupMatchingLocaleByPrefix(isAvailable, [
      requestedLocales[i],
    ]);
    // c. If match is not undefined, append locale to subset.
    if (match !== undefined) listPush(subset, requestedLocales[i]);
  }
  // 5. Return CreateArrayFromList(subset).
  return CreateArrayFromList(subset);
}
