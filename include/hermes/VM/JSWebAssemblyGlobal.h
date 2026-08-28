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
  ///
  /// THESE NUMERIC VALUES ARE AN ABI, not an implementation detail.
  /// `WasmIRGen::globalValTypeCode` hardcodes 0/1/2/3 and emits them as IR
  /// literals into the wasmLinkGlobal call that validates every global
  /// import, because the Wasm frontend does not depend on VM headers.
  /// Reordering this enum without updating that function would silently make
  /// every global import accept the wrong type. The static_asserts below turn
  /// that into a build error.
  enum class ValType : uint8_t { I32, I64, F32, F64 };
  static_assert(
      static_cast<uint8_t>(ValType::I32) == 0 &&
          static_cast<uint8_t>(ValType::I64) == 1 &&
          static_cast<uint8_t>(ValType::F32) == 2 &&
          static_cast<uint8_t>(ValType::F64) == 3,
      "ValType codes are baked into WasmIRGen::globalValTypeCode; update it "
      "before changing them");

  /// Get the stored value. Only meaningful for i32/f32/f64; an i64 global
  /// stores its value in the 64-bit slot instead, because a double cannot
  /// represent every i64 exactly.
  double getValue() const {
    return value_;
  }

  /// Set the stored value.
  void setValue(double val) {
    value_ = val;
  }

  /// Get the stored i64 value. Only meaningful when getValType() is I64.
  int64_t getI64Value() const {
    return i64Value_;
  }

  /// Set the stored i64 value.
  void setI64Value(int64_t val) {
    i64Value_ = val;
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

  /// The global's current value, for i32/f32/f64.
  double value_{0.0};

  /// The global's current value, for i64. A double cannot hold every i64
  /// exactly, so i64 globals are stored here and surfaced to JS as a BigInt,
  /// which is also what the spec requires of Global.prototype.value.
  int64_t i64Value_{0};

  /// The value type descriptor.
  ValType valType_{ValType::I32};

  /// Whether the global is mutable.
  bool mutable_{false};
};

} // namespace vm
} // namespace hermes

#endif // HERMES_VM_JSWEBASSEMBLYGLOBAL_H
