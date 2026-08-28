/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/WasmFrontend/BinaryReaderHermesIRGen.h"

#include "hermes/Support/UTF8.h"
#include "hermes/WasmIRGen/WasmIRGen.h"

#include <cassert>
#include <cstring>

// wabt segment flags from wabt/common.h.
namespace {
constexpr uint8_t SegPassive = 1;
constexpr uint8_t SegDeclared = 3;

/// Normalize a UTF-8 string to Hermes's internal representation.
/// Hermes encodes supplementary plane characters (>= U+10000) as surrogate
/// pairs, with each surrogate individually encoded in UTF-8 (6 bytes total),
/// rather than as standard 4-byte UTF-8 sequences.  Wasm binary names use
/// standard UTF-8, so we must convert them to match Hermes's encoding before
/// they enter the string table.  This performs the same UTF-8 -> UTF-16 ->
/// UTF-8 roundtrip that ConsecutiveStringStorage uses, ensuring consistent
/// keys.
std::string normalizeToHermesUTF8(std::string_view input) {
  // Fast path: pure ASCII needs no conversion.
  bool hasNonASCII = false;
  for (char c : input) {
    if (static_cast<unsigned char>(c) > 0x7F) {
      hasNonASCII = true;
      break;
    }
  }
  if (!hasNonASCII)
    return std::string(input);

  // Convert UTF-8 -> UTF-16 -> UTF-8 (Hermes internal representation).
  std::vector<char16_t> u16;
  hermes::convertUTF8WithSurrogatesToUTF16(
      std::back_inserter(u16), input.data(), input.data() + input.size());
  std::string result;
  hermes::convertUTF16ToUTF8WithSingleSurrogates(result, u16);
  return result;
}
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
    case wabt::ExternalKind::Tag:
      return WasmExternalKind::Tag;
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
  imp.moduleName = normalizeToHermesUTF8(moduleName);
  imp.fieldName = normalizeToHermesUTF8(fieldName);
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
  imp.moduleName = normalizeToHermesUTF8(moduleName);
  imp.fieldName = normalizeToHermesUTF8(fieldName);
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
  imp.moduleName = normalizeToHermesUTF8(moduleName);
  imp.fieldName = normalizeToHermesUTF8(fieldName);
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
  imp.moduleName = normalizeToHermesUTF8(moduleName);
  imp.fieldName = normalizeToHermesUTF8(fieldName);
  imp.kind = WasmExternalKind::Global;
  imp.globalType.type = convertType(type);
  imp.globalType.mutable_ = mutable_;
  moduleInfo_.imports.push_back(std::move(imp));
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnImportTag(
    wabt::Index importIndex,
    std::string_view moduleName,
    std::string_view fieldName,
    wabt::Index tagIndex,
    wabt::Index sigIndex) {
  WasmImport imp;
  imp.moduleName = normalizeToHermesUTF8(moduleName);
  imp.fieldName = normalizeToHermesUTF8(fieldName);
  imp.kind = WasmExternalKind::Tag;
  imp.tagTypeIndex = sigIndex;
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

// --- Tag section ---

wabt::Result BinaryReaderHermesIRGen::OnTagCount(wabt::Index count) {
  moduleInfo_.tags.reserve(count);
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnTagType(
    wabt::Index index,
    wabt::Index sigIndex) {
  WasmTag tag;
  tag.typeIndex = sigIndex;
  moduleInfo_.tags.push_back(std::move(tag));
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
  exp.name = normalizeToHermesUTF8(name);
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
  moduleInfo_.names.moduleName = normalizeToHermesUTF8(name);
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
        normalizeToHermesUTF8(functionName);
  }
  return wabt::Result::Ok;
}

// --- Code section / Function body callbacks ---

wabt::Result BinaryReaderHermesIRGen::BeginCodeSection(wabt::Offset size) {
  // By the time the code section is reached, all module-level sections
  // (type, import, function, table, memory, global, export, start, elem)
  // have been parsed. Create the IR functions now.
  if (irgen_ && !functionsCreated_) {
    irgen_->createFunctions();
    functionsCreated_ = true;
  }
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
      g.initExpr.push_back(
          InitExprOp::makeI32Const(static_cast<int32_t>(value)));
      break;
    }
    case InitExprContext::ElemSegmentOffset: {
      assert(
          currentInitExprIndex_ < moduleInfo_.elements.size() &&
          "elem index out of range");
      auto &seg = moduleInfo_.elements[currentInitExprIndex_];
      seg.offsetKind = WasmGlobal::InitKind::I32Const;
      seg.offsetValue = static_cast<int32_t>(value);
      seg.offsetExpr.push_back(
          InitExprOp::makeI32Const(static_cast<int32_t>(value)));
      break;
    }
    case InitExprContext::DataSegment: {
      assert(
          currentInitExprIndex_ < moduleInfo_.dataSegments.size() &&
          "data index out of range");
      auto &seg = moduleInfo_.dataSegments[currentInitExprIndex_];
      seg.offsetKind = WasmGlobal::InitKind::I32Const;
      seg.offsetValue = static_cast<int32_t>(value);
      seg.offsetExpr.push_back(
          InitExprOp::makeI32Const(static_cast<int32_t>(value)));
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
  if (inFunctionBody_ && irgen_) {
    irgen_->onGlobalGet(globalIndex);
    return wabt::Result::Ok;
  }
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
      g.initExpr.push_back(InitExprOp::makeGlobalGet(globalIndex));
      break;
    }
    case InitExprContext::ElemSegmentOffset: {
      assert(
          currentInitExprIndex_ < moduleInfo_.elements.size() &&
          "elem index out of range");
      auto &seg = moduleInfo_.elements[currentInitExprIndex_];
      seg.offsetKind = WasmGlobal::InitKind::GlobalGet;
      seg.offsetGlobalIdx = globalIndex;
      seg.offsetExpr.push_back(InitExprOp::makeGlobalGet(globalIndex));
      break;
    }
    case InitExprContext::DataSegment: {
      assert(
          currentInitExprIndex_ < moduleInfo_.dataSegments.size() &&
          "data index out of range");
      auto &seg = moduleInfo_.dataSegments[currentInitExprIndex_];
      seg.offsetKind = WasmGlobal::InitKind::GlobalGet;
      seg.offsetGlobalIdx = globalIndex;
      seg.offsetExpr.push_back(InitExprOp::makeGlobalGet(globalIndex));
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
  if (inFunctionBody_ && irgen_) {
    irgen_->warnUnsupported("ref.null", 0, 1);
    return wabt::Result::Ok;
  }
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
  if (inFunctionBody_ && irgen_) {
    irgen_->warnUnsupported("ref.func", 0, 1);
    return wabt::Result::Ok;
  }
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

// --- Binary (two-operand) instruction callback ---

wabt::Result BinaryReaderHermesIRGen::OnBinaryExpr(wabt::Opcode opcode) {
  // Handle binary ops in init expression contexts (extended const exprs).
  if (initExprContext_ == InitExprContext::DataSegment ||
      initExprContext_ == InitExprContext::ElemSegmentOffset ||
      initExprContext_ == InitExprContext::Global) {
    // Every sibling init-expr callback asserts the index before using it;
    // do the same here rather than indexing blind.
    assert(
        currentInitExprIndex_ <
            (initExprContext_ == InitExprContext::DataSegment
                 ? moduleInfo_.dataSegments.size()
                 : initExprContext_ == InitExprContext::ElemSegmentOffset
                 ? moduleInfo_.elements.size()
                 : moduleInfo_.globals.size()) &&
        "init expr index out of range");
    auto &expr = (initExprContext_ == InitExprContext::DataSegment)
        ? moduleInfo_.dataSegments[currentInitExprIndex_].offsetExpr
        : (initExprContext_ == InitExprContext::ElemSegmentOffset)
        ? moduleInfo_.elements[currentInitExprIndex_].offsetExpr
        : moduleInfo_.globals[currentInitExprIndex_].initExpr;
    switch (static_cast<wabt::Opcode::Enum>(opcode)) {
      case wabt::Opcode::I32Add:
        expr.push_back(InitExprOp::makeAdd());
        break;
      case wabt::Opcode::I32Sub:
        expr.push_back(InitExprOp::makeSub());
        break;
      case wabt::Opcode::I32Mul:
        expr.push_back(InitExprOp::makeMul());
        break;
      default:
        break;
    }
    return wabt::Result::Ok;
  }

  if (!inFunctionBody_ || !irgen_)
    return wabt::Result::Ok;

  switch (static_cast<wabt::Opcode::Enum>(opcode)) {
    case wabt::Opcode::I32Add:
      irgen_->onI32Add();
      break;
    case wabt::Opcode::I32Sub:
      irgen_->onI32Sub();
      break;
    case wabt::Opcode::I32Mul:
      irgen_->onI32Mul();
      break;
    case wabt::Opcode::I32And:
      irgen_->onI32And();
      break;
    case wabt::Opcode::I32Or:
      irgen_->onI32Or();
      break;
    case wabt::Opcode::I32Xor:
      irgen_->onI32Xor();
      break;
    case wabt::Opcode::I32Shl:
      irgen_->onI32Shl();
      break;
    case wabt::Opcode::I32ShrS:
      irgen_->onI32ShrS();
      break;
    case wabt::Opcode::I32ShrU:
      irgen_->onI32ShrU();
      break;
    // --- i32 trapping division/remainder (F.2) ---
    case wabt::Opcode::I32DivS:
      irgen_->onI32DivS();
      break;
    case wabt::Opcode::I32DivU:
      irgen_->onI32DivU();
      break;
    case wabt::Opcode::I32RemS:
      irgen_->onI32RemS();
      break;
    case wabt::Opcode::I32RemU:
      irgen_->onI32RemU();
      break;
    case wabt::Opcode::I32Rotl:
      irgen_->onI32Rotl();
      break;
    case wabt::Opcode::I32Rotr:
      irgen_->onI32Rotr();
      break;
    // --- i64 binary ops (G.3) ---
    case wabt::Opcode::I64Add:
      irgen_->onI64Add();
      break;
    case wabt::Opcode::I64Sub:
      irgen_->onI64Sub();
      break;
    case wabt::Opcode::I64Mul:
      irgen_->onI64Mul();
      break;
    case wabt::Opcode::I64DivS:
      irgen_->onI64DivS();
      break;
    case wabt::Opcode::I64DivU:
      irgen_->onI64DivU();
      break;
    case wabt::Opcode::I64RemS:
      irgen_->onI64RemS();
      break;
    case wabt::Opcode::I64RemU:
      irgen_->onI64RemU();
      break;
    case wabt::Opcode::I64And:
      irgen_->onI64And();
      break;
    case wabt::Opcode::I64Or:
      irgen_->onI64Or();
      break;
    case wabt::Opcode::I64Xor:
      irgen_->onI64Xor();
      break;
    case wabt::Opcode::I64Shl:
      irgen_->onI64Shl();
      break;
    case wabt::Opcode::I64ShrS:
      irgen_->onI64ShrS();
      break;
    case wabt::Opcode::I64ShrU:
      irgen_->onI64ShrU();
      break;
    case wabt::Opcode::I64Rotl:
      irgen_->onI64Rotl();
      break;
    case wabt::Opcode::I64Rotr:
      irgen_->onI64Rotr();
      break;
    // --- f32 binary ops (E.2) ---
    case wabt::Opcode::F32Add:
      irgen_->onF32Add();
      break;
    case wabt::Opcode::F32Sub:
      irgen_->onF32Sub();
      break;
    case wabt::Opcode::F32Mul:
      irgen_->onF32Mul();
      break;
    case wabt::Opcode::F32Div:
      irgen_->onF32Div();
      break;
    case wabt::Opcode::F32Min:
      irgen_->onF32Min();
      break;
    case wabt::Opcode::F32Max:
      irgen_->onF32Max();
      break;
    case wabt::Opcode::F32Copysign:
      irgen_->onF32Copysign();
      break;
    // --- f64 binary ops (E.1) ---
    case wabt::Opcode::F64Add:
      irgen_->onF64Add();
      break;
    case wabt::Opcode::F64Sub:
      irgen_->onF64Sub();
      break;
    case wabt::Opcode::F64Mul:
      irgen_->onF64Mul();
      break;
    case wabt::Opcode::F64Div:
      irgen_->onF64Div();
      break;
    case wabt::Opcode::F64Min:
      irgen_->onF64Min();
      break;
    case wabt::Opcode::F64Max:
      irgen_->onF64Max();
      break;
    case wabt::Opcode::F64Copysign:
      irgen_->onF64Copysign();
      break;
    default:
      // Non-MVP binary opcode (SIMD, etc.) — warn.
      irgen_->warnUnsupported("binary(unknown)", 2, 1);
      break;
  }
  return wabt::Result::Ok;
}

// --- Compare (two-operand comparison) instruction callback ---

wabt::Result BinaryReaderHermesIRGen::OnCompareExpr(wabt::Opcode opcode) {
  if (!inFunctionBody_ || !irgen_)
    return wabt::Result::Ok;

  switch (static_cast<wabt::Opcode::Enum>(opcode)) {
    case wabt::Opcode::I32Eq:
      irgen_->onI32Eq();
      break;
    case wabt::Opcode::I32Ne:
      irgen_->onI32Ne();
      break;
    case wabt::Opcode::I32LtS:
      irgen_->onI32LtS();
      break;
    case wabt::Opcode::I32GtS:
      irgen_->onI32GtS();
      break;
    case wabt::Opcode::I32LeS:
      irgen_->onI32LeS();
      break;
    case wabt::Opcode::I32GeS:
      irgen_->onI32GeS();
      break;
    case wabt::Opcode::I32LtU:
      irgen_->onI32LtU();
      break;
    case wabt::Opcode::I32GtU:
      irgen_->onI32GtU();
      break;
    case wabt::Opcode::I32LeU:
      irgen_->onI32LeU();
      break;
    case wabt::Opcode::I32GeU:
      irgen_->onI32GeU();
      break;
    // --- i64 compare ops (G.3) ---
    case wabt::Opcode::I64Eq:
      irgen_->onI64Eq();
      break;
    case wabt::Opcode::I64Ne:
      irgen_->onI64Ne();
      break;
    case wabt::Opcode::I64LtS:
      irgen_->onI64LtS();
      break;
    case wabt::Opcode::I64GtS:
      irgen_->onI64GtS();
      break;
    case wabt::Opcode::I64LeS:
      irgen_->onI64LeS();
      break;
    case wabt::Opcode::I64GeS:
      irgen_->onI64GeS();
      break;
    case wabt::Opcode::I64LtU:
      irgen_->onI64LtU();
      break;
    case wabt::Opcode::I64GtU:
      irgen_->onI64GtU();
      break;
    case wabt::Opcode::I64LeU:
      irgen_->onI64LeU();
      break;
    case wabt::Opcode::I64GeU:
      irgen_->onI64GeU();
      break;
    // --- f32 compare ops (E.3) ---
    case wabt::Opcode::F32Eq:
      irgen_->onF32Eq();
      break;
    case wabt::Opcode::F32Ne:
      irgen_->onF32Ne();
      break;
    case wabt::Opcode::F32Lt:
      irgen_->onF32Lt();
      break;
    case wabt::Opcode::F32Gt:
      irgen_->onF32Gt();
      break;
    case wabt::Opcode::F32Le:
      irgen_->onF32Le();
      break;
    case wabt::Opcode::F32Ge:
      irgen_->onF32Ge();
      break;
    // --- f64 compare ops (E.1) ---
    case wabt::Opcode::F64Eq:
      irgen_->onF64Eq();
      break;
    case wabt::Opcode::F64Ne:
      irgen_->onF64Ne();
      break;
    case wabt::Opcode::F64Lt:
      irgen_->onF64Lt();
      break;
    case wabt::Opcode::F64Gt:
      irgen_->onF64Gt();
      break;
    case wabt::Opcode::F64Le:
      irgen_->onF64Le();
      break;
    case wabt::Opcode::F64Ge:
      irgen_->onF64Ge();
      break;
    default:
      // Non-MVP compare opcode (SIMD, etc.) — warn.
      irgen_->warnUnsupported("compare(unknown)", 2, 1);
      break;
  }
  return wabt::Result::Ok;
}

// --- Convert instruction callback ---
// wabt dispatches i32.eqz, i64.eqz, and all type conversions via
// OnConvertExpr (not OnUnaryExpr).

wabt::Result BinaryReaderHermesIRGen::OnConvertExpr(wabt::Opcode opcode) {
  if (!inFunctionBody_ || !irgen_)
    return wabt::Result::Ok;

  switch (static_cast<wabt::Opcode::Enum>(opcode)) {
    case wabt::Opcode::I32Eqz:
      irgen_->onI32Eqz();
      break;
    // --- i64 test (G.3) ---
    case wabt::Opcode::I64Eqz:
      irgen_->onI64Eqz();
      break;
    // --- Int-to-int conversions (deferred to Parts F, G) ---
    case wabt::Opcode::I32WrapI64:
      irgen_->onI32WrapI64();
      break;
    case wabt::Opcode::I64ExtendI32S:
      irgen_->onI64ExtendI32S();
      break;
    case wabt::Opcode::I64ExtendI32U:
      irgen_->onI64ExtendI32U();
      break;
    // --- Float-to-int truncations (F.4) ---
    case wabt::Opcode::I32TruncF32S:
      irgen_->onI32TruncF32S();
      break;
    case wabt::Opcode::I32TruncF32U:
      irgen_->onI32TruncF32U();
      break;
    case wabt::Opcode::I32TruncF64S:
      irgen_->onI32TruncF64S();
      break;
    case wabt::Opcode::I32TruncF64U:
      irgen_->onI32TruncF64U();
      break;
    case wabt::Opcode::I64TruncF32S:
      irgen_->onI64TruncF32S();
      break;
    case wabt::Opcode::I64TruncF32U:
      irgen_->onI64TruncF32U();
      break;
    case wabt::Opcode::I64TruncF64S:
      irgen_->onI64TruncF64S();
      break;
    case wabt::Opcode::I64TruncF64U:
      irgen_->onI64TruncF64U();
      break;
    // --- Int-to-float conversions (F.4) ---
    case wabt::Opcode::F32ConvertI32S:
      irgen_->onF32ConvertI32S();
      break;
    case wabt::Opcode::F32ConvertI32U:
      irgen_->onF32ConvertI32U();
      break;
    case wabt::Opcode::F32ConvertI64S:
      irgen_->onF32ConvertI64S();
      break;
    case wabt::Opcode::F32ConvertI64U:
      irgen_->onF32ConvertI64U();
      break;
    case wabt::Opcode::F64ConvertI32S:
      irgen_->onF64ConvertI32S();
      break;
    case wabt::Opcode::F64ConvertI32U:
      irgen_->onF64ConvertI32U();
      break;
    case wabt::Opcode::F64ConvertI64S:
      irgen_->onF64ConvertI64S();
      break;
    case wabt::Opcode::F64ConvertI64U:
      irgen_->onF64ConvertI64U();
      break;
    // --- Float-to-float conversions (E.1, E.2) ---
    case wabt::Opcode::F32DemoteF64:
      irgen_->onF32DemoteF64();
      break;
    case wabt::Opcode::F64PromoteF32:
      irgen_->onF64PromoteF32();
      break;
    // --- Reinterpret/bitcast (F.4) ---
    case wabt::Opcode::I32ReinterpretF32:
      irgen_->onI32ReinterpretF32();
      break;
    case wabt::Opcode::I64ReinterpretF64:
      irgen_->onI64ReinterpretF64();
      break;
    case wabt::Opcode::F32ReinterpretI32:
      irgen_->onF32ReinterpretI32();
      break;
    case wabt::Opcode::F64ReinterpretI64:
      irgen_->onF64ReinterpretI64();
      break;
    // --- Saturating truncations (F.4) ---
    case wabt::Opcode::I32TruncSatF32S:
      irgen_->onI32TruncSatF32S();
      break;
    case wabt::Opcode::I32TruncSatF32U:
      irgen_->onI32TruncSatF32U();
      break;
    case wabt::Opcode::I32TruncSatF64S:
      irgen_->onI32TruncSatF64S();
      break;
    case wabt::Opcode::I32TruncSatF64U:
      irgen_->onI32TruncSatF64U();
      break;
    case wabt::Opcode::I64TruncSatF32S:
      irgen_->onI64TruncSatF32S();
      break;
    case wabt::Opcode::I64TruncSatF32U:
      irgen_->onI64TruncSatF32U();
      break;
    case wabt::Opcode::I64TruncSatF64S:
      irgen_->onI64TruncSatF64S();
      break;
    case wabt::Opcode::I64TruncSatF64U:
      irgen_->onI64TruncSatF64U();
      break;
    default:
      // Non-MVP convert opcode (SIMD, etc.) — warn.
      irgen_->warnUnsupported("convert(unknown)", 1, 1);
      break;
  }
  return wabt::Result::Ok;
}

// --- Return and drop instruction callbacks ---

wabt::Result BinaryReaderHermesIRGen::OnReturnExpr() {
  if (inFunctionBody_ && irgen_) {
    irgen_->onReturn();
  }
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnDropExpr() {
  if (inFunctionBody_ && irgen_) {
    irgen_->onDrop();
  }
  return wabt::Result::Ok;
}

// --- Block/end/br/br_if instruction callbacks ---

/// Convert a block/loop/if/try signature type to a WasmFuncType with both
/// params and results. Handles both simple value types (void, i32, f64, etc.)
/// and type index references (for multi-value blocks and block params).
WasmFuncType BinaryReaderHermesIRGen::convertBlockSigType(
    wabt::Type sigType) {
  WasmFuncType blockType;
  if (static_cast<wabt::Type::Enum>(sigType) == wabt::Type::Void) {
    // No params or results.
    return blockType;
  }
  if (sigType.IsIndex()) {
    // Multi-value block or block with params: look up the type index.
    auto idx = sigType.GetIndex();
    if (idx < moduleInfo_.types.size()) {
      return moduleInfo_.types[idx];
    }
    // Invalid type index — return empty (treated as void).
    return blockType;
  }
  // Simple value type — no params, one result.
  blockType.results.push_back(convertType(sigType));
  return blockType;
}

wabt::Result BinaryReaderHermesIRGen::OnBlockExpr(wabt::Type sigType) {
  if (!inFunctionBody_ || !irgen_)
    return wabt::Result::Ok;

  auto blockType = convertBlockSigType(sigType);
  irgen_->onBlock(blockType.params, blockType.results);
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnLoopExpr(wabt::Type sigType) {
  if (!inFunctionBody_ || !irgen_)
    return wabt::Result::Ok;

  auto blockType = convertBlockSigType(sigType);
  irgen_->onLoop(blockType.params, blockType.results);
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnIfExpr(wabt::Type sigType) {
  if (!inFunctionBody_ || !irgen_)
    return wabt::Result::Ok;

  auto blockType = convertBlockSigType(sigType);
  irgen_->onIf(blockType.params, blockType.results);
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnElseExpr() {
  if (!inFunctionBody_ || !irgen_)
    return wabt::Result::Ok;

  irgen_->onElse();
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnEndExpr() {
  if (!inFunctionBody_ || !irgen_)
    return wabt::Result::Ok;

  irgen_->onEnd();
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnBrExpr(wabt::Index depth) {
  if (!inFunctionBody_ || !irgen_)
    return wabt::Result::Ok;

  irgen_->onBr(depth);
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnBrIfExpr(wabt::Index depth) {
  if (!inFunctionBody_ || !irgen_)
    return wabt::Result::Ok;

  irgen_->onBrIf(depth);
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnBrTableExpr(
    wabt::Index numTargets,
    wabt::Index *targetDepths,
    wabt::Index defaultTargetDepth) {
  if (!inFunctionBody_ || !irgen_)
    return wabt::Result::Ok;

  irgen_->onBrTable(targetDepths, numTargets, defaultTargetDepth);
  return wabt::Result::Ok;
}

// --- Select instruction callback ---

wabt::Result BinaryReaderHermesIRGen::OnSelectExpr(
    wabt::Index resultCount,
    wabt::Type *resultTypes) {
  if (inFunctionBody_ && irgen_) {
    irgen_->onSelect();
  }
  return wabt::Result::Ok;
}

// --- Call instruction callback ---

wabt::Result BinaryReaderHermesIRGen::OnCallExpr(wabt::Index funcIndex) {
  if (inFunctionBody_ && irgen_) {
    irgen_->onCall(funcIndex);
  }
  return wabt::Result::Ok;
}

// --- Unreachable and nop instruction callbacks ---

wabt::Result BinaryReaderHermesIRGen::OnUnreachableExpr() {
  if (inFunctionBody_ && irgen_) {
    irgen_->onUnreachable();
  }
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnNopExpr() {
  if (inFunctionBody_ && irgen_) {
    irgen_->onNop();
  }
  return wabt::Result::Ok;
}

// --- global.set instruction callback ---

wabt::Result BinaryReaderHermesIRGen::OnGlobalSetExpr(
    wabt::Index globalIndex) {
  if (inFunctionBody_ && irgen_) {
    irgen_->onGlobalSet(globalIndex);
  }
  return wabt::Result::Ok;
}

// --- Memory load/store instruction callbacks ---

wabt::Result BinaryReaderHermesIRGen::OnLoadExpr(
    wabt::Opcode opcode,
    wabt::Index memidx,
    wabt::Address alignmentLog2,
    wabt::Address offset) {
  if (!inFunctionBody_ || !irgen_)
    return wabt::Result::Ok;

  irgen_->onLoad(
      opcode.GetName(),
      static_cast<uint32_t>(alignmentLog2),
      static_cast<uint32_t>(offset));
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnStoreExpr(
    wabt::Opcode opcode,
    wabt::Index memidx,
    wabt::Address alignmentLog2,
    wabt::Address offset) {
  if (!inFunctionBody_ || !irgen_)
    return wabt::Result::Ok;

  irgen_->onStore(
      opcode.GetName(),
      static_cast<uint32_t>(alignmentLog2),
      static_cast<uint32_t>(offset));
  return wabt::Result::Ok;
}

// --- Memory size/grow instruction callbacks ---

wabt::Result BinaryReaderHermesIRGen::OnMemorySizeExpr(wabt::Index memidx) {
  if (inFunctionBody_ && irgen_) {
    irgen_->onMemorySize();
  }
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnMemoryGrowExpr(wabt::Index memidx) {
  if (inFunctionBody_ && irgen_) {
    irgen_->onMemoryGrow();
  }
  return wabt::Result::Ok;
}

// --- call_indirect instruction callback ---

wabt::Result BinaryReaderHermesIRGen::OnCallIndirectExpr(
    wabt::Index sigIndex,
    wabt::Index tableIndex) {
  if (!inFunctionBody_ || !irgen_)
    return wabt::Result::Ok;

  assert(sigIndex < moduleInfo_.types.size() && "sigIndex out of range");
  irgen_->onCallIndirect(sigIndex, tableIndex);
  return wabt::Result::Ok;
}

// --- Unary instruction callback ---

wabt::Result BinaryReaderHermesIRGen::OnUnaryExpr(wabt::Opcode opcode) {
  if (!inFunctionBody_ || !irgen_)
    return wabt::Result::Ok;

  switch (static_cast<wabt::Opcode::Enum>(opcode)) {
    // --- i32 unary ops (F.3) ---
    case wabt::Opcode::I32Clz:
      irgen_->onI32Clz();
      break;
    case wabt::Opcode::I32Ctz:
      irgen_->onI32Ctz();
      break;
    case wabt::Opcode::I32Popcnt:
      irgen_->onI32Popcnt();
      break;
    // --- i64 unary ops (G.3) ---
    case wabt::Opcode::I64Clz:
      irgen_->onI64Clz();
      break;
    case wabt::Opcode::I64Ctz:
      irgen_->onI64Ctz();
      break;
    case wabt::Opcode::I64Popcnt:
      irgen_->onI64Popcnt();
      break;
    // --- f32 unary ops (E.2) ---
    case wabt::Opcode::F32Abs:
      irgen_->onF32Abs();
      break;
    case wabt::Opcode::F32Neg:
      irgen_->onF32Neg();
      break;
    case wabt::Opcode::F32Ceil:
      irgen_->onF32Ceil();
      break;
    case wabt::Opcode::F32Floor:
      irgen_->onF32Floor();
      break;
    case wabt::Opcode::F32Trunc:
      irgen_->onF32Trunc();
      break;
    case wabt::Opcode::F32Nearest:
      irgen_->onF32Nearest();
      break;
    case wabt::Opcode::F32Sqrt:
      irgen_->onF32Sqrt();
      break;
    // --- f64 unary ops (E.1) ---
    case wabt::Opcode::F64Abs:
      irgen_->onF64Abs();
      break;
    case wabt::Opcode::F64Neg:
      irgen_->onF64Neg();
      break;
    case wabt::Opcode::F64Ceil:
      irgen_->onF64Ceil();
      break;
    case wabt::Opcode::F64Floor:
      irgen_->onF64Floor();
      break;
    case wabt::Opcode::F64Trunc:
      irgen_->onF64Trunc();
      break;
    case wabt::Opcode::F64Nearest:
      irgen_->onF64Nearest();
      break;
    case wabt::Opcode::F64Sqrt:
      irgen_->onF64Sqrt();
      break;
    // --- Sign-extension operators (F.3) ---
    case wabt::Opcode::I32Extend8S:
      irgen_->onI32Extend8S();
      break;
    case wabt::Opcode::I32Extend16S:
      irgen_->onI32Extend16S();
      break;
    case wabt::Opcode::I64Extend8S:
      irgen_->onI64Extend8S();
      break;
    case wabt::Opcode::I64Extend16S:
      irgen_->onI64Extend16S();
      break;
    case wabt::Opcode::I64Extend32S:
      irgen_->onI64Extend32S();
      break;
    default:
      // Non-MVP unary opcode (SIMD, etc.) — warn.
      irgen_->warnUnsupported("unary(unknown)", 1, 1);
      break;
  }
  return wabt::Result::Ok;
}

// --- Table instruction callbacks ---

wabt::Result BinaryReaderHermesIRGen::OnTableGetExpr(
    wabt::Index tableIndex) {
  if (!inFunctionBody_ || !irgen_)
    return wabt::Result::Ok;
  irgen_->onTableGet(static_cast<uint32_t>(tableIndex));
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnTableSetExpr(
    wabt::Index tableIndex) {
  if (!inFunctionBody_ || !irgen_)
    return wabt::Result::Ok;
  irgen_->onTableSet(static_cast<uint32_t>(tableIndex));
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnTableSizeExpr(
    wabt::Index tableIndex) {
  if (!inFunctionBody_ || !irgen_)
    return wabt::Result::Ok;
  irgen_->onTableSize(static_cast<uint32_t>(tableIndex));
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnTableGrowExpr(
    wabt::Index tableIndex) {
  if (!inFunctionBody_ || !irgen_)
    return wabt::Result::Ok;
  irgen_->onTableGrow(static_cast<uint32_t>(tableIndex));
  return wabt::Result::Ok;
}

// --- Bulk memory operation callbacks ---

wabt::Result BinaryReaderHermesIRGen::OnMemoryFillExpr(
    wabt::Index memidx) {
  if (!inFunctionBody_ || !irgen_)
    return wabt::Result::Ok;
  irgen_->onMemoryFill();
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnMemoryCopyExpr(
    wabt::Index destmemidx,
    wabt::Index srcmemidx) {
  if (!inFunctionBody_ || !irgen_)
    return wabt::Result::Ok;
  irgen_->onMemoryCopy();
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnMemoryInitExpr(
    wabt::Index segment_index,
    wabt::Index memidx) {
  if (!inFunctionBody_ || !irgen_)
    return wabt::Result::Ok;
  irgen_->onMemoryInit(static_cast<uint32_t>(segment_index));
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnDataDropExpr(
    wabt::Index segment_index) {
  if (!inFunctionBody_ || !irgen_)
    return wabt::Result::Ok;
  irgen_->onDataDrop(static_cast<uint32_t>(segment_index));
  return wabt::Result::Ok;
}

// --- Bulk table operation callbacks ---

wabt::Result BinaryReaderHermesIRGen::OnTableFillExpr(
    wabt::Index table_index) {
  if (!inFunctionBody_ || !irgen_)
    return wabt::Result::Ok;
  irgen_->onTableFill(static_cast<uint32_t>(table_index));
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnTableCopyExpr(
    wabt::Index dst_index,
    wabt::Index src_index) {
  if (!inFunctionBody_ || !irgen_)
    return wabt::Result::Ok;
  irgen_->onTableCopy(
      static_cast<uint32_t>(dst_index),
      static_cast<uint32_t>(src_index));
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnTableInitExpr(
    wabt::Index segment_index,
    wabt::Index table_index) {
  if (!inFunctionBody_ || !irgen_)
    return wabt::Result::Ok;
  irgen_->onTableInit(
      static_cast<uint32_t>(segment_index),
      static_cast<uint32_t>(table_index));
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnElemDropExpr(
    wabt::Index segment_index) {
  if (!inFunctionBody_ || !irgen_)
    return wabt::Result::Ok;
  irgen_->onElemDrop(static_cast<uint32_t>(segment_index));
  return wabt::Result::Ok;
}

// --- Exception handling instruction callbacks ---

wabt::Result BinaryReaderHermesIRGen::OnTryExpr(wabt::Type sigType) {
  if (!inFunctionBody_ || !irgen_)
    return wabt::Result::Ok;

  irgen_->onTry(convertBlockSigType(sigType).results);
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnCatchExpr(wabt::Index tagIndex) {
  if (!inFunctionBody_ || !irgen_)
    return wabt::Result::Ok;
  irgen_->onCatch(static_cast<uint32_t>(tagIndex));
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnCatchAllExpr() {
  if (!inFunctionBody_ || !irgen_)
    return wabt::Result::Ok;
  irgen_->onCatchAll();
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnThrowExpr(wabt::Index tagIndex) {
  if (!inFunctionBody_ || !irgen_)
    return wabt::Result::Ok;
  irgen_->onThrow(static_cast<uint32_t>(tagIndex));
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnRethrowExpr(wabt::Index depth) {
  if (!inFunctionBody_ || !irgen_)
    return wabt::Result::Ok;
  irgen_->onRethrow(static_cast<uint32_t>(depth));
  return wabt::Result::Ok;
}

wabt::Result BinaryReaderHermesIRGen::OnDelegateExpr(wabt::Index depth) {
  if (!inFunctionBody_ || !irgen_)
    return wabt::Result::Ok;
  irgen_->onDelegate(static_cast<uint32_t>(depth));
  return wabt::Result::Ok;
}

// --- Module end ---

wabt::Result BinaryReaderHermesIRGen::EndModule() {
  // If the module had no code section (e.g., a minimal module with only
  // a header), ensure createFunctions() is still called so the IR module
  // gets a top-level function.
  if (irgen_ && !functionsCreated_) {
    irgen_->createFunctions();
    functionsCreated_ = true;
  }
  // Finalize the module: apply data segments, call start function, build
  // exports object, and emit the return instruction. This must happen after
  // all sections (including the data section) have been parsed.
  if (irgen_) {
    irgen_->finalizeModule();
  }
  return wabt::Result::Ok;
}

} // namespace wasm
} // namespace hermes
