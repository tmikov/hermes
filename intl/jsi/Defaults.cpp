/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "Defaults.h"

#include "RustCore.h"

#include <algorithm>
#include <utility>

#if defined(__APPLE__)
#include <CoreFoundation/CoreFoundation.h>
#elif defined(__ANDROID__)
#include <sys/system_properties.h>
#elif defined(_WIN32)
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#include <climits>
#include <cwchar>
#include <iterator>
#else
#include <unistd.h>
#include <climits>
#endif

namespace hermes_intl {
namespace {

#if !defined(__APPLE__) && !defined(_WIN32)
/// Append TZ to \p out if it is set. A set but empty TZ means UTC, as it
/// does to the C library that Date's local time comes from: glibc treats
/// TZ="" as UTC (`env TZ= date +%Z` prints "Universal"), and so does the
/// upstream tzcode bionic uses. It is a candidate, so the sources after it
/// are not consulted; only an unset TZ falls through to them.
void addTimeZoneEnv(std::vector<std::string> &out, const HostEnvironment &env) {
  if (const char *value = env.getEnv("TZ"))
    out.emplace_back(*value ? value : "UTC");
}
#endif

#if !defined(__APPLE__) && !defined(__ANDROID__) && !defined(_WIN32)
/// Append the variable \p name to \p out if it is set and not empty. POSIX
/// treats an empty locale variable as unset.
void addEnv(
    std::vector<std::string> &out,
    const HostEnvironment &env,
    const char *name) {
  const char *value = env.getEnv(name);
  if (value && *value)
    out.emplace_back(value);
}

/// The target of the symlink at \p path, or "" if it is not a symlink.
std::string readSymlink(const std::string &path) {
  char buf[PATH_MAX];
  ssize_t n = ::readlink(path.c_str(), buf, sizeof(buf) - 1);
  return n > 0 ? std::string(buf, n) : std::string();
}
#endif

#if defined(__ANDROID__)
/// The Android system property \p name, or "".
std::string systemProperty(const char *name) {
  char buf[PROP_VALUE_MAX];
  int n = __system_property_get(name, buf);
  return n > 0 ? std::string(buf, n) : std::string();
}
#endif

#if defined(__APPLE__)
/// \p s as UTF-8; "" if \p s is null or cannot be converted.
std::string toUtf8(CFStringRef s) {
  if (!s)
    return "";
  CFIndex length = CFStringGetLength(s);
  CFIndex size =
      CFStringGetMaximumSizeForEncoding(length, kCFStringEncodingUTF8) + 1;
  std::string result(size, '\0');
  if (!CFStringGetCString(s, result.data(), size, kCFStringEncodingUTF8))
    return "";
  result.resize(std::char_traits<char>::length(result.c_str()));
  return result;
}
#endif

#if defined(_WIN32)
/// The \p len UTF-16 units at \p s as UTF-8; "" if they are not valid
/// UTF-16.
std::string toUtf8(const wchar_t *s, size_t len) {
  if (len == 0 || len > INT_MAX)
    return "";
  int n = static_cast<int>(len);
  int size = ::WideCharToMultiByte(
      CP_UTF8, WC_ERR_INVALID_CHARS, s, n, nullptr, 0, nullptr, nullptr);
  if (size <= 0)
    return "";
  std::string result(size, '\0');
  if (::WideCharToMultiByte(
          CP_UTF8,
          WC_ERR_INVALID_CHARS,
          s,
          n,
          result.data(),
          size,
          nullptr,
          nullptr) != size)
    return "";
  return result;
}

/// The NUL-terminated \p s as UTF-8.
std::string toUtf8(const wchar_t *s) {
  return toUtf8(s, std::wcslen(s));
}

/// The user's region from GetUserDefaultGeoName (an ISO 3166 code or a UN
/// M.49 number), or "" if there is none. The function exists from Windows
/// 10 1709, so it is looked up at run time: the plugin still loads on older
/// systems, which get the territory 001 zone.
std::string userRegion() {
  using GetUserDefaultGeoNameFn = int(WINAPI *)(LPWSTR, int);
  HMODULE kernel32 = ::GetModuleHandleW(L"kernel32.dll");
  if (!kernel32)
    return "";
  auto getUserDefaultGeoName =
      reinterpret_cast<GetUserDefaultGeoNameFn>(reinterpret_cast<void *>(
          ::GetProcAddress(kernel32, "GetUserDefaultGeoName")));
  if (!getUserDefaultGeoName)
    return "";
  wchar_t buf[16];
  int n = getUserDefaultGeoName(buf, static_cast<int>(std::size(buf)));
  // n counts the terminating NUL.
  return n > 1 ? toUtf8(buf, n - 1) : "";
}
#endif

/// \p tags as a comma-separated list of quoted strings, for messages.
std::string quoteList(const std::vector<std::string> &tags) {
  std::string out;
  for (const std::string &tag : tags) {
    if (!out.empty())
      out += ", ";
    out += "\"" + tag + "\"";
  }
  return out;
}

} // namespace

std::string windowsLocaleCandidate(std::string_view name) {
  // A sort order follows the locale after an underscore, which no locale
  // name contains otherwise.
  return std::string(name.substr(0, name.find('_')));
}

std::string localeFromCandidates(const std::vector<std::string> &candidates) {
  for (const std::string &raw : candidates) {
    if (auto tag = core::localeFromHost(raw))
      return std::move(*tag);
  }
  return "en-US";
}

std::vector<std::string> hostLocaleCandidates(const HostEnvironment &env) {
  std::vector<std::string> out;
#if defined(__APPLE__)
  (void)env;
  if (CFArrayRef languages = CFLocaleCopyPreferredLanguages()) {
    for (CFIndex i = 0, n = CFArrayGetCount(languages); i < n; ++i) {
      std::string language = toUtf8(
          static_cast<CFStringRef>(CFArrayGetValueAtIndex(languages, i)));
      if (!language.empty())
        out.push_back(std::move(language));
    }
    CFRelease(languages);
  }
#elif defined(_WIN32)
  // The user's regional format ("Region" in Settings), as ICU uses.
  (void)env;
  wchar_t name[LOCALE_NAME_MAX_LENGTH];
  if (::GetUserDefaultLocaleName(name, LOCALE_NAME_MAX_LENGTH) > 0) {
    std::string candidate = windowsLocaleCandidate(toUtf8(name));
    if (!candidate.empty())
      out.push_back(std::move(candidate));
  }
#elif defined(__ANDROID__)
  (void)env;
  for (const char *name : {"persist.sys.locale", "ro.product.locale"}) {
    std::string value = systemProperty(name);
    if (!value.empty())
      out.push_back(std::move(value));
  }
#else
  // POSIX precedence for LC_MESSAGES: LC_ALL overrides it, LANG is the
  // fallback.
  addEnv(out, env, "LC_ALL");
  addEnv(out, env, "LC_MESSAGES");
  addEnv(out, env, "LANG");
#endif
  return out;
}

std::vector<std::string> hostTimeZoneCandidates(const HostEnvironment &env) {
  std::vector<std::string> out;
#if defined(__APPLE__)
  (void)env;
  if (CFTimeZoneRef zone = CFTimeZoneCopySystem()) {
    std::string name = toUtf8(CFTimeZoneGetName(zone));
    if (!name.empty())
      out.push_back(std::move(name));
    CFRelease(zone);
  }
#elif defined(_WIN32)
  // The registry key name of the system zone, which CLDR's windowsZones
  // maps; StandardName is localized. The user's region picks a territory's
  // own row.
  (void)env;
  DYNAMIC_TIME_ZONE_INFORMATION info{};
  if (::GetDynamicTimeZoneInformation(&info) != TIME_ZONE_ID_INVALID) {
    // The key is NUL-terminated unless it fills the array.
    std::string key = toUtf8(
        info.TimeZoneKeyName,
        ::wcsnlen(info.TimeZoneKeyName, std::size(info.TimeZoneKeyName)));
    if (auto zone = core::timeZoneFromWindows(key, userRegion()))
      out.push_back(std::move(*zone));
  }
#elif defined(__ANDROID__)
  // bionic's own order (libc/tzcode/bionic.cpp): TZ, then the property.
  addTimeZoneEnv(out, env);
  std::string value = systemProperty("persist.sys.timezone");
  if (!value.empty())
    out.push_back(std::move(value));
#else
  addTimeZoneEnv(out, env);
  // Only a symlink into a zoneinfo directory names a zone; a copied file
  // does not. core::timeZoneFromHost checks this too.
  std::string link = readSymlink(env.localtimePath);
  if (link.find("zoneinfo/") != std::string::npos)
    out.push_back(std::move(link));
#endif
  return out;
}

std::string detectDefaultLocale(const HostEnvironment &env) {
  return localeFromCandidates(hostLocaleCandidates(env));
}

std::string detectDefaultTimeZone(const HostEnvironment &env) {
  for (const std::string &raw : hostTimeZoneCandidates(env)) {
    if (auto zone = core::timeZoneFromHost(raw))
      return std::move(*zone);
  }
  return "UTC";
}

std::string resolveDefaults(
    hermes_intl_config *config,
    const HostEnvironment &env) {
  // DefaultLocale must be available and have no Unicode extensions
  // (ECMA-402), so explicit locales get the reduction detected ones get.
  std::vector<std::string> locales;
  for (const std::string &tag : config->default_locales) {
    if (!core::canonicalizeLocale(tag))
      return "IntlDefaultLocales: \"" + tag +
          "\" is not a well-formed BCP 47 language tag";
    auto reduced = core::resolveDefaultLocale(tag);
    if (reduced &&
        std::find(locales.begin(), locales.end(), *reduced) == locales.end())
      locales.push_back(std::move(*reduced));
  }
  if (!config->default_locales.empty() && locales.empty())
    return "IntlDefaultLocales: none of " + quoteList(config->default_locales) +
        " is an available locale with data in its own script, even after "
        "dropping extensions and trailing subtags";
  if (locales.empty())
    locales.push_back(detectDefaultLocale(env));

  std::string timeZone;
  if (!config->default_time_zone.empty()) {
    auto zone = core::canonicalizeTimeZone(config->default_time_zone);
    if (!zone)
      return "IntlDefaultTimeZone: \"" + config->default_time_zone +
          "\" is not an IANA time zone the Intl plugin knows";
    timeZone = std::move(*zone);
  } else {
    timeZone = detectDefaultTimeZone(env);
  }

  config->default_locales = std::move(locales);
  config->default_time_zone = std::move(timeZone);
  return "";
}

std::string resolveDefaults(hermes_intl_config *config) {
  return resolveDefaults(config, HostEnvironment{});
}

} // namespace hermes_intl
