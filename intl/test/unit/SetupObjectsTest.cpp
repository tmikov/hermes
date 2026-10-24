/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The two objects the plugin's C++ passes to the JS setup function, built in
// a real runtime. No JS path reads a property they lack, so a prototype other
// than null cannot be seen through Intl; it is checked here directly.

#include "NativeHelpers.h"

#include <hermes/hermes.h>
#include <jsi/jsi.h>

#include <gtest/gtest.h>

using namespace facebook;

namespace {

/// A runtime without the plugin, whose %Object.prototype% has a getter for
/// "absent" that returns "inherited".
std::unique_ptr<jsi::Runtime> makeTamperedRuntime() {
  std::unique_ptr<jsi::Runtime> rt =
      facebook::hermes::makeHermesRuntime(::hermes::vm::RuntimeConfig::Builder()
                                              .withEnableIntlPlugin(false)
                                              .build());
  rt->evaluateJavaScript(
      std::make_shared<jsi::StringBuffer>(
          "Object.defineProperty(Object.prototype, 'absent', "
          "{get: function () { return 'inherited'; }});"),
      "tamper.js");
  return rt;
}

/// Whether \p obj's prototype is null.
bool hasNullPrototype(jsi::Runtime &rt, const jsi::Object &obj) {
  jsi::Function getPrototypeOf =
      rt.global()
          .getPropertyAsObject(rt, "Object")
          .getPropertyAsFunction(rt, "getPrototypeOf");
  return getPrototypeOf.call(rt, obj).isNull();
}

TEST(SetupObjectsTest, NativeHelpersHaveNullPrototype) {
  auto rt = makeTamperedRuntime();
  jsi::Object helpers = hermes_intl::makeNativeHelpers(*rt);
  EXPECT_TRUE(hasNullPrototype(*rt, helpers));
  EXPECT_TRUE(helpers.getProperty(*rt, "absent").isUndefined());
  EXPECT_TRUE(helpers.getProperty(*rt, "canonicalizeLocale").isObject());
}

TEST(SetupObjectsTest, DefaultsHaveNullPrototype) {
  auto rt = makeTamperedRuntime();
  jsi::Object defaults =
      hermes_intl::makeSetupDefaults(*rt, {"fr-FR", "de"}, "Europe/Paris");
  EXPECT_TRUE(hasNullPrototype(*rt, defaults));
  EXPECT_TRUE(defaults.getProperty(*rt, "absent").isUndefined());
  jsi::Array locales =
      defaults.getPropertyAsObject(*rt, "locales").getArray(*rt);
  ASSERT_EQ(locales.size(*rt), 2u);
  EXPECT_EQ(locales.getValueAtIndex(*rt, 0).getString(*rt).utf8(*rt), "fr-FR");
  EXPECT_EQ(locales.getValueAtIndex(*rt, 1).getString(*rt).utf8(*rt), "de");
  EXPECT_EQ(
      defaults.getProperty(*rt, "timeZone").getString(*rt).utf8(*rt),
      "Europe/Paris");
}

} // namespace
