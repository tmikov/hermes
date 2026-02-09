/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/WasmFrontend/BinaryReaderHermesIRGen.h"

#include "hermes/WasmIRGen/WasmIRGen.h"

#include <cassert>
#include <cstring>

// wabt segment flags from wabt/common.h.
namespace {
constexpr uint8_t SegPassive = 1;
constexpr uint8_t SegDeclared = 3;
} // namespace

namespace hermes {
namespace wasm {

BinaryReaderHermesIRGen::BinaryReaderHermesIRGen(WasmModuleInfo &moduleInfo)
    : moduleInfo_(moduleInfo) {}

// --- Type conversions ---

WasmValType BinaryReaderHermesIRGen::convertType(wabt::Type type) {
  switch (static_cast<wabt::Type::Enum>(type)) {
    case wabt::Type::I32:
      return WasmValType::I32;
    case wabt::Type::I64:
      return WasmValType::I64;
    case wabt::Type::F32:
      return WasmValType::F32;
    case wabt::Type::F64:
      return WasmValType::F64;
    case wabt::Type::V128:
      return WasmValType::V128;
    case wabt::Type::FuncRef:
      return WasmValType::FuncRef;
    case wabt::Type::ExternRef:
      return WasmValType::ExternRef;
    default:
      assert(false && "unsupported wabt type");
      return WasmValType::I32;
  }
}

WasmLimits BinaryReaderHermesIRGen::convertLimits(
    const wabt::Limits *limits) {
  WasmLimits result;
  result.initial = static_cast<uint32_t>(limits->initial);
  result.hasMaximum = limits->has_max;
  if (limits->has_max) {
    result.maximum = static_cast<uint32_t>(limits->max);
  }
  return result;
}

WasmExternalKind BinaryReaderHermesIRGen::convertExternalKind(
    wabt::ExternalKind kind) {
  switch (kind) {
    case wabt::ExternalKind::Func:
      return WasmExternalKind::Function;
    case wabt::ExternalKind::Table:
      return WasmExternalKind::Table;
    case wabt::ExternalKind::Memory:
      return WasmExternalKind::Memory;
    case wabt::ExternalKind::Global:
      return WasmExternalKind::Global;
    default:
      assert(false && "unsupported external kind");
      return WasmExternalKind::Function;
  }
}

// --- Type section ---

wabt::Result BinaryReaderHermesIRGen::OnTypeCount(wabt::Index count) {
  moduleInfo_.types.reserve(count);
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnFuncType(
    wabt::Index index,
    wabt::Index paramCount,
    wabt::Type *paramTypes,
    wabt::Index resultCount,
    wabt::Type *resultTypes) {
  WasmFuncType ft;
  ft.params.reserve(paramCount);
  for (wabt::Index i = 0; i < paramCount; ++i) {
    ft.params.push_back(convertType(paramTypes[i]));
  }
  ft.results.reserve(resultCount);
  for (wabt::Index i = 0; i < resultCount; ++i) {
    ft.results.push_back(convertType(resultTypes[i]));
  }
  moduleInfo_.types.push_back(std::move(ft));
  return wabt::Result::Ok;
}

// --- Import section ---

wabt::Result BinaryReaderHermesIRGen::OnImportCount(wabt::Index count) {
  moduleInfo_.imports.reserve(count);
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnImportFunc(
    wabt::Index importIndex,
    std::string_view moduleName,
    std::string_view fieldName,
    wabt::Index funcIndex,
    wabt::Index sigIndex) {
  WasmImport imp;
  imp.moduleName = std::string(moduleName);
  imp.fieldName = std::string(fieldName);
  imp.kind = WasmExternalKind::Function;
  imp.typeIndex = sigIndex;
  moduleInfo_.imports.push_back(std::move(imp));
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnImportTable(
    wabt::Index importIndex,
    std::string_view moduleName,
    std::string_view fieldName,
    wabt::Index tableIndex,
    wabt::Type elemType,
    const wabt::Limits *elemLimits) {
  WasmImport imp;
  imp.moduleName = std::string(moduleName);
  imp.fieldName = std::string(fieldName);
  imp.kind = WasmExternalKind::Table;
  imp.tableType.elemType = convertType(elemType);
  imp.tableType.limits = convertLimits(elemLimits);
  moduleInfo_.imports.push_back(std::move(imp));
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnImportMemory(
    wabt::Index importIndex,
    std::string_view moduleName,
    std::string_view fieldName,
    wabt::Index memoryIndex,
    const wabt::Limits *pageLimits,
    uint32_t pageSize) {
  WasmImport imp;
  imp.moduleName = std::string(moduleName);
  imp.fieldName = std::string(fieldName);
  imp.kind = WasmExternalKind::Memory;
  imp.memoryType.limits = convertLimits(pageLimits);
  moduleInfo_.imports.push_back(std::move(imp));
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnImportGlobal(
    wabt::Index importIndex,
    std::string_view moduleName,
    std::string_view fieldName,
    wabt::Index globalIndex,
    wabt::Type type,
    bool mutable_) {
  WasmImport imp;
  imp.moduleName = std::string(moduleName);
  imp.fieldName = std::string(fieldName);
  imp.kind = WasmExternalKind::Global;
  imp.globalType.type = convertType(type);
  imp.globalType.mutable_ = mutable_;
  moduleInfo_.imports.push_back(std::move(imp));
  return wabt::Result::Ok;
}

// --- Function section ---

wabt::Result BinaryReaderHermesIRGen::OnFunctionCount(wabt::Index count) {
  moduleInfo_.functions.reserve(count);
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnFunction(
    wabt::Index index,
    wabt::Index sigIndex) {
  WasmFunction fn;
  fn.typeIndex = sigIndex;
  moduleInfo_.functions.push_back(std::move(fn));
  return wabt::Result::Ok;
}

// --- Table section ---

wabt::Result BinaryReaderHermesIRGen::OnTableCount(wabt::Index count) {
  moduleInfo_.tables.reserve(count);
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnTable(
    wabt::Index index,
    wabt::Type elemType,
    const wabt::Limits *elemLimits) {
  WasmTableType tt;
  tt.elemType = convertType(elemType);
  tt.limits = convertLimits(elemLimits);
  moduleInfo_.tables.push_back(std::move(tt));
  return wabt::Result::Ok;
}

// --- Memory section ---

wabt::Result BinaryReaderHermesIRGen::OnMemoryCount(wabt::Index count) {
  moduleInfo_.memories.reserve(count);
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnMemory(
    wabt::Index index,
    const wabt::Limits *limits,
    uint32_t pageSize) {
  WasmMemoryType mt;
  mt.limits = convertLimits(limits);
  moduleInfo_.memories.push_back(std::move(mt));
  return wabt::Result::Ok;
}

// --- Global section ---

wabt::Result BinaryReaderHermesIRGen::OnGlobalCount(wabt::Index count) {
  moduleInfo_.globals.reserve(count);
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::BeginGlobal(
    wabt::Index index,
    wabt::Type type,
    bool mutable_) {
  WasmGlobal g;
  g.type.type = convertType(type);
  g.type.mutable_ = mutable_;
  moduleInfo_.globals.push_back(std::move(g));
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::BeginGlobalInitExpr(wabt::Index index) {
  initExprContext_ = InitExprContext::Global;
  currentInitExprIndex_ = index;
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::EndGlobalInitExpr(wabt::Index index) {
  initExprContext_ = InitExprContext::None;
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::EndGlobal(wabt::Index index) {
  return wabt::Result::Ok;
}

// --- Export section ---

wabt::Result BinaryReaderHermesIRGen::OnExportCount(wabt::Index count) {
  moduleInfo_.exports.reserve(count);
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnExport(
    wabt::Index index,
    wabt::ExternalKind kind,
    wabt::Index itemIndex,
    std::string_view name) {
  WasmExport exp;
  exp.name = std::string(name);
  exp.kind = convertExternalKind(kind);
  exp.index = itemIndex;
  moduleInfo_.exports.push_back(std::move(exp));
  return wabt::Result::Ok;
}

// --- Start section ---

wabt::Result BinaryReaderHermesIRGen::OnStartFunction(wabt::Index funcIndex) {
  moduleInfo_.startFunction = funcIndex;
  return wabt::Result::Ok;
}

// --- Element section ---

wabt::Result BinaryReaderHermesIRGen::OnElemSegmentCount(wabt::Index count) {
  moduleInfo_.elements.reserve(count);
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::BeginElemSegment(
    wabt::Index index,
    wabt::Index tableIndex,
    uint8_t flags) {
  WasmElemSegment seg;
  if (flags == SegDeclared) {
    seg.mode = WasmElemSegment::Mode::Declarative;
  } else if (flags & SegPassive) {
    seg.mode = WasmElemSegment::Mode::Passive;
  } else {
    seg.mode = WasmElemSegment::Mode::Active;
    seg.tableIndex = tableIndex;
  }
  moduleInfo_.elements.push_back(std::move(seg));
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::BeginElemSegmentInitExpr(
    wabt::Index index) {
  initExprContext_ = InitExprContext::ElemSegmentOffset;
  currentInitExprIndex_ = index;
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::EndElemSegmentInitExpr(
    wabt::Index index) {
  initExprContext_ = InitExprContext::None;
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnElemSegmentElemExprCount(
    wabt::Index index,
    wabt::Index count) {
  assert(index < moduleInfo_.elements.size());
  moduleInfo_.elements[index].funcIndices.reserve(count);
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::BeginElemExpr(
    wabt::Index elemIndex,
    wabt::Index exprIndex) {
  initExprContext_ = InitExprContext::ElemExpr;
  currentInitExprIndex_ = elemIndex;
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::EndElemExpr(
    wabt::Index elemIndex,
    wabt::Index exprIndex) {
  initExprContext_ = InitExprContext::None;
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::EndElemSegment(wabt::Index index) {
  return wabt::Result::Ok;
}

// --- Data section ---

wabt::Result BinaryReaderHermesIRGen::OnDataSegmentCount(wabt::Index count) {
  moduleInfo_.dataSegments.reserve(count);
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::BeginDataSegment(
    wabt::Index index,
    wabt::Index memoryIndex,
    uint8_t flags) {
  WasmDataSegment seg;
  if (flags & SegPassive) {
    seg.mode = WasmDataSegment::Mode::Passive;
  } else {
    seg.mode = WasmDataSegment::Mode::Active;
    seg.memoryIndex = memoryIndex;
  }
  moduleInfo_.dataSegments.push_back(std::move(seg));
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::BeginDataSegmentInitExpr(
    wabt::Index index) {
  initExprContext_ = InitExprContext::DataSegment;
  currentInitExprIndex_ = index;
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::EndDataSegmentInitExpr(
    wabt::Index index) {
  initExprContext_ = InitExprContext::None;
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnDataSegmentData(
    wabt::Index index,
    const void *data,
    wabt::Address size) {
  assert(index < moduleInfo_.dataSegments.size());
  auto *bytes = static_cast<const uint8_t *>(data);
  moduleInfo_.dataSegments[index].data.assign(bytes, bytes + size);
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::EndDataSegment(wabt::Index index) {
  return wabt::Result::Ok;
}

// --- Names section ---

wabt::Result BinaryReaderHermesIRGen::OnModuleName(std::string_view name) {
  moduleInfo_.names.moduleName = std::string(name);
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnFunctionNamesCount(
    wabt::Index numFunctions) {
  // Pre-size the vector. Function name entries may be sparse, so resize
  // to the total function count if available, otherwise use numFunctions.
  uint32_t totalFuncs = moduleInfo_.totalFunctionCount();
  if (totalFuncs > 0) {
    moduleInfo_.names.functionNames.resize(totalFuncs);
  } else {
    moduleInfo_.names.functionNames.resize(numFunctions);
  }
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnFunctionName(
    wabt::Index functionIndex,
    std::string_view functionName) {
  if (functionIndex < moduleInfo_.names.functionNames.size()) {
    moduleInfo_.names.functionNames[functionIndex] =
        std::string(functionName);
  }
  return wabt::Result::Ok;
}

// --- Code section / Function body callbacks ---

wabt::Result BinaryReaderHermesIRGen::BeginCodeSection(wabt::Offset size) {
  // By the time the code section is reached, all module-level sections
  // (type, import, function, table, memory, global, export, start, elem)
  // have been parsed. Create the IR functions now.
  if (irgen_)
    irgen_->createFunctions();
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::BeginFunctionBody(
    wabt::Index index,
    wabt::Offset size) {
  if (!irgen_)
    return wabt::Result::Ok;

  // wabt passes the full Wasm function index (num_func_imports + i),
  // not the code-section-relative index.
  currentBodyFuncIndex_ = index;
  currentLocalTypes_.clear();
  inFunctionBody_ = true;
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnLocalDeclCount(wabt::Index count) {
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnLocalDecl(
    wabt::Index declIndex,
    wabt::Index count,
    wabt::Type type) {
  if (!irgen_)
    return wabt::Result::Ok;

  WasmValType vt = convertType(type);
  for (wabt::Index i = 0; i < count; ++i) {
    currentLocalTypes_.push_back(vt);
  }
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::EndLocalDecls() {
  if (!irgen_)
    return wabt::Result::Ok;

  // All local declarations have been accumulated. Now begin the function
  // in WasmIRGen, which creates AllocStackInsts for params and locals.
  irgen_->beginFunction(currentBodyFuncIndex_, currentLocalTypes_);
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::EndFunctionBody(wabt::Index index) {
  if (!irgen_) {
    inFunctionBody_ = false;
    return wabt::Result::Ok;
  }

  irgen_->endFunction();
  inFunctionBody_ = false;
  return wabt::Result::Ok;
}

// --- Init expression callbacks ---
// These are called in the context of globals, element segments, and data
// segments. We use initExprContext_ to route the values to the right place.

wabt::Result BinaryReaderHermesIRGen::OnI32ConstExpr(uint32_t value) {
  // In a function body, dispatch to WasmIRGen.
  if (inFunctionBody_ && irgen_) {
    irgen_->onI32Const(static_cast<int32_t>(value));
    return wabt::Result::Ok;
  }

  switch (initExprContext_) {
    case InitExprContext::Global: {
      assert(
          currentInitExprIndex_ < moduleInfo_.globals.size() &&
          "global index out of range");
      auto &g = moduleInfo_.globals[currentInitExprIndex_];
      g.initKind = WasmGlobal::InitKind::I32Const;
      g.initValue.i32Val = static_cast<int32_t>(value);
      break;
    }
    case InitExprContext::ElemSegmentOffset: {
      assert(
          currentInitExprIndex_ < moduleInfo_.elements.size() &&
          "elem index out of range");
      auto &seg = moduleInfo_.elements[currentInitExprIndex_];
      seg.offsetKind = WasmGlobal::InitKind::I32Const;
      seg.offsetValue = static_cast<int32_t>(value);
      break;
    }
    case InitExprContext::DataSegment: {
      assert(
          currentInitExprIndex_ < moduleInfo_.dataSegments.size() &&
          "data index out of range");
      auto &seg = moduleInfo_.dataSegments[currentInitExprIndex_];
      seg.offsetKind = WasmGlobal::InitKind::I32Const;
      seg.offsetValue = static_cast<int32_t>(value);
      break;
    }
    case InitExprContext::ElemExpr:
    case InitExprContext::None:
      break;
  }
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnI64ConstExpr(uint64_t value) {
  if (inFunctionBody_ && irgen_) {
    irgen_->onI64Const(static_cast<int64_t>(value));
    return wabt::Result::Ok;
  }

  if (initExprContext_ == InitExprContext::Global) {
    assert(
        currentInitExprIndex_ < moduleInfo_.globals.size() &&
        "global index out of range");
    auto &g = moduleInfo_.globals[currentInitExprIndex_];
    g.initKind = WasmGlobal::InitKind::I64Const;
    g.initValue.i64Val = static_cast<int64_t>(value);
  }
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnF32ConstExpr(uint32_t valueBits) {
  if (inFunctionBody_ && irgen_) {
    float f;
    memcpy(&f, &valueBits, sizeof(f));
    irgen_->onF32Const(f);
    return wabt::Result::Ok;
  }

  if (initExprContext_ == InitExprContext::Global) {
    assert(
        currentInitExprIndex_ < moduleInfo_.globals.size() &&
        "global index out of range");
    auto &g = moduleInfo_.globals[currentInitExprIndex_];
    g.initKind = WasmGlobal::InitKind::F32Const;
    float f;
    memcpy(&f, &valueBits, sizeof(f));
    g.initValue.f32Val = f;
  }
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnF64ConstExpr(uint64_t valueBits) {
  if (inFunctionBody_ && irgen_) {
    double d;
    memcpy(&d, &valueBits, sizeof(d));
    irgen_->onF64Const(d);
    return wabt::Result::Ok;
  }

  if (initExprContext_ == InitExprContext::Global) {
    assert(
        currentInitExprIndex_ < moduleInfo_.globals.size() &&
        "global index out of range");
    auto &g = moduleInfo_.globals[currentInitExprIndex_];
    g.initKind = WasmGlobal::InitKind::F64Const;
    double d;
    memcpy(&d, &valueBits, sizeof(d));
    g.initValue.f64Val = d;
  }
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnGlobalGetExpr(
    wabt::Index globalIndex) {
  // In a function body, global.get is handled by a later step (K.1).
  if (inFunctionBody_)
    return wabt::Result::Ok;

  switch (initExprContext_) {
    case InitExprContext::Global: {
      assert(
          currentInitExprIndex_ < moduleInfo_.globals.size() &&
          "global index out of range");
      auto &g = moduleInfo_.globals[currentInitExprIndex_];
      g.initKind = WasmGlobal::InitKind::GlobalGet;
      g.initValue.globalIndex = globalIndex;
      break;
    }
    case InitExprContext::ElemSegmentOffset: {
      assert(
          currentInitExprIndex_ < moduleInfo_.elements.size() &&
          "elem index out of range");
      auto &seg = moduleInfo_.elements[currentInitExprIndex_];
      seg.offsetKind = WasmGlobal::InitKind::GlobalGet;
      seg.offsetGlobalIdx = globalIndex;
      break;
    }
    case InitExprContext::DataSegment: {
      assert(
          currentInitExprIndex_ < moduleInfo_.dataSegments.size() &&
          "data index out of range");
      auto &seg = moduleInfo_.dataSegments[currentInitExprIndex_];
      seg.offsetKind = WasmGlobal::InitKind::GlobalGet;
      seg.offsetGlobalIdx = globalIndex;
      break;
    }
    case InitExprContext::ElemExpr:
    case InitExprContext::None:
      break;
  }
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnRefNullExpr(wabt::Type type) {
  // In a function body, ref.null is handled by a later step.
  if (inFunctionBody_)
    return wabt::Result::Ok;

  if (initExprContext_ == InitExprContext::Global) {
    assert(
        currentInitExprIndex_ < moduleInfo_.globals.size() &&
        "global index out of range");
    auto &g = moduleInfo_.globals[currentInitExprIndex_];
    g.initKind = WasmGlobal::InitKind::RefNull;
  }
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnRefFuncExpr(wabt::Index funcIndex) {
  // In a function body, ref.func is handled by a later step.
  if (inFunctionBody_)
    return wabt::Result::Ok;

  switch (initExprContext_) {
    case InitExprContext::Global: {
      assert(
          currentInitExprIndex_ < moduleInfo_.globals.size() &&
          "global index out of range");
      auto &g = moduleInfo_.globals[currentInitExprIndex_];
      g.initKind = WasmGlobal::InitKind::RefFunc;
      g.initValue.funcIndex = funcIndex;
      break;
    }
    case InitExprContext::ElemExpr: {
      // ref.func in an element expression adds the func index.
      assert(
          currentInitExprIndex_ < moduleInfo_.elements.size() &&
          "elem index out of range");
      moduleInfo_.elements[currentInitExprIndex_].funcIndices.push_back(
          funcIndex);
      break;
    }
    case InitExprContext::ElemSegmentOffset:
    case InitExprContext::None:
    case InitExprContext::DataSegment:
      break;
  }
  return wabt::Result::Ok;
}

// --- Local variable instruction callbacks ---

wabt::Result BinaryReaderHermesIRGen::OnLocalGetExpr(
    wabt::Index localIndex) {
  if (inFunctionBody_ && irgen_) {
    irgen_->onLocalGet(localIndex);
  }
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnLocalSetExpr(
    wabt::Index localIndex) {
  if (inFunctionBody_ && irgen_) {
    irgen_->onLocalSet(localIndex);
  }
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnLocalTeeExpr(
    wabt::Index localIndex) {
  if (inFunctionBody_ && irgen_) {
    irgen_->onLocalTee(localIndex);
  }
  return wabt::Result::Ok;
}

} // namespace wasm
} // namespace hermes
