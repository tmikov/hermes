/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The Unicode locale grammar (UTS 35 Part 1, section 3.2) and the ECMA-402
// "Abstract Operations for Locale Objects" (GetLocaleBaseName through
// WeekdayUValueToNumber), following the draft
// https://tc39.es/ecma402/ dated October 24, 2026. The grammar checks match
// ASCII letters and digits only, in either case; a non-ASCII code unit
// never matches.

/// Whether c, a code unit, is an ASCII letter.
function isAsciiAlpha(c) {
  // 1. Return whether c is in A-Z or a-z.
  return (c >= 0x41 && c <= 0x5a) || (c >= 0x61 && c <= 0x7a);
}

/// Whether c, a code unit, is an ASCII digit.
function isAsciiDigit(c) {
  // 1. Return whether c is in 0-9.
  return c >= 0x30 && c <= 0x39;
}

/// Whether c, a code unit, is an ASCII letter or digit.
function isAsciiAlnum(c) {
  // 1. Return whether c is an ASCII letter or an ASCII digit.
  return isAsciiAlpha(c) || isAsciiDigit(c);
}

/// Whether the code units of s from start to end are all ASCII letters.
function isAlphaRange(s, start, end) {
  // 1. For each index from start to end, if the code unit is not a letter,
  //    return false.
  for (var i = start; i < end; i++) {
    if (!isAsciiAlpha(charCodeAt(s, i))) return false;
  }
  // 2. Return true.
  return true;
}

/// Whether the code units of s from start to end are all ASCII digits.
function isDigitRange(s, start, end) {
  // 1. For each index from start to end, if the code unit is not a digit,
  //    return false.
  for (var i = start; i < end; i++) {
    if (!isAsciiDigit(charCodeAt(s, i))) return false;
  }
  // 2. Return true.
  return true;
}

/// Whether the code units of s from start to end are all ASCII letters or
/// digits.
function isAlnumRange(s, start, end) {
  // 1. For each index from start to end, if the code unit is neither a
  //    letter nor a digit, return false.
  for (var i = start; i < end; i++) {
    if (!isAsciiAlnum(charCodeAt(s, i))) return false;
  }
  // 2. Return true.
  return true;
}

/// Whether s is matched by unicode_language_subtag (UTS 35 3.2):
/// alpha{2,3} or alpha{5,8}.
function isUnicodeLanguageSubtag(s) {
  // 1. Let length be the length of s.
  var length = s.length;
  // 2. If length is not in 2 to 3 or 5 to 8, return false.
  if (length < 2 || length === 4 || length > 8) return false;
  // 3. Return whether every code unit of s is a letter.
  return isAlphaRange(s, 0, length);
}

/// Whether s is matched by unicode_script_subtag (UTS 35 3.2): alpha{4}.
function isUnicodeScriptSubtag(s) {
  // 1. Return whether s has 4 code units, all letters.
  return s.length === 4 && isAlphaRange(s, 0, 4);
}

/// Whether s is matched by unicode_region_subtag (UTS 35 3.2): alpha{2} or
/// digit{3}.
function isUnicodeRegionSubtag(s) {
  // 1. If s has 2 code units, return whether both are letters.
  if (s.length === 2) return isAlphaRange(s, 0, 2);
  // 2. Return whether s has 3 code units, all digits.
  return s.length === 3 && isDigitRange(s, 0, 3);
}

/// Whether s is matched by unicode_variant_subtag (UTS 35 3.2): alphanum{5,8}
/// or a digit followed by alphanum{3}.
function isUnicodeVariantSubtag(s) {
  // 1. Let length be the length of s.
  var length = s.length;
  // 2. If length is in 5 to 8, return whether every code unit is a letter or
  //    digit.
  if (length >= 5 && length <= 8) return isAlnumRange(s, 0, length);
  // 3. If length is 4, return whether the first code unit is a digit and the
  //    rest are letters or digits.
  if (length === 4)
    return isAsciiDigit(charCodeAt(s, 0)) && isAlnumRange(s, 1, 4);
  // 4. Return false.
  return false;
}

