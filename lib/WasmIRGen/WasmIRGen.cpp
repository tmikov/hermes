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

WasmIRGen::WasmIRGen(Module &M, WasmModuleInfo &moduleInfo)
    : moduleInfo_(moduleInfo), builder_(&M), helpers_(builder_) {}

void WasmIRGen::createFunctions() {
  // Create the top-level function first (must be before other functions).
  auto *topLevel = builder_.createTopLevelFunction(
      "global", true /* strictMode */);
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
  uint32_t numTables = moduleInfo_.totalTableCount();
  if (numTables > 0) {
    tableFuncVars_.resize(numTables, nullptr);
    tableTypeVars_.resize(numTables, nullptr);
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

  // Create all Wasm functions and a Variable in the top-level scope for each.
  uint32_t totalFuncs = moduleInfo_.totalFunctionCount();
  irFunctions_.resize(totalFuncs, nullptr);
  closureVars_.resize(totalFuncs, nullptr);

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

    // Create a variable in the top-level scope to hold the pre-created closure.
    closureVars_[i] = builder_.createVariable(
        topLevelVS_,
        ("closure_" + llvh::Twine(i)),
        Type::createAnyType(),
        /* hidden */ true);

    // Add a "this" parameter (required by Hermes calling convention).
    builder_.createJSThisParam(func);

    // Add JSDynamicParams per Wasm parameter. i64 params need two slots
    // (lo32, hi32) for the split representation.
    uint32_t jsParamCount = 0;
    for (uint32_t p = 0; p < funcType.params.size(); ++p) {
      if (funcType.params[p] == WasmValType::I64) {
        builder_.createJSDynamicParam(
            func, ("p" + llvh::Twine(p) + "_lo").str());
        builder_.createJSDynamicParam(
            func, ("p" + llvh::Twine(p) + "_hi").str());
        jsParamCount += 2;
      } else {
        builder_.createJSDynamicParam(
            func, ("p" + llvh::Twine(p)).str());
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

  // Populate the top-level function body.
  // Create all closures once and store them in the top-level scope.
  tlEntry_ = builder_.createBasicBlock(topLevel);
  builder_.setInsertionBlock(tlEntry_);

  // Create a scope for the top-level function.
  tlScope_ = builder_.createCreateScopeInst(
      topLevelVS_, builder_.getEmptySentinel());
  auto *tlScope = tlScope_;

  // Resolve imported functions from the imports object.
  // The imports object is read from the global `__wasm_imports__` property.
  // It has the shape: { moduleName: { fieldName: func } }.
  // When M.4 (WebAssembly.Instance) is implemented, the imports will be
  // passed via the Instance constructor and set on the global before
  // evaluating the compiled module.
  if (numImportedFuncs > 0) {
    auto *importsVal = builder_.createTryLoadGlobalPropertyInst(
        builder_.getLiteralString("__wasm_imports__"));
    uint32_t importFuncIdx = 0;
    for (const auto &imp : moduleInfo_.imports) {
      if (imp.kind != WasmExternalKind::Function)
        continue;
      // imports[moduleName][fieldName]
      auto *moduleObj = builder_.createLoadPropertyInst(
          importsVal, builder_.getLiteralString(imp.moduleName));
      auto *funcVal = builder_.createLoadPropertyInst(
          moduleObj, builder_.getLiteralString(imp.fieldName));
      builder_.createStoreFrameInst(
          tlScope, funcVal, importFuncVars_[importFuncIdx]);
      ++importFuncIdx;
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

  // Create and initialize tables, and apply element segments.
  if (numTables > 0) {
    createTables(tlScope);
  }

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

  // Switch back to the top-level entry block after creating trampolines.
  // finalizeModule() will continue from here.
  builder_.setInsertionBlock(tlEntry_);
}

void WasmIRGen::finalizeModule() {
  auto *tlScope = tlScope_;
  bool hasMemory = moduleInfo_.totalMemoryCount() > 0;

  // Ensure insertion is at the top-level entry block.
  builder_.setInsertionBlock(tlEntry_);

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
        continue;
      }

      // Create a Uint8Array and fill it with segment data.
      auto *segArr = emitNew(
          Uint8ArrayCtor,
          {builder_.getLiteralNumber(
              static_cast<double>(seg.data.size()))});
      for (uint32_t bi = 0; bi < seg.data.size(); ++bi) {
        builder_.createStorePropertyStrictInst(
            builder_.getLiteralNumber(
                static_cast<double>(seg.data[bi])),
            segArr,
            builder_.getLiteralNumber(static_cast<double>(bi)));
      }
      builder_.createStorePropertyStrictInst(
          segArr,
          segsArr,
          builder_.getLiteralNumber(static_cast<double>(si)));
    }
  }

  // Apply active data segments: copy bytes into linear memory.
  if (hasMemory) {
    for (uint32_t si = 0; si < moduleInfo_.dataSegments.size(); ++si) {
      const auto &seg = moduleInfo_.dataSegments[si];
      if (seg.mode != WasmDataSegment::Mode::Active)
        continue;
      if (seg.data.empty())
        continue;

      // Compute offset.
      Value *offset = nullptr;
      if (seg.offsetKind == WasmGlobal::InitKind::I32Const) {
        offset = builder_.getLiteralNumber(
            static_cast<double>(seg.offsetValue));
      } else {
        llvh::errs()
            << "warning: unsupported data segment offset expression\n";
        continue;
      }

      // Load HEAPU8 view for byte-level writes.
      auto *heapu8 = builder_.createLoadFrameInst(
          tlScope, memViewVars_[static_cast<uint8_t>(MemView::HEAPU8)]);

      // Store each byte of the data segment.
      for (uint32_t i = 0; i < seg.data.size(); ++i) {
        Value *idx;
        if (i == 0) {
          idx = offset;
        } else {
          idx = builder_.createBinaryOperatorInst(
              offset,
              builder_.getLiteralNumber(static_cast<double>(i)),
              ValueKind::BinaryAddInstKind);
        }
        builder_.createStorePropertyStrictInst(
            builder_.getLiteralNumber(static_cast<double>(seg.data[i])),
            heapu8,
            idx);
      }

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
  // Each element is a JS Array of interleaved [func, typeIdx, func, typeIdx, ...]
  // or null for segments that have been dropped.
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

      // Create an interleaved array: [func0, typeIdx0, func1, typeIdx1, ...]
      uint32_t numEntries = seg.funcIndices.size();
      auto *segArr = emitNew(
          builder_.createTryLoadGlobalPropertyInst("Array"),
          {builder_.getLiteralNumber(
              static_cast<double>(numEntries * 2))});

      for (uint32_t i = 0; i < numEntries; ++i) {
        uint32_t funcIdx = seg.funcIndices[i];

        // Store the closure.
        if (funcIdx < closureVars_.size()) {
          auto *closure = builder_.createLoadFrameInst(
              tlScope, closureVars_[funcIdx]);
          builder_.createStorePropertyStrictInst(
              closure,
              segArr,
              builder_.getLiteralNumber(static_cast<double>(i * 2)));
        } else {
          builder_.createStorePropertyStrictInst(
              builder_.getLiteralNull(),
              segArr,
              builder_.getLiteralNumber(static_cast<double>(i * 2)));
        }

        // Store the type index.
        uint32_t typeIdx = 0;
        if (funcIdx < moduleInfo_.importedFunctionCount()) {
          uint32_t importFuncIdx = 0;
          for (const auto &imp : moduleInfo_.imports) {
            if (imp.kind != WasmExternalKind::Function)
              continue;
            if (importFuncIdx == funcIdx) {
              typeIdx = imp.typeIndex;
              break;
            }
            ++importFuncIdx;
          }
        } else {
          typeIdx = moduleInfo_
                        .functions[funcIdx -
                                   moduleInfo_.importedFunctionCount()]
                        .typeIndex;
        }
        builder_.createStorePropertyStrictInst(
            builder_.getLiteralNumber(static_cast<double>(typeIdx)),
            segArr,
            builder_.getLiteralNumber(static_cast<double>(i * 2 + 1)));
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
          /* newTarget */ builder_.getLiteralUndefined(),
          /* thisValue */ builder_.getLiteralUndefined(),
          {});
    }
  }

  // Build export wrapper functions and the exports object.
  // For each exported function, create a wrapper that presents a clean
  // JS-compatible interface (1 param per Wasm param, argument coercion,
  // return value marshaling). The exports object maps export names to
  // wrapper closures. Only function exports are handled; other export kinds
  // (memory, table, global) are silently skipped for now.

  // Create wrapper functions first (this switches insertion point).
  struct ExportWrapperInfo {
    std::string name;
    Function *wrapperFunc;
  };
  std::vector<ExportWrapperInfo> wrappers;
  for (const auto &exp : moduleInfo_.exports) {
    if (exp.kind != WasmExternalKind::Function)
      continue;
    assert(
        exp.index < closureVars_.size() &&
        "export function index out of range");
    auto *wrapperFunc =
        createExportWrapper(exp.index, exp.name, tlScope);
    wrappers.push_back({exp.name, wrapperFunc});
  }

  // Switch back to the top-level entry block to emit closures and the
  // exports object.
  builder_.setInsertionBlock(tlEntry_);

  auto *exportsObj = builder_.createAllocObjectLiteralInst({});
  for (const auto &w : wrappers) {
    auto *wrapperClosure = builder_.createCreateFunctionInst(
        tlScope, w.wrapperFunc);
    builder_.createStorePropertyStrictInst(
        wrapperClosure, exportsObj, builder_.getLiteralString(w.name));
  }

  builder_.createReturnInst(exportsObj);
}

Function *WasmIRGen::createExportWrapper(
    uint32_t funcIndex,
    llvh::StringRef exportName,
    Instruction *tlScope) {
  const WasmFuncType &funcType = moduleInfo_.getFunctionType(funcIndex);

  // Create the wrapper function.
  auto *wrapperFunc = builder_.createFunction(
      ("wasm_export_" + exportName).str(),
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
        auto *lo = helpers_.emitBigIntToI64(paramVal);
        auto *hi = helpers_.emitI64HiResult();
        callArgs.push_back(lo);
        callArgs.push_back(hi);
        break;
      }
      case WasmValType::F32:
      case WasmValType::F64:
        // Float/double: pass through (already a JS Number).
        callArgs.push_back(paramVal);
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
      /* newTarget */ builder_.getLiteralUndefined(),
      /* thisValue */ builder_.getLiteralUndefined(),
      callArgs);

  // Marshal the return value.
  if (funcType.results.empty()) {
    // Void function: return undefined.
    builder_.createReturnInst(builder_.getLiteralUndefined());
  } else if (funcType.results[0] == WasmValType::I64) {
    // Internal function returns lo32 and stashes hi32.
    // Combine into a BigInt for JS.
    auto *lo = callResult;
    auto *hi = helpers_.emitI64HiResult();
    auto *bigint = helpers_.emitI64ToBigInt(lo, hi);
    builder_.createReturnInst(bigint);
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
  uint32_t jsParamIdx = 1; // 0 = "this", skip it
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
  } else {
    switch (funcType.results[0]) {
      case WasmValType::I32:
        // Coerce JS Number to int32 and return.
        builder_.createReturnInst(
            builder_.createAsInt32Inst(callResult));
        break;
      case WasmValType::I64: {
        // JS import returns a BigInt. Convert to split (lo, hi).
        // emitBigIntToI64 returns lo32 and stashes hi32.
        auto *lo = helpers_.emitBigIntToI64(callResult);
        builder_.createReturnInst(lo);
        break;
      }
      case WasmValType::F32:
      case WasmValType::F64:
        // Float/double: return as-is (JS Numbers are doubles).
        builder_.createReturnInst(callResult);
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
  uint32_t jsParamIdx = 1; // 0 = "this"
  for (uint32_t i = 0; i < numParams; ++i) {
    localSlotIndex_.push_back(locals_.size());
    if (funcType.params[i] == WasmValType::I64) {
      // i64 param: allocate lo and hi stack slots.
      auto *allocLo = builder_.createAllocStackInst(
          ("local_" + llvh::Twine(i) + "_lo").str(),
          Type::createAnyType());
      auto *allocHi = builder_.createAllocStackInst(
          ("local_" + llvh::Twine(i) + "_hi").str(),
          Type::createAnyType());
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
          Type::createAnyType());
      locals_.push_back(alloc);

      auto *param = currentFunc_->getJSDynamicParam(jsParamIdx);
      builder_.createStoreStackInst(
          builder_.createLoadParamInst(param), alloc);
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
          Type::createAnyType());
      auto *allocHi = builder_.createAllocStackInst(
          ("local_" + llvh::Twine(numParams + i) + "_hi").str(),
          Type::createAnyType());
      locals_.push_back(allocLo);
      locals_.push_back(allocHi);
      builder_.createStoreStackInst(builder_.getLiteralNumber(0), allocLo);
      builder_.createStoreStackInst(builder_.getLiteralNumber(0), allocHi);
    } else {
      auto *alloc = builder_.createAllocStackInst(
          ("local_" + llvh::Twine(numParams + i)).str(),
          Type::createAnyType());
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
    if (!funcType.results.empty() && !valueStack_.empty()) {
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
        helpers_.emitI64HiStash(hi);
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

  if (!funcType.results.empty()) {
    // Pop all result values from the stack in reverse order (last result
    // type is on top of stack). For multi-value returns, we discard all
    // but the first result since JS functions can only return one value.
    // Pop trailing results (index > 0) in reverse order and discard.
    for (size_t i = funcType.results.size(); i > 1; --i) {
      if (funcType.results[i - 1] == WasmValType::I64) {
        popI64();
      } else {
        pop();
      }
    }
    // Pop and return the first result.
    if (funcType.results[0] == WasmValType::I64) {
      auto [lo, hi] = popI64();
      helpers_.emitI64HiStash(hi);
      builder_.createReturnInst(lo);
    } else {
      Value *result = pop();
      builder_.createReturnInst(result);
    }
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
  Value *rhs = pop();
  Value *lhs = pop();
  auto *add = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryAddInstKind);
  push(builder_.createAsInt32Inst(add));
}

void WasmIRGen::onI32Sub() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *sub = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinarySubtractInstKind);
  push(builder_.createAsInt32Inst(sub));
}

void WasmIRGen::onI32Mul() {
  // Use Math.imul for correctness: double multiplication loses precision
  // for large int32 products (e.g., 65536 * 65536 overflows 53-bit mantissa).
  Value *rhs = pop();
  Value *lhs = pop();
  auto *imul = builder_.createCallBuiltinInst(
      BuiltinMethod::Math_imul, {lhs, rhs});
  push(imul);
}

void WasmIRGen::onI32And() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryAndInstKind));
}

