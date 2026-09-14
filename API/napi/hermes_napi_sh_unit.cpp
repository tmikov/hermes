/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes_napi_internal.h"

#include "hermes/VM/StaticHUtils.h"
#include "hermes/VM/static_h.h"

/// No fixture built from bundled MODULES can reach the failure branch below.
/// Every module compiles inside `(function (exports, require, module,
/// __filename, __dirname) { ... })`, so a unit's global code does exactly
/// one thing -- create that closure and return it. A throwing module body
/// throws when the LOADER calls the returned closure, which happens after
/// this function has already returned napi_ok. The failure branch is real
/// and required regardless (an unwrapped unit reaches it directly), and is
/// exercised in NapiShUnitTest.cpp with a hand-built throwing unit.
napi_status NAPI_CDECL
hermes_init_sh_unit(napi_env env, SHUnitCreator creator, napi_value *result) {
  NAPI_PREAMBLE(env);
  CHECK_ARG(env, creator);
  CHECK_ARG(env, result);

  SHLegacyValue value;
  // The guarded form, not _sh_unit_init: it supplies the GCScope and
  // catches Static Hermes's longjmp unwind. Letting a _sh_throw cross this
  // boundary is not something the boundary is built for.
  bool ok = _sh_unit_init_guarded(
      hermes::vm::getSHRuntime(env->runtime), creator, &value);

  if (!ok) {
    // Deliberately NOT captureRuntimeException(): _sh_catch() has already
    // taken the thrown value out of the runtime and cleared it, so asking
    // the runtime again finds nothing pending.
    env->pendingException = hermes::vm::HermesValue::fromRaw(value.raw);
    env->hasPendingException = true;
    return napi_set_last_error(env, napi_pending_exception);
  }

  // Rooted before it leaves: SHLegacyValue and HermesValue share a
  // representation, so the reinterpretation is sound here, but handing an
  // unrooted value out as a napi_value is not.
  *result = env->addToCurrentScope(hermes::vm::HermesValue::fromRaw(value.raw));
  return napi_clear_last_error(env);
}
