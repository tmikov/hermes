/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#ifndef HERMES_IR_IRTYPECONTEXT_INLINE_H
#define HERMES_IR_IRTYPECONTEXT_INLINE_H

#include "hermes/IR/IR.h"

namespace hermes {

// One-line forwarders from the public Type-taking IRTypeContext API to
// the private uint32_t implementation overloads. Defined inline so the
// compiler can collapse them at every call site in all build modes.
// IRTypeContext is friend of Type, so accessing Type::id_ here is legal.
// Inside each body, the unqualified call resolves to the private
// uint32_t overload because t.id_ is a uint32_t and Type's constructor
// from uint32_t is explicit.

inline TypeKind IRTypeContext::getKind(Type t) const {
  return getKind(t.id_);
}

inline llvh::ArrayRef<Type> IRTypeContext::getUnionArms(Type t) const {
  return getUnionArms(t.id_);
}

inline bool IRTypeContext::isNoType(Type t) const {
  return isNoType(t.id_);
}

inline bool IRTypeContext::canBeNumber(Type t) const {
  return canBeNumber(t.id_);
}

inline bool IRTypeContext::canBeString(Type t) const {
  return canBeString(t.id_);
}

inline bool IRTypeContext::canBeObject(Type t) const {
  return canBeObject(t.id_);
}

inline bool IRTypeContext::canBeNull(Type t) const {
  return canBeNull(t.id_);
}

inline bool IRTypeContext::canBeUndefined(Type t) const {
  return canBeUndefined(t.id_);
}

inline bool IRTypeContext::canBeEmpty(Type t) const {
  return canBeEmpty(t.id_);
}

inline bool IRTypeContext::canBeUninit(Type t) const {
  return canBeUninit(t.id_);
}

inline bool IRTypeContext::canBeBigInt(Type t) const {
  return canBeBigInt(t.id_);
}

inline bool IRTypeContext::canBeBoolean(Type t) const {
  return canBeBoolean(t.id_);
}

inline bool IRTypeContext::canBeSymbol(Type t) const {
  return canBeSymbol(t.id_);
}

inline bool IRTypeContext::isPrimitive(Type t) const {
  return isPrimitive(t.id_);
}

inline bool IRTypeContext::canBePrimitive(Type t) const {
  return canBePrimitive(t.id_);
}

inline bool IRTypeContext::isNonPtr(Type t) const {
  return isNonPtr(t.id_);
}

inline bool IRTypeContext::isSubsetOf(Type a, Type b) const {
  return isSubsetOf(a.id_, b.id_);
}

inline bool IRTypeContext::areDisjoint(Type a, Type b) const {
  return areDisjoint(a.id_, b.id_);
}

inline Type IRTypeContext::unionTy(Type a, Type b) {
  return Type(unionTy(a.id_, b.id_));
}

inline Type IRTypeContext::intersectTy(Type a, Type b) {
  return Type(intersectTy(a.id_, b.id_));
}

inline Type IRTypeContext::subtractTy(Type a, Type b) {
  return Type(subtractTy(a.id_, b.id_));
}

inline unsigned IRTypeContext::countKinds(Type t) const {
  return countKinds(t.id_);
}

inline TypeKind IRTypeContext::getFirstKind(Type t) const {
  return getFirstKind(t.id_);
}

inline void IRTypeContext::format(llvh::raw_ostream &OS, Type t) const {
  format(OS, t.id_);
}

} // namespace hermes

#endif // HERMES_IR_IRTYPECONTEXT_INLINE_H
