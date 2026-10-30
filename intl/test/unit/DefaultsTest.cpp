/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "Defaults.h"
#include "RustCore.h"

#include <gtest/gtest.h>

#include <cstdio>
#include <iostream>
#include <map>
#include <memory>
#include <string>
#include <utility>

#if defined(__linux__) && !defined(__ANDROID__)
// mkdtemp, symlink, unlink and rmdir, for LocaltimeFile.
#include <unistd.h>
#endif

using namespace hermes_intl;

namespace {

/// A HostEnvironment with the given variables and /etc/localtime path.
HostEnvironment fakeHost(
    std::map<std::string, std::string> vars,
    std::string localtime = "/nonexistent/localtime") {
  HostEnvironment env;
  auto shared =
      std::make_shared<std::map<std::string, std::string>>(std::move(vars));
  env.getEnv = [shared](const char *name) -> const char * {
    auto it = shared->find(name);
    return it == shared->end() ? nullptr : it->second.c_str();
  };
  env.localtimePath = std::move(localtime);
  return env;
}

#if defined(__linux__) && !defined(__ANDROID__)

TEST(DefaultsTest, LocaleSourcesInOrder) {
  auto env = fakeHost(
      {{"LC_ALL", "de_DE.UTF-8"},
       {"LC_MESSAGES", "fr_FR.UTF-8"},
       {"LANG", "it_IT.UTF-8"}});
  EXPECT_EQ(detectDefaultLocale(env), "de-DE");
  env = fakeHost({{"LC_MESSAGES", "fr_FR.UTF-8"}, {"LANG", "it_IT.UTF-8"}});
  EXPECT_EQ(detectDefaultLocale(env), "fr-FR");
  env = fakeHost({{"LANG", "it_IT.UTF-8"}});
  EXPECT_EQ(detectDefaultLocale(env), "it-IT");
}

TEST(DefaultsTest, UnusableLocaleSourcesAreSkipped) {
  auto env = fakeHost(
      {{"LC_ALL", ""}, {"LC_MESSAGES", "garbage!!"}, {"LANG", "sr_RS@latin"}});
  EXPECT_EQ(detectDefaultLocale(env), "sr-Latn-RS");
  // ff-SN is available, but the data has only Adlam ff, and ff-SN is
  // likely Latin.
  env = fakeHost({{"LC_ALL", "ff_SN.UTF-8"}, {"LANG", "de_DE.UTF-8"}});
  EXPECT_EQ(detectDefaultLocale(env), "de-DE");
}

TEST(DefaultsTest, LocaleFallsBackToEnUS) {
  EXPECT_EQ(detectDefaultLocale(fakeHost({})), "en-US");
  EXPECT_EQ(detectDefaultLocale(fakeHost({{"LANG", "C.UTF-8"}})), "en-US");
}

TEST(DefaultsTest, TimeZoneFromTZ) {
  EXPECT_EQ(
      detectDefaultTimeZone(fakeHost({{"TZ", ":Europe/Paris"}})),
      "Europe/Paris");
  EXPECT_EQ(
      detectDefaultTimeZone(
          fakeHost({{"TZ", "/usr/share/zoneinfo/Asia/Calcutta"}})),
      "Asia/Kolkata");
}

/// A temporary directory holding one entry named "localtime"; both are
/// removed when the fixture goes away. Tests must ASSERT_TRUE(ok()) before
/// using path().
class LocaltimeFile {
 public:
  /// A symlink to \p target, or a regular file when \p target is empty.
  explicit LocaltimeFile(const std::string &target);
  /// Remove the entry and the directory.
  ~LocaltimeFile();
  /// Whether the directory and the entry were both created.
  bool ok() const {
    return ok_;
  }
  /// The path of the "localtime" entry.
  const std::string &path() const {
    return path_;
  }

