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

#include <cctype>
#include <climits>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <map>
#include <memory>
#include <mutex>
#include <optional>
#include <string>
#include <vector>

#if defined(__EMSCRIPTEN__)
/// The plugin is not built for Emscripten, so no library is loaded there;
/// embedders can register a plugin with registerIntlPlugin.
#define HERMES_INTL_DYNAMIC_LOADING 0
#elif defined(_WIN32)
#define HERMES_INTL_DYNAMIC_LOADING 1
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#else
#define HERMES_INTL_DYNAMIC_LOADING 1
#include <dlfcn.h>
#include <sys/stat.h>
#if defined(__linux__)
#include <sys/sysmacros.h>
#endif
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

#if HERMES_INTL_DYNAMIC_LOADING && !defined(__APPLE__) && \
    (defined(__ANDROID__) || defined(__GLIBC__) || defined(__linux__))
/// Whether default resolution ends with the library search path. Only with
/// bionic, glibc and musl (taken to be any other Linux C library), whose
/// dlopen messages tell an absent library from a broken one: see
/// isLibraryNotFound. With any other C library a broken library would look
/// absent and be silently ignored, so the step is left out, as on Apple
/// platforms, where dyld's messages cannot be relied on either (see
/// findDefaultLibrary).
#define HERMES_INTL_SEARCH_PATH 1
#else
#define HERMES_INTL_SEARCH_PATH 0
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

