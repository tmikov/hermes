/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/WasmIRGen/WasmIRGen.h"

#include "hermes/FrontEndDefs/Builtins.h"
#include "hermes/IR/Analysis.h"
#include "hermes/IR/IR.h"
#include "hermes/IR/IRBuilder.h"
#include "hermes/IR/Instrs.h"
#include "hermes/WasmFrontend/WasmModuleInfo.h"

#include "llvh/ADT/DenseMap.h"
#include "llvh/ADT/SmallVector.h"
#include "llvh/ADT/Twine.h"
#include "llvh/Support/raw_ostream.h"

#include <cmath>

namespace hermes {
namespace wasm {

/// Map a WasmValType to its spec name, for diagnostics.
static const char *valTypeName(WasmValType vt) {
  switch (vt) {
    case WasmValType::I32: return "i32";
    case WasmValType::I64: return "i64";
    case WasmValType::F32: return "f32";
    case WasmValType::F64: return "f64";
    case WasmValType::FuncRef: return "funcref";
    case WasmValType::ExternRef: return "externref";
    case WasmValType::V128: return "v128";
  }
  return "unknown";
}

/// Map a WasmValType to a single character code for type strings.
static char valTypeChar(WasmValType vt) {
  switch (vt) {
    case WasmValType::I32: return 'i';
    case WasmValType::I64: return 'l';
    case WasmValType::F32: return 'f';
    case WasmValType::F64: return 'd';
    case WasmValType::FuncRef: return 'r';
    case WasmValType::ExternRef: return 'e';
    case WasmValType::V128: return 'v';
  }
  return '?';
}

/// Build a type string for a function type, e.g. "func:ii:i".
static std::string buildFuncTypeString(const WasmFuncType &ft) {
  std::string s = "func:";
  for (auto p : ft.params)
    s += valTypeChar(p);
  s += ':';
  for (auto r : ft.results)
    s += valTypeChar(r);
  return s;
}

/// Map a WasmValType to the numeric code wasmLinkGlobal compares against.
/// The codes are JSWebAssemblyGlobal::ValType, which is the enum stored in a
/// WebAssembly.Global's internal field; they are spelled out here rather than
/// included so that the Wasm frontend does not depend on a VM header. That
/// makes them an ABI between two files that cannot see each other, so
/// JSWebAssemblyGlobal.h carries static_asserts pinning the four values and
/// naming this function -- reordering the enum is a build error, not a
/// silently wrong type check.
/// 0xFF is "a Wasm type no Global can have" -- every reference type, and
/// v128. It matches nothing, which is exactly the old behaviour: no
/// __wasm_type__ string the Global constructor wrote ever named one either.
static uint8_t globalValTypeCode(WasmValType vt) {
  switch (vt) {
    case WasmValType::I32: return 0; // JSWebAssemblyGlobal::ValType::I32
    case WasmValType::I64: return 1; // JSWebAssemblyGlobal::ValType::I64
    case WasmValType::F32: return 2; // JSWebAssemblyGlobal::ValType::F32
    case WasmValType::F64: return 3; // JSWebAssemblyGlobal::ValType::F64
    default: return 0xFF;
  }
}

/// Build a type string for a tag type, e.g. "tag:i:".
/// Tags have parameters but no results per spec.
static std::string buildTagTypeString(const WasmFuncType &ft) {
  std::string s = "tag:";
  for (auto p : ft.params)
    s += valTypeChar(p);
  s += ':';
  return s;
}

/// Map a WasmValType to an IR Type.
/// If \p val is an AsInt32Inst whose operand is boolean, return the boolean
/// operand directly (suitable for use as a CondBranchInst condition).
/// Otherwise return \p val unchanged.
static Value *peekThroughAsInt32(Value *val) {
  if (auto *ai = llvh::dyn_cast<AsInt32Inst>(val)) {
    if (ai->getSingleOperand()->getType().isBooleanType())
      return ai->getSingleOperand();
  }
  return val;
}

/// Map a WasmValType to an IR Type.
static Type wasmValTypeToIRType(WasmValType vt) {
  switch (vt) {
    case WasmValType::I32:
    case WasmValType::I64:
    case WasmValType::F32:
    case WasmValType::F64:
      return Type::createNumber();
    case WasmValType::FuncRef:
      return Type::createObject();
    case WasmValType::ExternRef:
      return Type::createAnyType();
    case WasmValType::V128:
      return Type::createAnyType();
  }
  return Type::createAnyType();
}

WasmIRGen::WasmIRGen(Module &M, WasmModuleInfo &moduleInfo)
    : moduleInfo_(moduleInfo),
      builder_(&M),
      helpers_(builder_),
      test262_(M.getContext().getCodeGenerationSettings().test262) {}

bool WasmIRGen::needsReturnBuffer(const WasmFuncType &funcType) {
  if (funcType.results.size() > 1)
    return true;
  if (funcType.results.size() == 1 && funcType.results[0] == WasmValType::I64)
    return true;
  return false;
}

std::pair<std::vector<uint32_t>, uint32_t> WasmIRGen::computeRetBufLayout(
    const std::vector<WasmValType> &results) {
  std::vector<uint32_t> offsets;
  uint32_t offset = 0;
  for (auto vt : results) {
    switch (vt) {
      case WasmValType::I32:
        // Align to 4.
        offset = (offset + 3) & ~3u;
        offsets.push_back(offset);
        offset += 4;
        break;
      case WasmValType::I64:
        // Align to 4. Uses I[offset/4] (lo) + I[offset/4+1] (hi).
        offset = (offset + 3) & ~3u;
        offsets.push_back(offset);
        offset += 8;
        break;
      case WasmValType::F32:
      case WasmValType::F64:
        // Align to 8. Uses F[offset/8].
        offset = (offset + 7) & ~7u;
        offsets.push_back(offset);
        offset += 8;
        break;
      default:
        // FuncRef, ExternRef, etc: treat like i32.
        offset = (offset + 3) & ~3u;
        offsets.push_back(offset);
        offset += 4;
        break;
    }
  }
  return {offsets, offset};
}

std::pair<Value *, Value *> WasmIRGen::readI64FromRetBuf() {
  auto *loRaw = builder_.createLoadPropertyInst(
      retBufI_, builder_.getLiteralNumber(0));
  auto *hiRaw = builder_.createLoadPropertyInst(
      retBufI_, builder_.getLiteralNumber(1));
  // Uint32Array returns unsigned values. Convert to signed int32 so that
  // the split i64 representation is consistent (lo/hi are signed int32 values
  // that reconstruct the i64 when combined). This ensures i32.wrap_i64 and
  // other consumers see the correct signed value.
  auto *lo = builder_.createAsInt32Inst(loRaw);
  auto *hi = builder_.createAsInt32Inst(hiRaw);
  return {lo, hi};
}

void WasmIRGen::emitRetBufStores(const WasmFuncType &funcType) {
  auto [offsets, totalSize] = computeRetBufLayout(funcType.results);

  // Collect all result values from the stack (pop in reverse order).
  llvh::SmallVector<Value *, 8> resultVals;
  // For i64, we store lo and hi separately.
  llvh::SmallVector<Value *, 4> i64His;

  // Pop in reverse order.
  llvh::SmallVector<std::pair<Value *, Value *>, 8> poppedResults(
      funcType.results.size());
  for (size_t i = funcType.results.size(); i > 0; --i) {
    if (funcType.results[i - 1] == WasmValType::I64) {
      poppedResults[i - 1] = popI64();
    } else {
      poppedResults[i - 1] = {pop(), nullptr};
    }
  }

  // The reference array is not a parameter -- the calling convention passes
  // only the two typed-array views -- so reach it through the top-level scope
  // on demand. There is one buffer per module, so the top-level array is the
  // same object the caller will read from (the same aliasing the float view
  // already relies on; see emitRetBufLoads).
  Value *rbR = nullptr;
  auto getRbR = [&]() -> Value * {
    assert(retBufRVar_ && "reference result but no reference array");
    if (!rbR)
      rbR = builder_.createLoadFrameInst(parentScopeInst_, retBufRVar_);
    return rbR;
  };

  // Store each result into the buffer at its computed offset.
  for (size_t i = 0; i < funcType.results.size(); ++i) {
    uint32_t byteOff = offsets[i];
    switch (funcType.results[i]) {
      case WasmValType::I32: {
        // I[byteOff / 4] = val
        uint32_t idx = byteOff / 4;
        builder_.createStorePropertyStrictInst(
            poppedResults[i].first,
            retBufI_,
            builder_.getLiteralNumber(idx));
        break;
      }
      case WasmValType::I64: {
        // I[byteOff / 4] = lo, I[byteOff / 4 + 1] = hi
        uint32_t idx = byteOff / 4;
        builder_.createStorePropertyStrictInst(
            poppedResults[i].first,
            retBufI_,
            builder_.getLiteralNumber(idx));
        builder_.createStorePropertyStrictInst(
            poppedResults[i].second,
            retBufI_,
            builder_.getLiteralNumber(idx + 1));
        break;
      }
      case WasmValType::F32:
      case WasmValType::F64: {
        // F[byteOff / 8] = val
        uint32_t idx = byteOff / 8;
        builder_.createStorePropertyStrictInst(
            poppedResults[i].first,
            retBufF_,
            builder_.getLiteralNumber(idx));
        break;
      }
      case WasmValType::FuncRef:
      case WasmValType::ExternRef: {
        // R[byteOff / 4] = val. A funcref is a JS closure and an externref an
        // arbitrary JS value; neither survives a store into the Uint32Array
        // view, which coerces it to NaN and then to 0.
        uint32_t idx = byteOff / 4;
        builder_.createStorePropertyStrictInst(
            poppedResults[i].first,
            getRbR(),
            builder_.getLiteralNumber(idx));
        break;
      }
      default: {
        // V128: still unsupported. Keep the existing behavior.
        uint32_t idx = byteOff / 4;
        builder_.createStorePropertyStrictInst(
            poppedResults[i].first,
            retBufI_,
            builder_.getLiteralNumber(idx));
        break;
      }
    }
  }

  builder_.createReturnInst(builder_.getLiteralNumber(0));
}

void WasmIRGen::emitRetBufLoads(const WasmFuncType &funcType) {
  auto [offsets, totalSize] = computeRetBufLayout(funcType.results);

  // retBufF_ is the *current* function's float view, and it is set only when
  // this function itself returns through the buffer. These loads read the
  // results of a CALLEE, so a caller that returns nothing through the buffer
  // still needs the view whenever the callee has an f32/f64 result. Load it
  // on demand rather than in every function's preamble. There is one buffer
  // per module, so the top-level view is the object the callee was handed.
  Value *rbF = retBufF_;
  auto getRbF = [&]() -> Value * {
    if (!rbF)
      rbF = builder_.createLoadFrameInst(parentScopeInst_, retBufFVar_);
    return rbF;
  };

  // The reference array is never a parameter, so it is always loaded from the
  // top-level scope, for the same reason and under the same one-buffer-per-
  // module assumption as getRbF() above.
  Value *rbR = nullptr;
  auto getRbR = [&]() -> Value * {
    assert(retBufRVar_ && "reference result but no reference array");
    if (!rbR)
      rbR = builder_.createLoadFrameInst(parentScopeInst_, retBufRVar_);
    return rbR;
  };

  for (size_t i = 0; i < funcType.results.size(); ++i) {
    uint32_t byteOff = offsets[i];
    switch (funcType.results[i]) {
      case WasmValType::I32: {
        uint32_t idx = byteOff / 4;
        auto *raw = builder_.createLoadPropertyInst(
            retBufI_, builder_.getLiteralNumber(idx));
        // Convert Uint32Array unsigned value to signed int32.
        push(builder_.createAsInt32Inst(raw));
        break;
      }
      case WasmValType::I64: {
        uint32_t idx = byteOff / 4;
        auto *loRaw = builder_.createLoadPropertyInst(
            retBufI_, builder_.getLiteralNumber(idx));
        auto *hiRaw = builder_.createLoadPropertyInst(
            retBufI_, builder_.getLiteralNumber(idx + 1));
        // Convert Uint32Array unsigned values to signed int32.
        pushI64(
            builder_.createAsInt32Inst(loRaw),
            builder_.createAsInt32Inst(hiRaw));
        break;
      }
      case WasmValType::F32:
      case WasmValType::F64: {
        uint32_t idx = byteOff / 8;
        auto *val = builder_.createLoadPropertyInst(
            getRbF(), builder_.getLiteralNumber(idx));
        push(val);
        break;
      }
      case WasmValType::FuncRef:
      case WasmValType::ExternRef: {
        // Read the reference back from the parallel array. No AsInt32Inst
        // here: that narrowing exists to undo the Uint32Array's unsigned
        // reads, and a reference is not a number.
        uint32_t idx = byteOff / 4;
        push(builder_.createLoadPropertyInst(
            getRbR(), builder_.getLiteralNumber(idx)));
        break;
      }
      default: {
        // V128: still unsupported. Keep the existing behavior.
        uint32_t idx = byteOff / 4;
        auto *raw = builder_.createLoadPropertyInst(
            retBufI_, builder_.getLiteralNumber(idx));
        push(builder_.createAsInt32Inst(raw));
        break;
      }
    }
  }
}

void WasmIRGen::buildCanonicalTypeMap() {
  const auto &types = moduleInfo_.types;
  canonicalTypeIndex_.resize(types.size());

  for (uint32_t i = 0; i < types.size(); ++i) {
    // Find the first type with the same signature.
    uint32_t canon = i;
    for (uint32_t j = 0; j < i; ++j) {
      if (types[j].params == types[i].params &&
          types[j].results == types[i].results) {
        canon = canonicalTypeIndex_[j];
        break;
      }
    }
    canonicalTypeIndex_[i] = canon;
  }
}

void WasmIRGen::computeEscapableFuncs() {
  // Which function indices a funcref VALUE can name. In the supported feature
  // set a funcref is introduced in exactly two places, both of which name the
  // function by index in moduleInfo_:
  //
  //   1. Element segments -- the function goes into a table, from where
  //      table.get, WebAssembly.Table.prototype.get, table.copy, table.init
  //      and table.fill all reproduce it (the bulk operations only move
  //      already-listed functions, so they add no indices).
  //   2. A ref.func global initializer.
  //
  // ref.func inside a function body is unsupported (it warns and pushes a
  // placeholder), so there is no dynamic way to materialize a funcref for an
  // arbitrary function.
  //
  // What this set is FOR is exportedFuncVars_: an index in it gets a canonical
  // Exported Function even if it is neither exported nor imported, because
  // that wrapper is the object every one of those funcref values carries. If
  // a new way to introduce a funcref lands -- ref.func in code, call_ref --
  // it must be added here, or the index will have no wrapper and there will
  // be nothing to hand out but the internal closure. The safe fallback is to
  // put every function in the set.
  //
  // It no longer affects parameter typing: the J4 interim typed float params
  // of these functions `:any` and coerced them at entry, and that is gone now
  // that no route yields the closure (see createFunctions()).
  //
  // A funcref global cannot currently be EXPORTED at all: finalizeModule's
  // export loop has no case for a reference type and hits an llvm_unreachable
  // ("unsupported global export type"), which aborts hermesc rather than
  // diagnosing. Covering ref.func initializers here is still right -- the
  // value reaches the value stack through global.get regardless -- and the
  // abort is recorded as a separate defect rather than being described as a
  // rejection here.
  for (const auto &seg : moduleInfo_.elements)
    for (uint32_t fi : seg.funcIndices)
      escapableFuncs_.insert(fi);
  for (const auto &g : moduleInfo_.globals)
    if (g.initKind == WasmGlobal::InitKind::RefFunc)
      escapableFuncs_.insert(g.initValue.funcIndex);
}

void WasmIRGen::createFunctions() {
  // Build canonical type index map for structural type comparison.
  buildCanonicalTypeMap();

  // Determine which functions a funcref can name, before exportedFuncVars_ is
  // sized below: that set decides which indices get a canonical wrapper.
  computeEscapableFuncs();

  // Create the top-level function first (must be before other functions).
  auto *topLevel = builder_.createTopLevelFunction(
      "global", true /* strictMode */);
  topLevel->setReturnType(Type::createObject());
  topLevel->setExpectedParamCountIncludingThis(1); // just "this"

  // Create a VariableScope for the top-level function (no parent).
  topLevelVS_ = builder_.createVariableScope(nullptr);

  // If the module has memory, create Variables for the 8 typed array views.
  bool hasMemory = moduleInfo_.totalMemoryCount() > 0;
  if (hasMemory) {
    static const char *viewNames[NUM_MEM_VIEWS] = {
        "HEAP8",
        "HEAPU8",
        "HEAP16",
        "HEAPU16",
        "HEAP32",
        "HEAPU32",
        "HEAPF32",
        "HEAPF64",
    };
    for (uint8_t i = 0; i < NUM_MEM_VIEWS; ++i) {
      memViewVars_[i] = builder_.createVariable(
          topLevelVS_,
          viewNames[i],
          Type::createAnyType(),
          /* hidden */ true);
    }
  }

  // If the module has tables, create Variables for each table.
  // Each table gets two JS Arrays: one for function closures, one for type
  // indices (used by call_indirect for type checking).
  // One variable per tag, holding the object that identifies it.
  {
    uint32_t numTags = moduleInfo_.totalTagCount();
    tagVars_.resize(numTags, nullptr);
    for (uint32_t i = 0; i < numTags; ++i) {
      tagVars_[i] = builder_.createVariable(
          topLevelVS_,
          ("wasm_tag_" + llvh::Twine(i)),
          Type::createAnyType(),
          /* hidden */ true);
    }
  }

  // One variable per type, holding its interned id (see internTypeIds).
  typeIdVars_.resize(moduleInfo_.types.size(), nullptr);
  for (uint32_t i = 0; i < moduleInfo_.types.size(); ++i) {
    typeIdVars_[i] = builder_.createVariable(
        topLevelVS_,
        ("wasm_type_id_" + llvh::Twine(i)),
        Type::createAnyType(),
        /* hidden */ true);
  }

  uint32_t numTables = moduleInfo_.totalTableCount();
  if (numTables > 0) {
    tableFuncVars_.resize(numTables, nullptr);
    tableTypeVars_.resize(numTables, nullptr);
    tableExportVars_.resize(numTables, nullptr);
    // A funcref table is backed by a WebAssembly.Table object -- one the
    // module constructs for a defined table, or the one that satisfied an
    // import -- held here so createTables(), the import wiring and the table
    // export all publish and operate on the same one. Externref tables, which
    // the Table constructor does not build, leave their slot unset.
    tableObjVars_.resize(numTables, nullptr);
    for (uint32_t i = 0; i < numTables; ++i) {
      tableFuncVars_[i] = builder_.createVariable(
          topLevelVS_,
          ("table_" + llvh::Twine(i) + "_funcs"),
          Type::createAnyType(),
          /* hidden */ true);
      tableTypeVars_[i] = builder_.createVariable(
          topLevelVS_,
          ("table_" + llvh::Twine(i) + "_types"),
          Type::createAnyType(),
          /* hidden */ true);
      tableExportVars_[i] = builder_.createVariable(
          topLevelVS_,
          ("table_" + llvh::Twine(i) + "_exported"),
          Type::createAnyType(),
          /* hidden */ true);
      tableObjVars_[i] = builder_.createVariable(
          topLevelVS_,
          ("table_" + llvh::Twine(i) + "_obj"),
          Type::createAnyType(),
          /* hidden */ true);
    }
  }

  // Create Variables for Wasm globals in the top-level scope.
  // Each global gets one Variable (or two for i64: lo32, hi32).
  uint32_t numGlobals = moduleInfo_.totalGlobalCount();
  uint32_t numImportedGlobals = moduleInfo_.importedGlobalCount();
  if (numGlobals > 0) {
    globalSlotIndex_.resize(numGlobals);
    uint32_t slotIdx = 0;
    for (uint32_t i = 0; i < numGlobals; ++i) {
      globalSlotIndex_[i] = slotIdx;
      // Get the global's type.
      WasmValType gType;
      if (i < numImportedGlobals) {
        // Imported global: find the i-th global import.
        uint32_t importGlobalIdx = 0;
        for (const auto &imp : moduleInfo_.imports) {
          if (imp.kind != WasmExternalKind::Global)
            continue;
          if (importGlobalIdx == i) {
            gType = imp.globalType.type;
            // A mutable imported global is shared state: it is read and
            // written through the host's WebAssembly.Global, not through
            // the frame slot allocated below.
            if (imp.globalType.mutable_)
              importedMutableGlobals_.insert(i);
            break;
          }
          ++importGlobalIdx;
        }
      } else {
        // Defined global.
        gType = moduleInfo_.globals[i - numImportedGlobals].type.type;
      }

      globalVars_.push_back(builder_.createVariable(
          topLevelVS_,
          ("global_" + llvh::Twine(i)),
          Type::createAnyType(),
          /* hidden */ true));
      ++slotIdx;

      if (gType == WasmValType::I64) {
        globalVars_.push_back(builder_.createVariable(
            topLevelVS_,
            ("global_" + llvh::Twine(i) + "_hi"),
            Type::createAnyType(),
            /* hidden */ true));
        ++slotIdx;
      }
    }
  }

  // Create Variables for imported JS functions in the top-level scope.
  // These hold the JS callables looked up from the imports object.
  uint32_t numImportedFuncs = moduleInfo_.importedFunctionCount();
  importFuncVars_.resize(numImportedFuncs, nullptr);
  for (uint32_t i = 0; i < numImportedFuncs; ++i) {
    importFuncVars_[i] = builder_.createVariable(
        topLevelVS_,
        ("import_func_" + llvh::Twine(i)),
        Type::createAnyType(),
        /* hidden */ true);
  }

  // Create Variables for imported globals in the top-level scope. These hold
  // the import resolved during validation: its value for an immutable
  // import, the WebAssembly.Global object itself for a mutable one, which
  // global.get/global.set then read and write through.
  importGlobalVals_.resize(numImportedGlobals, nullptr);
  for (uint32_t i = 0; i < numImportedGlobals; ++i) {
    importGlobalVals_[i] = builder_.createVariable(
        topLevelVS_,
        ("import_global_val_" + llvh::Twine(i)),
        Type::createAnyType(),
        /* hidden */ true);
  }

  // If the module imports a memory, hold the memory's own maximum and its
  // buffer -- both read out of its internal fields by the one wasmLinkMemory
  // call that validated it, so nothing about the memory is asked twice.
  if (moduleInfo_.importedMemoryCount() > 0) {
    importedMemMaxVar_ = builder_.createVariable(
        topLevelVS_,
        "imported_mem_max",
        Type::createAnyType(),
        /* hidden */ true);
    importedMemBufVar_ = builder_.createVariable(
        topLevelVS_,
        "imported_mem_buf",
        Type::createAnyType(),
        /* hidden */ true);
  }

  // The linear memory is backed by a WebAssembly.Memory object -- constructed
  // here for a defined memory, supplied by the embedder for an imported one.
  // createMemoryViews(), the memory export and onMemoryGrow() all go through
  // this one object, so the module, the embedder and any importer are looking
  // at the same memory.
  if (moduleInfo_.totalMemoryCount() > 0) {
    memObjVar_ = builder_.createVariable(
        topLevelVS_,
        "mem_obj",
        Type::createAnyType(),
        /* hidden */ true);
  }

  // For each imported table, create a variable to hold that table's OWN
  // maximum, read from the table itself during validation. Like the memory
  // case above, the import declaration's limits are only a bound on what was
  // actually supplied, so table.grow must respect the supplied table's
  // maximum rather than the declaration's. The current size needs no
  // variable: it is the length of the storage, which is always to hand.
  {
    uint32_t numImportedTables = moduleInfo_.importedTableCount();
    importedTableMaxVars_.resize(numImportedTables, nullptr);
    for (uint32_t i = 0; i < numImportedTables; ++i) {
      importedTableMaxVars_[i] = builder_.createVariable(
          topLevelVS_,
          ("imported_table_max_" + llvh::Twine(i)),
          Type::createAnyType(),
          /* hidden */ true);
    }
  }

  // Determine the return buffer size. The buffer is used for i64 arithmetic
  // builtins (which write lo/hi to retBufI[0]/[1]) and multi-value returns.
  // We always create at least an 8-byte buffer because function bodies may
  // use i64 operations even if no function type signature mentions i64.
  {
    uint32_t maxRetBufSize = 8; // Minimum for i64 arithmetic builtins.
    for (const auto &ft : moduleInfo_.types) {
      if (needsReturnBuffer(ft)) {
        auto [offsets, size] = computeRetBufLayout(ft.results);
        maxRetBufSize = std::max(maxRetBufSize, size);
        // A reference result cannot be stored in the ArrayBuffer views, so
        // such a module also needs the parallel reference array. Gate it:
        // creating the array unconditionally would add an allocation to every
        // module that merely does i64 arithmetic, and would churn the golden
        // IR of every Wasm test. V128 is deliberately excluded -- it stays
        // unsupported and keeps its diagnostic.
        for (auto vt : ft.results) {
          if (vt == WasmValType::FuncRef || vt == WasmValType::ExternRef)
            retBufHasRefResult_ = true;
        }
      }
    }

    // Round up to a multiple of 8 so Float64Array can be created on the
    // same ArrayBuffer.
    maxRetBufSize = (maxRetBufSize + 7) & ~7u;

    retBufIVar_ = builder_.createVariable(
        topLevelVS_,
        "retBufI",
        Type::createAnyType(),
        /* hidden */ true);
    retBufFVar_ = builder_.createVariable(
        topLevelVS_,
        "retBufF",
        Type::createAnyType(),
        /* hidden */ true);
    if (retBufHasRefResult_) {
      retBufRVar_ = builder_.createVariable(
          topLevelVS_,
          "retBufR",
          Type::createAnyType(),
          /* hidden */ true);
    }
    retBufSize_ = maxRetBufSize;
  }

  // Create all Wasm functions and a Variable in the top-level scope for each.
  uint32_t totalFuncs = moduleInfo_.totalFunctionCount();
  irFunctions_.resize(totalFuncs, nullptr);
  closureVars_.resize(totalFuncs, nullptr);
  exportedFuncVars_.resize(totalFuncs, nullptr);

  // Which indices get a canonical Exported Function. An index needs one
  // exactly when its function can reach script: through an export name,
  // through an import (whose wrapper wraps the trampoline, so a JS function
  // placed in a table is reached the same way as a native one), or through
  // escapableFuncs_ -- element segments and ref.func global initializers.
  llvh::DenseSet<uint32_t> wrapped = escapableFuncs_;
  for (uint32_t i = 0; i < moduleInfo_.importedFunctionCount(); ++i)
    wrapped.insert(i);
  for (const auto &exp : moduleInfo_.exports)
    if (exp.kind == WasmExternalKind::Function && exp.index < totalFuncs)
      wrapped.insert(exp.index);

  for (uint32_t i = 0; i < totalFuncs; ++i) {
    const WasmFuncType &funcType = moduleInfo_.getFunctionType(i);

    // Derive a name for the function.
    std::string name;
    if (i < moduleInfo_.names.functionNames.size() &&
        !moduleInfo_.names.functionNames[i].empty()) {
      name = moduleInfo_.names.functionNames[i];
    } else {
      name = ("wasm_func_" + llvh::Twine(i)).str();
    }

    // Create the IR function.
    auto *func = builder_.createFunction(
        name,
        Function::DefinitionKind::ES5Function,
        true /* strictMode */);

    // Set the return type based on the Wasm function type.
    if (funcType.results.empty())
      func->setReturnType(Type::createUndefined());
    else if (!needsReturnBuffer(funcType))
      func->setReturnType(wasmValTypeToIRType(funcType.results[0]));
    else
      func->setReturnType(Type::createNumber());

    // Create a variable in the top-level scope to hold the pre-created closure.
    closureVars_[i] = builder_.createVariable(
        topLevelVS_,
        ("closure_" + llvh::Twine(i)),
        Type::createAnyType(),
        /* hidden */ true);

    // And, where the function can reach script at all, a variable for its
    // single canonical Exported Function (filled in by finalizeModule).
    if (wrapped.count(i)) {
      exportedFuncVars_[i] = builder_.createVariable(
          topLevelVS_,
          ("exported_func_" + llvh::Twine(i)),
          Type::createAnyType(),
          /* hidden */ true);
    }

    // Add a "this" parameter (required by Hermes calling convention).
    builder_.createJSThisParam(func);

    // For functions that need a return buffer (i64 or multi-value returns),
    // prepend retBufI and retBufF parameters before the Wasm params.
    uint32_t jsParamCount = 0;
    if (needsReturnBuffer(funcType)) {
      auto *rbI = builder_.createJSDynamicParam(func, "retbuf_I");
      rbI->setType(Type::createObject());
      auto *rbF = builder_.createJSDynamicParam(func, "retbuf_F");
      rbF->setType(Type::createObject());
      jsParamCount += 2;
    }

    // Add JSDynamicParams per Wasm parameter. i64 params need two slots
    // (lo32, hi32) for the split representation.
    for (uint32_t p = 0; p < funcType.params.size(); ++p) {
      if (funcType.params[p] == WasmValType::I64) {
        auto *lo = builder_.createJSDynamicParam(
            func, ("p" + llvh::Twine(p) + "_lo").str());
        lo->setType(Type::createNumber());
        auto *hi = builder_.createJSDynamicParam(
            func, ("p" + llvh::Twine(p) + "_hi").str());
        hi->setType(Type::createNumber());
        jsParamCount += 2;
      } else {
        auto *param = builder_.createJSDynamicParam(
            func, ("p" + llvh::Twine(p)).str());
        // Every parameter carries its Wasm type, f32/f64 included. That is an
        // assertion the backend trusts -- FBinaryMathInst reads the raw double
        // bits, so a non-number reaching getDouble() is a Debug assert and a
        // Release segfault -- and it is honest exactly as long as EVERY caller
        // of this closure is Wasm.
        //
        // It is. The closure lives in a hidden top-level Variable and is
        // loaded only by a direct call, a call_indirect through the table's
        // closure array, the start-function call, and its own export wrapper.
        // Nothing script can name holds it: a table slot, a funcref result, a
        // funcref global, an argument to an import trampoline and an exception
        // payload all carry the canonical Exported Function instead, and the
        // `__wasm_funcs__`/`__wasm_types__`/`__wasm_exported__` publications
        // that used to hand the array out are gone.
        //
        // That is finding J4, and the claim is not asserted here: it is
        // enumerated and executed by test/wasm/e2e-no-closure-escape.wat,
        // which walks all twenty routes and brand-checks what each one yields.
        // The interim fix -- typing float params of "escapable" functions
        // `:any` and coercing at function entry -- is gone with the routes it
        // defended against. ANY NEW ROUTE OUT (ref.func in a function body,
        // call_ref, a funcref global export) must be added to that test in the
        // same change that adds it, or this annotation becomes a lie again.
        //
        // The JS->Wasm coercion itself did not disappear, it moved to where it
        // belongs: createExportWrapper does ToNumber (plus fround for f32) on
        // the wrapper's parameters, which is the actual boundary.
        param->setType(wasmValTypeToIRType(funcType.params[p]));
        jsParamCount += 1;
      }
    }

    // Set the expected param count (including "this").
    func->setExpectedParamCountIncludingThis(jsParamCount + 1);

    // Create a single entry basic block with ReturnInst(undefined).
    auto *entry = builder_.createBasicBlock(func);
    builder_.setInsertionBlock(entry);
    builder_.createReturnInst(builder_.getLiteralUndefined());

    irFunctions_[i] = func;
  }

  // Create the __wasm_instantiate__ function. This will contain all the
  // initialization logic (import resolution, closures, memory views, tables,
  // globals, trampolines). The top-level function will just return a module
  // info object with an instantiate closure and descriptor arrays.
  instantiateFunc_ = builder_.createFunction(
      "__wasm_instantiate__",
      Function::DefinitionKind::ES5Function,
      true /* strictMode */);
  instantiateFunc_->setReturnType(Type::createObject());
  // "this" plus the import object.
  instantiateFunc_->setExpectedParamCountIncludingThis(2);
  builder_.createJSThisParam(instantiateFunc_);
  importsParam_ = builder_.createJSDynamicParam(instantiateFunc_, "imports");

  // Populate the __wasm_instantiate__ function body.
  // Create all closures once and store them in the top-level scope.
  tlEntry_ = builder_.createBasicBlock(instantiateFunc_);
  builder_.setInsertionBlock(tlEntry_);

  // Create a scope for the top-level function.
  tlScope_ = builder_.createCreateScopeInst(
      topLevelVS_, builder_.getEmptySentinel());
  auto *tlScope = tlScope_;

  // Resolve and validate ALL imports from the imports object.
  // The imports object arrives as instantiate()'s parameter.
  // It has the shape: { moduleName: { fieldName: value } }.
  // Each import is validated:
  //   - Module object must not be undefined.
  //   - Import value must not be undefined.
  //   - The value must match the declaration, by a check that differs per
  //     kind:
  //       * table, memory and global imports must be a GENUINE
  //         WebAssembly.Table/Memory/Global, established by the dyn_vmcast
  //         inside wasmLinkTable/wasmLinkMemory/wasmLinkGlobal. No property
  //         on the supplied object takes part.
  //       * function and tag imports still compare a __wasm_type__ string
  //         carried on the value, when it has one; a plain JS callable with
  //         no __wasm_type__ satisfies a function import.
  //       * an IMMUTABLE global import also accepts a raw JS number or
  //         BigInt, per spec.
  if (!moduleInfo_.imports.empty()) {
    auto *importsVal = builder_.createLoadParamInst(importsParam_);
    auto *undefinedVal = builder_.getLiteralUndefined();
    auto *topLevelFunc = tlEntry_->getParent();

    uint32_t importFuncIdx = 0;
    uint32_t importGlobalIdx = 0;
    uint32_t importTableIdx = 0;
    uint32_t importTagIdx = 0;

    // Cache module objects to avoid redundant loads for the same module.
    std::string lastModuleName;
    Value *lastModuleObj = nullptr;

    for (const auto &imp : moduleInfo_.imports) {
      // Load module object (deduplicate consecutive same-module loads).
      Value *moduleObj;
      if (imp.moduleName == lastModuleName && lastModuleObj) {
        moduleObj = lastModuleObj;
      } else {
        moduleObj = builder_.createLoadPropertyInst(
            importsVal, builder_.getLiteralString(imp.moduleName));

        // Check module object is not undefined.
        auto *modIsUndef = builder_.createBinaryOperatorInst(
            moduleObj, undefinedVal,
            ValueKind::BinaryStrictlyEqualInstKind);
        auto *modFailBB = builder_.createBasicBlock(topLevelFunc);
        auto *modOkBB = builder_.createBasicBlock(topLevelFunc);
        builder_.createCondBranchInst(modIsUndef, modFailBB, modOkBB);

        builder_.setInsertionBlock(modFailBB);
        helpers_.emitLinkError(builder_.getLiteralString(
            "module has no import namespace " + imp.moduleName));
        builder_.createUnreachableInst();

        builder_.setInsertionBlock(modOkBB);
        tlEntry_ = modOkBB;

        lastModuleName = imp.moduleName;
        lastModuleObj = moduleObj;
      }

      // Load the import value.
      auto *importVal = builder_.createLoadPropertyInst(
          moduleObj, builder_.getLiteralString(imp.fieldName));

      // Check import value is not undefined.
      auto *impIsUndef = builder_.createBinaryOperatorInst(
          importVal, undefinedVal,
          ValueKind::BinaryStrictlyEqualInstKind);
      auto *impFailBB = builder_.createBasicBlock(topLevelFunc);
      auto *impOkBB = builder_.createBasicBlock(topLevelFunc);
      builder_.createCondBranchInst(impIsUndef, impFailBB, impOkBB);

      builder_.setInsertionBlock(impFailBB);
      helpers_.emitLinkError(builder_.getLiteralString(
          "module has no import " + imp.moduleName + "." + imp.fieldName));
      builder_.createUnreachableInst();

      builder_.setInsertionBlock(impOkBB);
      tlEntry_ = impOkBB;

      // Per-kind type validation.
      switch (imp.kind) {
        case WasmExternalKind::Function: {
          // Load __wasm_type__ from the import value.
          auto *typeStr = builder_.createLoadPropertyInst(
              importVal, builder_.getLiteralString("__wasm_type__"));
          auto *typeIsUndef = builder_.createBinaryOperatorInst(
              typeStr, undefinedVal,
              ValueKind::BinaryStrictlyEqualInstKind);
          // If undefined → could be plain JS function. Check typeof.
          auto *checkCallableBB =
              builder_.createBasicBlock(topLevelFunc);
          auto *checkTypeBB = builder_.createBasicBlock(topLevelFunc);
          auto *acceptBB = builder_.createBasicBlock(topLevelFunc);
          auto *linkErrorBB = builder_.createBasicBlock(topLevelFunc);
          builder_.createCondBranchInst(
              typeIsUndef, checkCallableBB, checkTypeBB);

          // Check that the import value is callable (typeof === "function").
          // This runs on both paths: carrying a matching __wasm_type__ says
          // what the value claims to be, not that it can be called, and a
          // non-callable used to link happily and fail as a TypeError at the
          // first call instead of a LinkError at instantiation.
          builder_.setInsertionBlock(checkCallableBB);
          auto *typeofVal = builder_.createTypeOfInst(importVal);
          auto *isFunc = builder_.createBinaryOperatorInst(
              typeofVal,
              builder_.getLiteralString("function"),
              ValueKind::BinaryStrictlyEqualInstKind);
          builder_.createCondBranchInst(
              isFunc, acceptBB, linkErrorBB);

          builder_.setInsertionBlock(checkTypeBB);
          // Compare type string against expected.
          std::string expectedType =
              buildFuncTypeString(moduleInfo_.getFunctionType(
                  importFuncIdx));
          auto *mismatch = builder_.createBinaryOperatorInst(
              typeStr,
              builder_.getLiteralString(expectedType),
              ValueKind::BinaryStrictlyNotEqualInstKind);
          auto *typedCallableBB =
              builder_.createBasicBlock(topLevelFunc);
          builder_.createCondBranchInst(
              mismatch, linkErrorBB, typedCallableBB);

          builder_.setInsertionBlock(typedCallableBB);
          auto *typedIsFunc = builder_.createBinaryOperatorInst(
              builder_.createTypeOfInst(importVal),
              builder_.getLiteralString("function"),
              ValueKind::BinaryStrictlyEqualInstKind);
          builder_.createCondBranchInst(
              typedIsFunc, acceptBB, linkErrorBB);

          builder_.setInsertionBlock(linkErrorBB);
          helpers_.emitLinkError(builder_.getLiteralString(
              "import " + imp.moduleName + "." + imp.fieldName +
              " is not a function"));
          builder_.createUnreachableInst();

          builder_.setInsertionBlock(acceptBB);
          tlEntry_ = acceptBB;

          // Store the validated import function.
          builder_.createStoreFrameInst(
              tlScope, importVal, importFuncVars_[importFuncIdx]);
          ++importFuncIdx;
          break;
        }

        case WasmExternalKind::Global: {
          // A global import is satisfied either by a GENUINE
          // WebAssembly.Global of the declared type and mutability, or -- for
          // an immutable import only -- by a raw JS value. There is no
          // __wasm_type__ read here any more: the type used to be an ordinary
          // string property, which made a global the one kind where a plain
          // object literal linked outright and handed the module its own
          // `value`. wasmLinkGlobal brand-checks with dyn_vmcast instead, and
          // returns the value from the internal field rather than through the
          // replaceable `.value` accessor.
          //
          // What a raw value may be is decided per import at compile time,
          // per spec: a raw value allocates an *immutable* global, so it can
          // never satisfy a mutable import; an i64 import takes a BigInt, and
          // every other type takes a Number. Accepting either typeof for
          // every type would let a BigInt satisfy an i32 import and a Number
          // an i64 one.
          const bool isI64 = imp.globalType.type == WasmValType::I64;
          const bool rawAllowed = !imp.globalType.mutable_;

          auto *linked = helpers_.emitLinkGlobal(
              importVal,
              builder_.getLiteralNumber(
                  static_cast<double>(globalValTypeCode(imp.globalType.type))),
              builder_.getLiteralBool(imp.globalType.mutable_));
          // null means "not a WebAssembly.Global"; undefined means "a
          // WebAssembly.Global that does not match". They must stay apart:
          // only the first can legitimately be a raw JS value, and reporting
          // the second as "not a WebAssembly.Global" would be false.
          auto *notAGlobal = builder_.createBinaryOperatorInst(
              linked,
              builder_.getLiteralNull(),
              ValueKind::BinaryStrictlyEqualInstKind);

          auto *checkMatchBB = builder_.createBasicBlock(topLevelFunc);
          auto *acceptBB = builder_.createBasicBlock(topLevelFunc);
          auto *linkErrorBB = builder_.createBasicBlock(topLevelFunc);
          auto *rawErrorBB = builder_.createBasicBlock(topLevelFunc);
          BasicBlock *checkRawBB = nullptr;
          if (rawAllowed) {
            checkRawBB = builder_.createBasicBlock(topLevelFunc);
            builder_.createCondBranchInst(
                notAGlobal, checkRawBB, checkMatchBB);

            builder_.setInsertionBlock(checkRawBB);
            auto *typeofVal = builder_.createTypeOfInst(importVal);
            auto *rawOk = builder_.createBinaryOperatorInst(
                typeofVal,
                builder_.getLiteralString(isI64 ? "bigint" : "number"),
                ValueKind::BinaryStrictlyEqualInstKind);
            builder_.createCondBranchInst(
                rawOk, acceptBB, rawErrorBB);
          } else {
            builder_.createCondBranchInst(
                notAGlobal, rawErrorBB, checkMatchBB);
          }

          builder_.setInsertionBlock(checkMatchBB);
          auto *mismatch = builder_.createBinaryOperatorInst(
              linked, undefinedVal, ValueKind::BinaryStrictlyEqualInstKind);
          builder_.createCondBranchInst(mismatch, linkErrorBB, acceptBB);

          // For an immutable import the VALUE is what is kept, and
          // wasmLinkGlobal already read it out of the internal field. For a
          // mutable one the OBJECT is, because the module and the host share
          // the global and each must see the other's writes.
          Value *globalObjValue = imp.globalType.mutable_
              ? static_cast<Value *>(importVal)
              : static_cast<Value *>(linked);

          builder_.setInsertionBlock(linkErrorBB);
          helpers_.emitLinkError(builder_.getLiteralString(
              "import " + imp.moduleName + "." + imp.fieldName +
              " is a WebAssembly.Global that does not match the declared " +
              (imp.globalType.mutable_ ? "mutable " : "immutable ") +
              valTypeName(imp.globalType.type) + " global import"));
          builder_.createUnreachableInst();

          builder_.setInsertionBlock(rawErrorBB);
          {
            std::string rawErrMsg =
                "import " + imp.moduleName + "." + imp.fieldName;
            if (!rawAllowed)
              rawErrMsg +=
                  " must be a WebAssembly.Global to satisfy a mutable"
                  " global import";
            else if (isI64)
              rawErrMsg += " must be a BigInt to satisfy an i64 global"
                           " import";
            else
              rawErrMsg += " must be a Number to satisfy an " +
                  std::string(valTypeName(imp.globalType.type)) +
                  " global import";
            helpers_.emitLinkError(builder_.getLiteralString(rawErrMsg));
            builder_.createUnreachableInst();
          }

          builder_.setInsertionBlock(acceptBB);
          tlEntry_ = acceptBB;

          // Resolve the import HERE, under the check that was just performed,
          // and store the result. Deciding again later by asking the object
          // a second time would let a getter or Proxy answer differently and
          // send a WebAssembly.Global down the raw-value path, storing the
          // object itself into an i32 slot. Nothing about this value is read
          // off the import object again.
          auto *resolved = builder_.createPhiInst();
          if (checkRawBB)
            resolved->addEntry(importVal, checkRawBB);
          resolved->addEntry(globalObjValue, checkMatchBB);

          // Store the resolved import into importGlobalVals_ -- its value
          // for an immutable import, the WebAssembly.Global object itself
          // for a mutable one. We use a new Variable per imported global to
          // pass it to initializeGlobals() and, for a mutable import, to
          // every global.get/global.set of it.
          if (importGlobalIdx < importGlobalVals_.size()) {
            builder_.createStoreFrameInst(
                tlScope, resolved,
                importGlobalVals_[importGlobalIdx]);
          }
          ++importGlobalIdx;
          break;
        }

        case WasmExternalKind::Table: {
          // A table import is satisfied by a GENUINE WebAssembly.Table and
          // nothing else. The brand check inside wasmLinkTable is a
          // dyn_vmcast, so no object literal, no forged prototype and no
          // Proxy can pass it -- which is what retires the old validation:
          // reading a __wasm_type__ string and three arrays off an ordinary
          // object let script choose the module's table storage outright.
          //
          // There is no __wasm_type__ read here at all. The brand subsumes
          // the "table:funcref" half of it, and the element type is decided
          // by `declaredIsFuncRef` below rather than by a string on the
          // object, so an externref DECLARATION can no longer be paired with
          // a funcref table's storage.
          auto *linkErrorBB = builder_.createBasicBlock(topLevelFunc);
          // A table that really is a Table but does not satisfy the declared
          // limits gets its own message; saying it "is not a
          // WebAssembly.Table" would be false. (Mirrors the memory case.)
          auto *limitsErrorBB = builder_.createBasicBlock(topLevelFunc);
          auto *checkLimitsBB = builder_.createBasicBlock(topLevelFunc);
          auto *acceptBB = builder_.createBasicBlock(topLevelFunc);

          auto *linked = helpers_.emitLinkTable(
              importVal,
              builder_.getLiteralBool(
                  imp.tableType.elemType == WasmValType::FuncRef));
          auto *linkFailed = builder_.createBinaryOperatorInst(
              linked,
              builder_.getLiteralNull(),
              ValueKind::BinaryStrictlyEqualInstKind);
          builder_.createCondBranchInst(
              linkFailed, linkErrorBB, checkLimitsBB);

          builder_.setInsertionBlock(checkLimitsBB);
          // [funcs, types, exported, max]. These are the table's internal
          // fields, not copies: an importer and an exporter of one table
          // write the same slots.
          auto *funcsResult = builder_.createLoadPropertyInst(
              linked, builder_.getLiteralNumber(0));
          auto *typesResult = builder_.createLoadPropertyInst(
              linked, builder_.getLiteralNumber(1));
          auto *exportedResult = builder_.createLoadPropertyInst(
              linked, builder_.getLiteralNumber(2));
          // The table's OWN maximum, which table.grow must respect: the
          // declaration below is only an upper bound on it.
          auto *actualMax = builder_.createLoadPropertyInst(
              linked, builder_.getLiteralNumber(3));
          // The current size, which reflects every grow so far. Reading it
          // from the storage rather than from a recorded snapshot is what
          // keeps it honest.
          auto *actualMin = builder_.createLoadPropertyInst(
              funcsResult, builder_.getLiteralString("length"));
          auto *minOk = builder_.createBinaryOperatorInst(
              actualMin,
              builder_.getLiteralNumber(
                  static_cast<double>(imp.tableType.limits.initial)),
              ValueKind::BinaryGreaterThanOrEqualInstKind);

          if (imp.tableType.limits.hasMaximum) {
            auto *checkMaxBB =
                builder_.createBasicBlock(topLevelFunc);
            builder_.createCondBranchInst(
                minOk, checkMaxBB, limitsErrorBB);

            // If import requires max, actual must also have max.
            builder_.setInsertionBlock(checkMaxBB);
            auto *hasNoMax = builder_.createBinaryOperatorInst(
                actualMax,
                builder_.getLiteralNumber(-1),
                ValueKind::BinaryStrictlyEqualInstKind);
            auto *checkMaxValBB =
                builder_.createBasicBlock(topLevelFunc);
            builder_.createCondBranchInst(
                hasNoMax, limitsErrorBB, checkMaxValBB);

            builder_.setInsertionBlock(checkMaxValBB);
            auto *maxOk = builder_.createBinaryOperatorInst(
                actualMax,
                builder_.getLiteralNumber(
                    static_cast<double>(
                        imp.tableType.limits.maximum)),
                ValueKind::BinaryLessThanOrEqualInstKind);
            builder_.createCondBranchInst(
                maxOk, acceptBB, limitsErrorBB);
          } else {
            builder_.createCondBranchInst(
                minOk, acceptBB, limitsErrorBB);
          }

          builder_.setInsertionBlock(linkErrorBB);
          // Two ways to arrive here, and they must not share a message: a
          // value that is not a table, and a declaration that no table can
          // satisfy. Saying "is not a WebAssembly.Table" of a genuine table
          // supplied to an externref-declaring module is the same false-message
          // class as the limits case below.
          helpers_.emitLinkError(builder_.getLiteralString(
              imp.tableType.elemType == WasmValType::FuncRef
                  ? "import " + imp.moduleName + "." + imp.fieldName +
                      " is not a WebAssembly.Table"
                  : "import " + imp.moduleName + "." + imp.fieldName +
                      " declares a non-funcref table, which nothing can "
                      "satisfy: WebAssembly.Table builds only funcref tables"));
          builder_.createUnreachableInst();

          builder_.setInsertionBlock(limitsErrorBB);
          helpers_.emitLinkError(builder_.getLiteralString(
              "import " + imp.moduleName + "." + imp.fieldName +
              " is a WebAssembly.Table that does not satisfy the declared "
              "limits"));
          builder_.createUnreachableInst();

          builder_.setInsertionBlock(acceptBB);

          // Record the table's own maximum, which table.grow reads, and the
          // table object itself, which a re-export publishes. Re-exporting
          // the very object that was imported is what keeps the storage
          // shared: constructing a fresh WebAssembly.Table around the same
          // arrays is no longer possible, and would be wrong anyway -- the
          // spec's export of an imported table is the same table.
          builder_.createStoreFrameInst(
              tlScope, actualMax, importedTableMaxVars_[importTableIdx]);
          builder_.createStoreFrameInst(
              tlScope, importVal, tableObjVars_[importTableIdx]);

          builder_.createStoreFrameInst(
              tlScope, funcsResult, tableFuncVars_[importTableIdx]);
          builder_.createStoreFrameInst(
              tlScope, typesResult, tableTypeVars_[importTableIdx]);
          builder_.createStoreFrameInst(
              tlScope, exportedResult, tableExportVars_[importTableIdx]);

          // No wasmCheckTableArrays call: these came out of a table this
          // engine built, so they are JSArrays by construction. The check
          // remains for externref tables, whose arrays come from
          // globalThis.Array.

          ++importTableIdx;
          tlEntry_ = acceptBB;
          break;
        }

        case WasmExternalKind::Memory: {
          // A memory import is satisfied by a GENUINE WebAssembly.Memory and
          // nothing else. The brand check inside wasmLinkMemory is a
          // dyn_vmcast, so no object literal, no forged prototype and no Proxy
          // can pass it. That replaces both halves of the old validation: a
          // __wasm_type__ string compare and an `instanceof`, neither of which
          // an object INHERITING from a real Memory failed -- such an object
          // used to reach the buffer read and die there as a TypeError from
          // inside generated code, naming nothing.
          auto *linkErrorBB = builder_.createBasicBlock(topLevelFunc);
          // A memory that really is a Memory but does not satisfy the
          // declared limits needs its own message: saying it "is not a
          // WebAssembly.Memory" would be false and would send whoever reads
          // it looking in the wrong place.
          auto *limitsErrorBB = builder_.createBasicBlock(topLevelFunc);
          auto *acceptBB = builder_.createBasicBlock(topLevelFunc);
          auto *checkLimitsBB = builder_.createBasicBlock(topLevelFunc);

          auto *linked = helpers_.emitLinkMemory(importVal);
          auto *linkFailed = builder_.createBinaryOperatorInst(
              linked,
              builder_.getLiteralNull(),
              ValueKind::BinaryStrictlyEqualInstKind);
          builder_.createCondBranchInst(
              linkFailed, linkErrorBB, checkLimitsBB);

          // Check limits: actualMin >= expectedMin
          builder_.setInsertionBlock(checkLimitsBB);
          // [currentPages, max, buffer], all read out of the memory's own
          // internal fields by the builtin. Three things follow, and each
          // used to be a hazard:
          //   * the size is the buffer's, measured now, so a memory grown
          //     since it was built satisfies the declaration it now meets.
          //     The old __wasm_min__ was a snapshot the constructor wrote and
          //     grow never updated (H7).
          //   * these are values, not property reads, so nothing can answer
          //     differently on a second look. __wasm_max__ used to be read
          //     once to validate and again to store, and a getter between the
          //     two could raise the ceiling memory.grow enforces.
          //   * the maximum is a number by construction, so it needs no
          //     AsNumberInst before reaching wasmMemoryGrow's getNumber().
          // This block dominates acceptBB on both the hasMaximum and the
          // no-maximum path, so all three values are live at the stores below.
          auto *actualMin = builder_.createLoadPropertyInst(
              linked, builder_.getLiteralNumber(0));
          auto *actualMax = builder_.createLoadPropertyInst(
              linked, builder_.getLiteralNumber(1));
          auto *actualBuf = builder_.createLoadPropertyInst(
              linked, builder_.getLiteralNumber(2));
          auto *minOk = builder_.createBinaryOperatorInst(
              actualMin,
              builder_.getLiteralNumber(
                  static_cast<double>(imp.memoryType.limits.initial)),
              ValueKind::BinaryGreaterThanOrEqualInstKind);

          if (imp.memoryType.limits.hasMaximum) {
            auto *checkMaxBB =
                builder_.createBasicBlock(topLevelFunc);
            builder_.createCondBranchInst(
                minOk, checkMaxBB, limitsErrorBB);

            builder_.setInsertionBlock(checkMaxBB);
            auto *hasNoMax = builder_.createBinaryOperatorInst(
                actualMax,
                builder_.getLiteralNumber(-1),
                ValueKind::BinaryStrictlyEqualInstKind);
            auto *checkMaxValBB =
                builder_.createBasicBlock(topLevelFunc);
            builder_.createCondBranchInst(
                hasNoMax, limitsErrorBB, checkMaxValBB);

            builder_.setInsertionBlock(checkMaxValBB);
            auto *maxOk = builder_.createBinaryOperatorInst(
                actualMax,
                builder_.getLiteralNumber(
                    static_cast<double>(
                        imp.memoryType.limits.maximum)),
                ValueKind::BinaryLessThanOrEqualInstKind);
            builder_.createCondBranchInst(
                maxOk, acceptBB, limitsErrorBB);
          } else {
            builder_.createCondBranchInst(
                minOk, acceptBB, limitsErrorBB);
          }

          builder_.setInsertionBlock(linkErrorBB);
          helpers_.emitLinkError(builder_.getLiteralString(
              "import " + imp.moduleName + "." + imp.fieldName +
              " is not a WebAssembly.Memory"));
          builder_.createUnreachableInst();

          builder_.setInsertionBlock(limitsErrorBB);
          helpers_.emitLinkError(builder_.getLiteralString(
              "import " + imp.moduleName + "." + imp.fieldName +
              " does not satisfy the declared memory limits"));
          builder_.createUnreachableInst();

          builder_.setInsertionBlock(acceptBB);
          // Record the imported Memory itself. The module's views are built
          // over its buffer, so the module operates on the embedder's memory
          // rather than on a private copy sized from advertised metadata.
          builder_.createStoreFrameInst(tlScope, importVal, memObjVar_);
          // The same values that were validated above, not fresh reads. The
          // buffer in particular: reading `.buffer` again in
          // createMemoryViews() would go through a prototype accessor that
          // script can replace, so the views could end up over a different
          // buffer from the one whose size just satisfied the declaration.
          builder_.createStoreFrameInst(
              tlScope, actualMax, importedMemMaxVar_);
          assert(
              importedMemBufVar_ &&
              "a memory import must have allocated importedMemBufVar_");
          builder_.createStoreFrameInst(
              tlScope, actualBuf, importedMemBufVar_);
          tlEntry_ = acceptBB;
          break;
        }

        case WasmExternalKind::Tag: {
          // Load __wasm_type__ from the import value.
          auto *typeStr = builder_.createLoadPropertyInst(
              importVal, builder_.getLiteralString("__wasm_type__"));
          auto *typeIsUndef = builder_.createBinaryOperatorInst(
              typeStr, undefinedVal,
              ValueKind::BinaryStrictlyEqualInstKind);
          auto *acceptBB = builder_.createBasicBlock(topLevelFunc);
          auto *checkTypeBB = builder_.createBasicBlock(topLevelFunc);
          auto *linkErrorBB = builder_.createBasicBlock(topLevelFunc);
          // If __wasm_type__ is undefined, accept (raw JS value as tag).
          builder_.createCondBranchInst(
              typeIsUndef, acceptBB, checkTypeBB);

          builder_.setInsertionBlock(checkTypeBB);
          const WasmFuncType &tagFuncType =
              moduleInfo_.types[imp.tagTypeIndex];
          std::string expectedType = buildTagTypeString(tagFuncType);
          auto *mismatch = builder_.createBinaryOperatorInst(
              typeStr,
              builder_.getLiteralString(expectedType),
              ValueKind::BinaryStrictlyNotEqualInstKind);
          builder_.createCondBranchInst(
              mismatch, linkErrorBB, acceptBB);

          builder_.setInsertionBlock(linkErrorBB);
          helpers_.emitLinkError(builder_.getLiteralString(
              "import " + imp.moduleName + "." + imp.fieldName +
              " is not a valid tag import"));
          builder_.createUnreachableInst();

          builder_.setInsertionBlock(acceptBB);
          tlEntry_ = acceptBB;
          // Store the imported tag object. Every other import kind stores its
          // validated value; tags did not, so the object was validated and
          // then discarded, leaving throw/catch nothing to compare identity
          // against.
          if (importTagIdx < tagVars_.size())
            builder_.createStoreFrameInst(
                tlScope, importVal, tagVars_[importTagIdx]);
          ++importTagIdx;
          break;
        }
      }
    }
  }

  // Pre-create closures for all Wasm functions and store in the environment.
  for (uint32_t i = 0; i < totalFuncs; ++i) {
    auto *closure = builder_.createCreateFunctionInst(
        tlScope, irFunctions_[i]);
    builder_.createStoreFrameInst(tlScope, closure, closureVars_[i]);
  }

  // Create typed array views for the linear memory if present.
  if (hasMemory) {
    createMemoryViews(tlScope);
  }

  // Create the per-module return buffer if needed.
  //
  // REENTRANCY INVARIANT: there is exactly one return buffer per module
  // instance, shared by every function that returns an i64 or a multi-value
  // result. A function marshals its results into the buffer and its caller
  // reads them straight back out, so the buffer must not be written again
  // between those two points. Any operation that could re-enter Wasm --
  // calling back into an export, or running arbitrary JS such as a property
  // getter, valueOf, or a Proxy trap -- while a result sits unread in the
  // buffer will overwrite it. The marshalling code therefore computes every
  // result into an SSA value first and only then stores them (see
  // emitRetBufLoads / the multi-value trampoline), so no user code runs
  // between the write and the read.
  //
  // The buffer is built from globalThis.ArrayBuffer / Uint32Array /
  // Float64Array, which a script can replace, so the native builtins that
  // read it (writeI64ToRetBuf and friends) treat arg0 as untrusted and
  // reject a non-typed-array rather than casting it blindly.
  if (retBufSize_ > 0) {
    auto *ArrayBufferCtor =
        builder_.createTryLoadGlobalPropertyInst("ArrayBuffer");
    auto *Uint32ArrayCtor =
        builder_.createTryLoadGlobalPropertyInst("Uint32Array");
    auto *Float64ArrayCtor =
        builder_.createTryLoadGlobalPropertyInst("Float64Array");
    auto *buf = emitNew(
        ArrayBufferCtor,
        {builder_.getLiteralNumber(static_cast<double>(retBufSize_))});
    auto *retBufI = emitNew(Uint32ArrayCtor, {buf});
    auto *retBufF = emitNew(Float64ArrayCtor, {buf});
    builder_.createStoreFrameInst(tlScope, retBufI, retBufIVar_);
    builder_.createStoreFrameInst(tlScope, retBufF, retBufFVar_);
    if (retBufRVar_) {
      // Parallel reference slots, indexed like the Uint32Array view. This
      // array holds the last reference written to each slot until it is
      // overwritten -- bounded by retBufSize_/4 entries per instance, so it
      // retains a little longer than strictly necessary but does not grow.
      auto *ArrayCtor = builder_.createTryLoadGlobalPropertyInst("Array");
      auto *retBufR = emitNew(
          ArrayCtor,
          {builder_.getLiteralNumber(static_cast<double>(retBufSize_ / 4))});
      builder_.createStoreFrameInst(tlScope, retBufR, retBufRVar_);
    }
  }

  // Tag identity is needed by throw/catch whether or not the module has any
  // tables, so this is not gated on numTables.
  createTagObjects(tlScope);

  // Interned type ids are consumed by element segments and call_indirect,
  // both of which require a table, and by the canonical Exported Functions,
  // whose WasmFuncTypeId must be a real interned id even in a module with no
  // table of its own: the wrapper can be handed to another module's table.
  bool hasExportedFuncs = std::any_of(
      exportedFuncVars_.begin(),
      exportedFuncVars_.end(),
      [](Variable *v) { return v != nullptr; });
  if (numTables > 0 || hasExportedFuncs)
    internTypeIds(tlScope);

  // Build the canonical Exported Functions. This must precede createTables():
  // a table slot holds the Exported Function, so an element segment applied
  // below loads exportedFuncVars_ and would otherwise read a Variable that has
  // not been stored yet. It must follow internTypeIds() and the closure
  // pre-creation above, both of which it reads.
  createExportedFunctions(tlScope);

  // Create and initialize tables, and apply element segments.
  if (numTables > 0)
    createTables(tlScope);

  // Initialize Wasm globals (both imported and defined).
  if (numGlobals > 0) {
    initializeGlobals(tlScope);
  }

  // Create import trampoline bodies for all imported functions.
  // This replaces the stub bodies (ReturnInst(undefined)) with actual
  // trampolines that call the imported JS functions.
  for (uint32_t i = 0; i < numImportedFuncs; ++i) {
    createImportTrampoline(i, tlScope);
  }

  // Switch back to the instantiate function entry block after creating
  // trampolines. finalizeModule() will continue building this function.
  builder_.setInsertionBlock(tlEntry_);

  // --- Build the top-level function body ---
  // The top-level function returns a module info object:
  //   {instantiate: <closure>, exportDescs: [...], importDescs: [...]}
  // This is a lightweight function; the real initialization happens when
  // the instantiate closure is called.

  auto *topLevelBody = builder_.createBasicBlock(topLevel);
  builder_.setInsertionBlock(topLevelBody);

  // Create a scope instance of topLevelVS_ so we can create the instantiate
  // closure via CreateFunctionInst.
  auto *topLevelScope = builder_.createCreateScopeInst(
      topLevelVS_,
      builder_.getEmptySentinel());

  // Create the instantiate closure.
  auto *instClosure = builder_.createCreateFunctionInst(
      topLevelScope, instantiateFunc_);

  // Helper: convert WasmExternalKind to a string literal.
  auto kindToString = [this](WasmExternalKind kind) -> LiteralString * {
    switch (kind) {
      case WasmExternalKind::Function:
        return builder_.getLiteralString("function");
      case WasmExternalKind::Table:
        return builder_.getLiteralString("table");
      case WasmExternalKind::Memory:
        return builder_.getLiteralString("memory");
      case WasmExternalKind::Global:
        return builder_.getLiteralString("global");
      case WasmExternalKind::Tag:
        return builder_.getLiteralString("tag");
    }
    return builder_.getLiteralString("function");
  };

  // Build exportDescs array.
  auto *exportDescsArr = emitNew(
      builder_.createTryLoadGlobalPropertyInst("Array"),
      {builder_.getLiteralNumber(
          static_cast<double>(moduleInfo_.exports.size()))});
  for (uint32_t i = 0; i < moduleInfo_.exports.size(); ++i) {
    const auto &exp = moduleInfo_.exports[i];
    auto *desc = builder_.createAllocObjectLiteralInst({});
    builder_.createStorePropertyStrictInst(
        builder_.getLiteralString(exp.name), desc,
        builder_.getLiteralString("name"));
    builder_.createStorePropertyStrictInst(
        kindToString(exp.kind), desc,
        builder_.getLiteralString("kind"));
    builder_.createStorePropertyStrictInst(
        desc, exportDescsArr,
        builder_.getLiteralNumber(static_cast<double>(i)));
  }

  // Build importDescs array.
  auto *importDescsArr = emitNew(
      builder_.createTryLoadGlobalPropertyInst("Array"),
      {builder_.getLiteralNumber(
          static_cast<double>(moduleInfo_.imports.size()))});
  for (uint32_t i = 0; i < moduleInfo_.imports.size(); ++i) {
    const auto &imp = moduleInfo_.imports[i];
    auto *desc = builder_.createAllocObjectLiteralInst({});
    builder_.createStorePropertyStrictInst(
        builder_.getLiteralString(imp.moduleName), desc,
        builder_.getLiteralString("module"));
    builder_.createStorePropertyStrictInst(
        builder_.getLiteralString(imp.fieldName), desc,
        builder_.getLiteralString("name"));
    builder_.createStorePropertyStrictInst(
        kindToString(imp.kind), desc,
        builder_.getLiteralString("kind"));
    builder_.createStorePropertyStrictInst(
        desc, importDescsArr,
        builder_.getLiteralNumber(static_cast<double>(i)));
  }

  // Build module info object: {instantiate, exportDescs, importDescs}.
  auto *moduleInfoObj = builder_.createAllocObjectLiteralInst({});
  builder_.createStorePropertyStrictInst(
      instClosure, moduleInfoObj,
      builder_.getLiteralString("instantiate"));
  builder_.createStorePropertyStrictInst(
      exportDescsArr, moduleInfoObj,
      builder_.getLiteralString("exportDescs"));
  builder_.createStorePropertyStrictInst(
      importDescsArr, moduleInfoObj,
      builder_.getLiteralString("importDescs"));
  builder_.createReturnInst(moduleInfoObj);
}

void WasmIRGen::emitRuntimeLimits(
    Value *descriptor,
    Value *actualMin,
    Value *actualMax) {
  builder_.createStorePropertyStrictInst(
      actualMin, descriptor, builder_.getLiteralString("initial"));

  // -1 means unbounded. Both the Memory and the Table constructor reject a
  // negative `maximum` with a RangeError, so leave the property out.
  auto *hasMax = builder_.createBinaryOperatorInst(
      actualMax,
      builder_.getLiteralNumber(-1),
      ValueKind::BinaryStrictlyNotEqualInstKind);
  auto *setMaxBB = builder_.createBasicBlock(tlEntry_->getParent());
  auto *maxDoneBB = builder_.createBasicBlock(tlEntry_->getParent());
  builder_.createCondBranchInst(hasMax, setMaxBB, maxDoneBB);

  builder_.setInsertionBlock(setMaxBB);
  builder_.createStorePropertyStrictInst(
      actualMax, descriptor, builder_.getLiteralString("maximum"));
  builder_.createBranchInst(maxDoneBB);

  builder_.setInsertionBlock(maxDoneBB);
  tlEntry_ = maxDoneBB;
}

bool WasmIRGen::validateExportIndices() {
  for (const auto &exp : moduleInfo_.exports) {
    // Initialized rather than left to the switch: -Wswitch would flag a new
    // WasmExternalKind, but HERMES_ENABLE_WERROR is OFF by default, so an
    // unhandled kind would otherwise compare against an uninitialized limit
    // and format an uninitialized pointer -- UB inside the function whose job
    // is to prevent it.
    uint32_t limit = 0;
    const char *space = "";
    switch (exp.kind) {
      case WasmExternalKind::Function:
        limit = moduleInfo_.totalFunctionCount();
        space = "function";
        break;
      case WasmExternalKind::Table:
        limit = moduleInfo_.totalTableCount();
        space = "table";
        break;
      case WasmExternalKind::Memory:
        limit = moduleInfo_.totalMemoryCount();
        space = "memory";
        break;
      case WasmExternalKind::Global:
        limit = moduleInfo_.totalGlobalCount();
        space = "global";
        break;
      case WasmExternalKind::Tag:
        limit = moduleInfo_.totalTagCount();
        space = "tag";
        break;
    }
    if (LLVM_UNLIKELY(exp.index >= limit)) {
      errorMsg_ = "export \"" + exp.name + "\" names " + space + " index " +
          std::to_string(exp.index) + ", but the module has " +
          std::to_string(limit) + " of them";
      return false;
    }
  }
  return true;
}

bool WasmIRGen::finalizeModule() {
  auto *tlScope = tlScope_;
  bool hasMemory = moduleInfo_.totalMemoryCount() > 0;

  // Every export names an index into one of the module's five index spaces,
  // and a MALFORMED module can name one past the end. That reaches the export
  // loops below directly, because `hermesc --wasm` does not validate its
  // input: compileWasmModule() runs wabt::ReadBinary only, never
  // wabt::ValidateModule (H19). The table export's
  // `moduleInfo_.tables[exp.index - numImportedTables]` was a
  // heap-buffer-overflow read under ASan on a .wasm whose export index had
  // been patched by hand; the global export had a bare `assert`, which is not
  // a diagnostic in a release build, and the tag export had nothing at all.
  //
  // Refused here, once, before a single instruction is emitted, rather than
  // guarding each of the four loops: an out-of-range export index means the
  // module is invalid, and the answer is to reject the module, not to skip
  // the export and compile the rest of it.
  if (LLVM_UNLIKELY(!validateExportIndices()))
    return false;

  // Ensure insertion is at the instantiate function's entry block.
  // tlEntry_ names the block the instantiate body continues in. Every helper
  // that splits that body into more blocks must leave tlEntry_ pointing at
  // the last, still-open one; an already-terminated block here means one of
  // them broke that contract and everything emitted below is unreachable.
  assert(
      !tlEntry_->getTerminator() &&
      "tlEntry_ must name an unterminated block: helpers that split the "
      "instantiate body must leave it pointing at the block emission "
      "continues in");
  builder_.setInsertionBlock(tlEntry_);

  // Running offset into the binary data storage blob. Each data segment's
  // bytes are appended in order (by WasmCompile.cpp), so we compute each
  // segment's blob offset by accumulating sizes.
  uint32_t binaryDataOffset = 0;

  // Initialize the data segments array (for memory.init/data.drop).
  // Each element is a Uint8Array containing the segment's data bytes,
  // or null for segments that have been dropped.
  if (dataSegVar_) {
    uint32_t numSegs = moduleInfo_.dataSegments.size();
    auto *Uint8ArrayCtor =
        builder_.createTryLoadGlobalPropertyInst("Uint8Array");
    auto *segsArr = emitNew(
        builder_.createTryLoadGlobalPropertyInst("Array"),
        {builder_.getLiteralNumber(static_cast<double>(numSegs))});
    builder_.createStoreFrameInst(tlScope, segsArr, dataSegVar_);

    for (uint32_t si = 0; si < numSegs; ++si) {
      const auto &seg = moduleInfo_.dataSegments[si];
      if (seg.data.empty()) {
        // Empty segment: store null (same as dropped).
        builder_.createStorePropertyStrictInst(
            builder_.getLiteralNull(),
            segsArr,
            builder_.getLiteralNumber(static_cast<double>(si)));
        // Still advance binaryDataOffset for consistency with the blob.
        binaryDataOffset += seg.data.size();
        continue;
      }

      // Create a Uint8Array and bulk-fill it from the binary data blob.
      auto *segArr = emitNew(
          Uint8ArrayCtor,
          {builder_.getLiteralNumber(
              static_cast<double>(seg.data.size()))});
      helpers_.emitDataSegmentInit(
          segArr,
          builder_.getLiteralNumber(
              static_cast<double>(binaryDataOffset)),
          builder_.getLiteralNumber(
              static_cast<double>(seg.data.size())),
          builder_.getLiteralNumber(0));
      binaryDataOffset += seg.data.size();
      builder_.createStorePropertyStrictInst(
          segArr,
          segsArr,
          builder_.getLiteralNumber(static_cast<double>(si)));
    }
  } else {
    // Even when dataSegVar_ is not set, we still need to advance
    // binaryDataOffset for all segments.
    for (const auto &seg : moduleInfo_.dataSegments) {
      binaryDataOffset += seg.data.size();
    }
  }

  // Apply active data segments: copy bytes into linear memory.
  if (hasMemory) {
    // Reset binaryDataOffset — the active loop iterates the same segments
    // array and computes its own offsets.
    binaryDataOffset = 0;
    // Compute initial memory size for data segment bounds checking.
    // Only a locally-defined memory has a size known at compile time. An
    // import declaration states a *minimum*, and the module operates on
    // whatever memory it is actually given, which may be larger -- so a
    // segment past the declared minimum is not out of bounds, and refusing
    // it at compile time rejects modules other engines accept. Those
    // segments are left to the runtime bounds check, which measures the
    // memory in hand.
    uint64_t memoryBytes = 0;
    bool canBoundsCheck = false;
    if (!moduleInfo_.memories.empty()) {
      memoryBytes =
          static_cast<uint64_t>(moduleInfo_.memories[0].limits.initial) *
          65536;
      canBoundsCheck = true;
    }

    for (uint32_t si = 0; si < moduleInfo_.dataSegments.size(); ++si) {
      const auto &seg = moduleInfo_.dataSegments[si];
      if (seg.mode != WasmDataSegment::Mode::Active) {
        binaryDataOffset += seg.data.size();
        continue;
      }

      // Step 1: Compute offset as an IR Value.
      Value *offset = nullptr;
      if (seg.offsetExpr.size() <= 1) {
        // Simple case: single I32Const or GlobalGet.
        if (seg.offsetKind == WasmGlobal::InitKind::I32Const) {
          offset = builder_.getLiteralNumber(
              static_cast<double>(seg.offsetValue));
        } else if (seg.offsetKind == WasmGlobal::InitKind::GlobalGet) {
          uint32_t slotIdx = globalSlotIndex_[seg.offsetGlobalIdx];
          offset =
              builder_.createLoadFrameInst(tlScope, globalVars_[slotIdx]);
        } else {
          llvh::errs()
              << "warning: unsupported data segment offset expression\n";
          binaryDataOffset += seg.data.size();
          continue;
        }
      } else {
        // Extended const expression: evaluate the stack machine.
        offset = emitInitExpr(seg.offsetExpr, tlScope);
        if (!offset) {
          llvh::errs()
              << "warning: malformed data segment offset expression\n";
          continue;
        }
      }

      // Step 2: Compile-time bounds check (locally-defined memory +
      // I32Const offset only). An OOB segment traps unconditionally and
      // prevents all further initialization.
      if (canBoundsCheck && seg.offsetExpr.size() <= 1 &&
          seg.offsetKind == WasmGlobal::InitKind::I32Const) {
        uint64_t offsetU =
            static_cast<uint64_t>(static_cast<uint32_t>(seg.offsetValue));
        if (offsetU + seg.data.size() > memoryBytes) {
          helpers_.emitTrap();
          builder_.createUnreachableInst();
          // Create a new dead block for remaining initialization code.
          // Replace tlEntry_ so that code after createExportWrapper()
          // (which resets insertion to tlEntry_) also goes into the dead
          // block.
          tlEntry_ = builder_.createBasicBlock(
              tlEntry_->getParent());
          builder_.setInsertionBlock(tlEntry_);
          break;
        }
      }

      // Step 3: Runtime bounds check when the offset is not a simple
      // i32.const (unknown value at compile time). This includes
      // GlobalGet offsets and extended const expressions. Emits:
      // if (offset >>> 0 + data_size > HEAPU8.length) trap. Only applies
      // when canBoundsCheck is true (memory size is reliable enough for
      // checking — locally-defined memories always qualify; imported
      // memories qualify when the declared minimum is > 0, since Hermes
      // creates the memory with exactly that size).
      if (canBoundsCheck &&
          !(seg.offsetExpr.size() <= 1 &&
            seg.offsetKind == WasmGlobal::InitKind::I32Const)) {
        // Get memory byte length: HEAPU8.length
        auto *heapu8Chk = builder_.createLoadFrameInst(
            tlScope,
            memViewVars_[static_cast<uint8_t>(MemView::HEAPU8)]);
        auto *memLength = builder_.createLoadPropertyInst(
            heapu8Chk, builder_.getLiteralString("length"));

        // Treat offset as unsigned: offset >>> 0
        auto *offsetU = builder_.createBinaryOperatorInst(
            offset,
            builder_.getLiteralNumber(0),
            ValueKind::BinaryUnsignedRightShiftInstKind);

        // end = offsetU + seg.data.size()
        auto *end = builder_.createBinaryOperatorInst(
            offsetU,
            builder_.getLiteralNumber(
                static_cast<double>(seg.data.size())),
            ValueKind::BinaryAddInstKind);

        // if (end > memLength) trap;
        auto *isOOB = builder_.createBinaryOperatorInst(
            end, memLength, ValueKind::BinaryGreaterThanInstKind);
        auto *trapBlock =
            builder_.createBasicBlock(tlEntry_->getParent());
        auto *okBlock =
            builder_.createBasicBlock(tlEntry_->getParent());
        builder_.createCondBranchInst(isOOB, trapBlock, okBlock);

        builder_.setInsertionBlock(trapBlock);
        helpers_.emitTrap();
        builder_.createUnreachableInst();

        builder_.setInsertionBlock(okBlock);
        // Update tlEntry_ so that code after createExportWrapper()
        // (which resets insertion to tlEntry_) continues in the ok
        // block rather than the now-terminated original entry block.
        tlEntry_ = okBlock;
      }

      if (seg.data.empty()) {
        binaryDataOffset += seg.data.size();
        continue;
      }

      // Load HEAPU8 view and bulk-copy from the binary data blob.
      auto *heapu8 = builder_.createLoadFrameInst(
          tlScope, memViewVars_[static_cast<uint8_t>(MemView::HEAPU8)]);

      helpers_.emitDataSegmentInit(
          heapu8,
          builder_.getLiteralNumber(
              static_cast<double>(binaryDataOffset)),
          builder_.getLiteralNumber(
              static_cast<double>(seg.data.size())),
          offset);
      binaryDataOffset += seg.data.size();

      // After applying an active data segment, mark it as dropped.
      if (dataSegVar_) {
        auto *dataSegsArr = builder_.createLoadFrameInst(
            tlScope, dataSegVar_);
        builder_.createStorePropertyStrictInst(
            builder_.getLiteralNull(),
            dataSegsArr,
            builder_.getLiteralNumber(static_cast<double>(si)));
      }
    }
  }

  // Initialize the element segments array (for table.init/elem.drop).
  // Each element is a JS Array of Exported Functions, one per entry, or null
  // for segments that have been dropped. Only the wrapper is stored: table.init
  // writes through the slot funnel, which derives the closure and the interned
  // type id from it, so a segment cannot describe a slot inconsistently.
  if (elemSegVar_) {
    uint32_t numElemSegs = moduleInfo_.elements.size();
    auto *elemsArr = emitNew(
        builder_.createTryLoadGlobalPropertyInst("Array"),
        {builder_.getLiteralNumber(static_cast<double>(numElemSegs))});
    builder_.createStoreFrameInst(tlScope, elemsArr, elemSegVar_);

    for (uint32_t si = 0; si < numElemSegs; ++si) {
      const auto &seg = moduleInfo_.elements[si];

      // Declarative segments are immediately dropped.
      if (seg.mode == WasmElemSegment::Mode::Declarative) {
        builder_.createStorePropertyStrictInst(
            builder_.getLiteralNull(),
            elemsArr,
            builder_.getLiteralNumber(static_cast<double>(si)));
        continue;
      }

      if (seg.funcIndices.empty()) {
        // Empty segment: store null (same as dropped).
        builder_.createStorePropertyStrictInst(
            builder_.getLiteralNull(),
            elemsArr,
            builder_.getLiteralNumber(static_cast<double>(si)));
        continue;
      }

      // Create the segment array: [exportedFunc0, exportedFunc1, ...]
      uint32_t numEntries = seg.funcIndices.size();
      auto *segArr = emitNew(
          builder_.createTryLoadGlobalPropertyInst("Array"),
          {builder_.getLiteralNumber(static_cast<double>(numEntries))});

      for (uint32_t i = 0; i < numEntries; ++i) {
        uint32_t funcIdx = seg.funcIndices[i];
        // Every function index named by an element segment is in
        // escapableFuncs_, so it has a canonical Exported Function; the null
        // is for a segment naming an index this module does not have.
        bool known =
            funcIdx < exportedFuncVars_.size() && exportedFuncVars_[funcIdx];
        builder_.createStorePropertyStrictInst(
            known ? static_cast<Value *>(builder_.createLoadFrameInst(
                        tlScope, exportedFuncVars_[funcIdx]))
                  : builder_.getLiteralNull(),
            segArr,
            builder_.getLiteralNumber(static_cast<double>(i)));
      }

      builder_.createStorePropertyStrictInst(
          segArr,
          elemsArr,
          builder_.getLiteralNumber(static_cast<double>(si)));

      // Active segments are dropped after their contents have been applied
      // (applied in createTables during createFunctions).
      if (seg.mode == WasmElemSegment::Mode::Active) {
        builder_.createStorePropertyStrictInst(
            builder_.getLiteralNull(),
            elemsArr,
            builder_.getLiteralNumber(static_cast<double>(si)));
      }
    }
  }

  // Call the start function if specified (load its pre-created closure).
  if (moduleInfo_.startFunction.has_value()) {
    uint32_t startIdx = *moduleInfo_.startFunction;
    if (startIdx < irFunctions_.size()) {
      auto *closure = builder_.createLoadFrameInst(
          tlScope, closureVars_[startIdx]);
      builder_.createCallInst(
          closure,
          /* target */ irFunctions_[startIdx],
          /* calleeIsAlwaysClosure */ true,
          /* env */ builder_.getEmptySentinel(),
          /* newTarget */ builder_.getLiteralUndefined(),
          /* thisValue */ builder_.getLiteralUndefined(),
          {});
    }
  }

  // Build the exports object. Each export name maps to the ONE canonical
  // Exported Function of its function index (built earlier, by
  // createExportedFunctions), so a function exported under several names is
  // the same object under all of them. Function, global, tag, memory, and
  // table exports are handled.
  auto *exportsObj = builder_.createAllocObjectLiteralInst({});
  for (const auto &exp : moduleInfo_.exports) {
    if (exp.kind != WasmExternalKind::Function)
      continue;
    // One canonical wrapper per index, looked up rather than built here, so
    // two names for one function name the same object.
    assert(
        exp.index < exportedFuncVars_.size() && exportedFuncVars_[exp.index] &&
        "every exported function index must have a canonical wrapper");
    builder_.createStorePropertyStrictInst(
        builder_.createLoadFrameInst(tlScope, exportedFuncVars_[exp.index]),
        exportsObj,
        builder_.getLiteralString(exp.name));
  }

  // Add global exports as WebAssembly.Global objects. Each exported global is
  // wrapped in a WebAssembly.Global because that is what an importer's brand
  // check requires: the type and mutability a cross-module import compares
  // against live in the Global's internal fields, and are read by
  // wasmLinkGlobal. (They used to be published as a __wasm_type__ string on
  // the wrapper, which is what the importer compared; that publication is
  // gone and a WebAssembly.Global has no own properties at all.)
  // The value is a snapshot at init time; mutable globals won't reflect
  // later mutations (that would require live wiring, a separate change).

  // Load WebAssembly.Global constructor once if there are global exports.
  Value *wasmGlobalCtor = nullptr;
  // An imported mutable global is re-exported as the object it was imported
  // as, so it does not need the constructor.
  bool hasGlobalExports = std::any_of(
      moduleInfo_.exports.begin(),
      moduleInfo_.exports.end(),
      [this](const WasmExport &e) {
        return e.kind == WasmExternalKind::Global &&
            !importedMutableGlobals_.count(e.index);
      });
  if (hasGlobalExports) {
    auto *wasmObj =
        builder_.createTryLoadGlobalPropertyInst("WebAssembly");
    wasmGlobalCtor = builder_.createLoadPropertyInst(
        wasmObj, builder_.getLiteralString("Global"));
  }

  for (const auto &exp : moduleInfo_.exports) {
    if (exp.kind != WasmExternalKind::Global)
      continue;
    // validateExportIndices() has already refused an out-of-range index with a
    // message; this only records that the two vectors are sized from the same
    // count it compared against.
    assert(exp.index < globalSlotIndex_.size() && "global index out of range");

    // Re-exporting an imported mutable global publishes the very global that
    // was imported: it is shared state, and both this module and the host
    // write it. Wrapping a copy of its link-time value in a fresh
    // WebAssembly.Global would hand out a snapshot that tracks neither.
    if (importedMutableGlobals_.count(exp.index)) {
      auto *globalObj = builder_.createLoadFrameInst(
          tlScope, importGlobalVals_[exp.index]);
      builder_.createStorePropertyStrictInst(
          globalObj, exportsObj, builder_.getLiteralString(exp.name));
      continue;
    }

    uint32_t slotIdx = globalSlotIndex_[exp.index];
    auto *val = builder_.createLoadFrameInst(tlScope, globalVars_[slotIdx]);

    // Determine the global's type and mutability.
    uint32_t numImportedGlobals = moduleInfo_.importedGlobalCount();
    WasmGlobalType gType{WasmValType::I32, false};
    if (exp.index < numImportedGlobals) {
      uint32_t idx = 0;
      for (const auto &imp : moduleInfo_.imports) {
        if (imp.kind != WasmExternalKind::Global)
          continue;
        if (idx == exp.index) {
          gType = imp.globalType;
          break;
        }
        ++idx;
      }
    } else {
      gType = moduleInfo_.globals[exp.index - numImportedGlobals].type;
    }

    // The value for the Global constructor. An i64 global is stored as a
    // split lo/hi pair, so recombine it into a BigInt: passing the lo32 half
    // alone silently discards the upper word, and WebAssembly.Global now
    // stores i64 exactly and exposes it as a BigInt, per spec.
    Value *rawValue = val;
    if (gType.type == WasmValType::I64) {
      auto *hi =
          builder_.createLoadFrameInst(tlScope, globalVars_[slotIdx + 1]);
      rawValue = helpers_.emitI64ToBigInt(val, hi);
    }

    // Build the type descriptor string for the Global constructor.
    const char *typeName;
    switch (gType.type) {
      case WasmValType::I32:
        typeName = "i32";
        break;
      case WasmValType::I64:
        typeName = "i64";
        break;
      case WasmValType::F32:
        typeName = "f32";
        break;
      case WasmValType::F64:
        typeName = "f64";
        break;
      default:
        llvm_unreachable("unsupported global export type");
    }

    // Create descriptor: {value: "i32", mutable: false}
    auto *descriptor = builder_.createAllocObjectLiteralInst({});
    builder_.createStorePropertyStrictInst(
        builder_.getLiteralString(typeName),
        descriptor,
        builder_.getLiteralString("value"));
    builder_.createStorePropertyStrictInst(
        builder_.getLiteralBool(gType.mutable_),
        descriptor,
        builder_.getLiteralString("mutable"));

    // Construct: new WebAssembly.Global(descriptor, rawValue)
    auto *globalObj = emitNew(wasmGlobalCtor, {descriptor, rawValue});

    builder_.createStorePropertyStrictInst(
        globalObj, exportsObj, builder_.getLiteralString(exp.name));
  }

  // Add tag exports as plain objects with __wasm_type__ metadata.
  for (const auto &exp : moduleInfo_.exports) {
    if (exp.kind != WasmExternalKind::Tag)
      continue;
    // Export the object that identifies this tag, not a fresh one: an
    // importer compares identity against it, so a copy would never match.
    assert(exp.index < tagVars_.size() && "tag index out of range");
    auto *tagObj = builder_.createLoadFrameInst(tlScope, tagVars_[exp.index]);
    builder_.createStorePropertyStrictInst(
        tagObj, exportsObj, builder_.getLiteralString(exp.name));
  }

  // Add memory exports. There is nothing to construct: the module already
  // operates on a WebAssembly.Memory, and that is the object to publish.
  for (const auto &exp : moduleInfo_.exports) {
    if (exp.kind != WasmExternalKind::Memory)
      continue;

    // The memory the module operates on is a WebAssembly.Memory -- either
    // constructed for a defined memory or supplied for an imported one.
    // Export that same object. Re-exporting an import this way also gives
    // the identity the spec requires, and its limits are its own, so nothing
    // can understate them.
    builder_.createStorePropertyStrictInst(
        builder_.createLoadFrameInst(tlScope, memObjVar_),
        exportsObj,
        builder_.getLiteralString(exp.name));
  }

  // Add table exports as WebAssembly.Table objects. A funcref table -- one the
  // module defined or one it imported -- already HAS its WebAssembly.Table,
  // and that object is published as it stands. Nothing is stamped onto it:
  // the storage a cross-module import needs now comes out of the object's
  // internal fields through the wasmLinkTable brand check, so there is no
  // publication left to make and no forgeable copy of the ABI to leak.
  // Loaded lazily: only an externref table export needs the constructor now,
  // and reading globalThis.WebAssembly.Table when nothing will use it would
  // run a user getter for nothing.
  Value *wasmTableCtor = nullptr;

  for (const auto &exp : moduleInfo_.exports) {
    if (exp.kind != WasmExternalKind::Table)
      continue;

    // Determine table type from the table index space.
    // Imported tables come first, then defined tables.
    // validateExportIndices() bounded exp.index by totalTableCount(), which is
    // what makes both the subscript below and tableObjVars_[exp.index] safe.
    assert(exp.index < moduleInfo_.totalTableCount() && "table index OOR");
    uint32_t numImportedTables = moduleInfo_.importedTableCount();
    bool isImported = exp.index < numImportedTables;
    WasmTableType tType{};
    if (isImported) {
      uint32_t idx = 0;
      for (const auto &imp : moduleInfo_.imports) {
        if (imp.kind != WasmExternalKind::Table)
          continue;
        if (idx == exp.index) {
          tType = imp.tableType;
          break;
        }
        ++idx;
      }
    } else {
      tType = moduleInfo_.tables[exp.index - numImportedTables];
    }

    // A funcref table already HAS its WebAssembly.Table: the one the module
    // constructed for a defined table, or the very one that satisfied the
    // import. Publish it, so exports.tbl.get/set/grow/length operate on the
    // module's real storage rather than a disconnected copy -- and so that
    // re-exporting an imported table yields the same object, which is both
    // what the spec says and the only way the storage can still be shared now
    // that it lives in internal fields.
    if (tType.elemType == WasmValType::FuncRef) {
      builder_.createStorePropertyStrictInst(
          builder_.createLoadFrameInst(tlScope, tableObjVars_[exp.index]),
          exportsObj,
          builder_.getLiteralString(exp.name));
      continue;
    }

    // An EXTERNREF table has no WebAssembly.Table -- the constructor accepts
    // only "anyfunc"/"funcref" -- so exporting one raises a TypeError from
    // the constructor below at instantiate time. That is pre-existing and
    // unchanged here; the code is kept rather than turned into a compile-time
    // diagnostic because the diagnostic belongs with the rest of the
    // externref work, not with this change. An IMPORTED externref table
    // cannot link at all (no object can satisfy the declaration), so only the
    // declared limits are used.
    if (!wasmTableCtor) {
      auto *wasmObj =
          builder_.createTryLoadGlobalPropertyInst("WebAssembly");
      wasmTableCtor = builder_.createLoadPropertyInst(
          wasmObj, builder_.getLiteralString("Table"));
    }
    auto *descriptor = builder_.createAllocObjectLiteralInst({});
    builder_.createStorePropertyStrictInst(
        builder_.getLiteralString("externref"),
        descriptor,
        builder_.getLiteralString("element"));
    builder_.createStorePropertyStrictInst(
        builder_.getLiteralNumber(
            static_cast<double>(tType.limits.initial)),
        descriptor,
        builder_.getLiteralString("initial"));
    if (tType.limits.hasMaximum) {
      builder_.createStorePropertyStrictInst(
          builder_.getLiteralNumber(
              static_cast<double>(tType.limits.maximum)),
          descriptor,
          builder_.getLiteralString("maximum"));
    }

    // Construct: new WebAssembly.Table(descriptor)
    auto *tableObj = emitNew(wasmTableCtor, {descriptor});
    builder_.createStorePropertyStrictInst(
        tableObj, exportsObj, builder_.getLiteralString(exp.name));
  }

  builder_.createReturnInst(exportsObj);
  return true;
}

void WasmIRGen::createExportedFunctions(BaseScopeInst *tlScope) {
  // Create the wrapper bodies first (this switches the insertion point).
  struct ExportWrapperInfo {
    Function *wrapperFunc;
    uint32_t funcIndex;
  };
  std::vector<ExportWrapperInfo> wrappers;
  for (uint32_t fi = 0; fi < exportedFuncVars_.size(); ++fi) {
    if (!exportedFuncVars_[fi])
      continue;
    wrappers.push_back(
        {createExportWrapper(fi, exportWrapperName(fi), tlScope), fi});
  }
  if (wrappers.empty())
    return;

  // Switch back to the instantiate function's entry block to emit the wrapper
  // closures. tlEntry_ must still be unterminated: it names the block the
  // instantiate body continues in, and createExportWrapper does not touch it.
  assert(
      !tlEntry_->getTerminator() &&
      "tlEntry_ must name an unterminated block: helpers that split the "
      "instantiate body must leave it pointing at the block emission "
      "continues in");
  builder_.setInsertionBlock(tlEntry_);

  for (const auto &w : wrappers) {
    auto *wrapperClosure = builder_.createCreateFunctionInst(
        tlScope, w.wrapperFunc);
    // Set __wasm_type__ on the wrapper closure for import type validation.
    std::string typeStr =
        buildFuncTypeString(moduleInfo_.getFunctionType(w.funcIndex));
    builder_.createStorePropertyStrictInst(
        builder_.getLiteralString(typeStr),
        wrapperClosure,
        builder_.getLiteralString("__wasm_type__"));

    // Stamp the internal state that makes this an Exported Function: the
    // closure it wraps and the INTERNED id of its signature. Interned, not
    // module-local: the same signature is numbered differently in another
    // module (which would trap spuriously) and different signatures can share
    // a number (which would miss a trap). canonicalTypeIndex_ collapses
    // structurally identical entries of this module's own type section first.
    uint32_t typeIdx =
        canonicalTypeIndex_[moduleInfo_.getFunctionTypeIndex(w.funcIndex)];
    // Not "the Variable exists" -- those are created unconditionally -- but
    // "the interning ran", which is what makes the slot hold an id rather than
    // undefined. Narrowing internTypeIds() to only some types must trip this.
    assert(
        internedTypeIds_ &&
        "an exported function's type id must have been interned");
    builder_.createCallBuiltinInst(
        BuiltinMethod::HermesBuiltin_wasmSetFuncInfo,
        {wrapperClosure,
         builder_.createLoadFrameInst(tlScope, closureVars_[w.funcIndex]),
         builder_.createLoadFrameInst(tlScope, typeIdVars_[typeIdx])});

    builder_.createStoreFrameInst(
        tlScope, wrapperClosure, exportedFuncVars_[w.funcIndex]);
  }
}

std::string WasmIRGen::exportWrapperName(uint32_t funcIndex) const {
  for (const auto &exp : moduleInfo_.exports)
    if (exp.kind == WasmExternalKind::Function && exp.index == funcIndex)
      return ("wasm_export_" + exp.name);
  return ("wasm_funcref_" + llvh::Twine(funcIndex)).str();
}

Function *WasmIRGen::createExportWrapper(
    uint32_t funcIndex,
    llvh::StringRef wrapperName,
    Instruction *tlScope) {
  const WasmFuncType &funcType = moduleInfo_.getFunctionType(funcIndex);

  // Create the wrapper function.
  auto *wrapperFunc = builder_.createFunction(
      wrapperName,
      Function::DefinitionKind::ES5Function,
      true /* strictMode */);

  // Wrapper takes 1 JS param per Wasm param (BigInt for i64).
  builder_.createJSThisParam(wrapperFunc);
  uint32_t numParams = funcType.params.size();
  for (uint32_t i = 0; i < numParams; ++i) {
    builder_.createJSDynamicParam(
        wrapperFunc, ("p" + llvh::Twine(i)).str());
  }
  wrapperFunc->setExpectedParamCountIncludingThis(numParams + 1);

  // Build the wrapper function body.
  auto *entryBB = builder_.createBasicBlock(wrapperFunc);
  builder_.setInsertionBlock(entryBB);

  // Get the parent scope to load the internal function's closure.
  auto *parentScope = builder_.createGetParentScopeInst(
      topLevelVS_, wrapperFunc->getParentScopeParam());

  // Load the internal function's closure from the top-level scope.
  auto *internalClosure = builder_.createLoadFrameInst(
      parentScope, closureVars_[funcIndex]);

  // Marshal arguments: coerce each JS param to the expected Wasm type.
  // For i64 params, the internal function expects two JS args (lo, hi).
  llvh::SmallVector<Value *, 8> callArgs;

  // Check if we need retBufI for any purpose (return buffer or i64 params).
  bool hasI64Param = false;
  for (auto p : funcType.params)
    if (p == WasmValType::I64)
      hasI64Param = true;

  // Load retBuf views if needed.
  Value *rbI = nullptr;
  Value *rbF = nullptr;
  if (retBufIVar_ && (needsReturnBuffer(funcType) || hasI64Param)) {
    rbI = builder_.createLoadFrameInst(parentScope, retBufIVar_);
  }
  if (retBufFVar_ && needsReturnBuffer(funcType)) {
    rbF = builder_.createLoadFrameInst(parentScope, retBufFVar_);
  }
  // The reference array is not part of the calling convention; it is reached
  // through the top-level scope like every other per-module object.
  Value *rbR = nullptr;
  auto getRbR = [&]() -> Value * {
    assert(retBufRVar_ && "reference result but no reference array");
    if (!rbR)
      rbR = builder_.createLoadFrameInst(parentScope, retBufRVar_);
    return rbR;
  };

  // If the internal function needs a return buffer, prepend retBufI/retBufF.
  if (needsReturnBuffer(funcType)) {
    callArgs.push_back(rbI);
    callArgs.push_back(rbF);
  }

  for (uint32_t i = 0; i < numParams; ++i) {
    // JS param index: 0=this, 1..N=user params. getJSDynamicParam(1+i).
    auto *jsParam = wrapperFunc->getJSDynamicParam(1 + i);
    auto *paramVal = builder_.createLoadParamInst(jsParam);

    switch (funcType.params[i]) {
      case WasmValType::I32:
        // Coerce to int32.
        callArgs.push_back(builder_.createAsInt32Inst(paramVal));
        break;
      case WasmValType::I64: {
        // JS passes a BigInt. Convert to split (lo, hi) for internal call.
        // emitBigIntToI64 writes lo/hi to retBufI[0]/[1].
        helpers_.emitBigIntToI64(rbI, paramVal);
        // rbI is a Uint32Array, so these read back unsigned. Narrow to int32
        // like every other buffer read: the internal function's parameters
        // are typed, and Wasm i32 halves are signed -- without this,
        // i32.wrap_i64(-1n) yields 4294967295 instead of -1, and the
        // untyped value reaches type-checked arithmetic.
        auto *lo = builder_.createAsInt32Inst(builder_.createLoadPropertyInst(
            rbI, builder_.getLiteralNumber(0)));
        auto *hi = builder_.createAsInt32Inst(builder_.createLoadPropertyInst(
            rbI, builder_.getLiteralNumber(1)));
        callArgs.push_back(lo);
        callArgs.push_back(hi);
        break;
      }
      case WasmValType::F64:
        // ToNumber. This is the ONLY place a JS value becomes an f64
        // parameter: every route by which script can reach a Wasm function
        // yields this wrapper (e2e-no-closure-escape.wat enumerates them), so
        // the internal function's `:number` parameter annotation rests on this
        // instruction. It is a real ToNumber and not a trusted narrowing for
        // that reason -- the float backend reads the raw double bits.
        callArgs.push_back(builder_.createAsNumberInst(paramVal));
        break;
      case WasmValType::F32:
        // ToNumber, then round to single precision. ToWebAssemblyValue for f32
        // is ToNumber followed by "round to nearest representable f32", and an
        // f32 local must hold an f32 value: `(func (param f32) (result f32)
        // (local.get 0))` called with 1.1 has to answer 1.100000023841858, not
        // 1.1. The rounding used to live at the internal function's entry and
        // only for functions whose closure could reach JS (the J4 interim), so
        // every OTHER exported f32-parameter function -- the common case --
        // silently skipped it. The spec suite cannot see that: its literals
        // are already f32-representable, so the rounding is a no-op on every
        // value it passes.
        callArgs.push_back(
            emitFround(builder_.createAsNumberInst(paramVal)));
        break;
      default:
        // FuncRef, ExternRef, etc: pass through for now.
        callArgs.push_back(paramVal);
        break;
    }
  }

  // Call the internal Wasm function.
  auto *callResult = builder_.createCallInst(
      internalClosure,
      /* target */ irFunctions_[funcIndex],
      /* calleeIsAlwaysClosure */ true,
      /* env */ builder_.getEmptySentinel(),
      /* newTarget */ builder_.getLiteralUndefined(),
      /* thisValue */ builder_.getLiteralUndefined(),
      callArgs);

  // Marshal the return value.
  if (funcType.results.empty()) {
    // Void function: return undefined.
    builder_.createReturnInst(builder_.getLiteralUndefined());
  } else if (needsReturnBuffer(funcType)) {
    if (funcType.results.size() == 1 &&
        funcType.results[0] == WasmValType::I64) {
      // Single i64: read lo/hi from buffer, convert to BigInt.
      // rbI is a Uint32Array, so the halves read back unsigned. Narrow to
      // int32 like every other return-buffer read (see emitRetBufLoads and
      // the I32 case below): the split i64 convention is a pair of *signed*
      // int32 halves. wasmI64ToBigInt happens to re-truncate its arguments,
      // so this is not a behavior change -- it keeps the invariant visible at
      // the read site instead of resting on the builtin's internals.
      auto *lo = builder_.createAsInt32Inst(builder_.createLoadPropertyInst(
          rbI, builder_.getLiteralNumber(0)));
      auto *hi = builder_.createAsInt32Inst(builder_.createLoadPropertyInst(
          rbI, builder_.getLiteralNumber(1)));
      auto *bigint = helpers_.emitI64ToBigInt(lo, hi);
      builder_.createReturnInst(bigint);
    } else {
      // Multi-value: return a JS Array of results.
      auto [offsets, totalSize] = computeRetBufLayout(funcType.results);
      auto *ArrayCtor =
          builder_.createTryLoadGlobalPropertyInst("Array");
      auto *resultArr = emitNew(
          ArrayCtor,
          {builder_.getLiteralNumber(
              static_cast<double>(funcType.results.size()))});
      for (size_t i = 0; i < funcType.results.size(); ++i) {
        uint32_t byteOff = offsets[i];
        Value *val;
        switch (funcType.results[i]) {
          case WasmValType::I32: {
            uint32_t idx = byteOff / 4;
            auto *raw = builder_.createLoadPropertyInst(
                rbI, builder_.getLiteralNumber(idx));
            val = builder_.createAsInt32Inst(raw);
            break;
          }
          case WasmValType::I64: {
            // Narrow both halves for the same reason as the I32 case above:
            // rbI is a Uint32Array and the split i64 halves are signed int32.
            uint32_t idx = byteOff / 4;
            auto *lo =
                builder_.createAsInt32Inst(builder_.createLoadPropertyInst(
                    rbI, builder_.getLiteralNumber(idx)));
            auto *hi =
                builder_.createAsInt32Inst(builder_.createLoadPropertyInst(
                    rbI, builder_.getLiteralNumber(idx + 1)));
            val = helpers_.emitI64ToBigInt(lo, hi);
            break;
          }
          case WasmValType::F32:
          case WasmValType::F64: {
            uint32_t idx = byteOff / 8;
            val = builder_.createLoadPropertyInst(
                rbF, builder_.getLiteralNumber(idx));
            break;
          }
          case WasmValType::FuncRef:
          case WasmValType::ExternRef: {
            // The reference was stored into the parallel reference array, not
            // into the Uint32Array view, so the real value is still here.
            // No AsInt32Inst: a reference is not a number.
            uint32_t idx = byteOff / 4;
            val = builder_.createLoadPropertyInst(
                getRbR(), builder_.getLiteralNumber(idx));
            break;
          }
          default: {
            // V128 only. It has no representation in either the buffer or the
            // reference array, so keep failing loudly rather than silently
            // misleading: report the construct and substitute the undefined
            // placeholder, which cannot be mistaken for a real result.
            llvh::errs() << "warning: unsupported Wasm result type: "
                         << valTypeName(funcType.results[i])
                         << " (wasm function " << funcIndex << ", result " << i
                         << "); returning undefined\n";
            val = builder_.getLiteralUndefined();
            break;
          }
        }
        builder_.createStorePropertyStrictInst(
            val, resultArr,
            builder_.getLiteralNumber(static_cast<double>(i)));
      }
      builder_.createReturnInst(resultArr);
    }
  } else {
    // i32/f32/f64: return the call result directly.
    builder_.createReturnInst(callResult);
  }

  return wrapperFunc;
}

void WasmIRGen::createImportTrampoline(
    uint32_t funcIndex,
    Instruction *tlScope) {
  assert(
      funcIndex < moduleInfo_.importedFunctionCount() &&
      "funcIndex out of range for import trampoline");

  const WasmFuncType &funcType = moduleInfo_.getFunctionType(funcIndex);
  auto *func = irFunctions_[funcIndex];

  // Clear the placeholder stub body (ReturnInst(undefined)).
  auto &entryBB = func->getBasicBlockList().front();
  while (!entryBB.empty()) {
    entryBB.back().eraseFromParent();
  }
  builder_.setInsertionBlock(&entryBB);

  // Get the parent scope to load the imported JS function.
  auto *parentScope = builder_.createGetParentScopeInst(
      topLevelVS_, func->getParentScopeParam());

  // Load the imported JS callable from the top-level scope.
  auto *jsFunc = builder_.createLoadFrameInst(
      parentScope, importFuncVars_[funcIndex]);

  // Marshal Wasm-typed arguments to JS arguments.
  // The trampoline function uses the split i64 calling convention internally
  // (matching what onCall() emits), but calls the JS function with JS types.
  // i32/f32/f64 → pass through (already JS Numbers).
  // i64 → convert split (lo, hi) to BigInt for JS.
  llvh::SmallVector<Value *, 8> jsArgs;
  // Skip retBuf params if present.
  uint32_t jsParamIdx = needsReturnBuffer(funcType) ? 3 : 1; // 0 = "this"

  // Load retBuf params if this function uses them.
  Value *rbI = nullptr;
  Value *rbF = nullptr;
  if (needsReturnBuffer(funcType)) {
    auto *paramI = func->getJSDynamicParam(1);
    auto *paramF = func->getJSDynamicParam(2);
    rbI = builder_.createLoadParamInst(paramI);
    rbF = builder_.createLoadParamInst(paramF);
  }
  // The reference array is not a parameter; reach it through the top-level
  // scope on demand, as the other reference-array users do.
  Value *rbR = nullptr;
  auto getRbR = [&]() -> Value * {
    assert(retBufRVar_ && "reference result but no reference array");
    if (!rbR)
      rbR = builder_.createLoadFrameInst(parentScope, retBufRVar_);
    return rbR;
  };
  for (uint32_t i = 0; i < funcType.params.size(); ++i) {
    auto *param = func->getJSDynamicParam(jsParamIdx);
    auto *paramVal = builder_.createLoadParamInst(param);

    if (funcType.params[i] == WasmValType::I64) {
      // Convert split (lo, hi) to BigInt for the JS callee.
      auto *hiParam = func->getJSDynamicParam(jsParamIdx + 1);
      auto *hiVal = builder_.createLoadParamInst(hiParam);
      jsArgs.push_back(helpers_.emitI64ToBigInt(paramVal, hiVal));
      jsParamIdx += 2; // skip both lo and hi JS params
    } else {
      jsArgs.push_back(paramVal);
      jsParamIdx += 1;
    }
  }

  // Call the imported JS function.
  auto *callResult = builder_.createCallInst(
      jsFunc,
      /* newTarget */ builder_.getLiteralUndefined(),
      /* thisValue */ builder_.getLiteralUndefined(),
      jsArgs);

  // Marshal the JS return value back to the expected Wasm type.
  if (funcType.results.empty()) {
    // Void: return undefined.
    builder_.createReturnInst(builder_.getLiteralUndefined());
  } else if (needsReturnBuffer(funcType)) {
    // Write results to the return buffer and return 0.
    if (funcType.results.size() == 1 &&
        funcType.results[0] == WasmValType::I64) {
      // Single i64: JS import returns a BigInt. Convert to lo/hi in buffer.
      helpers_.emitBigIntToI64(rbI, callResult);
      builder_.createReturnInst(builder_.getLiteralNumber(0));
    } else {
      // Multi-value: JS import returns an Array. Read elements and store.
      //
      // In two passes. emitBigIntToI64 always writes its lo/hi through
      // rbI[0]/rbI[1], using them as scratch, so converting an i64 result
      // destroys whatever has already been stored at bytes 0-7 -- for
      // (result i32 i64) that is result 0, which then reads back as the
      // i64's low word. Compute every result into an SSA value first, so a
      // later conversion's scratch use is harmless, and only then store them
      // at their offsets.
      auto [offsets, totalSize] = computeRetBufLayout(funcType.results);
      // One entry per result: {first, second} is {lo, hi} for i64 and
      // {value, nullptr} for everything else.
      std::vector<std::pair<Value *, Value *>> vals;
      vals.reserve(funcType.results.size());
      for (size_t i = 0; i < funcType.results.size(); ++i) {
        auto *jsVal = builder_.createLoadPropertyInst(
            callResult,
            builder_.getLiteralNumber(static_cast<double>(i)));
        switch (funcType.results[i]) {
          case WasmValType::I32:
            vals.emplace_back(builder_.createAsInt32Inst(jsVal), nullptr);
            break;
          case WasmValType::I64: {
            // JS element is a BigInt; convert to lo/hi and capture both
            // before any later conversion overwrites the scratch slots.
            helpers_.emitBigIntToI64(rbI, jsVal);
            auto *lo = builder_.createAsInt32Inst(
                builder_.createLoadPropertyInst(
                    rbI, builder_.getLiteralNumber(0)));
            auto *hi = builder_.createAsInt32Inst(
                builder_.createLoadPropertyInst(
                    rbI, builder_.getLiteralNumber(1)));
            vals.emplace_back(lo, hi);
            break;
          }
          case WasmValType::F32:
          case WasmValType::F64: {
            // Same as the single-result case: the JS element is untyped.
            Value *coerced = builder_.createAsNumberInst(jsVal);
            if (funcType.results[i] == WasmValType::F32)
              coerced = emitFround(coerced);
            vals.emplace_back(coerced, nullptr);
            break;
          }
          default:
            vals.emplace_back(jsVal, nullptr);
            break;
        }
      }
      for (size_t i = 0; i < funcType.results.size(); ++i) {
        uint32_t byteOff = offsets[i];
        switch (funcType.results[i]) {
          case WasmValType::I64:
            builder_.createStorePropertyStrictInst(
                vals[i].first, rbI,
                builder_.getLiteralNumber(byteOff / 4));
            builder_.createStorePropertyStrictInst(
                vals[i].second, rbI,
                builder_.getLiteralNumber(byteOff / 4 + 1));
            break;
          case WasmValType::F32:
          case WasmValType::F64:
            builder_.createStorePropertyStrictInst(
                vals[i].first, rbF,
                builder_.getLiteralNumber(byteOff / 8));
            break;
          case WasmValType::FuncRef:
          case WasmValType::ExternRef:
            // The JS value passes through untouched on the way in (see the
            // parameter loop above); on the way out it must go to the
            // reference array, since the Uint32Array view would coerce it
            // to 0.
            builder_.createStorePropertyStrictInst(
                vals[i].first, getRbR(),
                builder_.getLiteralNumber(byteOff / 4));
            break;
          default:
            // V128: still unsupported. Keep the existing behavior.
            builder_.createStorePropertyStrictInst(
                vals[i].first, rbI,
                builder_.getLiteralNumber(byteOff / 4));
            break;
        }
      }
      builder_.createReturnInst(builder_.getLiteralNumber(0));
    }
  } else {
    switch (funcType.results[0]) {
      case WasmValType::I32:
        // Coerce JS Number to int32 and return.
        builder_.createReturnInst(
            builder_.createAsInt32Inst(callResult));
        break;
      case WasmValType::F32:
        // The JS callee can return anything, so convert rather than assume.
        // Without this the result stays :any and any float arithmetic on it
        // fails lowered-IR verification.
        builder_.createReturnInst(
            emitFround(builder_.createAsNumberInst(callResult)));
        break;
      case WasmValType::F64:
        builder_.createReturnInst(builder_.createAsNumberInst(callResult));
        break;
      default:
        // FuncRef, ExternRef, etc: pass through for now.
        builder_.createReturnInst(callResult);
        break;
    }
  }
}

void WasmIRGen::beginFunction(
    uint32_t funcIndex,
    const std::vector<WasmValType> &localTypes) {
  assert(
      funcIndex < irFunctions_.size() &&
      "funcIndex out of range");
  currentFuncIndex_ = funcIndex;
  currentFunc_ = irFunctions_[funcIndex];
  assert(currentFunc_ && "IR function not created");

  valueStack_.clear();
  valueStackIsI64Hi_.clear();
  locals_.clear();
  localSlotIndex_.clear();
  localTypes_.clear();
  controlStack_.clear();
  unreachable_ = false;

  const WasmFuncType &funcType = moduleInfo_.getFunctionType(funcIndex);

  // Remove the placeholder return instruction and entry block content.
  // The entry block was created with a single ReturnInst(undefined) by
  // createFunctions(). We clear it and reuse the block.
  auto &entryBB = currentFunc_->getBasicBlockList().front();
  // Remove all instructions from the entry block.
  while (!entryBB.empty()) {
    entryBB.back().eraseFromParent();
  }
  builder_.setInsertionBlock(&entryBB);

  // Get the parent (top-level) scope. Used to load pre-created closures
  // from the environment at call sites.
  parentScopeInst_ = builder_.createGetParentScopeInst(
      topLevelVS_, currentFunc_->getParentScopeParam());

  // Load return buffer views for this function.
  retBufI_ = nullptr;
  retBufF_ = nullptr;
  if (needsReturnBuffer(funcType)) {
    // Function receives retBufI and retBufF as its first two params.
    auto *paramI = currentFunc_->getJSDynamicParam(1);
    auto *paramF = currentFunc_->getJSDynamicParam(2);
    retBufI_ = builder_.createLoadParamInst(paramI);
    retBufF_ = builder_.createLoadParamInst(paramF);
  } else if (retBufIVar_) {
    // Function doesn't receive buffer params but may do i64 arithmetic.
    // Load retBufI from the top-level scope.
    retBufI_ = builder_.createLoadFrameInst(
        parentScopeInst_, retBufIVar_);
  }

  // Build local type map (params + declared locals).
  uint32_t numParams = funcType.params.size();
  for (uint32_t i = 0; i < numParams; ++i) {
    localTypes_.push_back(funcType.params[i]);
  }
  for (uint32_t i = 0; i < localTypes.size(); ++i) {
    localTypes_.push_back(localTypes[i]);
  }

  // Create AllocStackInst for each parameter. i64 params use 2 slots.
  // JSDynamicParam index tracks the expanding JS param list.
  // Skip retBuf params (indices 1,2) if this function has them.
  uint32_t jsParamIdx = needsReturnBuffer(funcType) ? 3 : 1; // 0 = "this"
  for (uint32_t i = 0; i < numParams; ++i) {
    localSlotIndex_.push_back(locals_.size());
    if (funcType.params[i] == WasmValType::I64) {
      // i64 param: allocate lo and hi stack slots.
      auto *allocLo = builder_.createAllocStackInst(
          ("local_" + llvh::Twine(i) + "_lo").str(),
          Type::createNumber());
      auto *allocHi = builder_.createAllocStackInst(
          ("local_" + llvh::Twine(i) + "_hi").str(),
          Type::createNumber());
      locals_.push_back(allocLo);
      locals_.push_back(allocHi);

      auto *paramLo = currentFunc_->getJSDynamicParam(jsParamIdx);
      auto *paramHi = currentFunc_->getJSDynamicParam(jsParamIdx + 1);
      builder_.createStoreStackInst(
          builder_.createLoadParamInst(paramLo), allocLo);
      builder_.createStoreStackInst(
          builder_.createLoadParamInst(paramHi), allocHi);
      jsParamIdx += 2;
    } else {
      auto *alloc = builder_.createAllocStackInst(
          ("local_" + llvh::Twine(i)).str(),
          wasmValTypeToIRType(funcType.params[i]));
      locals_.push_back(alloc);

      auto *param = currentFunc_->getJSDynamicParam(jsParamIdx);
      Value *paramVal = builder_.createLoadParamInst(param);
      // Stored raw. The parameter already carries its Wasm type (see
      // createFunctions), and every caller of this closure is Wasm: a direct
      // call, a call_indirect, the start function, or the export wrapper,
      // which is where a JS value is converted. There is no entry coercion
      // here any more -- the J4 interim put one on f32/f64 parameters of
      // "escapable" functions, and the routes that made a function escapable
      // now hand out the wrapper instead of this closure.
      builder_.createStoreStackInst(paramVal, alloc);
      jsParamIdx += 1;
    }
  }

  // Create AllocStackInst for each declared local, initialized to zero.
  for (uint32_t i = 0; i < localTypes.size(); ++i) {
    localSlotIndex_.push_back(locals_.size());
    if (localTypes[i] == WasmValType::I64) {
      // i64 local: allocate lo and hi stack slots, both initialized to 0.
      auto *allocLo = builder_.createAllocStackInst(
          ("local_" + llvh::Twine(numParams + i) + "_lo").str(),
          Type::createNumber());
      auto *allocHi = builder_.createAllocStackInst(
          ("local_" + llvh::Twine(numParams + i) + "_hi").str(),
          Type::createNumber());
      locals_.push_back(allocLo);
      locals_.push_back(allocHi);
      builder_.createStoreStackInst(builder_.getLiteralNumber(0), allocLo);
      builder_.createStoreStackInst(builder_.getLiteralNumber(0), allocHi);
    } else {
      auto *alloc = builder_.createAllocStackInst(
          ("local_" + llvh::Twine(numParams + i)).str(),
          wasmValTypeToIRType(localTypes[i]));
      locals_.push_back(alloc);

      // Initialize locals to their zero value.
      Value *zeroVal;
      switch (localTypes[i]) {
        case WasmValType::I32:
        case WasmValType::F32:
        case WasmValType::F64:
          zeroVal = builder_.getLiteralNumber(0);
          break;
        case WasmValType::FuncRef:
        case WasmValType::ExternRef:
          zeroVal = builder_.getLiteralNull();
          break;
        default:
          zeroVal = builder_.getLiteralNumber(0);
          break;
      }
      builder_.createStoreStackInst(zeroVal, alloc);
    }
  }

  // Push an implicit function-level control entry. The function body acts
  // as an implicit block — wabt calls OnEndExpr for the function body's
  // final "end", which pops this entry via onEnd().
  auto *exitBlock = builder_.createBasicBlock(currentFunc_);

  ControlEntry funcEntry;
  funcEntry.kind = ControlEntry::Block;
  funcEntry.contBlock = exitBlock;
  funcEntry.elseBlock = nullptr;
  funcEntry.resultTypes = funcType.results;
  funcEntry.stackHeight = 0;

  if (!funcType.results.empty()) {
    auto *savedBlock = builder_.getInsertionBlock();
    builder_.setInsertionBlock(exitBlock);
    for (size_t i = 0; i < funcType.results.size(); ++i) {
      funcEntry.resultPhis.push_back(builder_.createPhiInst());
      // i64 results need a second phi for the hi32 part.
      if (funcType.results[i] == WasmValType::I64) {
        funcEntry.resultPhis.push_back(builder_.createPhiInst());
      }
    }
    builder_.setInsertionBlock(savedBlock);
  }

  controlStack_.push_back(std::move(funcEntry));
}

void WasmIRGen::endFunction() {
  // If the control stack still has the implicit function-level entry (e.g.,
  // in unit tests that skip onEnd), pop remaining entries.
  while (!controlStack_.empty()) {
    onEnd();
  }

  // Emit a return if the current block is not terminated.
  // Clear unreachable for the final return emission — we need to access
  // the value stack directly to get the function result phi.
  if (!isCurrentBlockTerminated()) {
    unreachable_ = false;
    const WasmFuncType &funcType =
        moduleInfo_.getFunctionType(currentFuncIndex_);
    if (needsReturnBuffer(funcType) && !valueStack_.empty()) {
      // Store all results to the return buffer and return 0.
      emitRetBufStores(funcType);
    } else if (!funcType.results.empty() && !valueStack_.empty()) {
      // Pop trailing results (index > 0) in reverse order and discard.
      for (size_t i = funcType.results.size(); i > 1; --i) {
        if (valueStack_.empty())
          break;
        if (funcType.results[i - 1] == WasmValType::I64 && isTopI64()) {
          popI64();
        } else {
          pop();
        }
      }
      // Pop and return the first result.
      if (funcType.results[0] == WasmValType::I64 && isTopI64()) {
        auto [lo, hi] = popI64();
        // Single i64 uses retBuf — should have been caught above.
        builder_.createReturnInst(lo);
      } else if (!valueStack_.empty()) {
        Value *result = pop();
        builder_.createReturnInst(result);
      } else {
        builder_.createReturnInst(builder_.getLiteralUndefined());
      }
    } else {
      builder_.createReturnInst(builder_.getLiteralUndefined());
    }
  }

  // Remove dead blocks that were created after unconditional branches/returns
  // but never received instructions. These blocks have no terminator (they're
  // unreachable dead code). We need to erase them since BCGen requires all
  // blocks to have terminators and be reachable.
  llvh::SmallVector<BasicBlock *, 4> deadBlocks;
  for (auto &BB : currentFunc_->getBasicBlockList()) {
    if (BB.empty() || !llvh::isa<TerminatorInst>(&BB.back())) {
      deadBlocks.push_back(&BB);
    }
  }
  for (auto *BB : deadBlocks) {
    BB->eraseFromParent();
  }

  // Fix up catch targets on ThrowInst instructions inside try blocks.
  fixupCatchTargets(currentFunc_);

  currentFunc_ = nullptr;
  parentScopeInst_ = nullptr;
  retBufI_ = nullptr;
  retBufF_ = nullptr;
  valueStack_.clear();
  valueStackIsI64Hi_.clear();
  locals_.clear();
  localSlotIndex_.clear();
  localTypes_.clear();
  controlStack_.clear();
}

void WasmIRGen::onI32Const(int32_t value) {
  if (unreachable_)
    return;
  push(builder_.getLiteralNumber(static_cast<double>(value)));
}

void WasmIRGen::onI64Const(int64_t value) {
  if (unreachable_)
    return;
  // Split i64 into lo32 and hi32 (Phase 1 representation).
  auto lo = static_cast<int32_t>(value & 0xFFFFFFFF);
  auto hi = static_cast<int32_t>(
      (static_cast<uint64_t>(value) >> 32) & 0xFFFFFFFF);
  pushI64(
      builder_.getLiteralNumber(static_cast<double>(lo)),
      builder_.getLiteralNumber(static_cast<double>(hi)));
}

void WasmIRGen::onF32Const(float value) {
  if (unreachable_)
    return;
  auto *lit = builder_.getLiteralNumber(static_cast<double>(value));
  // Record original f32 NaN bits before promotion to f64 loses them.
  if (LLVM_UNLIKELY(std::isnan(value)))
    f32NanBitsMap_[lit] = llvh::FloatToBits(value);
  push(lit);
}

void WasmIRGen::onF64Const(double value) {
  if (unreachable_)
    return;
  push(builder_.getLiteralNumber(value));
}

void WasmIRGen::onLocalGet(uint32_t localIndex) {
  if (unreachable_)
    return;
  assert(localIndex < localSlotIndex_.size() && "localIndex out of range");
  uint32_t slot = localSlotIndex_[localIndex];
  if (localTypes_[localIndex] == WasmValType::I64) {
    auto *lo = builder_.createLoadStackInst(locals_[slot]);
    auto *hi = builder_.createLoadStackInst(locals_[slot + 1]);
    pushI64(lo, hi);
  } else {
    push(builder_.createLoadStackInst(locals_[slot]));
  }
}

void WasmIRGen::onLocalSet(uint32_t localIndex) {
  if (unreachable_)
    return;
  assert(localIndex < localSlotIndex_.size() && "localIndex out of range");
  uint32_t slot = localSlotIndex_[localIndex];
  if (localTypes_[localIndex] == WasmValType::I64) {
    auto [lo, hi] = popI64();
    builder_.createStoreStackInst(lo, locals_[slot]);
    builder_.createStoreStackInst(hi, locals_[slot + 1]);
  } else {
    Value *val = pop();
    builder_.createStoreStackInst(val, locals_[slot]);
  }
}

void WasmIRGen::onLocalTee(uint32_t localIndex) {
  if (unreachable_)
    return;
  assert(localIndex < localSlotIndex_.size() && "localIndex out of range");
  uint32_t slot = localSlotIndex_[localIndex];
  if (localTypes_[localIndex] == WasmValType::I64) {
    auto [lo, hi] = popI64();
    builder_.createStoreStackInst(lo, locals_[slot]);
    builder_.createStoreStackInst(hi, locals_[slot + 1]);
    pushI64(lo, hi);
  } else {
    Value *val = pop();
    builder_.createStoreStackInst(val, locals_[slot]);
    push(val);
  }
}

// --- Return and drop (D.5) ---

void WasmIRGen::onReturn() {
  if (unreachable_)
    return;

  const WasmFuncType &funcType =
      moduleInfo_.getFunctionType(currentFuncIndex_);

  if (needsReturnBuffer(funcType)) {
    // Store all results to the return buffer and return 0.
    emitRetBufStores(funcType);
  } else if (!funcType.results.empty()) {
    // Single i32/f32/f64 result — pop and return directly.
    Value *result = pop();
    builder_.createReturnInst(result);
  } else {
    builder_.createReturnInst(builder_.getLiteralUndefined());
  }

  // After an unconditional return, code is unreachable.
  unreachable_ = true;

  // Create a new dead basic block for any dead code that follows.
  auto *deadBlock = builder_.createBasicBlock(currentFunc_);
  builder_.setInsertionBlock(deadBlock);
}

void WasmIRGen::onDrop() {
  if (unreachable_)
    return;
  if (isTopI64()) {
    popI64();
  } else {
    pop();
  }
}

// --- i32 arithmetic (D.3) ---

void WasmIRGen::onI32Add() {
  Value *rhs = asNumber(pop());
  Value *lhs = asNumber(pop());
  push(builder_.createAsInt32Inst(
      builder_.createFBinaryMathInst(ValueKind::FAddInstKind, lhs, rhs)));
}

void WasmIRGen::onI32Sub() {
  Value *rhs = asNumber(pop());
  Value *lhs = asNumber(pop());
  push(builder_.createAsInt32Inst(
      builder_.createFBinaryMathInst(
          ValueKind::FSubtractInstKind, lhs, rhs)));
}

void WasmIRGen::onI32Mul() {
  // Use Math.imul for correctness: double multiplication loses precision
  // for large int32 products (e.g., 65536 * 65536 overflows 53-bit mantissa).
  Value *rhs = pop();
  Value *lhs = pop();
  auto *imul = builder_.createCallBuiltinInst(
      BuiltinMethod::Math_imul, {lhs, rhs});
  imul->setType(Type::createNumber());
  push(imul);
}

void WasmIRGen::onI32And() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *inst = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryAndInstKind);
  inst->setType(Type::createNumber());
  push(inst);
}

