/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

/**
 * native-unit-throws-throwing.js_ is an ordinary JS unit, compiled with
 * --exported-unit but WITHOUT -wasm, so SH.cpp does not generate the
 * self-registering constructor a real Wasm unit gets (see
 * options.wasmUnit in lib/BCGen/SH/SH.cpp). This shim performs that
 * registration by hand, so the JS unit's throwing top level is reachable
 * through WebAssembly.Module.fromNativeUnit() like any other native unit.
 * The shim states the same codegen configuration a real Wasm unit would.
 */

#include <hermes/VM/static_h.h>

/* The configuration goodmod -- a real Wasm unit, compiled by this very
 * shermes -- carries, extracted by the RUN lines from its generated C. The
 * runtime runs a unit only when its configuration matches its own, so a
 * registration stating none, or a hard-coded string that drifted with the
 * next version bump, would be refused before the throwing top level ran:
 * the test would then report a TypeError instead of "boom". */
#include "goodmod-config.h"
#ifndef GOODMOD_CODEGEN_CONFIG
#error "goodmod-config.h did not define GOODMOD_CODEGEN_CONFIG"
#endif

extern SHUnit *sh_export_throwmod(void);

static SHWasmUnitReg reg = {
    "throwmod", sh_export_throwmod, GOODMOD_CODEGEN_CONFIG, 0};

__attribute__((constructor)) static void registerThrowmod(void) {
  _sh_wasm_register_unit(&reg);
}
