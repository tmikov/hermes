/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// wabt headers use #if on macros that may not be defined, triggering -Wundef.
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wundef"
#include "wabt/binary-reader.h"
#pragma GCC diagnostic pop

#include "gtest/gtest.h"

namespace {

/// Placeholder test to verify the WasmFrontend test target builds and links.
TEST(WasmCompileTest, Placeholder) {
  EXPECT_TRUE(true);
}

} // namespace