void WasmIRGen::onI32Or() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *inst = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryOrInstKind);
  inst->setType(Type::createNumber());
  push(inst);
}

void WasmIRGen::onI32Xor() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *inst = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryXorInstKind);
  inst->setType(Type::createNumber());
  push(inst);
}

void WasmIRGen::onI32Shl() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *inst = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryLeftShiftInstKind);
  inst->setType(Type::createNumber());
  push(inst);
}

void WasmIRGen::onI32ShrS() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *inst = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryRightShiftInstKind);
  inst->setType(Type::createNumber());
  push(inst);
}

void WasmIRGen::onI32ShrU() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *inst = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryUnsignedRightShiftInstKind);
  inst->setType(Type::createNumber());
  push(inst);
}

// --- i32 trapping division (F.2) ---

void WasmIRGen::onI32DivS() {
  if (unreachable_)
    return;
  Value *rhs = pop();
  Value *lhs = pop();
  push(helpers_.emitI32DivS(lhs, rhs));
}

void WasmIRGen::onI32DivU() {
  if (unreachable_)
    return;
  Value *rhs = pop();
  Value *lhs = pop();
  push(helpers_.emitI32DivU(lhs, rhs));
}

void WasmIRGen::onI32RemS() {
  if (unreachable_)
    return;
  Value *rhs = pop();
  Value *lhs = pop();
  push(helpers_.emitI32RemS(lhs, rhs));
}

