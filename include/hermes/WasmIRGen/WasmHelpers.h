/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#ifndef HERMES_WASMIRGEN_WASMHELPERS_H
#define HERMES_WASMIRGEN_WASMHELPERS_H

#include "hermes/FrontEndDefs/Builtins.h"
#include "hermes/IR/IRBuilder.h"

namespace hermes {
namespace wasm {

/// Provides IR generation helpers for calling Wasm runtime helper builtins.
///
/// Each Wasm operation that has no direct JS/asm.js equivalent (e.g., trapping
/// division, bit manipulation, type conversions) is implemented as a private
/// builtin registered in Builtins.def. This class wraps the IRBuilder calls to
/// emit CallBuiltinInst for those helpers.
///
/// Usage:
///   WasmHelpers helpers(builder);
///   Value *result = helpers.emitTrap(builder);
class WasmHelpers {
 public:
  explicit WasmHelpers(IRBuilder &builder) : builder_(builder) {}

  /// Emit a call to the wasmTrap builtin, which throws an Error with the
  /// message "unreachable executed". Used for the Wasm `unreachable`
  /// instruction.
  Instruction *emitTrap();

  /// Emit i32 signed division with trapping on division by zero or overflow.
  /// \return the CallBuiltinInst for the result.
  Instruction *emitI32DivS(Value *a, Value *b);

  /// Emit i32 unsigned division with trapping on division by zero.
  Instruction *emitI32DivU(Value *a, Value *b);

  /// Emit i32 signed remainder with trapping on division by zero.
  Instruction *emitI32RemS(Value *a, Value *b);

  /// Emit i32 unsigned remainder with trapping on division by zero.
  Instruction *emitI32RemU(Value *a, Value *b);

  /// Emit i32 count leading zeros.
  Instruction *emitI32Clz(Value *a);

  /// Emit i32 count trailing zeros.
  Instruction *emitI32Ctz(Value *a);

  /// Emit i32 population count (number of set bits).
  Instruction *emitI32Popcnt(Value *a);

  /// Emit i32 rotate left.
  Instruction *emitI32Rotl(Value *a, Value *b);

  /// Emit i32 rotate right.
  Instruction *emitI32Rotr(Value *a, Value *b);

  /// Emit i32.trunc_f64_s (also used for i32.trunc_f32_s):
  /// trapping truncation from float/double to signed i32.
  Instruction *emitI32TruncF64S(Value *a);

  /// Emit i32.trunc_f64_u (also used for i32.trunc_f32_u):
  /// trapping truncation from float/double to unsigned i32.
  Instruction *emitI32TruncF64U(Value *a);

  /// Emit i32.trunc_sat_f64_s (also used for i32.trunc_sat_f32_s):
  /// saturating truncation from float/double to signed i32.
  Instruction *emitI32TruncSatF64S(Value *a);

  /// Emit i32.trunc_sat_f64_u (also used for i32.trunc_sat_f32_u):
  /// saturating truncation from float/double to unsigned i32.
  Instruction *emitI32TruncSatF64U(Value *a);

  /// Emit i32.reinterpret_f32: bitcast f32 to i32.
  Instruction *emitI32ReinterpretF32(Value *a);

  /// Emit f32.reinterpret_i32: bitcast i32 to f32.
  Instruction *emitF32ReinterpretI32(Value *a);

  /// Emit f64.copysign(a, b): copy the sign bit of b onto the magnitude of a.
  Instruction *emitF64Copysign(Value *a, Value *b);

  /// Emit f32.copysign(a, b): copy the sign bit of b onto the magnitude of a.
  Instruction *emitF32Copysign(Value *a, Value *b);

  /// Emit f64.nearest / f32.nearest: IEEE 754 round-ties-to-even.
  Instruction *emitNearest(Value *a);

  // --- i64 helpers (G.3) ---
  // Binary ops take retBufI as first arg, write lo/hi to retBufI[0]/[1].

