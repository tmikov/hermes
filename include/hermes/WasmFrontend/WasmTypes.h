/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#ifndef HERMES_WASMFRONTEND_WASMTYPES_H
#define HERMES_WASMFRONTEND_WASMTYPES_H

#include <cstdint>
#include <string>
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

/// External kind for imports and exports.
enum class WasmExternalKind : uint8_t {
  Function = 0,
  Table = 1,
  Memory = 2,
  Global = 3,
};

/// A Wasm import entry.
struct WasmImport {
  std::string moduleName;
  std::string fieldName;
  WasmExternalKind kind = WasmExternalKind::Function;
  /// Index into types[] for function imports.
  uint32_t typeIndex = 0;
  /// For table imports.
  WasmTableType tableType;
  /// For memory imports.
  WasmMemoryType memoryType;
  /// For global imports.
  WasmGlobalType globalType;
};

/// A Wasm export entry.
struct WasmExport {
  std::string name;
  WasmExternalKind kind = WasmExternalKind::Function;
  /// Index into the respective index space.
  uint32_t index = 0;
};

/// A Wasm function declaration (body is translated directly to IR during
/// parsing, so it is not stored here).
struct WasmFunction {
  /// Index into types[].
  uint32_t typeIndex = 0;
};

/// A Wasm global variable.
struct WasmGlobal {
  WasmGlobalType type;

  /// Kind of the constant init expression.
  enum class InitKind : uint8_t {
    I32Const,
    I64Const,
    F32Const,
    F64Const,
    GlobalGet,
    RefNull,
    RefFunc,
  };
  InitKind initKind = InitKind::I32Const;

  /// Value of the constant init expression.
  union InitValue {
    int32_t i32Val;
    int64_t i64Val;
    float f32Val;
    double f64Val;
    uint32_t globalIndex;
    uint32_t funcIndex;

    InitValue() : i32Val(0) {}
  } initValue;
};

} // namespace wasm
} // namespace hermes

#endif // HERMES_WASMFRONTEND_WASMTYPES_H