 private:
  /// The temporary directory; empty if it could not be created.
  std::string dir_;
  /// dir_ + "/localtime".
  std::string path_;
  /// See ok().
  bool ok_ = false;
};

LocaltimeFile::LocaltimeFile(const std::string &target) {
  std::string pattern = testing::internal::TempDir() + "intl-defaults-XXXXXX";
  if (!::mkdtemp(pattern.data()))
    return;
  dir_ = pattern;
  path_ = dir_ + "/localtime";
  if (target.empty()) {
    if (FILE *f = std::fopen(path_.c_str(), "w")) {
      std::fclose(f);
      ok_ = true;
    }
  } else {
    ok_ = ::symlink(target.c_str(), path_.c_str()) == 0;
  }
}

LocaltimeFile::~LocaltimeFile() {
  if (dir_.empty())
    return;
  ::unlink(path_.c_str());
  ::rmdir(dir_.c_str());
}

TEST(DefaultsTest, TimeZoneFromLocaltimeLink) {
  LocaltimeFile link("../usr/share/zoneinfo/America/New_York");
  ASSERT_TRUE(link.ok());
  EXPECT_EQ(
      detectDefaultTimeZone(fakeHost({}, link.path())), "America/New_York");
  // TZ comes first.
  EXPECT_EQ(
      detectDefaultTimeZone(fakeHost({{"TZ", "Asia/Tokyo"}}, link.path())),
      "Asia/Tokyo");
  // An unusable TZ (a POSIX rule) falls through to the link.
  EXPECT_EQ(
      detectDefaultTimeZone(
          fakeHost({{"TZ", "EST5EDT,M3.2.0,M11.1.0"}}, link.path())),
      "America/New_York");
}

TEST(DefaultsTest, LocaltimeOutsideZoneinfoIsIgnored) {
  LocaltimeFile link("/etc/somewhere/Europe/Paris");
  ASSERT_TRUE(link.ok());
  EXPECT_EQ(detectDefaultTimeZone(fakeHost({}, link.path())), "UTC");
}

TEST(DefaultsTest, LocaltimeCopyIsIgnored) {
  LocaltimeFile copy("");
  ASSERT_TRUE(copy.ok());
  EXPECT_EQ(detectDefaultTimeZone(fakeHost({}, copy.path())), "UTC");
}

TEST(DefaultsTest, EmptyTZIsUTC) {
  // glibc treats a set but empty TZ as UTC, so the link must not win.
  LocaltimeFile link("../usr/share/zoneinfo/America/New_York");
  ASSERT_TRUE(link.ok());
  EXPECT_EQ(detectDefaultTimeZone(fakeHost({{"TZ", ""}}, link.path())), "UTC");
  // Unset still falls through to the link.
  EXPECT_EQ(
      detectDefaultTimeZone(fakeHost({}, link.path())), "America/New_York");
}

/// The Review Focus inputs, through the C++ wiring, give what the Rust core's
/// host tables (intl/core/src/host.rs) give.
TEST(DefaultsTest, ReviewFocusHostValues) {
  const std::pair<const char *, const char *> locales[] = {
      {"sr_RS.UTF-8@latin", "sr-Latn-RS"},
      {"ca_ES@valencia", "ca-ES-valencia"},
      {"de_DE@euro", "de-DE"},
      {"zh-Hans-CN", "zh-Hans-CN"},
      {"sr_Latn_RS@latin", "sr-Latn-RS"},
      {"iw_IL", "he-IL"},
      {"C.UTF-8", "en-US"},
      {"en_US.ISO-8859-1", "en-US"},
      // Junk is skipped; with no other source the fallback is used.
      {"garbage!!", "en-US"},
      {".UTF-8", "en-US"},
  };
  for (const auto &[raw, want] : locales)
    EXPECT_EQ(detectDefaultLocale(fakeHost({{"LANG", raw}})), want) << raw;

  const std::pair<const char *, const char *> zones[] = {
      {":Europe/Paris", "Europe/Paris"},
      {"Etc/UTC", "UTC"},
      {"GMT", "UTC"},
      {"europe/paris", "Europe/Paris"},
      {"Asia/Calcutta", "Asia/Kolkata"},
      {"/var/db/timezone/zoneinfo/Europe/Berlin", "Europe/Berlin"},
      // A POSIX rule is skipped; with no other source the fallback is used.
      {"EST5EDT,M3.2.0,M11.1.0", "UTC"},
  };
  for (const auto &[raw, want] : zones)
    EXPECT_EQ(detectDefaultTimeZone(fakeHost({{"TZ", raw}})), want) << raw;
}

TEST(DefaultsTest, MissingValuesAreDetected) {
  hermes_intl_config config{{}, ""};
  auto env = fakeHost({{"LANG", "de_DE.UTF-8"}, {"TZ", "Europe/Berlin"}});
  EXPECT_EQ(resolveDefaults(&config, env), "");
  EXPECT_EQ(config.default_locales, (std::vector<std::string>{"de-DE"}));
  EXPECT_EQ(config.default_time_zone, "Europe/Berlin");
}

#endif // __linux__ && !__ANDROID__

TEST(DefaultsTest, ExplicitValuesAreCanonicalized) {
  hermes_intl_config config{{"EN-us", "iw"}, "asia/calcutta"};
  EXPECT_EQ(resolveDefaults(&config, fakeHost({})), "");
  EXPECT_EQ(config.default_locales, (std::vector<std::string>{"en-US", "he"}));
  EXPECT_EQ(config.default_time_zone, "Asia/Kolkata");
}

TEST(DefaultsTest, ExplicitValuesWinOverTheHost) {
  hermes_intl_config config{{"ja-JP"}, "Asia/Tokyo"};
  auto env = fakeHost({{"LANG", "de_DE.UTF-8"}, {"TZ", "Europe/Berlin"}});
  EXPECT_EQ(resolveDefaults(&config, env), "");
  EXPECT_EQ(config.default_locales, (std::vector<std::string>{"ja-JP"}));
  EXPECT_EQ(config.default_time_zone, "Asia/Tokyo");
}

TEST(DefaultsTest, InvalidExplicitValuesAreErrors) {
  hermes_intl_config badLocale{{"en_US"}, ""};
  EXPECT_NE(
      resolveDefaults(&badLocale, fakeHost({})).find("IntlDefaultLocales"),
      std::string::npos);
  hermes_intl_config badZone{{}, "Mars/Olympus_Mons"};
  EXPECT_NE(
      resolveDefaults(&badZone, fakeHost({})).find("IntlDefaultTimeZone"),
      std::string::npos);
}

// DefaultLocale must be available and have no Unicode extensions
// (ECMA-402), so explicit locales are reduced the way detected ones are.

TEST(DefaultsTest, ExplicitLocaleExtensionsAreDropped) {
  hermes_intl_config config{{"en-US-u-ca-buddhist"}, ""};
  EXPECT_EQ(resolveDefaults(&config, fakeHost({})), "");
  EXPECT_EQ(config.default_locales, (std::vector<std::string>{"en-US"}));
}

TEST(DefaultsTest, ExplicitLocaleWithLongTlangIsReduced) {
  // ICU4X does not parse the whole tag ("posix" has 5 letters); the default
  // depends only on its language identifier.
  hermes_intl_config config{{"en-US-t-posix"}, ""};
  EXPECT_EQ(resolveDefaults(&config, fakeHost({})), "");
  EXPECT_EQ(config.default_locales, (std::vector<std::string>{"en-US"}));
}

TEST(DefaultsTest, ExplicitAvailableLocaleIsKept) {
  hermes_intl_config config{{"zh-TW"}, ""};
  EXPECT_EQ(resolveDefaults(&config, fakeHost({})), "");
  EXPECT_EQ(config.default_locales, (std::vector<std::string>{"zh-TW"}));
}

TEST(DefaultsTest, UnavailableExplicitLocaleIsDropped) {
  hermes_intl_config config{{"tlh", "fr-FR"}, ""};
  EXPECT_EQ(resolveDefaults(&config, fakeHost({})), "");
  EXPECT_EQ(config.default_locales, (std::vector<std::string>{"fr-FR"}));
}

TEST(DefaultsTest, ExplicitLocaleKeepsItsScript) {
  // tt-Latn-RU is not available, and its prefix "tt" is Cyrillic.
  hermes_intl_config config{{"tt-Latn-RU", "ru"}, ""};
  EXPECT_EQ(resolveDefaults(&config, fakeHost({})), "");
  EXPECT_EQ(config.default_locales, (std::vector<std::string>{"ru"}));
}

TEST(DefaultsTest, ExplicitLocaleNeedsDataInItsScript) {
  // ff, ff-BF and bal are available, but the data has only Adlam ff and
  // Latin bal, and they are likely Latin and Arabic.
  hermes_intl_config config{{"ff", "ff-BF", "bal", "ff-Adlm-BF"}, ""};
  EXPECT_EQ(resolveDefaults(&config, fakeHost({})), "");
  EXPECT_EQ(config.default_locales, (std::vector<std::string>{"ff-Adlm-BF"}));
}

TEST(DefaultsTest, NoAvailableExplicitLocaleIsAnError) {
  hermes_intl_config config{{"tlh", "tlh-US"}, ""};
  std::string error = resolveDefaults(&config, fakeHost({{"LANG", "de_DE"}}));
  EXPECT_NE(error.find("IntlDefaultLocales"), std::string::npos) << error;
  EXPECT_NE(error.find("available"), std::string::npos) << error;
}

TEST(DefaultsTest, IllFormedExplicitLocaleIsAnError) {
  // Even after usable and unavailable elements.
  hermes_intl_config config{{"en-US", "tlh", "de_DE"}, ""};
  std::string error = resolveDefaults(&config, fakeHost({}));
  EXPECT_NE(error.find("IntlDefaultLocales"), std::string::npos) << error;
  EXPECT_NE(error.find("\"de_DE\""), std::string::npos) << error;
}

TEST(DefaultsTest, ExplicitLocaleDuplicatesAreCollapsed) {
  hermes_intl_config config{
      {"en-US-u-ca-buddhist", "fr", "EN-us", "en-US-x-foo", "en-AQ", "en"}, ""};
  EXPECT_EQ(resolveDefaults(&config, fakeHost({})), "");
  EXPECT_EQ(
      config.default_locales, (std::vector<std::string>{"en-US", "fr", "en"}));
}

// The pure helpers of the Windows branch, which run on every host.

TEST(DefaultsTest, WindowsLocaleNameLosesItsSortSuffix) {
  EXPECT_EQ(windowsLocaleCandidate("de-DE_phoneb"), "de-DE");
  EXPECT_EQ(windowsLocaleCandidate("es-ES_tradnl"), "es-ES");
  EXPECT_EQ(windowsLocaleCandidate("zh-CN_stroke"), "zh-CN");
  EXPECT_EQ(windowsLocaleCandidate("sr-Latn-RS"), "sr-Latn-RS");
  EXPECT_EQ(windowsLocaleCandidate(""), "");
  EXPECT_EQ(
      localeFromCandidates({windowsLocaleCandidate("de-DE_phoneb")}), "de-DE");
  EXPECT_EQ(
      localeFromCandidates({windowsLocaleCandidate("sr-Latn-RS")}),
      "sr-Latn-RS");
}

TEST(DefaultsTest, WindowsPseudoLocaleFallsBackToEnUS) {
  for (const char *name : {"qps-ploc", "qps-ploca", "qps-plocm"}) {
    EXPECT_EQ(windowsLocaleCandidate(name), name);
    EXPECT_EQ(localeFromCandidates({windowsLocaleCandidate(name)}), "en-US")
        << name;
  }
}

TEST(DefaultsTest, LocaleFromCandidatesTakesTheFirstUsable) {
  EXPECT_EQ(localeFromCandidates({"garbage!!", "fr_FR", "de_DE"}), "fr-FR");
  EXPECT_EQ(localeFromCandidates({}), "en-US");
}

TEST(DefaultsTest, WindowsTimeZoneKeys) {
  EXPECT_EQ(
      core::timeZoneFromWindows("Pacific Standard Time", "").value_or(""),
      "America/Los_Angeles");
  EXPECT_EQ(
      core::timeZoneFromWindows("Eastern Standard Time", "CA").value_or(""),
      "America/Toronto");
  EXPECT_EQ(
      core::timeZoneFromWindows("India Standard Time", "US").value_or(""),
      "Asia/Kolkata");
  EXPECT_EQ(core::timeZoneFromWindows("UTC", "").value_or(""), "UTC");
  EXPECT_FALSE(core::timeZoneFromWindows("Mars Standard Time", ""));
}

/// Detection against the real host, with a default-constructed
/// HostEnvironment (no fakes). The values depend on the machine, so only
/// their shape is checked: the locale must already be canonical,
/// extension-free and available, the time zone a primary IANA id. They are
/// printed so that a person can compare them with the machine's settings.
TEST(DefaultsTest, RealHostDetectionIsUsable) {
  HostEnvironment env;
  std::string locale = detectDefaultLocale(env);
  std::string tz = detectDefaultTimeZone(env);
  std::cout << "detected default locale: " << locale << "\n"
            << "detected default time zone: " << tz << std::endl;
  RecordProperty("detected_locale", locale);
  RecordProperty("detected_time_zone", tz);

  ASSERT_FALSE(locale.empty());
  EXPECT_EQ(core::resolveDefaultLocale(locale).value_or("<none>"), locale);
  ASSERT_FALSE(tz.empty());
  EXPECT_EQ(core::canonicalizeTimeZone(tz).value_or("<none>"), tz);

  hermes_intl_config config{{}, ""};
  EXPECT_EQ(resolveDefaults(&config), "");
  EXPECT_FALSE(config.default_locales.empty());
  EXPECT_EQ(config.default_time_zone, tz);
  if (!config.default_locales.empty())
    EXPECT_EQ(config.default_locales[0], locale);
}

} // namespace
