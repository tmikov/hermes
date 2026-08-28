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

// --- i64 helpers (G.3, G.5) ---

Instruction *WasmHelpers::emitI64HiStash(Value *hi) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64HiStash, {hi});
}

Instruction *WasmHelpers::emitI64HiResult() {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64HiResult, {});
}

Instruction *WasmHelpers::emitI64Add(
    Value *loA, Value *hiA, Value *loB, Value *hiB) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64Add, {loA, hiA, loB, hiB});
}

Instruction *WasmHelpers::emitI64Sub(
    Value *loA, Value *hiA, Value *loB, Value *hiB) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64Sub, {loA, hiA, loB, hiB});
}

Instruction *WasmHelpers::emitI64Mul(
    Value *loA, Value *hiA, Value *loB, Value *hiB) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64Mul, {loA, hiA, loB, hiB});
}

Instruction *WasmHelpers::emitI64DivS(
    Value *loA, Value *hiA, Value *loB, Value *hiB) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64DivS, {loA, hiA, loB, hiB});
}

Instruction *WasmHelpers::emitI64DivU(
    Value *loA, Value *hiA, Value *loB, Value *hiB) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64DivU, {loA, hiA, loB, hiB});
}

Instruction *WasmHelpers::emitI64RemS(
    Value *loA, Value *hiA, Value *loB, Value *hiB) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64RemS, {loA, hiA, loB, hiB});
}

Instruction *WasmHelpers::emitI64RemU(
    Value *loA, Value *hiA, Value *loB, Value *hiB) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64RemU, {loA, hiA, loB, hiB});
}

Instruction *WasmHelpers::emitI64Shl(
    Value *loA, Value *hiA, Value *loB, Value *hiB) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64Shl, {loA, hiA, loB, hiB});
}

Instruction *WasmHelpers::emitI64ShrS(
    Value *loA, Value *hiA, Value *loB, Value *hiB) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64ShrS, {loA, hiA, loB, hiB});
}

Instruction *WasmHelpers::emitI64ShrU(
    Value *loA, Value *hiA, Value *loB, Value *hiB) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64ShrU, {loA, hiA, loB, hiB});
}

Instruction *WasmHelpers::emitI64Rotl(
    Value *loA, Value *hiA, Value *loB, Value *hiB) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64Rotl, {loA, hiA, loB, hiB});
}

Instruction *WasmHelpers::emitI64Rotr(
    Value *loA, Value *hiA, Value *loB, Value *hiB) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64Rotr, {loA, hiA, loB, hiB});
}

Instruction *WasmHelpers::emitI64Clz(Value *lo, Value *hi) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64Clz, {lo, hi});
}

Instruction *WasmHelpers::emitI64Ctz(Value *lo, Value *hi) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64Ctz, {lo, hi});
}

Instruction *WasmHelpers::emitI64Popcnt(Value *lo, Value *hi) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64Popcnt, {lo, hi});
}

Instruction *WasmHelpers::emitI64Eqz(Value *lo, Value *hi) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64Eqz, {lo, hi});
}

Instruction *WasmHelpers::emitI64Eq(
    Value *loA, Value *hiA, Value *loB, Value *hiB) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64Eq, {loA, hiA, loB, hiB});
}

Instruction *WasmHelpers::emitI64Ne(
    Value *loA, Value *hiA, Value *loB, Value *hiB) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64Ne, {loA, hiA, loB, hiB});
}

Instruction *WasmHelpers::emitI64LtS(
    Value *loA, Value *hiA, Value *loB, Value *hiB) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64LtS, {loA, hiA, loB, hiB});
}

Instruction *WasmHelpers::emitI64GtS(
    Value *loA, Value *hiA, Value *loB, Value *hiB) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64GtS, {loA, hiA, loB, hiB});
}

Instruction *WasmHelpers::emitI64LeS(
    Value *loA, Value *hiA, Value *loB, Value *hiB) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64LeS, {loA, hiA, loB, hiB});
}

Instruction *WasmHelpers::emitI64GeS(
    Value *loA, Value *hiA, Value *loB, Value *hiB) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64GeS, {loA, hiA, loB, hiB});
}

Instruction *WasmHelpers::emitI64LtU(
    Value *loA, Value *hiA, Value *loB, Value *hiB) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64LtU, {loA, hiA, loB, hiB});
}

Instruction *WasmHelpers::emitI64GtU(
    Value *loA, Value *hiA, Value *loB, Value *hiB) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64GtU, {loA, hiA, loB, hiB});
}

Instruction *WasmHelpers::emitI64LeU(
    Value *loA, Value *hiA, Value *loB, Value *hiB) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64LeU, {loA, hiA, loB, hiB});
}

