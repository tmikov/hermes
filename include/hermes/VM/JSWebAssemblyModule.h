/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#ifndef HERMES_VM_JSWEBASSEMBLYMODULE_H
#define HERMES_VM_JSWEBASSEMBLYMODULE_H

#include "hermes/VM/JSObject.h"
#include "hermes/VM/Runtime.h"
#include "hermes/WasmFrontend/WasmModuleData.h"

#include <memory>

namespace hermes {
namespace vm {

/// A JSWebAssemblyModule wraps a compiled WebAssembly module.
/// Created by `new WebAssembly.Module(bytes)`.
class JSWebAssemblyModule final : public JSObject {
 public:
  using Super = JSObject;
  static const ObjectVTable vt;

  static constexpr CellKind getCellKind() {
    return CellKind::JSWebAssemblyModuleKind;
  }
  static bool classof(const GCCell *cell) {
    return cell->getKind() == CellKind::JSWebAssemblyModuleKind;
  }

  /// Create a JSWebAssemblyModule with the given prototype.
  static PseudoHandle<JSWebAssemblyModule> create(
      Runtime &runtime,
      Handle<JSObject> prototype);

  /// Get the module data, or nullptr if not set.
  WasmModuleData *getModuleData() {
    return moduleData_.get();
  }
  const WasmModuleData *getModuleData() const {
    return moduleData_.get();
  }

  /// Set the module data. Takes ownership.
  void setModuleData(std::unique_ptr<WasmModuleData> data) {
    moduleData_ = std::move(data);
  }

 public:
  JSWebAssemblyModule(
      Runtime &runtime,
      Handle<JSObject> parent,
      Handle<HiddenClass> clazz)
      : JSObject(runtime, *parent, *clazz) {}

  ~JSWebAssemblyModule() = default;

 protected:
  static void _finalizeImpl(GCCell *cell, GC &gc);
  static size_t _mallocSizeImpl(GCCell *cell);

 private:
  friend void JSWebAssemblyModuleBuildMeta(
      const GCCell *cell,
      Metadata::Builder &mb);

  /// Opaque module data (metadata + compiled bytecode).
  std::unique_ptr<WasmModuleData> moduleData_;
};

} // namespace vm
} // namespace hermes

#endif // HERMES_VM_JSWEBASSEMBLYMODULE_H
