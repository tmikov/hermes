/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#ifndef HERMES_VM_JSWEBASSEMBLYEXCEPTION_H
#define HERMES_VM_JSWEBASSEMBLYEXCEPTION_H

#include "hermes/VM/JSArray.h"
#include "hermes/VM/JSObject.h"
#include "hermes/VM/JSWebAssemblyTag.h"
#include "hermes/VM/Runtime.h"

namespace hermes {
namespace vm {

/// A JSWebAssemblyException represents a thrown WebAssembly exception.
/// Created by `new WebAssembly.Exception(tag, [v0, v1, ...])`.
///
/// Stores a reference to the tag that identifies the exception type and
/// a JSArray of payload values. The `.is(tag)` method checks if this
/// exception matches a given tag, and `.getArg(tag, index)` extracts
/// a payload value.
class JSWebAssemblyException final : public JSObject {
 public:
  using Super = JSObject;
  static const ObjectVTable vt;

  static constexpr CellKind getCellKind() {
    return CellKind::JSWebAssemblyExceptionKind;
  }
  static bool classof(const GCCell *cell) {
    return cell->getKind() == CellKind::JSWebAssemblyExceptionKind;
  }

  /// Create a JSWebAssemblyException with the given prototype.
  static PseudoHandle<JSWebAssemblyException> create(
      Runtime &runtime,
      Handle<JSObject> prototype);

  /// Get the tag associated with this exception.
  JSWebAssemblyTag *getTag(Runtime &runtime) const {
    return tag_.get(runtime);
  }

  /// Set the tag associated with this exception.
  void setTag(Runtime &runtime, JSWebAssemblyTag *tag) {
    tag_.set(runtime, tag, runtime.getHeap());
  }

  /// Get the payload values array.
  JSArray *getPayload(Runtime &runtime) const {
    return payload_.get(runtime);
  }

  /// Set the payload values array.
  void setPayload(Runtime &runtime, JSArray *arr) {
    payload_.set(runtime, arr, runtime.getHeap());
  }

 public:
  JSWebAssemblyException(
      Runtime &runtime,
      Handle<JSObject> parent,
      Handle<HiddenClass> clazz)
      : JSObject(runtime, *parent, *clazz),
        tag_(runtime, nullptr, runtime.getHeap()),
        payload_(runtime, nullptr, runtime.getHeap()) {}

  ~JSWebAssemblyException() = default;

 private:
  friend void JSWebAssemblyExceptionBuildMeta(
      const GCCell *cell,
      Metadata::Builder &mb);

  /// The tag identifying the exception type.
  GCPointer<JSWebAssemblyTag> tag_;

  /// The payload values (one per tag parameter).
  GCPointer<JSArray> payload_;
};

} // namespace vm
} // namespace hermes

#endif // HERMES_VM_JSWEBASSEMBLYEXCEPTION_H