  /// i64 binary arithmetic ops. Take (retBufI, lo_a, hi_a, lo_b, hi_b).
  Instruction *emitI64Add(
      Value *retBufI, Value *loA, Value *hiA, Value *loB, Value *hiB);
  Instruction *emitI64Sub(
      Value *retBufI, Value *loA, Value *hiA, Value *loB, Value *hiB);
  Instruction *emitI64Mul(
      Value *retBufI, Value *loA, Value *hiA, Value *loB, Value *hiB);
  Instruction *emitI64DivS(
      Value *retBufI, Value *loA, Value *hiA, Value *loB, Value *hiB);
  Instruction *emitI64DivU(
      Value *retBufI, Value *loA, Value *hiA, Value *loB, Value *hiB);
  Instruction *emitI64RemS(
      Value *retBufI, Value *loA, Value *hiA, Value *loB, Value *hiB);
  Instruction *emitI64RemU(
      Value *retBufI, Value *loA, Value *hiA, Value *loB, Value *hiB);
  Instruction *emitI64Shl(
      Value *retBufI, Value *loA, Value *hiA, Value *loB, Value *hiB);
  Instruction *emitI64ShrS(
      Value *retBufI, Value *loA, Value *hiA, Value *loB, Value *hiB);
  Instruction *emitI64ShrU(
      Value *retBufI, Value *loA, Value *hiA, Value *loB, Value *hiB);
  Instruction *emitI64Rotl(
      Value *retBufI, Value *loA, Value *hiA, Value *loB, Value *hiB);
  Instruction *emitI64Rotr(
      Value *retBufI, Value *loA, Value *hiA, Value *loB, Value *hiB);

  /// i64 unary ops. Take (lo, hi), return a single i32 result.
  Instruction *emitI64Clz(Value *lo, Value *hi);
  Instruction *emitI64Ctz(Value *lo, Value *hi);
  Instruction *emitI64Popcnt(Value *lo, Value *hi);

  // --- i64 conversion helpers (G.4b) ---
  // Take (retBufI, f64_arg), write lo/hi result to retBufI[0]/[1].

  /// i64.trunc_f64_s (also used for i64.trunc_f32_s):
  /// trapping truncation from double to signed i64.
  Instruction *emitI64TruncF64S(Value *retBufI, Value *a);

  /// i64.trunc_f64_u (also used for i64.trunc_f32_u):
  /// trapping truncation from double to unsigned i64.
  Instruction *emitI64TruncF64U(Value *retBufI, Value *a);

  /// i64.trunc_sat_f64_s (also used for i64.trunc_sat_f32_s):
  /// saturating truncation from double to signed i64.
  Instruction *emitI64TruncSatF64S(Value *retBufI, Value *a);

  /// i64.trunc_sat_f64_u (also used for i64.trunc_sat_f32_u):
  /// saturating truncation from double to unsigned i64.
  Instruction *emitI64TruncSatF64U(Value *retBufI, Value *a);

  // --- i64→float conversion helpers (G.4c) ---
  // Take split lo/hi i64 args, return a single f64.

  /// f64.convert_i64_s: convert signed i64 to f64.
  Instruction *emitF64ConvertI64S(Value *lo, Value *hi);

  /// f64.convert_i64_u: convert unsigned i64 to f64.
  Instruction *emitF64ConvertI64U(Value *lo, Value *hi);

  /// f32.convert_i64_s: convert signed i64 to f32 (as double).
  Instruction *emitF32ConvertI64S(Value *lo, Value *hi);

  /// f32.convert_i64_u: convert unsigned i64 to f32 (as double).
  Instruction *emitF32ConvertI64U(Value *lo, Value *hi);

  /// i64.reinterpret_f64: bitcast f64 to i64. Writes lo/hi to retBufI.
  Instruction *emitI64ReinterpretF64(Value *retBufI, Value *a);

  /// f64.reinterpret_i64: bitcast i64 to f64. Takes split lo/hi.
  Instruction *emitF64ReinterpretI64(Value *lo, Value *hi);

  // --- Memory helpers (H.2) ---

