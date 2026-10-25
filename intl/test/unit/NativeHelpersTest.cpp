/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The NativeHelpers.def expansions and conversion tags, with helpers of this
// file's own, and the plugin's collator helpers, in a real runtime.

#include "NativeHelpers.h"

#include <hermes/hermes.h>
#include <jsi/jsi.h>

#include <gtest/gtest.h>

#include <variant>

using namespace facebook;
using hermes_intl::Bool;
using hermes_intl::Int;
using hermes_intl::Object;
using hermes_intl::Undefined;
using hermes_intl::Utf16;
using hermes_intl::Utf8;

namespace {

/// The helpers HERMES_INTL_ADD_HELPER finds as helpers::name here.
namespace helpers {
/// A helper with no arguments, declared as the NativeHelpers.def line
/// HERMES_INTL_HELPER(noArguments, Utf8) declares it.
HERMES_INTL_DECLARE_HELPER(noArguments, Utf8)

std::string noArguments(jsi::Runtime &) {
  return "no arguments";
}

/// HERMES_INTL_HELPER(echoUtf16, Utf16, Utf16): its argument.
HERMES_INTL_DECLARE_HELPER(echoUtf16, Utf16, Utf16)

std::u16string echoUtf16(jsi::Runtime &, const std::u16string &s) {
  return s;
}

/// HERMES_INTL_HELPER(objectAndBool, Bool, Object, Bool): its second
/// argument.
HERMES_INTL_DECLARE_HELPER(objectAndBool, Bool, Object, Bool)

bool objectAndBool(jsi::Runtime &, const jsi::Object &, const bool &b) {
  return b;
}

/// HERMES_INTL_HELPER(minusOne, Int): -1.
HERMES_INTL_DECLARE_HELPER(minusOne, Int)

int minusOne(jsi::Runtime &) {
  return -1;
}

/// HERMES_INTL_HELPER(nothing, Undefined): undefined.
HERMES_INTL_DECLARE_HELPER(nothing, Undefined)

std::monostate nothing(jsi::Runtime &) {
  return {};
}
} // namespace helpers

/// A runtime without the plugin.
std::unique_ptr<jsi::Runtime> makeRuntime() {
  return facebook::hermes::makeHermesRuntime(
      ::hermes::vm::RuntimeConfig::Builder()
          .withEnableIntlPlugin(false)
          .build());
}

/// Evaluate \p src in \p rt.
jsi::Value eval(jsi::Runtime &rt, const char *src) {
  return rt.evaluateJavaScript(
      std::make_shared<jsi::StringBuffer>(src), "t.js");
}

/// Expect calling \p fn throws a jsi::JSError whose message contains
/// \p expected.
template <class F>
void expectJSError(F fn, const std::string &expected) {
  try {
    fn();
    ADD_FAILURE() << "expected an Error containing: " << expected;
  } catch (const jsi::JSError &e) {
    EXPECT_NE(e.getMessage().find(expected), std::string::npos)
        << e.getMessage();
  }
}

TEST(NativeHelpersTest, ZeroArgumentHelper) {
  auto rt = makeRuntime();
  jsi::Object object(*rt);
  HERMES_INTL_ADD_HELPER(*rt, object, noArguments, Utf8);
  jsi::Function fn = object.getPropertyAsFunction(*rt, "noArguments");
  EXPECT_EQ(fn.getProperty(*rt, "length").asNumber(), 0);
  EXPECT_EQ(fn.call(*rt).getString(*rt).utf8(*rt), "no arguments");
  // A call with an argument is a plugin bug, reported as an Error.
  try {
    fn.call(*rt, 1);
    FAIL() << "expected an Error";
  } catch (const jsi::JSError &e) {
    EXPECT_NE(
        e.getMessage().find("noArguments: wrong argument count"),
        std::string::npos)
        << e.getMessage();
  }
}

TEST(NativeHelpersTest, Utf16RoundTrip) {
  auto rt = makeRuntime();
  jsi::Object h(*rt);
  HERMES_INTL_ADD_HELPER(*rt, h, echoUtf16, Utf16, Utf16);
  rt->global().setProperty(*rt, "h", h);
  // Two unpaired surrogates and a supplementary character (U+10428), which
  // a conversion through UTF-8 would not preserve.
  jsi::Value units = eval(
      *rt,
      "var s = h.echoUtf16('a\\uD800b\\uDC00𐐨');"
      "var units = [];"
      "for (var i = 0; i < s.length; ++i) units.push(s.charCodeAt(i));"
      "units.join(',');");
  EXPECT_EQ(units.getString(*rt).utf8(*rt), "97,55296,98,56320,55297,56360");
}

TEST(NativeHelpersTest, ArgumentTypesAreChecked) {
  auto rt = makeRuntime();
  jsi::Object h(*rt);
  HERMES_INTL_ADD_HELPER(*rt, h, objectAndBool, Bool, Object, Bool);
  HERMES_INTL_ADD_HELPER(*rt, h, echoUtf16, Utf16, Utf16);
  rt->global().setProperty(*rt, "h", h);
  EXPECT_TRUE(eval(*rt, "h.objectAndBool({}, true)").getBool());
  expectJSError(
      [&] { eval(*rt, "h.objectAndBool(1, true)"); },
      "objectAndBool: expected an object");
  expectJSError(
      [&] { eval(*rt, "h.objectAndBool({}, 1)"); },
      "objectAndBool: expected a boolean");
  expectJSError(
      [&] { eval(*rt, "h.echoUtf16(1)"); }, "echoUtf16: expected a string");
}

TEST(NativeHelpersTest, IntAndUndefinedResults) {
  auto rt = makeRuntime();
  jsi::Object h(*rt);
  HERMES_INTL_ADD_HELPER(*rt, h, minusOne, Int);
  HERMES_INTL_ADD_HELPER(*rt, h, nothing, Undefined);
  rt->global().setProperty(*rt, "h", h);
  EXPECT_TRUE(eval(*rt, "h.minusOne() === -1").getBool());
  EXPECT_TRUE(eval(*rt, "h.nothing() === undefined").getBool());
}

/// Native state that is not a CollatorState.
class OtherState : public jsi::NativeState {};

/// A runtime whose global "native" is the plugin's helpers object.
std::unique_ptr<jsi::Runtime> makeRuntimeWithPluginHelpers() {
  auto rt = makeRuntime();
  rt->global().setProperty(*rt, "native", hermes_intl::makeNativeHelpers(*rt));
  return rt;
}

TEST(NativeHelpersTest, CollatorStateFreedWithRuntime) {
  // The runtime's destruction frees the collator's CollatorState; under
  // ASan, LeakSanitizer fails the test binary if it does not.
  auto rt = makeRuntimeWithPluginHelpers();
  EXPECT_TRUE(eval(
                  *rt,
                  "var c = {};"
                  "native.collatorInit(c, 'en', 'default', 'variant', "
                  "false, false, 'false') === undefined")
                  .getBool());
  EXPECT_EQ(eval(*rt, "native.collatorCompare(c, 'a', 'b')").getNumber(), -1);
  rt.reset();
}

TEST(NativeHelpersTest, CollatorHelpersRejectMisuse) {
  auto rt = makeRuntimeWithPluginHelpers();
  expectJSError(
      [&] { eval(*rt, "native.collatorCompare({}, 'a', 'b')"); },
      "Intl internal error");
  // An object with native state of another kind.
  jsi::Object other(*rt);
  other.setNativeState(*rt, std::make_shared<OtherState>());
  rt->global().setProperty(*rt, "other", other);
  expectJSError(
      [&] { eval(*rt, "native.collatorCompare(other, 'a', 'b')"); },
      "Intl internal error");
  // The second collatorInit on one object, after a first that succeeds.
  eval(
      *rt,
      "var c = {};"
      "native.collatorInit(c, 'en', 'default', 'variant', false, false, "
      "'false');");
  expectJSError(
      [&] {
        eval(
            *rt,
            "native.collatorInit(c, 'en', 'default', 'variant', false, "
            "false, 'false');");
      },
      "Intl internal error");
  expectJSError(
      [&] {
        eval(
            *rt,
            "native.collatorInit({}, 'en', 'default', 'Base', false, false, "
            "'false');");
      },
      "Intl internal error");
}

TEST(NativeHelpersTest, CollatorLocaleDefaults) {
  auto rt = makeRuntimeWithPluginHelpers();
  EXPECT_EQ(
      eval(*rt, "native.collatorLocaleDefaults('da')").getString(*rt).utf8(*rt),
      "upper false");
  EXPECT_EQ(
      eval(*rt, "native.collatorLocaleDefaults('th')").getString(*rt).utf8(*rt),
      "false true");
  EXPECT_EQ(
      eval(*rt, "native.collatorLocaleDefaults('en')").getString(*rt).utf8(*rt),
      "false false");
  expectJSError(
      [&] { eval(*rt, "native.collatorLocaleDefaults('en_US')"); },
      "Intl internal error: collatorLocaleDefaults: locale does not parse");
}

TEST(NativeHelpersTest, CaseMapKeepsUnpairedSurrogates) {
  auto rt = makeRuntimeWithPluginHelpers();
  auto result = eval(*rt, "native.caseMap('a\\uD800b', 'und', true)");
  rt->global().setProperty(*rt, "r", result);
  EXPECT_EQ(eval(*rt, "r.length").getNumber(), 3);
  EXPECT_EQ(eval(*rt, "r.charCodeAt(0)").getNumber(), 0x41);
  EXPECT_EQ(eval(*rt, "r.charCodeAt(1)").getNumber(), 0xD800);
  EXPECT_EQ(eval(*rt, "r.charCodeAt(2)").getNumber(), 0x42);
  expectJSError(
      [&] { eval(*rt, "native.caseMap('a', 'en_US', true)"); },
      "Intl internal error: caseMap: language does not parse");
}

} // namespace
