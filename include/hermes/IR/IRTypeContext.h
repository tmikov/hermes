/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#ifndef HERMES_IR_IRTYPECONTEXT_H
#define HERMES_IR_IRTYPECONTEXT_H

#include "llvh/ADT/ArrayRef.h"
#include "llvh/ADT/DenseMap.h"
#include "llvh/ADT/Hashing.h"
#include "llvh/ADT/SmallVector.h"

#include <cassert>
#include <cstdint>
#include <vector>

namespace llvh {
class raw_ostream;
} // namespace llvh

namespace hermes {

/// Kinds of types in the IR type system.
enum class TypeKind : uint8_t {
  // --- Leaf kinds (no sub-type data) ---
  NoType, ///< Empty set, bottom.
  Undefined,
  Null,
  Boolean,
  Number, ///< Unrefined fp64.
  BigInt,
  String,
  Symbol,
  Empty, ///< TDZ.
  Uninit, ///< Declared, not initialized.
  Environment,
  PrivateName,
  FunctionCode,
  Object, ///< Unrefined JS object.
  Bits32, ///< Physical 32-bit integer, signless.

  // --- Number subtypes (still fp64, value constrained) ---
  Int32, ///< Integer in [-2^31, 2^31 - 1].
  Uint32, ///< Integer in [0, 2^32 - 1].
  Int31, ///< Integer in [0, 2^31 - 1] (Int32 ∩ Uint32).

  // --- Refined object types ---
  ClassInstance, ///< Nominal class instance.
  Array, ///< Array with known element type.
  Tuple, ///< Fixed-length positional types.
  Function, ///< Callable with known signature.
  ExactObject, ///< Known exact field set.

  // --- Composite ---
  Union, ///< Union of two or more types.
};

/// An entry in the IRTypeContext type table.
struct TypeEntry {
  TypeKind kind;

  union {
    /// ClassInstance payload.
    struct {
      uint32_t classId;
    } classInstance;

    /// Array payload.
    struct {
      uint32_t elemTypeId;
    } array;

    /// Tuple payload.
    struct {
      uint32_t elemOffset; ///< into typeArrays_
      uint16_t elemCount;
    } tuple;

    /// Function payload.
    struct {
      uint32_t returnTypeId;
      uint32_t thisTypeId;
      uint32_t restTypeId;
      uint32_t paramOffset; ///< into paramArrays_
      uint16_t paramCount;
      bool isAsync;
      bool isGenerator;
    } function;

    /// ExactObject payload.
    struct {
      uint32_t fieldOffset; ///< into fieldArrays_
      uint16_t fieldCount;
    } exactObject;

    /// Union payload.
    struct {
      uint32_t armOffset; ///< into typeArrays_
      uint16_t armCount; ///< >= 2
    } union_;
  };

  /// Construct a leaf type entry with no payload.
  static TypeEntry createLeaf(TypeKind k) {
    TypeEntry e{};
    e.kind = k;
    return e;
  }

  /// Construct a union type entry.
  static TypeEntry createUnion(uint32_t armOffset, uint16_t armCount) {
    assert(armCount >= 2 && "Union must have at least 2 arms");
    TypeEntry e{};
    e.kind = TypeKind::Union;
    e.union_.armOffset = armOffset;
    e.union_.armCount = armCount;
    return e;
  }
};

/// Side array element for function parameters.
struct FunctionParam {
  uint32_t typeId;
  bool isOptional;
};

/// Side array element for exact object fields.
struct ExactObjectField {
  // Identifier name; // Deferred until ExactObject is used.
  uint32_t typeId;
};

// Pre-allocated type IDs (assigned by IRTypeContext constructor).
// These are constants, valid for any IRTypeContext instance.
// The well-known ID assignment is frozen: IDs are never removed or reordered.
// New well-known IDs are only appended before kFirstDynamicId.
static constexpr uint32_t kNoTypeId = 0;
static constexpr uint32_t kEmptyId = 1;
static constexpr uint32_t kUninitId = 2;
static constexpr uint32_t kUndefinedId = 3;
static constexpr uint32_t kNullId = 4;
static constexpr uint32_t kBooleanId = 5;
static constexpr uint32_t kStringId = 6;
static constexpr uint32_t kNumberId = 7;
static constexpr uint32_t kBigIntId = 8;
static constexpr uint32_t kSymbolId = 9;
static constexpr uint32_t kEnvironmentId = 10;
static constexpr uint32_t kPrivateNameId = 11;
static constexpr uint32_t kFunctionCodeId = 12;
static constexpr uint32_t kObjectId = 13;
static constexpr uint32_t kBits32Id = 14;
static constexpr uint32_t kInt32Id = 15;
static constexpr uint32_t kUint32Id = 16;
/// Int32 ∩ Uint32: integer in [0, 2^31 - 1].
static constexpr uint32_t kInt31Id = 17;
/// Union of all JS-observable types.
static constexpr uint32_t kAnyTypeId = 18;
/// Number | BigInt.
static constexpr uint32_t kNumericId = 19;
/// any | Empty | Uninit.
static constexpr uint32_t kAnyEmptyUninitId = 20;
/// Null | Undefined.
static constexpr uint32_t kNullOrUndefId = 21;
/// IDs below this are reserved for well-known types.
static constexpr uint32_t kFirstDynamicId = 32;

/// Hashable key for interning union types. Contains sorted arm IDs.
struct UnionInternKey {
  llvh::SmallVector<uint32_t, 8> arms;