Instruction *WasmHelpers::emitI64GeU(
    Value *loA, Value *hiA, Value *loB, Value *hiB) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64GeU, {loA, hiA, loB, hiB});
}

// --- i64 conversion helpers (G.4b) ---

Instruction *WasmHelpers::emitI64TruncF64S(Value *a) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64TruncF64S, {a});
}

Instruction *WasmHelpers::emitI64TruncF64U(Value *a) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64TruncF64U, {a});
}

Instruction *WasmHelpers::emitI64TruncSatF64S(Value *a) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64TruncSatF64S, {a});
}

Instruction *WasmHelpers::emitI64TruncSatF64U(Value *a) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64TruncSatF64U, {a});
}

// --- i64→float conversion helpers (G.4c) ---

Instruction *WasmHelpers::emitF64ConvertI64S(Value *lo, Value *hi) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmF64ConvertI64S, {lo, hi});
}

Instruction *WasmHelpers::emitF64ConvertI64U(Value *lo, Value *hi) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmF64ConvertI64U, {lo, hi});
}

Instruction *WasmHelpers::emitF32ConvertI64S(Value *lo, Value *hi) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmF32ConvertI64S, {lo, hi});
}

Instruction *WasmHelpers::emitF32ConvertI64U(Value *lo, Value *hi) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmF32ConvertI64U, {lo, hi});
}

Instruction *WasmHelpers::emitI64ReinterpretF64(Value *a) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64ReinterpretF64, {a});
}

Instruction *WasmHelpers::emitF64ReinterpretI64(Value *lo, Value *hi) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmF64ReinterpretI64, {lo, hi});
}

// --- Memory helpers (H.2) ---

Instruction *WasmHelpers::emitMemoryGrow(
    Value *heapu8, Value *delta, Value *maxPages) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmMemoryGrow, {heapu8, delta, maxPages});
}

Instruction *WasmHelpers::emitCallIndirect(
    Value *funcsArr,
    Value *typesArr,
    Value *index,
    Value *expectedTypeIdx) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmCallIndirect,
      {funcsArr, typesArr, index, expectedTypeIdx});
}

Instruction *WasmHelpers::emitCreateException(
    Value *tagIndex,
    llvh::ArrayRef<Value *> payloadValues) {
  // Build the args: [tagIndex, v0, v1, ...]
  llvh::SmallVector<Value *, 8> args;
  args.push_back(tagIndex);
  for (Value *v : payloadValues)
    args.push_back(v);
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmCreateException, args);
}

Instruction *WasmHelpers::emitMatchException(
    Value *caught,
    Value *tagIndex) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmMatchException, {caught, tagIndex});
}

Instruction *WasmHelpers::emitMemoryFill(
    Value *heapu8,
    Value *dest,
    Value *val,
    Value *size) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmMemoryFill,
      {heapu8, dest, val, size});
}

Instruction *WasmHelpers::emitMemoryCopy(
    Value *heapu8,
    Value *dest,
    Value *src,
    Value *size) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmMemoryCopy,
      {heapu8, dest, src, size});
}

Instruction *WasmHelpers::emitMemoryInit(
    Value *heapu8,
    Value *dataSegs,
    Value *segIdx,
    Value *dest,
    Value *src,
    Value *size) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmMemoryInit,
      {heapu8, dataSegs, segIdx, dest, src, size});
}

Instruction *WasmHelpers::emitDataDrop(Value *dataSegs, Value *segIdx) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmDataDrop, {dataSegs, segIdx});
}

Instruction *WasmHelpers::emitTableFill(
    Value *funcsArr,
    Value *idx,
    Value *val,
    Value *count) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmTableFill,
      {funcsArr, idx, val, count});
}

Instruction *WasmHelpers::emitTableCopy(
    Value *dstFuncs,
    Value *srcFuncs,
    Value *dstTypes,
    Value *srcTypes,
    Value *dst,
    Value *src,
    Value *count) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmTableCopy,
      {dstFuncs, srcFuncs, dstTypes, srcTypes, dst, src, count});
}

Instruction *WasmHelpers::emitTableInit(
    Value *funcsArr,
    Value *typesArr,
    Value *elemSegs,
    Value *segIdx,
    Value *dst,
    Value *src,
    Value *count) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmTableInit,
      {funcsArr, typesArr, elemSegs, segIdx, dst, src, count});
}

Instruction *WasmHelpers::emitElemDrop(Value *elemSegs, Value *segIdx) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmElemDrop, {elemSegs, segIdx});
}

// --- BigInt ↔ i64 conversion helpers ---

Instruction *WasmHelpers::emitBigIntToI64(Value *bigint) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmBigIntToI64, {bigint});
}

Instruction *WasmHelpers::emitI64ToBigInt(Value *lo, Value *hi) {
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64ToBigInt, {lo, hi});
}

} // namespace wasm
} // namespace hermes