  /// Emit memory.grow: takes (heapu8View, delta, maxPages).
  /// Returns new ArrayBuffer on success, or -1 on failure.
  /// \p memObj is the WebAssembly.Memory backing the linear memory, or
  /// undefined for an imported memory that has none. When present the
  /// builtin installs the grown buffer on it, so exported references to the
  /// memory observe the growth.
  Instruction *emitMemoryGrow(
      Value *heapu8,
      Value *delta,
      Value *maxPages,
      Value *memObj);

  // --- Table helpers (J.2) ---

  /// Emit call_indirect validation: takes (funcsArr, typesArr, index,
  /// expectedTypeIdx). Returns the validated closure on success, traps on
  /// failure (out of bounds, null entry, or type mismatch).
  Instruction *emitCallIndirect(
      Value *funcsArr,
      Value *typesArr,
      Value *index,
      Value *expectedTypeIdx);

  // --- Exception handling helpers (L.1) ---

  /// Emit wasmCreateException(tagIndex, v0, v1, ...): creates a Wasm exception
  /// object (JSArray) with the given tag index and payload values.
  Instruction *emitCreateException(
      Value *tagIndex,
      llvh::ArrayRef<Value *> payloadValues);

  /// Emit wasmMatchException(caught, tagIndex): if the caught value is a Wasm
  /// exception with matching tag, returns the exception array; otherwise
  /// returns undefined.
  Instruction *emitMatchException(Value *caught, Value *tagIndex);

  // --- Bulk memory helpers (N.1) ---

  /// Emit memory.fill: fills \p size bytes at \p dest with \p value.
  /// \p heapu8 is the Uint8Array view of linear memory.
  Instruction *emitMemoryFill(Value *heapu8, Value *dest, Value *val,
                              Value *size);

  /// Emit memory.copy: copies \p size bytes from \p src to \p dest.
  /// \p heapu8 is the Uint8Array view of linear memory.
  Instruction *emitMemoryCopy(Value *heapu8, Value *dest, Value *src,
                              Value *size);

  /// Emit memory.init: copies \p size bytes from data segment at offset \p src
  /// to \p dest in linear memory.
  /// \p heapu8 is the Uint8Array view. \p dataSegs is the data segments array.
  /// \p segIdx is a LiteralNumber for the segment index.
  Instruction *emitMemoryInit(Value *heapu8, Value *dataSegs, Value *segIdx,
                              Value *dest, Value *src, Value *size);

  /// Emit data.drop: marks data segment \p segIdx as dropped.
  /// \p dataSegs is the data segments array.
  Instruction *emitDataDrop(Value *dataSegs, Value *segIdx);

  // --- Table slot accesses ---

  /// Emit a read of one table slot's Exported Function (or null). A builtin
  /// rather than a LoadPropertyInst: the array can come from a table import,
  /// and an accessor installed at an index would run user JS inside a Wasm
  /// function body.
  Instruction *emitTableGetSlot(Value *exportedArr, Value *idx);

  /// Emit a write of one table slot. This is the ONLY way generated code
  /// writes a table: the closure and the interned type id are derived from
  /// \p val, the Exported Function (or null), so the three parallel arrays
  /// cannot drift apart. \p val must be null or an Exported Function; anything
  /// else raises a TypeError at runtime.
  /// \p isFuncRef is a literal 1 for a funcref table and 0 for an externref
  /// one, whose slots hold arbitrary JS values and no wrapper at all.
  Instruction *emitTableSetSlot(
      Value *funcsArr,
      Value *typesArr,
      Value *exportedArr,
      Value *idx,
      Value *val,
      Value *isFuncRef);

  /// Emit table.copy: copies \p count slots from src to dst table. All three
  /// arrays of each table are copied together.
  Instruction *emitTableCopySlots(
      Value *dstFuncs,
      Value *dstTypes,
      Value *dstExported,
      Value *srcFuncs,
      Value *srcTypes,
      Value *srcExported,
      Value *dst,
      Value *src,
      Value *count);

  // --- Bulk table helpers (N.2) ---

  /// Emit table.fill: fills \p count entries at \p idx with \p val.
  /// The three arguments are the table's three parallel arrays.
  Instruction *emitTableFill(
      Value *funcsArr,
      Value *typesArr,
      Value *exportedArr,
      Value *idx,
      Value *val,
      Value *count,
      Value *isFuncRef);

