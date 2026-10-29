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
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <functional>
#include <initializer_list>
#include <map>
#include <stdexcept>
#include <string>
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
  return HERMES_INTL_BUILD_FLAGS;
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

/// \p s with the regex metacharacters of the messages below escaped. The
/// death tests match exact messages: gtest's regular expressions are POSIX
/// extended ones on Linux and Apple platforms but its own simple ones on
/// Windows, which have no brackets or alternation; escaped literals mean
/// the same in both.
std::string escapeRegex(const std::string &s) {
  std::string out;
  for (char c : s) {
    if (std::strchr("().", c))
      out += '\\';
    out += c;
  }
  return out;
}

/// "0x<hex>" for \p flags.
std::string hex(uint32_t flags) {
  char buf[16];
  std::snprintf(buf, sizeof(buf), "0x%x", (unsigned)flags);
  return buf;
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
      "version " +
          std::to_string(HERMES_INTL_PLUGIN_INTERFACE_VERSION + 1) +
          ", Hermes expects " +
          std::to_string(HERMES_INTL_PLUGIN_INTERFACE_VERSION));
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
  const uint32_t host = hostFlags();
  const bool hostUnstable = host & HERMES_INTL_FLAG_JSI_UNSTABLE;
  EXPECT_DEATH(
      wrongFlags(),
      escapeRegex(
          std::string("Intl plugin registered with registerIntlPlugin: "
                      "built with JSI_UNSTABLE ") +
          (hostUnstable ? "off, Hermes with it on" : "on, Hermes with it off") +
          " (flags " + hex(host ^ HERMES_INTL_FLAG_JSI_UNSTABLE) +
          ", Hermes expects " + hex(host) + ")"));
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
      "Intl plugin registered with registerIntlPlugin: flags " +
          hex(hostFlags() | 0x100) + ", Hermes expects " + hex(hostFlags()));
}

/// Child: register a plugin whose flags are \p flags.
void flagsAre(uint32_t flags) {
  auto entry = goodEntry();
  entry.flags = flags;
  registerEntry(entry);
  makeHermesRuntime();
}

TEST_F(IntlLoaderTest, AbiBitMismatchesAreFatal) {
  // Every pattern of the MSVC ABI bits other than this build's, which on
  // other standard libraries is 0: each is rejected with a message naming
  // the plugin's value and Hermes's.
  constexpr uint32_t abiBits =
      HERMES_INTL_FLAG_ITERATOR_DEBUG_LEVEL_MASK | HERMES_INTL_FLAG_DEBUG_CRT;
  const uint32_t host = hostFlags();
  const unsigned hostLevel =
      (host & HERMES_INTL_FLAG_ITERATOR_DEBUG_LEVEL_MASK) >>
      HERMES_INTL_FLAG_ITERATOR_DEBUG_LEVEL_SHIFT;
  const bool hostDebugCrt = host & HERMES_INTL_FLAG_DEBUG_CRT;
  auto crt = [](bool debug) {
    return std::string(
        debug ? "the debug C runtime (/MDd)" : "the release C runtime (/MD)");
  };
  int checked = 0;
  for (unsigned level = 0; level < 4; ++level) {
    for (bool debugCrt : {false, true}) {
      uint32_t flags = (host & ~abiBits) |
          (level << HERMES_INTL_FLAG_ITERATOR_DEBUG_LEVEL_SHIFT) |
          (debugCrt ? HERMES_INTL_FLAG_DEBUG_CRT : 0);
      if (flags == host)
        continue;
      std::string why;
      if (level != hostLevel)
        why = "built with _ITERATOR_DEBUG_LEVEL " + std::to_string(level) +
            ", Hermes with " + std::to_string(hostLevel);
      if (debugCrt != hostDebugCrt)
        why += (why.empty() ? "" : "; ") + std::string("built with ") +
            crt(debugCrt) + ", Hermes with " + crt(hostDebugCrt);
      std::string message =
          "Intl plugin registered with registerIntlPlugin: " + why +
          " (flags " + hex(flags) + ", Hermes expects " + hex(host) + ")";
      EXPECT_DEATH(flagsAre(flags), escapeRegex(message)) << message;
      ++checked;
    }
  }
  EXPECT_EQ(checked, 7);
}

#if !defined(_MSC_VER)
TEST_F(IntlLoaderTest, AbiBitsAreZeroOffMsvc) {
  EXPECT_EQ(
      hostFlags() &
          (HERMES_INTL_FLAG_ITERATOR_DEBUG_LEVEL_MASK |
           HERMES_INTL_FLAG_DEBUG_CRT),
      0u);
}
#endif

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
  bool ok = config.getEnableIntlPlugin() &&
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
  auto parent = makeHermesRuntime(
      RuntimeConfig::Builder().withEnableIntlPlugin(false).build());
  RuntimeConfig config = makeWorkerRuntimeConfig(*parent);
  bool ok = !config.getEnableIntlPlugin();
  parent.reset();
  std::exit(ok ? 0 : 1);
}
TEST_F(IntlLoaderTest, WorkerConfigCopiesDisabledState) {
  EXPECT_EXIT(
      workerConfigCopiesDisabledState(), ::testing::ExitedWithCode(0), "");
}

