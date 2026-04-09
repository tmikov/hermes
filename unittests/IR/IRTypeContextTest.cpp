/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/IR/IRTypeContext.h"

#include "llvh/Support/raw_ostream.h"

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

TEST(IRTypeContextTest, IsSubsetOf) {
  IRTypeContext ctx;

  // Reflexive.
  EXPECT_TRUE(ctx.isSubsetOf(kNumberId, kNumberId));
  // NoType is subset of everything.
  EXPECT_TRUE(ctx.isSubsetOf(kNoTypeId, kNumberId));
  EXPECT_TRUE(ctx.isSubsetOf(kNoTypeId, kAnyTypeId));
  EXPECT_TRUE(ctx.isSubsetOf(kNoTypeId, kNoTypeId));
  // Nothing (except NoType) is subset of NoType.
  EXPECT_FALSE(ctx.isSubsetOf(kNumberId, kNoTypeId));
  // Leaf is subset of union containing it.
  EXPECT_TRUE(ctx.isSubsetOf(kNumberId, kAnyTypeId));
  EXPECT_TRUE(ctx.isSubsetOf(kNumberId, kNumericId));
  EXPECT_TRUE(ctx.isSubsetOf(kBigIntId, kNumericId));
  // Union is not subset of its member.
  EXPECT_FALSE(ctx.isSubsetOf(kAnyTypeId, kNumberId));
  EXPECT_FALSE(ctx.isSubsetOf(kNumericId, kNumberId));
  // Sub-union is subset of super-union.
  EXPECT_TRUE(ctx.isSubsetOf(kNumericId, kAnyTypeId));
  EXPECT_TRUE(ctx.isSubsetOf(kNullOrUndefId, kAnyTypeId));
  // Disjoint types.
  EXPECT_FALSE(ctx.isSubsetOf(kNumberId, kStringId));
  EXPECT_FALSE(ctx.isSubsetOf(kStringId, kNumberId));
}

TEST(IRTypeContextTest, AreDisjoint) {
  IRTypeContext ctx;

  // Same type is not disjoint with itself.
  EXPECT_FALSE(ctx.areDisjoint(kNumberId, kNumberId));
  // NoType is disjoint from everything (including itself).
  EXPECT_TRUE(ctx.areDisjoint(kNoTypeId, kNumberId));
  EXPECT_TRUE(ctx.areDisjoint(kNumberId, kNoTypeId));
  EXPECT_TRUE(ctx.areDisjoint(kNoTypeId, kNoTypeId));
  // Different leaf types are disjoint.
  EXPECT_TRUE(ctx.areDisjoint(kNumberId, kStringId));
  EXPECT_TRUE(ctx.areDisjoint(kBooleanId, kObjectId));
  // Union overlaps with its members.
  EXPECT_FALSE(ctx.areDisjoint(kNumberId, kNumericId));
  EXPECT_FALSE(ctx.areDisjoint(kNumberId, kAnyTypeId));
  // Disjoint unions.
  EXPECT_TRUE(ctx.areDisjoint(kNullOrUndefId, kNumericId));
  // Overlapping unions.
  EXPECT_FALSE(ctx.areDisjoint(kAnyTypeId, kNumericId));
}

TEST(IRTypeContextTest, UnionTyIdentity) {
  IRTypeContext ctx;

  EXPECT_EQ(ctx.unionTy(kNumberId, kNumberId), kNumberId);
  EXPECT_EQ(ctx.unionTy(kAnyTypeId, kAnyTypeId), kAnyTypeId);
}

TEST(IRTypeContextTest, UnionTyNoType) {
  IRTypeContext ctx;

  EXPECT_EQ(ctx.unionTy(kNoTypeId, kStringId), kStringId);
  EXPECT_EQ(ctx.unionTy(kStringId, kNoTypeId), kStringId);
  EXPECT_EQ(ctx.unionTy(kNoTypeId, kNoTypeId), kNoTypeId);
}

TEST(IRTypeContextTest, UnionTySubset) {
  IRTypeContext ctx;

  // Number is subset of AnyType.
  EXPECT_EQ(ctx.unionTy(kNumberId, kAnyTypeId), kAnyTypeId);
  EXPECT_EQ(ctx.unionTy(kAnyTypeId, kNumberId), kAnyTypeId);
  // Numeric is subset of AnyType.
  EXPECT_EQ(ctx.unionTy(kNumericId, kAnyTypeId), kAnyTypeId);
}

