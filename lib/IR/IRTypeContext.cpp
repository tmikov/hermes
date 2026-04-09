/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/IR/IRTypeContext.h"
#include "hermes/Support/ErrorHandling.h"

#include "llvh/Support/raw_ostream.h"

#include <algorithm>
#include <cassert>

namespace hermes {

namespace {

/// \return true if \p k is a number kind (including subtypes).
bool isNumberKind(TypeKind k) {
  return k == TypeKind::Number || k == TypeKind::Int32 ||
      k == TypeKind::Uint32 || k == TypeKind::Int31;
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
    case TypeKind::Int31:
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
    case TypeKind::Int31:
    case TypeKind::Boolean:
    case TypeKind::Null:
    case TypeKind::Undefined:
      return true;
    default:
      return false;
  }
}

/// \return true if leaf kind \p a is a subtype of leaf kind \p b.
bool isLeafSubtype(TypeKind a, TypeKind b) {
  if (a == b)
    return true;
  // Int32, Uint32, Int31 are subtypes of Number.
  if (b == TypeKind::Number &&
      (a == TypeKind::Int32 || a == TypeKind::Uint32 ||
       a == TypeKind::Int31))
    return true;
  // Int31 is a subtype of both Int32 and Uint32.
  if (a == TypeKind::Int31 &&
      (b == TypeKind::Int32 || b == TypeKind::Uint32))
    return true;
  // Object refinements are subtypes of Object.
  if (b == TypeKind::Object) {
    switch (a) {
      case TypeKind::ClassInstance:
      case TypeKind::Array:
      case TypeKind::Tuple:
      case TypeKind::Function:
      case TypeKind::ExactObject:
        return true;
      default:
        break;
    }
  }
  return false;
}

/// \return true if leaf kinds \p a and \p b have no values in common.
bool areLeafKindsDisjoint(TypeKind a, TypeKind b) {
  // If one is a subtype of the other, not disjoint.
  if (isLeafSubtype(a, b) || isLeafSubtype(b, a))
    return false;
  // Number family (Number, Int32, Uint32, Int31) overlap pairwise.
  auto isNumberFamily = [](TypeKind k) {
    return k == TypeKind::Number || k == TypeKind::Int32 ||
        k == TypeKind::Uint32 || k == TypeKind::Int31;
  };
  if (isNumberFamily(a) && isNumberFamily(b))
    return false;
  // All other pairs of different kinds are disjoint.
  return true;
}

/// \return the display name of a leaf kind. Matches the strings used by
/// the old Type::print().
llvh::StringRef kindName(TypeKind k) {
  switch (k) {
    case TypeKind::NoType:
      return "notype";
    case TypeKind::Empty:
      return "empty";
    case TypeKind::Uninit:
      return "uninit";
    case TypeKind::Undefined:
      return "undefined";
    case TypeKind::Null:
      return "null";
    case TypeKind::Boolean:
      return "boolean";
    case TypeKind::Number:
      return "number";
    case TypeKind::BigInt:
      return "bigint";
    case TypeKind::String:
      return "string";
    case TypeKind::Symbol:
      return "symbol";
    case TypeKind::Environment:
      return "environment";
    case TypeKind::PrivateName:
      return "privateName";
    case TypeKind::FunctionCode:
      return "functionCode";
    case TypeKind::Object:
      return "object";
    case TypeKind::Bits32:
      return "bits32";
    case TypeKind::Int32:
      return "int32";
    case TypeKind::Uint32:
      return "uint32";
    case TypeKind::Int31:
      return "int31";
    case TypeKind::ClassInstance:
      return "classInstance";
    case TypeKind::Array:
      return "array";
    case TypeKind::Tuple:
      return "tuple";
    case TypeKind::Function:
      return "function";
    case TypeKind::ExactObject:
      return "exactObject";
    case TypeKind::Union:
      return "union";
  }
  llvm_unreachable("unknown TypeKind");
}

} // anonymous namespace

unsigned IRTypeContext::countKinds(uint32_t id) const {
  assert(id < entries_.size() && "Type ID out of range");
  const auto &entry = entries_[id];
  if (entry.kind == TypeKind::NoType)
    return 0;
  if (entry.kind == TypeKind::Union)
    return entry.union_.armCount;
  return 1;
}

