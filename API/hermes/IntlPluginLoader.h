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
#include <string>

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
/// received, and the parent's Intl flag. Everything else is the default.
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

/// How many times this process has resolved the default plugin (0 or 1).
/// For tests.
unsigned intlDefaultResolutionsForTests();

} // namespace facebook::hermes