  UnionInternKey() = default;
  explicit UnionInternKey(llvh::ArrayRef<uint32_t> a)
      : arms(a.begin(), a.end()) {}
};

/// DenseMap info for UnionInternKey.
struct UnionInternKeyInfo {
  static UnionInternKey getEmptyKey() {
    UnionInternKey k;
    k.arms.push_back(UINT32_MAX);
    return k;
  }
  static UnionInternKey getTombstoneKey() {
    UnionInternKey k;
    k.arms.push_back(UINT32_MAX - 1);
    return k;
  }
  static unsigned getHashValue(const UnionInternKey &k) {
    return llvh::hash_combine_range(k.arms.begin(), k.arms.end());
  }
  static bool isEqual(const UnionInternKey &a, const UnionInternKey &b) {
    return a.arms == b.arms;
  }
};

/// Owns the type table for a Module and provides type operations.
///
/// The constructor pre-allocates entries for all well-known types (primitives
/// and common unions). Well-known IDs have fixed values that are the same
/// across all IRTypeContext instances.
class IRTypeContext {
 public:
  IRTypeContext();

  /// Return the kind of the type entry at \p id.
  TypeKind getKind(uint32_t id) const {
    assert(id < entries_.size() && "Type ID out of range");
    return entries_[id].kind;
  }

  /// Return the arm IDs of a union type entry. Asserts that \p id is a Union.
  llvh::ArrayRef<uint32_t> getUnionArms(uint32_t id) const {
    assert(id < entries_.size() && "Type ID out of range");
    const auto &entry = entries_[id];
    assert(entry.kind == TypeKind::Union && "Not a union type");
    assert(
        size_t(entry.union_.armOffset) + entry.union_.armCount <=
            typeArrays_.size() &&
        "Union arms out of bounds");
    return llvh::ArrayRef<uint32_t>(
        typeArrays_.data() + entry.union_.armOffset, entry.union_.armCount);
  }

  /// \return true if the type at \p id is NoType (empty set).
  bool isNoType(uint32_t id) const {
    return getKind(id) == TypeKind::NoType;
  }

  /// \return true if the type at \p id can represent a Number value.
  bool canBeNumber(uint32_t id) const;
  /// \return true if the type at \p id can represent a String value.
  bool canBeString(uint32_t id) const;
  /// \return true if the type at \p id can represent an Object value.
  bool canBeObject(uint32_t id) const;
  /// \return true if the type at \p id can represent a Null value.
  bool canBeNull(uint32_t id) const;
  /// \return true if the type at \p id can represent an Undefined value.
  bool canBeUndefined(uint32_t id) const;
  /// \return true if the type at \p id can represent an Empty value.
  bool canBeEmpty(uint32_t id) const;
  /// \return true if the type at \p id can represent an Uninit value.
  bool canBeUninit(uint32_t id) const;
  /// \return true if the type at \p id can represent a BigInt value.
  bool canBeBigInt(uint32_t id) const;
  /// \return true if the type at \p id can represent a Boolean value.
  bool canBeBoolean(uint32_t id) const;
  /// \return true if the type at \p id can represent a Symbol value.
  bool canBeSymbol(uint32_t id) const;

