/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#ifndef HERMES_VM_JSWEBASSEMBLYINSTANCE_H
#define HERMES_VM_JSWEBASSEMBLYINSTANCE_H

#include "hermes/VM/JSObject.h"
#include "hermes/VM/Runtime.h"

namespace hermes {
namespace vm {

/// A JSWebAssemblyInstance wraps an instantiated WebAssembly module.
/// Created by `new WebAssembly.Instance(module, importObject)`.
///
/// The instance stores the exports object as an own property named "exports"
/// (a frozen object containing wrapped exported functions). The module
/// reference is not stored on the Instance (the spec does not require it).
class JSWebAssemblyInstance final : public JSObject {
 public:
  using Super = JSObject;
  static const ObjectVTable vt;

  static constexpr CellKind getCellKind() {
    return CellKind::JSWebAssemblyInstanceKind;
  }
  static bool classof(const GCCell *cell) {
    return cell->getKind() == CellKind::JSWebAssemblyInstanceKind;
  }

  /// Create a JSWebAssemblyInstance with the given prototype.
  static PseudoHandle<JSWebAssemblyInstance> create(
      Runtime &runtime,
      Handle<JSObject> prototype);

 public:
  JSWebAssemblyInstance(
      Runtime &runtime,
      Handle<JSObject> parent,
      Handle<HiddenClass> clazz)
      : JSObject(runtime, *parent, *clazz) {}

  ~JSWebAssemblyInstance() = default;

 private:
  friend void JSWebAssemblyInstanceBuildMeta(
      const GCCell *cell,
      Metadata::Builder &mb);
};

} // namespace vm
} // namespace hermes

#endif // HERMES_VM_JSWEBASSEMBLYINSTANCE_H