void WasmIRGen::onI32Or() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onI32Xor() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryXorInstKind));
}

void WasmIRGen::onI32Shl() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryLeftShiftInstKind));
}

void WasmIRGen::onI32ShrS() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryRightShiftInstKind));
}

void WasmIRGen::onI32ShrU() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryUnsignedRightShiftInstKind));
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
  push(builder_.createBinaryOperatorInst(
      shifted,
      builder_.getLiteralNumber(24),
      ValueKind::BinaryRightShiftInstKind));
}

void WasmIRGen::onI32Extend16S() {
  if (unreachable_)
    return;
  Value *a = pop();
  // Sign-extend from 16 bits: (a << 16) >> 16
  auto *shifted = builder_.createBinaryOperatorInst(
      a, builder_.getLiteralNumber(16), ValueKind::BinaryLeftShiftInstKind);
  push(builder_.createBinaryOperatorInst(
      shifted,
      builder_.getLiteralNumber(16),
      ValueKind::BinaryRightShiftInstKind));
}

// --- i32 comparisons (D.4) ---

void WasmIRGen::onI32Eq() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *cmp = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryStrictlyEqualInstKind);
  // Convert boolean to i32 (true→1, false→0) via BitOr with 0.
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onI32Ne() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *cmp = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryStrictlyNotEqualInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onI32LtS() {
  Value *rhs = pop();
  Value *lhs = pop();
  // Signed: cast both operands to int32 before comparing.
  auto *lhsI32 = builder_.createAsInt32Inst(lhs);
  auto *rhsI32 = builder_.createAsInt32Inst(rhs);
  auto *cmp = builder_.createBinaryOperatorInst(
      lhsI32, rhsI32, ValueKind::BinaryLessThanInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onI32GtS() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *lhsI32 = builder_.createAsInt32Inst(lhs);
  auto *rhsI32 = builder_.createAsInt32Inst(rhs);
  auto *cmp = builder_.createBinaryOperatorInst(
      lhsI32, rhsI32, ValueKind::BinaryGreaterThanInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onI32LeS() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *lhsI32 = builder_.createAsInt32Inst(lhs);
  auto *rhsI32 = builder_.createAsInt32Inst(rhs);
  auto *cmp = builder_.createBinaryOperatorInst(
      lhsI32, rhsI32, ValueKind::BinaryLessThanOrEqualInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onI32GeS() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *lhsI32 = builder_.createAsInt32Inst(lhs);
  auto *rhsI32 = builder_.createAsInt32Inst(rhs);
  auto *cmp = builder_.createBinaryOperatorInst(
      lhsI32, rhsI32, ValueKind::BinaryGreaterThanOrEqualInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onI32LtU() {
  Value *rhs = pop();
  Value *lhs = pop();
  // Unsigned: cast both operands to uint32 before comparing.
  auto *lhsU32 = builder_.createAsUint32Inst(lhs);
  auto *rhsU32 = builder_.createAsUint32Inst(rhs);
  auto *cmp = builder_.createBinaryOperatorInst(
      lhsU32, rhsU32, ValueKind::BinaryLessThanInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onI32GtU() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *lhsU32 = builder_.createAsUint32Inst(lhs);
  auto *rhsU32 = builder_.createAsUint32Inst(rhs);
  auto *cmp = builder_.createBinaryOperatorInst(
      lhsU32, rhsU32, ValueKind::BinaryGreaterThanInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onI32LeU() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *lhsU32 = builder_.createAsUint32Inst(lhs);
  auto *rhsU32 = builder_.createAsUint32Inst(rhs);
  auto *cmp = builder_.createBinaryOperatorInst(
      lhsU32, rhsU32, ValueKind::BinaryLessThanOrEqualInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onI32GeU() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *lhsU32 = builder_.createAsUint32Inst(lhs);
  auto *rhsU32 = builder_.createAsUint32Inst(rhs);
  auto *cmp = builder_.createBinaryOperatorInst(
      lhsU32, rhsU32, ValueKind::BinaryGreaterThanOrEqualInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onI32Eqz() {
  Value *val = pop();
  // eqz(x) == (x === 0) → boolean → i32.
  auto *cmp = builder_.createBinaryOperatorInst(
      val,
      builder_.getLiteralNumber(0),
      ValueKind::BinaryStrictlyEqualInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
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
  Value *cond = pop();

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
  Value *cond = pop();

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
      /* newTarget */ builder_.getLiteralUndefined(),
      /* thisValue */ builder_.getLiteralUndefined(),
      args);

  // Push return values onto the stack. The JS call only returns a single
  // value (the first result), so additional results get placeholders.
  if (!funcType.results.empty()) {
    // Push the first result (available from the JS return value).
    if (funcType.results[0] == WasmValType::I64) {
      auto *hi = helpers_.emitI64HiResult();
      pushI64(call, hi);
    } else {
      push(call);
    }
    // Push undefined placeholders for additional results (multi-value).
    for (size_t i = 1; i < funcType.results.size(); ++i) {
      if (funcType.results[i] == WasmValType::I64) {
        pushI64(
            builder_.getLiteralUndefined(),
            builder_.getLiteralUndefined());
      } else {
        push(builder_.getLiteralUndefined());
      }
    }
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
  auto *sigIdxLit = builder_.getLiteralNumber(sigIndex);
  auto *closure =
      helpers_.emitCallIndirect(funcsArr, typesArr, tableIdx, sigIdxLit);

  // Call the validated closure with the popped arguments.
  auto *call = builder_.createCallInst(
      closure,
      /* newTarget */ builder_.getLiteralUndefined(),
      /* thisValue */ builder_.getLiteralUndefined(),
      args);

  // Push return values onto the stack. The JS call only returns a single
  // value (the first result), so additional results get placeholders.
  if (!funcType.results.empty()) {
    // Push the first result (available from the JS return value).
    if (funcType.results[0] == WasmValType::I64) {
      auto *hi = helpers_.emitI64HiResult();
      pushI64(call, hi);
    } else {
      push(call);
    }
    // Push undefined placeholders for additional results (multi-value).
    for (size_t i = 1; i < funcType.results.size(); ++i) {
      if (funcType.results[i] == WasmValType::I64) {
        pushI64(
            builder_.getLiteralUndefined(),
            builder_.getLiteralUndefined());
      } else {
        push(builder_.getLiteralUndefined());
      }
    }
  }
}

// --- i64 arithmetic (G.3) ---
// i64 values are represented as two i32 values on the stack [lo, hi].
// Binary operations pop two i64 pairs and push one i64 pair.
// For operations that need a native helper: the helper returns lo32 and
// stashes hi32, which is retrieved via a second call to emitI64HiResult().

void WasmIRGen::onI64Add() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  auto *lo = helpers_.emitI64Add(loA, hiA, loB, hiB);
  auto *hi = helpers_.emitI64HiResult();
  pushI64(lo, hi);
}

void WasmIRGen::onI64Sub() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  auto *lo = helpers_.emitI64Sub(loA, hiA, loB, hiB);
  auto *hi = helpers_.emitI64HiResult();
  pushI64(lo, hi);
}

void WasmIRGen::onI64Mul() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  auto *lo = helpers_.emitI64Mul(loA, hiA, loB, hiB);
  auto *hi = helpers_.emitI64HiResult();
  pushI64(lo, hi);
}

void WasmIRGen::onI64DivS() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  auto *lo = helpers_.emitI64DivS(loA, hiA, loB, hiB);
  auto *hi = helpers_.emitI64HiResult();
  pushI64(lo, hi);
}

void WasmIRGen::onI64DivU() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  auto *lo = helpers_.emitI64DivU(loA, hiA, loB, hiB);
  auto *hi = helpers_.emitI64HiResult();
  pushI64(lo, hi);
}

void WasmIRGen::onI64RemS() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  auto *lo = helpers_.emitI64RemS(loA, hiA, loB, hiB);
  auto *hi = helpers_.emitI64HiResult();
  pushI64(lo, hi);
}

void WasmIRGen::onI64RemU() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  auto *lo = helpers_.emitI64RemU(loA, hiA, loB, hiB);
  auto *hi = helpers_.emitI64HiResult();
  pushI64(lo, hi);
}

void WasmIRGen::onI64And() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  auto *lo = builder_.createBinaryOperatorInst(
      loA, loB, ValueKind::BinaryAndInstKind);
  auto *hi = builder_.createBinaryOperatorInst(
      hiA, hiB, ValueKind::BinaryAndInstKind);
  pushI64(lo, hi);
}

void WasmIRGen::onI64Or() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  auto *lo = builder_.createBinaryOperatorInst(
      loA, loB, ValueKind::BinaryOrInstKind);
  auto *hi = builder_.createBinaryOperatorInst(
      hiA, hiB, ValueKind::BinaryOrInstKind);
  pushI64(lo, hi);
}

void WasmIRGen::onI64Xor() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  auto *lo = builder_.createBinaryOperatorInst(
      loA, loB, ValueKind::BinaryXorInstKind);
  auto *hi = builder_.createBinaryOperatorInst(
      hiA, hiB, ValueKind::BinaryXorInstKind);
  pushI64(lo, hi);
}

void WasmIRGen::onI64Shl() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  auto *lo = helpers_.emitI64Shl(loA, hiA, loB, hiB);
  auto *hi = helpers_.emitI64HiResult();
  pushI64(lo, hi);
}

