/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/IR/IRTypeContext.h"

#include "gtest/gtest.h"

using namespace hermes;

namespace {

TEST(IRTypeContextTest, LeafKinds) {
  IRTypeContext ctx;

  EXPECT_EQ(ctx.getKind(kNoTypeId), TypeKind::NoType);
  EXPECT_EQ(ctx.getKind(kEmptyId), TypeKind::Empty);
  EXPECT_EQ(ctx.getKind(kUninitId), TypeKind::Uninit);
  EXPECT_EQ(ctx.getKind(kUndefinedId), TypeKind::Undefined);
  EXPECT_EQ(ctx.getKind(kNullId), TypeKind::Null);
  EXPECT_EQ(ctx.getKind(kBooleanId), TypeKind::Boolean);
  EXPECT_EQ(ctx.getKind(kStringId), TypeKind::String);
  EXPECT_EQ(ctx.getKind(kNumberId), TypeKind::Number);
  EXPECT_EQ(ctx.getKind(kBigIntId), TypeKind::BigInt);
  EXPECT_EQ(ctx.getKind(kSymbolId), TypeKind::Symbol);
  EXPECT_EQ(ctx.getKind(kEnvironmentId), TypeKind::Environment);
  EXPECT_EQ(ctx.getKind(kPrivateNameId), TypeKind::PrivateName);
  EXPECT_EQ(ctx.getKind(kFunctionCodeId), TypeKind::FunctionCode);
  EXPECT_EQ(ctx.getKind(kObjectId), TypeKind::Object);
  EXPECT_EQ(ctx.getKind(kBits32Id), TypeKind::Bits32);
}

TEST(IRTypeContextTest, AnyTypeIsUnion) {
  IRTypeContext ctx;

  EXPECT_EQ(ctx.getKind(kAnyTypeId), TypeKind::Union);
  auto arms = ctx.getUnionArms(kAnyTypeId);
  // AnyType = Undefined | Null | Boolean | String | Number | BigInt | Symbol |
  // Object.
  EXPECT_EQ(arms.size(), 8u);

  // Verify all expected arms are present.
  auto contains = [&](uint32_t id) {
    for (auto arm : arms)
      if (arm == id)
        return true;
    return false;
  };
  EXPECT_TRUE(contains(kUndefinedId));
  EXPECT_TRUE(contains(kNullId));
  EXPECT_TRUE(contains(kBooleanId));
  EXPECT_TRUE(contains(kStringId));
  EXPECT_TRUE(contains(kNumberId));
  EXPECT_TRUE(contains(kBigIntId));
  EXPECT_TRUE(contains(kSymbolId));
  EXPECT_TRUE(contains(kObjectId));

  // Should NOT contain internal types.
  EXPECT_FALSE(contains(kEmptyId));
  EXPECT_FALSE(contains(kUninitId));
  EXPECT_FALSE(contains(kEnvironmentId));
  EXPECT_FALSE(contains(kPrivateNameId));
  EXPECT_FALSE(contains(kFunctionCodeId));
  EXPECT_FALSE(contains(kBits32Id));
}

TEST(IRTypeContextTest, NumericIsUnion) {
  IRTypeContext ctx;

  EXPECT_EQ(ctx.getKind(kNumericId), TypeKind::Union);
  auto arms = ctx.getUnionArms(kNumericId);
  EXPECT_EQ(arms.size(), 2u);

  bool hasNumber = false, hasBigInt = false;
  for (auto arm : arms) {
    if (arm == kNumberId)
      hasNumber = true;
    if (arm == kBigIntId)
      hasBigInt = true;
  }
  EXPECT_TRUE(hasNumber);
  EXPECT_TRUE(hasBigInt);
}

TEST(IRTypeContextTest, AnyEmptyUninitIsUnion) {
  IRTypeContext ctx;

  EXPECT_EQ(ctx.getKind(kAnyEmptyUninitId), TypeKind::Union);
  auto arms = ctx.getUnionArms(kAnyEmptyUninitId);
  // All AnyType arms + Empty + Uninit = 10.
  EXPECT_EQ(arms.size(), 10u);

  auto contains = [&](uint32_t id) {
    for (auto arm : arms)
      if (arm == id)
        return true;
    return false;
  };
  EXPECT_TRUE(contains(kEmptyId));
  EXPECT_TRUE(contains(kUninitId));
  EXPECT_TRUE(contains(kUndefinedId));
  EXPECT_TRUE(contains(kNullId));
  EXPECT_TRUE(contains(kBooleanId));
  EXPECT_TRUE(contains(kStringId));
  EXPECT_TRUE(contains(kNumberId));
  EXPECT_TRUE(contains(kBigIntId));
  EXPECT_TRUE(contains(kSymbolId));
  EXPECT_TRUE(contains(kObjectId));
}

TEST(IRTypeContextTest, NullOrUndefIsUnion) {
  IRTypeContext ctx;

  EXPECT_EQ(ctx.getKind(kNullOrUndefId), TypeKind::Union);
  auto arms = ctx.getUnionArms(kNullOrUndefId);
  EXPECT_EQ(arms.size(), 2u);

  bool hasUndefined = false, hasNull = false;
  for (auto arm : arms) {
    if (arm == kUndefinedId)
      hasUndefined = true;
    if (arm == kNullId)
      hasNull = true;
  }
  EXPECT_TRUE(hasUndefined);
  EXPECT_TRUE(hasNull);
}

TEST(IRTypeContextTest, ReservedSlots) {
  IRTypeContext ctx;
  // Padding slots between the last well-known union and kFirstDynamicId
  // should be NoType placeholders.
  for (uint32_t i = kNullOrUndefId + 1; i < kFirstDynamicId; ++i) {
    EXPECT_EQ(ctx.getKind(i), TypeKind::NoType)
        << "Reserved slot " << i << " should be NoType";
  }
}

} // anonymous namespace
