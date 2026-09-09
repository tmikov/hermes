/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/VM/JSWebAssemblyGlobal.h"

#include "hermes/VM/BigIntPrimitive.h"
#include "hermes/VM/BuildMetadata.h"
#include "hermes/VM/Runtime-inline.h"

namespace hermes {
namespace vm {

//===----------------------------------------------------------------------===//
// class JSWebAssemblyGlobal

const ObjectVTable JSWebAssemblyGlobal::vt{
    VTable(
        CellKind::JSWebAssemblyGlobalKind,
        cellSize<JSWebAssemblyGlobal>()),
    _getOwnIndexedRangeImpl,
    _haveOwnIndexedImpl,
    _getOwnIndexedPropertyFlagsImpl,
    _getOwnIndexedImpl,
    _setOwnIndexedImpl,
    _deleteOwnIndexedImpl,
    _checkAllOwnIndexedImpl,
};

void JSWebAssemblyGlobalBuildMeta(
    const GCCell *cell,
    Metadata::Builder &mb) {
  mb.addJSObjectOverlapSlots(
      JSObject::numOverlapSlots<JSWebAssemblyGlobal>());
  JSObjectBuildMeta(cell, mb);
  const auto *self = static_cast<const JSWebAssemblyGlobal *>(cell);
  mb.setVTable(&JSWebAssemblyGlobal::vt);
  // value_ IS a GC reference now -- a BigInt for an i64 global, and any JS
  // value for a reference-typed one -- so it is traced. It replaced two plain
  // scalars that were correctly left unregistered; missing this registration
  // would collect a snapshot global's value out from under it. The two
  // closures are GC references for the same reason: without them a live
  // global's accessor is collected.
  mb.addField("value", &self->value_);
  mb.addField("getter", &self->getter_);
  mb.addField("setter", &self->setter_);
}

ExecutionStatus JSWebAssemblyGlobal::setI64Value(
    Handle<JSWebAssemblyGlobal> self,
    Runtime &runtime,
    int64_t val) {
  assert(
      self->getValType() == ValType::I64 &&
      "setI64Value on a global whose valType_ is not I64");
  auto res = BigIntPrimitive::fromSigned(runtime, val);
  if (LLVM_UNLIKELY(res == ExecutionStatus::EXCEPTION))
    return ExecutionStatus::EXCEPTION;
  // fromSigned is the safepoint; `self` is a handle, so the destination is
  // still valid here, which is the whole reason this is not a member.
  self->setValue(runtime, *res);
  return ExecutionStatus::RETURNED;
}

PseudoHandle<JSWebAssemblyGlobal> JSWebAssemblyGlobal::create(
    Runtime &runtime,
    Handle<JSObject> parentHandle) {
  auto *cell = runtime.makeAFixed<JSWebAssemblyGlobal>(
      runtime,
      parentHandle,
      runtime.getHiddenClassForPrototype(
          *parentHandle, numOverlapSlots<JSWebAssemblyGlobal>()));
  return JSObjectInit::initToPseudoHandle(runtime, cell);
}

} // namespace vm
} // namespace hermes
