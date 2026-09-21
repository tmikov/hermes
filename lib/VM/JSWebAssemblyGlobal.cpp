/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/VM/JSWebAssemblyGlobal.h"

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
  // value_ and i64Value_ are plain scalars and are not registered. The two
  // closures are GC references and must be, or a live global's accessor is
  // collected out from under it.
  mb.addField("getter", &self->getter_);
  mb.addField("setter", &self->setter_);
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