  /// \return true if the type at \p id represents only primitive types
  /// (Number, String, BigInt, Null, Undefined, Boolean, Symbol).
  /// Returns false for NoType.
  bool isPrimitive(uint32_t id) const;

  /// \return true if any of the types at \p id are primitive.
  bool canBePrimitive(uint32_t id) const;

  /// \return true if the type at \p id is not referenced by a pointer
  /// (Number, Boolean, Null, Undefined only). Returns false for NoType.
  bool isNonPtr(uint32_t id) const;

  /// \return true if all values of type \p a are also values of type \p b.
  bool isSubsetOf(uint32_t a, uint32_t b) const;

  /// \return true if types \p a and \p b have no values in common.
  bool areDisjoint(uint32_t a, uint32_t b) const;

  /// \return the union of types \p a and \p b. May create and intern a new
  /// union type.
  uint32_t unionTy(uint32_t a, uint32_t b);

  /// \return the intersection of types \p a and \p b.
  uint32_t intersectTy(uint32_t a, uint32_t b);

  /// \return type \p a minus type \p b (conservative approximation).
  uint32_t subtractTy(uint32_t a, uint32_t b);

  /// \return the number of kinds in the type: 0 for NoType, arm count for
  /// unions, 1 for leaf types.
  unsigned countKinds(uint32_t id) const;

  /// \return the TypeKind of the type. For unions, returns the kind of the
  /// first arm. For NoType, returns TypeKind::NoType.
  TypeKind getFirstKind(uint32_t id) const;

  /// Print the human-readable type name to \p OS. Leaf kinds print their name
  /// (e.g. "number"). Unions print pipe-separated arms (e.g. "number|string").
  /// NoType prints "notype". AnyType prints "any".
  void format(llvh::raw_ostream &OS, uint32_t id) const;

 private:
  /// Type table. Index 0 = NoType. Pre-allocated entries for primitives.
  std::vector<TypeEntry> entries_;

  /// Side array storing union arm IDs (and in the future, tuple element IDs).
  /// Uses raw uint32_t since Type is still a bitmask at this point; changed
  /// to Type in P1-S8.
  std::vector<uint32_t> typeArrays_;

  /// Return true if any component of the type at \p id satisfies \p pred.
  /// For leaf types, tests the kind directly. For unions, tests any arm.
  template <typename Pred>
  bool containsMatchingKind(uint32_t id, Pred pred) const {
    assert(id < entries_.size() && "Type ID out of range");
    const auto &entry = entries_[id];
    if (entry.kind != TypeKind::Union)
      return pred(entry.kind);
    auto arms = getUnionArms(id);
    for (uint32_t armId : arms) {
      assert(entries_[armId].kind != TypeKind::Union && "Nested unions");
      if (pred(entries_[armId].kind))
        return true;
    }
    return false;
  }

  /// Return true if all components of the type at \p id satisfy \p pred.
  /// Returns false for NoType.
  template <typename Pred>
  bool allMatchKind(uint32_t id, Pred pred) const {
    assert(id < entries_.size() && "Type ID out of range");
    const auto &entry = entries_[id];
    if (entry.kind == TypeKind::NoType)
      return false;
    if (entry.kind != TypeKind::Union)
      return pred(entry.kind);
    auto arms = getUnionArms(id);
    for (uint32_t armId : arms) {
      if (!pred(entries_[armId].kind))
        return false;
    }
    return true;
  }

  /// Helper to append arms to typeArrays_ and create a union entry.
  uint32_t addUnionEntry(llvh::ArrayRef<uint32_t> arms);

  /// Intern table mapping sorted arm sets to existing union type IDs.
  llvh::DenseMap<UnionInternKey, uint32_t, UnionInternKeyInfo> internTable_;

  /// Create a union from two operands with full canonicalization and interning.
  /// Flattens, deduplicates, removes subsumed arms, sorts, and interns.
  uint32_t createUnionImpl(uint32_t a, uint32_t b);
};

} // namespace hermes

#endif // HERMES_IR_IRTYPECONTEXT_H
