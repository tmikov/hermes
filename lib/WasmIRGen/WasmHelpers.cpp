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

Instruction *WasmHelpers::emitI32Clz(Value *a) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI32Clz, {a});
}

Instruction *WasmHelpers::emitI32Ctz(Value *a) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI32Ctz, {a});
}

Instruction *WasmHelpers::emitI32Popcnt(Value *a) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI32Popcnt, {a});
}

Instruction *WasmHelpers::emitI32Rotl(Value *a, Value *b) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI32Rotl, {a, b});
}

Instruction *WasmHelpers::emitI32Rotr(Value *a, Value *b) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI32Rotr, {a, b});
}

Instruction *WasmHelpers::emitI32TruncF64S(Value *a) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI32TruncF64S, {a});
}

Instruction *WasmHelpers::emitI32TruncF64U(Value *a) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI32TruncF64U, {a});
}

Instruction *WasmHelpers::emitI32TruncSatF64S(Value *a) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI32TruncSatF64S, {a});
}

Instruction *WasmHelpers::emitI32TruncSatF64U(Value *a) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI32TruncSatF64U, {a});
}

Instruction *WasmHelpers::emitI32ReinterpretF32(Value *a) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI32ReinterpretF32, {a});
}

Instruction *WasmHelpers::emitF32ReinterpretI32(Value *a) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmF32ReinterpretI32, {a});
}

Instruction *WasmHelpers::emitF64Copysign(Value *a, Value *b) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmF64Copysign, {a, b});
}

Instruction *WasmHelpers::emitF32Copysign(Value *a, Value *b) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmF32Copysign, {a, b});
}

} // namespace wasm
} // namespace hermes
