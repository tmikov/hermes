/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#ifndef HERMES_VM_JSWEBASSEMBLYTABLE_H
#define HERMES_VM_JSWEBASSEMBLYTABLE_H

#include "hermes/VM/JSArray.h"
#include "hermes/VM/JSObject.h"
#include "hermes/VM/Runtime.h"

namespace hermes {
namespace vm {

/// A JSWebAssemblyTable wraps a WebAssembly table.
/// Created by `new WebAssembly.Table({element: "anyfunc", initial: N})`.
///
/// The table stores function references (or null) and is backed by a
/// JSArray accessible internally. Methods: get(idx), set(idx, func),
/// grow(delta), and a length getter.
class JSWebAssemblyTable final : public JSObject {
 public:
  using Super = JSObject;
  static const ObjectVTable vt;

  static constexpr CellKind getCellKind() {
    return CellKind::JSWebAssemblyTableKind;
  }
  static bool classof(const GCCell *cell) {
    return cell->getKind() == CellKind::JSWebAssemblyTableKind;
  }

  /// Create a JSWebAssemblyTable with the given prototype.
  static PseudoHandle<JSWebAssemblyTable> create(
      Runtime &runtime,
      Handle<JSObject> prototype);

  /// Get the underlying JSArray holding table elements.
  JSArray *getElements(Runtime &runtime) const {
    return elements_.get(runtime);
  }

  /// Set the underlying JSArray holding table elements.
  void setElements(Runtime &runtime, JSArray *arr) {
    elements_.set(runtime, arr, runtime.getHeap());
  }

  /// Get the underlying JSArray holding table entry type ids.
  JSArray *getTypes(Runtime &runtime) const {
    return types_.get(runtime);
  }

  /// Set the underlying JSArray holding table entry type ids.
  void setTypes(Runtime &runtime, JSArray *arr) {
    types_.set(runtime, arr, runtime.getHeap());
  }

  /// Get the underlying JSArray holding the Exported Function of each entry.
  JSArray *getExported(Runtime &runtime) const {
    return exported_.get(runtime);
  }

  /// Set the underlying JSArray holding the Exported Function of each entry.
  void setExported(Runtime &runtime, JSArray *arr) {
    exported_.set(runtime, arr, runtime.getHeap());
  }

  /// Get the maximum table size (UINT32_MAX means no explicit maximum).
  uint32_t getMaxSize() const {
    return maxSize_;
  }

  /// Set the maximum table size.
  void setMaxSize(uint32_t maxSize) {
    maxSize_ = maxSize;
  }

 public:
  JSWebAssemblyTable(
      Runtime &runtime,
      Handle<JSObject> parent,
      Handle<HiddenClass> clazz)
      : JSObject(runtime, *parent, *clazz),
        elements_(runtime, nullptr, runtime.getHeap()),
        types_(runtime, nullptr, runtime.getHeap()),
        exported_(runtime, nullptr, runtime.getHeap()) {}

  ~JSWebAssemblyTable() = default;

 private:
  friend void JSWebAssemblyTableBuildMeta(
      const GCCell *cell,
      Metadata::Builder &mb);

  /// The JSArray backing the table entries.
  GCPointer<JSArray> elements_;

  /// The JSArray holding the interned type id of each entry, parallel to
  /// \c elements_. Entries set from JS have no type id and read as empty,
  /// which call_indirect refuses.
  GCPointer<JSArray> types_;

  /// The JSArray holding the Exported Function of each entry (or null),
  /// parallel to \c elements_. This is the funcref value everything outside
  /// call_indirect sees: table.get, Table.prototype.get, and every funcref
  /// that travels on the Wasm value stack. \c elements_ keeps the internal
  /// closure instead, so the indirect-call hot path is unchanged.
  ///
  /// The three arrays agree slot by slot: either all three are null/empty, or
  /// \c elements_[i] is the closure, \c types_[i] its interned type id, and
  /// \c exported_[i] the wrapper carrying that same pair. The invariant is
  /// maintained by funnelling every write through wasmTableSetSlot /
  /// wasmTableCopySlots rather than by discipline at each writer.
  GCPointer<JSArray> exported_;

  /// Maximum table size. UINT32_MAX = no explicit maximum; the spec's
  /// limit is 2^32-1 entries, so a genuine maximum of UINT32_MAX and no
  /// maximum behave identically. 0 is a real, declarable maximum -- a
  /// {initial: 0, maximum: 0} table must never grow.
  uint32_t maxSize_{UINT32_MAX};
};

} // namespace vm
} // namespace hermes

#endif // HERMES_VM_JSWEBASSEMBLYTABLE_H