void WasmIRGen::onI64ShrS() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  auto *lo = helpers_.emitI64ShrS(loA, hiA, loB, hiB);
  auto *hi = helpers_.emitI64HiResult();
  pushI64(lo, hi);
}

void WasmIRGen::onI64ShrU() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  auto *lo = helpers_.emitI64ShrU(loA, hiA, loB, hiB);
  auto *hi = helpers_.emitI64HiResult();
  pushI64(lo, hi);
}

void WasmIRGen::onI64Rotl() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  auto *lo = helpers_.emitI64Rotl(loA, hiA, loB, hiB);
  auto *hi = helpers_.emitI64HiResult();
  pushI64(lo, hi);
}

void WasmIRGen::onI64Rotr() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  auto *lo = helpers_.emitI64Rotr(loA, hiA, loB, hiB);
  auto *hi = helpers_.emitI64HiResult();
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

void WasmIRGen::onI64Eqz() {
  if (unreachable_)
    return;
  auto [lo, hi] = popI64();
  push(helpers_.emitI64Eqz(lo, hi));
}

void WasmIRGen::onI64Eq() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  push(helpers_.emitI64Eq(loA, hiA, loB, hiB));
}

void WasmIRGen::onI64Ne() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  push(helpers_.emitI64Ne(loA, hiA, loB, hiB));
}

