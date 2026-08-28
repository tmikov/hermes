/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#ifndef HERMES_WASMFRONTEND_BINARYREADERHERMESIRGEN_H
#define HERMES_WASMFRONTEND_BINARYREADERHERMESIRGEN_H

#include "hermes/WasmFrontend/WasmModuleInfo.h"

// wabt headers use #if on macros that may not be defined, triggering -Wundef.
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wundef"
#include "wabt/binary-reader-nop.h"
#pragma GCC diagnostic pop

#include <string>
#include <vector>

namespace hermes {
namespace wasm {

class WasmIRGen;

/// A wabt BinaryReaderDelegate that populates a WasmModuleInfo with
/// module-level data (types, imports, functions, tables, memories, globals,
/// exports, start function, element segments, data segments, and names).
///
/// When a WasmIRGen is attached via setIRGen(), function body callbacks
/// are dispatched to it for IR generation. Otherwise they are no-ops.
class BinaryReaderHermesIRGen : public wabt::BinaryReaderNop {
 public:
  explicit BinaryReaderHermesIRGen(WasmModuleInfo &moduleInfo);

  /// Attach a WasmIRGen instance for function body translation.
  /// Must be called after module-level parsing is complete (i.e., after
  /// a first pass that populates WasmModuleInfo) or before a single-pass
  /// parse that handles both module-level and function body data.
  void setIRGen(WasmIRGen *irgen) {
    irgen_ = irgen;
  }

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
  wabt::Result OnImportTag(
      wabt::Index importIndex,
      std::string_view moduleName,
      std::string_view fieldName,
      wabt::Index tagIndex,
      wabt::Index sigIndex) override;

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

  // --- Tag section ---
  wabt::Result OnTagCount(wabt::Index count) override;
  wabt::Result OnTagType(wabt::Index index, wabt::Index sigIndex) override;

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

  // --- Code section / Function body callbacks ---
  wabt::Result BeginCodeSection(wabt::Offset size) override;
  wabt::Result BeginFunctionBody(
      wabt::Index index,
      wabt::Offset size) override;
  wabt::Result OnLocalDeclCount(wabt::Index count) override;
  wabt::Result OnLocalDecl(
      wabt::Index declIndex,
      wabt::Index count,
      wabt::Type type) override;
  wabt::Result EndLocalDecls() override;
  wabt::Result EndFunctionBody(wabt::Index index) override;

  // --- Instruction callbacks (function body + init expressions) ---
  wabt::Result OnI32ConstExpr(uint32_t value) override;
  wabt::Result OnI64ConstExpr(uint64_t value) override;
  wabt::Result OnF32ConstExpr(uint32_t valueBits) override;
  wabt::Result OnF64ConstExpr(uint64_t valueBits) override;
  wabt::Result OnGlobalGetExpr(wabt::Index globalIndex) override;
  wabt::Result OnRefNullExpr(wabt::Type type) override;
  wabt::Result OnRefFuncExpr(wabt::Index funcIndex) override;
  wabt::Result OnRefIsNullExpr() override;
  wabt::Result OnLocalGetExpr(wabt::Index localIndex) override;
  wabt::Result OnLocalSetExpr(wabt::Index localIndex) override;
  wabt::Result OnLocalTeeExpr(wabt::Index localIndex) override;
  wabt::Result OnBinaryExpr(wabt::Opcode opcode) override;
  wabt::Result OnCompareExpr(wabt::Opcode opcode) override;
  wabt::Result OnConvertExpr(wabt::Opcode opcode) override;
  wabt::Result OnReturnExpr() override;
  wabt::Result OnDropExpr() override;
  wabt::Result OnBlockExpr(wabt::Type sigType) override;
  wabt::Result OnLoopExpr(wabt::Type sigType) override;
  wabt::Result OnIfExpr(wabt::Type sigType) override;
  wabt::Result OnElseExpr() override;
  wabt::Result OnEndExpr() override;
  wabt::Result OnBrExpr(wabt::Index depth) override;
  wabt::Result OnBrIfExpr(wabt::Index depth) override;
  wabt::Result OnBrTableExpr(
      wabt::Index numTargets,
      wabt::Index *targetDepths,
      wabt::Index defaultTargetDepth) override;
  wabt::Result OnSelectExpr(
      wabt::Index resultCount,
      wabt::Type *resultTypes) override;
  wabt::Result OnCallExpr(wabt::Index funcIndex) override;
  wabt::Result OnCallIndirectExpr(
      wabt::Index sigIndex,
      wabt::Index tableIndex) override;
  wabt::Result OnUnreachableExpr() override;
  wabt::Result OnNopExpr() override;
  wabt::Result OnGlobalSetExpr(wabt::Index globalIndex) override;
  wabt::Result OnLoadExpr(
      wabt::Opcode opcode,
      wabt::Index memidx,
      wabt::Address alignmentLog2,
      wabt::Address offset) override;
  wabt::Result OnStoreExpr(
      wabt::Opcode opcode,
      wabt::Index memidx,
      wabt::Address alignmentLog2,
      wabt::Address offset) override;
  wabt::Result OnMemorySizeExpr(wabt::Index memidx) override;
  wabt::Result OnMemoryGrowExpr(wabt::Index memidx) override;
  wabt::Result OnUnaryExpr(wabt::Opcode opcode) override;
  wabt::Result OnTableGetExpr(wabt::Index tableIndex) override;
  wabt::Result OnTableSetExpr(wabt::Index tableIndex) override;
  wabt::Result OnTableSizeExpr(wabt::Index tableIndex) override;
  wabt::Result OnTableGrowExpr(wabt::Index tableIndex) override;