  /// Emit table.init: copies \p count entries from element segment to table.
  /// The first three arguments are the table's three parallel arrays.
  /// \p elemSegs is the element segments array, \p segIdx is the segment index.
  Instruction *emitTableInit(
      Value *funcsArr,
      Value *typesArr,
      Value *exportedArr,
      Value *elemSegs,
      Value *segIdx,
      Value *dst,
      Value *src,
      Value *count);

  /// Emit elem.drop: marks element segment \p segIdx as dropped.
  /// \p elemSegs is the element segments array.
  Instruction *emitElemDrop(Value *elemSegs, Value *segIdx);

  /// Emit table.grow: grows all three of the table's arrays by \p delta
  /// entries, filling the new slots with \p fillVal.
  /// \p maxEntries is the table's declared maximum size.
  /// Returns old size on success, -1 on failure.
  Instruction *emitTableGrow(
      Value *funcsArr,
      Value *typesArr,
      Value *exportedArr,
      Value *delta,
      Value *fillVal,
      Value *maxEntries,
      Value *actualMax,
      Value *isFuncRef);

  // --- BigInt ↔ i64 conversion helpers ---

  /// Convert a JS BigInt to split (lo, hi). Writes lo/hi to retBufI[0]/[1].
  Instruction *emitBigIntToI64(Value *retBufI, Value *bigint);

  /// Convert split (lo, hi) to a JS BigInt.
  Instruction *emitI64ToBigInt(Value *lo, Value *hi);

  /// Emit a call to the wasmLinkError builtin, which creates and throws a
  /// WebAssembly.LinkError with the given message string. Used for import
  /// type validation at instantiation time.
  Instruction *emitLinkError(Value *message);

  /// Emit wasmLinkTable: brand-check \p importVal as a genuine
  /// WebAssembly.Table and yield its three backing arrays and its own maximum
  /// as [funcs, types, exported, max], or null if it is not one (or if
  /// \p declaredIsFuncRef is false, which no table can satisfy).
  /// The arrays are the table's internal fields themselves, so two modules
  /// importing one table write the same slots.
  Instruction *emitLinkTable(Value *importVal, Value *declaredIsFuncRef);

  /// Emit wasmLinkMemory: brand-check \p importVal as a genuine
  /// WebAssembly.Memory and yield [currentPages, max, buffer], or null if it
  /// is not one. The page count is read from the buffer at the moment of the
  /// call, so it follows every grow, and the buffer comes back with it so the
  /// module's views are built over the very buffer that was measured.
  Instruction *emitLinkMemory(Value *importVal);

  /// Emit wasmLinkGlobal: brand-check \p importVal as a genuine
  /// WebAssembly.Global of the declared type and mutability, and yield its
  /// value. Yields undefined if it is a Global that does not match, and null
  /// if it is not a Global at all -- the caller needs the two apart, because
  /// only the second can legitimately be a raw JS value.
  Instruction *emitLinkGlobal(
      Value *importVal,
      Value *expectedValType,
      Value *expectedMutable);

  /// Emit wasmGlobalGet / wasmGlobalSet: read or write the shared value of an
  /// imported MUTABLE global, \p globalObj, in its internal field. That is
  /// what `.value` used to be used for, and `value` is a CONFIGURABLE accessor
  /// on WebAssembly.Global.prototype: replacing it let script decide what
  /// every global.get returned and discard every global.set the module made.
  /// The object is kept rather than snapshotted because a mutable import is
  /// genuinely shared state -- snapshotting it is H12.
  Instruction *emitGlobalGet(Value *globalObj);
  Instruction *emitGlobalSet(Value *globalObj, Value *value);

  /// Emit wasmDataSegmentInit: bulk-copy from binary data storage blob
  /// into linear memory. Args: (heapu8, blobOffset, length, dest).
  Instruction *emitDataSegmentInit(
      Value *heapu8,
      Value *blobOffset,
      Value *length,
      Value *dest);

 private:
  IRBuilder &builder_;
};

} // namespace wasm
} // namespace hermes

#endif // HERMES_WASMIRGEN_WASMHELPERS_H
