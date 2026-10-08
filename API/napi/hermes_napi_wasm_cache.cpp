/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes_napi.h"
#include "hermes_napi_internal.h"

#include "hermes/VM/WasmCacheHooks.h"

napi_status NAPI_CDECL hermes_set_wasm_cache(
    napi_env env, const hermes_wasm_cache_callbacks *callbacks) {
#ifdef HERMES_ENABLE_WASM
  NAPI_PREAMBLE(env);
  hermes::vm::Runtime &runtime = env->runtime;

  if (callbacks == nullptr) {
    runtime.setWasmCacheHooks(hermes::vm::WasmCacheHooks{});
    return napi_clear_last_error(env);
  }
  if (callbacks->struct_size < sizeof(hermes_wasm_cache_callbacks))
    return napi_set_last_error(env, napi_invalid_arg);
  CHECK_ARG(env, callbacks->lookup);
  CHECK_ARG(env, callbacks->store);
  CHECK_ARG(env, callbacks->discard);

  hermes::vm::WasmCacheHooks hooks{};
  hooks.ctx = callbacks->ctx;
  hooks.lookup = callbacks->lookup;
  hooks.store = callbacks->store;
  hooks.discard = callbacks->discard;
  runtime.setWasmCacheHooks(hooks);
  return napi_clear_last_error(env);
#else
  (void)callbacks;
  NAPI_PREAMBLE(env);
  return napi_set_last_error(env, napi_generic_failure);
#endif
}