/// Child: RuntimeConfig's Intl field is deprecated and ignored. With it true
/// or false, a runtime and a Worker created from it have the plugin when it
/// is enabled and nothing when it is disabled; neither has an Intl global,
/// since the fake plugin defines none and Hermes no longer has its own.
void intlFieldIsIgnored() {
  registerEntry(goodEntry());
  bool ok = true;
  for (bool intl : {true, false}) {
    for (bool plugin : {true, false}) {
      auto parent = makeHermesRuntime(RuntimeConfig::Builder()
                                          .withIntl(intl)
                                          .withEnableIntlPlugin(plugin)
                                          .build());
      auto worker = makeHermesRuntime(makeWorkerRuntimeConfig(*parent));
      std::string want = plugin ? "xx-detected" : "";
      for (jsi::Runtime *rt :
           std::initializer_list<jsi::Runtime *>{parent.get(), worker.get()}) {
        if (fakeLocale(*rt) != want || rt->global().hasProperty(*rt, "Intl")) {
          std::fprintf(
              stderr,
              "Intl=%d plugin=%d %s: plugin locale \"%s\", Intl %s\n",
              intl,
              plugin,
              rt == parent.get() ? "parent" : "worker",
              fakeLocale(*rt).c_str(),
              rt->global().hasProperty(*rt, "Intl") ? "defined" : "undefined");
          ok = false;
        }
      }
      // Destroy the runtimes first: LeakSanitizer reports them otherwise.
      worker.reset();
      parent.reset();
    }
  }
  std::exit(ok ? 0 : 1);
}
TEST_F(IntlLoaderTest, IntlFieldIsIgnored) {
  EXPECT_EXIT(intlFieldIsIgnored(), ::testing::ExitedWithCode(0), "");
}

/// Set while markingRunner runs its function, on that thread only.
thread_local bool inMarkingRunner = false;

/// A finalizer thread runner that tests can recognize: the function it runs
/// sees inMarkingRunner set.
void markingRunner(std::function<void()> run) {
  bool previous = inMarkingRunner;
  inMarkingRunner = true;
  run();
  inMarkingRunner = previous;
}

/// Whether \p config has a finalizer thread runner and it is markingRunner.
bool hasMarkingRunner(const RuntimeConfig &config) {
  auto runner = config.getFinalizerThreadRunner();
  if (!runner || !*runner)
    return false;
  bool marked = false;
  (*runner)([&marked] { marked = inMarkingRunner; });
  return marked;
}

