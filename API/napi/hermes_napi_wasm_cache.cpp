/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes_napi.h"
#include "hermes_napi_internal.h"

#include "hermes/VM/WasmCacheHooks.h"

#include <cstddef>

napi_status NAPI_CDECL hermes_set_wasm_cache(
    napi_env env, const hermes_wasm_cache_callbacks *callbacks) {
#ifdef HERMES_ENABLE_WASM
  NAPI_PREAMBLE(env);
  hermes::vm::Runtime &runtime = env->runtime;

  if (callbacks == nullptr) {
    runtime.setWasmCacheHooks(hermes::vm::WasmCacheHooks{});
    return napi_clear_last_error(env);
  }
  // The struct as it was before its optional fields were appended. Demanding
  // the CURRENT sizeof would reject every caller compiled against an older
  // header -- appending a field would then be a breaking change.
  constexpr size_t kMinStructSize =
      offsetof(hermes_wasm_cache_callbacks, lookup_native);
  if (callbacks->struct_size < kMinStructSize)
    return napi_set_last_error(env, napi_invalid_arg);
  CHECK_ARG(env, callbacks->lookup);
  CHECK_ARG(env, callbacks->store);
  CHECK_ARG(env, callbacks->discard);

  hermes::vm::WasmCacheHooks hooks{};
  hooks.ctx = callbacks->ctx;
  hooks.lookup = callbacks->lookup;
  hooks.store = callbacks->store;
  hooks.discard = callbacks->discard;
  // Read only when the caller's struct reaches the whole field: a shorter
  // struct was compiled before the field existed, and the bytes past its
  // end are not the caller's.
  if (callbacks->struct_size >=
      offsetof(hermes_wasm_cache_callbacks, lookup_native) +
          sizeof(callbacks->lookup_native))
    hooks.lookupNative = callbacks->lookup_native;
  if (callbacks->struct_size >=
      offsetof(hermes_wasm_cache_callbacks, accepted) +
          sizeof(callbacks->accepted))
    hooks.accepted = callbacks->accepted;
  runtime.setWasmCacheHooks(hooks);
  return napi_clear_last_error(env);
#else
  (void)callbacks;
  NAPI_PREAMBLE(env);
  return napi_set_last_error(env, napi_generic_failure);
#endif
}
