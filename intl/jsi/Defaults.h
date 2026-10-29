/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#pragma once

#include <hermes/Public/IntlPlugin.h>

#include <cstdlib>
#include <functional>
#include <string>
#include <string_view>
#include <vector>

namespace hermes_intl {

/// Where detection reads the host's settings; tests substitute their own.
/// Apple and Windows read neither: their settings come from system APIs.
struct HostEnvironment {
  /// Like std::getenv: nullptr when the variable is not set.
  std::function<const char *(const char *)> getEnv =
      [](const char *name) -> const char * { return std::getenv(name); };
  /// The system time zone link (Linux).
  std::string localtimePath = "/etc/localtime";
};

/// A Windows locale name (GetUserDefaultLocaleName) as a locale candidate:
/// the name without its sort-order suffix, so "de-DE_phoneb" is "de-DE".
/// Pseudo-locales ("qps-ploc") are kept; core::localeFromHost rejects them.
std::string windowsLocaleCandidate(std::string_view name);

/// The first of \p candidates that core::localeFromHost turns into an
/// available locale, else "en-US".
std::string localeFromCandidates(const std::vector<std::string> &candidates);

/// The host's locale settings, most authoritative first, unnormalized. See
/// "Default locale and time zone" in the spec.
std::vector<std::string> hostLocaleCandidates(const HostEnvironment &env);

/// The host's time zone settings, most authoritative first, unnormalized.
/// On Windows, the IANA name CLDR's windowsZones gives for the system zone
/// (TZ is not read: the C runtime's TZ is not an IANA name).
std::vector<std::string> hostTimeZoneCandidates(const HostEnvironment &env);

/// localeFromCandidates of the host's locale candidates.
std::string detectDefaultLocale(const HostEnvironment &env);

/// The first time zone candidate that names a known IANA zone, else "UTC".
std::string detectDefaultTimeZone(const HostEnvironment &env);

/// hermes_intl_entry::resolve_defaults against \p env. Explicit locales
/// are canonicalized, stripped of extensions and reduced to available
/// locales; those that reduce to nothing, and repeats, are dropped. The
/// explicit time zone is canonicalized. Missing values are detected.
/// Returns "" on success, leaving the effective values in \p config, or a
/// message naming the invalid value, leaving \p config unchanged. Invalid
/// means an ill-formed locale, a non-empty locale list none of whose
/// elements is usable, or an unknown time zone.
std::string resolveDefaults(
    hermes_intl_config *config,
    const HostEnvironment &env);

/// resolveDefaults against the real host.
std::string resolveDefaults(hermes_intl_config *config);

} // namespace hermes_intl
