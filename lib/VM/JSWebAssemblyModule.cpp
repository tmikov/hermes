/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/VM/JSWebAssemblyModule.h"

#include "hermes/VM/BuildMetadata.h"
#include "hermes/VM/Runtime-inline.h"

namespace hermes {
namespace vm {

//===----------------------------------------------------------------------===//
// class JSWebAssemblyModule

const ObjectVTable JSWebAssemblyModule::vt{
    VTable(
        CellKind::JSWebAssemblyModuleKind,
        cellSize<JSWebAssemblyModule>(),
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

void JSWebAssemblyModuleBuildMeta(
    const GCCell *cell,
    Metadata::Builder &mb) {
  mb.addJSObjectOverlapSlots(
      JSObject::numOverlapSlots<JSWebAssemblyModule>());
  JSObjectBuildMeta(cell, mb);
  mb.setVTable(&JSWebAssemblyModule::vt);
}

PseudoHandle<JSWebAssemblyModule> JSWebAssemblyModule::create(
    Runtime &runtime,
    Handle<JSObject> parentHandle) {
  auto *cell = runtime.makeAFixed<JSWebAssemblyModule, HasFinalizer::Yes>(
      runtime,
      parentHandle,
      runtime.getHiddenClassForPrototype(
          *parentHandle, numOverlapSlots<JSWebAssemblyModule>()));
  return JSObjectInit::initToPseudoHandle(runtime, cell);
}

void JSWebAssemblyModule::_finalizeImpl(GCCell *cell, GC &) {
  auto *self = vmcast<JSWebAssemblyModule>(cell);
  self->~JSWebAssemblyModule();
}

size_t JSWebAssemblyModule::_mallocSizeImpl(GCCell *cell) {
  auto *self = vmcast<JSWebAssemblyModule>(cell);
  if (auto *data = self->getModuleData()) {
    // Approximate: size of the WasmModuleData base + vectors.
    size_t sz = sizeof(WasmModuleData);
    sz += data->exportDescs.capacity() * sizeof(WasmModuleData::ExportDesc);
    sz += data->importDescs.capacity() * sizeof(WasmModuleData::ImportDesc);
    return sz;
  }
  return 0;
}

} // namespace vm
} // namespace hermes
