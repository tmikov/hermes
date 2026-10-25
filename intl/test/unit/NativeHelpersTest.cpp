/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The NativeHelpers.def expansions and conversion tags, with helpers of this
// file's own, and the plugin's collator and plural rules helpers, in a real
// runtime.

#include "NativeHelpers.h"

#include <hermes/hermes.h>
#include <jsi/jsi.h>

#include <gtest/gtest.h>

#include <string>
#include <variant>
#include <vector>

using namespace facebook;
using hermes_intl::Bool;
using hermes_intl::Int;
using hermes_intl::Object;
using hermes_intl::Undefined;
using hermes_intl::Utf16;
using hermes_intl::Utf8;
using hermes_intl::Utf8List;

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

/// HERMES_INTL_HELPER(threeStrings, Utf8List): {"a", "", "b"}.
HERMES_INTL_DECLARE_HELPER(threeStrings, Utf8List)

std::vector<std::string> threeStrings(jsi::Runtime &) {
  return {"a", "", "b"};
}

/// HERMES_INTL_HELPER(oddStrings, Utf8List): strings with the characters
/// JSON escapes, U+0000, and characters outside ASCII.
HERMES_INTL_DECLARE_HELPER(oddStrings, Utf8List)

std::vector<std::string> oddStrings(jsi::Runtime &) {
  return {
      "q\"b\\s", std::string("\0x", 2), "\t\x1f", "\xc3\xa9\xf0\x9d\x9f\x8e"};
}

/// HERMES_INTL_HELPER(echoInt, Int, Int): its argument.
HERMES_INTL_DECLARE_HELPER(echoInt, Int, Int)