TEST(IRTypeContextTest, UnionTyCreatesDynamic) {
  IRTypeContext ctx;

  uint32_t numStr = ctx.unionTy(kNumberId, kStringId);
  EXPECT_EQ(ctx.getKind(numStr), TypeKind::Union);
  auto arms = ctx.getUnionArms(numStr);
  EXPECT_EQ(arms.size(), 2u);
  // Arms sorted by ID: kStringId(6), kNumberId(7).
  EXPECT_EQ(arms[0], kStringId);
  EXPECT_EQ(arms[1], kNumberId);
}

TEST(IRTypeContextTest, UnionTyInterning) {
  IRTypeContext ctx;

  uint32_t a = ctx.unionTy(kNumberId, kStringId);
  uint32_t b = ctx.unionTy(kNumberId, kStringId);
  EXPECT_EQ(a, b);

  // Reverse order produces same result.
  uint32_t c = ctx.unionTy(kStringId, kNumberId);
  EXPECT_EQ(a, c);
}

TEST(IRTypeContextTest, UnionTyReturnsWellKnown) {
  IRTypeContext ctx;

  // unionTy(Number, BigInt) should return the well-known Numeric.
  EXPECT_EQ(ctx.unionTy(kNumberId, kBigIntId), kNumericId);
  // unionTy(Null, Undefined) should return well-known NullOrUndef.
  EXPECT_EQ(ctx.unionTy(kNullId, kUndefinedId), kNullOrUndefId);
}

TEST(IRTypeContextTest, IntersectTy) {
  IRTypeContext ctx;

  // Disjoint types.
  EXPECT_EQ(ctx.intersectTy(kNumberId, kStringId), kNoTypeId);
  // Subset: Number intersect AnyType = Number.
  EXPECT_EQ(ctx.intersectTy(kNumberId, kAnyTypeId), kNumberId);
  EXPECT_EQ(ctx.intersectTy(kAnyTypeId, kNumberId), kNumberId);
  // Same type.
  EXPECT_EQ(ctx.intersectTy(kNumberId, kNumberId), kNumberId);
  // NoType.
  EXPECT_EQ(ctx.intersectTy(kNoTypeId, kNumberId), kNoTypeId);
  EXPECT_EQ(ctx.intersectTy(kNumberId, kNoTypeId), kNoTypeId);
  // Union intersect Union: Numeric subset of AnyType.
  EXPECT_EQ(ctx.intersectTy(kNumericId, kAnyTypeId), kNumericId);
  // NullOrUndef intersect Numeric: disjoint → NoType.
  EXPECT_EQ(ctx.intersectTy(kNullOrUndefId, kNumericId), kNoTypeId);
}

TEST(IRTypeContextTest, SubtractTy) {
  IRTypeContext ctx;

  // Subtract member from union: AnyType - Number.
  uint32_t result = ctx.subtractTy(kAnyTypeId, kNumberId);
  EXPECT_EQ(ctx.getKind(result), TypeKind::Union);
  auto arms = ctx.getUnionArms(result);
  EXPECT_EQ(arms.size(), 7u);
  EXPECT_FALSE(ctx.canBeNumber(result));
  EXPECT_TRUE(ctx.canBeString(result));
  EXPECT_TRUE(ctx.canBeObject(result));
  EXPECT_TRUE(ctx.canBeBigInt(result));

  // Subset subtraction → NoType.
  EXPECT_EQ(ctx.subtractTy(kNumberId, kAnyTypeId), kNoTypeId);
  // Disjoint subtraction → unchanged.
  EXPECT_EQ(ctx.subtractTy(kNumberId, kStringId), kNumberId);
  // NoType cases.
  EXPECT_EQ(ctx.subtractTy(kNoTypeId, kNumberId), kNoTypeId);
  EXPECT_EQ(ctx.subtractTy(kNumberId, kNoTypeId), kNumberId);
  // Subtract union from union: AnyType - NullOrUndef.
  uint32_t r2 = ctx.subtractTy(kAnyTypeId, kNullOrUndefId);
  EXPECT_FALSE(ctx.canBeNull(r2));
  EXPECT_FALSE(ctx.canBeUndefined(r2));
  EXPECT_TRUE(ctx.canBeNumber(r2));
  EXPECT_TRUE(ctx.canBeString(r2));
}

