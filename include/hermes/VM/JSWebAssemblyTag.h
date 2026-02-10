/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#ifndef HERMES_VM_JSWEBASSEMBLYTAG_H
#define HERMES_VM_JSWEBASSEMBLYTAG_H

#include "hermes/VM/JSObject.h"
#include "hermes/VM/Runtime.h"

#include <vector>

namespace hermes {
namespace vm {

/// A JSWebAssemblyTag represents a WebAssembly exception tag type.
/// Created by `new WebAssembly.Tag({parameters: ['i32', 'f64']})`.
///
/// Stores the tag's parameter types (its signature). Used to identify
/// exception types when catching Wasm exceptions from JavaScript.
class JSWebAssemblyTag final : public JSObject {
 public:
  using Super = JSObject;
  static const ObjectVTable vt;

  static constexpr CellKind getCellKind() {
    return CellKind::JSWebAssemblyTagKind;
  }
  static bool classof(const GCCell *cell) {
    return cell->getKind() == CellKind::JSWebAssemblyTagKind;
  }

  /// Value type enum matching WebAssembly value types.
  enum class ValType : uint8_t { I32, I64, F32, F64 };

  /// Create a JSWebAssemblyTag with the given prototype.
  static PseudoHandle<JSWebAssemblyTag> create(
      Runtime &runtime,
      Handle<JSObject> prototype);

  /// Get the parameter types.
  const std::vector<ValType> &getParameters() const {
    return parameters_;
  }

  /// Set the parameter types.
  void setParameters(std::vector<ValType> params) {
    parameters_ = std::move(params);
  }

  /// Destructor callback for GC.
  static void _finalizeImpl(GCCell *cell, GC &gc);

  /// Report external memory usage.
  static size_t _mallocSizeImpl(GCCell *cell);

 public:
  JSWebAssemblyTag(
      Runtime &runtime,
      Handle<JSObject> parent,
      Handle<HiddenClass> clazz)
      : JSObject(runtime, *parent, *clazz) {}

  ~JSWebAssemblyTag() = default;

 private:
  friend void JSWebAssemblyTagBuildMeta(
      const GCCell *cell,
      Metadata::Builder &mb);

  /// The tag's parameter types (its signature).
  std::vector<ValType> parameters_;
};

} // namespace vm
} // namespace hermes

#endif // HERMES_VM_JSWEBASSEMBLYTAG_H
