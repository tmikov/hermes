/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/VM/JSWebAssemblyTag.h"

#include "hermes/VM/BuildMetadata.h"
#include "hermes/VM/Runtime-inline.h"

namespace hermes {
namespace vm {

//===----------------------------------------------------------------------===//
// class JSWebAssemblyTag

const ObjectVTable JSWebAssemblyTag::vt{
    VTable(
        CellKind::JSWebAssemblyTagKind,
        cellSize<JSWebAssemblyTag>(),
        /* allowLargeAlloc */ false,
        _finalizeImpl,
        _mallocSizeImpl),
    _getOwnIndexedRangeImpl,
    _haveOwnIndexedImpl,
    _getOwnIndexedPropertyFlagsImpl,
    _getOwnIndexedImpl,
    _setOwnIndexedImpl,
    _deleteOwnIndexedImpl,
    _checkAllOwnIndexedImpl,
};

void JSWebAssemblyTagBuildMeta(
    const GCCell *cell,
    Metadata::Builder &mb) {
  mb.addJSObjectOverlapSlots(
      JSObject::numOverlapSlots<JSWebAssemblyTag>());
  JSObjectBuildMeta(cell, mb);
  mb.setVTable(&JSWebAssemblyTag::vt);
  // No GC pointer fields — parameters_ is a plain std::vector.
}

PseudoHandle<JSWebAssemblyTag> JSWebAssemblyTag::create(
    Runtime &runtime,
    Handle<JSObject> parentHandle) {
  auto *cell = runtime.makeAFixed<JSWebAssemblyTag, HasFinalizer::Yes>(
      runtime,
      parentHandle,
      runtime.getHiddenClassForPrototype(
          *parentHandle, numOverlapSlots<JSWebAssemblyTag>()));
  return JSObjectInit::initToPseudoHandle(runtime, cell);
}

void JSWebAssemblyTag::_finalizeImpl(GCCell *cell, GC &) {
  auto *self = vmcast<JSWebAssemblyTag>(cell);
  self->~JSWebAssemblyTag();
}

size_t JSWebAssemblyTag::_mallocSizeImpl(GCCell *cell) {
  auto *self = vmcast<JSWebAssemblyTag>(cell);
  return self->parameters_.capacity() * sizeof(ValType);
}

} // namespace vm
} // namespace hermes