void WasmIRGen::onI32RemU() {
  if (unreachable_)
    return;
  Value *rhs = pop();
  Value *lhs = pop();
  push(helpers_.emitI32RemU(lhs, rhs));
}

// --- i32 bit manipulation (F.3) ---

void WasmIRGen::onI32Clz() {
  if (unreachable_)
    return;
  Value *a = pop();
  push(helpers_.emitI32Clz(a));
}

void WasmIRGen::onI32Ctz() {
  if (unreachable_)
    return;
  Value *a = pop();
  push(helpers_.emitI32Ctz(a));
}

void WasmIRGen::onI32Popcnt() {
  if (unreachable_)
    return;
  Value *a = pop();
  push(helpers_.emitI32Popcnt(a));
}

void WasmIRGen::onI32Rotl() {
  if (unreachable_)
    return;
  Value *rhs = pop();
  Value *lhs = pop();
  push(helpers_.emitI32Rotl(lhs, rhs));
}

void WasmIRGen::onI32Rotr() {
  if (unreachable_)
    return;
  Value *rhs = pop();
  Value *lhs = pop();
  push(helpers_.emitI32Rotr(lhs, rhs));
}

void WasmIRGen::onI32Extend8S() {
  if (unreachable_)
    return;
  Value *a = pop();
  // Sign-extend from 8 bits: (a << 24) >> 24
  auto *shifted = builder_.createBinaryOperatorInst(
      a, builder_.getLiteralNumber(24), ValueKind::BinaryLeftShiftInstKind);
  shifted->setType(Type::createNumber());
  auto *result = builder_.createBinaryOperatorInst(
      shifted,
      builder_.getLiteralNumber(24),
      ValueKind::BinaryRightShiftInstKind);
  result->setType(Type::createNumber());
  push(result);
}

