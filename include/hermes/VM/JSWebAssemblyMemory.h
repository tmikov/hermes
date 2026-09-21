/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#ifndef HERMES_VM_JSWEBASSEMBLYMEMORY_H
#define HERMES_VM_JSWEBASSEMBLYMEMORY_H

#include "hermes/VM/JSArrayBuffer.h"
#include "hermes/VM/JSObject.h"
#include "hermes/VM/Runtime.h"

namespace hermes {
namespace vm {

/// A JSWebAssemblyMemory wraps a WebAssembly linear memory.
/// Created by `new WebAssembly.Memory({initial: N, maximum: M})`.
///
/// The memory is backed by a JSArrayBuffer accessible via the `.buffer`
/// property. The `.grow(delta)` method grows the memory by `delta` pages
/// (each page is 64KB).
class JSWebAssemblyMemory final : public JSObject {
 public:
  using Super = JSObject;
  static const ObjectVTable vt;

  static constexpr CellKind getCellKind() {
    return CellKind::JSWebAssemblyMemoryKind;
  }
  static bool classof(const GCCell *cell) {
    return cell->getKind() == CellKind::JSWebAssemblyMemoryKind;
  }

  /// Create a JSWebAssemblyMemory with the given prototype.
  static PseudoHandle<JSWebAssemblyMemory> create(
      Runtime &runtime,
      Handle<JSObject> prototype);

  /// Get the underlying ArrayBuffer.
  JSArrayBuffer *getBuffer(Runtime &runtime) const {
    return buffer_.get(runtime);
  }

  /// Set the underlying ArrayBuffer.
  void setBuffer(Runtime &runtime, JSArrayBuffer *buf) {
    buffer_.set(runtime, buf, runtime.getHeap());
  }

  /// Get the maximum number of pages (UINT32_MAX means no explicit maximum).
  /// The distinction matters at link time: a module declaring a maximum can
  /// only import a memory that declares one too, so "no explicit maximum" and
  /// "the largest maximum Wasm allows" must not spell the same thing.
  uint32_t getMaxPages() const {
    return maxPages_;
  }

  /// Set the maximum number of pages.
  void setMaxPages(uint32_t maxPages) {
    maxPages_ = maxPages;
  }

 public:
  JSWebAssemblyMemory(
      Runtime &runtime,
      Handle<JSObject> parent,
      Handle<HiddenClass> clazz)
      : JSObject(runtime, *parent, *clazz),
        buffer_(runtime, nullptr, runtime.getHeap()) {}

  ~JSWebAssemblyMemory() = default;

 private:
  friend void JSWebAssemblyMemoryBuildMeta(
      const GCCell *cell,
      Metadata::Builder &mb);

  /// The underlying ArrayBuffer backing the linear memory.
  GCPointer<JSArrayBuffer> buffer_;

  /// Maximum number of pages (UINT32_MAX = no explicit maximum). Growth is
  /// additionally capped at 65536 pages -- the Wasm maximum -- everywhere,
  /// so the sentinel never lets a memory exceed that.
  uint32_t maxPages_{UINT32_MAX};
};

} // namespace vm
} // namespace hermes

#endif // HERMES_VM_JSWEBASSEMBLYMEMORY_H
