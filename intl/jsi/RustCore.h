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

namespace hermes_intl::core {

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

/// The values of \p kind for Intl.supportedValuesOf, space-separated.
inline std::string supportedValues(HiValueKind kind) {
  HiString out;
  hi_supported_values(kind, &out);
  return takeString(out);
}

} // namespace hermes_intl::core
