/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/VM/JSWebAssemblyMemory.h"

#include "hermes/VM/BuildMetadata.h"
#include "hermes/VM/Runtime-inline.h"

namespace hermes {
namespace vm {

//===----------------------------------------------------------------------===//
// class JSWebAssemblyMemory

const ObjectVTable JSWebAssemblyMemory::vt{
    VTable(
        CellKind::JSWebAssemblyMemoryKind,
        cellSize<JSWebAssemblyMemory>()),
    _getOwnIndexedRangeImpl,
    _haveOwnIndexedImpl,
    _getOwnIndexedPropertyFlagsImpl,
    _getOwnIndexedImpl,
    _setOwnIndexedImpl,
    _deleteOwnIndexedImpl,
    _checkAllOwnIndexedImpl,
};

void JSWebAssemblyMemoryBuildMeta(
    const GCCell *cell,
    Metadata::Builder &mb) {
  mb.addJSObjectOverlapSlots(
      JSObject::numOverlapSlots<JSWebAssemblyMemory>());
  JSObjectBuildMeta(cell, mb);
  const auto *self = static_cast<const JSWebAssemblyMemory *>(cell);
  mb.setVTable(&JSWebAssemblyMemory::vt);
  mb.addField("buffer", &self->buffer_);
}

PseudoHandle<JSWebAssemblyMemory> JSWebAssemblyMemory::create(
    Runtime &runtime,
    Handle<JSObject> parentHandle) {
  auto *cell = runtime.makeAFixed<JSWebAssemblyMemory>(
      runtime,
      parentHandle,
      runtime.getHiddenClassForPrototype(
          *parentHandle, numOverlapSlots<JSWebAssemblyMemory>()));
  return JSObjectInit::initToPseudoHandle(runtime, cell);
}

} // namespace vm
} // namespace hermes
