/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// A stand-in for libhermesintl, used by test/hermes/intl-plugin. Compile
// definitions select a broken variant:
//   FAKE_NO_ENTRY           do not export hermes_intl_get_entry
//   FAKE_NULL_ENTRY         hermes_intl_get_entry returns null
//   FAKE_INTERFACE_DELTA    added to the interface version
//   FAKE_BYTECODE_DELTA     added to the bytecode version
//   FAKE_FLIP_FLAGS         flip the JSI_UNSTABLE flag
//   FAKE_THROWING_INSTALL   install throws
//   FAKE_REJECT_DEFAULTS    resolve_defaults reports an error
//   FAKE_NEEDS_DEP          use a function from libfake_intl_dep.so

#include "hermes/BCGen/HBC/BytecodeVersion.h"
#include "hermes/Public/IntlPlugin.h"

#include <jsi/jsi.h>

#include <atomic>
#include <string>

#ifndef FAKE_INTERFACE_DELTA
#define FAKE_INTERFACE_DELTA 0
#endif
#ifndef FAKE_BYTECODE_DELTA
#define FAKE_BYTECODE_DELTA 0
#endif

using namespace facebook;

namespace {

/// How many times resolve_defaults detected the locales in this process.
std::atomic<int> detections{0};

/// Fill empty locales with "x-detected-<n>" and an empty time zone with
/// "Fake/Detected", or reject everything in the FAKE_REJECT_DEFAULTS variant.
std::string resolveDefaults(hermes_intl_config *config) {
#ifdef FAKE_REJECT_DEFAULTS
  (void)config;
  return "fake plugin rejects every default";
#else
  if (config->default_locales.empty())
    config->default_locales.push_back(
        "x-detected-" + std::to_string(++detections));
  if (config->default_time_zone.empty())
    config->default_time_zone = "Fake/Detected";
  return "";
#endif
}

/// Native state owned by the plugin, to check NativeState across the
/// boundary.
class FakeState final : public jsi::NativeState {
 public:
  /// The value __fakeIntlRead reports.
  int value = 42;
};

/// Install the host functions that exercise the binary contract:
///   __fakeIntlThrow()       throws jsi::JSError from plugin code; the host
///                           must catch it as JSError (the JS sees the
///                           message, not "Exception in HostFunction").
///   __fakeIntlAttach(o)     attaches a FakeState to o.
///   __fakeIntlRead(o)       FakeState::value via getNativeState<FakeState>,
///                           or -1 if hasNativeState<FakeState> is false.
void installContractChecks(jsi::Runtime &rt) {
  auto define = [&rt](
                    const char *name,
                    unsigned params,
                    jsi::HostFunctionType fn) {
    rt.global().setProperty(
        rt,
        name,
        jsi::Function::createFromHostFunction(
            rt, jsi::PropNameID::forAscii(rt, name), params, std::move(fn)));
  };
  define(
      "__fakeIntlThrow",
      0,
      [](jsi::Runtime &rt, const jsi::Value &, const jsi::Value *, size_t)
          -> jsi::Value { throw jsi::JSError(rt, "fake JSError"); });
  define(
      "__fakeIntlAttach",
      1,
      [](jsi::Runtime &rt, const jsi::Value &, const jsi::Value *args, size_t)
          -> jsi::Value {
        args[0].asObject(rt).setNativeState(rt, std::make_shared<FakeState>());
        return jsi::Value::undefined();
      });
  define(
      "__fakeIntlRead",
      1,
      [](jsi::Runtime &rt, const jsi::Value &, const jsi::Value *args, size_t)
          -> jsi::Value {
        jsi::Object o = args[0].asObject(rt);
        if (!o.hasNativeState<FakeState>(rt))
          return jsi::Value(-1);
        return jsi::Value(o.getNativeState<FakeState>(rt)->value);
      });
}

/// Install the contract checks and set globalThis.__fakeIntlPlugin to
/// {locales, timeZone, detections}, or throw in the FAKE_THROWING_INSTALL
/// variant.
void install(jsi::Runtime &rt, const hermes_intl_config *config) {
#ifdef FAKE_THROWING_INSTALL
  throw jsi::JSINativeException("fake plugin setup failure");
#endif
  installContractChecks(rt);
  jsi::Array locales(rt, config->default_locales.size());
  for (size_t i = 0; i < config->default_locales.size(); ++i)
    locales.setValueAtIndex(
        rt, i, jsi::String::createFromUtf8(rt, config->default_locales[i]));
  jsi::Object info(rt);
  info.setProperty(rt, "locales", locales);
  info.setProperty(
      rt,
      "timeZone",
      jsi::String::createFromUtf8(rt, config->default_time_zone));
  info.setProperty(rt, "detections", detections.load());
  rt.global().setProperty(rt, "__fakeIntlPlugin", info);
}

/// The entry's flags: this build's JSI_UNSTABLE setting, flipped in the
/// FAKE_FLIP_FLAGS variant.
uint32_t flags() {
#ifdef JSI_UNSTABLE
  uint32_t f = HERMES_INTL_FLAG_JSI_UNSTABLE;
#else
  uint32_t f = 0;
#endif
#ifdef FAKE_FLIP_FLAGS
  f ^= HERMES_INTL_FLAG_JSI_UNSTABLE;
#endif
  return f;
}

/// The entry hermes_intl_get_entry returns.
const hermes_intl_entry kEntry{
    sizeof(hermes_intl_entry),
    HERMES_INTL_PLUGIN_INTERFACE_VERSION + FAKE_INTERFACE_DELTA,
    ::hermes::hbc::BYTECODE_VERSION + FAKE_BYTECODE_DELTA,
    flags(),
    resolveDefaults,
    install};

} // namespace

#ifndef FAKE_NO_ENTRY
/// The plugin's entry point; null in the FAKE_NULL_ENTRY variant.
extern "C" __attribute__((visibility("default"))) const hermes_intl_entry *
hermes_intl_get_entry() {
#ifdef FAKE_NULL_ENTRY
  return nullptr;
#else
  return &kEntry;
#endif
}
#endif

#ifdef FAKE_NEEDS_DEP
/// Defined in libfake_intl_dep.so.
extern "C" int fake_intl_dep();
/// A real use of fake_intl_dep, so the linker keeps the dependency.
extern "C" __attribute__((visibility("default"))) int fake_intl_use_dep() {
  return fake_intl_dep();
}
#endif
