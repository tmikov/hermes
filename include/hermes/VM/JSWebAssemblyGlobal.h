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
  /// `WasmIRGen::globalValTypeCode` hardcodes 0..5 and emits them as IR
  /// literals into the wasmLinkGlobal call that validates every global
  /// import, because the Wasm frontend does not depend on VM headers.
  /// Reordering this enum without updating that function would silently make
  /// every global import accept the wrong type. The static_asserts below turn
  /// that into a build error.
  ///
  /// There is deliberately no V128. A v128 global is not diagnosed today --
  /// that is Task 12 of the reference-types plan. What keeps v128 out of THIS
  /// enum meanwhile is globalValTypeCode, which maps it to a code no Global
  /// can hold, so no Global object ever matches a v128 declaration and no
  /// v128 export can be wrapped in one. That covers the Global-object routes
  /// only; see the note on globalValTypeCode for what it does not cover.
  enum class ValType : uint8_t { I32, I64, F32, F64, ExternRef, FuncRef };
  static_assert(
      static_cast<uint8_t>(ValType::I32) == 0 &&
          static_cast<uint8_t>(ValType::I64) == 1 &&
          static_cast<uint8_t>(ValType::F32) == 2 &&
          static_cast<uint8_t>(ValType::F64) == 3 &&
          static_cast<uint8_t>(ValType::ExternRef) == 4 &&
          static_cast<uint8_t>(ValType::FuncRef) == 5,
      "ValType codes are baked into WasmIRGen::globalValTypeCode; update it "
      "before changing them");

  /// \return the canonical slot contents, which is already the JS form the
  /// table above value_ describes -- a Number for i32/f32/f64, a BigInt for
  /// i64, the reference itself for externref/funcref. A snapshot reader
  /// returns this unchanged; it needs no per-type dispatch and it allocates
  /// nothing. Meaningless for a LIVE global, whose storage is the module's
  /// frame slot; see isLive().
  HermesValue getValue() const {
    return value_;
  }

  /// Store \p val as this global's i64 value: a BigIntPrimitive wrapped to
  /// 64 bits, which is both the canonical slot content for an I64 global and
  /// what Global.prototype.value must return.
  ///
  /// This ALLOCATES, which is why it is a static taking a handle rather than
  /// a member: the BigInt materialization is a safepoint, so the destination
  /// must be rooted across it and the store can fail. It replaces a scalar
  /// field write that could do neither.
  /// \return EXCEPTION if the BigInt could not be allocated.
  static ExecutionStatus
  setI64Value(Handle<JSWebAssemblyGlobal> self, Runtime &runtime, int64_t val);

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
        // +0 is canonical for the default valType_ of I32, so the slot
        // invariant holds from construction rather than from the first
        // store. Non-pointer, so no constructor write barrier is needed.
        value_(
            HermesValue::encodeTrustedNumberValue(0),
            runtime.getHeap(),
            nullptr),
        getter_(runtime, nullptr, runtime.getHeap()),
        setter_(runtime, nullptr, runtime.getHeap()) {}

  ~JSWebAssemblyGlobal() = default;

 private:
  friend void JSWebAssemblyGlobalBuildMeta(
      const GCCell *cell,
      Metadata::Builder &mb);

  /// The storage funnel, and the reason the two stores below are private.
  /// What access control buys is narrow and worth stating exactly: code
  /// OUTSIDE this class cannot write value_ except through this function, so
  /// a new writer added elsewhere is a build error. It does not make the slot
  /// canonical -- the funnel's arms narrow, but its funcref assertion admits
  /// any object, setValType can change the type out from under a stored
  /// value, and a member added to this class keeps private access, as
  /// setI64Value does. Canonicality is still the callers' dispatch; this only
  /// keeps the set of callers small enough to read.
  friend void setWasmGlobalValue(
      Runtime &runtime,
      JSWebAssemblyGlobal *glob,
      HermesValue val);

  /// Store \p val, which the caller must have already made canonical for
  /// getValType(). Performs the write barrier. Does not allocate, so a raw
  /// pointer to this global stays valid across it.
  void setValue(Runtime &runtime, HermesValue val) {
    value_.set(val, runtime.getHeap());
  }

  /// Store the number \p val, which the caller must have already narrowed to
  /// getValType(). i32/f32/f64 only; setWasmGlobalValue is what narrows.
  /// Does not allocate.
  void setNumberValue(Runtime &runtime, double val) {
    value_.setNonPtr(
        HermesValue::encodeTrustedNumberValue(val), runtime.getHeap());
  }

  /// The global's current value, in the canonical JS form for valType_.
  /// A SNAPSHOT global's single source of truth; unused by a live one, whose
  /// storage is the module's frame Variable (see getter_ below).
  ///
  /// | valType_  | slot holds                          |
  /// |-----------|-------------------------------------|
  /// | I32       | number, ToInt32-narrowed            |
  /// | F32       | number, fround-narrowed             |
  /// | F64       | number                              |
  /// | I64       | BigIntPrimitive*, wrapped to 64 bits|
  /// | ExternRef | any HermesValue                     |
  /// | FuncRef   | null, or an Exported Function       |
  ///
  /// The narrowing and the wrapping happen at STORE time, so every reader is
  /// a plain slot read: getValue() is the answer for every type and the
  /// snapshot readers do no per-type dispatch. Two functions write this field
  /// -- setWasmGlobalValue, which narrows the first three rows and stores the
  /// last two as they stand, and setI64Value, which builds the fourth's
  /// BigInt. The private setters above keep any writer OUTSIDE this class to
  /// the first of those; a member of this class could still add a third, and
  /// the table itself is kept true by what each caller validates before it
  /// stores.
  ///
  /// This replaced a `double value_` plus an `int64_t i64Value_`. It is a
  /// GCHermesValue -- a full 64-bit HermesValue in every heap mode, unlike
  /// GCSmallHermesValue -- because a reference-typed global's value is a GC
  /// pointer and must be traced and write-barriered. The cell does not shrink
  /// as a result: allocation is max(sizeof(Derived), cellSizeJSObject()), so
  /// what the field reduction buys back is an overlap slot, not bytes.
  GCHermesValue value_;

  /// The value type descriptor.
  ValType valType_{ValType::I32};

  /// Whether the global is mutable.
  bool mutable_{false};

  /// For a LIVE global, the closure that reads the module's storage; null for
  /// a snapshot global. A live global stores no value of its own: value_ is
  /// unused and the module's frame Variable is the single source of truth,
  /// which is what makes an exported mutable global a two-way view rather
  /// than a copy taken at instantiation.
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
