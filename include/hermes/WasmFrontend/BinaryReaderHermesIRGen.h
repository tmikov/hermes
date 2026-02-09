/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#ifndef HERMES_WASMFRONTEND_BINARYREADERHERMESIRGEN_H
#define HERMES_WASMFRONTEND_BINARYREADERHERMESIRGEN_H

#include "hermes/WasmFrontend/WasmModuleInfo.h"

#include "wabt/binary-reader-nop.h"

#include <string>

namespace hermes {
namespace wasm {

/// A wabt BinaryReaderDelegate that populates a WasmModuleInfo with
/// module-level data (types, imports, functions, tables, memories, globals,
/// exports, start function, element segments, data segments, and names).
///
/// Function body callbacks are left as no-ops for now — they will be wired
/// to WasmIRGen in a later step.
class BinaryReaderHermesIRGen : public wabt::BinaryReaderNop {
 public:
  explicit BinaryReaderHermesIRGen(WasmModuleInfo &moduleInfo);

  /// \return a reference to the populated module info.
  WasmModuleInfo &getModuleInfo() {
    return moduleInfo_;
  }

  // --- Type section ---
  wabt::Result OnTypeCount(wabt::Index count) override;
  wabt::Result OnFuncType(
      wabt::Index index,
      wabt::Index paramCount,
      wabt::Type *paramTypes,
      wabt::Index resultCount,
      wabt::Type *resultTypes) override;

  // --- Import section ---
  wabt::Result OnImportCount(wabt::Index count) override;
  wabt::Result OnImportFunc(
      wabt::Index importIndex,
      std::string_view moduleName,
      std::string_view fieldName,
      wabt::Index funcIndex,
      wabt::Index sigIndex) override;
  wabt::Result OnImportTable(
      wabt::Index importIndex,
      std::string_view moduleName,
      std::string_view fieldName,
      wabt::Index tableIndex,
      wabt::Type elemType,
      const wabt::Limits *elemLimits) override;
  wabt::Result OnImportMemory(
      wabt::Index importIndex,
      std::string_view moduleName,
      std::string_view fieldName,
      wabt::Index memoryIndex,
      const wabt::Limits *pageLimits,
      uint32_t pageSize) override;
  wabt::Result OnImportGlobal(
      wabt::Index importIndex,
      std::string_view moduleName,
      std::string_view fieldName,
      wabt::Index globalIndex,
      wabt::Type type,
      bool mutable_) override;

  // --- Function section ---
  wabt::Result OnFunctionCount(wabt::Index count) override;
  wabt::Result OnFunction(wabt::Index index, wabt::Index sigIndex) override;

  // --- Table section ---
  wabt::Result OnTableCount(wabt::Index count) override;
  wabt::Result OnTable(
      wabt::Index index,
      wabt::Type elemType,
      const wabt::Limits *elemLimits) override;

  // --- Memory section ---
  wabt::Result OnMemoryCount(wabt::Index count) override;
  wabt::Result OnMemory(
      wabt::Index index,
      const wabt::Limits *limits,
      uint32_t pageSize) override;

  // --- Global section ---
  wabt::Result OnGlobalCount(wabt::Index count) override;
  wabt::Result BeginGlobal(
      wabt::Index index,
      wabt::Type type,
      bool mutable_) override;
  wabt::Result BeginGlobalInitExpr(wabt::Index index) override;
  wabt::Result EndGlobalInitExpr(wabt::Index index) override;
  wabt::Result EndGlobal(wabt::Index index) override;

  // --- Export section ---
  wabt::Result OnExportCount(wabt::Index count) override;
  wabt::Result OnExport(
      wabt::Index index,
      wabt::ExternalKind kind,
      wabt::Index itemIndex,
      std::string_view name) override;

  // --- Start section ---
  wabt::Result OnStartFunction(wabt::Index funcIndex) override;

  // --- Element section ---
  wabt::Result OnElemSegmentCount(wabt::Index count) override;
  wabt::Result BeginElemSegment(
      wabt::Index index,
      wabt::Index tableIndex,
      uint8_t flags) override;
  wabt::Result BeginElemSegmentInitExpr(wabt::Index index) override;
  wabt::Result EndElemSegmentInitExpr(wabt::Index index) override;
  wabt::Result OnElemSegmentElemExprCount(
      wabt::Index index,
      wabt::Index count) override;
  wabt::Result BeginElemExpr(
      wabt::Index elemIndex,
      wabt::Index exprIndex) override;
  wabt::Result EndElemExpr(
      wabt::Index elemIndex,
      wabt::Index exprIndex) override;
  wabt::Result EndElemSegment(wabt::Index index) override;

  // --- Data section ---
  wabt::Result OnDataSegmentCount(wabt::Index count) override;
  wabt::Result BeginDataSegment(
      wabt::Index index,
      wabt::Index memoryIndex,
      uint8_t flags) override;
  wabt::Result BeginDataSegmentInitExpr(wabt::Index index) override;
  wabt::Result EndDataSegmentInitExpr(wabt::Index index) override;
  wabt::Result OnDataSegmentData(
      wabt::Index index,
      const void *data,
      wabt::Address size) override;
  wabt::Result EndDataSegment(wabt::Index index) override;

  // --- Names section ---
  wabt::Result OnModuleName(std::string_view name) override;
  wabt::Result OnFunctionNamesCount(wabt::Index numFunctions) override;
  wabt::Result OnFunctionName(
      wabt::Index functionIndex,
      std::string_view functionName) override;

  // --- Init expression callbacks (shared by globals, elems, data) ---
  wabt::Result OnI32ConstExpr(uint32_t value) override;
  wabt::Result OnI64ConstExpr(uint64_t value) override;
  wabt::Result OnF32ConstExpr(uint32_t valueBits) override;
  wabt::Result OnF64ConstExpr(uint64_t valueBits) override;
  wabt::Result OnGlobalGetExpr(wabt::Index globalIndex) override;
  wabt::Result OnRefNullExpr(wabt::Type type) override;
  wabt::Result OnRefFuncExpr(wabt::Index funcIndex) override;

 private:
  /// Convert a wabt Type to our WasmValType.
  static WasmValType convertType(wabt::Type type);
  /// Convert a wabt Limits to our WasmLimits.
  static WasmLimits convertLimits(const wabt::Limits *limits);
  /// Convert a wabt ExternalKind to our WasmExternalKind.
  static WasmExternalKind convertExternalKind(wabt::ExternalKind kind);

  WasmModuleInfo &moduleInfo_;

  /// Tracks which init expression context we are in.
  enum class InitExprContext {
    None,
    Global,
    ElemSegmentOffset,
    ElemExpr,
    DataSegment,
  };
  InitExprContext initExprContext_ = InitExprContext::None;

  /// Index of the current item whose init expr we are parsing.
  wabt::Index currentInitExprIndex_ = 0;
};

} // namespace wasm
} // namespace hermes

#endif // HERMES_WASMFRONTEND_BINARYREADERHERMESIRGEN_H