void WasmIRGen::onI64LtS() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  push(helpers_.emitI64LtS(loA, hiA, loB, hiB));
}

void WasmIRGen::onI64GtS() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  push(helpers_.emitI64GtS(loA, hiA, loB, hiB));
}

void WasmIRGen::onI64LeS() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  push(helpers_.emitI64LeS(loA, hiA, loB, hiB));
}

void WasmIRGen::onI64GeS() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  push(helpers_.emitI64GeS(loA, hiA, loB, hiB));
}

void WasmIRGen::onI64LtU() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  push(helpers_.emitI64LtU(loA, hiA, loB, hiB));
}

void WasmIRGen::onI64GtU() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  push(helpers_.emitI64GtU(loA, hiA, loB, hiB));
}

void WasmIRGen::onI64LeU() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  push(helpers_.emitI64LeU(loA, hiA, loB, hiB));
}

void WasmIRGen::onI64GeU() {
  if (unreachable_)
    return;
  auto [loB, hiB] = popI64();
  auto [loA, hiA] = popI64();
  push(helpers_.emitI64GeU(loA, hiA, loB, hiB));
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
  auto *newLo = builder_.createBinaryOperatorInst(
      shifted,
      builder_.getLiteralNumber(24),
      ValueKind::BinaryRightShiftInstKind);
  auto *newHi = builder_.createBinaryOperatorInst(
      newLo,
      builder_.getLiteralNumber(31),
      ValueKind::BinaryRightShiftInstKind);
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
  auto *newLo = builder_.createBinaryOperatorInst(
      shifted,
      builder_.getLiteralNumber(16),
      ValueKind::BinaryRightShiftInstKind);
  auto *newHi = builder_.createBinaryOperatorInst(
      newLo,
      builder_.getLiteralNumber(31),
      ValueKind::BinaryRightShiftInstKind);
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
  pushI64(lo, newHi);
}

