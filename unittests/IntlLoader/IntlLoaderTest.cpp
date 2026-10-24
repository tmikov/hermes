/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// Every test that registers a plugin or resolves the default plugin runs in a
// child process (EXPECT_EXIT / EXPECT_DEATH), because both are once per
// process.

#include "hermes/BCGen/HBC/BytecodeVersion.h"
#include "hermes/IntlPluginLoader.h"
#include "hermes/Public/IntlPlugin.h"
#include "hermes/hermes.h"

#include <jsi/threadsafe.h>

#include <gtest/gtest.h>

#include <atomic>
#include <cstdlib>
#include <functional>
#include <map>
#include <stdexcept>
#include <thread>
#include <vector>

using namespace facebook;
using namespace facebook::hermes;
using ::hermes::vm::RuntimeConfig;

namespace {

/// How many times a fake plugin's resolve_defaults ran in this process.
std::atomic<int> resolveCalls{0};
/// How many times a fake plugin's install ran in this process.
std::atomic<int> installCalls{0};

/// Fill in missing defaults the way a real plugin's detection would.
std::string fakeResolve(hermes_intl_config *config) {
  ++resolveCalls;
  if (config->default_locales.empty())
    config->default_locales.push_back("xx-detected");
  if (config->default_time_zone.empty())
    config->default_time_zone = "Fake/Detected";
  return "";
}

/// Record the first default locale where JS can see it.
void fakeInstall(jsi::Runtime &rt, const hermes_intl_config *config) {
  ++installCalls;
  rt.global().setProperty(
      rt,
      "__fakeIntlLocale",
      jsi::String::createFromUtf8(rt, config->default_locales.at(0)));
}

/// A resolve_defaults that rejects whatever it is given.
std::string rejectingResolve(hermes_intl_config *) {
  return "fake rejection";
}

/// A resolve_defaults that throws.
std::string throwingResolve(hermes_intl_config *) {
  throw std::runtime_error("fake resolver failure");
}

/// An install that throws, as a plugin whose setup JS failed would.
void throwingInstall(jsi::Runtime &, const hermes_intl_config *) {
  throw jsi::JSINativeException("fake setup failure");
}

/// The flags of a plugin built like this Hermes.
uint32_t hostFlags() {
#ifdef JSI_UNSTABLE
  return HERMES_INTL_FLAG_JSI_UNSTABLE;
#else
  return 0;
#endif
}

/// A valid entry; tests break one field at a time.
hermes_intl_entry goodEntry() {
  return hermes_intl_entry{
      sizeof(hermes_intl_entry),
      HERMES_INTL_PLUGIN_INTERFACE_VERSION,
      ::hermes::hbc::BYTECODE_VERSION,
      hostFlags(),
      fakeResolve,
      fakeInstall};
}

/// The entry registered by the current child process.
hermes_intl_entry registered;

/// Register \p entry for the rest of this process.
void registerEntry(const hermes_intl_entry &entry) {
  registered = entry;
  registerIntlPlugin(&registered);
}

/// The value of __fakeIntlLocale, or "" if the plugin was not installed.
std::string fakeLocale(jsi::Runtime &rt) {
  auto v = rt.global().getProperty(rt, "__fakeIntlLocale");
  return v.isString() ? v.getString(rt).utf8(rt) : "";
}

/// Runs death tests in the "threadsafe" style, which re-executes the test
/// binary, because the tests create threads (the finalizer, workers).
class IntlLoaderTest : public ::testing::Test {
 protected:
  void SetUp() override {
    ::testing::FLAGS_gtest_death_test_style = "threadsafe";
  }
};

/// Child: a registered plugin is installed with its detected defaults.
void registeredPluginIsInstalled() {
  registerEntry(goodEntry());
  auto rt = makeHermesRuntime();
  bool ok = fakeLocale(*rt) == "xx-detected";
  // Destroy the runtime first: LeakSanitizer reports it at exit otherwise.
  rt.reset();
  std::exit(ok ? 0 : 1);
}
TEST_F(IntlLoaderTest, RegisteredPluginIsInstalled) {
  EXPECT_EXIT(registeredPluginIsInstalled(), ::testing::ExitedWithCode(0), "");
}

/// Child: RuntimeConfig's defaults are what the plugin receives.
void explicitDefaultsReachThePlugin() {
  registerEntry(goodEntry());
  auto rt = makeHermesRuntime(RuntimeConfig::Builder()
                                  .withIntlDefaultLocales({"fr-FR", "de"})
                                  .withIntlDefaultTimeZone("Europe/Paris")
                                  .build());
  bool ok = fakeLocale(*rt) == "fr-FR";
  // Destroy the runtime first: LeakSanitizer reports it at exit otherwise.
  rt.reset();
  std::exit(ok ? 0 : 1);
}
TEST_F(IntlLoaderTest, ExplicitDefaultsReachThePlugin) {
  EXPECT_EXIT(
      explicitDefaultsReachThePlugin(), ::testing::ExitedWithCode(0), "");
}

/// Child: EnableIntlPlugin=false installs nothing.
void disabledSkipsTheRegisteredPlugin() {
  registerEntry(goodEntry());
  auto rt = makeHermesRuntime(
      RuntimeConfig::Builder().withEnableIntlPlugin(false).build());
  bool ok = fakeLocale(*rt).empty() && installCalls == 0;
  // Destroy the runtime first: LeakSanitizer reports it at exit otherwise.
  rt.reset();
  std::exit(ok ? 0 : 1);
}
TEST_F(IntlLoaderTest, DisabledSkipsTheRegisteredPlugin) {
  EXPECT_EXIT(
      disabledSkipsTheRegisteredPlugin(), ::testing::ExitedWithCode(0), "");
}

/// Child: makeThreadSafeHermesRuntime installs the plugin too.
void threadSafeFactoryInstallsThePlugin() {
  registerEntry(goodEntry());
  auto rt = makeThreadSafeHermesRuntime();
  bool ok = fakeLocale(*rt) == "xx-detected";
  // Destroy the runtime first: LeakSanitizer reports it at exit otherwise.
  rt.reset();
  std::exit(ok ? 0 : 1);
}
TEST_F(IntlLoaderTest, ThreadSafeFactoryInstallsThePlugin) {
  EXPECT_EXIT(
      threadSafeFactoryInstallsThePlugin(), ::testing::ExitedWithCode(0), "");
}

/// Child: runtimes created at once on several threads resolve the default
/// plugin exactly once, and every one of them gets it.
void concurrentRuntimesShareOneResolution() {
  registerEntry(goodEntry());
  constexpr int kThreads = 8;
  std::atomic<int> installed{0};
  std::atomic<int> ready{0};
  std::vector<std::thread> threads;
  for (int i = 0; i < kThreads; ++i) {
    threads.emplace_back([&installed, &ready] {
      // Start creating runtimes together, so resolution races if it can.
      ++ready;
      while (ready < kThreads) {
      }
      auto rt = makeHermesRuntime();
      if (fakeLocale(*rt) == "xx-detected")
        ++installed;
    });
  }
  for (auto &t : threads)
    t.join();
  std::exit(
      installed == kThreads && installCalls == kThreads &&
              intlDefaultResolutionsForTests() == 1
          ? 0
          : 1);
}
TEST_F(IntlLoaderTest, ConcurrentRuntimesShareOneResolution) {
  EXPECT_EXIT(
      concurrentRuntimesShareOneResolution(), ::testing::ExitedWithCode(0), "");
}

/// Child: register a plugin with the wrong interface version.
void wrongInterfaceVersion() {
  auto entry = goodEntry();
  entry.plugin_interface_version = HERMES_INTL_PLUGIN_INTERFACE_VERSION + 1;
  registerEntry(entry);
  makeHermesRuntime();
}
TEST_F(IntlLoaderTest, WrongInterfaceVersionIsFatal) {
  EXPECT_DEATH(
      wrongInterfaceVersion(),
      "Intl plugin registered with registerIntlPlugin: plugin interface "
      "version [0-9]+, Hermes expects [0-9]+");
}

/// Child: register a plugin with the wrong bytecode version.
void wrongBytecodeVersion() {
  auto entry = goodEntry();
  entry.bytecode_version = ::hermes::hbc::BYTECODE_VERSION + 1;
  registerEntry(entry);
  makeHermesRuntime();
}
TEST_F(IntlLoaderTest, WrongBytecodeVersionIsFatal) {
  EXPECT_DEATH(
      wrongBytecodeVersion(),
      "Intl plugin registered with registerIntlPlugin: bytecode version");
}

/// Child: register a plugin built with the other JSI_UNSTABLE setting.
void wrongFlags() {
  auto entry = goodEntry();
  entry.flags ^= HERMES_INTL_FLAG_JSI_UNSTABLE;
  registerEntry(entry);
  makeHermesRuntime();
}
TEST_F(IntlLoaderTest, WrongFlagsAreFatal) {
  EXPECT_DEATH(
      wrongFlags(),
      "Intl plugin registered with registerIntlPlugin: built with "
      "JSI_UNSTABLE (on, Hermes with it off|off, Hermes with it on) "
      "\\(flags 0x[01], Hermes expects 0x[01]\\)");
}

/// Child: register a plugin with a flag bit this Hermes does not know.
void unknownFlag() {
  auto entry = goodEntry();
  entry.flags |= 0x100;
  registerEntry(entry);
  makeHermesRuntime();
}
TEST_F(IntlLoaderTest, UnknownFlagIsFatal) {
  // Only the unknown bit differs, so the message must not mention
  // JSI_UNSTABLE.
  EXPECT_DEATH(
      unknownFlag(),
      "Intl plugin registered with registerIntlPlugin: flags 0x10[01], "
      "Hermes expects 0x[01]");
}

/// Child: register a plugin whose entry struct is too small.
void smallStruct() {
  auto entry = goodEntry();
  entry.struct_size = 4;
  registerEntry(entry);
  makeHermesRuntime();
}
TEST_F(IntlLoaderTest, SmallEntryStructIsFatal) {
  EXPECT_DEATH(smallStruct(), "entry struct is too small");
}

/// Child: register a plugin with no resolve_defaults.
void nullResolve() {
  auto entry = goodEntry();
  entry.resolve_defaults = nullptr;
  registerEntry(entry);
  makeHermesRuntime();
}
TEST_F(IntlLoaderTest, NullResolveIsFatal) {
  EXPECT_DEATH(
      nullResolve(),
      "Intl plugin registered with registerIntlPlugin: entry has a null "
      "function");
}

/// Child: register a plugin with no install.
void nullInstall() {
  auto entry = goodEntry();
  entry.install = nullptr;
  registerEntry(entry);
  makeHermesRuntime();
}
TEST_F(IntlLoaderTest, NullInstallIsFatal) {
  EXPECT_DEATH(
      nullInstall(),
      "Intl plugin registered with registerIntlPlugin: entry has a null "
      "function");
}

/// Child: register a plugin that rejects the defaults.
void rejectedDefaults() {
  auto entry = goodEntry();
  entry.resolve_defaults = rejectingResolve;
  registerEntry(entry);
  makeHermesRuntime();
}
TEST_F(IntlLoaderTest, RejectedDefaultsAreFatal) {
  EXPECT_DEATH(
      rejectedDefaults(),
      "rejected the default locale or time zone: fake rejection");
}

/// Child: register a plugin whose resolve_defaults throws.
void throwingResolver() {
  auto entry = goodEntry();
  entry.resolve_defaults = throwingResolve;
  registerEntry(entry);
  makeHermesRuntime();
}
TEST_F(IntlLoaderTest, ThrowingResolverIsFatal) {
  EXPECT_DEATH(
      throwingResolver(),
      "Intl plugin registered with registerIntlPlugin: default resolution "
      "failed: fake resolver failure");
}

/// Child: register a plugin whose install throws.
void throwingSetup() {
  auto entry = goodEntry();
  entry.install = throwingInstall;
  registerEntry(entry);
  makeHermesRuntime();
}
TEST_F(IntlLoaderTest, ThrowingSetupIsFatal) {
  EXPECT_DEATH(throwingSetup(), "setup failed: fake setup failure");
}

/// Child: register a plugin after the default plugin was resolved.
void registerAfterResolution() {
  makeHermesRuntime();
  registerEntry(goodEntry());
}
TEST_F(IntlLoaderTest, RegisterAfterResolutionIsFatal) {
  EXPECT_DEATH(
      registerAfterResolution(),
      "registerIntlPlugin was called after the default plugin was resolved");
}

/// Child: a Worker's config copies the parent's effective Intl state.
void workerConfigCopiesEffectiveDefaults() {
  registerEntry(goodEntry());
  auto parent = makeHermesRuntime();
  RuntimeConfig config = makeWorkerRuntimeConfig(*parent);
  bool ok = config.getEnableIntlPlugin() && config.getIntl() &&
      config.getIntlPluginPath().empty() &&
      config.getIntlDefaultLocales() ==
          std::vector<std::string>{"xx-detected"} &&
      config.getIntlDefaultTimeZone() == "Fake/Detected";
  // A worker's worker gets the same.
  auto worker = makeHermesRuntime(config);
  RuntimeConfig nested = makeWorkerRuntimeConfig(*worker);
  ok = ok && fakeLocale(*worker) == "xx-detected" &&
      nested.getIntlDefaultLocales() == config.getIntlDefaultLocales() &&
      nested.getIntlDefaultTimeZone() == config.getIntlDefaultTimeZone();
  worker.reset();
  parent.reset();
  std::exit(ok ? 0 : 1);
}
TEST_F(IntlLoaderTest, WorkerConfigCopiesEffectiveDefaults) {
  EXPECT_EXIT(
      workerConfigCopiesEffectiveDefaults(), ::testing::ExitedWithCode(0), "");
}

/// Child: a parent without the plugin has Workers without it.
void workerConfigCopiesDisabledState() {
  registerEntry(goodEntry());
  auto parent = makeHermesRuntime(RuntimeConfig::Builder()
                                      .withEnableIntlPlugin(false)
                                      .withIntl(false)
                                      .build());
  RuntimeConfig config = makeWorkerRuntimeConfig(*parent);
  bool ok = !config.getEnableIntlPlugin() && !config.getIntl();
  parent.reset();
  std::exit(ok ? 0 : 1);
}
TEST_F(IntlLoaderTest, WorkerConfigCopiesDisabledState) {
  EXPECT_EXIT(
      workerConfigCopiesDisabledState(), ::testing::ExitedWithCode(0), "");
}

/// A fake stat over \p files: path to its id.
std::function<MappedFileId(const std::string &)> fakeStat(
    std::map<std::string, MappedFileId> files) {
  return [files](const std::string &path) {
    auto it = files.find(path);
    return it == files.end() ? MappedFileId{} : it->second;
  };
}

/// A /proc/self/maps line for device 08:02, inode 1234, and \p path.
std::string mapsLine(const std::string &path) {
  return "7f0000000000-7f0000100000 r-xp 00000000 08:02 1234"
         "                       " +
      path + "\n";
}

/// The id the mapping of mapsLine has, and one it does not.
const MappedFileId kMapped{true, 8, 2, 1234};
const MappedFileId kOther{true, 0, 0x2a, 99};

TEST(IntlMapsLineTest, PlainPathIsUsedAsWritten) {
  std::string path = "/usr/lib/libhermesvm.so";
  EXPECT_EQ(
      pathFromMapsLine(mapsLine(path), fakeStat({{path, kMapped}})), path);
  // On overlayfs and btrfs subvolumes stat reports another device and inode
  // than the mapping: the path is still the file's.
  EXPECT_EQ(pathFromMapsLine(mapsLine(path), fakeStat({{path, kOther}})), path);
  // Escapes other than "\012" do not exist: "\141" is four characters.
  std::string literal = "/opt/lib\\141/libhermesvm.so";
  EXPECT_EQ(
      pathFromMapsLine(
          mapsLine(literal),
          fakeStat({{literal, kOther}, {"/opt/liba/libhermesvm.so", kMapped}})),
      literal);
}

TEST(IntlMapsLineTest, MissingPathFallsBack) {
  std::string path = "/usr/lib/libhermesvm.so";
  EXPECT_EQ(pathFromMapsLine(mapsLine(path), fakeStat({})), "");
}

TEST(IntlMapsLineTest, DeletedPathNeedsAnIdentityMatch) {
  std::string path = "/usr/lib/libhermesvm.so";
  std::string marked = path + " (deleted)";
  // A removed file, and an unrelated file at its old name: fall back.
  EXPECT_EQ(pathFromMapsLine(mapsLine(marked), fakeStat({{path, kOther}})), "");
  // An unrelated file whose name really ends in " (deleted)": fall back.
  EXPECT_EQ(
      pathFromMapsLine(mapsLine(marked), fakeStat({{marked, kOther}})), "");
  // The file itself, under either reading.
  EXPECT_EQ(
      pathFromMapsLine(mapsLine(marked), fakeStat({{marked, kMapped}})),
      marked);
  EXPECT_EQ(
      pathFromMapsLine(mapsLine(marked), fakeStat({{path, kMapped}})), path);
}

TEST(IntlMapsLineTest, NewlineEscapeNeedsAnIdentityMatch) {
  std::string raw = "/opt/x\\012y/libhermesvm.so";
  std::string decoded = "/opt/x\ny/libhermesvm.so";
  // The identity decides.
  EXPECT_EQ(
      pathFromMapsLine(
          mapsLine(raw), fakeStat({{raw, kMapped}, {decoded, kOther}})),
      raw);
  EXPECT_EQ(
      pathFromMapsLine(
          mapsLine(raw), fakeStat({{raw, kOther}, {decoded, kMapped}})),
      decoded);
  // Without an identity match, a reading that merely exists is not taken.
  EXPECT_EQ(pathFromMapsLine(mapsLine(raw), fakeStat({{decoded, kOther}})), "");
  EXPECT_EQ(pathFromMapsLine(mapsLine(raw), fakeStat({{raw, kOther}})), "");
  EXPECT_EQ(
      pathFromMapsLine(
          mapsLine(raw), fakeStat({{raw, kOther}, {decoded, kOther}})),
      "");
  EXPECT_EQ(pathFromMapsLine(mapsLine(raw), fakeStat({})), "");
}

TEST(IntlMapsLineTest, MixedNewlineReadingFallsBack) {
  // Two "\012"s: the real file is at a mixed reading (one newline, one four
  // characters), which is never tried, and an unrelated file is at the
  // reading as written. Fall back.
  std::string raw = "/opt/a\\012b\\012c/libhermesvm.so";
  std::string mixed = "/opt/a\nb\\012c/libhermesvm.so";
  EXPECT_EQ(
      pathFromMapsLine(
          mapsLine(raw), fakeStat({{mixed, kMapped}, {raw, kOther}})),
      "");
}

TEST(IntlMapsLineTest, LinesWithoutAFileGiveNothing) {
  auto all = [](const std::string &) { return kMapped; };
  EXPECT_EQ(
      pathFromMapsLine(
          "7f0000000000-7f0000100000 rw-p 00000000 00:00 0 \n", all),
      "");
  EXPECT_EQ(
      pathFromMapsLine(
          "7ffd00000000-7ffd00021000 rw-p 00000000 00:00 0  [stack]\n", all),
      "");
  EXPECT_EQ(pathFromMapsLine("garbage\n", all), "");
}

} // namespace
