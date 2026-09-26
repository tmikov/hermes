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
 */

#include <hermes/VM/static_h.h>

extern SHUnit *sh_export_throwmod(void);

static SHWasmUnitReg reg = {"throwmod", sh_export_throwmod, 0};

__attribute__((constructor)) static void registerThrowmod(void) {
  _sh_wasm_register_unit(&reg);
}
