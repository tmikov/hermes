/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/VM/static_h.h"

#include "gtest/gtest.h"

#include <string>

namespace {

#define SHABI_STR_HELPER(x) #x
#define SHABI_STR(x) SHABI_STR_HELPER(x)

/// Generated C allocates a UnitData that embeds SHUnit by value, so any
/// change to the SHUnit layout makes an existing object file the wrong size.
/// The model symbol already encodes the heap configuration so that a
/// mismatched runtime fails at link time; it must encode the unit layout for
/// the same reason.
TEST(SHUnitABITest, ModelEncodesUnitLayoutVersion) {
  std::string model = SHABI_STR(HERMESVM_MODEL);
  EXPECT_NE(model.find("_u"), std::string::npos)
      << "HERMESVM_MODEL is \"" << model
      << "\", which carries no SHUnit layout version";
}

} // namespace