void WasmIRGen::onI32Extend16S() {
  if (unreachable_)
    return;
  Value *a = pop();
  // Sign-extend from 16 bits: (a << 16) >> 16
  auto *shifted = builder_.createBinaryOperatorInst(
      a, builder_.getLiteralNumber(16), ValueKind::BinaryLeftShiftInstKind);
  shifted->setType(Type::createNumber());
  auto *result = builder_.createBinaryOperatorInst(
      shifted,
      builder_.getLiteralNumber(16),
      ValueKind::BinaryRightShiftInstKind);
  result->setType(Type::createNumber());
  push(result);
}

// --- i32 comparisons (D.4) ---

void WasmIRGen::onI32Eq() {
  Value *rhs = asNumber(pop());
  Value *lhs = asNumber(pop());
  auto *cmp = builder_.createFCompareInst(
      ValueKind::FEqualInstKind, lhs, rhs);
  // Convert boolean to i32 (true→1, false→0).
  auto *asI32 = builder_.createAsInt32Inst(cmp);
  push(asI32);
}

void WasmIRGen::onI32Ne() {
  Value *rhs = asNumber(pop());
  Value *lhs = asNumber(pop());
  auto *cmp = builder_.createFCompareInst(
      ValueKind::FNotEqualInstKind, lhs, rhs);
  auto *asI32 = builder_.createAsInt32Inst(cmp);
  push(asI32);
}

void WasmIRGen::onI32LtS() {
  Value *rhs = pop();
  Value *lhs = pop();
  // Signed: cast both operands to int32 before comparing.
  auto *lhsI32 = builder_.createAsInt32Inst(lhs);
  auto *rhsI32 = builder_.createAsInt32Inst(rhs);
  auto *cmp = builder_.createFCompareInst(
      ValueKind::FLessThanInstKind, lhsI32, rhsI32);
  auto *asI32 = builder_.createAsInt32Inst(cmp);
  push(asI32);
}

void WasmIRGen::onI32GtS() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *lhsI32 = builder_.createAsInt32Inst(lhs);
  auto *rhsI32 = builder_.createAsInt32Inst(rhs);
  auto *cmp = builder_.createFCompareInst(
      ValueKind::FGreaterThanInstKind, lhsI32, rhsI32);
  auto *asI32 = builder_.createAsInt32Inst(cmp);
  push(asI32);
}

void WasmIRGen::onI32LeS() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *lhsI32 = builder_.createAsInt32Inst(lhs);
  auto *rhsI32 = builder_.createAsInt32Inst(rhs);
  auto *cmp = builder_.createFCompareInst(
      ValueKind::FLessThanOrEqualInstKind, lhsI32, rhsI32);
  auto *asI32 = builder_.createAsInt32Inst(cmp);
  push(asI32);
}

void WasmIRGen::onI32GeS() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *lhsI32 = builder_.createAsInt32Inst(lhs);
  auto *rhsI32 = builder_.createAsInt32Inst(rhs);
  auto *cmp = builder_.createFCompareInst(
      ValueKind::FGreaterThanOrEqualInstKind, lhsI32, rhsI32);
  auto *asI32 = builder_.createAsInt32Inst(cmp);
  push(asI32);
}

void WasmIRGen::onI32LtU() {
  Value *rhs = pop();
  Value *lhs = pop();
  // Unsigned: cast both operands to uint32 before comparing.
  auto *lhsU32 = builder_.createAsUint32Inst(lhs);
  auto *rhsU32 = builder_.createAsUint32Inst(rhs);
  auto *cmp = builder_.createFCompareInst(
      ValueKind::FLessThanInstKind, lhsU32, rhsU32);
  auto *asI32 = builder_.createAsInt32Inst(cmp);
  push(asI32);
}

void WasmIRGen::onI32GtU() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *lhsU32 = builder_.createAsUint32Inst(lhs);
  auto *rhsU32 = builder_.createAsUint32Inst(rhs);
  auto *cmp = builder_.createFCompareInst(
      ValueKind::FGreaterThanInstKind, lhsU32, rhsU32);
  auto *asI32 = builder_.createAsInt32Inst(cmp);
  push(asI32);
}

void WasmIRGen::onI32LeU() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *lhsU32 = builder_.createAsUint32Inst(lhs);
  auto *rhsU32 = builder_.createAsUint32Inst(rhs);
  auto *cmp = builder_.createFCompareInst(
      ValueKind::FLessThanOrEqualInstKind, lhsU32, rhsU32);
  auto *asI32 = builder_.createAsInt32Inst(cmp);
  push(asI32);
}

void WasmIRGen::onI32GeU() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *lhsU32 = builder_.createAsUint32Inst(lhs);
  auto *rhsU32 = builder_.createAsUint32Inst(rhs);
  auto *cmp = builder_.createFCompareInst(
      ValueKind::FGreaterThanOrEqualInstKind, lhsU32, rhsU32);
  auto *asI32 = builder_.createAsInt32Inst(cmp);
  push(asI32);
}

void WasmIRGen::onI32Eqz() {
  Value *val = asNumber(pop());
  // eqz(x) == (x === 0) → boolean → i32.
  auto *cmp = builder_.createFCompareInst(
      ValueKind::FEqualInstKind,
      val,
      builder_.getLiteralNumber(0));
  auto *asI32 = builder_.createAsInt32Inst(cmp);
  push(asI32);
}

// --- Control flow (D.6, D.7) ---

void WasmIRGen::onBlock(
    const std::vector<WasmValType> &paramTypes,
    const std::vector<WasmValType> &resultTypes) {
  // Count param stack slots (i64 params use 2 slots).
  size_t numParamSlots = 0;
  for (auto t : paramTypes) {
    numParamSlots += (t == WasmValType::I64) ? 2 : 1;
  }

  if (unreachable_) {
    // In unreachable code, push a lightweight entry so onEnd can pop it.
    ControlEntry entry;
    entry.kind = ControlEntry::Block;
    entry.contBlock = nullptr;
    entry.paramTypes = paramTypes;
    entry.resultTypes = resultTypes;
    entry.stackHeight = valueStack_.size();
    entry.outerUnreachable = true;
    controlStack_.push_back(std::move(entry));
    return;
  }

  // Create a continuation basic block (target of br 0 = after end).
  auto *contBlock = builder_.createBasicBlock(currentFunc_);

  ControlEntry entry;
  entry.kind = ControlEntry::Block;
  entry.contBlock = contBlock;
  entry.paramTypes = paramTypes;
  entry.resultTypes = resultTypes;
  // Stack height is set below the params so they are part of the block's
  // accessible stack.
  entry.stackHeight = valueStack_.size() - numParamSlots;
  entry.outerUnreachable = false;

  // Create phi nodes in the continuation block (i64 results get 2 phis).
  if (!resultTypes.empty()) {
    entry.resultPhis = createResultPhis(contBlock, resultTypes);
  }

  controlStack_.push_back(std::move(entry));
}

void WasmIRGen::onLoop(
    const std::vector<WasmValType> &paramTypes,
    const std::vector<WasmValType> &resultTypes) {
  // Count param stack slots (i64 params use 2 slots).
  size_t numParamSlots = 0;
  for (auto t : paramTypes) {
    numParamSlots += (t == WasmValType::I64) ? 2 : 1;
  }

  if (unreachable_) {
    // In unreachable code, push a lightweight entry so onEnd can pop it.
    ControlEntry entry;
    entry.kind = ControlEntry::Loop;
    entry.contBlock = nullptr;
    entry.endBlock = nullptr;
    entry.paramTypes = paramTypes;
    entry.resultTypes = resultTypes;
    entry.stackHeight = valueStack_.size();
    entry.outerUnreachable = true;
    controlStack_.push_back(std::move(entry));
    return;
  }

  // Create the loop header block. br targeting this loop jumps here.
  auto *headerBlock = builder_.createBasicBlock(currentFunc_);

  // Create the end block (after the loop's end, where fallthrough goes).
  auto *endBlock = builder_.createBasicBlock(currentFunc_);

  ControlEntry entry;
  entry.kind = ControlEntry::Loop;
  entry.contBlock = headerBlock; // br targets the loop header
  entry.endBlock = endBlock; // fallthrough after end goes here
  entry.paramTypes = paramTypes;
  entry.resultTypes = resultTypes;
  // Stack height is set below the params.
  entry.stackHeight = valueStack_.size() - numParamSlots;
  entry.outerUnreachable = false;

  // Result phis go in the end block (for fallthrough values).
  // i64 results get 2 phis each.
  if (!resultTypes.empty()) {
    entry.resultPhis = createResultPhis(endBlock, resultTypes);
  }

  // Save the pre-loop block for phi entries.
  auto *preLoopBlock = builder_.getInsertionBlock();

  // Branch from the current block to the loop header.
  if (!isCurrentBlockTerminated()) {
    builder_.createBranchInst(headerBlock);
  }

  // Set insertion point to the loop header.
  builder_.setInsertionBlock(headerBlock);

  // Create phi nodes in the header block for loop parameters.
  // br/br_if targeting this loop will pass updated values via these phis.
  if (!paramTypes.empty()) {
    entry.paramPhis = createResultPhis(headerBlock, paramTypes);

    // Add initial values from the pre-loop block.
    size_t numPhis = entry.paramPhis.size();
    size_t stackTop = valueStack_.size();
    for (size_t i = 0; i < numPhis; ++i) {
      entry.paramPhis[i]->addEntry(
          valueStack_[stackTop - numPhis + i], preLoopBlock);
    }

    // Replace the param values on the stack with the phi nodes,
    // so the loop body uses the phis (updated on each back-edge).
    for (size_t i = 0; i < numPhis; ++i) {
      valueStack_[stackTop - numPhis + i] = entry.paramPhis[i];
    }
  }

  controlStack_.push_back(std::move(entry));
}

void WasmIRGen::onIf(
    const std::vector<WasmValType> &paramTypes,
    const std::vector<WasmValType> &resultTypes) {
  // Count param stack slots (i64 params use 2 slots).
  size_t numParamSlots = 0;
  for (auto t : paramTypes) {
    numParamSlots += (t == WasmValType::I64) ? 2 : 1;
  }

  if (unreachable_) {
    // Push a dummy If entry so onEnd/onElse can pop it.
    ControlEntry entry;
    entry.kind = ControlEntry::If;
    entry.contBlock = nullptr;
    entry.elseBlock = nullptr;
    entry.paramTypes = paramTypes;
    entry.resultTypes = resultTypes;
    entry.stackHeight = valueStack_.size();
    entry.outerUnreachable = true;
    controlStack_.push_back(std::move(entry));
    return;
  }

  // Pop the condition.
  Value *cond = peekThroughAsInt32(pop());

  // Create thenBlock, elseBlock, mergeBlock.
  auto *thenBlock = builder_.createBasicBlock(currentFunc_);
  auto *elseBlock = builder_.createBasicBlock(currentFunc_);
  auto *mergeBlock = builder_.createBasicBlock(currentFunc_);

  // Emit CondBranchInst: non-zero → thenBlock, zero → elseBlock.
  builder_.createCondBranchInst(cond, thenBlock, elseBlock);

  ControlEntry entry;
  entry.kind = ControlEntry::If;
  entry.contBlock = mergeBlock; // br target and end continuation
  entry.elseBlock = elseBlock;
  entry.paramTypes = paramTypes;
  entry.resultTypes = resultTypes;
  entry.outerUnreachable = unreachable_;

  // Stack height is set below the params so they are accessible inside
  // the block body. Save the param values so they can be re-pushed
  // at the start of the else branch.
  entry.stackHeight = valueStack_.size() - numParamSlots;
  if (numParamSlots > 0) {
    entry.savedParamValues.assign(
        valueStack_.end() - numParamSlots, valueStack_.end());
  }

  // Create phi nodes in the merge block for results (i64 results get 2 phis).
  if (!resultTypes.empty()) {
    entry.resultPhis = createResultPhis(mergeBlock, resultTypes);
  }

  controlStack_.push_back(std::move(entry));

  // Set insertion point to the thenBlock.
  builder_.setInsertionBlock(thenBlock);
}

void WasmIRGen::onElse() {
  assert(!controlStack_.empty() && "control stack underflow");
  ControlEntry &entry = controlStack_.back();
  assert(entry.kind == ControlEntry::If && "onElse without matching if");

  if (!entry.outerUnreachable) {
    bool fallsThrough = !unreachable_ && !isCurrentBlockTerminated();

    if (fallsThrough) {
      // The then-block falls through to mergeBlock.
      addBranchPhiOperands(entry);
      builder_.createBranchInst(entry.contBlock);
      // Mark that the merge block has been targeted (from the then arm),
      // so onEnd knows the merge block is reachable.
      entry.branchTargeted = true;
    }

    // Set insertion point to the elseBlock.
    builder_.setInsertionBlock(entry.elseBlock);
  }

  // Restore the value stack to the entry height (discard then-block values).
  valueStack_.resize(entry.stackHeight);
  valueStackIsI64Hi_.resize(entry.stackHeight);

  // Reset unreachable for the else arm BEFORE re-pushing params,
  // because push() is a no-op when unreachable_ is true.
  unreachable_ = entry.outerUnreachable;

  // Re-push the saved param values for the else branch.
  if (!entry.savedParamValues.empty()) {
    size_t paramIdx = 0;
    for (auto t : entry.paramTypes) {
      push(entry.savedParamValues[paramIdx++]);
      if (t == WasmValType::I64) {
        push(entry.savedParamValues[paramIdx++]);
        valueStackIsI64Hi_.back() = true;
      }
    }
  }

  // Mark that we've consumed the else block (so onEnd knows).
  entry.elseBlock = nullptr;
}

void WasmIRGen::onEnd() {
  assert(!controlStack_.empty() && "control stack underflow");
  ControlEntry entry = std::move(controlStack_.back());
  controlStack_.pop_back();

  if (entry.kind == ControlEntry::Try) {
    if (entry.outerUnreachable) {
      // Try was pushed in unreachable context — no real IR generated.
      valueStack_.resize(entry.stackHeight);
      valueStackIsI64Hi_.resize(entry.stackHeight);
      for (auto t : entry.resultTypes) {
        if (t == WasmValType::I64) {
          push(builder_.getLiteralUndefined());
          push(builder_.getLiteralUndefined());
          valueStackIsI64Hi_.back() = true;
        } else {
          push(builder_.getLiteralUndefined());
        }
      }
      unreachable_ = true;
      return;
    }

    bool fallsThrough = !unreachable_ && !isCurrentBlockTerminated();

    if (!entry.inCatch) {
      // Try with no catch clauses (shouldn't happen in valid Wasm, but
      // handle gracefully). End the try body.
      if (fallsThrough) {
        addBranchPhiOperands(entry);
        builder_.createTryEndInst(entry.catchBlock, entry.contBlock);
      }
      // The catch block needs at least a CatchInst + rethrow.
      auto *savedBlock = builder_.getInsertionBlock();
      builder_.setInsertionBlock(entry.catchBlock);
      auto *caught = builder_.createCatchInst();
      builder_.createThrowInst(caught);
      builder_.setInsertionBlock(savedBlock);

      builder_.setInsertionBlock(entry.contBlock);
      unreachable_ =
          !fallsThrough && !entry.branchTargeted;
    } else {
      // Try with catch clauses. End the current handler.
      if (fallsThrough) {
        addBranchPhiOperands(entry);
        builder_.createBranchInst(entry.contBlock);
      }

      // If there's no catch_all, the nextCatchBlock re-throws.
      if (!entry.hasCatchAll && entry.nextCatchBlock) {
        auto *savedBlock = builder_.getInsertionBlock();
        builder_.setInsertionBlock(entry.nextCatchBlock);
        builder_.createThrowInst(entry.caughtValue);
        builder_.setInsertionBlock(savedBlock);
      }

      builder_.setInsertionBlock(entry.contBlock);
      // contBlock is reachable if any handler fell through, or if br targeted
      // it, or if the catch_all handler reached it.
      unreachable_ = !fallsThrough && !entry.branchTargeted;
    }

    // Restore value stack and push result phis.
    valueStack_.resize(entry.stackHeight);
    valueStackIsI64Hi_.resize(entry.stackHeight);
    pushResultPhis(entry);
  } else if (entry.kind == ControlEntry::Block || entry.kind == ControlEntry::If) {
    if (entry.outerUnreachable) {
      // This block/if was entered in unreachable context (e.g., inside dead
      // code after a br/return). No real IR was generated. Just restore state
      // and remain unreachable. Push placeholder values for the result types
      // so the value stack has the right shape for outer code.
      valueStack_.resize(entry.stackHeight);
      valueStackIsI64Hi_.resize(entry.stackHeight);
      for (auto t : entry.resultTypes) {
        if (t == WasmValType::I64) {
          push(builder_.getLiteralUndefined());
          push(builder_.getLiteralUndefined());
          valueStackIsI64Hi_.back() = true;
        } else {
          push(builder_.getLiteralUndefined());
        }
      }
      unreachable_ = true;
      return;
    }

    bool fallsThrough = !unreachable_ && !isCurrentBlockTerminated();

    if (fallsThrough) {
      // Add phi operands from the fallthrough path.
      addBranchPhiOperands(entry);
      builder_.createBranchInst(entry.contBlock);
    }

    // For If without else: the else block branches directly to merge.
    // If the block has params, pass them through as results (Wasm validation
    // guarantees params == results for if-without-else).
    if (entry.kind == ControlEntry::If && entry.elseBlock != nullptr) {
      auto *savedBlock = builder_.getInsertionBlock();
      builder_.setInsertionBlock(entry.elseBlock);
      if (!entry.resultPhis.empty() && !entry.savedParamValues.empty()) {
        for (size_t i = 0; i < entry.resultPhis.size(); ++i) {
          Value *val = (i < entry.savedParamValues.size())
              ? entry.savedParamValues[i]
              : builder_.getLiteralUndefined();
          entry.resultPhis[i]->addEntry(val, entry.elseBlock);
        }
      }
      builder_.createBranchInst(entry.contBlock);
      builder_.setInsertionBlock(savedBlock);
    }

    // Set insertion point to the continuation block.
    builder_.setInsertionBlock(entry.contBlock);

    // The continuation block is reachable if we fell through, if any
    // branch (br/br_if) targeted this block, or if there was an if
    // without else (the else path always reaches merge).
    bool ifWithoutElse =
        entry.kind == ControlEntry::If && entry.elseBlock != nullptr;
    unreachable_ = !fallsThrough && !entry.branchTargeted && !ifWithoutElse;

    // Restore the value stack to the height it was at when the block started.
    // This removes any leftover values from unreachable code paths inside the
    // block (e.g., a nested block whose result was pushed but is dead).
    valueStack_.resize(entry.stackHeight);
    valueStackIsI64Hi_.resize(entry.stackHeight);

    // Push phi results onto the value stack (i64 results push 2 values).
    pushResultPhis(entry);
  } else if (entry.kind == ControlEntry::Loop) {
    if (entry.outerUnreachable) {
      // Loop entered in unreachable context — no real IR generated.
      valueStack_.resize(entry.stackHeight);
      valueStackIsI64Hi_.resize(entry.stackHeight);
      for (auto t : entry.resultTypes) {
        if (t == WasmValType::I64) {
          push(builder_.getLiteralUndefined());
          push(builder_.getLiteralUndefined());
          valueStackIsI64Hi_.back() = true;
        } else {
          push(builder_.getLiteralUndefined());
        }
      }
      unreachable_ = true;
      return;
    }

    bool fallsThrough = !unreachable_ && !isCurrentBlockTerminated();

    if (fallsThrough) {
      // Add phi operands to the end block from the fallthrough path.
      // We handle this directly here rather than via addBranchPhiOperands,
      // because addBranchPhiOperands skips Loop entries (since br to a
      // loop targets the header, not the end block).
      if (!entry.resultPhis.empty()) {
        auto *currentBlock = builder_.getInsertionBlock();
        size_t numPhis = entry.resultPhis.size();
        size_t available = valueStack_.size();
        if (available >= numPhis) {
          for (size_t i = 0; i < numPhis; ++i) {
            Value *val = valueStack_[available - numPhis + i];
            entry.resultPhis[i]->addEntry(val, currentBlock);
          }
          valueStack_.resize(available - numPhis);
          valueStackIsI64Hi_.resize(available - numPhis);
        } else {
          // Stack underflow — use undefined as placeholder.
          for (size_t i = 0; i < numPhis; ++i) {
            Value *val = (i >= numPhis - available)
                ? valueStack_[i - (numPhis - available)]
                : builder_.getLiteralUndefined();
            entry.resultPhis[i]->addEntry(val, currentBlock);
          }
          valueStack_.clear();
          valueStackIsI64Hi_.clear();
        }
      }
      builder_.createBranchInst(entry.endBlock);
    }

    // Set insertion point to the end block (after the loop).
    builder_.setInsertionBlock(entry.endBlock);

    // The end block is reachable if we fell through.
    // Note: branchTargeted only tracks br to the loop header; it does NOT
    // make the end block reachable. Only fallthrough makes it reachable.
    unreachable_ = !fallsThrough;

    // Push phi results onto the value stack (i64 results push 2 values).
    pushResultPhis(entry);
  }
}