TEST(IRTypeContextTest, Int31WellKnownId) {
  IRTypeContext ctx;

  // Int31 is pre-allocated with its own well-known ID.
  EXPECT_EQ(ctx.getKind(kInt31Id), TypeKind::Int31);
  EXPECT_EQ(ctx.getKind(kInt32Id), TypeKind::Int32);
  EXPECT_EQ(ctx.getKind(kUint32Id), TypeKind::Uint32);
}

TEST(IRTypeContextTest, Int31SubtypeRelationships) {
  IRTypeContext ctx;

  // Int31 <: Int32, Uint32, Number.
  EXPECT_TRUE(ctx.isSubsetOf(kInt31Id, kInt32Id));
  EXPECT_TRUE(ctx.isSubsetOf(kInt31Id, kUint32Id));
  EXPECT_TRUE(ctx.isSubsetOf(kInt31Id, kNumberId));
  // Not the reverse.
  EXPECT_FALSE(ctx.isSubsetOf(kInt32Id, kInt31Id));
  EXPECT_FALSE(ctx.isSubsetOf(kUint32Id, kInt31Id));
  EXPECT_FALSE(ctx.isSubsetOf(kNumberId, kInt31Id));
  // Int32/Uint32 are not subsets of each other.
  EXPECT_FALSE(ctx.isSubsetOf(kInt32Id, kUint32Id));
  EXPECT_FALSE(ctx.isSubsetOf(kUint32Id, kInt32Id));
  // But both are subsets of Number.
  EXPECT_TRUE(ctx.isSubsetOf(kInt32Id, kNumberId));
  EXPECT_TRUE(ctx.isSubsetOf(kUint32Id, kNumberId));
}

TEST(IRTypeContextTest, Int31Disjointness) {
  IRTypeContext ctx;

  // Int31 is not disjoint from its supertypes.
  EXPECT_FALSE(ctx.areDisjoint(kInt31Id, kInt32Id));
  EXPECT_FALSE(ctx.areDisjoint(kInt31Id, kUint32Id));
  EXPECT_FALSE(ctx.areDisjoint(kInt31Id, kNumberId));
  // Int32 and Uint32 overlap (via Int31).
  EXPECT_FALSE(ctx.areDisjoint(kInt32Id, kUint32Id));
  // Int31 is disjoint from non-number types.
  EXPECT_TRUE(ctx.areDisjoint(kInt31Id, kStringId));
  EXPECT_TRUE(ctx.areDisjoint(kInt31Id, kObjectId));
  EXPECT_TRUE(ctx.areDisjoint(kInt31Id, kBooleanId));
}

TEST(IRTypeContextTest, IntersectInt32Uint32) {
  IRTypeContext ctx;

  // Int32 ∩ Uint32 = Int31.
  EXPECT_EQ(ctx.intersectTy(kInt32Id, kUint32Id), kInt31Id);
  EXPECT_EQ(ctx.intersectTy(kUint32Id, kInt32Id), kInt31Id);
  // Number ∩ Int32 = Int32 (subset).
  EXPECT_EQ(ctx.intersectTy(kNumberId, kInt32Id), kInt32Id);
  // Number ∩ Uint32 = Uint32 (subset).
  EXPECT_EQ(ctx.intersectTy(kNumberId, kUint32Id), kUint32Id);
  // Int31 ∩ Int32 = Int31 (subset).
  EXPECT_EQ(ctx.intersectTy(kInt31Id, kInt32Id), kInt31Id);
  // Int31 ∩ String = NoType (disjoint).
  EXPECT_EQ(ctx.intersectTy(kInt31Id, kStringId), kNoTypeId);
}

TEST(IRTypeContextTest, Int31Predicates) {
  IRTypeContext ctx;

  EXPECT_TRUE(ctx.canBeNumber(kInt31Id));
  EXPECT_TRUE(ctx.isPrimitive(kInt31Id));
  EXPECT_TRUE(ctx.isNonPtr(kInt31Id));
  EXPECT_FALSE(ctx.canBeString(kInt31Id));
  EXPECT_FALSE(ctx.canBeObject(kInt31Id));
}

