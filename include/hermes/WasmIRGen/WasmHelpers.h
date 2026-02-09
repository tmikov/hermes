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

 private:
  IRBuilder &builder_;
};

} // namespace wasm
} // namespace hermes

#endif // HERMES_WASMIRGEN_WASMHELPERS_H
