/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#pragma once

/// The interface between Hermes and the Intl plugin library (libhermesintl).
/// See doc/superpowers/specs/2026-10-22-intl-plugin-design.md, "Loading and
/// installation". A plugin is supported only with the Hermes it was built
/// with: this is a C++ interface, and the loader checks the versions and
/// flags below before calling into the plugin.
///
/// The loader holds a lock while it loads a plugin library and calls its
/// hermes_intl_get_entry. A plugin's static constructors and its
/// hermes_intl_get_entry must therefore not create runtimes or call
/// registerIntlPlugin, which would deadlock.

#include <cstdint>
#include <string>
#include <vector>

namespace facebook::jsi {
class Runtime;
} // namespace facebook::jsi

/// Version of this interface: this header, IHermesBuiltinBytecode, and the
/// meaning of every field below. Bump it on any change to them.
#define HERMES_INTL_PLUGIN_INTERFACE_VERSION 1

/// The name of the function every plugin library exports, of type
/// hermes_intl_get_entry_fn.
#define HERMES_INTL_ENTRY_SYMBOL "hermes_intl_get_entry"

/// Set in hermes_intl_entry::flags when the plugin was compiled with
/// JSI_UNSTABLE, which changes the layout of jsi::Runtime.
#define HERMES_INTL_FLAG_JSI_UNSTABLE 1u

/// The defaults a runtime's Intl starts with. All strings are UTF-8.
struct hermes_intl_config {
  /// Preferred locales, most preferred first, as BCP 47 tags.
  std::vector<std::string> default_locales;
  /// An IANA time zone name.
  std::string default_time_zone;
};

/// What hermes_intl_get_entry returns: a static description of the plugin.
struct hermes_intl_entry {
  /// sizeof(hermes_intl_entry) in the plugin.
  uint32_t struct_size;
  /// HERMES_INTL_PLUGIN_INTERFACE_VERSION in the plugin.
  uint32_t plugin_interface_version;
  /// The Hermes bytecode version of the plugin's embedded JS.
  uint32_t bytecode_version;
  /// HERMES_INTL_FLAG_* bits.
  uint32_t flags;
  /// Turn the defaults requested by RuntimeConfig (possibly empty) into the
  /// effective ones, in place: canonicalize the given values and detect the
  /// missing ones. Returns "" on success, or a message naming the invalid
  /// value.
  std::string (*resolve_defaults)(hermes_intl_config *config);
  /// Install Intl into \p rt using \p config as resolved by
  /// resolve_defaults. Throws on failure.
  void (*install)(facebook::jsi::Runtime &rt, const hermes_intl_config *config);
};

/// The type of HERMES_INTL_ENTRY_SYMBOL.
typedef const hermes_intl_entry *(*hermes_intl_get_entry_fn)();
