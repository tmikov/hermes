/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/WasmIRGen/WasmHelpers.h"

namespace hermes {
namespace wasm {

Instruction *WasmHelpers::emitTrap() {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmTrap, {});
}

} // namespace wasm
} // namespace hermes