// --- f64 arithmetic (E.1) ---
// We use BinaryOperatorInst (not FBinaryMathInst) because the F-prefixed
// instructions require number-typed inputs, but our values are loaded from
// AllocStackInst with :any type. The regular BinaryOperatorInst works
// correctly on number values and can be optimized to F-instructions later.

void WasmIRGen::onF64Add() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryAddInstKind));
}

void WasmIRGen::onF64Sub() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinarySubtractInstKind));
}

void WasmIRGen::onF64Mul() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryMultiplyInstKind));
}

void WasmIRGen::onF64Div() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryDivideInstKind));
}

void WasmIRGen::onF64Neg() {
  Value *val = pop();
  push(builder_.createUnaryOperatorInst(
      val, ValueKind::UnaryMinusInstKind));
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
  // Note: Wasm nearest is "round ties to even" (IEEE 754), while Math.round
  // rounds ties to +infinity. This is a known approximation for Phase 1.
  Value *val = pop();
  push(builder_.createCallBuiltinInst(BuiltinMethod::Math_round, {val}));
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
// Use BinaryStrictlyEqualInst/etc. (same pattern as i32 comparisons in D.4).
// For float comparisons, strict equality works correctly: NaN !== NaN,
// and the ordering comparisons follow IEEE 754 semantics (NaN comparisons
// return false, which is correct for Wasm).

void WasmIRGen::onF64Eq() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *cmp = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryStrictlyEqualInstKind);
  // Convert boolean to i32 (true→1, false→0) via BitOr with 0.
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onF64Ne() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *cmp = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryStrictlyNotEqualInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onF64Lt() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *cmp = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryLessThanInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onF64Gt() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *cmp = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryGreaterThanInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onF64Le() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *cmp = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryLessThanOrEqualInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onF64Ge() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *cmp = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryGreaterThanOrEqualInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

// --- f32 arithmetic (E.2) ---
// All f32 operations produce f32-precision results by wrapping the result
// in Math.fround. Constants are correctly rounded via float cast in
// onF32Const, so they don't need fround.

void WasmIRGen::onF32Add() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(emitFround(builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryAddInstKind)));
}

void WasmIRGen::onF32Sub() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(emitFround(builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinarySubtractInstKind)));
}

void WasmIRGen::onF32Mul() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(emitFround(builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryMultiplyInstKind)));
}

void WasmIRGen::onF32Div() {
  Value *rhs = pop();
  Value *lhs = pop();
  push(emitFround(builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryDivideInstKind)));
}

void WasmIRGen::onF32Neg() {
  Value *val = pop();
  push(emitFround(builder_.createUnaryOperatorInst(
      val, ValueKind::UnaryMinusInstKind)));
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
  // Same approximation as f64.nearest: Math.round instead of round-ties-even.
  Value *val = pop();
  push(emitFround(
      builder_.createCallBuiltinInst(BuiltinMethod::Math_round, {val})));
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
// Same pattern as f64 comparisons. f32 values are represented as doubles
// that are already f32-precise, so comparisons work correctly including
// NaN handling (IEEE 754).

void WasmIRGen::onF32Eq() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *cmp = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryStrictlyEqualInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onF32Ne() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *cmp = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryStrictlyNotEqualInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onF32Lt() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *cmp = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryLessThanInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onF32Gt() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *cmp = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryGreaterThanInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onF32Le() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *cmp = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryLessThanOrEqualInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
}

