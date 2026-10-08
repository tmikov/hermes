/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// A Wasm cache whose native tier answers with units linked into this addon,
// so that new WebAssembly.Module(bytes) goes through createModuleFromBytes's
// native tier -- the path WebAssembly.Module.fromNativeUnit() never takes.
// Loaded by the hermes binary through loadNativeModule(), which gives it a
// napi_env on the runtime the test script runs on.
//
// lkmod and lkmod262 are lkmod.wat_ compiled without and with -test262;
// throwmod is native-unit-throws-throwing.js_, an ordinary JS unit whose top
// level throws. The build generates all three as C (unittests/napi/
// CMakeLists.txt).

#include "hermes/VM/static_h.h"
#include "hermes_napi.h"
#include "node_api.h"

#include <cstring>

extern "C" {
SHUnit *sh_export_lkmod(void);
SHUnit *sh_export_lkmod262(void);
SHUnit *sh_export_throwmod(void);
}

namespace {

SHUnitCreator s_answer = nullptr;
int s_native, s_lookup, s_store, s_discard, s_accepted;

SHUnitCreator lookupNative(void *, const uint8_t *, size_t) {
  ++s_native;
  return s_answer;
}

bool lookup(
    void *,
    const uint8_t *,
    size_t,
    const uint8_t *,
    size_t,
    const uint8_t **,
    size_t *,
    void (**)(const uint8_t *, size_t, void *),
    void **,
    void **storeToken) {
  ++s_lookup;
  *storeToken = nullptr;
  return false;
}

void store(void *, void *, const uint8_t *, size_t) {
  ++s_store;
}

void discard(void *, void *) {
  ++s_discard;
}

void accepted(void *, void *) {
  ++s_accepted;
}

// throwmod is a JS unit, and a JS unit does not register itself. Registered
// at module init -- every constructor in this addon has run by then, so
// lkmod's registration exists -- with lkmod's configuration, which is what a
// real Wasm unit compiled by this shermes carries. Stating none, or a
// different one, would get it refused before its top level threw.
SHWasmUnitReg s_throwReg = {"throwmod", sh_export_throwmod, nullptr, nullptr};

napi_value install(napi_env env, napi_callback_info info) {
  size_t argc = 1;
  napi_value argv[1];
  char mode[32] = {0};
  size_t len = 0;
  napi_get_cb_info(env, info, &argc, argv, nullptr, nullptr);
  napi_get_value_string_utf8(env, argv[0], mode, sizeof(mode), &len);
  if (std::strcmp(mode, "lkmod") == 0)
    s_answer = sh_export_lkmod;
  else if (std::strcmp(mode, "lkmod262") == 0)
    s_answer = sh_export_lkmod262;
  else if (std::strcmp(mode, "throwmod") == 0)
    s_answer = sh_export_throwmod;
  else
    s_answer = nullptr;
  s_native = s_lookup = s_store = s_discard = s_accepted = 0;

  hermes_wasm_cache_callbacks cbs{};
  cbs.struct_size = sizeof(cbs);
  cbs.lookup = lookup;
  cbs.store = store;
  cbs.discard = discard;
  cbs.lookup_native = lookupNative;
  cbs.accepted = accepted;
  napi_value ok;
  napi_get_boolean(env, hermes_set_wasm_cache(env, &cbs) == napi_ok, &ok);
  return ok;
}

void setInt(napi_env env, napi_value obj, const char *name, int v) {
  napi_value n;
  napi_create_int32(env, v, &n);
  napi_set_named_property(env, obj, name, n);
}

napi_value counts(napi_env env, napi_callback_info) {
  napi_value obj;
  napi_create_object(env, &obj);
  setInt(env, obj, "native", s_native);
  setInt(env, obj, "lookup", s_lookup);
  setInt(env, obj, "store", s_store);
  setInt(env, obj, "discard", s_discard);
  setInt(env, obj, "accepted", s_accepted);
  return obj;
}

} // namespace

// NAPI_MODULE_INIT supplies C linkage AND default visibility. Both are
// needed: Hermes compiles everything -fvisibility=hidden
// (hermes/CMakeLists.txt), and hermes_napi_load_module() finds the entry
// point with dlsym().
NAPI_MODULE_INIT() {
  const SHWasmUnitReg *lk = _sh_wasm_find_unit_reg(sh_export_lkmod);
  if (lk && !s_throwReg.codegen_config) {
    s_throwReg.codegen_config = lk->codegen_config;
    _sh_wasm_register_unit(&s_throwReg);
  }
  napi_value fn;
  napi_create_function(env, "install", NAPI_AUTO_LENGTH, install, nullptr, &fn);
  napi_set_named_property(env, exports, "install", fn);
  napi_create_function(env, "counts", NAPI_AUTO_LENGTH, counts, nullptr, &fn);
  napi_set_named_property(env, exports, "counts", fn);
  return exports;
}
