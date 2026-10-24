/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The NativeHelpers.def expansions, with a helper of this file's own rather
// than one of the plugin's, in a real runtime.

#include "NativeHelpers.h"

#include <hermes/hermes.h>
#include <jsi/jsi.h>

#include <gtest/gtest.h>

using namespace facebook;
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
} // namespace helpers

TEST(NativeHelpersTest, ZeroArgumentHelper) {
  auto rt =
      facebook::hermes::makeHermesRuntime(::hermes::vm::RuntimeConfig::Builder()
                                              .withEnableIntlPlugin(false)
                                              .build());
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

} // namespace