void WasmIRGen::onBr(uint32_t depth) {
  if (unreachable_)
    return;

  ControlEntry &entry = getControlEntry(depth);
  entry.branchTargeted = true;

  // Add phi operands for the branch target.
  addBranchPhiOperands(entry);

  // Branch to the target.
  builder_.createBranchInst(entry.contBlock);

  // After an unconditional branch, code is unreachable.
  unreachable_ = true;

  // Create a new dead basic block for any dead code that follows.
  auto *deadBlock = builder_.createBasicBlock(currentFunc_);
  builder_.setInsertionBlock(deadBlock);
}

void WasmIRGen::onBrIf(uint32_t depth) {
  if (unreachable_)
    return;

  ControlEntry &entry = getControlEntry(depth);
  entry.branchTargeted = true;

  // Pop the condition.
  Value *cond = peekThroughAsInt32(pop());

  // Create a fallthrough block for when the condition is false.
  auto *fallthroughBlock = builder_.createBasicBlock(currentFunc_);

  // If the block has results, peek at the value stack (don't pop) and add
  // phi operands from the branch-taken path. Values stay for fallthrough.
  peekBranchPhiOperands(entry);

  // Emit conditional branch: non-zero condition branches to target.
  builder_.createCondBranchInst(cond, entry.contBlock, fallthroughBlock);

  // Continue generating code in the fallthrough block.
  builder_.setInsertionBlock(fallthroughBlock);
}

void WasmIRGen::onBrTable(
    const uint32_t *depths,
    uint32_t numTargets,
    uint32_t defaultDepth) {
  if (unreachable_)
    return;

  // Pop the index value.
  Value *index = pop();

  // For each target (including the default), we need to create a trampoline
  // block that adds phi operands to the target's continuation block and then
  // branches there. We use a SwitchInst to dispatch to these trampolines.

  // Collect all unique depths and create trampoline blocks.
  // Multiple case values may share the same depth. We can reuse the same
  // trampoline block for cases with the same depth.
  llvh::DenseMap<uint32_t, BasicBlock *> depthToTrampoline;

  auto getOrCreateTrampoline = [&](uint32_t depth) -> BasicBlock * {
    auto it = depthToTrampoline.find(depth);
    if (it != depthToTrampoline.end())
      return it->second;
    auto *trampoline = builder_.createBasicBlock(currentFunc_);
    depthToTrampoline[depth] = trampoline;
    return trampoline;
  };

  // Create trampoline blocks for all targets.
  llvh::SmallVector<Literal *, 8> caseValues;
  llvh::SmallVector<BasicBlock *, 8> caseBlocks;
  for (uint32_t i = 0; i < numTargets; ++i) {
    caseValues.push_back(builder_.getLiteralNumber(static_cast<double>(i)));
    caseBlocks.push_back(getOrCreateTrampoline(depths[i]));
  }

  BasicBlock *defaultTrampoline = getOrCreateTrampoline(defaultDepth);

  if (numTargets == 0) {
    // No case targets — just branch to the default trampoline.
    builder_.createBranchInst(defaultTrampoline);
  } else {
    // Emit the SwitchInst.
    builder_.createSwitchInst(index, defaultTrampoline, caseValues, caseBlocks);
  }

  // Now populate each trampoline block with phi operands and branch.
  for (auto &pair : depthToTrampoline) {
    uint32_t depth = pair.first;
    BasicBlock *trampoline = pair.second;

    ControlEntry &entry = getControlEntry(depth);
    entry.branchTargeted = true;

    builder_.setInsertionBlock(trampoline);

    // Add phi operands. For Block/If entries, peek at the value stack and
    // add values as phi incoming edges (the values were on the stack before
    // the index was popped, so they're still there).
    if ((entry.kind == ControlEntry::Block ||
         entry.kind == ControlEntry::If) &&
        !entry.resultPhis.empty()) {
      size_t numPhis = entry.resultPhis.size();
      size_t available = valueStack_.size();
      for (size_t i = 0; i < numPhis; ++i) {
        Value *val;
        if (available >= numPhis) {
          val = valueStack_[available - numPhis + i];
        } else {
          val = builder_.getLiteralUndefined();
        }
        entry.resultPhis[i]->addEntry(val, trampoline);
      }
    }
    // For Loop entries, br targets the header and passes param values.
    if (entry.kind == ControlEntry::Loop && !entry.paramPhis.empty()) {
      size_t numPhis = entry.paramPhis.size();
      size_t available = valueStack_.size();
      for (size_t i = 0; i < numPhis; ++i) {
        Value *val;
        if (available >= numPhis) {
          val = valueStack_[available - numPhis + i];
        } else {
          val = builder_.getLiteralUndefined();
        }
        entry.paramPhis[i]->addEntry(val, trampoline);
      }
    }

    builder_.createBranchInst(entry.contBlock);
  }

  // After br_table, code is unreachable.
  unreachable_ = true;

  // Create a new dead basic block for any dead code that follows.
  auto *deadBlock = builder_.createBasicBlock(currentFunc_);
  builder_.setInsertionBlock(deadBlock);
}

// --- Parametric instructions (D.10) ---

void WasmIRGen::onSelect() {
  if (unreachable_)
    return;

  Value *cond = pop();

  // Check if the values are i64 (val2 is on top, then val1 below).
  bool isI64 = isTopI64();

  if (isI64) {
    auto [lo2, hi2] = popI64(); // value if cond == 0 (false)
    auto [lo1, hi1] = popI64(); // value if cond != 0 (true)

    auto *trueBlock = builder_.createBasicBlock(currentFunc_);
    auto *falseBlock = builder_.createBasicBlock(currentFunc_);
    auto *mergeBlock = builder_.createBasicBlock(currentFunc_);

    builder_.createCondBranchInst(cond, trueBlock, falseBlock);

    builder_.setInsertionBlock(trueBlock);
    builder_.createBranchInst(mergeBlock);

    builder_.setInsertionBlock(falseBlock);
    builder_.createBranchInst(mergeBlock);

    builder_.setInsertionBlock(mergeBlock);
    auto *phiLo = builder_.createPhiInst();
    phiLo->addEntry(lo1, trueBlock);
    phiLo->addEntry(lo2, falseBlock);
    auto *phiHi = builder_.createPhiInst();
    phiHi->addEntry(hi1, trueBlock);
    phiHi->addEntry(hi2, falseBlock);

    pushI64(phiLo, phiHi);
  } else {
    Value *val2 = pop(); // value if cond == 0 (false)
    Value *val1 = pop(); // value if cond != 0 (true)

    auto *trueBlock = builder_.createBasicBlock(currentFunc_);
    auto *falseBlock = builder_.createBasicBlock(currentFunc_);
    auto *mergeBlock = builder_.createBasicBlock(currentFunc_);

    builder_.createCondBranchInst(cond, trueBlock, falseBlock);

    builder_.setInsertionBlock(trueBlock);
    builder_.createBranchInst(mergeBlock);

    builder_.setInsertionBlock(falseBlock);
    builder_.createBranchInst(mergeBlock);

    builder_.setInsertionBlock(mergeBlock);
    auto *phi = builder_.createPhiInst();
    phi->addEntry(val1, trueBlock);
    phi->addEntry(val2, falseBlock);

    push(phi);
  }
}

// --- Function calls (D.12) ---

void WasmIRGen::onCall(uint32_t funcIndex) {
  if (unreachable_)
    return;

  assert(
      funcIndex < irFunctions_.size() &&
      "call funcIndex out of range");

  // Look up the called function's type signature.
  const WasmFuncType &funcType = moduleInfo_.getFunctionType(funcIndex);

  // Pop arguments from the value stack in reverse order.
  // Wasm pushes args left-to-right, so the last arg is on top.
  // i64 params occupy 2 stack slots (lo, hi) and become 2 JS args.
  // First, collect the Wasm-level values in reverse order.
  llvh::SmallVector<Value *, 8> args;
  // Temporary storage for popped values in reverse Wasm param order.
  llvh::SmallVector<std::pair<Value *, Value *>, 8> wasmArgs(
      funcType.params.size());
  for (uint32_t i = funcType.params.size(); i > 0; --i) {
    if (funcType.params[i - 1] == WasmValType::I64) {
      wasmArgs[i - 1] = popI64();
    } else {
      wasmArgs[i - 1] = {pop(), nullptr};
    }
  }
  // Build the JS arg list in forward order.
  // If the callee needs a return buffer, prepend retBufI and retBufF.
  if (needsReturnBuffer(funcType)) {
    auto *rbI = builder_.createLoadFrameInst(
        parentScopeInst_, retBufIVar_);
    auto *rbF = builder_.createLoadFrameInst(
        parentScopeInst_, retBufFVar_);
    args.push_back(rbI);
    args.push_back(rbF);
  }
  for (uint32_t i = 0; i < funcType.params.size(); ++i) {
    if (funcType.params[i] == WasmValType::I64) {
      args.push_back(wasmArgs[i].first); // lo
      args.push_back(wasmArgs[i].second); // hi
    } else {
      args.push_back(wasmArgs[i].first);
    }
  }

  // Load the pre-created closure from the top-level environment.
  auto *closure = builder_.createLoadFrameInst(
      parentScopeInst_, closureVars_[funcIndex]);
  auto *call = builder_.createCallInst(
      closure,
      /* target */ irFunctions_[funcIndex],
      /* calleeIsAlwaysClosure */ true,
      /* env */ builder_.getEmptySentinel(),
      /* newTarget */ builder_.getLiteralUndefined(),
      /* thisValue */ builder_.getLiteralUndefined(),
      args);

  // Set the return type on the CallInst based on the known callee signature.
  if (funcType.results.empty())
    call->setType(Type::createUndefined());
  else if (!needsReturnBuffer(funcType))
    call->setType(wasmValTypeToIRType(funcType.results[0]));
  else
    call->setType(Type::createNumber());

  // Push return values onto the stack.
  if (needsReturnBuffer(funcType)) {
    // All results are in the return buffer. Read them out.
    emitRetBufLoads(funcType);
  } else if (!funcType.results.empty()) {
    // Single non-buffer result: push the JS return value.
    push(call);
  }
}

void WasmIRGen::onCallIndirect(uint32_t sigIndex, uint32_t tableIndex) {
  if (unreachable_)
    return;

  assert(
      sigIndex < moduleInfo_.types.size() &&
      "call_indirect sigIndex out of range");
  assert(
      tableIndex < tableFuncVars_.size() &&
      "call_indirect tableIndex out of range");

  const WasmFuncType &funcType = moduleInfo_.types[sigIndex];

  // Pop the table element index (always i32, on top of the args).
  auto *tableIdx = pop();

  // Pop arguments from the value stack in reverse order (same as onCall).
  llvh::SmallVector<Value *, 8> args;
  llvh::SmallVector<std::pair<Value *, Value *>, 8> wasmArgs(
      funcType.params.size());
  for (uint32_t i = funcType.params.size(); i > 0; --i) {
    if (funcType.params[i - 1] == WasmValType::I64) {
      wasmArgs[i - 1] = popI64();
    } else {
      wasmArgs[i - 1] = {pop(), nullptr};
    }
  }
  // Build the JS arg list in forward order.
  // If the callee needs a return buffer, prepend retBufI and retBufF.
  if (needsReturnBuffer(funcType)) {
    auto *rbI = builder_.createLoadFrameInst(
        parentScopeInst_, retBufIVar_);
    auto *rbF = builder_.createLoadFrameInst(
        parentScopeInst_, retBufFVar_);
    args.push_back(rbI);
    args.push_back(rbF);
  }
  for (uint32_t i = 0; i < funcType.params.size(); ++i) {
    if (funcType.params[i] == WasmValType::I64) {
      args.push_back(wasmArgs[i].first); // lo
      args.push_back(wasmArgs[i].second); // hi
    } else {
      args.push_back(wasmArgs[i].first);
    }
  }

  // Load table arrays from the top-level scope.
  auto *funcsArr = loadTableFuncs(tableIndex);
  auto *typesArr = loadTableTypes(tableIndex);

  // Call the builtin helper to validate and get the closure.
  // Takes (funcsArr, typesArr, index, expectedTypeIdx).
  // Compare interned ids, which agree across modules. A module-local index
  // would not: the same signature can be numbered differently in another
  // module (a spurious trap) and different signatures identically (a missed
  // trap). Still a plain integer compare in the builtin.
  auto *sigIdxLit =
      builder_.createLoadFrameInst(parentScopeInst_, typeIdVars_[sigIndex]);
  auto *closure =
      helpers_.emitCallIndirect(funcsArr, typesArr, tableIdx, sigIdxLit);

  // Call the validated closure with the popped arguments.
  auto *call = builder_.createCallInst(
      closure,
      /* newTarget */ builder_.getLiteralUndefined(),
      /* thisValue */ builder_.getLiteralUndefined(),
      args);

  // Set the return type based on the call_indirect signature.
  if (funcType.results.empty())
    call->setType(Type::createUndefined());
  else if (!needsReturnBuffer(funcType))
    call->setType(wasmValTypeToIRType(funcType.results[0]));
  else
    call->setType(Type::createNumber());

  // Push return values onto the stack.
  if (needsReturnBuffer(funcType)) {
    // All results are in the return buffer. Read them out.
    emitRetBufLoads(funcType);
  } else if (!funcType.results.empty()) {
    // Single non-buffer result: push the JS return value.
    push(call);
  }
}

// --- i64 arithmetic (G.3) ---
// i64 values are represented as two i32 values on the stack [lo, hi].
// Binary operations pop two i64 pairs and push one i64 pair.
// For operations that need a native helper: the helper takes retBufI as
// its first arg and writes lo/hi to retBufI[0]/[1].

void WasmIRGen::onI64Add() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  helpers_.emitI64Add(retBufI_, loA, hiA, loB, hiB);
  auto [lo, hi] = readI64FromRetBuf();
  pushI64(lo, hi);
}

void WasmIRGen::onI64Sub() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  helpers_.emitI64Sub(retBufI_, loA, hiA, loB, hiB);
  auto [lo, hi] = readI64FromRetBuf();
  pushI64(lo, hi);
}

void WasmIRGen::onI64Mul() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  helpers_.emitI64Mul(retBufI_, loA, hiA, loB, hiB);
  auto [lo, hi] = readI64FromRetBuf();
  pushI64(lo, hi);
}

void WasmIRGen::onI64DivS() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  helpers_.emitI64DivS(retBufI_, loA, hiA, loB, hiB);
  auto [lo, hi] = readI64FromRetBuf();
  pushI64(lo, hi);
}

void WasmIRGen::onI64DivU() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  helpers_.emitI64DivU(retBufI_, loA, hiA, loB, hiB);
  auto [lo, hi] = readI64FromRetBuf();
  pushI64(lo, hi);
}

void WasmIRGen::onI64RemS() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  helpers_.emitI64RemS(retBufI_, loA, hiA, loB, hiB);
  auto [lo, hi] = readI64FromRetBuf();
  pushI64(lo, hi);
}

void WasmIRGen::onI64RemU() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  helpers_.emitI64RemU(retBufI_, loA, hiA, loB, hiB);
  auto [lo, hi] = readI64FromRetBuf();
  pushI64(lo, hi);
}

void WasmIRGen::onI64And() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  auto *lo = builder_.createBinaryOperatorInst(
      loA, loB, ValueKind::BinaryAndInstKind);
  lo->setType(Type::createNumber());
  auto *hi = builder_.createBinaryOperatorInst(
      hiA, hiB, ValueKind::BinaryAndInstKind);
  hi->setType(Type::createNumber());
  pushI64(lo, hi);
}

void WasmIRGen::onI64Or() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  auto *lo = builder_.createBinaryOperatorInst(
      loA, loB, ValueKind::BinaryOrInstKind);
  lo->setType(Type::createNumber());
  auto *hi = builder_.createBinaryOperatorInst(
      hiA, hiB, ValueKind::BinaryOrInstKind);
  hi->setType(Type::createNumber());
  pushI64(lo, hi);
}

void WasmIRGen::onI64Xor() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  auto *lo = builder_.createBinaryOperatorInst(
      loA, loB, ValueKind::BinaryXorInstKind);
  lo->setType(Type::createNumber());
  auto *hi = builder_.createBinaryOperatorInst(
      hiA, hiB, ValueKind::BinaryXorInstKind);
  hi->setType(Type::createNumber());
  pushI64(lo, hi);
}

void WasmIRGen::onI64Shl() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  helpers_.emitI64Shl(retBufI_, loA, hiA, loB, hiB);
  auto [lo, hi] = readI64FromRetBuf();
  pushI64(lo, hi);
}

void WasmIRGen::onI64ShrS() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  helpers_.emitI64ShrS(retBufI_, loA, hiA, loB, hiB);
  auto [lo, hi] = readI64FromRetBuf();
  pushI64(lo, hi);
}

void WasmIRGen::onI64ShrU() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  helpers_.emitI64ShrU(retBufI_, loA, hiA, loB, hiB);
  auto [lo, hi] = readI64FromRetBuf();
  pushI64(lo, hi);
}

void WasmIRGen::onI64Rotl() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  helpers_.emitI64Rotl(retBufI_, loA, hiA, loB, hiB);
  auto [lo, hi] = readI64FromRetBuf();
  pushI64(lo, hi);
}

void WasmIRGen::onI64Rotr() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  helpers_.emitI64Rotr(retBufI_, loA, hiA, loB, hiB);
  auto [lo, hi] = readI64FromRetBuf();
  pushI64(lo, hi);
}

// --- i64 unary (G.3) ---
// clz/ctz/popcnt return i64 per the Wasm spec, but the result always fits
// in [0, 64]. The native helper returns a single i32 value. We push it as
// an i64 with hi = 0.

void WasmIRGen::onI64Clz() {
  if (unreachable_)
    return;
  auto [lo, hi] = popI64();
  auto *result = helpers_.emitI64Clz(lo, hi);
  pushI64(result, builder_.getLiteralNumber(0));
}

void WasmIRGen::onI64Ctz() {
  if (unreachable_)
    return;
  auto [lo, hi] = popI64();
  auto *result = helpers_.emitI64Ctz(lo, hi);
  pushI64(result, builder_.getLiteralNumber(0));
}

void WasmIRGen::onI64Popcnt() {
  if (unreachable_)
    return;
  auto [lo, hi] = popI64();
  auto *result = helpers_.emitI64Popcnt(lo, hi);
  pushI64(result, builder_.getLiteralNumber(0));
}

// --- i64 comparisons (G.3) ---
// These take i64 operands and return i32 (0 or 1). The result is pushed
// as a single i32 value (not an i64 pair).
// Inlined as IR rather than calling builtins, enabling subsequent
// optimization passes (constant folding, DCE, compare-and-branch fusion).

void WasmIRGen::onI64Eqz() {
  if (unreachable_)
    return;
  auto [lo, hi] = popI64();
  // Coerce to consistent encoding before comparing.
  auto *loI = ensureInt32(lo);
  auto *hiI = ensureInt32(hi);
  // (lo | hi) == 0: non-zero if either half is non-zero.
  auto *combined = builder_.createBinaryOperatorInst(
      loI, hiI, ValueKind::BinaryOrInstKind);
  combined->setType(Type::createNumber());
  auto *cmp = builder_.createFCompareInst(
      ValueKind::FEqualInstKind, combined, builder_.getLiteralNumber(0));
  push(builder_.createAsInt32Inst(cmp));
}

void WasmIRGen::onI64Eq() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  // Coerce all halves to consistent encoding (signed int32) before XOR.
  // Raw stack values may be signed or unsigned doubles for the same bits.
  auto *loA_i = ensureInt32(loA);
  auto *loB_i = ensureInt32(loB);
  auto *hiA_i = ensureInt32(hiA);
  auto *hiB_i = ensureInt32(hiB);
  // XOR each half: 0 if equal. OR the results: 0 iff both halves equal.
  auto *xorLo = builder_.createBinaryOperatorInst(
      loA_i, loB_i, ValueKind::BinaryXorInstKind);
  xorLo->setType(Type::createNumber());
  auto *xorHi = builder_.createBinaryOperatorInst(
      hiA_i, hiB_i, ValueKind::BinaryXorInstKind);
  xorHi->setType(Type::createNumber());
  auto *combined = builder_.createBinaryOperatorInst(
      xorLo, xorHi, ValueKind::BinaryOrInstKind);
  combined->setType(Type::createNumber());
  auto *cmp = builder_.createFCompareInst(
      ValueKind::FEqualInstKind, combined, builder_.getLiteralNumber(0));
  push(builder_.createAsInt32Inst(cmp));
}

void WasmIRGen::onI64Ne() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  // Coerce all halves to consistent encoding before XOR.
  auto *loA_i = ensureInt32(loA);
  auto *loB_i = ensureInt32(loB);
  auto *hiA_i = ensureInt32(hiA);
  auto *hiB_i = ensureInt32(hiB);
  // XOR each half, OR: non-zero iff any half differs.
  auto *xorLo = builder_.createBinaryOperatorInst(
      loA_i, loB_i, ValueKind::BinaryXorInstKind);
  xorLo->setType(Type::createNumber());
  auto *xorHi = builder_.createBinaryOperatorInst(
      hiA_i, hiB_i, ValueKind::BinaryXorInstKind);
  xorHi->setType(Type::createNumber());
  auto *combined = builder_.createBinaryOperatorInst(
      xorLo, xorHi, ValueKind::BinaryOrInstKind);
  combined->setType(Type::createNumber());
  auto *cmp = builder_.createFCompareInst(
      ValueKind::FNotEqualInstKind, combined, builder_.getLiteralNumber(0));
  push(builder_.createAsInt32Inst(cmp));
}

Value *WasmIRGen::ensureInt32(Value *val) {
  switch (val->getKind()) {
    case ValueKind::AsInt32InstKind:
    case ValueKind::BinaryAndInstKind:
    case ValueKind::BinaryOrInstKind:
    case ValueKind::BinaryXorInstKind:
    case ValueKind::BinaryLeftShiftInstKind:
    case ValueKind::BinaryRightShiftInstKind:
      return val;
    default:
      return builder_.createAsInt32Inst(val);
  }
}

Value *WasmIRGen::ensureUint32(Value *val) {
  switch (val->getKind()) {
    case ValueKind::AsUint32InstKind:
    case ValueKind::BinaryUnsignedRightShiftInstKind:
      return val;
    default:
      return builder_.createAsUint32Inst(val);
  }
}

Value *WasmIRGen::emitI64OrderedCmp(
    Value *loA,
    Value *hiA,
    Value *loB,
    Value *hiB,
    ValueKind hiOp,
    ValueKind loOp,
    bool hiSigned) {
  // Coerce hi words: signed (ensureInt32) or unsigned (ensureUint32).
  Value *hiA_c = hiSigned ? ensureInt32(hiA) : ensureUint32(hiA);
  Value *hiB_c = hiSigned ? ensureInt32(hiB) : ensureUint32(hiB);
  // Lo words always use unsigned interpretation for ordering.
  auto *loA_u = ensureUint32(loA);
  auto *loB_u = ensureUint32(loB);

  // hiOp on coerced hi words. FCompareInst auto-sets type to boolean.
  auto *hiCmp = builder_.createFCompareInst(hiOp, hiA_c, hiB_c);
  // Equality on coerced hi words. We reuse the same coerced values
  // because raw stack values may have inconsistent encodings (signed
  // vs unsigned doubles for the same 32-bit value).
  auto *hiEq = builder_.createFCompareInst(
      ValueKind::FEqualInstKind, hiA_c, hiB_c);
  // loOp on unsigned lo words.
  auto *loCmp = builder_.createFCompareInst(loOp, loA_u, loB_u);

  // Combine: hiCmp || (hiEq && loCmp)
  // AsInt32Inst converts boolean to i32 (auto-sets type).
  auto *hiCmpI = builder_.createAsInt32Inst(hiCmp);
  auto *hiEqI = builder_.createAsInt32Inst(hiEq);
  auto *loCmpI = builder_.createAsInt32Inst(loCmp);
  auto *eqAndLo = builder_.createBinaryOperatorInst(
      hiEqI, loCmpI, ValueKind::BinaryAndInstKind);
  eqAndLo->setType(Type::createNumber());
  auto *result = builder_.createBinaryOperatorInst(
      hiCmpI, eqAndLo, ValueKind::BinaryOrInstKind);
  result->setType(Type::createNumber());
  return result;
}

void WasmIRGen::onI64LtS() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  push(emitI64OrderedCmp(
      loA, hiA, loB, hiB, ValueKind::FLessThanInstKind,
      ValueKind::FLessThanInstKind, /*hiSigned=*/true));
}

void WasmIRGen::onI64GtS() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  push(emitI64OrderedCmp(
      loA, hiA, loB, hiB, ValueKind::FGreaterThanInstKind,
      ValueKind::FGreaterThanInstKind, /*hiSigned=*/true));
}

void WasmIRGen::onI64LeS() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  push(emitI64OrderedCmp(
      loA, hiA, loB, hiB, ValueKind::FLessThanInstKind,
      ValueKind::FLessThanOrEqualInstKind, /*hiSigned=*/true));
}

void WasmIRGen::onI64GeS() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  push(emitI64OrderedCmp(
      loA, hiA, loB, hiB, ValueKind::FGreaterThanInstKind,
      ValueKind::FGreaterThanOrEqualInstKind, /*hiSigned=*/true));
}

void WasmIRGen::onI64LtU() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  push(emitI64OrderedCmp(
      loA, hiA, loB, hiB, ValueKind::FLessThanInstKind,
      ValueKind::FLessThanInstKind, /*hiSigned=*/false));
}

void WasmIRGen::onI64GtU() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  push(emitI64OrderedCmp(
      loA, hiA, loB, hiB, ValueKind::FGreaterThanInstKind,
      ValueKind::FGreaterThanInstKind, /*hiSigned=*/false));
}

void WasmIRGen::onI64LeU() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  push(emitI64OrderedCmp(
      loA, hiA, loB, hiB, ValueKind::FLessThanInstKind,
      ValueKind::FLessThanOrEqualInstKind, /*hiSigned=*/false));
}

void WasmIRGen::onI64GeU() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  push(emitI64OrderedCmp(
      loA, hiA, loB, hiB, ValueKind::FGreaterThanInstKind,
      ValueKind::FGreaterThanOrEqualInstKind, /*hiSigned=*/false));
}

// --- i64 conversions: inline IR (G.4a) ---

void WasmIRGen::onI32WrapI64() {
  if (unreachable_)
    return;
  auto [lo, hi] = popI64();
  // i32.wrap_i64: take lo32, discard hi32.
  (void)hi;
  push(lo);
}

void WasmIRGen::onI64ExtendI32S() {
  if (unreachable_)
    return;
  Value *val = pop();
  // Sign-extend i32 to i64: lo = val, hi = (val >> 31).
  auto *hi = builder_.createBinaryOperatorInst(
      builder_.createAsInt32Inst(val),
      builder_.getLiteralNumber(31),
      ValueKind::BinaryRightShiftInstKind);
  hi->setType(Type::createNumber());
  pushI64(val, hi);
}

void WasmIRGen::onI64ExtendI32U() {
  if (unreachable_)
    return;
  Value *val = pop();
  // Zero-extend i32 to i64: lo = val, hi = 0.
  pushI64(val, builder_.getLiteralNumber(0));
}

void WasmIRGen::onI64Extend8S() {
  if (unreachable_)
    return;
  auto [lo, hi] = popI64();
  (void)hi;
  // Sign-extend lowest 8 bits: lo = (lo << 24) >> 24, hi = (lo >> 31).
  auto *shifted = builder_.createBinaryOperatorInst(
      lo, builder_.getLiteralNumber(24), ValueKind::BinaryLeftShiftInstKind);
  shifted->setType(Type::createNumber());
  auto *newLo = builder_.createBinaryOperatorInst(
      shifted,
      builder_.getLiteralNumber(24),
      ValueKind::BinaryRightShiftInstKind);
  newLo->setType(Type::createNumber());
  auto *newHi = builder_.createBinaryOperatorInst(
      newLo,
      builder_.getLiteralNumber(31),
      ValueKind::BinaryRightShiftInstKind);
  newHi->setType(Type::createNumber());
  pushI64(newLo, newHi);
}

void WasmIRGen::onI64Extend16S() {
  if (unreachable_)
    return;
  auto [lo, hi] = popI64();
  (void)hi;
  // Sign-extend lowest 16 bits: lo = (lo << 16) >> 16, hi = (lo >> 31).
  auto *shifted = builder_.createBinaryOperatorInst(
      lo, builder_.getLiteralNumber(16), ValueKind::BinaryLeftShiftInstKind);
  shifted->setType(Type::createNumber());
  auto *newLo = builder_.createBinaryOperatorInst(
      shifted,
      builder_.getLiteralNumber(16),
      ValueKind::BinaryRightShiftInstKind);
  newLo->setType(Type::createNumber());
  auto *newHi = builder_.createBinaryOperatorInst(
      newLo,
      builder_.getLiteralNumber(31),
      ValueKind::BinaryRightShiftInstKind);
  newHi->setType(Type::createNumber());
  pushI64(newLo, newHi);
}

void WasmIRGen::onI64Extend32S() {
  if (unreachable_)
    return;
  auto [lo, hi] = popI64();
  (void)hi;
  // Sign-extend lowest 32 bits (i.e., the lo half): hi = (lo >> 31).
  auto *newHi = builder_.createBinaryOperatorInst(
      builder_.createAsInt32Inst(lo),
      builder_.getLiteralNumber(31),
      ValueKind::BinaryRightShiftInstKind);
  newHi->setType(Type::createNumber());
  pushI64(lo, newHi);
}

// --- f64 arithmetic (E.1) ---
// We use BinaryOperatorInst (not FBinaryMathInst) because the F-prefixed
// instructions require number-typed inputs, but our values are loaded from
// Wasm operands are statically typed as :number, so we emit typed
// F-instructions directly (FAddInst, FCompareInst, FNegate, etc.).
// Values popped from the Wasm stack are wrapped in asNumber() to ensure
// they are typed as :number, since some IR instructions (calls, loads)
// produce :any even though the Wasm type system guarantees number.

void WasmIRGen::onF64Add() {
  Value *rhs = asNumber(pop());
  Value *lhs = asNumber(pop());
  push(builder_.createFBinaryMathInst(ValueKind::FAddInstKind, lhs, rhs));
}

void WasmIRGen::onF64Sub() {
  Value *rhs = asNumber(pop());
  Value *lhs = asNumber(pop());
  push(builder_.createFBinaryMathInst(
      ValueKind::FSubtractInstKind, lhs, rhs));
}

void WasmIRGen::onF64Mul() {
  Value *rhs = asNumber(pop());
  Value *lhs = asNumber(pop());
  push(builder_.createFBinaryMathInst(
      ValueKind::FMultiplyInstKind, lhs, rhs));
}

void WasmIRGen::onF64Div() {
  Value *rhs = asNumber(pop());
  Value *lhs = asNumber(pop());
  push(builder_.createFBinaryMathInst(
      ValueKind::FDivideInstKind, lhs, rhs));
}

void WasmIRGen::onF64Neg() {
  Value *val = asNumber(pop());
  push(builder_.createFUnaryMathInst(ValueKind::FNegateKind, val));
}

void WasmIRGen::onF64Abs() {
  Value *val = pop();
  push(builder_.createCallBuiltinInst(BuiltinMethod::Math_abs, {val}));
}

void WasmIRGen::onF64Sqrt() {
  Value *val = pop();
  push(builder_.createCallBuiltinInst(BuiltinMethod::Math_sqrt, {val}));
}

void WasmIRGen::onF64Ceil() {
  Value *val = pop();
  push(builder_.createCallBuiltinInst(BuiltinMethod::Math_ceil, {val}));
}

void WasmIRGen::onF64Floor() {
  Value *val = pop();
  push(builder_.createCallBuiltinInst(BuiltinMethod::Math_floor, {val}));
}

void WasmIRGen::onF64Trunc() {
  Value *val = pop();
  push(builder_.createCallBuiltinInst(BuiltinMethod::Math_trunc, {val}));
}

void WasmIRGen::onF64Nearest() {
  Value *val = pop();
  push(helpers_.emitNearest(val));
}

void WasmIRGen::onF64Min() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(builder_.createCallBuiltinInst(BuiltinMethod::Math_min, {lhs, rhs}));
}

void WasmIRGen::onF64Max() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(builder_.createCallBuiltinInst(BuiltinMethod::Math_max, {lhs, rhs}));
}

// --- f64 comparisons (E.1) ---
// Use FCompareInst since both operands are :number. The result is :boolean,
// converted to i32 (0/1) via |0.

void WasmIRGen::onF64Eq() {
  Value *rhs = asNumber(pop());
  Value *lhs = asNumber(pop());
  auto *cmp = builder_.createFCompareInst(
      ValueKind::FEqualInstKind, lhs, rhs);
  auto *asI32 = builder_.createAsInt32Inst(cmp);
  push(asI32);
}

void WasmIRGen::onF64Ne() {
  Value *rhs = asNumber(pop());
  Value *lhs = asNumber(pop());
  auto *cmp = builder_.createFCompareInst(
      ValueKind::FNotEqualInstKind, lhs, rhs);
  auto *asI32 = builder_.createAsInt32Inst(cmp);
  push(asI32);
}

void WasmIRGen::onF64Lt() {
  Value *rhs = asNumber(pop());
  Value *lhs = asNumber(pop());
  auto *cmp = builder_.createFCompareInst(
      ValueKind::FLessThanInstKind, lhs, rhs);
  auto *asI32 = builder_.createAsInt32Inst(cmp);
  push(asI32);
}

void WasmIRGen::onF64Gt() {
  Value *rhs = asNumber(pop());
  Value *lhs = asNumber(pop());
  auto *cmp = builder_.createFCompareInst(
      ValueKind::FGreaterThanInstKind, lhs, rhs);
  auto *asI32 = builder_.createAsInt32Inst(cmp);
  push(asI32);
}

void WasmIRGen::onF64Le() {
  Value *rhs = asNumber(pop());
  Value *lhs = asNumber(pop());
  auto *cmp = builder_.createFCompareInst(
      ValueKind::FLessThanOrEqualInstKind, lhs, rhs);
  auto *asI32 = builder_.createAsInt32Inst(cmp);
  push(asI32);
}

void WasmIRGen::onF64Ge() {
  Value *rhs = asNumber(pop());
  Value *lhs = asNumber(pop());
  auto *cmp = builder_.createFCompareInst(
      ValueKind::FGreaterThanOrEqualInstKind, lhs, rhs);
  auto *asI32 = builder_.createAsInt32Inst(cmp);
  push(asI32);
}

// --- f32 arithmetic (E.2) ---
// All f32 operations produce f32-precision results by wrapping the result
// in Math.fround. Constants are correctly rounded via float cast in
// onF32Const, so they don't need fround.

void WasmIRGen::onF32Add() {
  Value *rhs = asNumber(pop());
  Value *lhs = asNumber(pop());
  push(emitFround(builder_.createFBinaryMathInst(
      ValueKind::FAddInstKind, lhs, rhs)));
}