/// What installIntlPlugin recorded for one runtime; makeWorkerRuntimeConfig
/// copies it into a Worker's config.
struct IntlRecord {
  /// Whether a plugin was installed.
  bool installed = false;
  /// The plugin's library path from RuntimeConfig; empty for the default.
  std::string pluginPath;
  /// The defaults the plugin's install received.
  hermes_intl_config defaults;
  /// The runtime's FinalizerThreadRunner from RuntimeConfig. It is not Intl
  /// state, but this record is what a Worker's config is built from, and a
  /// Worker must use the runner its parent was created with: falling back to
  /// the platform default would attach a command-line tool's Worker threads
  /// to a JVM that does not exist on Android.
  std::optional<::hermes::vm::ThreadRunner> finalizerThreadRunner;
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

/// The flags a plugin must have to match this Hermes: its JSI_UNSTABLE
/// setting and, with MSVC, its _ITERATOR_DEBUG_LEVEL and C runtime.
uint32_t expectedFlags() {
  return HERMES_INTL_BUILD_FLAGS;
}

/// "0x<hex>" for \p flags, for messages.
std::string flagsString(uint32_t flags) {
  char buf[16];
  std::snprintf(buf, sizeof(buf), "0x%x", (unsigned)flags);
  return buf;
}

/// The _ITERATOR_DEBUG_LEVEL recorded in \p flags.
unsigned iteratorDebugLevel(uint32_t flags) {
  return (flags & HERMES_INTL_FLAG_ITERATOR_DEBUG_LEVEL_MASK) >>
      HERMES_INTL_FLAG_ITERATOR_DEBUG_LEVEL_SHIFT;
}

/// The C runtime recorded in \p flags, for messages.
const char *crtName(uint32_t flags) {
  return (flags & HERMES_INTL_FLAG_DEBUG_CRT) ? "the debug C runtime (/MDd)"
                                              : "the release C runtime (/MD)";
}

/// Why a plugin with \p flags does not match this Hermes, which expects
/// \p expected (they differ): each known flag that differs, with both
/// values, then both flag words.
std::string flagsMismatch(uint32_t flags, uint32_t expected) {
  uint32_t diff = flags ^ expected;
  std::string reasons;
  auto add = [&reasons](const std::string &reason) {
    if (!reasons.empty())
      reasons += "; ";
    reasons += reason;
  };
  if (diff & HERMES_INTL_FLAG_JSI_UNSTABLE) {
    add(std::string("built with JSI_UNSTABLE ") +
        ((flags & HERMES_INTL_FLAG_JSI_UNSTABLE) ? "on" : "off") +
        ", Hermes with it " +
        ((expected & HERMES_INTL_FLAG_JSI_UNSTABLE) ? "on" : "off"));
  }
  if (diff & HERMES_INTL_FLAG_ITERATOR_DEBUG_LEVEL_MASK) {
    add("built with _ITERATOR_DEBUG_LEVEL " +
        std::to_string(iteratorDebugLevel(flags)) + ", Hermes with " +
        std::to_string(iteratorDebugLevel(expected)));
  }
  if (diff & HERMES_INTL_FLAG_DEBUG_CRT) {
    add(std::string("built with ") + crtName(flags) + ", Hermes with " +
        crtName(expected));
  }
  std::string words = "flags " + flagsString(flags) + ", Hermes expects " +
      flagsString(expected);
  return reasons.empty() ? words : reasons + " (" + words + ")";
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

#if HERMES_INTL_DYNAMIC_LOADING && !defined(_WIN32)

/// Make the Hermes library global before loading a plugin; see below.
void promoteHermesImage();

/// The checked entry of the plugin library \p handle, loaded as \p where;
/// fatal on any failure.
LoadedPlugin entryOfLibrary(void *handle, const std::string &where) {
  auto getEntry = reinterpret_cast<hermes_intl_get_entry_fn>(
      ::dlsym(handle, HERMES_INTL_ENTRY_SYMBOL));
  if (!getEntry)
    fail(where, "does not export " HERMES_INTL_ENTRY_SYMBOL);
  return checkEntry(getEntry(), where);
}

/// Load the library at \p path and check its entry; fatal on any failure.
/// Caller holds the mutex.
LoadedPlugin loadLibrary(const std::string &path) {
  promoteHermesImage();
  void *handle = ::dlopen(path.c_str(), RTLD_NOW | RTLD_LOCAL);
  if (!handle)
    fail(path, std::string("cannot be loaded: ") + ::dlerror());
  return entryOfLibrary(handle, path);
}

/// Load \p path, a default-resolution candidate that exists, and check its
/// entry; fatal on any failure, except that on Apple platforms a library
/// with no code for the running architecture gives nullopt, as if it were
/// absent. An app may ship a plugin without every slice of Hermes (there is
/// no x86_64 plugin for the tvOS and visionOS simulators), and that must
/// leave the app without a plugin rather than abort every runtime creation.
/// The Hermes image is promoted before the dlopen, as in loadLibrary, so a
/// candidate counted as absent this way has still paid for the promotion;
/// that is harmless in framework builds, where the plugin links hermesvm.
/// Caller holds the mutex.
std::optional<LoadedPlugin> loadCandidate(const std::string &path) {
#if defined(__APPLE__)
  promoteHermesImage();
  void *handle = ::dlopen(path.c_str(), RTLD_NOW | RTLD_LOCAL);
  if (!handle) {
    std::string error = ::dlerror();
    if (isIncompatibleArchitecture(error, path))
      return std::nullopt;
    fail(path, "cannot be loaded: " + error);
  }
  return entryOfLibrary(handle, path);
#else
  return loadLibrary(path);
#endif
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
/// What stat says about \p path.
MappedFileId statFileId(const std::string &path) {
  struct stat st;
  if (::stat(path.c_str(), &st) != 0)
    return MappedFileId{};
  return MappedFileId{true, major(st.st_dev), minor(st.st_dev), st.st_ino};
}

/// The absolute path of the file mapped at \p addr, from /proc/self/maps
/// (see pathFromMapsLine); empty if it cannot be read or the file cannot be
/// named. \p mapping gets the mapping's device and inode when the line has
/// them (exists is true), even when the path is empty. Unlike dladdr's
/// dli_fname, which is the path the library was loaded by and may be relative
/// to a directory the process has since left, the kernel's path is absolute.
std::string mappedFilePath(const void *addr, MappedFileId *mapping) {
  *mapping = MappedFileId{};
  FILE *maps = std::fopen("/proc/self/maps", "re");
  if (!maps)
    return "";
  auto a = reinterpret_cast<uintptr_t>(addr);
  std::string path;
  char *line = nullptr;
  size_t capacity = 0;
  while (::getline(&line, &capacity, maps) > 0) {
    unsigned long long start, end, inode;
    unsigned devMajor, devMinor;
    if (std::sscanf(line, "%llx-%llx", &start, &end) == 2 && a >= start &&
        a < end) {
      if (std::sscanf(
              line,
              "%*llx-%*llx %*s %*s %x:%x %llu",
              &devMajor,
              &devMinor,
              &inode) == 3 &&
          inode != 0)
        *mapping = MappedFileId{true, devMajor, devMinor, inode};
      path = pathFromMapsLine(line, statFileId);
      break;
    }
  }
  std::free(line);
  std::fclose(maps);
  return path;
}
#endif

/// The directory holding the code of the Hermes library or executable this
/// function is part of; empty if unknown.
///
/// For a library, dladdr reports the path given to dlopen, which a host may
/// have given relative to a directory it has since left. On Linux and
/// Android the library's file is found in /proc/self/maps by the address of
/// this function instead (see pathFromMapsLine), and dladdr's path is the
/// fallback. On Apple platforms dyld records an absolute path
/// for a library loaded from disk (it asks the kernel for the path of the
/// file it opened), so dladdr's path is used.
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
    MappedFileId mapping;
    path = mappedFilePath(
        reinterpret_cast<const void *>(&hermesDirectory), &mapping);
    if (path.empty() && info.dli_fname) {
      // dladdr's path is the one given to dlopen. A relative one may be
      // relative to a directory the process has left, and realpath shows
      // only that some file is there now: use it only if it is absolute,
      // resolves, and, when the mapping's device and inode are known, is
      // the mapped file.
      char resolved[PATH_MAX];
      if (info.dli_fname[0] != '/' || !::realpath(info.dli_fname, resolved))
        return "";
      if (mapping.exists) {
        MappedFileId id = statFileId(resolved);
        if (!id.exists || id.devMajor != mapping.devMajor ||
            id.devMinor != mapping.devMinor || id.inode != mapping.inode)
          return "";
      }
      path = resolved;
    }
#else
    if (info.dli_fname) {
      // dyld records a canonical absolute path for a library loaded from
      // disk. Unless realpath finds the file, it names no directory that
      // can be trusted.
      char resolved[PATH_MAX];
      if (!::realpath(info.dli_fname, resolved))
        return "";
      path = resolved;
    }
#endif
  }
  if (path.empty())
    return "";
  char resolved[PATH_MAX];
  if (::realpath(path.c_str(), resolved))
    path = resolved;
  auto slash = path.rfind('/');
  return slash == std::string::npos ? "" : path.substr(0, slash);
}

#if HERMES_INTL_SEARCH_PATH
/// Whether \p error, from dlopen of the bare kLibraryName, says that the
/// library itself was not found. A missing dependency, a wrong architecture
/// or unresolved symbols mean it was found but is broken.
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
#else
  // musl: "Error loading shared library libhermesintl.so: No such file or
  // directory". A missing dependency appends " (needed by ...)", so the
  // exact comparison treats it as found but broken.
  std::string expected = std::string("Error loading shared library ") +
      kLibraryName + ": No such file or directory";
  return error == expected;
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
  return entryOfLibrary(handle, kLibraryName);
}
#endif // HERMES_INTL_SEARCH_PATH

