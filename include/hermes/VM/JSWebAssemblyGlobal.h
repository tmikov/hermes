/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#ifndef HERMES_VM_JSWEBASSEMBLYGLOBAL_H
#define HERMES_VM_JSWEBASSEMBLYGLOBAL_H

#include "hermes/VM/JSObject.h"
#include "hermes/VM/Runtime.h"

namespace hermes {
namespace vm {

/// A JSWebAssemblyGlobal wraps a single WebAssembly global value.
/// Created by `new WebAssembly.Global({value: "i32", mutable: true}, 42)`.
///
/// Stores the global's value and type information. The `.value` getter/setter
/// provides JS access to the global's current value.
class JSWebAssemblyGlobal final : public JSObject {
 public:
  using Super = JSObject;
  static const ObjectVTable vt;

  static constexpr CellKind getCellKind() {
    return CellKind::JSWebAssemblyGlobalKind;
  }
  static bool classof(const GCCell *cell) {
    return cell->getKind() == CellKind::JSWebAssemblyGlobalKind;
  }

  /// Create a JSWebAssemblyGlobal with the given prototype.
  static PseudoHandle<JSWebAssemblyGlobal> create(
      Runtime &runtime,
      Handle<JSObject> prototype);

  /// Value type enum matching WebAssembly value types.
  enum class ValType : uint8_t { I32, I64, F32, F64 };

  /// Get the stored value.
  double getValue() const {
    return value_;
  }

  /// Set the stored value.
  void setValue(double val) {
    value_ = val;
  }

  /// Get the value type.
  ValType getValType() const {
    return valType_;
  }

  /// Set the value type.
  void setValType(ValType vt) {
    valType_ = vt;
  }

  /// Check if the global is mutable.
  bool isMutable() const {
    return mutable_;
  }

  /// Set mutability.
  void setMutable(bool m) {
    mutable_ = m;
  }

 public:
  JSWebAssemblyGlobal(
      Runtime &runtime,
      Handle<JSObject> parent,
      Handle<HiddenClass> clazz)
      : JSObject(runtime, *parent, *clazz) {}

  ~JSWebAssemblyGlobal() = default;

 private:
  friend void JSWebAssemblyGlobalBuildMeta(
      const GCCell *cell,
      Metadata::Builder &mb);

  /// The global's current value (all Wasm numeric types fit in double).
  double value_{0.0};

  /// The value type descriptor.
  ValType valType_{ValType::I32};

  /// Whether the global is mutable.
  bool mutable_{false};
};

} // namespace vm
} // namespace hermes

#endif // HERMES_VM_JSWEBASSEMBLYGLOBAL_H