TypeKind IRTypeContext::getFirstKind(uint32_t id) const {
  assert(id < entries_.size() && "Type ID out of range");
  const auto &entry = entries_[id];
  if (entry.kind != TypeKind::Union)
    return entry.kind;
  // For unions, return the kind of the first arm.
  auto arms = getUnionArms(id);
  assert(!arms.empty() && "Union with no arms");
  return entries_[arms[0]].kind;
}

void IRTypeContext::format(llvh::raw_ostream &OS, uint32_t id) const {
  assert(id < entries_.size() && "Type ID out of range");
  const auto &entry = entries_[id];

  if (entry.kind == TypeKind::NoType) {
    OS << "notype";
    return;
  }

  if (entry.kind != TypeKind::Union) {
    OS << kindName(entry.kind);
    return;
  }

  // Union: check for the "any" shorthand. If this union is a superset of
  // AnyType, print "any" plus any extra arms (empty, uninit).
  if (isSubsetOf(kAnyTypeId, id)) {
    OS << "any";
    auto arms = getUnionArms(id);
    for (uint32_t armId : arms) {
      TypeKind k = entries_[armId].kind;
      // Skip kinds that are part of AnyType.
      if (containsMatchingKind(
              kAnyTypeId, [k](TypeKind ak) { return ak == k; }))
        continue;
      OS << '|' << kindName(k);
    }
    return;
  }

  // General union: pipe-separated arms.
  auto arms = getUnionArms(id);
  for (size_t i = 0; i < arms.size(); ++i) {
    if (i != 0)
      OS << '|';
    OS << kindName(entries_[arms[i]].kind);
  }
}

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

bool IRTypeContext::isSubsetOf(uint32_t a, uint32_t b) const {
  if (a == b)
    return true;
  if (a == kNoTypeId)
    return true;
  if (b == kNoTypeId)
    return false;

  TypeKind aKind = getKind(a);
  TypeKind bKind = getKind(b);

  // Union on left: all arms must be subsets of b.
  if (aKind == TypeKind::Union) {
    for (uint32_t arm : getUnionArms(a)) {
      if (!isSubsetOf(arm, b))
        return false;
    }
    return true;
  }

  // Leaf on left, union on right: a must be subset of some arm.
  if (bKind == TypeKind::Union) {
    for (uint32_t arm : getUnionArms(b)) {
      if (isSubsetOf(a, arm))
        return true;
    }
    return false;
  }

  // Both leaf types.
  return isLeafSubtype(aKind, bKind);
}

bool IRTypeContext::areDisjoint(uint32_t a, uint32_t b) const {
  if (a == kNoTypeId || b == kNoTypeId)
    return true;
  if (a == b)
    return false;

  TypeKind aKind = getKind(a);
  TypeKind bKind = getKind(b);

  // Union on left: disjoint iff all arms are disjoint from b.
  if (aKind == TypeKind::Union) {
    for (uint32_t arm : getUnionArms(a)) {
      if (!areDisjoint(arm, b))
        return false;
    }
    return true;
  }

  // Union on right: disjoint iff a is disjoint from all arms.
  if (bKind == TypeKind::Union) {
    for (uint32_t arm : getUnionArms(b)) {
      if (!areDisjoint(a, arm))
        return false;
    }
    return true;
  }

  // Both leaf types.
  return areLeafKindsDisjoint(aKind, bKind);
}

uint32_t IRTypeContext::createUnionImpl(uint32_t a, uint32_t b) {
  llvh::SmallVector<uint32_t, 16> arms;

  // Flatten: collect leaf arms from both operands.
  auto collect = [&](uint32_t id) {
    if (getKind(id) == TypeKind::Union) {
      auto ua = getUnionArms(id);
      arms.append(ua.begin(), ua.end());
    } else if (id != kNoTypeId) {
      arms.push_back(id);
    }
  };
  collect(a);
  collect(b);

  // Sort by ID for deterministic comparison.
  std::sort(arms.begin(), arms.end());

  // Remove consecutive duplicates.
  arms.erase(std::unique(arms.begin(), arms.end()), arms.end());

  // Remove subsumed arms: if arms[i] <: arms[j] for some j != i, drop i.
  llvh::SmallVector<uint32_t, 16> filtered;
  for (size_t i = 0; i < arms.size(); ++i) {
    bool subsumed = false;
    for (size_t j = 0; j < arms.size(); ++j) {
      if (i != j && isSubsetOf(arms[i], arms[j])) {
        subsumed = true;
        break;
      }
    }
    if (!subsumed)
      filtered.push_back(arms[i]);
  }

  if (filtered.empty())
    return kNoTypeId;
  if (filtered.size() == 1)
    return filtered[0];

  // Intern: check if this union already exists.
  UnionInternKey key(filtered);
  auto it = internTable_.find(key);
  if (it != internTable_.end())
    return it->second;

  // Create new union entry.
  uint32_t id = addUnionEntry(filtered);
  internTable_[std::move(key)] = id;
  return id;
}

