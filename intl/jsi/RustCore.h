/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#pragma once

/// C++ wrappers over the Rust core's C interface (hermes_intl_core.h,
/// generated from intl/core/src/lib.rs).

#include "hermes_intl_core.h"

#include <optional>
#include <string>
#include <string_view>
#include <vector>

namespace hermes_intl::core {

/// The bytes of \p s as the Rust core takes them.
inline const uint8_t *bytes(std::string_view s) {
  return reinterpret_cast<const uint8_t *>(s.data());
}

/// Frees a string the Rust core returned when it goes out of scope, so that
/// the buffer is released even if copying it throws.
template <class RustString, void (*Free)(RustString)>
class RustStringOwner {
 public:
  explicit RustStringOwner(RustString s) : s_(s) {}
  RustStringOwner(const RustStringOwner &) = delete;
  RustStringOwner &operator=(const RustStringOwner &) = delete;
  ~RustStringOwner() {
    Free(s_);
  }

 private:
  RustString s_;
};

/// Take ownership of a string the Rust core returned.
inline std::string takeString(HiString s) {
  RustStringOwner<HiString, hi_string_free> owner(s);
  std::string result;
  if (s.len)
    result.assign(reinterpret_cast<const char *>(s.data), s.len);
  return result;
}

/// Take ownership of a UTF-16 string the Rust core returned.
inline std::u16string takeString16(HiString16 s) {
  RustStringOwner<HiString16, hi_string16_free> owner(s);
  std::u16string result;
  if (s.len)
    result.assign(reinterpret_cast<const char16_t *>(s.data), s.len);
  return result;
}

/// Call a Rust function of the shape (data, len, out) -> bool.
template <class F>
std::optional<std::string> callOptional(F function, std::string_view in) {
  HiString out;
  if (!function(reinterpret_cast<const uint8_t *>(in.data()), in.size(), &out))
    return std::nullopt;
  return takeString(out);
}

/// IsStructurallyValidLanguageTag + CanonicalizeUnicodeLocaleId.
inline std::optional<std::string> canonicalizeLocale(std::string_view tag) {
  return callOptional(hi_locale_canonicalize, tag);
}

/// Add Likely Subtags of a canonical tag; nullopt if it does not parse.
inline std::optional<std::string> addLikelySubtags(std::string_view tag) {
  return callOptional(hi_locale_add_likely_subtags, tag);
}

/// Remove Likely Subtags of a canonical tag; nullopt if it does not parse.
inline std::optional<std::string> removeLikelySubtags(std::string_view tag) {
  return callOptional(hi_locale_remove_likely_subtags, tag);
}

/// The CLDR calendar types for \p region, preferred first, space-separated;
/// the "001" ones for a region with no line of its own, nullopt only if
/// \p region is not a region code.
inline std::optional<std::string> calendarPreferences(std::string_view region) {
  return callOptional(hi_locale_calendar_preferences, region);
}

/// The CLDR hour cycles for \p key (a region or language-region), preferred
/// first, space-separated; the "001" ones for a region with no line of its
/// own, nullopt for a language-region key with no line.
inline std::optional<std::string> hourCycles(std::string_view key) {
  return callOptional(hi_locale_hour_cycles, key);
}

/// The primary IANA ids of the time zones of \p region, sorted,
/// space-separated; empty for a region with none.
inline std::string timeZonesOfRegion(std::string_view region) {
  return callOptional(hi_locale_time_zones_of_region, region).value_or("");
}

/// The week data of \p region: the ISO number of the first day of the week
/// (Monday 1 to Sunday 7), then the weekend days ascending, space-separated;
/// nullopt if \p region is not a region code.
inline std::optional<std::string> weekInfo(std::string_view region) {
  return callOptional(hi_locale_week_info, region);
}

/// "ltr" or "rtl" for a script code; nullopt if unknown.
inline std::optional<std::string> scriptDirection(std::string_view script) {
  return callOptional(hi_locale_script_direction, script);
}

/// The numbering system of \p locale's decimal data; nullopt if \p locale
/// does not parse.
inline std::optional<std::string> defaultNumberingSystem(
    std::string_view locale) {
  return callOptional(hi_locale_default_numbering_system, locale);
}

/// The collation types \p locale has tailorings for, sorted,
/// space-separated.
inline std::string collationsOfLocale(std::string_view locale) {
  return callOptional(hi_locale_collations, locale).value_or("");
}

/// Whether \p tag (canonical, no extensions) is an available locale.
inline bool isAvailableLocale(std::string_view tag) {
  return hi_locale_is_available(
      reinterpret_cast<const uint8_t *>(tag.data()), tag.size());
}

/// A host locale name to a canonical, extension-free, available BCP 47 tag.
inline std::optional<std::string> localeFromHost(std::string_view raw) {
  return callOptional(hi_locale_from_host, raw);
}

/// A well-formed tag from IntlDefaultLocales to a canonical, extension-free,
/// available tag of the same script that has data in that script; nullopt
/// if it is ill-formed or nothing of it is usable. The same reduction
/// localeFromHost ends in.
inline std::optional<std::string> resolveDefaultLocale(std::string_view tag) {
  return callOptional(hi_locale_resolve_default, tag);
}

/// A TZ value or /etc/localtime link target to a primary IANA name.
inline std::optional<std::string> timeZoneFromHost(std::string_view raw) {
  return callOptional(hi_time_zone_from_host, raw);
}

/// A case-insensitive IANA name to its primary identifier.
inline std::optional<std::string> canonicalizeTimeZone(std::string_view name) {
  return callOptional(hi_time_zone_canonicalize, name);
}

/// ICU4X's default "<caseFirst> <ignorePunctuation>" for the language
/// identifier of \p locale ("upper false"); nullopt if \p locale does not
/// parse.
inline std::optional<std::string> collatorLocaleDefaults(
    std::string_view locale) {
  return callOptional(hi_collator_locale_defaults, locale);
}

/// A collator for \p locale (canonical; only its language identifier is
/// used), \p collation ("default" or a supported collation),
/// \p sensitivity ("base", "accent", "case" or "variant") and \p caseFirst
/// ("upper", "lower" or "false"); null for any other value or if there is
/// no data. The caller frees it with hi_collator_free.
inline HiCollator *collatorNew(
    std::string_view locale,
    std::string_view collation,
    std::string_view sensitivity,
    bool ignorePunctuation,
    bool numeric,
    std::string_view caseFirst) {
  return hi_collator_new(
      bytes(locale),
      locale.size(),
      bytes(collation),
      collation.size(),
      bytes(sensitivity),
      sensitivity.size(),
      ignorePunctuation,
      numeric,
      bytes(caseFirst),
      caseFirst.size());
}

/// CompareStrings of \p x and \p y with \p collator: -1, 0 or 1. An
/// unpaired surrogate compares as U+FFFD.
inline int collatorCompare(
    const HiCollator *collator,
    std::u16string_view x,
    std::u16string_view y) {
  return hi_collator_compare(
      collator,
      reinterpret_cast<const uint16_t *>(x.data()),
      x.size(),
      reinterpret_cast<const uint16_t *>(y.data()),
      y.size());
}

/// The full upper (\p upper) or lower case mapping of \p text with the
/// tailoring of \p language ("und" or a language tag). Unpaired surrogates
/// are kept. nullopt if \p language does not parse.
inline std::optional<std::u16string>
caseMap(std::u16string_view text, std::string_view language, bool upper) {
  HiString16 out;
  if (!hi_case_map(
          reinterpret_cast<const uint16_t *>(text.data()),
          text.size(),
          bytes(language),
          language.size(),
          upper,
          &out))
    return std::nullopt;
  return takeString16(out);
}

/// The cardinal (\p ordinal false) or ordinal plural rules for the language
/// identifier of \p locale (canonical); null if \p locale does not parse or
/// has no data. The caller frees them with hi_plural_rules_free.
inline HiPluralRules *pluralRulesNew(std::string_view locale, bool ordinal) {
  return hi_plural_rules_new(bytes(locale), locale.size(), ordinal);
}

/// The plural category of \p decimal x 10^\p exponent with \p rules;
/// nullopt unless \p decimal is ASCII digits with an optional "." between
/// digits.
inline std::optional<std::string> pluralRulesSelect(
    const HiPluralRules *rules,
    std::string_view decimal,
    int32_t exponent) {
  HiString out;
  if (!hi_plural_rules_select(
          rules, bytes(decimal), decimal.size(), exponent, &out))
    return std::nullopt;
  return takeString(out);
}

/// The plural category of a range whose ends have the categories \p start
/// and \p end, with \p rules; nullopt if either is not a category name.
inline std::optional<std::string> pluralRulesSelectRange(
    const HiPluralRules *rules,
    std::string_view start,
    std::string_view end) {
  HiString out;
  if (!hi_plural_rules_select_range(
          rules, bytes(start), start.size(), bytes(end), end.size(), &out))
    return std::nullopt;
  return takeString(out);
}

/// Take ownership of a list the Rust core returned as one string, its
/// entries separated by U+0000. An entry may be empty; there is always at
/// least one.
inline std::vector<std::string> takeList(HiString s) {
  std::string joined = takeString(s);
  std::vector<std::string> result;
  size_t start = 0;
  for (;;) {
    size_t end = joined.find('\0', start);
    if (end == std::string::npos) {
      result.emplace_back(joined, start);
      return result;
    }
    result.emplace_back(joined, start, end - start);
    start = end + 1;
  }
}

/// The categories of \p rules in the order zero, one, two, few, many,
/// other.
inline std::vector<std::string> pluralRulesCategories(
    const HiPluralRules *rules) {
  HiString out;
  hi_plural_rules_categories(rules, &out);
  return takeList(out);
}

/// The list format for the language identifier of \p locale (canonical),
/// \p type ("conjunction", "disjunction" or "unit") and \p style ("long",
/// "short" or "narrow"); null for any other value, if \p locale does not
/// parse, or if there is no data. The caller frees it with
/// hi_list_format_free.
inline HiListFormat *listFormatNew(
    std::string_view locale,
    std::string_view type,
    std::string_view style) {
  return hi_list_format_new(
      bytes(locale),
      locale.size(),
      bytes(type),
      type.size(),
      bytes(style),
      style.size());
}

/// The literal text \p format puts around the elements of a list of two
/// (\p pair) or more elements whose last element is \p last: the 3 or 5
/// entries hi_list_format_literals documents.
inline std::vector<std::string> listFormatLiterals(
    const HiListFormat *format,
    bool pair,
    std::u16string_view last) {
  HiString out;
  hi_list_format_literals(
      format,
      pair,
      reinterpret_cast<const uint16_t *>(last.data()),
      last.size(),
      &out);
  return takeList(out);
}

/// The segmenter of \p granularity ("grapheme", "word" or "sentence") with
/// the language identifier of \p locale (canonical) as content locale; null
/// for any other granularity, if \p locale does not parse, or if there is no
/// data. The caller frees it with hi_segmenter_free.
inline HiSegmenter *segmenterNew(
    std::string_view locale,
    std::string_view granularity) {
  return hi_segmenter_new(
      bytes(locale), locale.size(), bytes(granularity), granularity.size());
}

/// The segments of \p text by \p segmenter; null if \p text is longer than
/// INT_MAX code units. The caller frees them with hi_segments_free.
inline HiSegments *segmentsNew(
    const HiSegmenter *segmenter,
    std::u16string_view text) {
  return hi_segments_new(
      segmenter, reinterpret_cast<const uint16_t *>(text.data()), text.size());
}

/// FindBoundary in \p segments: the last boundary at or before \p index, or
/// with \p after the first boundary after it. nullopt unless 0 <= \p index <
/// the length of the string.
inline std::optional<int>
segmentsFindBoundary(const HiSegments *segments, int index, bool after) {
  int32_t boundary = hi_segments_find_boundary(segments, index, after);
  if (boundary < 0)
    return std::nullopt;
  return boundary;
}

/// Whether the segment of \p segments that starts at the boundary \p start
/// is word-like. nullopt if the granularity is not word, or if \p start is
/// not a boundary or is the length of the string.
inline std::optional<bool> segmentsIsWordLike(
    const HiSegments *segments,
    int start) {
  bool wordLike;
  if (!hi_segments_is_word_like(segments, start, &wordLike))
    return std::nullopt;
  return wordLike;
}

/// NumberFormat's symbols and decimal and percent patterns for the language
/// identifier of \p locale (canonical) and the numbering system \p nu: the
/// 23 entries hi_number_symbols documents. nullopt if \p locale does not
/// parse or \p nu is not a numbering system type.
inline std::optional<std::vector<std::string>> numberSymbols(
    std::string_view locale,
    std::string_view nu) {
  HiString out;
  if (!hi_number_symbols(
          bytes(locale), locale.size(), bytes(nu), nu.size(), &out))
    return std::nullopt;
  return takeList(out);
}

/// NumberFormat's currency patterns for \p locale, \p nu, \p currency
/// (three uppercase letters) and the currencyDisplay \p display, standard
/// or \p accounting: the entries hi_number_currency_patterns documents.
/// nullopt for any other currency or display, or as numberSymbols.
inline std::optional<std::vector<std::string>> currencyPatterns(
    std::string_view locale,
    std::string_view nu,
    std::string_view currency,
    std::string_view display,
    bool accounting) {
  HiString out;
  if (!hi_number_currency_patterns(
          bytes(locale),
          locale.size(),
          bytes(nu),
          nu.size(),
          bytes(currency),
          currency.size(),
          bytes(display),
          display.size(),
          accounting,
          &out))
    return std::nullopt;
  return takeList(out);
}

/// NumberFormat's patterns for compact notation with the currency style for
/// \p locale, \p nu, \p currency (three uppercase letters) and the
/// currencyDisplay \p display ("code", "symbol" or "narrowSymbol"),
/// standard or \p accounting: the entries
/// hi_number_compact_currency_patterns documents. nullopt for any other
/// currency or display, or as numberSymbols.
inline std::optional<std::vector<std::string>> compactCurrencyPatterns(
    std::string_view locale,
    std::string_view nu,
    std::string_view currency,
    std::string_view display,
    bool accounting) {
  HiString out;
  if (!hi_number_compact_currency_patterns(
          bytes(locale),
          locale.size(),
          bytes(nu),
          nu.size(),
          bytes(currency),
          currency.size(),
          bytes(display),
          display.size(),
          accounting,
          &out))
    return std::nullopt;
  return takeList(out);
}

/// The currency style's decimal and grouping separators for \p locale,
/// \p nu and \p currency (three uppercase letters): the two entries
/// hi_number_currency_separators documents. nullopt for any other currency,
/// or as numberSymbols.
inline std::optional<std::vector<std::string>> currencySeparators(
    std::string_view locale,
    std::string_view nu,
    std::string_view currency) {
  HiString out;
  if (!hi_number_currency_separators(
          bytes(locale),
          locale.size(),
          bytes(nu),
          nu.size(),
          bytes(currency),
          currency.size(),
          &out))
    return std::nullopt;
  return takeList(out);
}

/// NumberFormat's short or (\p long_) long compact patterns for \p locale
/// and \p nu: the entries hi_number_compact_patterns documents. nullopt as
/// numberSymbols.
inline std::optional<std::vector<std::string>>
compactPatterns(std::string_view locale, std::string_view nu, bool long_) {
  HiString out;
  if (!hi_number_compact_patterns(
          bytes(locale), locale.size(), bytes(nu), nu.size(), long_, &out))
    return std::nullopt;
  return takeList(out);
}

/// The six unit patterns (zero, one, two, few, many, other; CLDR form) of
/// \p unit at \p width for \p locale. nullopt for a unit or width the
/// patterns are not defined for.
inline std::optional<std::vector<std::string>> unitPatterns(
    std::string_view locale,
    std::string_view unit,
    std::string_view width) {
  HiString out;
  if (!hi_unit_patterns(
          bytes(locale),
          locale.size(),
          bytes(unit),
          unit.size(),
          bytes(width),
          width.size(),
          &out))
    return std::nullopt;
  return takeList(out);
}

/// RelativeTimeFormat's CLDR data for \p locale and \p style ("long",
/// "short" or "narrow"): the entries hi_relative_time_patterns documents.
/// nullopt for any other style, if \p locale does not parse, or if there is
/// no data.
inline std::optional<std::vector<std::string>> relativeTimePatterns(
    std::string_view locale,
    std::string_view style) {
  HiString out;
  if (!hi_relative_time_patterns(
          bytes(locale), locale.size(), bytes(style), style.size(), &out))
    return std::nullopt;
  return takeList(out);
}

/// A DisplayNames name of \p code for \p locale (canonical), \p kind and
/// \p style, as hi_display_name documents: the outer nullopt for any other
/// kind or style or if \p locale does not parse, the inner one when there is
/// no name.
inline std::optional<std::optional<std::string>> displayName(
    std::string_view locale,
    std::string_view kind,
    std::string_view code,
    std::string_view style) {
  bool found;
  HiString out;
  if (!hi_display_name(
          bytes(locale),
          locale.size(),
          bytes(kind),
          kind.size(),
          bytes(code),
          code.size(),
          bytes(style),
          style.size(),
          &found,
          &out))
    return std::nullopt;
  if (!found)
    return std::optional<std::string>();
  return std::optional<std::string>(takeString(out));
}

/// The locale display patterns of \p locale (canonical): localePattern and
/// localeSeparator in CLDR's form. nullopt if \p locale does not parse.
inline std::optional<std::vector<std::string>> localeDisplayPatterns(
    std::string_view locale) {
  HiString out;
  if (!hi_locale_display_patterns(bytes(locale), locale.size(), &out))
    return std::nullopt;
  return takeList(out);
}

/// DateTimeFormat's CLDR patterns for \p locale and \p calendar (one of the
/// 16 supported calendars): the entries hi_date_time_patterns documents.
/// nullopt if \p locale does not parse or the calendar is not supported.
inline std::optional<std::vector<std::string>> dateTimePatterns(
    std::string_view locale,
    std::string_view calendar) {
  HiString out;
  if (!hi_date_time_patterns(
          bytes(locale), locale.size(), bytes(calendar), calendar.size(), &out))
    return std::nullopt;
  return takeList(out);
}

/// DateTimeFormat's flexible day periods for \p locale: the entries
/// hi_day_periods documents. nullopt if \p locale does not parse.
inline std::optional<std::vector<std::string>> dayPeriods(
    std::string_view locale) {
  HiString out;
  if (!hi_day_periods(bytes(locale), locale.size(), &out))
    return std::nullopt;
  return takeList(out);
}

/// The native state of a DateTimeFormat for the language identifier of
/// \p locale (canonical), \p calendar, \p timeZone and \p symbols (CLDR name
/// symbols separated by single spaces); null for any value
/// hi_date_time_format_new rejects. The caller frees it with
/// hi_date_time_format_free.
inline HiDateTimeFormat *dateTimeFormatNew(
    std::string_view locale,
    std::string_view calendar,
    std::string_view timeZone,
    std::string_view symbols) {
  return hi_date_time_format_new(
      bytes(locale),
      locale.size(),
      bytes(calendar),
      calendar.size(),
      bytes(timeZone),
      timeZone.size(),
      bytes(symbols),
      symbols.size());
}

/// The local time of \p epochMs with \p format and the text of its symbols:
/// when it returns HiLocalTimeStatus_Ok, \p result is the entries
/// hi_date_time_format_local_time documents; otherwise the status says
/// whether \p epochMs was not a time value or ICU4X failed, and \p result
/// is unchanged.
inline HiLocalTimeStatus dateTimeFormatLocalTime(
    const HiDateTimeFormat *format,
    double epochMs,
    std::vector<std::string> &result) {
  HiString out;
  HiLocalTimeStatus status =
      hi_date_time_format_local_time(format, epochMs, &out);
  if (status == HiLocalTimeStatus_Ok)
    result = takeList(out);
  return status;
}

/// CurrencyDigits of \p currency (three uppercase letters): CLDR's fraction
/// digits, 2 for a currency CLDR does not list. nullopt for anything else.
inline std::optional<int> currencyDigits(std::string_view currency) {
  int32_t digits = hi_number_currency_digits(bytes(currency), currency.size());
  if (digits < 0)
    return std::nullopt;
  return digits;
}

/// The values of \p kind for Intl.supportedValuesOf, space-separated.
inline std::string supportedValues(HiValueKind kind) {
  HiString out;
  hi_supported_values(kind, &out);
  return takeString(out);
}

} // namespace hermes_intl::core