/// Child: a Worker's config carries the parent's finalizer thread runner,
/// with and without the plugin, so a Worker never falls back to the platform
/// default (the JNI one on Android) when its parent did not use it. A parent
/// without a runner gives Workers none either.
void workerConfigCopiesFinalizerThreadRunner() {
  registerEntry(goodEntry());
  auto withPlugin =
      makeHermesRuntime(RuntimeConfig::Builder()
                            .withFinalizerThreadRunner(markingRunner)
                            .build());
  RuntimeConfig config = makeWorkerRuntimeConfig(*withPlugin);
  bool ok = config.getEnableIntlPlugin() && hasMarkingRunner(config);
  // A worker's worker gets it too.
  auto worker = makeHermesRuntime(config);
  ok = ok && hasMarkingRunner(makeWorkerRuntimeConfig(*worker));

  auto withoutPlugin =
      makeHermesRuntime(RuntimeConfig::Builder()
                            .withEnableIntlPlugin(false)
                            .withFinalizerThreadRunner(markingRunner)
                            .build());
  RuntimeConfig noPlugin = makeWorkerRuntimeConfig(*withoutPlugin);
  ok = ok && !noPlugin.getEnableIntlPlugin() && hasMarkingRunner(noPlugin);

  auto unset = makeHermesRuntime();
  ok = ok && !makeWorkerRuntimeConfig(*unset).getFinalizerThreadRunner();
  worker.reset();
  withPlugin.reset();
  withoutPlugin.reset();
  unset.reset();
  std::exit(ok ? 0 : 1);
}
TEST_F(IntlLoaderTest, WorkerConfigCopiesFinalizerThreadRunner) {
  EXPECT_EXIT(
      workerConfigCopiesFinalizerThreadRunner(),
      ::testing::ExitedWithCode(0),
      "");
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

TEST(IntlAppleFrameworkTest, ShallowBundleGivesTheSibling) {
  // iOS: .../App.app/Frameworks/hermesvm.framework/hermesvm.
  EXPECT_EQ(
      appleFrameworkCandidate("/var/App.app/Frameworks/hermesvm.framework"),
      "/var/App.app/Frameworks/hermesintl.framework/hermesintl");
}

TEST(IntlAppleFrameworkTest, DeepBundleGivesTheSibling) {
  // macOS and Catalyst: .../hermesvm.framework/Versions/<v>/hermesvm.
  EXPECT_EQ(
      appleFrameworkCandidate(
          "/A.app/Contents/Frameworks/hermesvm.framework/Versions/1"),
      "/A.app/Contents/Frameworks/hermesintl.framework/hermesintl");
  EXPECT_EQ(
      appleFrameworkCandidate("/F/hermesvm.framework/Versions/A"),
      "/F/hermesintl.framework/hermesintl");
}

TEST(IntlAppleFrameworkTest, AnotherFrameworkStillGivesTheSibling) {
  // Hermes linked statically into another framework.
  EXPECT_EQ(
      appleFrameworkCandidate("/App.app/Frameworks/MyKit.framework"),
      "/App.app/Frameworks/hermesintl.framework/hermesintl");
  EXPECT_EQ(
      appleFrameworkCandidate("/F/MyKit.framework/Versions/B"),
      "/F/hermesintl.framework/hermesintl");
  EXPECT_EQ(
      appleFrameworkCandidate("/hermesvm.framework"),
      "/hermesintl.framework/hermesintl");
}

TEST(IntlAppleFrameworkTest, TrailingSlashesAreIgnored) {
  EXPECT_EQ(
      appleFrameworkCandidate("/F/hermesvm.framework/"),
      "/F/hermesintl.framework/hermesintl");
  EXPECT_EQ(
      appleFrameworkCandidate("/F/hermesvm.framework///"),
      "/F/hermesintl.framework/hermesintl");
  EXPECT_EQ(
      appleFrameworkCandidate("/F/hermesvm.framework/Versions/1/"),
      "/F/hermesintl.framework/hermesintl");
}

TEST(IntlAppleFrameworkTest, OtherDirectoriesGiveNothing) {
  EXPECT_EQ(appleFrameworkCandidate("/usr/local/lib"), "");
  EXPECT_EQ(appleFrameworkCandidate("/opt/hermes/bin"), "");
  // The main executable of an iOS app.
  EXPECT_EQ(appleFrameworkCandidate("/var/App.app"), "");
  EXPECT_EQ(appleFrameworkCandidate("/F/hermesvm.frameworks"), "");
  EXPECT_EQ(appleFrameworkCandidate("/F/hermesvm.framework.d"), "");
  EXPECT_EQ(appleFrameworkCandidate("/F/.framework"), "");
  // Inside a framework, but not its binary's directory.
  EXPECT_EQ(appleFrameworkCandidate("/F/hermesvm.framework/Versions"), "");
  EXPECT_EQ(appleFrameworkCandidate("/F/hermesvm.framework/Versions/"), "");
  EXPECT_EQ(appleFrameworkCandidate("/F/hermesvm.framework/Resources"), "");
  EXPECT_EQ(
      appleFrameworkCandidate("/F/hermesvm.framework/Versions/1/lib"), "");
  EXPECT_EQ(appleFrameworkCandidate("/F/hermesvm.framework/Other/1"), "");
  EXPECT_EQ(appleFrameworkCandidate("/F/x/Versions/1"), "");
  // No parent directory, or no directory at all.
  EXPECT_EQ(appleFrameworkCandidate("hermesvm.framework"), "");
  EXPECT_EQ(appleFrameworkCandidate("Versions/1"), "");
  EXPECT_EQ(appleFrameworkCandidate("/"), "");
  EXPECT_EQ(appleFrameworkCandidate(""), "");
}

// The candidate in the tests below, and the extra entries dyld4 lists for it.
static const std::string kArchPath =
    "/App.app/Frameworks/hermesintl.framework/hermesintl";
static const std::string kCryptex =
    "'/System/Volumes/Preboot/Cryptexes/OS" + kArchPath + "' (no such file)";

TEST(IntlArchitectureTest, DyldIncompatibleArchitectureIsRecognised) {
  // dyld's text for a thin library of another architecture (macOS, Xcode
  // 26.4), with the cryptex entry dyld adds.
  const std::string &p = kArchPath;
  EXPECT_TRUE(isIncompatibleArchitecture(
      "dlopen(" + p + ", 0x0006): tried: '" + p +
          "' (mach-o file, but is an incompatible architecture (have "
          "'x86_64', need 'arm64e, arm64e.v1, arm64, arm64')), " +
          kCryptex + ", '" + p +
          "' (mach-o file, but is an incompatible architecture (have "
          "'x86_64', need 'arm64e, arm64e.v1, arm64, arm64'))",
      p));
}

TEST(IntlArchitectureTest, DyldMissingSliceIsRecognised) {
  // dyld's text for a universal library that lacks the running slice.
  const std::string &p = kArchPath;
  EXPECT_TRUE(isIncompatibleArchitecture(
      "dlopen(" + p + ", 0x0006): tried: '" + p +
          "' (fat file, but missing compatible architecture (have "
          "'x86_64', need 'arm64e, arm64e.v1, arm64, arm64')), " +
          kCryptex,
      p));
}

TEST(IntlArchitectureTest, CandidateAfterOtherEntriesIsRecognised) {
  // In the simulator dyld first tries the path under the runtime root.
  const std::string &p = kArchPath;
  EXPECT_TRUE(isIncompatibleArchitecture(
      "dlopen(" + p +
          ", 0x0006): tried: '/Library/Developer/CoreSimulator/"
          "Volumes/iOS_23A343/Library/Developer/CoreSimulator/Profiles/"
          "Runtimes/iOS 26.0.simruntime/Contents/Resources/RuntimeRoot" +
          p + "' (no such file), '" + p +
          "' (mach-o file, but is an incompatible architecture (have "
          "'arm64', need 'x86_64'))",
      p));
}

TEST(IntlArchitectureTest, PhraseInPathIsNotRecognised) {
  // A broken library whose directory names a phrase.
  const std::string p =
      "/tmp/mach-o file, but is an incompatible architecture/"
      "fat file, but missing compatible architecture/hermesintl";
  EXPECT_FALSE(isIncompatibleArchitecture(
      "dlopen(" + p + ", 0x0006): tried: '" + p +
          "' (slice is not valid mach-o file), "
          "'/System/Volumes/Preboot/Cryptexes/OS" +
          p + "' (no such file)",
      p));
}

TEST(IntlArchitectureTest, DependencyArchitectureIsNotRecognised) {
  // The plugin is fine, but hermesvm.framework, which it links, lacks the
  // running architecture.
  const std::string &p = kArchPath;
  const std::string vm = "/App.app/Frameworks/hermesvm.framework/hermesvm";
  EXPECT_FALSE(isIncompatibleArchitecture(
      "dlopen(" + p +
          ", 0x0006): Library not loaded: @rpath/hermesvm.framework/"
          "hermesvm\n  Referenced from: <UUID> " +
          p + "\n  Reason: tried: '" + vm +
          "' (mach-o file, but is an incompatible architecture (have "
          "'arm64', need 'x86_64')), '/System/Volumes/Preboot/Cryptexes/OS" +
          vm + "' (no such file)",
      p));
}

TEST(IntlArchitectureTest, CandidateWithOtherReasonIsNotRecognised) {
  const std::string &p = kArchPath;
  // Not a Mach-O file (dyld's text for a truncated library).
  EXPECT_FALSE(isIncompatibleArchitecture(
      "dlopen(" + p + ", 0x0006): tried: '" + p +
          "' (slice is not valid mach-o file), " + kCryptex,
      p));
  // A phrase later in another reason does not count.
  EXPECT_FALSE(isIncompatibleArchitecture(
      "dlopen(" + p + ", 0x0006): tried: '" + p +
          "' (not a mach-o file: mach-o file, but is an incompatible "
          "architecture)",
      p));
  // An entry for a file whose name only ends with the candidate's.
  EXPECT_FALSE(isIncompatibleArchitecture(
      "dlopen(" + p + ", 0x0006): tried: '/x" + p +
          "' (mach-o file, but is an incompatible architecture (have "
          "'arm64', need 'x86_64'))",
      p));
}

TEST(IntlArchitectureTest, UnterminatedReasonIsNotRecognised) {
  const std::string &p = kArchPath;
  EXPECT_FALSE(isIncompatibleArchitecture(
      "dlopen(" + p + ", 0x0006): tried: '" + p +
          "' (mach-o file, but is an incompatible architecture",
      p));
  EXPECT_FALSE(isIncompatibleArchitecture(
      "dlopen(" + p + ", 0x0006): tried: '" + p +
          "' (mach-o file, but is an incompatible architecture (have "
          "'x86_64', need 'arm64')",
      p));
}

TEST(IntlArchitectureTest, EntryInsideDependencyPathIsNotRecognised) {
  // A dependency whose pathname holds text shaped like the candidate's
  // entry, in a Library-not-loaded message.
  const std::string &p = kArchPath;
  const std::string dep = "/x/tried: '" + p +
      "' (mach-o file, but is an incompatible architecture (have 'arm64', "
      "need 'x86_64'))/hermesvm";
  EXPECT_FALSE(isIncompatibleArchitecture(
      "dlopen(" + p + ", 0x0006): Library not loaded: " + dep +
          "\n  Referenced from: <UUID> " + p + "\n  Reason: tried: '" + dep +
          "' (no such file)",
      p));
  // The same text closing the message, as the last entry's file and reason.
  EXPECT_FALSE(isIncompatibleArchitecture(
      "dlopen(" + p +
          ", 0x0006): Library not loaded: @rpath/x\n  Referenced from: "
          "<UUID> " +
          p + "\n  Reason: tried: '/x/tried: '" + p +
          "' (mach-o file, but is an incompatible architecture (have "
          "'arm64', need 'x86_64'))",
      p));
}

TEST(IntlArchitectureTest, MalformedMessagesAreNotRecognised) {
  const std::string &p = kArchPath;
  const std::string entry = "'" + p +
      "' (mach-o file, but is an incompatible architecture (have 'arm64', "
      "need 'x86_64'))";
  // Text after the last entry.
  EXPECT_FALSE(isIncompatibleArchitecture(
      "dlopen(" + p + ", 0x0006): tried: " + entry + " and more", p));
  EXPECT_FALSE(isIncompatibleArchitecture(
      "dlopen(" + p + ", 0x0006): tried: " + entry + ", ", p));
  // Another path given to dlopen, no flags, or text before the prefix.
  EXPECT_FALSE(
      isIncompatibleArchitecture("dlopen(/other, 0x0006): tried: " + entry, p));
  EXPECT_FALSE(
      isIncompatibleArchitecture("dlopen(" + p + ", 0x): tried: " + entry, p));
  EXPECT_FALSE(isIncompatibleArchitecture(
      "x dlopen(" + p + ", 0x0006): tried: " + entry, p));
  // A second entry for the candidate with another reason.
  EXPECT_FALSE(isIncompatibleArchitecture(
      "dlopen(" + p + ", 0x0006): tried: " + entry + ", '" + p +
          "' (slice is not valid mach-o file)",
      p));
}

TEST(IntlArchitectureTest, OtherDlopenFailuresAreNotRecognised) {
  const std::string &p = kArchPath;
  // Not found.
  EXPECT_FALSE(isIncompatibleArchitecture(
      "dlopen(" + p + ", 0x0006): tried: '" + p + "' (no such file), " +
          kCryptex,
      p));
  // A missing symbol.
  EXPECT_FALSE(isIncompatibleArchitecture(
      "dlopen(" + p +
          ", 0x0006): Symbol not found: "
          "__ZN8facebook3jsi7Runtime8getProp\n  Referenced from: <UUID> " +
          p + "\n  Expected in:     <UUID> /F/hermesvm.framework/hermesvm",
      p));
  // A code-signature rejection.
  EXPECT_FALSE(isIncompatibleArchitecture(
      "dlopen(" + p + ", 0x0006): tried: '" + p +
          "' (code signature in <UUID> '" + p +
          "' not valid for use in process: mapping process and mapped file "
          "(non-platform) have different Team IDs)",
      p));
  // Older loaders' wording, glibc's wrong-class message, text that does not
  // parse, and nothing.
  EXPECT_FALSE(isIncompatibleArchitecture(
      "dlopen(" + p + ", 6): no suitable image found.  Did find:\n\t" + p +
          ": mach-o, but wrong architecture",
      p));
  EXPECT_FALSE(isIncompatibleArchitecture(
      "/x/libhermesintl.so: wrong ELF class: ELFCLASS32", p));
  EXPECT_FALSE(isIncompatibleArchitecture(
      "tried: '" + p + "' mach-o file, but is an incompatible architecture",
      p));
  EXPECT_FALSE(isIncompatibleArchitecture("", p));
  EXPECT_FALSE(isIncompatibleArchitecture(
      "dlopen(, 0x0006): tried: '' (mach-o file, but is an incompatible "
      "architecture (have 'arm64', need 'x86_64'))",
      ""));
}

TEST(IntlWindowsPathTest, RootLengths) {
  EXPECT_EQ(windowsRootLength(u"C:\\a\\b"), 2u);
  EXPECT_EQ(windowsRootLength(u"c:"), 2u);
  EXPECT_EQ(windowsRootLength(u"\\\\?\\C:\\a"), 6u);
  EXPECT_EQ(windowsRootLength(u"\\\\.\\C:\\a"), 6u);
  EXPECT_EQ(windowsRootLength(u"\\\\server\\share\\a"), 14u);
  EXPECT_EQ(windowsRootLength(u"\\\\server\\share"), 14u);
  EXPECT_EQ(windowsRootLength(u"//server/share/a"), 14u);
  EXPECT_EQ(windowsRootLength(u"\\\\?\\UNC\\server\\share\\a"), 20u);
  EXPECT_EQ(windowsRootLength(u"\\\\?\\unc\\server\\share"), 20u);
  EXPECT_EQ(windowsRootLength(u"\\\\?\\Volume{1234}\\a"), 16u);
  EXPECT_EQ(windowsRootLength(u"a\\b"), 0u);
  EXPECT_EQ(windowsRootLength(u"\\a"), 0u);
  EXPECT_EQ(windowsRootLength(u""), 0u);
}

TEST(IntlWindowsPathTest, DirectoryOfAModule) {
  EXPECT_EQ(windowsDirectoryOf(u"C:\\h\\bin\\hermes.exe"), u"C:\\h\\bin");
  EXPECT_EQ(windowsDirectoryOf(u"C:\\hermes.exe"), u"C:");
  EXPECT_EQ(windowsDirectoryOf(u"C:/h/hermes.exe"), u"C:/h");
  EXPECT_EQ(windowsDirectoryOf(u"\\\\?\\C:\\h\\hermesvm.dll"), u"\\\\?\\C:\\h");
  EXPECT_EQ(windowsDirectoryOf(u"\\\\?\\C:\\hermesvm.dll"), u"\\\\?\\C:");
  EXPECT_EQ(
      windowsDirectoryOf(u"\\\\server\\share\\h\\hermes.exe"),
      u"\\\\server\\share\\h");
  EXPECT_EQ(
      windowsDirectoryOf(u"\\\\server\\share\\hermes.exe"),
      u"\\\\server\\share");
  EXPECT_EQ(
      windowsDirectoryOf(u"\\\\?\\UNC\\server\\share\\hermes.exe"),
      u"\\\\?\\UNC\\server\\share");
  EXPECT_EQ(
      windowsDirectoryOf(u"C:\\\u00dcn\u00efc\u00f8d\u00e9\\hermes.exe"),
      u"C:\\\u00dcn\u00efc\u00f8d\u00e9");
  // No separator after the root.
  EXPECT_EQ(windowsDirectoryOf(u"hermes.exe"), u"");
  EXPECT_EQ(windowsDirectoryOf(u"C:hermes.exe"), u"");
  EXPECT_EQ(windowsDirectoryOf(u"\\\\server\\share"), u"");
  EXPECT_EQ(windowsDirectoryOf(u"\\\\?\\C:"), u"");
  EXPECT_EQ(windowsDirectoryOf(u""), u"");
}

TEST(IntlWindowsPathTest, CandidateNextToHermes) {
  using V = std::vector<std::u16string>;
  EXPECT_EQ(windowsDefaultCandidates(u"C:\\h"), V{u"C:\\h\\hermesintl.dll"});
  EXPECT_EQ(windowsDefaultCandidates(u"C:"), V{u"C:\\hermesintl.dll"});
  EXPECT_EQ(
      windowsDefaultCandidates(u"\\\\?\\C:\\h\\lib"),
      V{u"\\\\?\\C:\\h\\lib\\hermesintl.dll"});
  EXPECT_EQ(windowsDefaultCandidates(u""), V{});
}

TEST(IntlWindowsPathTest, BinDirectoryAddsLib) {
  using V = std::vector<std::u16string>;
  EXPECT_EQ(
      windowsDefaultCandidates(u"C:\\h\\bin"),
      (V{u"C:\\h\\bin\\hermesintl.dll", u"C:\\h\\lib\\hermesintl.dll"}));
  // Any ASCII case.
  EXPECT_EQ(
      windowsDefaultCandidates(u"C:\\h\\BIN"),
      (V{u"C:\\h\\BIN\\hermesintl.dll", u"C:\\h\\lib\\hermesintl.dll"}));
  EXPECT_EQ(
      windowsDefaultCandidates(u"C:\\h\\Bin"),
      (V{u"C:\\h\\Bin\\hermesintl.dll", u"C:\\h\\lib\\hermesintl.dll"}));
  // The parent is formed by removing "bin", never with "..", which a
  // "\\?\" path does not resolve.
  EXPECT_EQ(
      windowsDefaultCandidates(u"\\\\?\\C:\\h\\bin"),
      (V{u"\\\\?\\C:\\h\\bin\\hermesintl.dll",
         u"\\\\?\\C:\\h\\lib\\hermesintl.dll"}));
  EXPECT_EQ(
      windowsDefaultCandidates(u"C:\\bin"),
      (V{u"C:\\bin\\hermesintl.dll", u"C:\\lib\\hermesintl.dll"}));
  EXPECT_EQ(
      windowsDefaultCandidates(u"\\\\?\\C:\\bin"),
      (V{u"\\\\?\\C:\\bin\\hermesintl.dll",
         u"\\\\?\\C:\\lib\\hermesintl.dll"}));
  EXPECT_EQ(
      windowsDefaultCandidates(u"C:/h/bin"),
      (V{u"C:/h/bin\\hermesintl.dll", u"C:/h\\lib\\hermesintl.dll"}));
  EXPECT_EQ(
      windowsDefaultCandidates(u"C:\\\u00dcn\u00efc\u00f8d\u00e9\\bin"),
      (V{u"C:\\\u00dcn\u00efc\u00f8d\u00e9\\bin\\hermesintl.dll",
         u"C:\\\u00dcn\u00efc\u00f8d\u00e9\\lib\\hermesintl.dll"}));
}

TEST(IntlWindowsPathTest, OtherDirectoriesAddNothing) {
  using V = std::vector<std::u16string>;
  for (const char16_t *dir :
       {u"C:\\h\\bins",
        u"C:\\h\\xbin",
        u"C:\\h\\bin.d",
        u"C:\\bin\\h",
        u"C:\\h\\b\u0131n"}) {
    EXPECT_EQ(
        windowsDefaultCandidates(dir),
        V{std::u16string(dir) + u"\\hermesintl.dll"});
  }
  // "bin" as part of the root is not a directory to leave.
  EXPECT_EQ(
      windowsDefaultCandidates(u"\\\\server\\bin"),
      V{u"\\\\server\\bin\\hermesintl.dll"});
  EXPECT_EQ(
      windowsDefaultCandidates(u"\\\\?\\UNC\\server\\bin"),
      V{u"\\\\?\\UNC\\server\\bin\\hermesintl.dll"});
  EXPECT_EQ(
      windowsDefaultCandidates(u"\\\\?\\bin"),
      V{u"\\\\?\\bin\\hermesintl.dll"});
  EXPECT_EQ(windowsDefaultCandidates(u"bin"), V{u"bin\\hermesintl.dll"});
}

TEST(IntlWindowsPathTest, BareNames) {
  EXPECT_TRUE(windowsIsBareName("hermesintl.dll"));
  EXPECT_TRUE(windowsIsBareName("hermesintl"));
  EXPECT_TRUE(windowsIsBareName("\xc3\x9c.dll"));
  EXPECT_FALSE(windowsIsBareName("C:hermesintl.dll"));
  EXPECT_FALSE(windowsIsBareName("C:\\h\\hermesintl.dll"));
  EXPECT_FALSE(windowsIsBareName("lib\\hermesintl.dll"));
  EXPECT_FALSE(windowsIsBareName("./hermesintl.dll"));
  EXPECT_FALSE(windowsIsBareName("/h/hermesintl.dll"));
}

TEST(IntlWindowsPathTest, LongPathsGetThePrefix) {
  // 259 characters stay as they are; 260 and more get the prefix.
  std::u16string dir = u"C:\\" + std::u16string(240, u'd');
  std::u16string at259 =
      dir + u"\\" + std::u16string(259 - dir.size() - 1, u'f');
  ASSERT_EQ(at259.size(), 259u);
  EXPECT_EQ(windowsLongPath(at259), at259);
  std::u16string at260 = at259 + u"f";
  EXPECT_EQ(windowsLongPath(at260), u"\\\\?\\" + at260);
  std::u16string candidate = dir + u"\\hermesintl.dll";
  EXPECT_EQ(windowsLongPath(candidate), candidate);
  candidate = dir + u"\\bin\\hermesintl.dll";
  EXPECT_EQ(windowsLongPath(candidate), u"\\\\?\\" + candidate);
  // "/" becomes "\", since the prefix turns that conversion off.
  std::u16string slashes =
      u"C:/" + std::u16string(250, u'd') + u"/hermesintl.dll";
  std::u16string expected =
      u"\\\\?\\C:\\" + std::u16string(250, u'd') + u"\\hermesintl.dll";
  EXPECT_EQ(windowsLongPath(slashes), expected);
  // UNC paths take the UNC form.
  std::u16string unc = u"\\\\server\\share\\" + std::u16string(250, u'd');
  EXPECT_EQ(
      windowsLongPath(unc),
      u"\\\\?\\UNC\\server\\share\\" + std::u16string(250, u'd'));
  // Already in the \\?\ or \\.\ namespace, or relative: unchanged.
  std::u16string prefixed = u"\\\\?\\C:\\" + std::u16string(260, u'd');
  EXPECT_EQ(windowsLongPath(prefixed), prefixed);
  std::u16string device = u"\\\\.\\C:\\" + std::u16string(260, u'd');
  EXPECT_EQ(windowsLongPath(device), device);
  std::u16string relative = std::u16string(270, u'r');
  EXPECT_EQ(windowsLongPath(relative), relative);
  EXPECT_EQ(
      windowsLongPath(u"C:\\h\\hermesintl.dll"), u"C:\\h\\hermesintl.dll");
  EXPECT_EQ(windowsLongPath(u""), u"");
}

TEST(IntlWindowsTextTest, InsertNamesThePath) {
  EXPECT_EQ(
      windowsSubstituteInsert(
          "%1 is not a valid Win32 application.\r\n", "C:\\h\\hermesintl.dll"),
      "C:\\h\\hermesintl.dll is not a valid Win32 application.\r\n");
  // Wine's text for 193.
  EXPECT_EQ(
      windowsSubstituteInsert("Bad EXE format for %1.", "x.dll"),
      "Bad EXE format for x.dll.");
  EXPECT_EQ(windowsSubstituteInsert("%1 and %1", "a"), "a and a");
  // Only %1: other inserts and the text without one are kept.
  EXPECT_EQ(windowsSubstituteInsert("%10 %2 %", "a"), "%10 %2 %");
  EXPECT_EQ(
      windowsSubstituteInsert("The specified module could not be found.", "a"),
      "The specified module could not be found.");
  // The insert itself is not scanned again.
  EXPECT_EQ(windowsSubstituteInsert("%1!", "%1"), "%1!");
  EXPECT_EQ(windowsSubstituteInsert("", "a"), "");
}

TEST(IntlWindowsTextTest, ValidUtf8Converts) {
  EXPECT_EQ(utf8ToUtf16("").value(), u"");
  EXPECT_EQ(utf8ToUtf16("C:\\h\\x.dll").value(), u"C:\\h\\x.dll");
  // Two, three and four bytes, the last a surrogate pair.
  EXPECT_EQ(utf8ToUtf16("\xc3\x9cn\xc3\xaf").value(), u"\u00dcn\u00ef");
  EXPECT_EQ(utf8ToUtf16("\xe6\x97\xa5\xe6\x9c\xac").value(), u"\u65e5\u672c");
  EXPECT_EQ(utf8ToUtf16("\xf0\x9f\x98\x80").value(), u"\U0001f600");
  // The limits of each length.
  EXPECT_EQ(utf8ToUtf16("\x7f").value(), u"\x7f");
  EXPECT_EQ(utf8ToUtf16("\xc2\x80").value(), u"\u0080");
  EXPECT_EQ(utf8ToUtf16("\xdf\xbf").value(), u"\u07ff");
  EXPECT_EQ(utf8ToUtf16("\xe0\xa0\x80").value(), u"\u0800");
  EXPECT_EQ(utf8ToUtf16("\xed\x9f\xbf").value(), u"\ud7ff");
  EXPECT_EQ(utf8ToUtf16("\xee\x80\x80").value(), u"\ue000");
  EXPECT_EQ(utf8ToUtf16("\xef\xbf\xbf").value(), u"\uffff");
  EXPECT_EQ(utf8ToUtf16("\xf0\x90\x80\x80").value(), u"\U00010000");
  EXPECT_EQ(utf8ToUtf16("\xf4\x8f\xbf\xbf").value(), u"\U0010ffff");
}

TEST(IntlWindowsTextTest, InvalidUtf8IsRejected) {
  for (const char *bad :
       {"\x80", // a stray continuation byte
        "a\xbf",
        "\xc3", // truncated
        "\xe6\x97",
        "\xf0\x9f\x98",
        "\xc3(", // a lead byte followed by a non-continuation
        "\xe6\x97(",
        "\xc0\x80", // overlong
        "\xc1\xbf",
        "\xe0\x80\x80",
        "\xe0\x9f\xbf",
        "\xf0\x80\x80\x80",
        "\xf0\x8f\xbf\xbf",
        "\xed\xa0\x80", // encoded surrogates
        "\xed\xbf\xbf",
        "\xed\xa0\xbd\xed\xb8\x80",
        "\xf4\x90\x80\x80", // above U+10FFFF
        "\xf7\xbf\xbf\xbf",
        "\xf8\x88\x80\x80\x80", // five and six-byte forms
        "\xfc\x84\x80\x80\x80\x80",
        "\xfe",
        "\xff"}) {
    EXPECT_FALSE(utf8ToUtf16(bad).has_value()) << testing::PrintToString(bad);
  }
  // Invalid anywhere in an otherwise valid path.
  EXPECT_FALSE(utf8ToUtf16("C:\\h\\\xe9t\xe9\\x.dll").has_value());
}

TEST(IntlWindowsTextTest, Utf16ConvertsForMessages) {
  EXPECT_EQ(utf16ToUtf8(u""), "");
  EXPECT_EQ(utf16ToUtf8(u"C:\\h\\x.dll"), "C:\\h\\x.dll");
  EXPECT_EQ(utf16ToUtf8(u"\u00dcn\u00ef"), "\xc3\x9cn\xc3\xaf");
  EXPECT_EQ(utf16ToUtf8(u"\u65e5\u672c"), "\xe6\x97\xa5\xe6\x9c\xac");
  EXPECT_EQ(utf16ToUtf8(u"\U0001f600"), "\xf0\x9f\x98\x80");
  EXPECT_EQ(utf16ToUtf8(u"\U0010ffff"), "\xf4\x8f\xbf\xbf");
  // Unpaired surrogates become U+FFFD.
  std::u16string lone{u'a', char16_t(0xd800), u'b'};
  EXPECT_EQ(
      utf16ToUtf8(lone),
      "a\xef\xbf\xbd"
      "b");
  lone = {char16_t(0xdc00), char16_t(0xd800)};
  EXPECT_EQ(utf16ToUtf8(lone), "\xef\xbf\xbd\xef\xbf\xbd");
  lone = {u'a', char16_t(0xdbff)};
  EXPECT_EQ(utf16ToUtf8(lone), "a\xef\xbf\xbd");
  // Valid text round-trips.
  std::string text = "C:\\\xc3\x9cn\xc3\xaf\\\xe6\x97\xa5\\\xf0\x9f\x98\x80";
  EXPECT_EQ(utf16ToUtf8(utf8ToUtf16(text).value()), text);
}

TEST(IntlWindowsTextTest, ErrorTextNamesTheCode) {
  EXPECT_EQ(
      windowsErrorText(126, "The specified module could not be found.\r\n"),
      "error 126: The specified module could not be found.");
  EXPECT_EQ(
      windowsErrorText(193, "%1 is not a valid Win32 application. \r\n"),
      "error 193: %1 is not a valid Win32 application.");
  EXPECT_EQ(windowsErrorText(5, " \r\n"), "error 5");
  EXPECT_EQ(windowsErrorText(87, ""), "error 87");
}

TEST(IntlSearchPathTest, OnlyKnownCLibrariesHaveTheSearchPath) {
#if defined(__APPLE__) || defined(_WIN32) || defined(__EMSCRIPTEN__)
  EXPECT_FALSE(searchPathSupported());
#elif defined(__ANDROID__) || defined(__GLIBC__) || defined(__linux__)
  // bionic, glibc, and musl (any other Linux C library).
  EXPECT_TRUE(searchPathSupported());
#else
  EXPECT_FALSE(searchPathSupported());
#endif
}

} // namespace