void WasmIRGen::onF32Sub() {
  Value *rhs = asNumber(pop());
  Value *lhs = asNumber(pop());
  push(emitFround(builder_.createFBinaryMathInst(
      ValueKind::FSubtractInstKind, lhs, rhs)));
}

void WasmIRGen::onF32Mul() {
  Value *rhs = asNumber(pop());
  Value *lhs = asNumber(pop());
  push(emitFround(builder_.createFBinaryMathInst(
      ValueKind::FMultiplyInstKind, lhs, rhs)));
}

void WasmIRGen::onF32Div() {
  Value *rhs = asNumber(pop());
  Value *lhs = asNumber(pop());
  push(emitFround(builder_.createFBinaryMathInst(
      ValueKind::FDivideInstKind, lhs, rhs)));
}

void WasmIRGen::onF32Neg() {
  Value *val = asNumber(pop());
  push(emitFround(builder_.createFUnaryMathInst(
      ValueKind::FNegateKind, val)));
}

void WasmIRGen::onF32Abs() {
  Value *val = pop();
  push(emitFround(
      builder_.createCallBuiltinInst(BuiltinMethod::Math_abs, {val})));
}

void WasmIRGen::onF32Sqrt() {
  Value *val = pop();
  push(emitFround(
      builder_.createCallBuiltinInst(BuiltinMethod::Math_sqrt, {val})));
}

void WasmIRGen::onF32Ceil() {
  Value *val = pop();
  push(emitFround(
      builder_.createCallBuiltinInst(BuiltinMethod::Math_ceil, {val})));
}

void WasmIRGen::onF32Floor() {
  Value *val = pop();
  push(emitFround(
      builder_.createCallBuiltinInst(BuiltinMethod::Math_floor, {val})));
}

void WasmIRGen::onF32Trunc() {
  Value *val = pop();
  push(emitFround(
      builder_.createCallBuiltinInst(BuiltinMethod::Math_trunc, {val})));
}

void WasmIRGen::onF32Nearest() {
  Value *val = pop();
  push(emitFround(helpers_.emitNearest(val)));
}

void WasmIRGen::onF32Min() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(emitFround(
      builder_.createCallBuiltinInst(BuiltinMethod::Math_min, {lhs, rhs})));
}

void WasmIRGen::onF32Max() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(emitFround(
      builder_.createCallBuiltinInst(BuiltinMethod::Math_max, {lhs, rhs})));
}

// --- f32 comparisons (E.3) ---
// Same pattern as f64 comparisons. Use FCompareInst since operands are :number.

void WasmIRGen::onF32Eq() {
  Value *rhs = asNumber(pop());
  Value *lhs = asNumber(pop());
  auto *cmp = builder_.createFCompareInst(
      ValueKind::FEqualInstKind, lhs, rhs);
  auto *asI32 = builder_.createAsInt32Inst(cmp);
  push(asI32);
}

void WasmIRGen::onF32Ne() {
  Value *rhs = asNumber(pop());
  Value *lhs = asNumber(pop());
  auto *cmp = builder_.createFCompareInst(
      ValueKind::FNotEqualInstKind, lhs, rhs);
  auto *asI32 = builder_.createAsInt32Inst(cmp);
  push(asI32);
}

void WasmIRGen::onF32Lt() {
  Value *rhs = asNumber(pop());
  Value *lhs = asNumber(pop());
  auto *cmp = builder_.createFCompareInst(
      ValueKind::FLessThanInstKind, lhs, rhs);
  auto *asI32 = builder_.createAsInt32Inst(cmp);
  push(asI32);
}

void WasmIRGen::onF32Gt() {
  Value *rhs = asNumber(pop());
  Value *lhs = asNumber(pop());
  auto *cmp = builder_.createFCompareInst(
      ValueKind::FGreaterThanInstKind, lhs, rhs);
  auto *asI32 = builder_.createAsInt32Inst(cmp);
  push(asI32);
}

void WasmIRGen::onF32Le() {
  Value *rhs = asNumber(pop());
  Value *lhs = asNumber(pop());
  auto *cmp = builder_.createFCompareInst(
      ValueKind::FLessThanOrEqualInstKind, lhs, rhs);
  auto *asI32 = builder_.createAsInt32Inst(cmp);
  push(asI32);
}

void WasmIRGen::onF32Ge() {
  Value *rhs = asNumber(pop());
  Value *lhs = asNumber(pop());
  auto *cmp = builder_.createFCompareInst(
      ValueKind::FGreaterThanOrEqualInstKind, lhs, rhs);
  auto *asI32 = builder_.createAsInt32Inst(cmp);
  push(asI32);
}

// --- f64/f32 copysign (F.5) ---

void WasmIRGen::onF64Copysign() {
  if (unreachable_)
    return;
  Value *b = pop();
  Value *a = pop();
  push(helpers_.emitF64Copysign(a, b));
}

void WasmIRGen::onF32Copysign() {
  if (unreachable_)
    return;
  Value *b = pop();
  Value *a = pop();
  push(emitFround(helpers_.emitF32Copysign(a, b)));
}

// --- f64/f32 conversions (E.1, E.2) ---

void WasmIRGen::onF64PromoteF32() {
  // f32 values are already represented as f64 doubles, so promotion is a
  // no-op. Just leave the value on the stack.
}

void WasmIRGen::onF32DemoteF64() {
  // Round f64 down to f32 precision via Math.fround.
  Value *val = pop();
  push(emitFround(val));
}

// --- Type conversions (F.4) ---

void WasmIRGen::onI32TruncF32S() {
  if (unreachable_)
    return;
  Value *a = pop();
  // f32 values are f32-precise doubles — reuse the f64 trapping truncation.
  push(helpers_.emitI32TruncF64S(a));
}

void WasmIRGen::onI32TruncF64S() {
  if (unreachable_)
    return;
  Value *a = pop();
  push(helpers_.emitI32TruncF64S(a));
}

void WasmIRGen::onI32TruncF32U() {
  if (unreachable_)
    return;
  Value *a = pop();
  push(helpers_.emitI32TruncF64U(a));
}

void WasmIRGen::onI32TruncF64U() {
  if (unreachable_)
    return;
  Value *a = pop();
  push(helpers_.emitI32TruncF64U(a));
}

void WasmIRGen::onI32TruncSatF32S() {
  if (unreachable_)
    return;
  Value *a = pop();
  push(helpers_.emitI32TruncSatF64S(a));
}

void WasmIRGen::onI32TruncSatF64S() {
  if (unreachable_)
    return;
  Value *a = pop();
  push(helpers_.emitI32TruncSatF64S(a));
}

void WasmIRGen::onI32TruncSatF32U() {
  if (unreachable_)
    return;
  Value *a = pop();
  push(helpers_.emitI32TruncSatF64U(a));
}

void WasmIRGen::onI32TruncSatF64U() {
  if (unreachable_)
    return;
  Value *a = pop();
  push(helpers_.emitI32TruncSatF64U(a));
}

void WasmIRGen::onF32ConvertI32S() {
  if (unreachable_)
    return;
  // Convert signed i32 to f32. AsInt32Inst ensures the value is treated as
  // signed, then fround rounds to f32 precision.
  Value *a = pop();
  push(emitFround(builder_.createAsInt32Inst(a)));
}

void WasmIRGen::onF32ConvertI32U() {
  if (unreachable_)
    return;
  // Convert unsigned i32 to f32. AsUint32Inst ensures the value is treated as
  // unsigned, then fround rounds to f32 precision.
  Value *a = pop();
  push(emitFround(builder_.createAsUint32Inst(a)));
}

void WasmIRGen::onF64ConvertI32S() {
  if (unreachable_)
    return;
  // Convert signed i32 to f64. Double can exactly represent all i32 values.
  Value *a = pop();
  push(builder_.createAsInt32Inst(a));
}

void WasmIRGen::onF64ConvertI32U() {
  if (unreachable_)
    return;
  // Convert unsigned i32 to f64. Double can exactly represent all uint32
  // values.
  Value *a = pop();
  push(builder_.createAsUint32Inst(a));
}

void WasmIRGen::onI32ReinterpretF32() {
  if (unreachable_)
    return;
  Value *a = pop();
  // Compile-time constant folding: recover the original f32 bit pattern.
  // For NaN values, use the saved bits from onF32Const since f32→f64
  // promotion may alter NaN payloads. For non-NaN values, convert back
  // to float directly.
  if (auto *lit = llvh::dyn_cast<LiteralNumber>(a)) {
    uint32_t bits;
    auto it = f32NanBitsMap_.find(lit);
    if (it != f32NanBitsMap_.end()) {
      bits = it->second;
    } else {
      bits = llvh::FloatToBits(static_cast<float>(lit->getValue()));
    }
    push(builder_.getLiteralNumber(
        static_cast<double>(static_cast<int32_t>(bits))));
    return;
  }
  push(helpers_.emitI32ReinterpretF32(a));
}

void WasmIRGen::onF32ReinterpretI32() {
  if (unreachable_)
    return;
  Value *a = pop();
  push(helpers_.emitF32ReinterpretI32(a));
}

// --- i64 conversion helpers: float→i64 truncations (G.4b) ---

void WasmIRGen::onI64TruncF64S() {
  if (unreachable_)
    return;
  Value *a = pop();
  helpers_.emitI64TruncF64S(retBufI_, a);
  auto [lo, hi] = readI64FromRetBuf();
  pushI64(lo, hi);
}

void WasmIRGen::onI64TruncF32S() {
  // Phase 1: all values are doubles, so f32 and f64 truncation are identical.
  onI64TruncF64S();
}

void WasmIRGen::onI64TruncF64U() {
  if (unreachable_)
    return;
  Value *a = pop();
  helpers_.emitI64TruncF64U(retBufI_, a);
  auto [lo, hi] = readI64FromRetBuf();
  pushI64(lo, hi);
}

void WasmIRGen::onI64TruncF32U() {
  // Phase 1: all values are doubles, so f32 and f64 truncation are identical.
  onI64TruncF64U();
}

void WasmIRGen::onI64TruncSatF64S() {
  if (unreachable_)
    return;
  Value *a = pop();
  helpers_.emitI64TruncSatF64S(retBufI_, a);
  auto [lo, hi] = readI64FromRetBuf();
  pushI64(lo, hi);
}

void WasmIRGen::onI64TruncSatF32S() {
  // Phase 1: all values are doubles, so f32 and f64 truncation are identical.
  onI64TruncSatF64S();
}

void WasmIRGen::onI64TruncSatF64U() {
  if (unreachable_)
    return;
  Value *a = pop();
  helpers_.emitI64TruncSatF64U(retBufI_, a);
  auto [lo, hi] = readI64FromRetBuf();
  pushI64(lo, hi);
}

void WasmIRGen::onI64TruncSatF32U() {
  // Phase 1: all values are doubles, so f32 and f64 truncation are identical.
  onI64TruncSatF64U();
}

// --- i64→float conversion helpers (G.4c) ---

void WasmIRGen::onF64ConvertI64S() {
  if (unreachable_)
    return;
  auto [lo, hi] = popI64();
  push(helpers_.emitF64ConvertI64S(lo, hi));
}

void WasmIRGen::onF64ConvertI64U() {
  if (unreachable_)
    return;
  auto [lo, hi] = popI64();
  push(helpers_.emitF64ConvertI64U(lo, hi));
}

void WasmIRGen::onF32ConvertI64S() {
  if (unreachable_)
    return;
  auto [lo, hi] = popI64();
  push(helpers_.emitF32ConvertI64S(lo, hi));
}

void WasmIRGen::onF32ConvertI64U() {
  if (unreachable_)
    return;
  auto [lo, hi] = popI64();
  push(helpers_.emitF32ConvertI64U(lo, hi));
}

void WasmIRGen::onI64ReinterpretF64() {
  if (unreachable_)
    return;
  Value *a = pop();
  // Compile-time constant folding: extract the raw bits directly from the
  // literal. This is essential for NaN values, whose bit patterns would be
  // canonicalized (and thus corrupted) by Hermes bytecode emission.
  if (auto *lit = llvh::dyn_cast<LiteralNumber>(a)) {
    uint64_t bits = llvh::DoubleToBits(lit->getValue());
    auto lo = static_cast<int32_t>(bits & 0xFFFFFFFF);
    auto hi = static_cast<int32_t>((bits >> 32) & 0xFFFFFFFF);
    pushI64(
        builder_.getLiteralNumber(static_cast<double>(lo)),
        builder_.getLiteralNumber(static_cast<double>(hi)));
    return;
  }
  helpers_.emitI64ReinterpretF64(retBufI_, a);
  auto [lo, hi] = readI64FromRetBuf();
  pushI64(lo, hi);
}

void WasmIRGen::onF64ReinterpretI64() {
  if (unreachable_)
    return;
  auto [lo, hi] = popI64();
  push(helpers_.emitF64ReinterpretI64(lo, hi));
}

// --- unreachable and nop (D.11) ---

void WasmIRGen::onUnreachable() {
  if (unreachable_)
    return;

  // Emit a call to the wasmTrap helper, which throws a runtime error.
  helpers_.emitTrap();

  // UnreachableInst serves as the block terminator for IR verification.
  // The trap call above always throws, so this is never reached at runtime.
  builder_.createUnreachableInst();

  // After unreachable, code is dead.
  unreachable_ = true;

  // Create a new dead basic block for any dead code that follows.
  auto *deadBlock = builder_.createBasicBlock(currentFunc_);
  builder_.setInsertionBlock(deadBlock);
}

void WasmIRGen::onNop() {
  // nop does nothing.
}

// --- Exception handling (L.1) ---

void WasmIRGen::onTry(const std::vector<WasmValType> &resultTypes) {
  if (unreachable_) {
    // Push a dummy Try entry so onEnd/onCatch can pop it.
    ControlEntry entry;
    entry.kind = ControlEntry::Try;
    entry.contBlock = nullptr;
    entry.catchBlock = nullptr;
    entry.resultTypes = resultTypes;
    entry.stackHeight = valueStack_.size();
    entry.outerUnreachable = true;
    controlStack_.push_back(std::move(entry));
    return;
  }

  // Create the continuation block (target of br 0, where execution resumes
  // after the try/catch construct).
  auto *contBlock = builder_.createBasicBlock(currentFunc_);
  // Create the catch dispatch block (target of TryStartInst for exceptions).
  auto *catchBlock = builder_.createBasicBlock(currentFunc_);
  // Create the try body block.
  auto *tryBodyBlock = builder_.createBasicBlock(currentFunc_);

  // Emit TryStartInst: branches to tryBodyBlock, exceptions go to catchBlock.
  builder_.createTryStartInst(tryBodyBlock, catchBlock);

  ControlEntry entry;
  entry.kind = ControlEntry::Try;
  entry.contBlock = contBlock; // br target and end continuation
  entry.catchBlock = catchBlock;
  entry.resultTypes = resultTypes;
  entry.stackHeight = valueStack_.size();
  entry.outerUnreachable = unreachable_;

  // Create phi nodes in the continuation block for results.
  if (!resultTypes.empty()) {
    entry.resultPhis = createResultPhis(contBlock, resultTypes);
  }

  controlStack_.push_back(std::move(entry));

  // Set insertion point to the try body block.
  builder_.setInsertionBlock(tryBodyBlock);
}

void WasmIRGen::onCatch(uint32_t tagIndex) {
  assert(!controlStack_.empty() && "control stack underflow");
  ControlEntry &entry = controlStack_.back();
  assert(entry.kind == ControlEntry::Try && "onCatch without matching try");

  if (entry.outerUnreachable) {
    // Reset unreachable for the catch handler.
    unreachable_ = true;
    return;
  }

  if (!entry.inCatch) {
    // First catch clause — transition from try body to catch handling.
    bool fallsThrough = !unreachable_ && !isCurrentBlockTerminated();

    if (fallsThrough) {
      // End the try body: TryEndInst exits the protected region.
      builder_.createTryEndInst(entry.catchBlock, entry.contBlock);
    }

    // Switch to the catch dispatch block.
    builder_.setInsertionBlock(entry.catchBlock);

    // CatchInst recovers the thrown exception value.
    entry.caughtValue = builder_.createCatchInst();

    entry.inCatch = true;
  } else {
    // Subsequent catch clause — end the current handler.
    bool fallsThrough = !unreachable_ && !isCurrentBlockTerminated();
    if (fallsThrough) {
      addBranchPhiOperands(entry);
      builder_.createBranchInst(entry.contBlock);
      entry.branchTargeted = true;
    }

    // Continue from the nextCatchBlock (where the previous tag check branches
    // on mismatch).
    builder_.setInsertionBlock(entry.nextCatchBlock);
  }

  // Check if the caught exception matches this tag.
  // The tag's identity, not its module-local index: another module numbers
  // its tags differently, so an index matched the wrong handler across a
  // module boundary.
  auto *tagLit =
      builder_.createLoadFrameInst(parentScopeInst_, tagVars_[tagIndex]);
  auto *matchResult = helpers_.emitMatchException(entry.caughtValue, tagLit);

  // Compare: matchResult !== undefined → match.
  auto *undef = builder_.getLiteralUndefined();
  auto *cmp = builder_.createBinaryOperatorInst(
      matchResult,
      undef,
      ValueKind::BinaryStrictlyNotEqualInstKind);

  auto *handlerBlock = builder_.createBasicBlock(currentFunc_);
  entry.nextCatchBlock = builder_.createBasicBlock(currentFunc_);

  builder_.createCondBranchInst(cmp, handlerBlock, entry.nextCatchBlock);

  // Set insertion to the handler block.
  builder_.setInsertionBlock(handlerBlock);

  // Restore value stack to the try entry height.
  valueStack_.resize(entry.stackHeight);
  valueStackIsI64Hi_.resize(entry.stackHeight);
  unreachable_ = false;

  // Extract payload values from the exception array and push to stack.
  // The array layout is: [tagIndex, v0, v1, ...]
  // where i64 values occupy two consecutive slots (lo, hi).
  // Payload values start at array index 1.
  const WasmFuncType &tagType = moduleInfo_.getTagType(tagIndex);
  uint32_t arrIdx = 1; // Start after tagIndex at position 0.
  for (size_t i = 0; i < tagType.params.size(); ++i) {
    auto *idx = builder_.getLiteralNumber(static_cast<double>(arrIdx));
    auto *val = builder_.createLoadPropertyInst(matchResult, idx);
    if (tagType.params[i] == WasmValType::I64) {
      // i64 payload: lo32 at arrIdx, hi32 at arrIdx+1.
      auto *hiIdx =
          builder_.getLiteralNumber(static_cast<double>(arrIdx + 1));
      auto *hiVal = builder_.createLoadPropertyInst(matchResult, hiIdx);
      pushI64(val, hiVal);
      arrIdx += 2;
    } else {
      push(val);
      arrIdx += 1;
    }
  }
}

void WasmIRGen::onCatchAll() {
  assert(!controlStack_.empty() && "control stack underflow");
  ControlEntry &entry = controlStack_.back();
  assert(entry.kind == ControlEntry::Try && "onCatchAll without matching try");

  if (entry.outerUnreachable) {
    unreachable_ = true;
    return;
  }

  if (!entry.inCatch) {
    // First handler is catch_all (no prior catch clauses).
    bool fallsThrough = !unreachable_ && !isCurrentBlockTerminated();

    if (fallsThrough) {
      builder_.createTryEndInst(entry.catchBlock, entry.contBlock);
    }

    builder_.setInsertionBlock(entry.catchBlock);
    entry.caughtValue = builder_.createCatchInst();
    entry.inCatch = true;
  } else {
    // catch_all after some catch clauses — end the current handler.
    bool fallsThrough = !unreachable_ && !isCurrentBlockTerminated();
    if (fallsThrough) {
      addBranchPhiOperands(entry);
      builder_.createBranchInst(entry.contBlock);
      entry.branchTargeted = true;
    }

    builder_.setInsertionBlock(entry.nextCatchBlock);
  }

  entry.hasCatchAll = true;
  entry.nextCatchBlock = nullptr;

  // Restore value stack to the try entry height.
  valueStack_.resize(entry.stackHeight);
  valueStackIsI64Hi_.resize(entry.stackHeight);
  unreachable_ = false;

  // catch_all has no payload — nothing pushed to the stack.
  // Phase 1: catches everything including traps (known spec deviation).
}

void WasmIRGen::onThrow(uint32_t tagIndex) {
  if (unreachable_)
    return;

  // Get the tag's payload types.
  const WasmFuncType &tagType = moduleInfo_.getTagType(tagIndex);

  // Pop payload values from the stack (in reverse order, top = last param).
  // Pop values in reverse and arrange them in forward order.
  // Note: the exception array stores them in forward order starting at index 1.
  llvh::SmallVector<Value *, 8> stackValues;
  for (size_t i = tagType.params.size(); i > 0; --i) {
    size_t idx = i - 1;
    if (tagType.params[idx] == WasmValType::I64) {
      auto [lo, hi] = popI64();
      stackValues.push_back(hi);
      stackValues.push_back(lo);
    } else {
      stackValues.push_back(pop());
    }
  }
  // stackValues is in reverse order, so reverse it.
  std::reverse(stackValues.begin(), stackValues.end());

  // Create the exception object via the builtin.
  // The tag's identity, not its module-local index: another module numbers
  // its tags differently, so an index matched the wrong handler across a
  // module boundary.
  auto *tagLit =
      builder_.createLoadFrameInst(parentScopeInst_, tagVars_[tagIndex]);
  auto *exceptionObj = helpers_.emitCreateException(tagLit, stackValues);

  // Throw it.
  builder_.createThrowInst(exceptionObj);

  // After throw, code is unreachable.
  unreachable_ = true;

  // Create a dead block for any subsequent dead code.
  auto *deadBlock = builder_.createBasicBlock(currentFunc_);
  builder_.setInsertionBlock(deadBlock);
}

void WasmIRGen::onRethrow(uint32_t depth) {
  if (unreachable_)
    return;

  // Find the catch block at the given depth. In Wasm exception handling,
  // rethrow targets the catch block, not the try's continuation.
  // The depth is relative to the current control stack.
  ControlEntry &entry = getControlEntry(depth);
  assert(
      entry.kind == ControlEntry::Try && entry.inCatch &&
      "rethrow must target a catch block");

  // Re-throw the caught exception.
  builder_.createThrowInst(entry.caughtValue);

  // After rethrow, code is unreachable.
  unreachable_ = true;

  auto *deadBlock = builder_.createBasicBlock(currentFunc_);
  builder_.setInsertionBlock(deadBlock);
}

void WasmIRGen::onDelegate(uint32_t depth) {
  assert(!controlStack_.empty() && "control stack underflow");
  ControlEntry entry = std::move(controlStack_.back());
  controlStack_.pop_back();
  assert(entry.kind == ControlEntry::Try && "onDelegate without matching try");

  if (entry.outerUnreachable) {
    unreachable_ = true;
    return;
  }

  bool fallsThrough = !unreachable_ && !isCurrentBlockTerminated();

  // End the try body. delegate pops the try entry.
  // If the delegate targets an enclosing try, exceptions are forwarded there.
  // For simplicity in Phase 1, we end the try body normally and let the
  // catch block re-throw (which the outer try will catch).
  if (fallsThrough) {
    addBranchPhiOperands(entry);
    builder_.createTryEndInst(entry.catchBlock, entry.contBlock);
  }

  // The catch block re-throws unconditionally (delegate just forwards).
  auto *savedBlock = builder_.getInsertionBlock();
  builder_.setInsertionBlock(entry.catchBlock);
  auto *caught = builder_.createCatchInst();
  builder_.createThrowInst(caught);
  builder_.setInsertionBlock(savedBlock);

  // Continue after the try.
  builder_.setInsertionBlock(entry.contBlock);
  unreachable_ = !fallsThrough && !entry.branchTargeted;
  if (entry.outerUnreachable)
    unreachable_ = true;

  // Restore value stack and push result phis.
  valueStack_.resize(entry.stackHeight);
  valueStackIsI64Hi_.resize(entry.stackHeight);
  pushResultPhis(entry);
}

// --- Unsupported opcode handling (D.13) ---

void WasmIRGen::warnUnsupported(
    const char *opcodeName,
    uint32_t numInputs,
    uint32_t numOutputs) {
  if (unreachable_)
    return;

  llvh::errs() << "warning: unsupported Wasm opcode: " << opcodeName << "\n";

  // Pop the expected number of inputs.
  for (uint32_t i = 0; i < numInputs; ++i) {
    if (!valueStack_.empty())
      pop();
  }

  // Push placeholder undefined values for outputs.
  for (uint32_t i = 0; i < numOutputs; ++i) {
    push(builder_.getLiteralUndefined());
  }
}

// --- Helper methods ---

Value *WasmIRGen::pop() {
  if (unreachable_) {
    // In unreachable code, return a placeholder without modifying the real
    // value stack. The stack will be restored by onEnd via resize to
    // stackHeight.
    return builder_.getLiteralUndefined();
  }
  assert(!valueStack_.empty() && "value stack underflow");
  Value *v = valueStack_.back();
  valueStack_.pop_back();
  valueStackIsI64Hi_.pop_back();
  return v;
}

void WasmIRGen::push(Value *v) {
  if (unreachable_)
    return;
  valueStack_.push_back(v);
  valueStackIsI64Hi_.push_back(false);
}

Value *WasmIRGen::emitFround(Value *val) {
  auto *inst = builder_.createCallBuiltinInst(
      BuiltinMethod::Math_fround, {val});
  inst->setType(Type::createNumber());
  return inst;
}

Value *WasmIRGen::asNumber(Value *val) {
  if (val->getType().isNumberType())
    return val;
  // A real conversion, not UnionNarrowTrustedInst. The verifier does not check
  // UnionNarrowTrustedInst, so asserting here would silence the very check that
  // catches a non-number reaching FBinaryMathInst/FCompareInst. Values that are
  // already numbers return above, so this only costs anything where the type is
  // genuinely unknown -- which is exactly where the assertion would be unsound.
  return builder_.createAsNumberInst(val);
}

void WasmIRGen::pushI64(Value *lo, Value *hi) {
  if (unreachable_)
    return;
  valueStack_.push_back(lo);
  valueStackIsI64Hi_.push_back(false); // lo32 is not the hi part
  valueStack_.push_back(hi);
  valueStackIsI64Hi_.push_back(true); // hi32 is marked
}

std::pair<Value *, Value *> WasmIRGen::popI64() {
  if (unreachable_) {
    auto *undef = builder_.getLiteralUndefined();
    return {undef, undef};
  }
  assert(valueStack_.size() >= 2 && "value stack underflow for i64 pop");
  assert(valueStackIsI64Hi_.back() && "expected i64 hi32 on top of stack");
  Value *hi = valueStack_.back();
  valueStack_.pop_back();
  valueStackIsI64Hi_.pop_back();
  assert(!valueStackIsI64Hi_.back() && "expected i64 lo32 below hi32");
  Value *lo = valueStack_.back();
  valueStack_.pop_back();
  valueStackIsI64Hi_.pop_back();
  return {lo, hi};
}

bool WasmIRGen::isTopI64() const {
  if (unreachable_)
    return false;
  return !valueStackIsI64Hi_.empty() && valueStackIsI64Hi_.back();
}

WasmIRGen::ControlEntry &WasmIRGen::getControlEntry(uint32_t depth) {
  assert(depth < controlStack_.size() && "branch depth out of range");
  return controlStack_[controlStack_.size() - 1 - depth];
}

size_t WasmIRGen::numPhisForResultTypes(
    const std::vector<WasmValType> &resultTypes) {
  size_t count = 0;
  for (auto t : resultTypes) {
    count += (t == WasmValType::I64) ? 2 : 1;
  }
  return count;
}

std::vector<PhiInst *> WasmIRGen::createResultPhis(
    BasicBlock *block,
    const std::vector<WasmValType> &resultTypes) {
  std::vector<PhiInst *> phis;
  auto *savedBlock = builder_.getInsertionBlock();
  builder_.setInsertionBlock(block);
  for (auto t : resultTypes) {
    phis.push_back(builder_.createPhiInst());
    if (t == WasmValType::I64) {
      phis.push_back(builder_.createPhiInst());
    }
  }
  builder_.setInsertionBlock(savedBlock);
  return phis;
}

void WasmIRGen::addBranchPhiOperands(ControlEntry &entry) {
  if ((entry.kind == ControlEntry::Block || entry.kind == ControlEntry::If ||
       entry.kind == ControlEntry::Try) &&
      !entry.resultPhis.empty()) {
    auto *currentBlock = builder_.getInsertionBlock();
    size_t numPhis = entry.resultPhis.size();
    // The number of stack slots consumed equals the number of phis
    // (i64 results have 2 phis and 2 stack slots).
    size_t available = valueStack_.size();

    if (available >= numPhis) {
      for (size_t i = 0; i < numPhis; ++i) {
        Value *val = valueStack_[available - numPhis + i];
        entry.resultPhis[i]->addEntry(val, currentBlock);
      }
      valueStack_.resize(available - numPhis);
      valueStackIsI64Hi_.resize(available - numPhis);
    } else {
      // Stack underflow — use undefined as placeholder.
      for (size_t i = 0; i < numPhis; ++i) {
        Value *val = (i >= numPhis - available)
            ? valueStack_[i - (numPhis - available)]
            : builder_.getLiteralUndefined();
        entry.resultPhis[i]->addEntry(val, currentBlock);
      }
      valueStack_.clear();
      valueStackIsI64Hi_.clear();
    }
  }
  // For Loop entries, br targets the loop header and passes param values.
  if (entry.kind == ControlEntry::Loop && !entry.paramPhis.empty()) {
    auto *currentBlock = builder_.getInsertionBlock();
    size_t numPhis = entry.paramPhis.size();
    size_t available = valueStack_.size();

    if (available >= numPhis) {
      for (size_t i = 0; i < numPhis; ++i) {
        Value *val = valueStack_[available - numPhis + i];
        entry.paramPhis[i]->addEntry(val, currentBlock);
      }
      valueStack_.resize(available - numPhis);
      valueStackIsI64Hi_.resize(available - numPhis);
    } else {
      for (size_t i = 0; i < numPhis; ++i) {
        Value *val = (i >= numPhis - available)
            ? valueStack_[i - (numPhis - available)]
            : builder_.getLiteralUndefined();
        entry.paramPhis[i]->addEntry(val, currentBlock);
      }
      valueStack_.clear();
      valueStackIsI64Hi_.clear();
    }
  }
}

void WasmIRGen::peekBranchPhiOperands(ControlEntry &entry) {
  if ((entry.kind == ControlEntry::Block || entry.kind == ControlEntry::If ||
       entry.kind == ControlEntry::Try) &&
      !entry.resultPhis.empty()) {
    size_t numPhis = entry.resultPhis.size();
    size_t available = valueStack_.size();
    auto *currentBlock = builder_.getInsertionBlock();
    for (size_t i = 0; i < numPhis; ++i) {
      Value *val;
      if (available >= numPhis) {
        val = valueStack_[available - numPhis + i];
      } else {
        val = builder_.getLiteralUndefined();
      }
      entry.resultPhis[i]->addEntry(val, currentBlock);
    }
  }
  // For Loop entries, br_if peeks at param values (don't pop — fallthrough
  // still needs them).
  if (entry.kind == ControlEntry::Loop && !entry.paramPhis.empty()) {
    size_t numPhis = entry.paramPhis.size();
    size_t available = valueStack_.size();
    auto *currentBlock = builder_.getInsertionBlock();
    for (size_t i = 0; i < numPhis; ++i) {
      Value *val;
      if (available >= numPhis) {
        val = valueStack_[available - numPhis + i];
      } else {
        val = builder_.getLiteralUndefined();
      }
      entry.paramPhis[i]->addEntry(val, currentBlock);
    }
  }
}

void WasmIRGen::pushResultPhis(const ControlEntry &entry) {
  size_t phiIdx = 0;
  for (auto t : entry.resultTypes) {
    if (t == WasmValType::I64) {
      assert(phiIdx + 1 < entry.resultPhis.size());
      pushI64(entry.resultPhis[phiIdx], entry.resultPhis[phiIdx + 1]);
      phiIdx += 2;
    } else {
      assert(phiIdx < entry.resultPhis.size());
      push(entry.resultPhis[phiIdx]);
      phiIdx += 1;
    }
  }
}

bool WasmIRGen::isCurrentBlockTerminated() {
  auto *bb = builder_.getInsertionBlock();
  if (!bb || bb->empty())
    return false;
  return llvh::isa<TerminatorInst>(&bb->back());
}

// --- Memory access (H.1) ---

Value *WasmIRGen::emitNew(Value *constructor, llvh::ArrayRef<Value *> args) {
  auto *thisArg = builder_.createCreateThisInst(constructor, constructor);
  auto *call = builder_.createCallInst(
      constructor, constructor, thisArg, args);
  return builder_.createGetConstructedObjectInst(thisArg, call);
}

void WasmIRGen::createMemoryViews(Instruction *tlScope) {
  // For a locally-defined memory, the backing store is a real
  // WebAssembly.Memory rather than a bare ArrayBuffer, and the views below
  // are built over *its* buffer. That is what makes an exported memory the
  // same object the module operates on: exporting a separately-constructed
  // Memory gives the embedder a buffer the module never writes to, and one
  // that does not follow memory.grow. onMemoryGrow() installs the grown
  // buffer back onto this object for the same reason.
  //
  Value *buffer = nullptr;
  if (memObjVar_ && moduleInfo_.memories.empty()) {
    // Imported memory: the embedder's WebAssembly.Memory *is* the module's
    // linear memory. Its buffer came out of the memory's internal field
    // during import validation, together with the page count that satisfied
    // the declaration -- so the views below are over the buffer that was
    // actually measured. Re-reading `.buffer` here would go through a
    // prototype accessor script can replace.
    assert(
        importedMemBufVar_ &&
        "an imported memory must have recorded its buffer");
    buffer = builder_.createLoadFrameInst(tlScope, importedMemBufVar_);
  } else if (!moduleInfo_.memories.empty()) {
    const auto &limits = moduleInfo_.memories[0].limits;
    auto *descriptor = builder_.createAllocObjectLiteralInst({});
    builder_.createStorePropertyStrictInst(
        builder_.getLiteralNumber(static_cast<double>(limits.initial)),
        descriptor,
        builder_.getLiteralString("initial"));
    if (limits.hasMaximum) {
      builder_.createStorePropertyStrictInst(
          builder_.getLiteralNumber(static_cast<double>(limits.maximum)),
          descriptor,
          builder_.getLiteralString("maximum"));
    }
    auto *wasmObj = builder_.createTryLoadGlobalPropertyInst("WebAssembly");
    auto *memCtor = builder_.createLoadPropertyInst(
        wasmObj, builder_.getLiteralString("Memory"));
    auto *memObj = emitNew(memCtor, {descriptor});
    builder_.createStoreFrameInst(tlScope, memObj, memObjVar_);
    // Take the buffer out of the memory's internal field, through the same
    // brand check the import path uses -- not from a `.buffer` property read.
    // That accessor is a CONFIGURABLE property of WebAssembly.Memory.prototype,
    // so reading it here let script substitute the module's entire linear
    // memory: the module wrote into storage script chose while the Memory it
    // exported, which an importing module brand-checks and therefore trusts,
    // still held its own untouched buffer. That is the same "validate one
    // object, use another" defect the import path fixed, and it was
    // cross-module: wasmLinkMemory would hand an importer a buffer that was
    // provably not this module's linear memory.
    //
    // The brand check CAN fail here: `globalThis.WebAssembly.Memory` is an
    // ordinary property and script may replace it with a constructor that
    // returns anything. Without the branch, `.buffer` yielded undefined,
    // `new Uint8Array(undefined)` gave a zero-length view, and instantiation
    // SUCCEEDED with a memory of no pages -- every access silently out of
    // bounds. Report it by name instead.
    auto *linked = helpers_.emitLinkMemory(memObj);
    auto *ctorFunc = builder_.getInsertionBlock()->getParent();
    auto *ctorBadBB = builder_.createBasicBlock(ctorFunc);
    auto *ctorOkBB = builder_.createBasicBlock(ctorFunc);
    builder_.createCondBranchInst(
        builder_.createBinaryOperatorInst(
            linked,
            builder_.getLiteralNull(),
            ValueKind::BinaryStrictlyEqualInstKind),
        ctorBadBB,
        ctorOkBB);
    builder_.setInsertionBlock(ctorBadBB);
    helpers_.emitLinkError(builder_.getLiteralString(
        "WebAssembly.Memory did not construct a memory for this module's "
        "memory 0"));
    builder_.createUnreachableInst();
    builder_.setInsertionBlock(ctorOkBB);

    // The brand is not the whole of it. A hostile constructor can return a
    // GENUINE WebAssembly.Memory with limits of its own choosing, and the
    // declaration's limits are compile-time constants of what this module
    // ASKED FOR, not of what came back. Checking only the brand was the same
    // "validate one object, use another" shape one level down. Reproduced
    // before this check existed: declare `(memory 1 4)`, return a memory
    // built with `{initial: 1, maximum: 2}`, and the module's memory.grow --
    // which uses the compile-time literal 4 for a defined memory -- grows it
    // to four pages, past the substituted object's own maximum, leaving
    // maxPages_ at 2 with a four-page buffer:
    //
    //   substituted maximum is 2; module grow(3) -> 1
    //   buffer now 4 pages
    //   mem.grow(0) at 4 pages -> RangeError: would exceed maximum
    //
    // Never memory-unsafe -- every access is bounds-checked against the real
    // buffer -- but the module runs on limits nobody agreed to, and the
    // exported object is left internally inconsistent.
    //
    // EXACT equality, not the import path's >= / <=: this is not "does the
    // supplied memory satisfy a declaration", it is "did the constructor
    // build the memory this module asked for". A genuine construction always
    // yields exactly the requested pages, and exactly the requested maximum
    // or -1 when none was requested, so anything else means the descriptor or
    // the constructor was interfered with. (The descriptor is reachable too:
    // it is a fresh object literal, and its `initial`/`maximum` stores walk
    // the prototype chain, so a setter on Object.prototype can rewrite them.)
    auto *actualPages = builder_.createLoadPropertyInst(
        linked, builder_.getLiteralNumber(0));
    auto *actualMax = builder_.createLoadPropertyInst(
        linked, builder_.getLiteralNumber(1));
    auto *limitsBadBB = builder_.createBasicBlock(ctorFunc);
    auto *checkMaxBB = builder_.createBasicBlock(ctorFunc);
    auto *limitsOkBB = builder_.createBasicBlock(ctorFunc);
    builder_.createCondBranchInst(
        builder_.createBinaryOperatorInst(
            actualPages,
            builder_.getLiteralNumber(static_cast<double>(limits.initial)),
            ValueKind::BinaryStrictlyEqualInstKind),
        checkMaxBB,
        limitsBadBB);
    builder_.setInsertionBlock(checkMaxBB);
    // -1 is how wasmLinkMemory spells "this memory declares no maximum".
    builder_.createCondBranchInst(
        builder_.createBinaryOperatorInst(
            actualMax,
            builder_.getLiteralNumber(
                limits.hasMaximum ? static_cast<double>(limits.maximum) : -1.0),
            ValueKind::BinaryStrictlyEqualInstKind),
        limitsOkBB,
        limitsBadBB);
    builder_.setInsertionBlock(limitsBadBB);
    helpers_.emitLinkError(builder_.getLiteralString(
        "WebAssembly.Memory did not construct a memory with this module's "
        "declared limits for memory 0"));
    builder_.createUnreachableInst();
    builder_.setInsertionBlock(limitsOkBB);
    tlEntry_ = limitsOkBB;
    // Index 2 is the buffer.
    buffer = builder_.createLoadPropertyInst(
        linked, builder_.getLiteralNumber(2));
  } else {
    // No memory at all -- only reached if createMemoryViews() is called
    // without a memory, which hasMemory guards against.
    auto *abCtor = builder_.createTryLoadGlobalPropertyInst("ArrayBuffer");
    buffer = emitNew(abCtor, {builder_.getLiteralNumber(0)});
  }

  // Create typed array views and store in top-level scope variables.
  static const char *ctorNames[NUM_MEM_VIEWS] = {
      "Int8Array",
      "Uint8Array",
      "Int16Array",
      "Uint16Array",
      "Int32Array",
      "Uint32Array",
      "Float32Array",
      "Float64Array",
  };
  for (uint8_t i = 0; i < NUM_MEM_VIEWS; ++i) {
    auto *ctor = builder_.createTryLoadGlobalPropertyInst(ctorNames[i]);
    auto *view = emitNew(ctor, {buffer});
    builder_.createStoreFrameInst(tlScope, view, memViewVars_[i]);
  }
}

