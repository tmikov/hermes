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
#else
#include <unistd.h>
#include <climits>
#endif

namespace hermes_intl {
namespace {

#if !defined(__APPLE__)
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

#if !defined(__APPLE__) && !defined(__ANDROID__)
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
  for (const std::string &raw : hostLocaleCandidates(env)) {
    if (auto tag = core::localeFromHost(raw))
      return std::move(*tag);
  }
  return "en-US";
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