/// Find the default plugin: next to the Hermes code; then in ../lib when
/// that code is in a bin directory; then, on Apple platforms, in the sibling
/// hermesintl.framework when that code is in a framework (see
/// appleFrameworkCandidate); then, with glibc, musl and bionic only, on the
/// library search path. Caller holds the mutex.
///
/// Apple platforms have no search-path step. dyld's error for a missing leaf
/// name cannot be told apart from a broken library reliably: older dyld
/// reports only "image not found", and a hardened or sandboxed process gets
/// policy reasons ("relative path not allowed in hardened program", "file
/// system sandbox blocked open()"). Either would make every runtime creation
/// fatal in such a process. A leaf name would also search the current
/// directory, which lets whoever controls it plant a library. Other C
/// libraries have none either (see HERMES_INTL_SEARCH_PATH).
std::optional<LoadedPlugin> findDefaultLibrary() {
  std::string dir = hermesDirectory();
  if (!dir.empty()) {
    std::vector<std::string> candidates{dir + "/" + kLibraryName};
    auto slash = dir.rfind('/');
    if (slash != std::string::npos && dir.substr(slash + 1) == "bin")
      candidates.push_back(dir.substr(0, slash) + "/lib/" + kLibraryName);
#if defined(__APPLE__)
    std::string framework = appleFrameworkCandidate(dir);
    if (!framework.empty())
      candidates.push_back(std::move(framework));
#endif
    for (const auto &candidate : candidates) {
      if (fileAbsent(candidate))
        continue;
      if (std::optional<LoadedPlugin> plugin = loadCandidate(candidate))
        return plugin;
    }
  }
#if HERMES_INTL_SEARCH_PATH
  return findOnSearchPath();
#else
  return std::nullopt;
#endif
}

#elif defined(_WIN32)

// The Windows loader. Paths are UTF-16 from the module path to the Win32
// calls; IntlPluginPath arrives as UTF-8 and must be valid UTF-8. Windows has
// no search-path step (a bare name never searches the current directory or
// PATH) and no promotion: a DLL resolves its imports by module, and the
// plugin links its own copy of JSI.

static_assert(
    sizeof(wchar_t) == sizeof(char16_t),
    "Win32 wide strings are UTF-16");

