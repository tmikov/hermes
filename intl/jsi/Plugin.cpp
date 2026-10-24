/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The Intl plugin's entry point. See "Loading and installation" in
// doc/superpowers/specs/2026-10-22-intl-plugin-design.md.

#include "Defaults.h"
#include "NativeHelpers.h"

#include <hermes/Public/IntlPlugin.h>
#include <jsi/hermes-interfaces.h>
#include <jsi/jsi.h>

#include <cstddef>
#include <cstdint>
#include <memory>
#include <utility>

using namespace facebook;

namespace hermes_intl {

/// The plugin's JS (intl/js) as bytecode; defined in IntlBytecode.cpp.
std::pair<const uint8_t *, size_t> intlBytecode();

namespace {

/// A jsi::Buffer over the embedded bytecode, which lives as long as the
/// library, which is never unloaded.
class StaticBuffer final : public jsi::Buffer {
 public:
  /// Wrap \p size bytes at \p data, which must outlive the buffer.
  StaticBuffer(const uint8_t *data, size_t size) : data_(data), size_(size) {}
  /// The number of bytes.
  size_t size() const override {
    return size_;
  }
  /// The bytes.
  const uint8_t *data() const override {
    return data_;
  }

 private:
  /// The embedded bytecode.
  const uint8_t *data_;
  /// Its size in bytes.
  size_t size_;
};

/// hermes_intl_entry::install: run the JS and call its setup function with
/// the native helpers and the resolved defaults.
void install(jsi::Runtime &rt, const hermes_intl_config *config) {
  auto *builtin =
      jsi::castInterface<facebook::hermes::IHermesBuiltinBytecode>(&rt);
  if (!builtin)
    throw jsi::JSINativeException(
        "the runtime does not implement IHermesBuiltinBytecode");
  auto bytecode = intlBytecode();
  jsi::Function setup =
      builtin
          ->runBuiltinBytecode(
              std::make_shared<StaticBuffer>(bytecode.first, bytecode.second),
              "hermes-intl.js")
          .asObject(rt)
          .asFunction(rt);

  setup.call(
      rt,
      makeNativeHelpers(rt),
      makeSetupDefaults(
          rt, config->default_locales, config->default_time_zone));
}

/// The flags this plugin was built with.
constexpr uint32_t kFlags =
#ifdef JSI_UNSTABLE
    HERMES_INTL_FLAG_JSI_UNSTABLE;
#else
    0;
#endif

/// What hermes_intl_get_entry returns.
const hermes_intl_entry kEntry{
    sizeof(hermes_intl_entry),
    HERMES_INTL_PLUGIN_INTERFACE_VERSION,
    HERMES_INTL_BYTECODE_VERSION,
    kFlags,
    resolveDefaults,
    install};

} // namespace
} // namespace hermes_intl

/// The plugin's one exported symbol, HERMES_INTL_ENTRY_SYMBOL.
extern "C" __attribute__((visibility("default"))) const hermes_intl_entry *
hermes_intl_get_entry() {
  return &hermes_intl::kEntry;
}
