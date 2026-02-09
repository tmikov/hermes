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

Instruction *WasmHelpers::emitI32DivS(Value *a, Value *b) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI32DivS, {a, b});
}

Instruction *WasmHelpers::emitI32DivU(Value *a, Value *b) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI32DivU, {a, b});
}

Instruction *WasmHelpers::emitI32RemS(Value *a, Value *b) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI32RemS, {a, b});
}

Instruction *WasmHelpers::emitI32RemU(Value *a, Value *b) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI32RemU, {a, b});
}

} // namespace wasm
} // namespace hermes
