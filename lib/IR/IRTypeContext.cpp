/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/IR/IRTypeContext.h"
#include "hermes/Support/ErrorHandling.h"

#include <cassert>

namespace hermes {

namespace {

/// \return true if \p k is a number kind (including subtypes Int32, Uint32).
bool isNumberKind(TypeKind k) {
  return k == TypeKind::Number || k == TypeKind::Int32 ||
      k == TypeKind::Uint32;
}

/// \return true if \p k is an object kind (including refinements).
bool isObjectKind(TypeKind k) {
  switch (k) {
    case TypeKind::Object:
    case TypeKind::ClassInstance:
    case TypeKind::Array:
    case TypeKind::Tuple:
    case TypeKind::Function:
    case TypeKind::ExactObject:
      return true;
    default:
      return false;
  }
}

/// \return true if \p k is a primitive kind (including number subtypes).
bool isPrimitiveKind(TypeKind k) {
  switch (k) {
    case TypeKind::Number:
    case TypeKind::Int32:
    case TypeKind::Uint32:
    case TypeKind::String:
    case TypeKind::BigInt:
    case TypeKind::Null:
    case TypeKind::Undefined:
    case TypeKind::Boolean:
    case TypeKind::Symbol:
      return true;
    default:
      return false;
  }
}

/// \return true if \p k is a non-pointer kind (including number subtypes).
bool isNonPtrKind(TypeKind k) {
  switch (k) {
    case TypeKind::Number:
    case TypeKind::Int32:
    case TypeKind::Uint32:
    case TypeKind::Boolean:
    case TypeKind::Null:
    case TypeKind::Undefined:
      return true;
    default:
      return false;
  }
}

} // anonymous namespace

bool IRTypeContext::canBeNumber(uint32_t id) const {
  // Fast path: well-known IDs.
  if (id == kNumberId || id == kAnyTypeId || id == kNumericId ||
      id == kAnyEmptyUninitId)
    return true;
  return containsMatchingKind(id, isNumberKind);
}

bool IRTypeContext::canBeString(uint32_t id) const {
  if (id == kStringId || id == kAnyTypeId || id == kAnyEmptyUninitId)
    return true;
  return containsMatchingKind(
      id, [](TypeKind k) { return k == TypeKind::String; });
}

bool IRTypeContext::canBeObject(uint32_t id) const {
  if (id == kObjectId || id == kAnyTypeId || id == kAnyEmptyUninitId)
    return true;
  return containsMatchingKind(id, isObjectKind);
}

bool IRTypeContext::canBeNull(uint32_t id) const {
  if (id == kNullId || id == kAnyTypeId || id == kNullOrUndefId ||
      id == kAnyEmptyUninitId)
    return true;
  return containsMatchingKind(
      id, [](TypeKind k) { return k == TypeKind::Null; });
}

bool IRTypeContext::canBeUndefined(uint32_t id) const {
  if (id == kUndefinedId || id == kAnyTypeId || id == kNullOrUndefId ||
      id == kAnyEmptyUninitId)
    return true;
  return containsMatchingKind(
      id, [](TypeKind k) { return k == TypeKind::Undefined; });
}

bool IRTypeContext::canBeEmpty(uint32_t id) const {
  if (id == kEmptyId || id == kAnyEmptyUninitId)
    return true;
  return containsMatchingKind(
      id, [](TypeKind k) { return k == TypeKind::Empty; });
}

bool IRTypeContext::canBeUninit(uint32_t id) const {
  if (id == kUninitId || id == kAnyEmptyUninitId)
    return true;
  return containsMatchingKind(
      id, [](TypeKind k) { return k == TypeKind::Uninit; });
}

bool IRTypeContext::canBeBigInt(uint32_t id) const {
  if (id == kBigIntId || id == kAnyTypeId || id == kNumericId ||
      id == kAnyEmptyUninitId)
    return true;
  return containsMatchingKind(
      id, [](TypeKind k) { return k == TypeKind::BigInt; });
}

bool IRTypeContext::canBeBoolean(uint32_t id) const {
  if (id == kBooleanId || id == kAnyTypeId || id == kAnyEmptyUninitId)
    return true;
  return containsMatchingKind(
      id, [](TypeKind k) { return k == TypeKind::Boolean; });
}

bool IRTypeContext::canBeSymbol(uint32_t id) const {
  if (id == kSymbolId || id == kAnyTypeId || id == kAnyEmptyUninitId)
    return true;
  return containsMatchingKind(
      id, [](TypeKind k) { return k == TypeKind::Symbol; });
}

bool IRTypeContext::isPrimitive(uint32_t id) const {
  return allMatchKind(id, isPrimitiveKind);
}

bool IRTypeContext::canBePrimitive(uint32_t id) const {
  if (id == kNoTypeId)
    return false;
  return containsMatchingKind(id, isPrimitiveKind);
}

bool IRTypeContext::isNonPtr(uint32_t id) const {
  return allMatchKind(id, isNonPtrKind);
}

uint32_t IRTypeContext::addUnionEntry(llvh::ArrayRef<uint32_t> arms) {
  assert(arms.size() >= 2 && "Union must have at least 2 arms");
  uint32_t offset = hermes_narrow_cast<uint32_t>(
      typeArrays_.size(), "type array offset overflow");
  typeArrays_.insert(typeArrays_.end(), arms.begin(), arms.end());
  uint32_t id = entries_.size();
  entries_.push_back(TypeEntry::createUnion(
      offset,
      hermes_narrow_cast<uint16_t>(arms.size(), "too many union arms")));
  return id;
}

IRTypeContext::IRTypeContext() {
  // Reserve space for well-known entries.
  entries_.reserve(kFirstDynamicId);

  // Pre-allocate leaf type entries. The order must match the well-known ID
  // constants exactly.

  // 0: NoType
  entries_.push_back(TypeEntry::createLeaf(TypeKind::NoType));
  assert(entries_.size() - 1 == kNoTypeId);

  // 1: Empty
  entries_.push_back(TypeEntry::createLeaf(TypeKind::Empty));
  assert(entries_.size() - 1 == kEmptyId);

  // 2: Uninit
  entries_.push_back(TypeEntry::createLeaf(TypeKind::Uninit));
  assert(entries_.size() - 1 == kUninitId);

  // 3: Undefined
  entries_.push_back(TypeEntry::createLeaf(TypeKind::Undefined));
  assert(entries_.size() - 1 == kUndefinedId);

  // 4: Null
  entries_.push_back(TypeEntry::createLeaf(TypeKind::Null));
  assert(entries_.size() - 1 == kNullId);

  // 5: Boolean
  entries_.push_back(TypeEntry::createLeaf(TypeKind::Boolean));
  assert(entries_.size() - 1 == kBooleanId);

  // 6: String
  entries_.push_back(TypeEntry::createLeaf(TypeKind::String));
  assert(entries_.size() - 1 == kStringId);

  // 7: Number
  entries_.push_back(TypeEntry::createLeaf(TypeKind::Number));
  assert(entries_.size() - 1 == kNumberId);

  // 8: BigInt
  entries_.push_back(TypeEntry::createLeaf(TypeKind::BigInt));
  assert(entries_.size() - 1 == kBigIntId);

  // 9: Symbol
  entries_.push_back(TypeEntry::createLeaf(TypeKind::Symbol));
  assert(entries_.size() - 1 == kSymbolId);

  // 10: Environment
  entries_.push_back(TypeEntry::createLeaf(TypeKind::Environment));
  assert(entries_.size() - 1 == kEnvironmentId);

  // 11: PrivateName
  entries_.push_back(TypeEntry::createLeaf(TypeKind::PrivateName));
  assert(entries_.size() - 1 == kPrivateNameId);

  // 12: FunctionCode
  entries_.push_back(TypeEntry::createLeaf(TypeKind::FunctionCode));
  assert(entries_.size() - 1 == kFunctionCodeId);

  // 13: Object
  entries_.push_back(TypeEntry::createLeaf(TypeKind::Object));
  assert(entries_.size() - 1 == kObjectId);

  // 14: Bits32
  entries_.push_back(TypeEntry::createLeaf(TypeKind::Bits32));
  assert(entries_.size() - 1 == kBits32Id);

  // 15: AnyType — union of all JS-observable types (matching TYPE_ANY_MASK).
  // Primitives + Object, excludes Empty, Uninit, Environment, PrivateName,
  // FunctionCode, Bits32.
  {
    uint32_t anyArms[] = {
        kUndefinedId,
        kNullId,
        kBooleanId,
        kStringId,
        kNumberId,
        kBigIntId,
        kSymbolId,
        kObjectId};
    uint32_t id = addUnionEntry(anyArms);
    (void)id;
    assert(id == kAnyTypeId);
  }

  // 16: Numeric — Number | BigInt.
  {
    uint32_t numericArms[] = {kNumberId, kBigIntId};
    uint32_t id = addUnionEntry(numericArms);
    (void)id;
    assert(id == kNumericId);
  }

  // 17: AnyEmptyUninit — any | Empty | Uninit.
  // This is a union of all the AnyType arms plus Empty and Uninit.
  {
    uint32_t aeuArms[] = {
        kEmptyId,
        kUninitId,
        kUndefinedId,
        kNullId,
        kBooleanId,
        kStringId,
        kNumberId,
        kBigIntId,
        kSymbolId,
        kObjectId};
    uint32_t id = addUnionEntry(aeuArms);
    (void)id;
    assert(id == kAnyEmptyUninitId);
  }

  // 18: NullOrUndef — Null | Undefined.
  {
    uint32_t nuArms[] = {kUndefinedId, kNullId};
    uint32_t id = addUnionEntry(nuArms);
    (void)id;
    assert(id == kNullOrUndefId);
  }

  // Pad remaining entries up to kFirstDynamicId with NoType placeholders.
  while (entries_.size() < kFirstDynamicId) {
    entries_.push_back(TypeEntry::createLeaf(TypeKind::NoType));
  }
  assert(entries_.size() == kFirstDynamicId);
}

} // namespace hermes