Value *WasmIRGen::loadMemView(MemView view) {
  return builder_.createLoadFrameInst(parentScopeInst_, memViewVars_[view]);
}

Variable *WasmIRGen::getOrCreateDataSegVar() {
  if (!dataSegVar_) {
    dataSegVar_ = builder_.createVariable(
        topLevelVS_,
        "data_segments",
        Type::createAnyType(),
        /* hidden */ true);
  }
  return dataSegVar_;
}

Variable *WasmIRGen::getOrCreateElemSegVar() {
  if (!elemSegVar_) {
    elemSegVar_ = builder_.createVariable(
        topLevelVS_,
        "elem_segments",
        Type::createAnyType(),
        /* hidden */ true);
  }
  return elemSegVar_;
}

uint8_t WasmIRGen::getNaturalAlignLog2(llvh::StringRef op) {
  // 64-bit: natural alignment = 8 bytes (log2 = 3).
  if (op == "i64.load" || op == "i64.store" || op == "f64.load" ||
      op == "f64.store")
    return 3;
  // 32-bit: natural alignment = 4 bytes (log2 = 2).
  // Includes i32, f32 full loads/stores and i64 narrow 32-bit variants.
  if (op == "i32.load" || op == "i32.store" || op == "f32.load" ||
      op == "f32.store" || op == "i64.load32_s" || op == "i64.load32_u" ||
      op == "i64.store32")
    return 2;
  // 16-bit: natural alignment = 2 bytes (log2 = 1).
  if (op.endswith("16_s") || op.endswith("16_u") || op.endswith("store16"))
    return 1;
  // Byte loads/stores: natural alignment = 1 byte (log2 = 0).
  // These are always naturally aligned, so unaligned path is never taken.
  return 0;
}

Value *WasmIRGen::emitUnalignedLoad(Value *addr, uint32_t numBytes) {
  // Load individual bytes from HEAPU8 and assemble them via shifts and OR.
  auto *view = loadMemView(HEAPU8);

  // Load byte 0.
  auto *b0 = builder_.createLoadPropertyInst(view, addr);

  // OOB check on the first byte.
  auto *isUndef = builder_.createBinaryOperatorInst(
      b0,
      builder_.getLiteralUndefined(),
      ValueKind::BinaryStrictlyEqualInstKind);
  auto *trapBlock = builder_.createBasicBlock(currentFunc_);
  auto *okBlock = builder_.createBasicBlock(currentFunc_);
  builder_.createCondBranchInst(isUndef, trapBlock, okBlock);

  builder_.setInsertionBlock(trapBlock);
  helpers_.emitTrap();
  builder_.createUnreachableInst();

  builder_.setInsertionBlock(okBlock);
  // We proved b0 !== undefined, so it must be a Number.
  auto *b0Typed = builder_.createUnionNarrowTrustedInst(
      b0, Type::createNumber());

  if (numBytes == 1)
    return b0Typed;

  // Assemble multi-byte value: result = b0 | (b1 << 8) | (b2 << 16) | ...
  Value *result = b0Typed;
  for (uint32_t i = 1; i < numBytes; ++i) {
    auto *addrI = builder_.createBinaryOperatorInst(
        addr,
        builder_.getLiteralNumber(static_cast<double>(i)),
        ValueKind::BinaryAddInstKind);
    addrI->setType(Type::createNumber());
    auto *bi = builder_.createLoadPropertyInst(view, addrI);
    auto *shifted = builder_.createBinaryOperatorInst(
        bi,
        builder_.getLiteralNumber(static_cast<double>(i * 8)),
        ValueKind::BinaryLeftShiftInstKind);
    shifted->setType(Type::createNumber());
    result = builder_.createBinaryOperatorInst(
        result, shifted, ValueKind::BinaryOrInstKind);
    result->setType(Type::createNumber());
  }
  return result;
}

void WasmIRGen::emitUnalignedStore(
    Value *addr,
    Value *value,
    uint32_t numBytes) {
  // Decompose value into bytes and store each byte to HEAPU8.
  auto *view = loadMemView(HEAPU8);

  // Store byte 0: value & 0xFF.
  auto *b0 = builder_.createBinaryOperatorInst(
      value,
      builder_.getLiteralNumber(0xFF),
      ValueKind::BinaryAndInstKind);
  b0->setType(Type::createNumber());
  builder_.createStorePropertyStrictInst(b0, view, addr);

  for (uint32_t i = 1; i < numBytes; ++i) {
    auto *addrI = builder_.createBinaryOperatorInst(
        addr,
        builder_.getLiteralNumber(static_cast<double>(i)),
        ValueKind::BinaryAddInstKind);
    addrI->setType(Type::createNumber());
    // Shift right by i*8, then mask to get byte i.
    auto *shifted = builder_.createBinaryOperatorInst(
        value,
        builder_.getLiteralNumber(static_cast<double>(i * 8)),
        ValueKind::BinaryUnsignedRightShiftInstKind);
    shifted->setType(Type::createNumber());
    auto *bi = builder_.createBinaryOperatorInst(
        shifted,
        builder_.getLiteralNumber(0xFF),
        ValueKind::BinaryAndInstKind);
    bi->setType(Type::createNumber());
    builder_.createStorePropertyStrictInst(bi, view, addrI);
  }
}

Value *WasmIRGen::emitEffectiveAddr(Value *base, uint32_t offset) {
  Value *b = base;
  if (test262_) {
    // Treat the base as unsigned: base >>> 0.
    b = builder_.createBinaryOperatorInst(
        base,
        builder_.getLiteralNumber(0),
        ValueKind::BinaryUnsignedRightShiftInstKind);
    b->setType(Type::createNumber());
  }
  if (offset != 0) {
    auto *addr = builder_.createBinaryOperatorInst(
        b,
        builder_.getLiteralNumber(static_cast<double>(offset)),
        ValueKind::BinaryAddInstKind);
    addr->setType(Type::createNumber());
    return addr;
  }
  return b;
}

void WasmIRGen::emitMemoryBoundsCheck(Value *addr, uint32_t numBytes) {
  if (!test262_)
    return;
  auto *end = builder_.createBinaryOperatorInst(
      addr,
      builder_.getLiteralNumber(static_cast<double>(numBytes)),
      ValueKind::BinaryAddInstKind);
  end->setType(Type::createNumber());
  auto *memLength = builder_.createLoadPropertyInst(
      loadMemView(HEAPU8), builder_.getLiteralString("length"));
  auto *isOOB = builder_.createBinaryOperatorInst(
      end, memLength, ValueKind::BinaryGreaterThanInstKind);
  auto *trapBlock = builder_.createBasicBlock(currentFunc_);
  auto *okBlock = builder_.createBasicBlock(currentFunc_);
  builder_.createCondBranchInst(isOOB, trapBlock, okBlock);
  builder_.setInsertionBlock(trapBlock);
  helpers_.emitTrap();
  builder_.createUnreachableInst();
  builder_.setInsertionBlock(okBlock);
}

void WasmIRGen::onLoad(
    const char *opcodeName,
    uint32_t alignLog2,
    uint32_t offset) {
  if (unreachable_)
    return;

  // Pop the base address.
  Value *base = pop();

  // Compute effective address: base + offset (with unsigned base in test262).
  Value *addr = emitEffectiveAddr(base, offset);

  // Determine which view to use and the element shift based on the opcode.
  llvh::StringRef op(opcodeName);

  // Check if we need the unaligned (byte-assembly) path.
  uint8_t naturalAlign = getNaturalAlignLog2(op);
  // When test262_, ignore alignment hints and always use the byte-assembly
  // path. The Wasm spec says alignment hints are advisory; engines must
  // produce correct results regardless of actual alignment.
  if (test262_)
    alignLog2 = 0;

  // i64 loads: handled specially (split into lo/hi).
  if (op == "i64.load") {
    emitMemoryBoundsCheck(addr, 8);
    if (alignLog2 < naturalAlign) {
      // Unaligned: byte-assemble lo32 and hi32 separately.
      auto *lo = emitUnalignedLoad(addr, 4);
      auto *addrHi = builder_.createBinaryOperatorInst(
          addr,
          builder_.getLiteralNumber(4),
          ValueKind::BinaryAddInstKind);
      addrHi->setType(Type::createNumber());
      auto *hi = emitUnalignedLoad(addrHi, 4);
      pushI64(lo, hi);
      return;
    }
    // Aligned path: load two consecutive i32 values from HEAPU32.
    auto *view = loadMemView(HEAPU32);
    auto *idx = builder_.createBinaryOperatorInst(
        addr,
        builder_.getLiteralNumber(2),
        ValueKind::BinaryUnsignedRightShiftInstKind);
    idx->setType(Type::createNumber());
    auto *lo = builder_.createLoadPropertyInst(view, idx);
    // Check OOB: if result === undefined, trap.
    auto *isUndef = builder_.createBinaryOperatorInst(
        lo,
        builder_.getLiteralUndefined(),
        ValueKind::BinaryStrictlyEqualInstKind);
    auto *trapBlock = builder_.createBasicBlock(currentFunc_);
    auto *okBlock = builder_.createBasicBlock(currentFunc_);
    builder_.createCondBranchInst(isUndef, trapBlock, okBlock);

    builder_.setInsertionBlock(trapBlock);
    helpers_.emitTrap();
    builder_.createUnreachableInst();

    builder_.setInsertionBlock(okBlock);
    // We proved lo !== undefined, so it must be a Number.
    auto *loTyped = builder_.createUnionNarrowTrustedInst(
        lo, Type::createNumber());
    // Load the hi32 word at idx+1.
    auto *idx1 = builder_.createBinaryOperatorInst(
        idx,
        builder_.getLiteralNumber(1),
        ValueKind::BinaryAddInstKind);
    idx1->setType(Type::createNumber());
    auto *hi = builder_.createLoadPropertyInst(view, idx1);
    // hi is also a number from the typed array (we trust it since lo was valid).
    auto *hiTyped = builder_.createUnionNarrowTrustedInst(
        hi, Type::createNumber());
    pushI64(loTyped, hiTyped);
    return;
  }

  if (op == "i64.load8_s" || op == "i64.load8_u") {
    emitMemoryBoundsCheck(addr, 1);
    bool isSigned = (op == "i64.load8_s");
    auto *view = loadMemView(isSigned ? HEAP8 : HEAPU8);
    auto *lo = builder_.createLoadPropertyInst(view, addr);
    // OOB check.
    auto *isUndef = builder_.createBinaryOperatorInst(
        lo,
        builder_.getLiteralUndefined(),
        ValueKind::BinaryStrictlyEqualInstKind);
    auto *trapBlock = builder_.createBasicBlock(currentFunc_);
    auto *okBlock = builder_.createBasicBlock(currentFunc_);
    builder_.createCondBranchInst(isUndef, trapBlock, okBlock);

    builder_.setInsertionBlock(trapBlock);
    helpers_.emitTrap();
    builder_.createUnreachableInst();

    builder_.setInsertionBlock(okBlock);
    // We proved lo !== undefined, so it must be a Number.
    auto *loNarrow = builder_.createUnionNarrowTrustedInst(
        lo, Type::createNumber());
    // Result as i32 (lower byte), zero or sign extended.
    auto *loI32 = builder_.createAsInt32Inst(loNarrow);
    // For i64: hi = sign-extended bits or 0.
    Value *hi;
    if (isSigned) {
      // Sign-extend: hi = lo >> 31 (arithmetic shift fills with sign bit).
      hi = builder_.createBinaryOperatorInst(
          loI32,
          builder_.getLiteralNumber(31),
          ValueKind::BinaryRightShiftInstKind);
      hi->setType(Type::createNumber());
    } else {
      hi = builder_.getLiteralNumber(0);
    }
    pushI64(loI32, hi);
    return;
  }

  if (op == "i64.load16_s" || op == "i64.load16_u") {
    emitMemoryBoundsCheck(addr, 2);
    bool isSigned = (op == "i64.load16_s");

    if (alignLog2 < naturalAlign) {
      // Unaligned: byte-assemble 2 bytes from HEAPU8.
      auto *raw = emitUnalignedLoad(addr, 2);
      Value *loI32;
      if (isSigned) {
        // Sign-extend from 16 bits: (raw << 16) >> 16.
        auto *shifted = builder_.createBinaryOperatorInst(
            raw,
            builder_.getLiteralNumber(16),
            ValueKind::BinaryLeftShiftInstKind);
        shifted->setType(Type::createNumber());
        loI32 = builder_.createBinaryOperatorInst(
            shifted,
            builder_.getLiteralNumber(16),
            ValueKind::BinaryRightShiftInstKind);
        loI32->setType(Type::createNumber());
      } else {
        loI32 = raw;
      }
      Value *hi;
      if (isSigned) {
        hi = builder_.createBinaryOperatorInst(
            loI32,
            builder_.getLiteralNumber(31),
            ValueKind::BinaryRightShiftInstKind);
        hi->setType(Type::createNumber());
      } else {
        hi = builder_.getLiteralNumber(0);
      }
      pushI64(loI32, hi);
      return;
    }

    // Aligned path.
    auto *view = loadMemView(isSigned ? HEAP16 : HEAPU16);
    auto *idx = builder_.createBinaryOperatorInst(
        addr,
        builder_.getLiteralNumber(1),
        ValueKind::BinaryUnsignedRightShiftInstKind);
    idx->setType(Type::createNumber());
    auto *lo = builder_.createLoadPropertyInst(view, idx);
    auto *isUndef = builder_.createBinaryOperatorInst(
        lo,
        builder_.getLiteralUndefined(),
        ValueKind::BinaryStrictlyEqualInstKind);
    auto *trapBlock = builder_.createBasicBlock(currentFunc_);
    auto *okBlock = builder_.createBasicBlock(currentFunc_);
    builder_.createCondBranchInst(isUndef, trapBlock, okBlock);

    builder_.setInsertionBlock(trapBlock);
    helpers_.emitTrap();
    builder_.createUnreachableInst();

    builder_.setInsertionBlock(okBlock);
    // We proved lo !== undefined, so it must be a Number.
    auto *loNarrow = builder_.createUnionNarrowTrustedInst(
        lo, Type::createNumber());
    auto *loI32 = builder_.createAsInt32Inst(loNarrow);
    Value *hi;
    if (isSigned) {
      hi = builder_.createBinaryOperatorInst(
          loI32,
          builder_.getLiteralNumber(31),
          ValueKind::BinaryRightShiftInstKind);
      hi->setType(Type::createNumber());
    } else {
      hi = builder_.getLiteralNumber(0);
    }
    pushI64(loI32, hi);
    return;
  }

  if (op == "i64.load32_s" || op == "i64.load32_u") {
    emitMemoryBoundsCheck(addr, 4);
    bool isSigned = (op == "i64.load32_s");

    if (alignLog2 < naturalAlign) {
      // Unaligned: byte-assemble 4 bytes from HEAPU8.
      auto *lo = emitUnalignedLoad(addr, 4);
      Value *hi;
      if (isSigned) {
        auto *loI32 = builder_.createAsInt32Inst(lo);
        hi = builder_.createBinaryOperatorInst(
            loI32,
            builder_.getLiteralNumber(31),
            ValueKind::BinaryRightShiftInstKind);
        hi->setType(Type::createNumber());
      } else {
        hi = builder_.getLiteralNumber(0);
      }
      pushI64(lo, hi);
      return;
    }

    // Aligned path.
    auto *view = loadMemView(isSigned ? HEAP32 : HEAPU32);
    auto *idx = builder_.createBinaryOperatorInst(
        addr,
        builder_.getLiteralNumber(2),
        ValueKind::BinaryUnsignedRightShiftInstKind);
    idx->setType(Type::createNumber());
    auto *lo = builder_.createLoadPropertyInst(view, idx);
    auto *isUndef = builder_.createBinaryOperatorInst(
        lo,
        builder_.getLiteralUndefined(),
        ValueKind::BinaryStrictlyEqualInstKind);
    auto *trapBlock = builder_.createBasicBlock(currentFunc_);
    auto *okBlock = builder_.createBasicBlock(currentFunc_);
    builder_.createCondBranchInst(isUndef, trapBlock, okBlock);

    builder_.setInsertionBlock(trapBlock);
    helpers_.emitTrap();
    builder_.createUnreachableInst();

    builder_.setInsertionBlock(okBlock);
    // We proved lo !== undefined, so it must be a Number.
    auto *loNarrow = builder_.createUnionNarrowTrustedInst(
        lo, Type::createNumber());
    Value *hi;
    if (isSigned) {
      // The HEAP32 (Int32Array) already returns a signed i32.
      // Sign-extend hi from bit 31.
      auto *loI32 = builder_.createAsInt32Inst(loNarrow);
      hi = builder_.createBinaryOperatorInst(
          loI32,
          builder_.getLiteralNumber(31),
          ValueKind::BinaryRightShiftInstKind);
      hi->setType(Type::createNumber());
    } else {
      hi = builder_.getLiteralNumber(0);
    }
    pushI64(loNarrow, hi);
    return;
  }

  if (op == "i64.store" || op == "i64.store8" || op == "i64.store16" ||
      op == "i64.store32") {
    // This is actually a load dispatch error — stores go through onStore.
    // But just in case, handle gracefully.
    assert(false && "i64 store dispatched to onLoad");
    return;
  }

  // Non-i64 loads.
  MemView view;
  uint8_t shift = 0; // log2(element_size)
  uint32_t numBytes = 0;

  if (op == "i32.load") {
    view = HEAP32;
    shift = 2;
    numBytes = 4;
  } else if (op == "i32.load8_s") {
    // Int8Array returns signed values natively.
    view = HEAP8;
    shift = 0;
    numBytes = 1;
  } else if (op == "i32.load8_u") {
    view = HEAPU8;
    shift = 0;
    numBytes = 1;
  } else if (op == "i32.load16_s") {
    // Int16Array returns signed values natively.
    view = HEAP16;
    shift = 1;
    numBytes = 2;
  } else if (op == "i32.load16_u") {
    view = HEAPU16;
    shift = 1;
    numBytes = 2;
  } else if (op == "f32.load") {
    view = HEAPF32;
    shift = 2;
    numBytes = 4;
  } else if (op == "f64.load") {
    view = HEAPF64;
    shift = 3;
    numBytes = 8;
  } else {
    // Unknown load opcode.
    llvh::errs() << "WARNING: unsupported load opcode: " << op << "\n";
    push(builder_.getLiteralUndefined());
    return;
  }

  emitMemoryBoundsCheck(addr, numBytes);

  // Unaligned path: byte-assemble from HEAPU8.
  if (alignLog2 < naturalAlign) {
    if (op == "f64.load") {
      // f64: load two 4-byte halves and reinterpret as f64.
      // JS bitwise ops are 32-bit, so we can't assemble 8 bytes at once.
      auto *lo = emitUnalignedLoad(addr, 4);
      auto *addrHi = builder_.createBinaryOperatorInst(
          addr,
          builder_.getLiteralNumber(4),
          ValueKind::BinaryAddInstKind);
      addrHi->setType(Type::createNumber());
      auto *hi = emitUnalignedLoad(addrHi, 4);
      push(helpers_.emitF64ReinterpretI64(lo, hi));
    } else if (op == "f32.load") {
      // f32: load 4 bytes and reinterpret as f32.
      auto *raw = emitUnalignedLoad(addr, 4);
      push(helpers_.emitF32ReinterpretI32(raw));
    } else if (op == "i32.load16_s") {
      // The 2-byte assembly produces an unsigned 16-bit value.
      // Sign-extend via (val << 16) >> 16.
      auto *raw = emitUnalignedLoad(addr, 2);
      auto *shifted = builder_.createBinaryOperatorInst(
          raw,
          builder_.getLiteralNumber(16),
          ValueKind::BinaryLeftShiftInstKind);
      shifted->setType(Type::createNumber());
      auto *result = builder_.createBinaryOperatorInst(
          shifted,
          builder_.getLiteralNumber(16),
          ValueKind::BinaryRightShiftInstKind);
      result->setType(Type::createNumber());
      push(result);
    } else {
      auto *raw = emitUnalignedLoad(addr, numBytes);
      push(raw);
    }
    return;
  }

  // Aligned path: use typed array view.

  // Compute typed array index.
  Value *idx;
  if (shift > 0) {
    idx = builder_.createBinaryOperatorInst(
        addr,
        builder_.getLiteralNumber(shift),
        ValueKind::BinaryUnsignedRightShiftInstKind);
    idx->setType(Type::createNumber());
  } else {
    idx = addr;
  }

  // Load from the typed array view.
  auto *viewVal = loadMemView(view);
  auto *loaded = builder_.createLoadPropertyInst(viewVal, idx);

  // OOB check: typed arrays return undefined for out-of-bounds reads.
  auto *isUndef = builder_.createBinaryOperatorInst(
      loaded,
      builder_.getLiteralUndefined(),
      ValueKind::BinaryStrictlyEqualInstKind);
  auto *trapBlock = builder_.createBasicBlock(currentFunc_);
  auto *okBlock = builder_.createBasicBlock(currentFunc_);
  builder_.createCondBranchInst(isUndef, trapBlock, okBlock);

  builder_.setInsertionBlock(trapBlock);
  helpers_.emitTrap();
  builder_.createUnreachableInst();

  builder_.setInsertionBlock(okBlock);
  // We proved loaded !== undefined, so it must be a Number.
  auto *typed = builder_.createUnionNarrowTrustedInst(
      loaded, Type::createNumber());
  push(typed);
}

void WasmIRGen::onStore(
    const char *opcodeName,
    uint32_t alignLog2,
    uint32_t offset) {
  if (unreachable_)
    return;

  llvh::StringRef op(opcodeName);
  uint8_t naturalAlign = getNaturalAlignLog2(op);
  // When test262_, ignore alignment hints and always use the byte-assembly
  // path. The Wasm spec says alignment hints are advisory; engines must
  // produce correct results regardless of actual alignment.
  if (test262_)
    alignLog2 = 0;

  // i64 stores: pop i64 pair (lo, hi) then base address.
  if (op == "i64.store") {
    auto [lo, hi] = popI64();
    Value *base = pop();
    Value *addr = emitEffectiveAddr(base, offset);
    emitMemoryBoundsCheck(addr, 8);

    if (alignLog2 < naturalAlign) {
      // Unaligned: byte-store lo32 and hi32 separately.
      emitUnalignedStore(addr, lo, 4);
      auto *addrHi = builder_.createBinaryOperatorInst(
          addr,
          builder_.getLiteralNumber(4),
          ValueKind::BinaryAddInstKind);
      addrHi->setType(Type::createNumber());
      emitUnalignedStore(addrHi, hi, 4);
      return;
    }

    // Aligned path.
    auto *view = loadMemView(HEAPU32);
    auto *idx = builder_.createBinaryOperatorInst(
        addr,
        builder_.getLiteralNumber(2),
        ValueKind::BinaryUnsignedRightShiftInstKind);
    idx->setType(Type::createNumber());
    builder_.createStorePropertyStrictInst(lo, view, idx);
    auto *idx1 = builder_.createBinaryOperatorInst(
        idx,
        builder_.getLiteralNumber(1),
        ValueKind::BinaryAddInstKind);
    idx1->setType(Type::createNumber());
    builder_.createStorePropertyStrictInst(hi, view, idx1);
    return;
  }

  if (op == "i64.store8") {
    auto [lo, hi] = popI64();
    (void)hi;
    Value *base = pop();
    Value *addr = emitEffectiveAddr(base, offset);
    emitMemoryBoundsCheck(addr, 1);
    // Byte stores are always naturally aligned (natural align = 0).
    auto *view = loadMemView(HEAPU8);
    builder_.createStorePropertyStrictInst(lo, view, addr);
    return;
  }

  if (op == "i64.store16") {
    auto [lo, hi] = popI64();
    (void)hi;
    Value *base = pop();
    Value *addr = emitEffectiveAddr(base, offset);
    emitMemoryBoundsCheck(addr, 2);

    if (alignLog2 < naturalAlign) {
      // Unaligned: byte-store the lo 2 bytes.
      emitUnalignedStore(addr, lo, 2);
      return;
    }

    // Aligned path.
    auto *view = loadMemView(HEAPU16);
    auto *idx = builder_.createBinaryOperatorInst(
        addr,
        builder_.getLiteralNumber(1),
        ValueKind::BinaryUnsignedRightShiftInstKind);
    idx->setType(Type::createNumber());
    builder_.createStorePropertyStrictInst(lo, view, idx);
    return;
  }

  if (op == "i64.store32") {
    auto [lo, hi] = popI64();
    (void)hi;
    Value *base = pop();
    Value *addr = emitEffectiveAddr(base, offset);
    emitMemoryBoundsCheck(addr, 4);

    if (alignLog2 < naturalAlign) {
      // Unaligned: byte-store 4 bytes.
      emitUnalignedStore(addr, lo, 4);
      return;
    }

    // Aligned path.
    auto *view = loadMemView(HEAPU32);
    auto *idx = builder_.createBinaryOperatorInst(
        addr,
        builder_.getLiteralNumber(2),
        ValueKind::BinaryUnsignedRightShiftInstKind);
    idx->setType(Type::createNumber());
    builder_.createStorePropertyStrictInst(lo, view, idx);
    return;
  }

  // Non-i64 stores: pop value, then base address.
  Value *value = pop();
  Value *base = pop();

  Value *addr = emitEffectiveAddr(base, offset);

  MemView view;
  uint8_t shift = 0;
  uint32_t numBytes = 0;

  if (op == "i32.store") {
    view = HEAP32;
    shift = 2;
    numBytes = 4;
  } else if (op == "i32.store8") {
    view = HEAPU8;
    shift = 0;
    numBytes = 1;
  } else if (op == "i32.store16") {
    view = HEAPU16;
    shift = 1;
    numBytes = 2;
  } else if (op == "f32.store") {
    view = HEAPF32;
    shift = 2;
    numBytes = 4;
  } else if (op == "f64.store") {
    view = HEAPF64;
    shift = 3;
    numBytes = 8;
  } else {
    llvh::errs() << "WARNING: unsupported store opcode: " << op << "\n";
    return;
  }

  emitMemoryBoundsCheck(addr, numBytes);

  // Unaligned path: byte-store to HEAPU8.
  if (alignLog2 < naturalAlign) {
    if (op == "f64.store") {
      // Reinterpret f64 → i64 (split lo/hi), then byte-store each half.
      helpers_.emitI64ReinterpretF64(retBufI_, value);
      auto *lo = builder_.createLoadPropertyInst(
          retBufI_, builder_.getLiteralNumber(0));
      auto *hi = builder_.createLoadPropertyInst(
          retBufI_, builder_.getLiteralNumber(1));
      emitUnalignedStore(addr, lo, 4);
      auto *addrHi = builder_.createBinaryOperatorInst(
          addr,
          builder_.getLiteralNumber(4),
          ValueKind::BinaryAddInstKind);
      addrHi->setType(Type::createNumber());
      emitUnalignedStore(addrHi, hi, 4);
    } else if (op == "f32.store") {
      // Reinterpret f32 → i32, then byte-store.
      auto *raw = helpers_.emitI32ReinterpretF32(value);
      emitUnalignedStore(addr, raw, 4);
    } else {
      emitUnalignedStore(addr, value, numBytes);
    }
    return;
  }

  // Aligned path: use typed array view.
  Value *idx;
  if (shift > 0) {
    idx = builder_.createBinaryOperatorInst(
        addr,
        builder_.getLiteralNumber(shift),
        ValueKind::BinaryUnsignedRightShiftInstKind);
    idx->setType(Type::createNumber());
  } else {
    idx = addr;
  }

  auto *viewVal = loadMemView(view);
  builder_.createStorePropertyStrictInst(value, viewVal, idx);
}

// --- Memory size/grow (H.2) ---

void WasmIRGen::onMemorySize() {
  if (unreachable_)
    return;

  // Load the HEAPU8 view and get its .length property.
  auto *heapu8 = loadMemView(HEAPU8);
  auto *len = builder_.createLoadPropertyInst(
      heapu8, builder_.getLiteralString("length"));

  // pages = length >>> 16 (divide by 65536).
  auto *pages = builder_.createBinaryOperatorInst(
      len,
      builder_.getLiteralNumber(16),
      ValueKind::BinaryUnsignedRightShiftInstKind);
  pages->setType(Type::createNumber());

  push(pages);
}

void WasmIRGen::onMemoryGrow() {
  if (unreachable_)
    return;

  // Pop delta (number of pages to grow).
  Value *delta = pop();

  // Load the HEAPU8 view.
  auto *heapu8 = loadMemView(HEAPU8);

  // Compute old page count: oldPages = HEAPU8.length >>> 16.
  auto *len = builder_.createLoadPropertyInst(
      heapu8, builder_.getLiteralString("length"));
  auto *oldPages = builder_.createBinaryOperatorInst(
      len,
      builder_.getLiteralNumber(16),
      ValueKind::BinaryUnsignedRightShiftInstKind);
  oldPages->setType(Type::createNumber());

  // Get maximum page count.
  // For locally-defined memories, use the declared maximum (compile-time).
  // For imported memories, use the memory's OWN maximum, which wasmLinkMemory
  // read out of its maxPages_ field at validation time and which may be more
  // restrictive than the import declaration's maximum. -1 there means the
  // memory declared no maximum; wasmMemoryGrow truncates that to UINT32_MAX,
  // which its own 65536-page cap then dominates.
  Value *maxPagesVal = nullptr;
  if (!moduleInfo_.memories.empty()) {
    uint32_t maxPages = 65536; // Default: 4GB (max Wasm memory).
    if (moduleInfo_.memories[0].limits.hasMaximum) {
      maxPages = moduleInfo_.memories[0].limits.maximum;
    }
    maxPagesVal = builder_.getLiteralNumber(maxPages);
  } else if (importedMemMaxVar_) {
    maxPagesVal =
        builder_.createLoadFrameInst(parentScopeInst_, importedMemMaxVar_);
  } else {
    maxPagesVal = builder_.getLiteralNumber(65536);
  }

  // Call the grow builtin: wasmMemoryGrow(heapu8, delta, maxPages).
  // Returns new ArrayBuffer on success, or -1 on failure.
  // Pass the Memory object when there is one, so the builtin installs the
  // grown buffer on it and exported references follow the growth. Imported
  // memories have no such object yet and pass undefined.
  Value *memObjArg = memObjVar_
      ? static_cast<Value *>(
            builder_.createLoadFrameInst(parentScopeInst_, memObjVar_))
      : static_cast<Value *>(builder_.getLiteralUndefined());
  auto *result =
      helpers_.emitMemoryGrow(heapu8, delta, maxPagesVal, memObjArg);

  // Check if result === -1 (failure).
  auto *negOne = builder_.getLiteralNumber(-1);
  auto *isFailure = builder_.createBinaryOperatorInst(
      result, negOne, ValueKind::BinaryStrictlyEqualInstKind);

  // Create blocks for the conditional.
  auto *successBlock = builder_.createBasicBlock(currentFunc_);
  auto *failBlock = builder_.createBasicBlock(currentFunc_);
  auto *mergeBlock = builder_.createBasicBlock(currentFunc_);

  builder_.createCondBranchInst(isFailure, failBlock, successBlock);

  // --- Fail block: push -1 ---
  builder_.setInsertionBlock(failBlock);
  auto *failVal = builder_.getLiteralNumber(-1);
  builder_.createBranchInst(mergeBlock);

  // --- Success block: create new views and store them ---
  builder_.setInsertionBlock(successBlock);

  // result is the new ArrayBuffer. Create 8 typed array views from it.
  static const char *ctorNames[NUM_MEM_VIEWS] = {
      "Int8Array",
      "Uint8Array",
      "Int16Array",
      "Uint16Array",
      "Int32Array",
      "Uint32Array",
      "Float32Array",
      "Float64Array",
  };
  for (uint8_t i = 0; i < NUM_MEM_VIEWS; ++i) {
    auto *ctor = builder_.createTryLoadGlobalPropertyInst(ctorNames[i]);
    auto *view = emitNew(ctor, {result});
    builder_.createStoreFrameInst(parentScopeInst_, view, memViewVars_[i]);
  }

  // oldPages was computed before the branch; use it as the success value.
  builder_.createBranchInst(mergeBlock);

  // --- Merge block: phi for the result ---
  builder_.setInsertionBlock(mergeBlock);
  auto *phi = builder_.createPhiInst();
  phi->addEntry(failVal, failBlock);
  phi->addEntry(oldPages, successBlock);

  push(phi);
}

// --- Bulk memory operations (N.1) ---

void WasmIRGen::onMemoryFill() {
  if (unreachable_)
    return;

  // Stack: [dest, val, size] (top = size).
  Value *size = pop();
  Value *val = pop();
  Value *dest = pop();

  auto *heapu8 = loadMemView(HEAPU8);
  helpers_.emitMemoryFill(heapu8, dest, val, size);
}

void WasmIRGen::onMemoryCopy() {
  if (unreachable_)
    return;

  // Stack: [dest, src, size] (top = size).
  Value *size = pop();
  Value *src = pop();
  Value *dest = pop();

  auto *heapu8 = loadMemView(HEAPU8);
  helpers_.emitMemoryCopy(heapu8, dest, src, size);
}

void WasmIRGen::onMemoryInit(uint32_t segmentIndex) {
  if (unreachable_)
    return;

  // Stack: [dest, src, size] (top = size).
  Value *size = pop();
  Value *src = pop();
  Value *dest = pop();

  auto *heapu8 = loadMemView(HEAPU8);
  auto *dataSegs = builder_.createLoadFrameInst(
      parentScopeInst_, getOrCreateDataSegVar());
  auto *segIdx = builder_.getLiteralNumber(
      static_cast<double>(segmentIndex));
  helpers_.emitMemoryInit(heapu8, dataSegs, segIdx, dest, src, size);
}

void WasmIRGen::onDataDrop(uint32_t segmentIndex) {
  if (unreachable_)
    return;

  auto *dataSegs = builder_.createLoadFrameInst(
      parentScopeInst_, getOrCreateDataSegVar());
  auto *segIdx = builder_.getLiteralNumber(
      static_cast<double>(segmentIndex));
  helpers_.emitDataDrop(dataSegs, segIdx);
}

// --- Table operations (J.1) ---

