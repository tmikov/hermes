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
  ///
  /// DO NOT INLINE THIS INTO THE CELL. Being a separately allocated
  /// unique_ptr pointee is load-bearing, not incidental:
  /// WebAssembly.Module.exports() and .imports() hold a REFERENCE into
  /// exportDescs/importDescs across a putNamed_RJS that can run a user
  /// setter, and they survive it because the struct does not move when the
  /// cell does. Storing it inline would make both of them use-after-frees --
  /// which is exactly what JSWebAssemblyTag::parameters_, an inline
  /// std::vector, was, until wasmExceptionConstructor was changed to copy it
  /// out. Those two call sites carry the full argument, including the two
  /// further conditions unique_ptr does not establish (a rooted owner, and no
  /// descriptor mutation during the loop).
  std::unique_ptr<WasmModuleData> moduleData_;
};

} // namespace vm
} // namespace hermes

#endif // HERMES_VM_JSWEBASSEMBLYMODULE_H