uint32_t IRTypeContext::unionTy(uint32_t a, uint32_t b) {
  if (a == b)
    return a;
  if (a == kNoTypeId)
    return b;
  if (b == kNoTypeId)
    return a;
  if (isSubsetOf(a, b))
    return b;
  if (isSubsetOf(b, a))
    return a;
  return createUnionImpl(a, b);
}

uint32_t IRTypeContext::intersectTy(uint32_t a, uint32_t b) {
  if (a == b)
    return a;
  if (a == kNoTypeId || b == kNoTypeId)
    return kNoTypeId;
  if (isSubsetOf(a, b))
    return a;
  if (isSubsetOf(b, a))
    return b;

  // Distribute over unions.
  if (getKind(a) == TypeKind::Union) {
    uint32_t result = kNoTypeId;
    for (uint32_t arm : getUnionArms(a))
      result = unionTy(result, intersectTy(arm, b));
    return result;
  }
  if (getKind(b) == TypeKind::Union) {
    uint32_t result = kNoTypeId;
    for (uint32_t arm : getUnionArms(b))
      result = unionTy(result, intersectTy(a, arm));
    return result;
  }

  // Both leaf, not in a subset relationship.
  // The only non-subset overlap in the number family is Int32 ∩ Uint32 = Int31.
  // All other number-family pairs are handled by the isSubsetOf fast paths above.
  if (isNumberKind(getKind(a)) && isNumberKind(getKind(b)))
    return kInt31Id;
  return kNoTypeId;
}

uint32_t IRTypeContext::subtractTy(uint32_t a, uint32_t b) {
  if (a == kNoTypeId)
    return kNoTypeId;
  if (b == kNoTypeId)
    return a;
  if (isSubsetOf(a, b))
    return kNoTypeId;
  if (areDisjoint(a, b))
    return a;

  // Distribute over unions in a.
  if (getKind(a) == TypeKind::Union) {
    uint32_t result = kNoTypeId;
    for (uint32_t arm : getUnionArms(a))
      result = unionTy(result, subtractTy(arm, b));
    return result;
  }

  // a is a leaf, not subset of b, not disjoint from b.
  // Conservative: return a.
  return a;
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

  // 15: Int32
  entries_.push_back(TypeEntry::createLeaf(TypeKind::Int32));
  assert(entries_.size() - 1 == kInt32Id);

  // 16: Uint32
  entries_.push_back(TypeEntry::createLeaf(TypeKind::Uint32));
  assert(entries_.size() - 1 == kUint32Id);

  // 17: Int31 (Int32 ∩ Uint32)
  entries_.push_back(TypeEntry::createLeaf(TypeKind::Int31));
  assert(entries_.size() - 1 == kInt31Id);

  // 18: AnyType — union of all JS-observable types (matching TYPE_ANY_MASK).
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

  // 19: Numeric — Number | BigInt.
  {
    uint32_t numericArms[] = {kNumberId, kBigIntId};
    uint32_t id = addUnionEntry(numericArms);
    (void)id;
    assert(id == kNumericId);
  }

  // 20: AnyEmptyUninit — any | Empty | Uninit.
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

  // 21: NullOrUndef — Null | Undefined.
  {
    uint32_t nuArms[] = {kUndefinedId, kNullId};
    uint32_t id = addUnionEntry(nuArms);
    (void)id;
    assert(id == kNullOrUndefId);
  }

  // Pre-populate intern table with well-known unions.
  for (uint32_t id :
       {kAnyTypeId, kNumericId, kAnyEmptyUninitId, kNullOrUndefId}) {
    internTable_[UnionInternKey(getUnionArms(id))] = id;
  }

  // Pad remaining entries up to kFirstDynamicId with NoType placeholders.
  while (entries_.size() < kFirstDynamicId) {
    entries_.push_back(TypeEntry::createLeaf(TypeKind::NoType));
  }
  assert(entries_.size() == kFirstDynamicId);
}

} // namespace hermes