/// \p s as the wide string Win32 takes.
const wchar_t *wide(const std::u16string &s) {
  return reinterpret_cast<const wchar_t *>(s.c_str());
}

/// The \p n wide characters at \p s as a std::u16string.
std::u16string fromWide(const wchar_t *s, size_t n) {
  return std::u16string(reinterpret_cast<const char16_t *>(s), n);
}

/// The message of Win32 error \p code, as windowsErrorText formats it, with
/// \p insert (the path the call was given) in place of any "%1".
std::string errorText(DWORD code, const std::string &insert) {
  wchar_t *buf = nullptr;
  DWORD n = ::FormatMessageW(
      FORMAT_MESSAGE_ALLOCATE_BUFFER | FORMAT_MESSAGE_FROM_SYSTEM |
          FORMAT_MESSAGE_IGNORE_INSERTS,
      nullptr,
      code,
      0,
      reinterpret_cast<LPWSTR>(&buf),
      0,
      nullptr);
  std::string text;
  if (n && buf)
    text = utf16ToUtf8(fromWide(buf, n));
  if (buf)
    ::LocalFree(buf);
  return windowsErrorText(code, windowsSubstituteInsert(text, insert));
}

/// The checked entry of the plugin module \p module, loaded as \p where;
/// fatal on any failure.
LoadedPlugin entryOfModule(HMODULE module, const std::string &where) {
  auto getEntry = reinterpret_cast<hermes_intl_get_entry_fn>(
      ::GetProcAddress(module, HERMES_INTL_ENTRY_SYMBOL));
  if (!getEntry)
    fail(where, "does not export " HERMES_INTL_ENTRY_SYMBOL);
  return checkEntry(getEntry(), where);
}

/// Load the module at \p path with the LoadLibraryExW search \p flags, or
/// fail, naming \p where and the Win32 error. The loader's error dialogs are
/// turned off for the call (the failure is reported here); the thread's other
/// error-mode flags are kept.
HMODULE
loadModule(const std::u16string &path, DWORD flags, const std::string &where) {
  DWORD oldMode = 0;
  BOOL modeSet = ::SetThreadErrorMode(
      ::GetThreadErrorMode() | SEM_FAILCRITICALERRORS | SEM_NOOPENFILEERRORBOX,
      &oldMode);
  HMODULE module = ::LoadLibraryExW(wide(path), nullptr, flags);
  DWORD error = ::GetLastError();
  if (modeSet)
    ::SetThreadErrorMode(oldMode, nullptr);
  if (!module)
    fail(where, "cannot be loaded: " + errorText(error, where));
  return module;
}

/// The search flags for a module loaded by its absolute path: its own
/// directory first for its imports, then the default directories.
constexpr DWORD kAbsoluteLoadFlags =
    LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS;

/// \p path made absolute against the current directory with
/// GetFullPathNameW; fatal, naming \p where, if that fails.
std::u16string fullPathName(
    const std::u16string &path,
    const std::string &where) {
  std::vector<wchar_t> buf(MAX_PATH);
  for (;;) {
    DWORD n = ::GetFullPathNameW(
        wide(path), static_cast<DWORD>(buf.size()), buf.data(), nullptr);
    if (n == 0)
      fail(
          where,
          "cannot be made absolute: " + errorText(::GetLastError(), where));
    // On success n excludes the terminating null; when the buffer is too
    // small it is the size needed, null included.
    if (n < buf.size())
      return fromWide(buf.data(), n);
    buf.resize(n);
  }
}

/// Load the library at \p path, an IntlPluginPath, and check its entry;
/// fatal on any failure. A bare name loads from the default DLL directories
/// only; any other path is made absolute first. Caller holds the mutex.
LoadedPlugin loadLibrary(const std::string &path) {
  std::optional<std::u16string> utf16 = utf8ToUtf16(path);
  if (!utf16)
    fail(path, "is not valid UTF-8");
  if (windowsIsBareName(path)) {
    return entryOfModule(
        loadModule(*utf16, LOAD_LIBRARY_SEARCH_DEFAULT_DIRS, path), path);
  }
  return entryOfModule(
      loadModule(
          windowsLongPath(fullPathName(*utf16, path)),
          kAbsoluteLoadFlags,
          path),
      path);
}