TEST(IRTypeContextTest, CountKinds) {
  IRTypeContext ctx;

  EXPECT_EQ(ctx.countKinds(kNoTypeId), 0u);
  EXPECT_EQ(ctx.countKinds(kNumberId), 1u);
  EXPECT_EQ(ctx.countKinds(kStringId), 1u);
  EXPECT_EQ(ctx.countKinds(kObjectId), 1u);
  // AnyType has 8 arms.
  EXPECT_EQ(ctx.countKinds(kAnyTypeId), 8u);
  // Numeric = Number | BigInt.
  EXPECT_EQ(ctx.countKinds(kNumericId), 2u);
  // NullOrUndef = Null | Undefined.
  EXPECT_EQ(ctx.countKinds(kNullOrUndefId), 2u);
  // AnyEmptyUninit = 10 arms.
  EXPECT_EQ(ctx.countKinds(kAnyEmptyUninitId), 10u);
  // Dynamic union.
  uint32_t numStr = ctx.unionTy(kNumberId, kStringId);
  EXPECT_EQ(ctx.countKinds(numStr), 2u);
}

TEST(IRTypeContextTest, GetFirstKind) {
  IRTypeContext ctx;

  EXPECT_EQ(ctx.getFirstKind(kNoTypeId), TypeKind::NoType);
  EXPECT_EQ(ctx.getFirstKind(kNumberId), TypeKind::Number);
  EXPECT_EQ(ctx.getFirstKind(kStringId), TypeKind::String);
  EXPECT_EQ(ctx.getFirstKind(kObjectId), TypeKind::Object);
  EXPECT_EQ(ctx.getFirstKind(kEmptyId), TypeKind::Empty);
  // AnyType first arm is Undefined (lowest ID arm).
  EXPECT_EQ(ctx.getFirstKind(kAnyTypeId), TypeKind::Undefined);
  // Numeric first arm is Number (ID 7 < ID 8).
  EXPECT_EQ(ctx.getFirstKind(kNumericId), TypeKind::Number);
  // NullOrUndef first arm is Undefined (ID 3 < ID 4).
  EXPECT_EQ(ctx.getFirstKind(kNullOrUndefId), TypeKind::Undefined);
}

TEST(IRTypeContextTest, FormatLeafTypes) {
  IRTypeContext ctx;
  auto fmt = [&](uint32_t id) -> std::string {
    std::string s;
    llvh::raw_string_ostream os(s);
    ctx.format(os, id);
    return s;
  };

  EXPECT_EQ(fmt(kNoTypeId), "notype");
  EXPECT_EQ(fmt(kNumberId), "number");
  EXPECT_EQ(fmt(kStringId), "string");
  EXPECT_EQ(fmt(kObjectId), "object");
  EXPECT_EQ(fmt(kBooleanId), "boolean");
  EXPECT_EQ(fmt(kNullId), "null");
  EXPECT_EQ(fmt(kUndefinedId), "undefined");
  EXPECT_EQ(fmt(kBigIntId), "bigint");
  EXPECT_EQ(fmt(kSymbolId), "symbol");
  EXPECT_EQ(fmt(kEmptyId), "empty");
  EXPECT_EQ(fmt(kUninitId), "uninit");
  EXPECT_EQ(fmt(kEnvironmentId), "environment");
  EXPECT_EQ(fmt(kPrivateNameId), "privateName");
  EXPECT_EQ(fmt(kFunctionCodeId), "functionCode");
  EXPECT_EQ(fmt(kBits32Id), "bits32");
}

TEST(IRTypeContextTest, FormatWellKnownUnions) {
  IRTypeContext ctx;
  auto fmt = [&](uint32_t id) -> std::string {
    std::string s;
    llvh::raw_string_ostream os(s);
    ctx.format(os, id);
    return s;
  };

  // AnyType prints as "any".
  EXPECT_EQ(fmt(kAnyTypeId), "any");
  // AnyEmptyUninit prints as "any|empty|uninit".
  EXPECT_EQ(fmt(kAnyEmptyUninitId), "any|empty|uninit");
  // Numeric = Number | BigInt.
  EXPECT_EQ(fmt(kNumericId), "number|bigint");
  // NullOrUndef = Undefined | Null (sorted by ID).
  EXPECT_EQ(fmt(kNullOrUndefId), "undefined|null");
}

TEST(IRTypeContextTest, FormatDynamicUnion) {
  IRTypeContext ctx;
  auto fmt = [&](uint32_t id) -> std::string {
    std::string s;
    llvh::raw_string_ostream os(s);
    ctx.format(os, id);
    return s;
  };

  uint32_t numStr = ctx.unionTy(kNumberId, kStringId);
  // Arms sorted by ID: String(6), Number(7).
  EXPECT_EQ(fmt(numStr), "string|number");
}

} // anonymous namespace
