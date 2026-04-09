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

TEST(IRTypeContextTest, CanBeNumberLeaf) {
  IRTypeContext ctx;

  EXPECT_TRUE(ctx.canBeNumber(kNumberId));
  EXPECT_FALSE(ctx.canBeNumber(kStringId));
  EXPECT_FALSE(ctx.canBeNumber(kObjectId));
  EXPECT_FALSE(ctx.canBeNumber(kNoTypeId));
  EXPECT_FALSE(ctx.canBeNumber(kBooleanId));
}

TEST(IRTypeContextTest, CanBeNumberUnion) {
  IRTypeContext ctx;

  // AnyType is a union containing Number.
  EXPECT_TRUE(ctx.canBeNumber(kAnyTypeId));
  // Numeric = Number | BigInt.
  EXPECT_TRUE(ctx.canBeNumber(kNumericId));
  // AnyEmptyUninit contains Number.
  EXPECT_TRUE(ctx.canBeNumber(kAnyEmptyUninitId));
  // NullOrUndef does not contain Number.
  EXPECT_FALSE(ctx.canBeNumber(kNullOrUndefId));
}

TEST(IRTypeContextTest, CanBeOtherKinds) {
  IRTypeContext ctx;

  EXPECT_TRUE(ctx.canBeString(kStringId));
  EXPECT_FALSE(ctx.canBeString(kNumberId));
  EXPECT_TRUE(ctx.canBeString(kAnyTypeId));

  EXPECT_TRUE(ctx.canBeObject(kObjectId));
  EXPECT_FALSE(ctx.canBeObject(kNumberId));
  EXPECT_TRUE(ctx.canBeObject(kAnyTypeId));

  EXPECT_TRUE(ctx.canBeNull(kNullId));
  EXPECT_FALSE(ctx.canBeNull(kNumberId));
  EXPECT_TRUE(ctx.canBeNull(kAnyTypeId));
  EXPECT_TRUE(ctx.canBeNull(kNullOrUndefId));

  EXPECT_TRUE(ctx.canBeUndefined(kUndefinedId));
  EXPECT_FALSE(ctx.canBeUndefined(kNumberId));
  EXPECT_TRUE(ctx.canBeUndefined(kAnyTypeId));
  EXPECT_TRUE(ctx.canBeUndefined(kNullOrUndefId));

  EXPECT_TRUE(ctx.canBeEmpty(kEmptyId));
  EXPECT_FALSE(ctx.canBeEmpty(kNumberId));
  EXPECT_FALSE(ctx.canBeEmpty(kAnyTypeId));
  EXPECT_TRUE(ctx.canBeEmpty(kAnyEmptyUninitId));

  EXPECT_TRUE(ctx.canBeUninit(kUninitId));
  EXPECT_FALSE(ctx.canBeUninit(kNumberId));
  EXPECT_FALSE(ctx.canBeUninit(kAnyTypeId));
  EXPECT_TRUE(ctx.canBeUninit(kAnyEmptyUninitId));

  EXPECT_TRUE(ctx.canBeBigInt(kBigIntId));
  EXPECT_FALSE(ctx.canBeBigInt(kStringId));
  EXPECT_TRUE(ctx.canBeBigInt(kNumericId));
  EXPECT_TRUE(ctx.canBeBigInt(kAnyTypeId));

  EXPECT_TRUE(ctx.canBeBoolean(kBooleanId));
  EXPECT_FALSE(ctx.canBeBoolean(kStringId));
  EXPECT_TRUE(ctx.canBeBoolean(kAnyTypeId));

  EXPECT_TRUE(ctx.canBeSymbol(kSymbolId));
  EXPECT_FALSE(ctx.canBeSymbol(kStringId));
  EXPECT_TRUE(ctx.canBeSymbol(kAnyTypeId));
}

TEST(IRTypeContextTest, IsNoType) {
  IRTypeContext ctx;

  EXPECT_TRUE(ctx.isNoType(kNoTypeId));
  EXPECT_FALSE(ctx.isNoType(kNumberId));
  EXPECT_FALSE(ctx.isNoType(kAnyTypeId));
}

TEST(IRTypeContextTest, IsPrimitive) {
  IRTypeContext ctx;

  // Primitive kinds: Number, String, BigInt, Null, Undefined, Boolean, Symbol.
  EXPECT_TRUE(ctx.isPrimitive(kNumberId));
  EXPECT_TRUE(ctx.isPrimitive(kStringId));
  EXPECT_TRUE(ctx.isPrimitive(kBigIntId));
  EXPECT_TRUE(ctx.isPrimitive(kNullId));
  EXPECT_TRUE(ctx.isPrimitive(kUndefinedId));
  EXPECT_TRUE(ctx.isPrimitive(kBooleanId));
  EXPECT_TRUE(ctx.isPrimitive(kSymbolId));

  // Object is NOT primitive.
  EXPECT_FALSE(ctx.isPrimitive(kObjectId));
  // Internal types are not primitive.
  EXPECT_FALSE(ctx.isPrimitive(kEmptyId));
  EXPECT_FALSE(ctx.isPrimitive(kUninitId));
  EXPECT_FALSE(ctx.isPrimitive(kEnvironmentId));
  EXPECT_FALSE(ctx.isPrimitive(kBits32Id));
  // NoType is not primitive.
  EXPECT_FALSE(ctx.isPrimitive(kNoTypeId));

  // AnyType contains Object, so it's not all-primitive.
  EXPECT_FALSE(ctx.isPrimitive(kAnyTypeId));
  // Numeric = Number | BigInt — both primitive.
  EXPECT_TRUE(ctx.isPrimitive(kNumericId));
  // NullOrUndef = Null | Undefined — both primitive.
  EXPECT_TRUE(ctx.isPrimitive(kNullOrUndefId));
}

TEST(IRTypeContextTest, CanBePrimitive) {
  IRTypeContext ctx;

  EXPECT_TRUE(ctx.canBePrimitive(kNumberId));
  EXPECT_FALSE(ctx.canBePrimitive(kObjectId));
  EXPECT_FALSE(ctx.canBePrimitive(kNoTypeId));
  // AnyType contains primitives (and Object).
  EXPECT_TRUE(ctx.canBePrimitive(kAnyTypeId));
  // AnyEmptyUninit contains primitives.
  EXPECT_TRUE(ctx.canBePrimitive(kAnyEmptyUninitId));
}

TEST(IRTypeContextTest, IsNonPtr) {
  IRTypeContext ctx;

  // NonPtr kinds: Number, Boolean, Null, Undefined.
  EXPECT_TRUE(ctx.isNonPtr(kNumberId));
  EXPECT_TRUE(ctx.isNonPtr(kBooleanId));
  EXPECT_TRUE(ctx.isNonPtr(kNullId));
  EXPECT_TRUE(ctx.isNonPtr(kUndefinedId));

  // String is a pointer type.
  EXPECT_FALSE(ctx.isNonPtr(kStringId));
  // Object is a pointer type.
  EXPECT_FALSE(ctx.isNonPtr(kObjectId));
  // NoType is not nonPtr.
  EXPECT_FALSE(ctx.isNonPtr(kNoTypeId));

  // NullOrUndef = Null | Undefined — both non-ptr.
  EXPECT_TRUE(ctx.isNonPtr(kNullOrUndefId));
  // AnyType contains String/Object, so not all non-ptr.
  EXPECT_FALSE(ctx.isNonPtr(kAnyTypeId));
  // Numeric contains BigInt, which is a pointer type.
  EXPECT_FALSE(ctx.isNonPtr(kNumericId));
}

} // anonymous namespace