int echoInt(jsi::Runtime &, const int &i) {
  return i;
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

TEST(NativeHelpersTest, Utf8ListResult) {
  auto rt = makeRuntime();
  jsi::Object h(*rt);
  HERMES_INTL_ADD_HELPER(*rt, h, threeStrings, Utf8List);
  rt->global().setProperty(*rt, "h", h);
  EXPECT_TRUE(eval(
                  *rt,
                  "var r = h.threeStrings();"
                  "Array.isArray(r) && r.length === 3 && r[0] === 'a' && "
                  "r[1] === '' && r[2] === 'b'")
                  .getBool());
  // A new array each call.
  EXPECT_TRUE(eval(*rt, "h.threeStrings() !== h.threeStrings()").getBool());
  // The characters JSON escapes, U+0000 and non-ASCII characters arrive
  // as they are.
  HERMES_INTL_ADD_HELPER(*rt, h, oddStrings, Utf8List);
  EXPECT_TRUE(eval(
                  *rt,
                  "var o = h.oddStrings();"
                  "o.length === 4 && o[0] === 'q\"b\\\\s' && "
                  "o[1] === '\\u0000x' && o[2] === '\\t\\u001f' && "
                  "o[3] === '\\u00e9\\ud835\\udfce'")
                  .getBool());
  // Setters for indices on Array.prototype are not run: the elements are
  // defined, not assigned.
  EXPECT_TRUE(eval(
                  *rt,
                  "var hits = 0;"
                  "for (var i = 0; i < 3; i++)"
                  "  Object.defineProperty(Array.prototype, i, {"
                  "    set() { hits++; }, configurable: true});"
                  "r = h.threeStrings();"
                  "for (var i = 0; i < 3; i++) delete Array.prototype[i];"
                  "hits === 0 && r.length === 3 && r[0] === 'a' && "
                  "r[1] === '' && r[2] === 'b'")
                  .getBool());
}

TEST(NativeHelpersTest, IntArgumentsAreChecked) {
  auto rt = makeRuntime();
  jsi::Object h(*rt);
  HERMES_INTL_ADD_HELPER(*rt, h, echoInt, Int, Int);
  rt->global().setProperty(*rt, "h", h);
  // Each of these must not throw; a throw is reported as a failure of this
  // test rather than ending the test binary.
  for (const char *src :
       {"h.echoInt(5) === 5",
        "h.echoInt(-2147483648) === -2147483648",
        "h.echoInt(2147483647) === 2147483647",
        // -0 is accepted, as 0.
        "Object.is(h.echoInt(-0), 0)"}) {
    try {
      EXPECT_TRUE(eval(*rt, src).getBool()) << src;
    } catch (const jsi::JSError &e) {
      ADD_FAILURE() << src << ": " << e.getMessage();
    }
  }
  for (const char *src :
       {"h.echoInt(1.5)",
        "h.echoInt('1')",
        "h.echoInt(2 ** 40)",
        "h.echoInt(2147483648)",
        "h.echoInt(-2147483649)",
        "h.echoInt(NaN)",
        "h.echoInt(Infinity)"})
    expectJSError(
        [&] { eval(*rt, src); }, "echoInt: expected an integer in int range");
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

TEST(NativeHelpersTest, PluralRulesStateFreedWithRuntime) {
  // The runtime's destruction frees the PluralRulesState; under ASan,
  // LeakSanitizer fails the test binary if it does not.
  auto rt = makeRuntimeWithPluginHelpers();
  EXPECT_TRUE(
      eval(
          *rt,
          "var p = {}; native.pluralRulesInit(p, 'en', false) === undefined")
          .getBool());
  EXPECT_EQ(
      eval(*rt, "native.pluralSelect(p, '1', 0)").getString(*rt).utf8(*rt),
      "one");
  rt.reset();
}

TEST(NativeHelpersTest, PluralHelpers) {
  auto rt = makeRuntimeWithPluginHelpers();
  eval(*rt, "var fr = {}; native.pluralRulesInit(fr, 'fr', false);");
  auto str = [&](const char *src) {
    return eval(*rt, src).getString(*rt).utf8(*rt);
  };
  EXPECT_EQ(str("native.pluralSelect(fr, '1.5', 6)"), "many");
  EXPECT_EQ(str("native.pluralSelect(fr, '1', -6)"), "one");
  EXPECT_EQ(str("native.pluralSelect(fr, '2', 0)"), "other");
  EXPECT_EQ(str("native.pluralSelectRange(fr, 'one', 'one')"), "one");
  EXPECT_EQ(str("native.pluralCategories(fr).join()"), "one,many,other");
  eval(*rt, "var en = {}; native.pluralRulesInit(en, 'en', true);");
  EXPECT_EQ(str("native.pluralSelect(en, '2', 0)"), "two");
  EXPECT_EQ(str("native.pluralCategories(en).join()"), "one,two,few,other");
}

TEST(NativeHelpersTest, PluralHelpersRejectMisuse) {
  auto rt = makeRuntimeWithPluginHelpers();
  expectJSError(
      [&] { eval(*rt, "native.pluralSelect({}, '1', 0)"); },
      "Intl internal error");
  // An object with native state of another kind.
  jsi::Object other(*rt);
  other.setNativeState(*rt, std::make_shared<OtherState>());
  rt->global().setProperty(*rt, "other", other);
  expectJSError(
      [&] { eval(*rt, "native.pluralCategories(other)"); },
      "Intl internal error");
  eval(*rt, "var p = {}; native.pluralRulesInit(p, 'en', false);");
  expectJSError(
      [&] { eval(*rt, "native.pluralRulesInit(p, 'en', false)"); },
      "Intl internal error");
  expectJSError(
      [&] { eval(*rt, "native.pluralSelect(p, '1e3', 0)"); },
      "Intl internal error");
  expectJSError(
      [&] { eval(*rt, "native.pluralSelectRange(p, 'one', 'lots')"); },
      "Intl internal error");
  expectJSError(
      [&] { eval(*rt, "native.pluralRulesInit({}, 'en_US', false)"); },
      "Intl internal error");
}

TEST(NativeHelpersTest, NumberHelpers) {
  auto rt = makeRuntimeWithPluginHelpers();
  auto str = [&](const char *src) {
    return eval(*rt, src).getString(*rt).utf8(*rt);
  };
  EXPECT_EQ(str("String(native.numberSymbols('en', 'latn').length)"), "21");
  EXPECT_EQ(
      str("native.currencySeparators('pt-PT', 'latn', 'PTE').join(' ')"),
      "$ ,");
  EXPECT_EQ(
      str("native.numberSymbols('de', 'latn').slice(0, 4).join(' ')"),
      ", . - +");
  EXPECT_EQ(
      str("JSON.stringify(native.currencyPatterns("
          "'en', 'latn', 'USD', 'symbol', true))"),
      "[\"{currencyPrefix}{number}\",\"{plusSign}{currencyPrefix}{number}\","
      "\"({currencyPrefix}{number})\",\"$\",\"{currencyPrefix}{number}\","
      "\"{plusSign}{currencyPrefix}{number}\",\"({currencyPrefix}{number})\"]");
  EXPECT_EQ(
      str("native.currencyPatterns('en', 'latn', 'USD', 'name', false)[5]"),
      "{0} US dollars");
  // en's 12 short currency magnitudes; 10^3 starts with exponent 3 and the
  // zero category's compact pattern and sign patterns.
  EXPECT_EQ(
      str("JSON.stringify(native.compactCurrencyPatterns("
          "'en', 'latn', 'USD', 'code', true).slice(0, 7))"),
      "[\"12\",\"3\",\"3\",\"{0}K\",\"{currencyPrefix}\u00a0{number}\","
      "\"{plusSign}{currencyPrefix}\u00a0{number}\","
      "\"({currencyPrefix}\u00a0{number})\"]");
  // Empty entries survive: en short's first entry has no exact-0 or exact-1
  // pattern.
  EXPECT_EQ(
      str("JSON.stringify(native.compactPatterns('en', 'latn', false)"
          ".slice(0, 6))"),
      "[\"4\",\"3\",\"3\",\"\",\"\",\"{0}K\"]");
  EXPECT_EQ(
      str("JSON.stringify(native.unitPatterns('en', 'mile', 'long'))"),
      "[\"{0} miles\",\"{0} mile\",\"{0} miles\",\"{0} miles\","
      "\"{0} miles\",\"{0} miles\"]");
  EXPECT_EQ(
      str("native.unitPatterns('en', 'kilobyte-per-hour', 'short')[5]"),
      "{0} kB/h");
  EXPECT_EQ(eval(*rt, "native.currencyDigits('JPY')").getNumber(), 0);
  EXPECT_EQ(eval(*rt, "native.currencyDigits('ABC')").getNumber(), 2);
  for (const char *bad :
       {"native.numberSymbols('en_US', 'latn')",
        "native.numberSymbols('en', 'Latn')",
        "native.currencyPatterns('en', 'latn', 'usd', 'symbol', false)",
        "native.currencyPatterns('en', 'latn', 'USD', 'long', false)",
        "native.compactCurrencyPatterns('en', 'latn', 'USD', 'name', false)",
        "native.currencySeparators('en', 'latn', 'usd')",
        "native.compactCurrencyPatterns('en_US', 'latn', 'USD', 'code', false)",
        "native.compactPatterns('en_US', 'latn', true)",
        "native.unitPatterns('en', 'furlong', 'long')",
        "native.unitPatterns('en', 'meter', 'Long')",
        "native.currencyDigits('usd')"})
    expectJSError([&] { eval(*rt, bad); }, "Intl internal error");
}

} // namespace
