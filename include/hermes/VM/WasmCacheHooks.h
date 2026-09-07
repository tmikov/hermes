/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#ifndef HERMES_VM_WASMCACHEHOOKS_H
#define HERMES_VM_WASMCACHEHOOKS_H

#include <cstddef>
#include <cstdint>

namespace hermes {
namespace vm {

/// An embedder-supplied cache for compiled Wasm modules.
///
/// Plain C function pointers, and no dependency in either direction: the VM
/// does not know about NAPI and the public C header does not include VM
/// headers, so the API layer copies its own struct onto this one field by
/// field.
///
/// Bytes handed back by \c lookup are TRUSTED, exactly as the bytes from an
/// IWasmModuleResolver are: they take the precompiled path without validation
/// or content sniffing. An embedder returning anything other than .hbc
/// produced by this Hermes version has the same consequences as handing
/// untrusted bytes to any other bytecode entry point.
///
/// OWNERSHIP: \c lookup always produces a store token, on hit and on miss
/// alike, and the VM calls exactly one of \c store or \c discard for it.
struct WasmCacheHooks {
  void *ctx = nullptr;

  /// Consult the cache for \p wasm.
  /// On a hit: sets \p hbc / \p hbcSize, and the finalizer the VM calls when
  /// the bytecode is no longer referenced (either may be null for a buffer
  /// the embedder manages itself), and returns true.
  /// On a miss: returns false. Sets \p storeToken either way.
  ///
  /// \p codegenConfig / \p codegenConfigSize describe everything about this
  /// build that affects generated code. It is opaque -- do not parse it --
  /// and MUST be part of the cache key, or the cache serves bytecode built
  /// under different rules. It is valid only for the duration of this call.
  bool (*lookup)(
      void *ctx,
      const uint8_t *wasm,
      size_t wasmSize,
      const uint8_t *codegenConfig,
      size_t codegenConfigSize,
      const uint8_t **hbc,
      size_t *hbcSize,
      void (**finalizeCb)(const uint8_t *, size_t, void *),
      void **finalizeHint,
      void **storeToken) = nullptr;

  /// Persist \p hbc against the identity in \p storeToken, and release it.
  void (*store)(
      void *ctx, void *storeToken, const uint8_t *hbc, size_t hbcSize) =
      nullptr;

  /// Release \p storeToken without persisting anything.
  void (*discard)(void *ctx, void *storeToken) = nullptr;

  bool installed() const {
    return lookup != nullptr && store != nullptr && discard != nullptr;
  }
};

} // namespace vm
} // namespace hermes

#endif // HERMES_VM_WASMCACHEHOOKS_H