/// Whether s is matched by type (UTS 35 3.2): alphanum{3,8} subtags
/// separated by "-". Linear in the length of s.
function isUnicodeType(s) {
  // 1. Let start be 0.
  var start = 0;
  // 2. Repeat, for each index i up to and including the length of s,
  for (var i = 0; i <= s.length; i++) {
    // a. If i is the end of s or the code unit at i is "-", then
    if (i === s.length || charCodeAt(s, i) === 0x2d) {
      // i. If the subtag from start to i is not 3 to 8 code units long,
      //    return false.
      if (i - start < 3 || i - start > 8) return false;
      // ii. Set start to i + 1.
      start = i + 1;
    } else if (!isAsciiAlnum(charCodeAt(s, i))) {
      // b. Else if the code unit is not a letter or digit, return false.
      return false;
    }
  }
  // 3. Return true.
  return true;
}

/// Whether s is matched by unicode_subdivision_id (UTS 35 3.2): a
/// unicode_region_subtag followed by alphanum{1,4}.
function isUnicodeSubdivisionId(s) {
  // 1. Let regionLength be 2 if s starts with two letters, 3 if it starts
  //    with three digits, and otherwise return false.
  var regionLength;
  if (s.length >= 2 && isAlphaRange(s, 0, 2)) regionLength = 2;
  else if (s.length >= 3 && isDigitRange(s, 0, 3)) regionLength = 3;
  else return false;
  // 2. Let rest be the number of code units after the region subtag.
  var rest = s.length - regionLength;
  // 3. Return whether rest is 1 to 4 and those code units are letters or
  //    digits.
  return rest >= 1 && rest <= 4 && isAlnumRange(s, regionLength, s.length);
}

// ECMA-262 StringSplitToList ( S, R )
function StringSplitToList(S, R) {
  // 1. Let substrings be a new empty List.
  var substrings = [];
  // 2. Let i be 0.
  var i = 0;
  // 3. Let j be StringIndexOf(S, R, 0).
  var j = stringIndexOf(S, R, 0);
  // 4. Repeat, while j is not not-found,
  while (j !== -1) {
    // a. Let T be the substring of S from i to j.
    // b. Append T to substrings.
    listPush(substrings, stringSlice(S, i, j));
    // c. Set i to j + the length of R.
    i = j + R.length;
    // d. Set j to StringIndexOf(S, R, i).
    j = stringIndexOf(S, R, i);
  }
  // 5. Let T be the substring of S from i.
  // 6. Append T to substrings.
  listPush(substrings, stringSlice(S, i));
  // 7. Return substrings.
  return substrings;
}

// GetLocaleBaseName ( locale )
function GetLocaleBaseName(locale) {
  // 1. Assert: locale can be matched by the unicode_locale_id Unicode locale
  //    nonterminal.
  // 2. Return the longest prefix of locale matched by the unicode_language_id
  //    Unicode locale nonterminal: a language subtag, an optional script, an
  //    optional region, and variants. The subtag after that is a singleton,
  //    which no other production can match.
  var subtags = StringSplitToList(locale, '-');
  var end = subtags[0].length;
  var k = 1;
  if (k < subtags.length && isUnicodeScriptSubtag(subtags[k])) {
    end = end + 1 + subtags[k].length;
    k++;
  }
  if (k < subtags.length && isUnicodeRegionSubtag(subtags[k])) {
    end = end + 1 + subtags[k].length;
    k++;
  }
  while (k < subtags.length && isUnicodeVariantSubtag(subtags[k])) {
    end = end + 1 + subtags[k].length;
    k++;
  }
  return stringSlice(locale, 0, end);
}

// GetLocaleLanguage ( locale )
function GetLocaleLanguage(locale) {
  // 1. Let baseName be GetLocaleBaseName(locale).
  var baseName = GetLocaleBaseName(locale);
  // 2. Let subtags be StringSplitToList(baseName, "-").
  var subtags = StringSplitToList(baseName, '-');
  // 3. Assert: subtags[0] is matched by the unicode_language_subtag Unicode
  //    locale nonterminal.
  // 4. Return subtags[0].
  return subtags[0];
}

