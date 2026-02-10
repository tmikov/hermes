/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/VM/JSWebAssemblyException.h"

#include "hermes/VM/BuildMetadata.h"
#include "hermes/VM/Runtime-inline.h"

namespace hermes {
namespace vm {

//===----------------------------------------------------------------------===//
// class JSWebAssemblyException

const ObjectVTable JSWebAssemblyException::vt{
    VTable(
        CellKind::JSWebAssemblyExceptionKind,
        cellSize<JSWebAssemblyException>()),
    _getOwnIndexedRangeImpl,
    _haveOwnIndexedImpl,
    _getOwnIndexedPropertyFlagsImpl,
    _getOwnIndexedImpl,
    _setOwnIndexedImpl,
    _deleteOwnIndexedImpl,
    _checkAllOwnIndexedImpl,
};

void JSWebAssemblyExceptionBuildMeta(
    const GCCell *cell,
    Metadata::Builder &mb) {
  mb.addJSObjectOverlapSlots(
      JSObject::numOverlapSlots<JSWebAssemblyException>());
  JSObjectBuildMeta(cell, mb);
  const auto *self = static_cast<const JSWebAssemblyException *>(cell);
  mb.setVTable(&JSWebAssemblyException::vt);
  mb.addField("tag", &self->tag_);
  mb.addField("payload", &self->payload_);
}

PseudoHandle<JSWebAssemblyException> JSWebAssemblyException::create(
    Runtime &runtime,
    Handle<JSObject> parentHandle) {
  auto *cell = runtime.makeAFixed<JSWebAssemblyException>(
      runtime,
      parentHandle,
      runtime.getHiddenClassForPrototype(
          *parentHandle, numOverlapSlots<JSWebAssemblyException>()));
  return JSObjectInit::initToPseudoHandle(runtime, cell);
}

} // namespace vm
} // namespace hermes