/// The path of the module (the executable or a DLL) holding this code,
/// through GetModuleFileNameW, which gives a full path; empty if unknown.
std::u16string hermesModulePath() {
  HMODULE module = nullptr;
  if (!::GetModuleHandleExW(
          GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS |
              GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
          reinterpret_cast<LPCWSTR>(
              reinterpret_cast<void *>(&installIntlPlugin)),
          &module))
    return {};
  // A path that fills the buffer may be truncated: grow it, up to twice the
  // longest path Windows supports.
  std::vector<wchar_t> buf(MAX_PATH);
  for (;;) {
    DWORD n = ::GetModuleFileNameW(
        module, buf.data(), static_cast<DWORD>(buf.size()));
    if (n == 0)
      return {};
    if (n < buf.size())
      return fromWide(buf.data(), n);
    if (buf.size() >= 65536)
      return {};
    buf.resize(buf.size() * 2);
  }
}

/// Whether \p path is absent. Any other failure (access denied, a bad
/// network path) counts as present, so loading it reports the real error.
bool fileAbsent(const std::u16string &path) {
  if (::GetFileAttributesW(wide(path)) != INVALID_FILE_ATTRIBUTES)
    return false;
  DWORD error = ::GetLastError();
  return error == ERROR_FILE_NOT_FOUND || error == ERROR_PATH_NOT_FOUND;
}

