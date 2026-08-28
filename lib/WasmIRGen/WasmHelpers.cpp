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
  // No return value (noreturn).
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmTrap, {});
}

Instruction *WasmHelpers::emitI32DivS(Value *a, Value *b) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI32DivS, {a, b});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitI32DivU(Value *a, Value *b) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI32DivU, {a, b});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitI32RemS(Value *a, Value *b) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI32RemS, {a, b});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitI32RemU(Value *a, Value *b) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI32RemU, {a, b});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitI32Clz(Value *a) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI32Clz, {a});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitI32Ctz(Value *a) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI32Ctz, {a});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitI32Popcnt(Value *a) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI32Popcnt, {a});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitI32Rotl(Value *a, Value *b) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI32Rotl, {a, b});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitI32Rotr(Value *a, Value *b) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI32Rotr, {a, b});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitI32TruncF64S(Value *a) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI32TruncF64S, {a});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitI32TruncF64U(Value *a) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI32TruncF64U, {a});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitI32TruncSatF64S(Value *a) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI32TruncSatF64S, {a});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitI32TruncSatF64U(Value *a) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI32TruncSatF64U, {a});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitI32ReinterpretF32(Value *a) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI32ReinterpretF32, {a});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitF32ReinterpretI32(Value *a) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmF32ReinterpretI32, {a});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitF64Copysign(Value *a, Value *b) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmF64Copysign, {a, b});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitF32Copysign(Value *a, Value *b) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmF32Copysign, {a, b});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitNearest(Value *a) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmNearest, {a});
  inst->setType(Type::createNumber());
  return inst;
}

// --- i64 helpers (G.3) ---
// i64 binary ops return lo32 (the hi32 is written to retBufI[1]).

Instruction *WasmHelpers::emitI64Add(
    Value *retBufI, Value *loA, Value *hiA, Value *loB, Value *hiB) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64Add,
      {retBufI, loA, hiA, loB, hiB});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitI64Sub(
    Value *retBufI, Value *loA, Value *hiA, Value *loB, Value *hiB) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64Sub,
      {retBufI, loA, hiA, loB, hiB});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitI64Mul(
    Value *retBufI, Value *loA, Value *hiA, Value *loB, Value *hiB) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64Mul,
      {retBufI, loA, hiA, loB, hiB});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitI64DivS(
    Value *retBufI, Value *loA, Value *hiA, Value *loB, Value *hiB) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64DivS,
      {retBufI, loA, hiA, loB, hiB});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitI64DivU(
    Value *retBufI, Value *loA, Value *hiA, Value *loB, Value *hiB) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64DivU,
      {retBufI, loA, hiA, loB, hiB});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitI64RemS(
    Value *retBufI, Value *loA, Value *hiA, Value *loB, Value *hiB) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64RemS,
      {retBufI, loA, hiA, loB, hiB});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitI64RemU(
    Value *retBufI, Value *loA, Value *hiA, Value *loB, Value *hiB) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64RemU,
      {retBufI, loA, hiA, loB, hiB});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitI64Shl(
    Value *retBufI, Value *loA, Value *hiA, Value *loB, Value *hiB) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64Shl,
      {retBufI, loA, hiA, loB, hiB});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitI64ShrS(
    Value *retBufI, Value *loA, Value *hiA, Value *loB, Value *hiB) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64ShrS,
      {retBufI, loA, hiA, loB, hiB});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitI64ShrU(
    Value *retBufI, Value *loA, Value *hiA, Value *loB, Value *hiB) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64ShrU,
      {retBufI, loA, hiA, loB, hiB});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitI64Rotl(
    Value *retBufI, Value *loA, Value *hiA, Value *loB, Value *hiB) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64Rotl,
      {retBufI, loA, hiA, loB, hiB});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitI64Rotr(
    Value *retBufI, Value *loA, Value *hiA, Value *loB, Value *hiB) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64Rotr,
      {retBufI, loA, hiA, loB, hiB});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitI64Clz(Value *lo, Value *hi) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64Clz, {lo, hi});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitI64Ctz(Value *lo, Value *hi) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64Ctz, {lo, hi});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitI64Popcnt(Value *lo, Value *hi) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64Popcnt, {lo, hi});
  inst->setType(Type::createNumber());
  return inst;
}

// --- i64 conversion helpers (G.4b) ---
// These write lo/hi to retBufI and return lo32.

Instruction *WasmHelpers::emitI64TruncF64S(Value *retBufI, Value *a) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64TruncF64S, {retBufI, a});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitI64TruncF64U(Value *retBufI, Value *a) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64TruncF64U, {retBufI, a});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitI64TruncSatF64S(Value *retBufI, Value *a) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64TruncSatF64S, {retBufI, a});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitI64TruncSatF64U(Value *retBufI, Value *a) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64TruncSatF64U, {retBufI, a});
  inst->setType(Type::createNumber());
  return inst;
}

