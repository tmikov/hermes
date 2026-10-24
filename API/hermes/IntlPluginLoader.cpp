/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/IntlPluginLoader.h"

#include "hermes/BCGen/HBC/BytecodeVersion.h"
#include "hermes/Public/IntlPlugin.h"
#include "hermes/Support/ErrorHandling.h"
#include "hermes/hermes.h"

#include <climits>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <map>
#include <memory>
#include <mutex>
#include <optional>
#include <string>
#include <vector>

#if defined(_WIN32) || defined(__EMSCRIPTEN__)
/// Loading the plugin from a library arrives on these platforms in Phase 6;
/// until then only registerIntlPlugin works.
#define HERMES_INTL_DYNAMIC_LOADING 0
#else
#define HERMES_INTL_DYNAMIC_LOADING 1
#include <dlfcn.h>
#include <sys/stat.h>
#include <unistd.h>
#include <cerrno>
#if defined(__linux__)
#include <elf.h>
#include <link.h>
#include <sys/auxv.h>
#elif defined(__APPLE__)
#include <TargetConditionals.h>
#include <mach-o/dyld.h>
#if TARGET_OS_OSX
#include <libproc.h>
#endif
#endif
#endif

namespace facebook::hermes {
namespace {

/// The file name of the default plugin library.
[[maybe_unused]] constexpr const char *kLibraryName =
#if defined(__APPLE__)
    "libhermesintl.dylib";
#elif defined(_WIN32)
    "hermesintl.dll";
#else
    "libhermesintl.so";
#endif

/// The runtime-data key of the IntlRecord installIntlPlugin stores.
/// Generated randomly, must be unique across all runtime data users.
constexpr jsi::UUID kIntlRecordUUID{
    0xdd08ef7e,
    0x9a6f,
    0x4841,
    0xb4f0,
    0x45f899fff1b0};

/// What installIntlPlugin did for one runtime; makeWorkerRuntimeConfig copies
/// it into a Worker's config.
struct IntlRecord {
  /// The runtime's Intl field (the old, built-in implementation).
  bool oldIntl = true;
  /// Whether a plugin was installed.
  bool installed = false;
  /// The plugin's library path from RuntimeConfig; empty for the default.
  std::string pluginPath;
  /// The defaults the plugin's install received.
  hermes_intl_config defaults;
};

/// A plugin entry that passed checkEntry.
struct LoadedPlugin {
  /// The entry.
  const hermes_intl_entry *entry;
  /// The library path, or "registered with registerIntlPlugin", for
  /// messages.
  std::string where;
};

/// Guards everything in LoaderState.
std::mutex &loaderMutex() {
  static std::mutex mutex;
  return mutex;
}

/// Process-wide loader state. Libraries are loaded once and never unloaded.
struct LoaderState {
  /// Set by registerIntlPlugin.
  const hermes_intl_entry *registered = nullptr;
  /// Whether the default plugin has been resolved.
  bool defaultResolved = false;
  /// The default plugin, if one was found.
  std::optional<LoadedPlugin> defaultPlugin;
  /// Plugins loaded from an explicit IntlPluginPath, by path.
  std::map<std::string, LoadedPlugin> byPath;
  /// How many times the default plugin was resolved; must stay <= 1.
  unsigned defaultResolutions = 0;
  /// Whether promoteHermesImage has run.
  bool hermesImagePromoted = false;
};

/// The process-wide LoaderState. Access it only with loaderMutex() held.
LoaderState &loaderState() {
  static LoaderState state;
  return state;
}

/// Report "Intl plugin <where>: <why>" as a fatal error.
[[noreturn]] void fail(const std::string &where, const std::string &why) {
  ::hermes::hermes_fatal("Intl plugin " + where + ": " + why);
}

/// The flags a plugin must have to match this Hermes.
uint32_t expectedFlags() {
#ifdef JSI_UNSTABLE
  return HERMES_INTL_FLAG_JSI_UNSTABLE;
#else
  return 0;
#endif
}

/// "0x<hex>" for \p flags, for messages.
std::string flagsString(uint32_t flags) {
  char buf[16];
  std::snprintf(buf, sizeof(buf), "0x%x", (unsigned)flags);
  return buf;
}

/// Why a plugin with \p flags does not match this Hermes, which expects
/// \p expected (they differ).
std::string flagsMismatch(uint32_t flags, uint32_t expected) {
  std::string why = "flags " + flagsString(flags) + ", Hermes expects " +
      flagsString(expected);
  if ((flags ^ expected) & HERMES_INTL_FLAG_JSI_UNSTABLE) {
    why = std::string("built with JSI_UNSTABLE ") +
        ((flags & HERMES_INTL_FLAG_JSI_UNSTABLE) ? "on" : "off") +
        ", Hermes with it " +
        ((expected & HERMES_INTL_FLAG_JSI_UNSTABLE) ? "on" : "off") + " (" +
        why + ")";
  }
  return why;
}

/// Check that \p entry can be used by this Hermes; fatal otherwise.
LoadedPlugin checkEntry(const hermes_intl_entry *entry, std::string where) {
  if (!entry)
    fail(where, "returned no entry");
  if (entry->struct_size < sizeof(hermes_intl_entry))
    fail(where, "entry struct is too small");
  if (entry->plugin_interface_version != HERMES_INTL_PLUGIN_INTERFACE_VERSION)
    fail(
        where,
        "plugin interface version " +
            std::to_string(entry->plugin_interface_version) +
            ", Hermes expects " +
            std::to_string(HERMES_INTL_PLUGIN_INTERFACE_VERSION));
  if (entry->bytecode_version != ::hermes::hbc::BYTECODE_VERSION)
    fail(
        where,
        "bytecode version " + std::to_string(entry->bytecode_version) +
            ", Hermes expects " +
            std::to_string(::hermes::hbc::BYTECODE_VERSION));
  if (entry->flags != expectedFlags())
    fail(where, flagsMismatch(entry->flags, expectedFlags()));
  if (!entry->resolve_defaults || !entry->install)
    fail(where, "entry has a null function");
  return LoadedPlugin{entry, std::move(where)};
}

#if HERMES_INTL_DYNAMIC_LOADING

/// Make the Hermes library global before loading a plugin; see below.
void promoteHermesImage();

/// Load the library at \p path and check its entry; fatal on any failure.
/// Caller holds the mutex.
LoadedPlugin loadLibrary(const std::string &path) {
  promoteHermesImage();
  void *handle = ::dlopen(path.c_str(), RTLD_NOW | RTLD_LOCAL);
  if (!handle)
    fail(path, std::string("cannot be loaded: ") + ::dlerror());
  auto getEntry = reinterpret_cast<hermes_intl_get_entry_fn>(
      ::dlsym(handle, HERMES_INTL_ENTRY_SYMBOL));
  if (!getEntry)
    fail(path, "does not export " HERMES_INTL_ENTRY_SYMBOL);
  return checkEntry(getEntry(), path);
}

/// Whether \p path is absent. Any other stat failure (permissions, I/O)
/// counts as present, so loading it reports the real error.
bool fileAbsent(const std::string &path) {
  struct stat st;
  return ::stat(path.c_str(), &st) != 0 &&
      (errno == ENOENT || errno == ENOTDIR);
}

/// The absolute path of the main executable, independent of the current
/// directory; "" if unknown.
std::string mainExecutablePath() {
#if defined(__linux__)
  char buf[PATH_MAX];
  ssize_t n = ::readlink("/proc/self/exe", buf, sizeof(buf) - 1);
  return n > 0 ? std::string(buf, n) : std::string();
#elif defined(__APPLE__)
#if TARGET_OS_OSX
  // proc_pidpath rebuilds the path from the kernel's name cache, so for a
  // hard-linked executable it may report another link's path. That is
  // accepted: hard-linked executables are not a supported layout for
  // default resolution.
  char buf[PROC_PIDPATHINFO_MAXSIZE];
  int n = ::proc_pidpath(::getpid(), buf, sizeof(buf));
  return n > 0 ? std::string(buf, n) : std::string();
#else
  // libproc is macOS-only. Elsewhere (iOS and the other Apple targets) apps
  // are launched by absolute path, so the path dyld recorded is enough.
  char buf[PATH_MAX];
  uint32_t size = sizeof(buf);
  return ::_NSGetExecutablePath(buf, &size) == 0 ? std::string(buf)
                                                 : std::string();
#endif
#else
  return "";
#endif
}

/// Whether \p base, a dladdr dli_fbase, is the main executable's.
bool isMainExecutable(const void *base) {
#if defined(__linux__)
  // AT_PHDR is the address of the main executable's program headers, which
  // sit e_phoff bytes after its ELF header (= dli_fbase).
  auto phdr = reinterpret_cast<const char *>(::getauxval(AT_PHDR));
  auto ehdr = reinterpret_cast<const ElfW(Ehdr) *>(base);
  return phdr && phdr - ehdr->e_phoff == reinterpret_cast<const char *>(base);
#elif defined(__APPLE__)
  return base == ::_dyld_get_image_header(0);
#else
  (void)base;
  return false;
#endif
}

/// Make the shared library holding this code (libhermesvm, in Hermes's own
/// builds) part of the global symbol scope, once per process. Called only
/// when a plugin library is about to be loaded, so a process that has no
/// plugin never pays for it. Caller holds the mutex.
///
/// The plugin links no JSI provider: its JSI references bind, when it is
/// loaded, to the copy in the global scope. A host that dlopen()s
/// libhermesvm RTLD_LOCAL, directly or as a dependency (shermes --exec does
/// this with the program it compiles), keeps libhermesvm's JSI out of that
/// scope, and the plugin's JSI references would not resolve. Promoting the
/// library makes the plugin bind to the JSI of the first Hermes copy that
/// loads a plugin: with two RTLD_LOCAL copies, the first one promoted wins.
/// The cost: libhermesvm's exported symbols become global in that process,
/// so libraries loaded later can bind to them too.
///
/// Nothing is done when this code is in the main executable, whose exported
/// symbols are already global. A failed promotion is not fatal by itself: if
/// the plugin then has unresolved symbols, loading it reports that.
void promoteHermesImage() {
  LoaderState &state = loaderState();
  if (state.hermesImagePromoted)
    return;
  state.hermesImagePromoted = true;
  Dl_info info;
  if (!::dladdr(reinterpret_cast<void *>(&installIntlPlugin), &info) ||
      !info.dli_fname || isMainExecutable(info.dli_fbase))
    return;
  // RTLD_NOLOAD only finds the already loaded library; closing the handle
  // undoes the reference this adds, and the promotion stays.
  if (void *handle =
          ::dlopen(info.dli_fname, RTLD_NOW | RTLD_NOLOAD | RTLD_GLOBAL))
    ::dlclose(handle);
  else
    ::dlerror();
}

#if defined(__linux__)
/// Whether \p path names an existing file.
bool fileExists(const std::string &path) {
  struct stat st;
  return ::stat(path.c_str(), &st) == 0;
}

/// \p path, a pathname as /proc/self/maps shows it, with the kernel's
/// escapes undone: it writes a newline in a name as "\012", a backslash and
/// three octal digits.
std::string unescapeMapsPath(const std::string &path) {
  auto octal = [&](size_t i) { return path[i] >= '0' && path[i] <= '7'; };
  std::string out;
  for (size_t i = 0; i < path.size(); ++i) {
    if (path[i] == '\\' && i + 3 < path.size() && octal(i + 1) &&
        octal(i + 2) && octal(i + 3)) {
      out += static_cast<char>(
          (path[i + 1] - '0') * 64 + (path[i + 2] - '0') * 8 +
          (path[i + 3] - '0'));
      i += 3;
    } else {
      out += path[i];
    }
  }
  return out;
}

/// The absolute path of the file mapped at \p addr, from /proc/self/maps;
/// empty if it cannot be read or has no such file. Unlike dladdr's
/// dli_fname, which is the path the library was loaded by and may be
/// relative to a directory the process has since left, the kernel's path
/// is absolute.
std::string mappedFilePath(const void *addr) {
  FILE *maps = std::fopen("/proc/self/maps", "re");
  if (!maps)
    return "";
  auto a = reinterpret_cast<uintptr_t>(addr);
  std::string path;
  char *line = nullptr;
  size_t capacity = 0;
  ssize_t length;
  while ((length = ::getline(&line, &capacity, maps)) > 0) {
    // start-end perms offset dev inode [pathname]
    unsigned long long start, end, inode;
    int pathStart = 0;
    if (std::sscanf(
            line,
            "%llx-%llx %*s %*s %*s %llu %n",
            &start,
            &end,
            &inode,
            &pathStart) < 3 ||
        a < start || a >= end)
      continue;
    if (inode != 0 && pathStart > 0 && line[pathStart] == '/') {
      path.assign(line + pathStart, length - pathStart);
      if (!path.empty() && path.back() == '\n')
        path.pop_back();
    }
    break;
  }
  std::free(line);
  std::fclose(maps);
  if (path.empty())
    return "";
  // A backslash is not escaped, so "\012" may also be four characters of
  // the name: take the reading that exists.
  std::string unescaped = unescapeMapsPath(path);
  if (fileExists(unescaped))
    return unescaped;
  if (fileExists(path))
    return path;
  // The kernel appends " (deleted)" to the name of a file that was removed
  // after it was mapped. The directory it was in is still the one to search.
  constexpr const char kDeleted[] = " (deleted)";
  constexpr size_t kDeletedLength = sizeof(kDeleted) - 1;
  if (unescaped.size() > kDeletedLength &&
      unescaped.compare(
          unescaped.size() - kDeletedLength, kDeletedLength, kDeleted) == 0)
    return unescaped.substr(0, unescaped.size() - kDeletedLength);
  return "";
}
#endif

/// The directory holding the code of the Hermes library or executable this
/// function is part of; empty if unknown.
///
/// For a library, dladdr reports the path given to dlopen, which a host may
/// have given relative to a directory it has since left. On Linux and
/// Android the library's file is found in /proc/self/maps by the address of
/// this function instead, and dladdr's path is the fallback. On Apple
/// platforms dyld records an absolute path for a library loaded from disk
/// (it asks the kernel for the path of the file it opened), so dladdr's
/// path is used.
std::string hermesDirectory() {
  Dl_info info;
  std::string path;
  if (!::dladdr(reinterpret_cast<void *>(&installIntlPlugin), &info))
    return "";
  // For the main executable dladdr reports argv[0], which may be relative
  // to a directory the process has since left; ask the OS instead.
  if (isMainExecutable(info.dli_fbase)) {
    path = mainExecutablePath();
  } else {
#if defined(__linux__)
    path = mappedFilePath(reinterpret_cast<const void *>(&hermesDirectory));
#endif
    if (path.empty() && info.dli_fname)
      path = info.dli_fname;
  }
  if (path.empty())
    return "";
  char resolved[PATH_MAX];
  if (::realpath(path.c_str(), resolved))
    path = resolved;
  auto slash = path.rfind('/');
  return slash == std::string::npos ? "" : path.substr(0, slash);
}

#if !defined(__APPLE__)
/// Whether \p error, from dlopen of the bare kLibraryName, says that the
/// library itself was not found. A missing dependency, a wrong architecture
/// or unresolved symbols mean it was found but is broken. With a C library
/// whose wording is not known here, every error counts as not found.
bool isLibraryNotFound(const std::string &error) {
#if defined(__ANDROID__)
  // bionic: dlopen failed: library "libhermesintl.so" not found
  return error.find(
             std::string("library \"") + kLibraryName + "\" not found") !=
      std::string::npos;
#elif defined(__GLIBC__)
  // glibc: "libhermesintl.so: cannot open shared object file: No such file
  // or directory". A missing dependency names the dependency instead, and a
  // permission error ends differently; both mean found but broken.
  std::string expected = std::string(kLibraryName) +
      ": cannot open shared object file: No such file or directory";
  return error == expected;
#elif defined(__linux__)
  // musl: "Error loading shared library libhermesintl.so: No such file or
  // directory". A missing dependency appends " (needed by ...)", so the
  // exact comparison treats it as found but broken.
  std::string expected = std::string("Error loading shared library ") +
      kLibraryName + ": No such file or directory";
  return error == expected;
#else
  // A C library whose wording is not known here (FreeBSD and the other
  // BSDs, among others): fail open. Until Phase 6, a broken library found by
  // the search path is treated as absent on these systems, because wrongly
  // calling it broken would abort every runtime creation.
  (void)error;
  return true;
#endif
}

/// The default plugin on the library search path, or nullopt if the search
/// path does not have it. Caller holds the mutex.
std::optional<LoadedPlugin> findOnSearchPath() {
  // Whether the search path has the library is only known by trying, so
  // try before promoting: a process without the plugin must not pay for
  // the promotion. If the library is there but fails to load, it may only
  // be missing the JSI this Hermes keeps local; promote and retry once.
  void *handle = ::dlopen(kLibraryName, RTLD_NOW | RTLD_LOCAL);
  if (!handle) {
    std::string error = ::dlerror();
    if (isLibraryNotFound(error))
      return std::nullopt;
    promoteHermesImage();
    handle = ::dlopen(kLibraryName, RTLD_NOW | RTLD_LOCAL);
    if (!handle)
      fail(kLibraryName, std::string("cannot be loaded: ") + ::dlerror());
  }
  auto getEntry = reinterpret_cast<hermes_intl_get_entry_fn>(
      ::dlsym(handle, HERMES_INTL_ENTRY_SYMBOL));
  if (!getEntry)
    fail(kLibraryName, "does not export " HERMES_INTL_ENTRY_SYMBOL);
  return checkEntry(getEntry(), kLibraryName);
}
#endif // !defined(__APPLE__)

/// Find the default plugin: next to the Hermes code, then in ../lib when
/// that code is in a bin directory, then, except on Apple platforms, on the
/// library search path. Caller holds the mutex.
///
/// Apple platforms have no search-path step. dyld's error for a missing leaf
/// name cannot be told apart from a broken library reliably: older dyld
/// reports only "image not found", and a hardened or sandboxed process gets
/// policy reasons ("relative path not allowed in hardened program", "file
/// system sandbox blocked open()"). Either would make every runtime creation
/// fatal in such a process. A leaf name would also search the current
/// directory, which lets whoever controls it plant a library.
std::optional<LoadedPlugin> findDefaultLibrary() {
  std::string dir = hermesDirectory();
  if (!dir.empty()) {
    std::vector<std::string> candidates{dir + "/" + kLibraryName};
    auto slash = dir.rfind('/');
    if (slash != std::string::npos && dir.substr(slash + 1) == "bin")
      candidates.push_back(dir.substr(0, slash) + "/lib/" + kLibraryName);
    for (const auto &candidate : candidates) {
      if (!fileAbsent(candidate))
        return loadLibrary(candidate);
    }
  }
#if defined(__APPLE__)
  return std::nullopt;
#else
  return findOnSearchPath();
#endif
}

#endif // HERMES_INTL_DYNAMIC_LOADING

/// The plugin at \p path, loading it the first time. Caller holds the mutex.
LoadedPlugin pluginAtPath(LoaderState &state, const std::string &path) {
  auto it = state.byPath.find(path);
  if (it != state.byPath.end())
    return it->second;
#if HERMES_INTL_DYNAMIC_LOADING
  LoadedPlugin plugin = loadLibrary(path);
  state.byPath.emplace(path, plugin);
  return plugin;
#else
  fail(path, "loading the plugin from a library is not supported here yet");
#endif
}

/// The default plugin, resolving it the first time. Caller holds the mutex.
std::optional<LoadedPlugin> defaultPlugin(LoaderState &state) {
  if (!state.defaultResolved) {
    ++state.defaultResolutions;
    if (state.registered) {
      state.defaultPlugin =
          checkEntry(state.registered, "registered with registerIntlPlugin");
    } else {
#if HERMES_INTL_DYNAMIC_LOADING
      state.defaultPlugin = findDefaultLibrary();
#endif
    }
    state.defaultResolved = true;
  }
  return state.defaultPlugin;
}

} // namespace

void registerIntlPlugin(const hermes_intl_entry *entry) {
  std::lock_guard<std::mutex> lock(loaderMutex());
  LoaderState &state = loaderState();
  if (state.defaultResolved)
    fail(
        "registration",
        "registerIntlPlugin was called after the default plugin was resolved");
  state.registered = entry;
}

void installIntlPlugin(
    jsi::Runtime &rt,
    const ::hermes::vm::RuntimeConfig &config) {
  auto record = std::make_shared<IntlRecord>();
  record->oldIntl = config.getIntl();
  std::optional<LoadedPlugin> plugin;
  if (config.getEnableIntlPlugin()) {
    std::lock_guard<std::mutex> lock(loaderMutex());
    LoaderState &state = loaderState();
    if (!config.getIntlPluginPath().empty())
      plugin = pluginAtPath(state, config.getIntlPluginPath());
    else
      plugin = defaultPlugin(state);
  }
  if (plugin) {
    record->installed = true;
    record->pluginPath = config.getIntlPluginPath();
    record->defaults.default_locales = config.getIntlDefaultLocales();
    record->defaults.default_time_zone = config.getIntlDefaultTimeZone();
    std::string error;
    try {
      error = plugin->entry->resolve_defaults(&record->defaults);
    } catch (const std::exception &e) {
      fail(
          plugin->where, std::string("default resolution failed: ") + e.what());
    } catch (...) {
      fail(
          plugin->where, "default resolution failed with an unknown exception");
    }
    if (!error.empty())
      fail(plugin->where, "rejected the default locale or time zone: " + error);
    try {
      plugin->entry->install(rt, &record->defaults);
    } catch (const std::exception &e) {
      fail(plugin->where, std::string("setup failed: ") + e.what());
    } catch (...) {
      fail(plugin->where, "setup failed with an unknown exception");
    }
  }
  rt.setRuntimeData(kIntlRecordUUID, record);
}

unsigned intlDefaultResolutionsForTests() {
  std::lock_guard<std::mutex> lock(loaderMutex());
  return loaderState().defaultResolutions;
}

::hermes::vm::RuntimeConfig makeWorkerRuntimeConfig(jsi::Runtime &parent) {
  auto record = std::static_pointer_cast<IntlRecord>(
      parent.getRuntimeData(kIntlRecordUUID));
  auto builder = ::hermes::vm::RuntimeConfig::Builder();
  if (!record)
    return builder.build();
  builder.withIntl(record->oldIntl).withEnableIntlPlugin(record->installed);
  if (record->installed) {
    builder.withIntlPluginPath(record->pluginPath)
        .withIntlDefaultLocales(record->defaults.default_locales)
        .withIntlDefaultTimeZone(record->defaults.default_time_zone);
  }
  return builder.build();
}

} // namespace facebook::hermes
