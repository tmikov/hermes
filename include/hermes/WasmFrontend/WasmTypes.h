/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#ifndef HERMES_WASMFRONTEND_WASMTYPES_H
#define HERMES_WASMFRONTEND_WASMTYPES_H

#include <cstdint>
#include <vector>

namespace hermes {
namespace wasm {

/// Wasm value types, using the same encoding as the binary format.
enum class WasmValType : uint8_t {
  I32 = 0x7F,
  I64 = 0x7E,
  F32 = 0x7D,
  F64 = 0x7C,
  V128 = 0x7B,
  FuncRef = 0x70,
  ExternRef = 0x6F,
};

/// A Wasm function type (signature).
struct WasmFuncType {
  std::vector<WasmValType> params;
  std::vector<WasmValType> results;
};

/// Limits for tables and memories.
struct WasmLimits {
  uint32_t initial = 0;
  /// UINT32_MAX means no maximum.
  uint32_t maximum = UINT32_MAX;
  bool hasMaximum = false;
};

/// Type of a Wasm table.
struct WasmTableType {
  /// Element type (funcref or externref).
  WasmValType elemType = WasmValType::FuncRef;
  WasmLimits limits;
};

/// Type of a Wasm linear memory.
struct WasmMemoryType {
  WasmLimits limits;
};

/// Type of a Wasm global variable.
struct WasmGlobalType {
  WasmValType type = WasmValType::I32;
  bool mutable_ = false;
};

} // namespace wasm
} // namespace hermes

#endif // HERMES_WASMFRONTEND_WASMTYPES_H