// --- i64→float conversion helpers (G.4c) ---

Instruction *WasmHelpers::emitF64ConvertI64S(Value *lo, Value *hi) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmF64ConvertI64S, {lo, hi});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitF64ConvertI64U(Value *lo, Value *hi) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmF64ConvertI64U, {lo, hi});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitF32ConvertI64S(Value *lo, Value *hi) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmF32ConvertI64S, {lo, hi});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitF32ConvertI64U(Value *lo, Value *hi) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmF32ConvertI64U, {lo, hi});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitI64ReinterpretF64(Value *retBufI, Value *a) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64ReinterpretF64, {retBufI, a});
  inst->setType(Type::createNumber());
  return inst;
}

Instruction *WasmHelpers::emitF64ReinterpretI64(Value *lo, Value *hi) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmF64ReinterpretI64, {lo, hi});
  inst->setType(Type::createNumber());
  return inst;
}

// --- Memory helpers (H.2) ---

Instruction *WasmHelpers::emitMemoryGrow(
    Value *heapu8, Value *delta, Value *maxPages, Value *memObj) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmMemoryGrow,
      {heapu8, delta, maxPages, memObj});
  // The builtin returns the new ArrayBuffer on success, or -1 on failure.
  // The type annotation is a GC-safety contract, not a hint: the JIT gives
  // number-typed frame registers callee-saved machine registers that the GC
  // does not update, so annotating this Number let a collection during the
  // view re-creation after a successful grow move the ArrayBuffer and leave
  // the register holding it dangling.
  inst->setType(inst->getModule()->getTypeContext().unionTy(
      Type::createNumber(), Type::createObject()));
  return inst;
}

Instruction *WasmHelpers::emitCallIndirect(
    Value *funcsArr,
    Value *typesArr,
    Value *index,
    Value *expectedTypeIdx) {
  // Return type depends on callee; caller sets type.
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
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmCreateException, args);
  inst->setType(Type::createObject());
  return inst;
}

Instruction *WasmHelpers::emitMatchException(
    Value *caught,
    Value *tagIndex) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmMatchException, {caught, tagIndex});
  // Returns the caught array (object) if matched, or undefined if not.
  return inst;
}

Instruction *WasmHelpers::emitMemoryFill(
    Value *heapu8,
    Value *dest,
    Value *val,
    Value *size) {
  // No meaningful return value.
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmMemoryFill,
      {heapu8, dest, val, size});
}

Instruction *WasmHelpers::emitMemoryCopy(
    Value *heapu8,
    Value *dest,
    Value *src,
    Value *size) {
  // No meaningful return value.
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
  // No meaningful return value.
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmMemoryInit,
      {heapu8, dataSegs, segIdx, dest, src, size});
}

Instruction *WasmHelpers::emitDataDrop(Value *dataSegs, Value *segIdx) {
  // No meaningful return value.
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmDataDrop, {dataSegs, segIdx});
}

Instruction *WasmHelpers::emitTableFill(
    Value *funcsArr,
    Value *idx,
    Value *val,
    Value *count) {
  // No meaningful return value.
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
  // No meaningful return value.
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
  // No meaningful return value.
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmTableInit,
      {funcsArr, typesArr, elemSegs, segIdx, dst, src, count});
}

Instruction *WasmHelpers::emitElemDrop(Value *elemSegs, Value *segIdx) {
  // No meaningful return value.
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmElemDrop, {elemSegs, segIdx});
}

Instruction *WasmHelpers::emitTableGrow(
    Value *funcsArr,
    Value *typesArr,
    Value *delta,
    Value *fillVal,
    Value *maxEntries,
    Value *actualMax) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmTableGrow,
      {funcsArr, typesArr, delta, fillVal, maxEntries, actualMax});
  inst->setType(Type::createNumber());
  return inst;
}

// --- BigInt ↔ i64 conversion helpers ---

Instruction *WasmHelpers::emitBigIntToI64(Value *retBufI, Value *bigint) {
  // No meaningful return value (writes to retBufI).
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmBigIntToI64, {retBufI, bigint});
}

Instruction *WasmHelpers::emitI64ToBigInt(Value *lo, Value *hi) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmI64ToBigInt, {lo, hi});
  inst->setType(Type::createBigInt());
  return inst;
}

Instruction *WasmHelpers::emitLinkError(Value *message) {
  // No return value (noreturn).
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmLinkError, {message});
}

Instruction *WasmHelpers::emitDataSegmentInit(
    Value *heapu8,
    Value *blobOffset,
    Value *length,
    Value *dest) {
  // No meaningful return value.
  return builder_.createCallBuiltinInst(
      BuiltinMethod::HermesBuiltin_wasmDataSegmentInit,
      {heapu8, blobOffset, length, dest});
}

} // namespace wasm
} // namespace hermes