void WasmIRGen::onF32Ge() {
  Value *rhs = pop();
  Value *lhs = pop();
  auto *cmp = builder_.createBinaryOperatorInst(
      lhs, rhs, ValueKind::BinaryGreaterThanOrEqualInstKind);
  push(builder_.createBinaryOperatorInst(
      cmp, builder_.getLiteralNumber(0), ValueKind::BinaryOrInstKind));
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
  Value *lo = helpers_.emitI64TruncF64S(a);
  Value *hi = helpers_.emitI64HiResult();
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
  Value *lo = helpers_.emitI64TruncF64U(a);
  Value *hi = helpers_.emitI64HiResult();
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
  Value *lo = helpers_.emitI64TruncSatF64S(a);
  Value *hi = helpers_.emitI64HiResult();
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
  Value *lo = helpers_.emitI64TruncSatF64U(a);
  Value *hi = helpers_.emitI64HiResult();
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
  Value *lo = helpers_.emitI64ReinterpretF64(a);
  Value *hi = helpers_.emitI64HiResult();
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
  auto *tagLit = builder_.getLiteralNumber(static_cast<double>(tagIndex));
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
  auto *tagLit = builder_.getLiteralNumber(static_cast<double>(tagIndex));
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
  return builder_.createCallBuiltinInst(BuiltinMethod::Math_fround, {val});
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
  // Determine initial memory size in bytes.
  // Use the first memory (Wasm MVP has at most one).
  uint32_t initialPages = 0;
  if (!moduleInfo_.memories.empty()) {
    initialPages = moduleInfo_.memories[0].limits.initial;
  } else {
    // Check imported memories.
    for (auto &imp : moduleInfo_.imports) {
      if (imp.kind == WasmExternalKind::Memory) {
        initialPages = imp.memoryType.limits.initial;
        break;
      }
    }
  }

  auto *memSize = builder_.getLiteralNumber(
      static_cast<double>(initialPages) * 65536.0);

  // Create: var buffer = new ArrayBuffer(memSize)
  auto *abCtor = builder_.createTryLoadGlobalPropertyInst("ArrayBuffer");
  auto *buffer = emitNew(abCtor, {memSize});

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

  if (numBytes == 1)
    return b0;

  // Assemble multi-byte value: result = b0 | (b1 << 8) | (b2 << 16) | ...
  Value *result = b0;
  for (uint32_t i = 1; i < numBytes; ++i) {
    auto *addrI = builder_.createBinaryOperatorInst(
        addr,
        builder_.getLiteralNumber(static_cast<double>(i)),
        ValueKind::BinaryAddInstKind);
    auto *bi = builder_.createLoadPropertyInst(view, addrI);
    auto *shifted = builder_.createBinaryOperatorInst(
        bi,
        builder_.getLiteralNumber(static_cast<double>(i * 8)),
        ValueKind::BinaryLeftShiftInstKind);
    result = builder_.createBinaryOperatorInst(
        result, shifted, ValueKind::BinaryOrInstKind);
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
  builder_.createStorePropertyStrictInst(b0, view, addr);

  for (uint32_t i = 1; i < numBytes; ++i) {
    auto *addrI = builder_.createBinaryOperatorInst(
        addr,
        builder_.getLiteralNumber(static_cast<double>(i)),
        ValueKind::BinaryAddInstKind);
    // Shift right by i*8, then mask to get byte i.
    auto *shifted = builder_.createBinaryOperatorInst(
        value,
        builder_.getLiteralNumber(static_cast<double>(i * 8)),
        ValueKind::BinaryUnsignedRightShiftInstKind);
    auto *bi = builder_.createBinaryOperatorInst(
        shifted,
        builder_.getLiteralNumber(0xFF),
        ValueKind::BinaryAndInstKind);
    builder_.createStorePropertyStrictInst(bi, view, addrI);
  }
}

void WasmIRGen::onLoad(
    const char *opcodeName,
    uint32_t alignLog2,
    uint32_t offset) {
  if (unreachable_)
    return;

  // Pop the base address.
  Value *base = pop();

  // Compute effective address: base + offset.
  Value *addr;
  if (offset != 0) {
    addr = builder_.createBinaryOperatorInst(
        base,
        builder_.getLiteralNumber(static_cast<double>(offset)),
        ValueKind::BinaryAddInstKind);
  } else {
    addr = base;
  }

  // Determine which view to use and the element shift based on the opcode.
  llvh::StringRef op(opcodeName);

  // Check if we need the unaligned (byte-assembly) path.
  uint8_t naturalAlign = getNaturalAlignLog2(op);

  // i64 loads: handled specially (split into lo/hi).
  if (op == "i64.load") {
    if (alignLog2 < naturalAlign) {
      // Unaligned: byte-assemble lo32 and hi32 separately.
      auto *lo = emitUnalignedLoad(addr, 4);
      auto *addrHi = builder_.createBinaryOperatorInst(
          addr,
          builder_.getLiteralNumber(4),
          ValueKind::BinaryAddInstKind);
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
    // Load the hi32 word at idx+1.
    auto *idx1 = builder_.createBinaryOperatorInst(
        idx,
        builder_.getLiteralNumber(1),
        ValueKind::BinaryAddInstKind);
    auto *hi = builder_.createLoadPropertyInst(view, idx1);
    pushI64(lo, hi);
    return;
  }

  if (op == "i64.load8_s" || op == "i64.load8_u") {
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
    // Result as i32 (lower byte), zero or sign extended.
    auto *loI32 = builder_.createAsInt32Inst(lo);
    // For i64: hi = sign-extended bits or 0.
    Value *hi;
    if (isSigned) {
      // Sign-extend: hi = lo >> 31 (arithmetic shift fills with sign bit).
      hi = builder_.createBinaryOperatorInst(
          loI32,
          builder_.getLiteralNumber(31),
          ValueKind::BinaryRightShiftInstKind);
    } else {
      hi = builder_.getLiteralNumber(0);
    }
    pushI64(loI32, hi);
    return;
  }

  if (op == "i64.load16_s" || op == "i64.load16_u") {
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
        loI32 = builder_.createBinaryOperatorInst(
            shifted,
            builder_.getLiteralNumber(16),
            ValueKind::BinaryRightShiftInstKind);
      } else {
        loI32 = raw;
      }
      Value *hi;
      if (isSigned) {
        hi = builder_.createBinaryOperatorInst(
            loI32,
            builder_.getLiteralNumber(31),
            ValueKind::BinaryRightShiftInstKind);
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
    auto *loI32 = builder_.createAsInt32Inst(lo);
    Value *hi;
    if (isSigned) {
      hi = builder_.createBinaryOperatorInst(
          loI32,
          builder_.getLiteralNumber(31),
          ValueKind::BinaryRightShiftInstKind);
    } else {
      hi = builder_.getLiteralNumber(0);
    }
    pushI64(loI32, hi);
    return;
  }

  if (op == "i64.load32_s" || op == "i64.load32_u") {
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
    Value *hi;
    if (isSigned) {
      // The HEAP32 (Int32Array) already returns a signed i32.
      // Sign-extend hi from bit 31.
      auto *loI32 = builder_.createAsInt32Inst(lo);
      hi = builder_.createBinaryOperatorInst(
          loI32,
          builder_.getLiteralNumber(31),
          ValueKind::BinaryRightShiftInstKind);
    } else {
      hi = builder_.getLiteralNumber(0);
    }
    pushI64(lo, hi);
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
      push(builder_.createBinaryOperatorInst(
          shifted,
          builder_.getLiteralNumber(16),
          ValueKind::BinaryRightShiftInstKind));
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
  push(loaded);
}

void WasmIRGen::onStore(
    const char *opcodeName,
    uint32_t alignLog2,
    uint32_t offset) {
  if (unreachable_)
    return;

  llvh::StringRef op(opcodeName);
  uint8_t naturalAlign = getNaturalAlignLog2(op);

  // i64 stores: pop i64 pair (lo, hi) then base address.
  if (op == "i64.store") {
    auto [lo, hi] = popI64();
    Value *base = pop();
    Value *addr;
    if (offset != 0) {
      addr = builder_.createBinaryOperatorInst(
          base,
          builder_.getLiteralNumber(static_cast<double>(offset)),
          ValueKind::BinaryAddInstKind);
    } else {
      addr = base;
    }

    if (alignLog2 < naturalAlign) {
      // Unaligned: byte-store lo32 and hi32 separately.
      emitUnalignedStore(addr, lo, 4);
      auto *addrHi = builder_.createBinaryOperatorInst(
          addr,
          builder_.getLiteralNumber(4),
          ValueKind::BinaryAddInstKind);
      emitUnalignedStore(addrHi, hi, 4);
      return;
    }

    // Aligned path.
    auto *view = loadMemView(HEAPU32);
    auto *idx = builder_.createBinaryOperatorInst(
        addr,
        builder_.getLiteralNumber(2),
        ValueKind::BinaryUnsignedRightShiftInstKind);
    builder_.createStorePropertyStrictInst(lo, view, idx);
    auto *idx1 = builder_.createBinaryOperatorInst(
        idx,
        builder_.getLiteralNumber(1),
        ValueKind::BinaryAddInstKind);
    builder_.createStorePropertyStrictInst(hi, view, idx1);
    return;
  }

  if (op == "i64.store8") {
    auto [lo, hi] = popI64();
    (void)hi;
    Value *base = pop();
    Value *addr;
    if (offset != 0) {
      addr = builder_.createBinaryOperatorInst(
          base,
          builder_.getLiteralNumber(static_cast<double>(offset)),
          ValueKind::BinaryAddInstKind);
    } else {
      addr = base;
    }
    // Byte stores are always naturally aligned (natural align = 0).
    auto *view = loadMemView(HEAPU8);
    builder_.createStorePropertyStrictInst(lo, view, addr);
    return;
  }

  if (op == "i64.store16") {
    auto [lo, hi] = popI64();
    (void)hi;
    Value *base = pop();
    Value *addr;
    if (offset != 0) {
      addr = builder_.createBinaryOperatorInst(
          base,
          builder_.getLiteralNumber(static_cast<double>(offset)),
          ValueKind::BinaryAddInstKind);
    } else {
      addr = base;
    }

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
    builder_.createStorePropertyStrictInst(lo, view, idx);
    return;
  }

  if (op == "i64.store32") {
    auto [lo, hi] = popI64();
    (void)hi;
    Value *base = pop();
    Value *addr;
    if (offset != 0) {
      addr = builder_.createBinaryOperatorInst(
          base,
          builder_.getLiteralNumber(static_cast<double>(offset)),
          ValueKind::BinaryAddInstKind);
    } else {
      addr = base;
    }

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
    builder_.createStorePropertyStrictInst(lo, view, idx);
    return;
  }

  // Non-i64 stores: pop value, then base address.
  Value *value = pop();
  Value *base = pop();

  Value *addr;
  if (offset != 0) {
    addr = builder_.createBinaryOperatorInst(
        base,
        builder_.getLiteralNumber(static_cast<double>(offset)),
        ValueKind::BinaryAddInstKind);
  } else {
    addr = base;
  }

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

  // Unaligned path: byte-store to HEAPU8.
  if (alignLog2 < naturalAlign) {
    if (op == "f64.store") {
      // Reinterpret f64 → i64 (split lo/hi), then byte-store each half.
      auto *reinterp = helpers_.emitI64ReinterpretF64(value);
      auto *hi = helpers_.emitI64HiResult();
      emitUnalignedStore(addr, reinterp, 4);
      auto *addrHi = builder_.createBinaryOperatorInst(
          addr,
          builder_.getLiteralNumber(4),
          ValueKind::BinaryAddInstKind);
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

  // Get maximum page count from module info.
  uint32_t maxPages = 65536; // Default: 4GB (max Wasm memory).
  if (!moduleInfo_.memories.empty()) {
    if (moduleInfo_.memories[0].limits.hasMaximum) {
      maxPages = moduleInfo_.memories[0].limits.maximum;
    }
  } else {
    for (auto &imp : moduleInfo_.imports) {
      if (imp.kind == WasmExternalKind::Memory) {
        if (imp.memoryType.limits.hasMaximum) {
          maxPages = imp.memoryType.limits.maximum;
        }
        break;
      }
    }
  }

  // Call the grow builtin: wasmMemoryGrow(heapu8, delta, maxPages).
  // Returns new ArrayBuffer on success, or -1 on failure.
  auto *result = helpers_.emitMemoryGrow(
      heapu8, delta, builder_.getLiteralNumber(maxPages));

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

void WasmIRGen::createTables(Instruction *tlScope) {
  // Determine initial size for each table. Tables may be defined in the
  // table section or imported.
  uint32_t numTables = moduleInfo_.totalTableCount();

  for (uint32_t tblIdx = 0; tblIdx < numTables; ++tblIdx) {
    uint32_t initialSize = 0;
    uint32_t importedTables = moduleInfo_.importedTableCount();

    if (tblIdx < importedTables) {
      // Imported table — find the corresponding import.
      uint32_t importTableIdx = 0;
      for (const auto &imp : moduleInfo_.imports) {
        if (imp.kind != WasmExternalKind::Table)
          continue;
        if (importTableIdx == tblIdx) {
          initialSize = imp.tableType.limits.initial;
          break;
        }
        ++importTableIdx;
      }
    } else {
      initialSize = moduleInfo_.tables[tblIdx - importedTables].limits.initial;
    }

    // Create the functions array: new Array(initialSize)
    // This creates a sparse JS array with .length = initialSize.
    // Uninitialized entries read as `undefined`.
    auto *arrayCtor = builder_.createTryLoadGlobalPropertyInst("Array");
    auto *sizeVal = builder_.getLiteralNumber(static_cast<double>(initialSize));
    auto *funcsArr = emitNew(arrayCtor, {sizeVal});
    builder_.createStoreFrameInst(tlScope, funcsArr, tableFuncVars_[tblIdx]);

    // Create the type-indices array: new Array(initialSize)
    // Uninitialized entries are `undefined` (treated as -1 / no type).
    auto *typesArr = emitNew(arrayCtor, {sizeVal});
    builder_.createStoreFrameInst(tlScope, typesArr, tableTypeVars_[tblIdx]);
  }

  // Apply active element segments.
  for (const auto &seg : moduleInfo_.elements) {
    if (seg.mode != WasmElemSegment::Mode::Active)
      continue;

    // The offset for active segments. For Phase 1, only i32.const offsets
    // are supported (global.get offsets would require globals to be
    // initialized first, which is not yet implemented).
    Value *offset = nullptr;
    if (seg.offsetKind == WasmGlobal::InitKind::I32Const) {
      offset = builder_.getLiteralNumber(
          static_cast<double>(seg.offsetValue));
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

    // Store each function reference and type index into the table.
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

      // Store the closure into the functions array.
      if (funcIdx < closureVars_.size()) {
        auto *closure = builder_.createLoadFrameInst(
            tlScope, closureVars_[funcIdx]);
        builder_.createStorePropertyStrictInst(closure, funcsArr, idx);

        // Store the type index into the type-indices array.
        uint32_t typeIdx = moduleInfo_.getFunctionType(funcIdx).params.size();
        // Actually, we need the type index from the function's type, not the
        // param count. The type index is used for call_indirect matching.
        typeIdx = 0;
        if (funcIdx < moduleInfo_.importedFunctionCount()) {
          // Find the import's type index.
          uint32_t importFuncIdx = 0;
          for (const auto &imp : moduleInfo_.imports) {
            if (imp.kind != WasmExternalKind::Function)
              continue;
            if (importFuncIdx == funcIdx) {
              typeIdx = imp.typeIndex;
              break;
            }
            ++importFuncIdx;
          }
        } else {
          typeIdx = moduleInfo_
                        .functions[funcIdx -
                                   moduleInfo_.importedFunctionCount()]
                        .typeIndex;
        }
        builder_.createStorePropertyStrictInst(
            builder_.getLiteralNumber(static_cast<double>(typeIdx)),
            typesArr,
            idx);
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

void WasmIRGen::onTableGet(uint32_t tableIndex) {
  if (unreachable_)
    return;

  Value *idx = pop();
  auto *funcsArr = loadTableFuncs(tableIndex);
  auto *result = builder_.createLoadPropertyInst(funcsArr, idx);
  push(result);
}

void WasmIRGen::onTableSet(uint32_t tableIndex) {
  if (unreachable_)
    return;

  Value *val = pop();
  Value *idx = pop();
  auto *funcsArr = loadTableFuncs(tableIndex);
  builder_.createStorePropertyStrictInst(val, funcsArr, idx);
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
  // Phase 1: not fully implemented — always returns -1 (failure).
  // This is valid per the Wasm spec (grow can fail).
  pop(); // delta
  pop(); // fill value
  push(builder_.getLiteralNumber(-1));
}

// --- Bulk table operations (N.2) ---

void WasmIRGen::onTableFill(uint32_t tableIndex) {
  if (unreachable_)
    return;

  // Stack: [idx, val, count] (top = count).
  Value *count = pop();
  Value *val = pop();
  Value *idx = pop();

  auto *funcsArr = loadTableFuncs(tableIndex);
  helpers_.emitTableFill(funcsArr, idx, val, count);
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

  auto *dstFuncs = loadTableFuncs(dstTableIndex);
  auto *srcFuncs = loadTableFuncs(srcTableIndex);
  auto *dstTypes = loadTableTypes(dstTableIndex);
  auto *srcTypes = loadTableTypes(srcTableIndex);
  helpers_.emitTableCopy(
      dstFuncs, srcFuncs, dstTypes, srcTypes, dst, src, count);
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
  auto *elemSegs = builder_.createLoadFrameInst(
      parentScopeInst_, getOrCreateElemSegVar());
  auto *segIdx =
      builder_.getLiteralNumber(static_cast<double>(segmentIndex));
  helpers_.emitTableInit(
      funcsArr, typesArr, elemSegs, segIdx, dst, src, count);
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

void WasmIRGen::initializeGlobals(Instruction *tlScope) {
  uint32_t numImportedGlobals = moduleInfo_.importedGlobalCount();

  // Initialize imported globals from the imports object.
  // Imported globals are read from __wasm_imports__[module][field].value
  // (for WebAssembly.Global objects) or directly as numbers.
  // Phase 1: treat imported globals as their numeric initial value (0).
  // This will be properly implemented when M.7 (WebAssembly.Global) exists.
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

    // Phase 1: initialize imported globals to 0.
    builder_.createStoreFrameInst(
        tlScope,
        builder_.getLiteralNumber(0),
        globalVars_[slotIdx]);
    if (gType == WasmValType::I64) {
      builder_.createStoreFrameInst(
          tlScope,
          builder_.getLiteralNumber(0),
          globalVars_[slotIdx + 1]);
    }
  }

  // Initialize defined globals from their init expressions.
  for (uint32_t di = 0; di < moduleInfo_.globals.size(); ++di) {
    uint32_t globalIdx = numImportedGlobals + di;
    uint32_t slotIdx = globalSlotIndex_[globalIdx];
    const WasmGlobal &g = moduleInfo_.globals[di];

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
        // Store the closure for the referenced function.
        if (g.initValue.funcIndex < closureVars_.size()) {
          auto *closure = builder_.createLoadFrameInst(
              tlScope, closureVars_[g.initValue.funcIndex]);
          builder_.createStoreFrameInst(
              tlScope, closure, globalVars_[slotIdx]);
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
