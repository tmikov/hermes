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

/// A Wasm exception tag.
struct WasmTag {
  /// Index into the module's types[] vector for the tag's signature.
  uint32_t typeIndex = 0;
};

/// External kind for imports and exports.
enum class WasmExternalKind : uint8_t {
  Function = 0,
  Table = 1,
  Memory = 2,
  Global = 3,
  Tag = 4,
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
  /// For tag imports: index into types[].
  uint32_t tagTypeIndex = 0;
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

/// A single operation in a Wasm init expression (small stack machine).
/// Used to represent extended constant expressions like
/// (i32.add (i32.const 1) (i32.const 2)).
struct InitExprOp {
  enum class Kind : uint8_t {
    I32Const,
    GlobalGet,
    I32Add,
    I32Sub,
    I32Mul,
  };
  Kind kind;
  union {
    int32_t i32Val;
    uint32_t globalIdx;
  };

  static InitExprOp makeI32Const(int32_t v) {
    InitExprOp op;
    op.kind = Kind::I32Const;
    op.i32Val = v;
    return op;
  }
  static InitExprOp makeGlobalGet(uint32_t idx) {
    InitExprOp op;
    op.kind = Kind::GlobalGet;
    op.globalIdx = idx;
    return op;
  }
  static InitExprOp makeAdd() {
    return {Kind::I32Add, {}};
  }
  static InitExprOp makeSub() {
    return {Kind::I32Sub, {}};
  }
  static InitExprOp makeMul() {
    return {Kind::I32Mul, {}};
  }
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

  /// Init expression as a sequence of stack-machine operations, for extended
  /// constant expressions such as (i32.add (i32.const 1) (i32.const 2)).
  /// When size > 1 this replaces initKind/initValue. Only i32 globals can
  /// currently produce one.
  std::vector<InitExprOp> initExpr;
};

/// A Wasm element segment (populates a table).
struct WasmElemSegment {
  enum class Mode : uint8_t { Active, Passive, Declarative };
  Mode mode = Mode::Active;
  /// Table index for active segments.
  uint32_t tableIndex = 0;
  /// Kind of the offset init expression (for active segments).
  WasmGlobal::InitKind offsetKind = WasmGlobal::InitKind::I32Const;
  /// i32.const value for the offset (common case).
  int32_t offsetValue = 0;
  /// Global index for global.get offset expression.
  uint32_t offsetGlobalIdx = 0;
  /// Offset init expression as a sequence of stack-machine operations.
  /// When size > 1, this replaces offsetKind/offsetValue/offsetGlobalIdx.
  std::vector<InitExprOp> offsetExpr;
  /// Element values (function indices).
  std::vector<uint32_t> funcIndices;
};

/// A Wasm data segment (initializes linear memory).
struct WasmDataSegment {
  enum class Mode : uint8_t { Active, Passive };
  Mode mode = Mode::Active;
  /// Memory index for active segments (always 0 in MVP).
  uint32_t memoryIndex = 0;
  /// Kind of the offset init expression (for active segments).
  WasmGlobal::InitKind offsetKind = WasmGlobal::InitKind::I32Const;
  /// i32.const value for the offset.
  int32_t offsetValue = 0;
  /// Global index for global.get offset expression.
  uint32_t offsetGlobalIdx = 0;
  /// Offset init expression as a sequence of stack-machine operations.
  /// When size > 1, this replaces offsetKind/offsetValue/offsetGlobalIdx.
  std::vector<InitExprOp> offsetExpr;
  /// Raw data bytes.
  std::vector<uint8_t> data;
};

/// Parsed name section (custom section "name").
struct WasmNameSection {
  std::string moduleName;
  /// Function names indexed by function index.
  std::vector<std::string> functionNames;
  // Local names omitted for now.
};

} // namespace wasm
} // namespace hermes

#endif // HERMES_WASMFRONTEND_WASMTYPES_H