  // --- Bulk memory operations ---
  wabt::Result OnMemoryFillExpr(wabt::Index memidx) override;
  wabt::Result OnMemoryCopyExpr(
      wabt::Index destmemidx,
      wabt::Index srcmemidx) override;
  wabt::Result OnMemoryInitExpr(
      wabt::Index segment_index,
      wabt::Index memidx) override;
  wabt::Result OnDataDropExpr(wabt::Index segment_index) override;

  // --- Bulk table operations ---
  wabt::Result OnTableFillExpr(wabt::Index table_index) override;
  wabt::Result OnTableCopyExpr(
      wabt::Index dst_index,
      wabt::Index src_index) override;
  wabt::Result OnTableInitExpr(
      wabt::Index segment_index,
      wabt::Index table_index) override;
  wabt::Result OnElemDropExpr(wabt::Index segment_index) override;

  // --- Module end ---
  wabt::Result EndModule() override;

  // --- Exception handling ---
  wabt::Result OnTryExpr(wabt::Type sigType) override;
  wabt::Result OnCatchExpr(wabt::Index tagIndex) override;
  wabt::Result OnCatchAllExpr() override;
  wabt::Result OnThrowExpr(wabt::Index tagIndex) override;
  wabt::Result OnRethrowExpr(wabt::Index depth) override;
  wabt::Result OnDelegateExpr(wabt::Index depth) override;

 private:
  /// Convert a wabt Type to our WasmValType.
  static WasmValType convertType(wabt::Type type);
  /// Convert a block/loop/if/try signature type to a WasmFuncType with
  /// both params and results. Handles simple types and type index refs.
  WasmFuncType convertBlockSigType(wabt::Type sigType);
  /// Convert a wabt Limits to our WasmLimits.
  static WasmLimits convertLimits(const wabt::Limits *limits);
  /// Convert a wabt ExternalKind to our WasmExternalKind.
  static WasmExternalKind convertExternalKind(wabt::ExternalKind kind);

  WasmModuleInfo &moduleInfo_;

  /// Optional WasmIRGen for function body translation.
  WasmIRGen *irgen_ = nullptr;

  /// Whether createFunctions() has been called on the IRGen.
  bool functionsCreated_ = false;

  /// Whether we are currently inside a function body.
  bool inFunctionBody_ = false;

  /// The Wasm function index of the current function body being parsed.
  /// This is the index in the Wasm function index space (imports + defined).
  uint32_t currentBodyFuncIndex_ = 0;

  /// Local types accumulated during OnLocalDecl callbacks for the current
  /// function body.
  std::vector<WasmValType> currentLocalTypes_;

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