// GetLocaleScript ( locale )
function GetLocaleScript(locale) {
  // 1. Let baseName be GetLocaleBaseName(locale).
  var baseName = GetLocaleBaseName(locale);
  // 2. NOTE: A unicode_script_subtag is only valid immediately after the
  //    initial unicode_language_subtag.
  // 3. Let subtags be StringSplitToList(baseName, "-").
  var subtags = StringSplitToList(baseName, '-');
  // 4. Assert: subtags[0] is matched by the unicode_language_subtag Unicode
  //    locale nonterminal.
  // 5. If subtags has at least 2 elements and subtags[1] is matched by the
  //    unicode_script_subtag Unicode locale nonterminal, return subtags[1].
  if (subtags.length >= 2 && isUnicodeScriptSubtag(subtags[1]))
    return subtags[1];
  // 6. Return undefined.
  return undefined;
}

// GetLocaleRegion ( locale )
function GetLocaleRegion(locale) {
  // 1. Let baseName be GetLocaleBaseName(locale).
  var baseName = GetLocaleBaseName(locale);
  // 2. NOTE: A unicode_region_subtag is only valid immediately after the
  //    initial unicode_language_subtag, optionally with a single
  //    unicode_script_subtag between them.
  // 3. Let subtags be StringSplitToList(baseName, "-").
  var subtags = StringSplitToList(baseName, '-');
  // 4. Assert: subtags[0] is matched by the unicode_language_subtag Unicode
  //    locale nonterminal.
  // 5. If subtags has at least 2 elements and subtags[1] is matched by the
  //    unicode_region_subtag Unicode locale nonterminal, return subtags[1].
  if (subtags.length >= 2 && isUnicodeRegionSubtag(subtags[1]))
    return subtags[1];
  // 6. If subtags has at least 3 elements and subtags[2] is matched by the
  //    unicode_region_subtag Unicode locale nonterminal, return subtags[2].
  if (subtags.length >= 3 && isUnicodeRegionSubtag(subtags[2]))
    return subtags[2];
  // 7. Return undefined.
  return undefined;
}

// GetLocaleVariants ( locale )
function GetLocaleVariants(locale) {
  // 1. Let baseName be GetLocaleBaseName(locale).
  var baseName = GetLocaleBaseName(locale);
  // 2. NOTE: Each non-initial subtag in baseName is a unicode_script_subtag,
  //    unicode_region_subtag or unicode_variant_subtag, whose productions
  //    are mutually disjoint.
  // 3. Let subtags be StringSplitToList(baseName, "-").
  var subtags = StringSplitToList(baseName, '-');
  // 4. Assert: subtags[0] is matched by the unicode_language_subtag Unicode
  //    locale nonterminal.
  // 5. Let dashIndex be the length of subtags[0].
  var dashIndex = subtags[0].length;
  // 6. Remove the first element from subtags.
  // 7. For each String subtag of subtags (from the second element), do
  for (var i = 1; i < subtags.length; i++) {
    var subtag = subtags[i];
    // a. If subtag is matched by the unicode_variant_subtag Unicode locale
    //    nonterminal, then
    //    i. Return the substring of baseName from dashIndex + 1.
    if (isUnicodeVariantSubtag(subtag))
      return stringSlice(baseName, dashIndex + 1);
    // b. Let subtagLength be the length of subtag.
    // c. Set dashIndex to dashIndex + subtagLength + 1.
    dashIndex = dashIndex + subtag.length + 1;
  }
  // 8. Return undefined.
  return undefined;
}

// UnicodeExtensionValue ( locale, key ). The draft's ~empty~ is
// undefined here.
function UnicodeExtensionValue(locale, key) {
  // 1. If locale contains a Unicode locale extension sequence, then
  var extension = splitUnicodeExtension(locale).extension;
  if (extension !== undefined) {
    // a. Let extension be the Unicode locale extension sequence of locale.
    // b. Let components be UnicodeExtensionComponents(extension).
    var components = UnicodeExtensionComponents(extension);
    // c. Let keywords be components.[[Keywords]].
    // d. If there exists an element entry of keywords such that
    //    entry.[[Key]] is key, return entry.[[Value]].
    var entry = findKeyword(components.keywords, key);
    if (entry !== undefined) return entry.value;
  }
  // 2. Return ~empty~.
  return undefined;
}