void WasmIRGen::createTagObjects(Instruction *tlScope) {
  uint32_t numImported = moduleInfo_.importedTagCount();
  for (uint32_t i = numImported; i < moduleInfo_.totalTagCount(); ++i) {
    if (!tagVars_[i])
      continue;
    // One object per tag, created once per instance. Its identity is the
    // tag's identity, so two tags with the same signature stay distinct and
    // an imported tag stays equal to the exporter's.
    auto *tagObj = builder_.createAllocObjectLiteralInst({});
    builder_.createStorePropertyStrictInst(
        builder_.getLiteralString(
            buildTagTypeString(moduleInfo_.getTagType(i))),
        tagObj,
        builder_.getLiteralString("__wasm_type__"));
    builder_.createStoreFrameInst(tlScope, tagObj, tagVars_[i]);
  }
}

void WasmIRGen::internTypeIds(Instruction *tlScope) {
  internedTypeIds_ = true;
  for (uint32_t i = 0; i < moduleInfo_.types.size(); ++i) {
    // Intern the STRUCTURAL string, so the id depends only on the signature.
    // This also subsumes canonicalTypeIndex_: two structurally identical types
    // produce the same string and therefore the same id.
    auto *id = builder_.createCallBuiltinInst(
        BuiltinMethod::HermesBuiltin_wasmInternType,
        {builder_.getLiteralString(
            buildFuncTypeString(moduleInfo_.types[i]))});
    builder_.createStoreFrameInst(tlScope, id, typeIdVars_[i]);
  }
}

void WasmIRGen::createTables(Instruction *tlScope) {
  // Determine initial size for each table. Tables may be defined in the
  // table section or imported.
  uint32_t numTables = moduleInfo_.totalTableCount();

  for (uint32_t tblIdx = 0; tblIdx < numTables; ++tblIdx) {
    uint32_t importedTables = moduleInfo_.importedTableCount();

    if (tblIdx < importedTables) {
      // Imported table — arrays already wired during import validation.
      continue;
    }

    const WasmTableType &tType = moduleInfo_.tables[tblIdx - importedTables];
    uint32_t initialSize = tType.limits.initial;
    auto *sizeVal = builder_.getLiteralNumber(static_cast<double>(initialSize));

    Value *funcsArr = nullptr;
    Value *typesArr = nullptr;
    Value *exportedArr = nullptr;

    if (tType.elemType == WasmValType::FuncRef) {
      // Back a defined funcref table with a real WebAssembly.Table, and use
      // *its* funcs/types arrays as the module's storage. Exporting that same
      // object (see finalizeModule) then makes get/set/grow/length operate on
      // the module's actual table -- publishing a fresh Table, or the arrays
      // alone, leaves the exported object's own storage disconnected from the
      // module's. The storage is reached through the same brand check the
      // import path uses, and the arrays are JSArrays by construction, so no
      // wasmCheckTableArrays call is needed.
      //
      // The brand check CAN fail here: `globalThis.WebAssembly.Table` is an
      // ordinary property and script may replace it with a constructor that
      // returns anything. It is branched on for the diagnostic, not for
      // safety -- without the branch the null result reaches an indexed load
      // and reports "Cannot read property 0 of null", which names nothing and
      // points at generated code.
      auto *descriptor = builder_.createAllocObjectLiteralInst({});
      builder_.createStorePropertyStrictInst(
          builder_.getLiteralString("anyfunc"),
          descriptor,
          builder_.getLiteralString("element"));
      builder_.createStorePropertyStrictInst(
          sizeVal, descriptor, builder_.getLiteralString("initial"));
      if (tType.limits.hasMaximum) {
        builder_.createStorePropertyStrictInst(
            builder_.getLiteralNumber(
                static_cast<double>(tType.limits.maximum)),
            descriptor,
            builder_.getLiteralString("maximum"));
      }
      auto *wasmObj = builder_.createTryLoadGlobalPropertyInst("WebAssembly");
      auto *tableCtor = builder_.createLoadPropertyInst(
          wasmObj, builder_.getLiteralString("Table"));
      auto *tableObj = emitNew(tableCtor, {descriptor});
      builder_.createStoreFrameInst(tlScope, tableObj, tableObjVars_[tblIdx]);
      auto *linked =
          helpers_.emitLinkTable(tableObj, builder_.getLiteralBool(true));
      auto *ctorFunc = builder_.getInsertionBlock()->getParent();
      auto *ctorBadBB = builder_.createBasicBlock(ctorFunc);
      auto *ctorOkBB = builder_.createBasicBlock(ctorFunc);
      builder_.createCondBranchInst(
          builder_.createBinaryOperatorInst(
              linked,
              builder_.getLiteralNull(),
              ValueKind::BinaryStrictlyEqualInstKind),
          ctorBadBB,
          ctorOkBB);
      builder_.setInsertionBlock(ctorBadBB);
      helpers_.emitLinkError(builder_.getLiteralString(
          "WebAssembly.Table did not construct a table for this module's "
          "table " + std::to_string(tblIdx)));
      builder_.createUnreachableInst();
      builder_.setInsertionBlock(ctorOkBB);
      funcsArr = builder_.createLoadPropertyInst(
          linked, builder_.getLiteralNumber(0));
      typesArr = builder_.createLoadPropertyInst(
          linked, builder_.getLiteralNumber(1));
      exportedArr = builder_.createLoadPropertyInst(
          linked, builder_.getLiteralNumber(2));

      // The brand is not the whole of it, exactly as for a defined memory
      // (createMemoryViews). A hostile `globalThis.WebAssembly.Table` can
      // return a GENUINE WebAssembly.Table with limits of its own choosing,
      // and the declaration's limits are compile-time constants of what this
      // module ASKED FOR, not of what came back. Reproduced against
      // `(table 1 2 funcref)` handed a genuine Table built with
      // `{initial: 1, maximum: 1}`:
      //
      //   instantiation: linked
      //   wasm table.grow(1) -> 1 ; t.length now 2
      //   JS t.grow(0) -> RangeError: would exceed maximum
      //
      // table.grow on a defined table is bounded by the compile-time literal,
      // so the substitute was grown past its own ceiling and left with
      // maxSize_ at 1 against storage of length 2.
      //
      // EXACT equality, not the import path's >= / <=, and for the same reason
      // the memory check gives: this is not "does the supplied table satisfy a
      // declaration" but "did the constructor build the table this module
      // asked for". A genuine construction always yields exactly the requested
      // entries and exactly the requested maximum, or none, so anything else
      // means the constructor or the descriptor was interfered with -- and the
      // descriptor is reachable too, being a fresh object literal whose
      // `initial`/`maximum` stores walk the prototype chain.
      //
      // Both numbers are compared, each with its own branch: a check on only
      // one of them would let the other through.
      auto *actualLen = builder_.createLoadPropertyInst(
          funcsArr, builder_.getLiteralString("length"));
      // Index 3 is the table's OWN maximum, -1 when it declares none. Nothing
      // read it before this check existed.
      auto *actualMax = builder_.createLoadPropertyInst(
          linked, builder_.getLiteralNumber(3));
      auto *limitsBadBB = builder_.createBasicBlock(ctorFunc);
      auto *checkMaxBB = builder_.createBasicBlock(ctorFunc);
      auto *limitsOkBB = builder_.createBasicBlock(ctorFunc);
      builder_.createCondBranchInst(
          builder_.createBinaryOperatorInst(
              actualLen,
              sizeVal,
              ValueKind::BinaryStrictlyEqualInstKind),
          checkMaxBB,
          limitsBadBB);
      builder_.setInsertionBlock(checkMaxBB);
      builder_.createCondBranchInst(
          builder_.createBinaryOperatorInst(
              actualMax,
              builder_.getLiteralNumber(
                  tType.limits.hasMaximum
                      ? static_cast<double>(tType.limits.maximum)
                      : -1.0),
              ValueKind::BinaryStrictlyEqualInstKind),
          limitsOkBB,
          limitsBadBB);
      builder_.setInsertionBlock(limitsBadBB);
      helpers_.emitLinkError(builder_.getLiteralString(
          "WebAssembly.Table did not construct a table with this module's "
          "declared limits for table " + std::to_string(tblIdx)));
      builder_.createUnreachableInst();
      builder_.setInsertionBlock(limitsOkBB);
      tlEntry_ = limitsOkBB;

      builder_.createStoreFrameInst(tlScope, funcsArr, tableFuncVars_[tblIdx]);
      builder_.createStoreFrameInst(tlScope, typesArr, tableTypeVars_[tblIdx]);
      builder_.createStoreFrameInst(
          tlScope, exportedArr, tableExportVars_[tblIdx]);
    } else {
      // externref tables are not built by the Table constructor, so keep the
      // plain-array backing. These come from globalThis.Array, which script
      // can replace, so validate once here to let call_indirect cast them
      // without re-checking on every indirect call.
      auto *arrayCtor = builder_.createTryLoadGlobalPropertyInst("Array");
      funcsArr = emitNew(arrayCtor, {sizeVal});
      builder_.createStoreFrameInst(tlScope, funcsArr, tableFuncVars_[tblIdx]);
      typesArr = emitNew(arrayCtor, {sizeVal});
      builder_.createStoreFrameInst(tlScope, typesArr, tableTypeVars_[tblIdx]);
      exportedArr = emitNew(arrayCtor, {sizeVal});
      builder_.createStoreFrameInst(
          tlScope, exportedArr, tableExportVars_[tblIdx]);
      builder_.createCallBuiltinInst(
          BuiltinMethod::HermesBuiltin_wasmCheckTableArrays,
          {funcsArr, typesArr, exportedArr});
    }
  }

  // Apply active element segments.
  for (const auto &seg : moduleInfo_.elements) {
    if (seg.mode != WasmElemSegment::Mode::Active)
      continue;

    // The offset for active segments. For Phase 1, only i32.const offsets
    // are supported (global.get offsets would require globals to be
    // initialized first, which is not yet implemented).
    // An extended constant expression carries the whole computation. The
    // scalar offsetKind/offsetValue fields only record the LAST constant
    // parsed, so using them for such a segment silently places the elements
    // at the wrong index.
    Value *offset = nullptr;
    if (seg.offsetExpr.size() > 1) {
      offset = emitInitExpr(seg.offsetExpr, tlScope);
      if (!offset) {
        llvh::errs()
            << "warning: malformed element segment offset expression\n";
        continue;
      }
    } else if (seg.offsetKind == WasmGlobal::InitKind::I32Const) {
      offset = builder_.getLiteralNumber(
          static_cast<double>(seg.offsetValue));
    } else if (seg.offsetKind == WasmGlobal::InitKind::GlobalGet) {
      uint32_t slotIdx = globalSlotIndex_[seg.offsetGlobalIdx];
      offset = builder_.createLoadFrameInst(tlScope, globalVars_[slotIdx]);
    } else {
      // Unsupported offset expression — skip this segment.
      llvh::errs()
          << "warning: unsupported element segment offset expression\n";
      continue;
    }

    // Load the table arrays.
    auto *funcsArr = builder_.createLoadFrameInst(
        tlScope, tableFuncVars_[seg.tableIndex]);
    auto *typesArr = builder_.createLoadFrameInst(
        tlScope, tableTypeVars_[seg.tableIndex]);
    auto *exportedArr = builder_.createLoadFrameInst(
        tlScope, tableExportVars_[seg.tableIndex]);

    // Write each entry through the slot funnel. Only the Exported Function is
    // handed over: the funnel derives the closure and the interned type id
    // from it, so the three arrays cannot disagree about what this slot holds.
    for (uint32_t i = 0; i < seg.funcIndices.size(); ++i) {
      uint32_t funcIdx = seg.funcIndices[i];
      // Compute the table index: offset + i.
      Value *idx;
      if (i == 0 && seg.offsetValue == 0) {
        idx = builder_.getLiteralNumber(0);
      } else {
        idx = builder_.createBinaryOperatorInst(
            offset,
            builder_.getLiteralNumber(static_cast<double>(i)),
            ValueKind::BinaryAddInstKind);
      }

      // Every function index named by an element segment is in
      // escapableFuncs_, so it has a canonical Exported Function.
      if (funcIdx < exportedFuncVars_.size() && exportedFuncVars_[funcIdx]) {
        helpers_.emitTableSetSlot(
            funcsArr,
            typesArr,
            exportedArr,
            idx,
            builder_.createLoadFrameInst(
                tlScope, exportedFuncVars_[funcIdx]),
            // An element segment names functions, so this is a funcref table.
            builder_.getLiteralNumber(1));
      }
    }
  }
}

Value *WasmIRGen::loadTableFuncs(uint32_t tableIndex) {
  assert(
      tableIndex < tableFuncVars_.size() && "table index out of range");
  return builder_.createLoadFrameInst(
      parentScopeInst_, tableFuncVars_[tableIndex]);
}

Value *WasmIRGen::loadTableTypes(uint32_t tableIndex) {
  assert(
      tableIndex < tableTypeVars_.size() && "table index out of range");
  return builder_.createLoadFrameInst(
      parentScopeInst_, tableTypeVars_[tableIndex]);
}

Value *WasmIRGen::loadTableExported(uint32_t tableIndex) {
  assert(
      tableIndex < tableExportVars_.size() && "table index out of range");
  return builder_.createLoadFrameInst(
      parentScopeInst_, tableExportVars_[tableIndex]);
}

bool WasmIRGen::tableIsFuncRef(uint32_t tableIndex) const {
  uint32_t importedTables = moduleInfo_.importedTableCount();
  if (tableIndex < importedTables) {
    uint32_t idx = 0;
    for (const auto &imp : moduleInfo_.imports) {
      if (imp.kind != WasmExternalKind::Table)
        continue;
      if (idx == tableIndex)
        return imp.tableType.elemType == WasmValType::FuncRef;
      ++idx;
    }
    return true;
  }
  uint32_t definedIdx = tableIndex - importedTables;
  if (definedIdx >= moduleInfo_.tables.size())
    return true;
  return moduleInfo_.tables[definedIdx].elemType == WasmValType::FuncRef;
}

Value *WasmIRGen::tableIsFuncRefLiteral(uint32_t tableIndex) {
  return builder_.getLiteralNumber(tableIsFuncRef(tableIndex) ? 1 : 0);
}

void WasmIRGen::emitTableBoundsCheck(Value *idx, Value *funcsArr) {
  auto *length = builder_.createLoadPropertyInst(
      funcsArr, builder_.getLiteralString("length"));
  // Unsigned comparison: (idx >>> 0) >= length
  auto *idxU = builder_.createBinaryOperatorInst(
      idx,
      builder_.getLiteralNumber(0),
      ValueKind::BinaryUnsignedRightShiftInstKind);
  auto *isOOB = builder_.createBinaryOperatorInst(
      idxU, length, ValueKind::BinaryGreaterThanOrEqualInstKind);
  auto *trapBlock = builder_.createBasicBlock(currentFunc_);
  auto *okBlock = builder_.createBasicBlock(currentFunc_);
  builder_.createCondBranchInst(isOOB, trapBlock, okBlock);

  builder_.setInsertionBlock(trapBlock);
  helpers_.emitTrap();
  builder_.createUnreachableInst();

  builder_.setInsertionBlock(okBlock);
}

void WasmIRGen::onTableGet(uint32_t tableIndex) {
  if (unreachable_)
    return;

  Value *idx = pop();
  // Bounds-check against the closure array, which is the one whose length is
  // the table's size, but yield the EXPORTED FUNCTION. This is the single
  // change that keeps internal closures off the Wasm value stack: everything a
  // funcref value can subsequently reach -- a table.set, a funcref result, an
  // argument to an import trampoline, JS -- sees the wrapper instead.
  auto *funcsArr = loadTableFuncs(tableIndex);
  emitTableBoundsCheck(idx, funcsArr);
  auto *result =
      helpers_.emitTableGetSlot(loadTableExported(tableIndex), idx);
  push(result);
}

void WasmIRGen::onTableSet(uint32_t tableIndex) {
  if (unreachable_)
    return;

  Value *val = pop();
  Value *idx = pop();
  auto *funcsArr = loadTableFuncs(tableIndex);
  emitTableBoundsCheck(idx, funcsArr);
  // Through the funnel: writing the closure array alone left the previous
  // occupant's type id in place, which is how a function became callable
  // through another function's signature.
  helpers_.emitTableSetSlot(
      funcsArr,
      loadTableTypes(tableIndex),
      loadTableExported(tableIndex),
      idx,
      val,
      tableIsFuncRefLiteral(tableIndex));
}

void WasmIRGen::onTableSize(uint32_t tableIndex) {
  if (unreachable_)
    return;

  auto *funcsArr = loadTableFuncs(tableIndex);
  auto *length = builder_.createLoadPropertyInst(
      funcsArr, builder_.getLiteralString("length"));
  push(length);
}

void WasmIRGen::onTableGrow(uint32_t tableIndex) {
  if (unreachable_)
    return;

  // table.grow pops: delta (top), fill value.
  // Pushes: old size on success, -1 on failure.
  auto *delta = pop();
  auto *fillVal = pop();

  auto *funcsArr = loadTableFuncs(tableIndex);
  auto *typesArr = loadTableTypes(tableIndex);
  auto *exportedArr = loadTableExported(tableIndex);

  // Get maximum table size from module info.
  uint32_t maxEntries = UINT32_MAX;
  uint32_t importedTables = moduleInfo_.importedTableCount();
  if (tableIndex < importedTables) {
    uint32_t importTableIdx = 0;
    for (const auto &imp : moduleInfo_.imports) {
      if (imp.kind != WasmExternalKind::Table)
        continue;
      if (importTableIdx == tableIndex) {
        if (imp.tableType.limits.hasMaximum)
          maxEntries = imp.tableType.limits.maximum;
        break;
      }
      ++importTableIdx;
    }
  } else {
    const auto &tbl =
        moduleInfo_.tables[tableIndex - importedTables];
    if (tbl.limits.hasMaximum)
      maxEntries = tbl.limits.maximum;
  }

  // For an imported table the declaration is not the only bound: the table
  // actually supplied has a maximum of its own, and link validation only
  // requires the actual max to be <= the declared one. Growing to the
  // declared max would push a shared table past what its owner allows.
  Value *actualMax = builder_.getLiteralNumber(-1);
  if (tableIndex < importedTables &&
      importedTableMaxVars_[tableIndex]) {
    actualMax = builder_.createLoadFrameInst(
        parentScopeInst_, importedTableMaxVars_[tableIndex]);
  }

  auto *result = helpers_.emitTableGrow(
      funcsArr,
      typesArr,
      exportedArr,
      delta,
      fillVal,
      builder_.getLiteralNumber(static_cast<double>(maxEntries)),
      actualMax,
      tableIsFuncRefLiteral(tableIndex));

  push(result);
}

// --- Bulk table operations (N.2) ---

void WasmIRGen::onTableFill(uint32_t tableIndex) {
  if (unreachable_)
    return;

  // Stack: [idx, val, count] (top = count).
  Value *count = pop();
  Value *val = pop();
  Value *idx = pop();

  helpers_.emitTableFill(
      loadTableFuncs(tableIndex),
      loadTableTypes(tableIndex),
      loadTableExported(tableIndex),
      idx,
      val,
      count,
      tableIsFuncRefLiteral(tableIndex));
}

void WasmIRGen::onTableCopy(
    uint32_t dstTableIndex,
    uint32_t srcTableIndex) {
  if (unreachable_)
    return;

  // Stack: [dst, src, count] (top = count).
  Value *count = pop();
  Value *src = pop();
  Value *dst = pop();

  helpers_.emitTableCopySlots(
      loadTableFuncs(dstTableIndex),
      loadTableTypes(dstTableIndex),
      loadTableExported(dstTableIndex),
      loadTableFuncs(srcTableIndex),
      loadTableTypes(srcTableIndex),
      loadTableExported(srcTableIndex),
      dst,
      src,
      count);
}

void WasmIRGen::onTableInit(
    uint32_t segmentIndex,
    uint32_t tableIndex) {
  if (unreachable_)
    return;

  // Stack: [dst, src, count] (top = count).
  Value *count = pop();
  Value *src = pop();
  Value *dst = pop();

  auto *funcsArr = loadTableFuncs(tableIndex);
  auto *typesArr = loadTableTypes(tableIndex);
  auto *exportedArr = loadTableExported(tableIndex);
  auto *elemSegs = builder_.createLoadFrameInst(
      parentScopeInst_, getOrCreateElemSegVar());
  auto *segIdx =
      builder_.getLiteralNumber(static_cast<double>(segmentIndex));
  helpers_.emitTableInit(
      funcsArr, typesArr, exportedArr, elemSegs, segIdx, dst, src, count);
}

void WasmIRGen::onRefNull() {
  if (unreachable_)
    return;
  push(builder_.getLiteralNull());
}

void WasmIRGen::onElemDrop(uint32_t segmentIndex) {
  if (unreachable_)
    return;

  auto *elemSegs = builder_.createLoadFrameInst(
      parentScopeInst_, getOrCreateElemSegVar());
  auto *segIdx =
      builder_.getLiteralNumber(static_cast<double>(segmentIndex));
  helpers_.emitElemDrop(elemSegs, segIdx);
}

// --- Globals (K.1) ---

Value *WasmIRGen::emitInitExpr(
    const std::vector<InitExprOp> &expr,
    Instruction *tlScope) {
  llvh::SmallVector<Value *, 4> stack;
  for (const auto &op : expr) {
    switch (op.kind) {
      case InitExprOp::Kind::I32Const:
        stack.push_back(
            builder_.getLiteralNumber(static_cast<double>(op.i32Val)));
        break;
      case InitExprOp::Kind::GlobalGet: {
        if (op.globalIdx >= globalSlotIndex_.size())
          return nullptr;
        uint32_t slotIdx = globalSlotIndex_[op.globalIdx];
        stack.push_back(
            builder_.createLoadFrameInst(tlScope, globalVars_[slotIdx]));
        break;
      }
      case InitExprOp::Kind::I32Add:
      case InitExprOp::Kind::I32Sub:
      case InitExprOp::Kind::I32Mul: {
        // Not an assert: the AOT path does not validate, so a malformed
        // module can under-run this stack. Popping an empty SmallVector
        // wraps its size and corrupts memory.
        if (stack.size() < 2)
          return nullptr;
        Value *rhs = stack.pop_back_val();
        Value *lhs = stack.pop_back_val();
        ValueKind binOp = op.kind == InitExprOp::Kind::I32Add
            ? ValueKind::BinaryAddInstKind
            : op.kind == InitExprOp::Kind::I32Sub
            ? ValueKind::BinarySubtractInstKind
            : ValueKind::BinaryMultiplyInstKind;
        stack.push_back(builder_.createBinaryOperatorInst(
            builder_.createBinaryOperatorInst(lhs, rhs, binOp),
            builder_.getLiteralNumber(0),
            ValueKind::BinaryOrInstKind));
        break;
      }
    }
  }
  if (stack.size() != 1)
    return nullptr;
  return stack[0];
}

void WasmIRGen::initializeGlobals(Instruction *tlScope) {
  uint32_t numImportedGlobals = moduleInfo_.importedGlobalCount();

  // Initialize imported globals from the validated import values.
  // During import validation, the resolved import was stored in
  // importGlobalVals_[i]: the value for an immutable import, the
  // WebAssembly.Global object for a mutable one.
  for (uint32_t i = 0; i < numImportedGlobals; ++i) {
    uint32_t slotIdx = globalSlotIndex_[i];

    // Find the i-th global import to determine its type.
    WasmValType gType = WasmValType::I32;
    uint32_t importGlobalIdx = 0;
    for (const auto &imp : moduleInfo_.imports) {
      if (imp.kind != WasmExternalKind::Global)
        continue;
      if (importGlobalIdx == i) {
        gType = imp.globalType.type;
        break;
      }
      ++importGlobalIdx;
    }

    // The import was already resolved during import validation, under the
    // brand check performed there. Asking the supplied object again here --
    // whether by re-running the check or by reading anything off it -- would
    // be a TOCTOU: a getter or Proxy can answer differently the second time.
    Value *resolvedVal = builder_.createLoadFrameInst(
        tlScope, importGlobalVals_[i]);

    // A mutable import resolves to the WebAssembly.Global object, which
    // global.get/global.set read and write directly. The frame slots below
    // therefore only hold a link-time snapshot of it, for the constant
    // expressions (data/element offsets, defined-global initializers) that
    // read a global's slot -- and Wasm validation restricts those to
    // immutable imported globals anyway.
    //
    // Through the builtin, not through `.value`: that accessor is
    // configurable, so taking the snapshot as a property read ran user JS
    // inside instantiation -- once per mutable global import -- and let it
    // choose the value every constant expression would then see.
    if (importedMutableGlobals_.count(i)) {
      resolvedVal = helpers_.emitGlobalGet(resolvedVal);
    }

    // Coerce to the declared Wasm type. i64 is left alone: it goes through
    // the BigInt lo/hi split below. This is load-bearing for an IMMUTABLE
    // import satisfied by a raw JS value -- `typeof x === 'number'` admits
    // 3.7 and 2^32+5 -- and a NO-OP for a mutable one, whose value came out
    // of an internal field two lines up.
    //
    // Narrowing it to the immutable case is a one-line `if`, not a phi split:
    // the two kinds are decided here at compile time by
    // importedMutableGlobals_, and a mutable import's `resolved` phi has
    // exactly ONE entry anyway, because rawAllowed is !mutable_ so checkRawBB
    // is never created for it. It is left unnarrowed deliberately, so that it
    // is retired in one place together with the raw-value path it is really
    // there for -- Task 6's J4 item. That is a scheduling choice; there is no
    // structural obstacle.
    resolvedVal = coerceImportedGlobalValue(resolvedVal, gType);

    if (gType == WasmValType::I64) {
      // An i64 global's value is a BigInt. Split it into the lo/hi pair the
      // compiler represents i64 with; storing it raw would put a BigInt where
      // every i64 operation expects a Number, and hard-coding hi to 0
      // silently truncated every imported i64 to its low word.
      auto *rbI = builder_.createLoadFrameInst(tlScope, retBufIVar_);
      helpers_.emitBigIntToI64(rbI, resolvedVal);
      // rbI is a Uint32Array, so these read back unsigned, but an i64 half
      // is a signed int32 everywhere else in the pipeline (see
      // emitRetBufLoads / the mutable-global path below): narrow with
      // AsInt32Inst like every other buffer read, or i32.wrap_i64 on an
      // imported global of e.g. -1n yields 4294967295 instead of -1. Same
      // mistake as C2 (the export-wrapper i64 param unmarshal), here in the
      // immutable-import snapshot.
      builder_.createStoreFrameInst(
          tlScope,
          builder_.createAsInt32Inst(builder_.createLoadPropertyInst(
              rbI, builder_.getLiteralNumber(0))),
          globalVars_[slotIdx]);
      builder_.createStoreFrameInst(
          tlScope,
          builder_.createAsInt32Inst(builder_.createLoadPropertyInst(
              rbI, builder_.getLiteralNumber(1))),
          globalVars_[slotIdx + 1]);
    } else {
      builder_.createStoreFrameInst(
          tlScope, resolvedVal, globalVars_[slotIdx]);
    }
  }

  // Initialize defined globals from their init expressions.
  for (uint32_t di = 0; di < moduleInfo_.globals.size(); ++di) {
    uint32_t globalIdx = numImportedGlobals + di;
    uint32_t slotIdx = globalSlotIndex_[globalIdx];
    const WasmGlobal &g = moduleInfo_.globals[di];

    // An extended constant expression carries the whole computation. The
    // scalar initKind/initValue fields only record the LAST constant parsed,
    // so using them for such a global silently initializes it to that
    // constant instead of the computed value.
    if (g.initExpr.size() > 1) {
      if (Value *init = emitInitExpr(g.initExpr, tlScope)) {
        builder_.createStoreFrameInst(tlScope, init, globalVars_[slotIdx]);
        continue;
      }
      llvh::errs()
          << "warning: malformed global init expression\n";
    }

    switch (g.initKind) {
      case WasmGlobal::InitKind::I32Const:
        builder_.createStoreFrameInst(
            tlScope,
            builder_.getLiteralNumber(
                static_cast<double>(g.initValue.i32Val)),
            globalVars_[slotIdx]);
        break;

      case WasmGlobal::InitKind::I64Const: {
        // Split i64 into lo32 and hi32.
        int64_t val = g.initValue.i64Val;
        int32_t lo = static_cast<int32_t>(val & 0xFFFFFFFF);
        int32_t hi = static_cast<int32_t>(
            static_cast<uint64_t>(val) >> 32);
        builder_.createStoreFrameInst(
            tlScope,
            builder_.getLiteralNumber(static_cast<double>(lo)),
            globalVars_[slotIdx]);
        builder_.createStoreFrameInst(
            tlScope,
            builder_.getLiteralNumber(static_cast<double>(hi)),
            globalVars_[slotIdx + 1]);
        break;
      }

      case WasmGlobal::InitKind::F32Const:
        builder_.createStoreFrameInst(
            tlScope,
            builder_.getLiteralNumber(
                static_cast<double>(g.initValue.f32Val)),
            globalVars_[slotIdx]);
        break;

      case WasmGlobal::InitKind::F64Const:
        builder_.createStoreFrameInst(
            tlScope,
            builder_.getLiteralNumber(g.initValue.f64Val),
            globalVars_[slotIdx]);
        break;

      case WasmGlobal::InitKind::GlobalGet: {
        // Initialize from another global's current value.
        uint32_t srcIdx = g.initValue.globalIndex;
        assert(srcIdx < globalIdx && "forward global reference in init expr");
        uint32_t srcSlotIdx = globalSlotIndex_[srcIdx];
        auto *val = builder_.createLoadFrameInst(
            tlScope, globalVars_[srcSlotIdx]);
        builder_.createStoreFrameInst(
            tlScope, val, globalVars_[slotIdx]);

        // If the target is i64, also copy the hi32 slot.
        if (g.type.type == WasmValType::I64) {
          auto *hiVal = builder_.createLoadFrameInst(
              tlScope, globalVars_[srcSlotIdx + 1]);
          builder_.createStoreFrameInst(
              tlScope, hiVal, globalVars_[slotIdx + 1]);
        }
        break;
      }

      case WasmGlobal::InitKind::RefNull:
        builder_.createStoreFrameInst(
            tlScope,
            builder_.getLiteralNull(),
            globalVars_[slotIdx]);
        break;

      case WasmGlobal::InitKind::RefFunc:
        // Store the Exported Function, not the internal closure: a funcref is
        // the wrapper everywhere, and global.get pushes this straight onto the
        // value stack, from where table.set and the export boundary take it.
        // computeEscapableFuncs() puts every ref.func global initializer in
        // escapableFuncs_, so the wrapper exists.
        if (g.initValue.funcIndex < exportedFuncVars_.size() &&
            exportedFuncVars_[g.initValue.funcIndex]) {
          auto *wrapper = builder_.createLoadFrameInst(
              tlScope, exportedFuncVars_[g.initValue.funcIndex]);
          builder_.createStoreFrameInst(
              tlScope, wrapper, globalVars_[slotIdx]);
        } else {
          builder_.createStoreFrameInst(
              tlScope,
              builder_.getLiteralNull(),
              globalVars_[slotIdx]);
        }
        break;
    }
  }
}

Value *WasmIRGen::coerceImportedGlobalValue(Value *value, WasmValType type) {
  switch (type) {
    case WasmValType::I32:
      return builder_.createAsInt32Inst(value);
    case WasmValType::F32:
      return emitFround(builder_.createAsNumberInst(value));
    case WasmValType::F64:
      return builder_.createAsNumberInst(value);
    default:
      return value;
  }
}

void WasmIRGen::onGlobalGet(uint32_t globalIndex) {
  if (unreachable_)
    return;

  assert(globalIndex < globalSlotIndex_.size() && "global index out of range");
  uint32_t slotIdx = globalSlotIndex_[globalIndex];

  // Determine the global's type.
  uint32_t numImportedGlobals = moduleInfo_.importedGlobalCount();
  WasmValType gType = WasmValType::I32;
  if (globalIndex < numImportedGlobals) {
    uint32_t importGlobalIdx = 0;
    for (const auto &imp : moduleInfo_.imports) {
      if (imp.kind != WasmExternalKind::Global)
        continue;
      if (importGlobalIdx == globalIndex) {
        gType = imp.globalType.type;
        break;
      }
      ++importGlobalIdx;
    }
  } else {
    gType = moduleInfo_.globals[globalIndex - numImportedGlobals].type.type;
  }

  // An imported mutable global is shared state: its value lives in the
  // host's WebAssembly.Global, which the host can write at any time. Read it
  // out of that object; the frame slot only holds the link-time snapshot, so
  // reading that would miss every write since instantiation (H12).
  //
  // Through wasmGlobalGet, which brand-checks and reads the internal field,
  // NOT through `.value`. `value` is a CONFIGURABLE accessor on
  // WebAssembly.Global.prototype: replacing it fed this line 999 for a global
  // holding 77. Reaching the field is what keeps the sharing and drops the
  // interposition; snapshotting the value instead would be H12 again.
  if (importedMutableGlobals_.count(globalIndex)) {
    auto *globalObj = builder_.createLoadFrameInst(
        parentScopeInst_, importGlobalVals_[globalIndex]);
    auto *value = helpers_.emitGlobalGet(globalObj);
    if (gType == WasmValType::I64) {
      // An i64 global's value crosses the JS boundary as a BigInt. Split it
      // into the lo/hi pair the compiler represents i64 with.
      auto *rbI = builder_.createLoadFrameInst(
          parentScopeInst_, retBufIVar_);
      helpers_.emitBigIntToI64(rbI, value);
      auto *lo = builder_.createLoadPropertyInst(
          rbI, builder_.getLiteralNumber(0));
      auto *hi = builder_.createLoadPropertyInst(
          rbI, builder_.getLiteralNumber(1));
      // The buffer is a Uint32Array, but an i64 half is a signed int32
      // everywhere else in the pipeline (see emitRetBufLoads), so convert.
      // Pushing the unsigned value would let i32.wrap_i64 yield 4294967295
      // where the Wasm value is -1.
      pushI64(
          builder_.createAsInt32Inst(lo), builder_.createAsInt32Inst(hi));
    } else {
      // This coercion IS A NO-OP on this path and is kept deliberately.
      // Read that as a statement about scope, not about trust: the value now
      // comes out of value_ through wasmGlobalGet, and setWasmGlobalNumber is
      // the only writer of that field, so it is already an int32-valued
      // double for an i32 global and float-representable for an f32 one --
      // and wasmLinkGlobal refused the import unless the Global's type
      // matched the declaration. Measured: deleting it here leaves every
      // behavioural test green, and fails only irgen-global-mutable-shared
      // .wat, which pins the instruction's presence precisely so that its
      // removal is a deliberate act. Retiring it belongs with Task 6's J4
      // work, which owns the other, still load-bearing caller (a raw JS value
      // satisfying an IMMUTABLE import, checked only with typeof).
      push(coerceImportedGlobalValue(value, gType));
    }
    return;
  }

  auto *val = builder_.createLoadFrameInst(
      parentScopeInst_, globalVars_[slotIdx]);

  if (gType == WasmValType::I64) {
    auto *hiVal = builder_.createLoadFrameInst(
        parentScopeInst_, globalVars_[slotIdx + 1]);
    pushI64(val, hiVal);
  } else {
    push(val);
  }
}

void WasmIRGen::onGlobalSet(uint32_t globalIndex) {
  if (unreachable_)
    return;

  assert(globalIndex < globalSlotIndex_.size() && "global index out of range");
  uint32_t slotIdx = globalSlotIndex_[globalIndex];

  // Determine the global's type.
  uint32_t numImportedGlobals = moduleInfo_.importedGlobalCount();
  WasmValType gType = WasmValType::I32;
  if (globalIndex < numImportedGlobals) {
    uint32_t importGlobalIdx = 0;
    for (const auto &imp : moduleInfo_.imports) {
      if (imp.kind != WasmExternalKind::Global)
        continue;
      if (importGlobalIdx == globalIndex) {
        gType = imp.globalType.type;
        break;
      }
      ++importGlobalIdx;
    }
  } else {
    gType = moduleInfo_.globals[globalIndex - numImportedGlobals].type.type;
  }

  // An imported mutable global is shared state: write through the host's
  // WebAssembly.Global, which is what makes the write visible to the host
  // and to every other module importing the same global. Writing the frame
  // slot instead would leave the write inside this instance.
  //
  // Through wasmGlobalSet, which brand-checks and writes the internal field,
  // NOT through a `.value` store. `value` is a CONFIGURABLE accessor pair on
  // WebAssembly.Global.prototype, so a replaced setter simply swallowed the
  // write -- verified: after the module's global.set(5) the real global still
  // held 77.
  if (importedMutableGlobals_.count(globalIndex)) {
    Value *value;
    if (gType == WasmValType::I64) {
      // An i64 global's value crosses the JS boundary as a BigInt.
      auto [lo, hi] = popI64();
      value = helpers_.emitI64ToBigInt(lo, hi);
    } else {
      value = pop();
    }
    auto *globalObj = builder_.createLoadFrameInst(
        parentScopeInst_, importGlobalVals_[globalIndex]);
    helpers_.emitGlobalSet(globalObj, value);
    return;
  }

  if (gType == WasmValType::I64) {
    auto [lo, hi] = popI64();
    builder_.createStoreFrameInst(
        parentScopeInst_, lo, globalVars_[slotIdx]);
    builder_.createStoreFrameInst(
        parentScopeInst_, hi, globalVars_[slotIdx + 1]);
  } else {
    Value *val = pop();
    builder_.createStoreFrameInst(
        parentScopeInst_, val, globalVars_[slotIdx]);
  }
}

} // namespace wasm
} // namespace hermes