/// Find the default plugin: next to the module holding the Hermes code, then
/// in ..\lib when that module is in a bin directory (see
/// windowsDefaultCandidates). A candidate is loaded unless it is absent, and
/// any failure to load it is fatal. Caller holds the mutex.
std::optional<LoadedPlugin> findDefaultLibrary() {
  for (const std::u16string &candidate :
       windowsDefaultCandidates(windowsDirectoryOf(hermesModulePath()))) {
    // A candidate of MAX_PATH or more gets the "\\?\" prefix, so that the
    // attribute check and the load see the file even in a process that is
    // not long-path aware; messages name it without the prefix.
    std::u16string path = windowsLongPath(candidate);
    if (fileAbsent(path))
      continue;
    std::string where = utf16ToUtf8(candidate);
    return entryOfModule(loadModule(path, kAbsoluteLoadFlags, where), where);
  }
  return std::nullopt;
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
  fail(
      path,
      "Hermes does not load plugin libraries on this platform; register the "
      "plugin with registerIntlPlugin instead");
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
  record->finalizerThreadRunner = config.getFinalizerThreadRunner();
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

namespace {

/// \p path, a pathname as /proc/self/maps shows it, with the kernel's one
/// escape undone: it writes a newline in a name as "\012". Nothing else is
/// escaped, a backslash included, so any other "\ooo" is literal.
std::string unescapeMapsPath(const std::string &path) {
  std::string out;
  for (size_t i = 0; i < path.size(); ++i) {
    if (path.compare(i, 4, "\\012") == 0) {
      out += '\n';
      i += 3;
    } else {
      out += path[i];
    }
  }
  return out;
}

} // namespace

std::string pathFromMapsLine(
    const std::string &line,
    const std::function<MappedFileId(const std::string &)> &lookup) {
  // start-end perms offset major:minor inode [pathname]
  unsigned devMajor = 0, devMinor = 0;
  unsigned long long inode = 0;
  int pathStart = 0;
  if (std::sscanf(
          line.c_str(),
          "%*llx-%*llx %*s %*s %x:%x %llu %n",
          &devMajor,
          &devMinor,
          &inode,
          &pathStart) < 3 ||
      inode == 0 || pathStart <= 0 || line.c_str()[pathStart] != '/')
    return "";
  std::string path = line.substr(pathStart);
  if (!path.empty() && path.back() == '\n')
    path.pop_back();
  // A plain pathname is the file's name as written. Its device and inode
  // are not checked: on overlayfs and btrfs subvolumes they differ from
  // what stat reports.
  constexpr const char kDeleted[] = " (deleted)";
  constexpr size_t kDeletedLength = sizeof(kDeleted) - 1;
  bool deleted = path.size() > kDeletedLength &&
      path.compare(path.size() - kDeletedLength, kDeletedLength, kDeleted) == 0;
  bool escaped = path.find("\\012") != std::string::npos;
  if (!deleted && !escaped)
    return lookup(path).exists ? path : "";
  // Otherwise the text is ambiguous: "\012" is a newline or those four
  // characters, since a backslash is not escaped, and " (deleted)" is
  // either the kernel's mark of a removed file or part of the name. A
  // reading is accepted only if its device and inode are the mapping's;
  // else the caller falls back. (A removed file has no name with its
  // identity; a deleted libhermesvm usually means an upgrade, and the
  // plugin now beside it would be from another build.)
  std::vector<std::string> readings{unescapeMapsPath(path), path};
  if (deleted) {
    std::string stripped = path.substr(0, path.size() - kDeletedLength);
    readings.push_back(unescapeMapsPath(stripped));
    readings.push_back(stripped);
  }
  for (const std::string &reading : readings) {
    MappedFileId id = lookup(reading);
    if (id.exists && id.devMajor == devMajor && id.devMinor == devMinor &&
        id.inode == inode)
      return reading;
  }
  return "";
}

std::string appleFrameworkCandidate(const std::string &dir) {
  std::string path = dir;
  while (!path.empty() && path.back() == '/')
    path.pop_back();
  // Split off the last component of path; false if it has no parent.
  auto popComponent = [&path](std::string &component) {
    auto slash = path.rfind('/');
    if (slash == std::string::npos)
      return false;
    component = path.substr(slash + 1);
    path.erase(slash);
    return true;
  };
  constexpr const char kSuffix[] = ".framework";
  constexpr size_t kSuffixLength = sizeof(kSuffix) - 1;
  auto isFramework = [&](const std::string &name) {
    return name.size() > kSuffixLength &&
        name.compare(name.size() - kSuffixLength, kSuffixLength, kSuffix) == 0;
  };
  std::string last;
  if (!popComponent(last))
    return "";
  if (!isFramework(last)) {
    // A deep bundle: <p>/<name>.framework/Versions/<v>, where last is <v>.
    std::string versions, bundle;
    if (!popComponent(versions) || versions != "Versions" ||
        !popComponent(bundle) || !isFramework(bundle))
      return "";
  }
  return path + "/hermesintl.framework/hermesintl";
}

bool isIncompatibleArchitecture(
    const std::string &message,
    const std::string &path) {
  // dyld4 words a failed dlopen of a file it could not use as
  // "dlopen(<path>, 0x<flags>): tried: '<file>' (<reason>), '<file>'
  // (<reason>), ..." and nothing after the last entry, for example
  // "dlopen(<path>, 0x0006): tried: '<path>' (mach-o file, but is an
  // incompatible architecture (have 'x86_64', need 'arm64')), '/System/
  // Volumes/Preboot/Cryptexes/OS<path>' (no such file), '<path>' (mach-o
  // file, ...)", or "(fat file, but missing compatible architecture (have
  // ..., need ...))" for a universal library that lacks the running slice.
  // The whole message must have that form, so a dependency's failure
  // ("dlopen(<path>, 0x0006): Library not loaded: ...") and text elsewhere
  // that merely looks like an entry never match.
  static const char *const kPhrases[] = {
      "mach-o file, but is an incompatible architecture",
      "fat file, but missing compatible architecture"};
  if (path.empty())
    return false;
  const std::string prefix = "dlopen(" + path + ", 0x";
  if (message.compare(0, prefix.size(), prefix) != 0)
    return false;
  size_t pos = prefix.size();
  size_t digits = pos;
  while (pos < message.size() &&
         std::isxdigit(static_cast<unsigned char>(message[pos])))
    ++pos;
  static const std::string kTried = "): tried: ";
  if (pos == digits || message.compare(pos, kTried.size(), kTried) != 0)
    return false;
  pos += kTried.size();

  // The entries, to the end of the message. Every entry for the candidate
  // must give an architecture reason, and there must be at least one.
  const std::string candidateEntry = "'" + path + "' (";
  bool found = false;
  for (;;) {
    if (pos >= message.size() || message[pos] != '\'')
      return false;
    size_t reason;
    if (message.compare(pos, candidateEntry.size(), candidateEntry) == 0) {
      reason = pos + candidateEntry.size();
      bool architecture = false;
      for (const char *phrase : kPhrases)
        architecture = architecture ||
            message.compare(reason, std::strlen(phrase), phrase) == 0;
      if (!architecture)
        return false;
      found = true;
    } else {
      size_t fileEnd = message.find("' (", pos + 1);
      if (fileEnd == std::string::npos)
        return false;
      reason = fileEnd + 3;
    }
    // The reason runs to its matching ")"; its "(" is at reason - 1.
    unsigned depth = 1;
    pos = reason;
    while (pos < message.size() && depth) {
      if (message[pos] == '(')
        ++depth;
      else if (message[pos] == ')')
        --depth;
      ++pos;
    }
    if (depth)
      return false;
    if (pos == message.size())
      return found;
    if (message.compare(pos, 2, ", ") != 0)
      return false;
    pos += 2;
  }
}

bool searchPathSupported() {
  return HERMES_INTL_SEARCH_PATH;
}

namespace {

/// Whether \p c separates Windows path components.
bool isWindowsSeparator(char16_t c) {
  return c == u'\\' || c == u'/';
}

/// Whether \p c is an ASCII letter.
bool isAsciiLetter(char16_t c) {
  return (c >= u'a' && c <= u'z') || (c >= u'A' && c <= u'Z');
}

/// \p c in ASCII lower case.
char16_t asciiLower(char16_t c) {
  return c >= u'A' && c <= u'Z' ? static_cast<char16_t>(c - u'A' + u'a') : c;
}

/// Whether \p s equals the lower-case ASCII \p lower in any ASCII case.
bool equalsAsciiIgnoringCase(const std::u16string &s, const char16_t *lower) {
  size_t i = 0;
  for (; i < s.size() && lower[i]; ++i) {
    if (asciiLower(s[i]) != lower[i])
      return false;
  }
  return i == s.size() && !lower[i];
}

/// The end of \p n path components of \p path from \p start: the index of
/// the separator after the last one, or path.size().
size_t endOfComponents(const std::u16string &path, size_t start, unsigned n) {
  size_t i = start;
  for (unsigned k = 0; k < n; ++k) {
    if (k > 0) {
      if (i == path.size())
        return i;
      ++i;
    }
    while (i < path.size() && !isWindowsSeparator(path[i]))
      ++i;
  }
  return i;
}

/// The index of the last separator of \p path that is not inside its root,
/// or npos.
size_t lastSeparatorAfterRoot(const std::u16string &path) {
  size_t root = windowsRootLength(path);
  for (size_t i = path.size(); i > root; --i) {
    if (isWindowsSeparator(path[i - 1]))
      return i - 1;
  }
  return std::u16string::npos;
}

/// The plugin's file name on Windows.
constexpr const char16_t kWindowsLibraryName[] = u"hermesintl.dll";

} // namespace

size_t windowsRootLength(const std::u16string &path) {
  auto sep = [&path](size_t i) {
    return i < path.size() && isWindowsSeparator(path[i]);
  };
  auto drive = [&path](size_t i) {
    return i + 1 < path.size() && isAsciiLetter(path[i]) && path[i + 1] == u':';
  };
  if (sep(0) && sep(1) && path.size() > 2 &&
      (path[2] == u'?' || path[2] == u'.') && sep(3)) {
    // "\\?\" (or the device namespace "\\.\") and what follows it.
    if (drive(4))
      return 6;
    if (path.size() >= 8 && asciiLower(path[4]) == u'u' &&
        asciiLower(path[5]) == u'n' && asciiLower(path[6]) == u'c' && sep(7))
      return endOfComponents(path, 8, 2);
    return endOfComponents(path, 4, 1);
  }
  if (sep(0) && sep(1) && !sep(2) && path.size() > 2)
    return endOfComponents(path, 2, 2);
  return drive(0) ? 2 : 0;
}

std::u16string windowsDirectoryOf(const std::u16string &path) {
  size_t sep = lastSeparatorAfterRoot(path);
  return sep == std::u16string::npos ? std::u16string() : path.substr(0, sep);
}

std::vector<std::u16string> windowsDefaultCandidates(
    const std::u16string &dir) {
  std::vector<std::u16string> candidates;
  if (dir.empty())
    return candidates;
  candidates.push_back(dir + u"\\" + kWindowsLibraryName);
  size_t sep = lastSeparatorAfterRoot(dir);
  if (sep != std::u16string::npos &&
      equalsAsciiIgnoringCase(dir.substr(sep + 1), u"bin"))
    candidates.push_back(dir.substr(0, sep) + u"\\lib\\" + kWindowsLibraryName);
  return candidates;
}

bool windowsIsBareName(const std::string &path) {
  return path.find_first_of("\\/:") == std::string::npos;
}

std::u16string windowsLongPath(const std::u16string &path) {
  // MAX_PATH, which counts the terminating null.
  constexpr size_t kMaxPath = 260;
  if (path.size() < kMaxPath)
    return path;
  auto sep = [&path](size_t i) {
    return i < path.size() && isWindowsSeparator(path[i]);
  };
  // "\\?\" turns off the conversion of "/" to "\", so convert here.
  auto backslashes = [](std::u16string s) {
    for (char16_t &c : s) {
      if (c == u'/')
        c = u'\\';
    }
    return s;
  };
  if (sep(0) && sep(1) && path.size() > 2 &&
      (path[2] == u'?' || path[2] == u'.') && sep(3))
    return path;
  if (path.size() > 2 && isAsciiLetter(path[0]) && path[1] == u':' && sep(2))
    return u"\\\\?\\" + backslashes(path);
  if (sep(0) && sep(1) && !sep(2) && path.size() > 2)
    return u"\\\\?\\UNC\\" + backslashes(path.substr(2));
  return path;
}

std::string windowsSubstituteInsert(
    const std::string &text,
    const std::string &insert) {
  std::string out;
  for (size_t i = 0; i < text.size(); ++i) {
    if (text.compare(i, 2, "%1") == 0 &&
        (i + 2 == text.size() || text[i + 2] < '0' || text[i + 2] > '9')) {
      out += insert;
      ++i;
    } else {
      out += text[i];
    }
  }
  return out;
}

std::optional<std::u16string> utf8ToUtf16(const std::string &utf8) {
  std::u16string out;
  out.reserve(utf8.size());
  for (size_t i = 0; i < utf8.size();) {
    auto lead = static_cast<unsigned char>(utf8[i]);
    if (lead < 0x80) {
      out.push_back(lead);
      ++i;
      continue;
    }
    size_t length;
    uint32_t cp, min;
    if ((lead & 0xe0) == 0xc0) {
      length = 2, cp = lead & 0x1f, min = 0x80;
    } else if ((lead & 0xf0) == 0xe0) {
      length = 3, cp = lead & 0x0f, min = 0x800;
    } else if ((lead & 0xf8) == 0xf0) {
      length = 4, cp = lead & 0x07, min = 0x10000;
    } else {
      return std::nullopt;
    }
    if (utf8.size() - i < length)
      return std::nullopt;
    for (size_t k = 1; k < length; ++k) {
      auto c = static_cast<unsigned char>(utf8[i + k]);
      if ((c & 0xc0) != 0x80)
        return std::nullopt;
      cp = (cp << 6) | (c & 0x3f);
    }
    if (cp < min || cp > 0x10ffff || (cp >= 0xd800 && cp <= 0xdfff))
      return std::nullopt;
    if (cp >= 0x10000) {
      cp -= 0x10000;
      out.push_back(static_cast<char16_t>(0xd800 + (cp >> 10)));
      out.push_back(static_cast<char16_t>(0xdc00 + (cp & 0x3ff)));
    } else {
      out.push_back(static_cast<char16_t>(cp));
    }
    i += length;
  }
  return out;
}

std::string utf16ToUtf8(const std::u16string &utf16) {
  std::string out;
  out.reserve(utf16.size());
  for (size_t i = 0; i < utf16.size(); ++i) {
    uint32_t cp = utf16[i];
    if (cp >= 0xd800 && cp <= 0xdbff && i + 1 < utf16.size() &&
        utf16[i + 1] >= 0xdc00 && utf16[i + 1] <= 0xdfff) {
      cp = 0x10000 + ((cp - 0xd800) << 10) + (utf16[i + 1] - 0xdc00);
      ++i;
    } else if (cp >= 0xd800 && cp <= 0xdfff) {
      cp = 0xfffd;
    }
    if (cp < 0x80) {
      out += static_cast<char>(cp);
    } else if (cp < 0x800) {
      out += static_cast<char>(0xc0 | (cp >> 6));
      out += static_cast<char>(0x80 | (cp & 0x3f));
    } else if (cp < 0x10000) {
      out += static_cast<char>(0xe0 | (cp >> 12));
      out += static_cast<char>(0x80 | ((cp >> 6) & 0x3f));
      out += static_cast<char>(0x80 | (cp & 0x3f));
    } else {
      out += static_cast<char>(0xf0 | (cp >> 18));
      out += static_cast<char>(0x80 | ((cp >> 12) & 0x3f));
      out += static_cast<char>(0x80 | ((cp >> 6) & 0x3f));
      out += static_cast<char>(0x80 | (cp & 0x3f));
    }
  }
  return out;
}

std::string windowsErrorText(unsigned long code, const std::string &text) {
  size_t end = text.find_last_not_of(" \t\r\n");
  std::string result = "error " + std::to_string(code);
  if (end != std::string::npos)
    result += ": " + text.substr(0, end + 1);
  return result;
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
  builder.withFinalizerThreadRunner(record->finalizerThreadRunner)
      .withEnableIntlPlugin(record->installed);
  if (record->installed) {
    builder.withIntlPluginPath(record->pluginPath)
        .withIntlDefaultLocales(record->defaults.default_locales)
        .withIntlDefaultTimeZone(record->defaults.default_time_zone);
  }
  return builder.build();
}

} // namespace facebook::hermes
