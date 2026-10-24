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

/// Take ownership of a string the Rust core returned.
inline std::string takeString(HiString s) {
  std::string result;
  if (s.len)
    result.assign(reinterpret_cast<const char *>(s.data), s.len);
  hi_string_free(s);
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
