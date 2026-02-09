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

 private:
  IRBuilder &builder_;
};

} // namespace wasm
} // namespace hermes

#endif // HERMES_WASMIRGEN_WASMHELPERS_H
