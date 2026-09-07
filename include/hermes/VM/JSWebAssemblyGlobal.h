/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#ifndef HERMES_VM_JSWEBASSEMBLYGLOBAL_H
#define HERMES_VM_JSWEBASSEMBLYGLOBAL_H

#include "hermes/VM/Callable.h"
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

  /// \return the closure that reads a live global's storage, or nullptr if
  /// this global is a snapshot.
  Callable *getGetter(Runtime &runtime) const {
    return getter_.get(runtime);
  }

  /// Set the closure that reads a live global's storage.
  void setGetter(Runtime &runtime, Callable *fn) {
    getter_.set(runtime, fn, runtime.getHeap());
  }

  /// \return the closure that writes a live mutable global's storage, or
  /// nullptr if this global is a snapshot or is immutable.
  Callable *getSetter(Runtime &runtime) const {
    return setter_.get(runtime);
  }

  /// Set the closure that writes a live mutable global's storage.
  void setSetter(Runtime &runtime, Callable *fn) {
    setter_.set(runtime, fn, runtime.getHeap());
  }

  /// \return true if this global reads and writes a module's storage through
  /// closures rather than holding a value of its own. A live global is always
  /// mutable and therefore always has both closures; wasmMakeGlobal refuses
  /// any other combination, and wasmLinkGlobal depends on that.
  bool isLive(Runtime &runtime) const {
    return getter_.get(runtime) != nullptr;
  }

 public:
  JSWebAssemblyGlobal(
      Runtime &runtime,
      Handle<JSObject> parent,
      Handle<HiddenClass> clazz)
      : JSObject(runtime, *parent, *clazz),
        getter_(runtime, nullptr, runtime.getHeap()),
        setter_(runtime, nullptr, runtime.getHeap()) {}

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

  /// For a LIVE global, the closure that reads the module's storage; null for
  /// a snapshot global. A live global stores no value of its own: value_ and
  /// i64Value_ are unused and the module's frame Variable is the single
  /// source of truth, which is what makes an exported mutable global a
  /// two-way view rather than a copy taken at instantiation.
  GCPointer<Callable> getter_;

  /// For a live global, the closure that writes the module's storage; null
  /// for a snapshot one. Non-null exactly when getter_ is: a live global is
  /// always mutable (wasmMakeGlobal refuses an immutable live global, and
  /// wasmLinkGlobal's success answer depends on that invariant).
  GCPointer<Callable> setter_;
};

} // namespace vm
} // namespace hermes

#endif // HERMES_VM_JSWEBASSEMBLYGLOBAL_H