// CanonicalUnicodeSubdivision ( locale, key )
function CanonicalUnicodeSubdivision(locale, key) {
  // 1. Let subdivision be UnicodeExtensionValue(locale, key).
  var subdivision = UnicodeExtensionValue(locale, key);
  // 2. If subdivision is ~empty~, return undefined.
  if (subdivision === undefined) return undefined;
  // 3. If subdivision cannot be matched by the unicode_subdivision_id
  //    Unicode locale nonterminal, return undefined.
  if (!isUnicodeSubdivisionId(subdivision)) return undefined;
  // 4. Let region be the longest prefix of subdivision matched by the
  //    unicode_region_subtag Unicode locale nonterminal: 3 digits, or else
  //    2 letters.
  var region = stringSlice(
    subdivision,
    0,
    isAsciiDigit(charCodeAt(subdivision, 0)) ? 3 : 2,
  );
  // 5. Let regionLocale be the string-concatenation of "und-" and region.
  var regionLocale = 'und-' + region;
  // 6. Set regionLocale to CanonicalizeUnicodeLocaleId(regionLocale).
  regionLocale = native.canonicalizeLocale(regionLocale);
  // 7. Return GetLocaleRegion(regionLocale).
  return GetLocaleRegion(regionLocale);
}

// RegionPreference ( locale )
function RegionPreference(locale) {
  // 1. Let region be GetLocaleRegion(locale).
  var region = GetLocaleRegion(locale);
  // 2. If region is undefined, then
  if (region === undefined) {
    // a. Set region to CanonicalUnicodeSubdivision(locale, "sd").
    region = CanonicalUnicodeSubdivision(locale, 'sd');
    // b. If region is undefined, then
    if (region === undefined) {
      // i. Let maximal be the result of the Add Likely Subtags algorithm
      //    applied to locale. If an error is signaled, set maximal to
      //    locale. (The native helper cannot fail on a canonical tag.)
      var maximal = native.addLikelySubtags(locale);
      // ii. Set maximal to CanonicalizeUnicodeLocaleId(maximal). (The helper
      //     returns a canonical tag.)
      // iii. Set region to GetLocaleRegion(maximal).
      region = GetLocaleRegion(maximal);
      // iv. If region is undefined, then
      //     1. Set region to "001".
      if (region === undefined) region = '001';
    }
  }
  // 3. Let regionOverride be CanonicalUnicodeSubdivision(locale, "rg").
  var regionOverride = CanonicalUnicodeSubdivision(locale, 'rg');
  // 4. Return { [[Region]]: region, [[RegionOverride]]: regionOverride }.
  return {__proto__: null, region: region, regionOverride: regionOverride};
}

// WeekdayToUValue ( fw )
function WeekdayToUValue(fw) {
  // 1. For each row of the Weekday String and Value table, except the
  //    header row, in table order, do
  //    a. Let weekday be the Weekday value of the current row.
  //    b. Let string be the String value of the current row.
  //    c. If fw is equal to weekday, return string.
  switch (fw) {
    case '0':
    case '7':
      return 'sun';
    case '1':
      return 'mon';
    case '2':
      return 'tue';
    case '3':
      return 'wed';
    case '4':
      return 'thu';
    case '5':
      return 'fri';
    case '6':
      return 'sat';
  }
  // 2. Return fw.
  return fw;
}

// WeekdayUValueToNumber ( fw )
function WeekdayUValueToNumber(fw) {
  // 1. For each row of the Weekday String and Value table, except the
  //    header row, in table order, do
  //    a. Let string be the String value of the current row.
  //    b. Let value be the Value value of the current row.
  //    c. If fw is equal to string, return value.
  switch (fw) {
    case 'sun':
      return 7;
    case 'mon':
      return 1;
    case 'tue':
      return 2;
    case 'wed':
      return 3;
    case 'thu':
      return 4;
    case 'fri':
      return 5;
    case 'sat':
      return 6;
  }
  // 2. Return undefined.
  return undefined;
}
