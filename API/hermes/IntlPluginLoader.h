/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#pragma once

#include "hermes/Public/RuntimeConfig.h"

#include <jsi/jsi.h>

#include <functional>
#include <optional>
#include <string>
#include <vector>

namespace facebook::hermes {

/// Install the Intl plugin into \p rt as \p config asks, and record the
/// outcome on \p rt for makeWorkerRuntimeConfig. Follows the "Loader" section
/// of doc/superpowers/specs/2026-10-22-intl-plugin-design.md: every row of
/// its error table marked fatal calls hermes_fatal.
void installIntlPlugin(
    jsi::Runtime &rt,
    const ::hermes::vm::RuntimeConfig &config);

/// The RuntimeConfig for a Worker started by \p parent: the parent's plugin
/// (or none), the default locales and time zone the parent's plugin actually
/// received, the parent's Intl flag, and the parent's finalizer thread runner.
/// Everything else is the default.
::hermes::vm::RuntimeConfig makeWorkerRuntimeConfig(jsi::Runtime &parent);

/// What stat says about a file: whether it exists, and its device numbers
/// and inode.
struct MappedFileId {
  bool exists = false;
  unsigned devMajor = 0;
  unsigned devMinor = 0;
  unsigned long long inode = 0;
};

/// The path of the file that \p line, one line of /proc/self/maps,
/// describes, or "" if it names none that can be trusted, in which case the
/// loader falls back to dladdr. \p lookup stats a path. The kernel escapes
/// only a newline in the pathname, as "\012", and appends " (deleted)" to a
/// removed file. A pathname with neither is used if it exists. One with
/// either is used only through a reading (the "\012"s decoded or as
/// written, with or without the " (deleted)" suffix) whose device and inode
/// are the mapping's.
/// Linux and Android only, in effect; portable so tests can run it on fake
/// lines.
std::string pathFromMapsLine(
    const std::string &line,
    const std::function<MappedFileId(const std::string &)> &lookup);

/// The sibling-framework candidate of default resolution on Apple platforms,
/// or "" if there is none. When \p dir, the directory holding the Hermes
/// code, is "<p>/<name>.framework" (a shallow bundle, as on iOS) or
/// "<p>/<name>.framework/Versions/<v>" (a deep bundle, as on macOS), it is
/// "<p>/hermesintl.framework/hermesintl". <name> need not be hermesvm: Hermes
/// may be linked statically into another framework, and the plugin is then
/// that framework's sibling. Trailing slashes in \p dir are ignored.
/// Used on Apple platforms only; portable so tests can run it anywhere.
std::string appleFrameworkCandidate(const std::string &dir);

/// Whether \p message, from a failed dlopen of \p path, says that the library
/// itself has no code for the running architecture. The whole message must
/// be dyld4's "dlopen(<path>, 0x<flags>): tried: '<file>' (<reason>), ...",
/// with balanced parentheses in each reason and nothing after the last
/// entry, and every entry whose file is exactly \p path (at least one) must
/// have a reason that begins with "mach-o file, but is an incompatible
/// architecture" (a thin library) or "fat file, but missing compatible
/// architecture" (a universal library without the running slice). A
/// dependency's failure ("Library not loaded: ..."), any other message, and
/// the wording of loaders older than dyld4 never match. On Apple platforms
/// such a default-resolution candidate counts as absent. Portable so tests
/// can run it anywhere.
bool isIncompatibleArchitecture(
    const std::string &message,
    const std::string &path);

/// Whether default resolution in this build has the library search-path
/// step: only with the C libraries whose dlopen "not found" message the
/// loader knows (glibc, musl and bionic).
bool searchPathSupported();

// Path and text helpers of the Windows loader. Windows paths are UTF-16,
// held here as std::u16string (wchar_t is 16 bits on Windows only). Used on
// Windows only; portable so tests can run them anywhere.

/// The length of the root of the Windows path \p path: "C:" (2), "\\?\C:" and
/// "\\.\C:" (6); "\\server\share", "\\?\UNC\server\share" and "\\?\<volume>"
/// up to the separator after the share or volume; 0 for a path without one
/// of these roots. A directory is never split inside its root.
size_t windowsRootLength(const std::u16string &path);

/// The directory of the module file \p path ("C:\a\hermes.exe" gives
/// "C:\a"), with any "\\?\" prefix kept; "" if \p path has no separator
/// after its root. "\" and "/" both separate.
std::u16string windowsDirectoryOf(const std::u16string &path);

/// The default-resolution candidates for a Hermes module in the directory
/// \p dir: "<dir>\hermesintl.dll"; then, when the last component of \p dir is
/// "bin" in any ASCII case and is not part of its root,
/// "<parent>\lib\hermesintl.dll", where <parent> is \p dir without that
/// component. The parent is never formed with "..", which a "\\?\" path
/// does not resolve. Nothing for an empty \p dir.
std::vector<std::u16string> windowsDefaultCandidates(const std::u16string &dir);

/// Whether \p path, an IntlPluginPath, is a bare name: it has no "\", "/"
/// or ":", so Windows loads it from the default DLL directories. Any other
/// path is made absolute first.
bool windowsIsBareName(const std::string &path);

/// \p path with the "\\?\" prefix when it has MAX_PATH (260) characters or
/// more, so that Win32 file functions accept it in a process that is not
/// long-path aware: "C:\..." becomes "\\?\C:\...", "\\server\share\..."
/// becomes "\\?\UNC\server\share\...", and "/" becomes "\" (the prefix turns
/// off that conversion). \p path must be absolute and normalized, as module
/// paths and GetFullPathNameW results are. A shorter path, a path already in
/// the "\\?\" or "\\.\" namespace, and a relative path are returned as given.
std::u16string windowsLongPath(const std::u16string &path);

/// \p text, a FormatMessage text read with FORMAT_MESSAGE_IGNORE_INSERTS,
/// with \p insert in place of each "%1" (not "%10" and up): error 193's
/// "%1 is not a valid Win32 application." names the file this way.
std::string windowsSubstituteInsert(
    const std::string &text,
    const std::string &insert);

/// \p utf8 as UTF-16, or nullopt if it is not valid UTF-8: a truncated or
/// overlong sequence, a stray continuation byte, an encoded surrogate, or a
/// code point above U+10FFFF.
std::optional<std::u16string> utf8ToUtf16(const std::string &utf8);

/// \p utf16 as UTF-8, with each unpaired surrogate replaced by U+FFFD. For
/// messages: Windows file names need not be valid UTF-16.
std::string utf16ToUtf8(const std::u16string &utf16);

/// "error <code>: <text>" for a Win32 error, \p text being what
/// FormatMessage gave, with trailing white space removed; "error <code>"
/// when no text is left.
std::string windowsErrorText(unsigned long code, const std::string &text);

/// How many times this process has resolved the default plugin (0 or 1).
/// For tests.
unsigned intlDefaultResolutionsForTests();

} // namespace facebook::hermes
