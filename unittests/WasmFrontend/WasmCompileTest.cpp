/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/WasmFrontend/WasmTypes.h"

#include "gtest/gtest.h"

using namespace hermes::wasm;

namespace {

TEST(WasmTypesTest, ValTypeEncoding) {
  // Verify that the enum values match the Wasm binary encoding.
  EXPECT_EQ(static_cast<uint8_t>(WasmValType::I32), 0x7F);
  EXPECT_EQ(static_cast<uint8_t>(WasmValType::I64), 0x7E);
  EXPECT_EQ(static_cast<uint8_t>(WasmValType::F32), 0x7D);
  EXPECT_EQ(static_cast<uint8_t>(WasmValType::F64), 0x7C);
  EXPECT_EQ(static_cast<uint8_t>(WasmValType::V128), 0x7B);
  EXPECT_EQ(static_cast<uint8_t>(WasmValType::FuncRef), 0x70);
  EXPECT_EQ(static_cast<uint8_t>(WasmValType::ExternRef), 0x6F);
}

TEST(WasmTypesTest, FuncType) {
  WasmFuncType ft;
  ft.params = {WasmValType::I32, WasmValType::I32};
  ft.results = {WasmValType::I32};

  EXPECT_EQ(ft.params.size(), 2u);
  EXPECT_EQ(ft.results.size(), 1u);
  EXPECT_EQ(ft.params[0], WasmValType::I32);
  EXPECT_EQ(ft.params[1], WasmValType::I32);
  EXPECT_EQ(ft.results[0], WasmValType::I32);
}

TEST(WasmTypesTest, FuncTypeEmpty) {
  // A function with no params and no results (void -> void).
  WasmFuncType ft;
  EXPECT_TRUE(ft.params.empty());
  EXPECT_TRUE(ft.results.empty());
}

TEST(WasmTypesTest, FuncTypeMultiReturn) {
  WasmFuncType ft;
  ft.params = {WasmValType::F64};
  ft.results = {WasmValType::I32, WasmValType::F64};

  EXPECT_EQ(ft.params.size(), 1u);
  EXPECT_EQ(ft.results.size(), 2u);
  EXPECT_EQ(ft.results[0], WasmValType::I32);
  EXPECT_EQ(ft.results[1], WasmValType::F64);
}

TEST(WasmTypesTest, LimitsDefaults) {
  WasmLimits lim;
  EXPECT_EQ(lim.initial, 0u);
  EXPECT_EQ(lim.maximum, UINT32_MAX);
  EXPECT_FALSE(lim.hasMaximum);
}

TEST(WasmTypesTest, LimitsWithMax) {
  WasmLimits lim;
  lim.initial = 1;
  lim.maximum = 10;
  lim.hasMaximum = true;

  EXPECT_EQ(lim.initial, 1u);
  EXPECT_EQ(lim.maximum, 10u);
  EXPECT_TRUE(lim.hasMaximum);
}

TEST(WasmTypesTest, TableType) {
  WasmTableType tt;
  EXPECT_EQ(tt.elemType, WasmValType::FuncRef);
  EXPECT_EQ(tt.limits.initial, 0u);
  EXPECT_FALSE(tt.limits.hasMaximum);

  tt.elemType = WasmValType::ExternRef;
  tt.limits.initial = 5;
  tt.limits.maximum = 100;
  tt.limits.hasMaximum = true;

  EXPECT_EQ(tt.elemType, WasmValType::ExternRef);
  EXPECT_EQ(tt.limits.initial, 5u);
  EXPECT_EQ(tt.limits.maximum, 100u);
  EXPECT_TRUE(tt.limits.hasMaximum);
}

TEST(WasmTypesTest, MemoryType) {
  WasmMemoryType mt;
  EXPECT_EQ(mt.limits.initial, 0u);
  EXPECT_FALSE(mt.limits.hasMaximum);

  mt.limits.initial = 1;
  mt.limits.maximum = 256;
  mt.limits.hasMaximum = true;

  EXPECT_EQ(mt.limits.initial, 1u);
  EXPECT_EQ(mt.limits.maximum, 256u);
  EXPECT_TRUE(mt.limits.hasMaximum);
}

TEST(WasmTypesTest, GlobalType) {
  WasmGlobalType gt;
  EXPECT_EQ(gt.type, WasmValType::I32);
  EXPECT_FALSE(gt.mutable_);

  gt.type = WasmValType::F64;
  gt.mutable_ = true;

  EXPECT_EQ(gt.type, WasmValType::F64);
  EXPECT_TRUE(gt.mutable_);
}

} // namespace
