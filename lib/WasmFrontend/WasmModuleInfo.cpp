/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/WasmFrontend/WasmModuleInfo.h"

#include <algorithm>
#include <cassert>

namespace hermes {
namespace wasm {

namespace {

/// Count the number of imports with the given kind.
uint32_t countImportsByKind(
    const std::vector<WasmImport> &imports,
    WasmExternalKind kind) {
  return static_cast<uint32_t>(std::count_if(
      imports.begin(), imports.end(), [kind](const WasmImport &imp) {
        return imp.kind == kind;
      }));
}

} // namespace

uint32_t WasmModuleInfo::totalFunctionCount() const {
  return importedFunctionCount() +
      static_cast<uint32_t>(functions.size());
}

uint32_t WasmModuleInfo::importedFunctionCount() const {
  return countImportsByKind(imports, WasmExternalKind::Function);
}

uint32_t WasmModuleInfo::getFunctionTypeIndex(uint32_t funcIndex) const {
  uint32_t numImported = importedFunctionCount();
  if (funcIndex < numImported) {
    // Find the N-th function import.
    uint32_t count = 0;
    for (const auto &imp : imports) {
      if (imp.kind == WasmExternalKind::Function) {
        if (count == funcIndex) {
          assert(imp.typeIndex < types.size() && "type index out of range");
          return imp.typeIndex;
        }
        ++count;
      }
    }
    // importedFunctionCount() counts the same imports this loop walks, so
    // reaching here means funcIndex was out of range to begin with.
    assert(false && "funcIndex out of range for imported functions");
    return 0;
  }
  uint32_t definedIdx = funcIndex - numImported;
  assert(definedIdx < functions.size() && "funcIndex out of range");
  assert(
      functions[definedIdx].typeIndex < types.size() &&
      "type index out of range");
  return functions[definedIdx].typeIndex;
}

const WasmFuncType &WasmModuleInfo::getFunctionType(
    uint32_t funcIndex) const {
  return types[getFunctionTypeIndex(funcIndex)];
}

uint32_t WasmModuleInfo::totalGlobalCount() const {
  return importedGlobalCount() + static_cast<uint32_t>(globals.size());
}

uint32_t WasmModuleInfo::importedGlobalCount() const {
  return countImportsByKind(imports, WasmExternalKind::Global);
}

uint32_t WasmModuleInfo::totalTableCount() const {
  return importedTableCount() + static_cast<uint32_t>(tables.size());
}

uint32_t WasmModuleInfo::importedTableCount() const {
  return countImportsByKind(imports, WasmExternalKind::Table);
}

uint32_t WasmModuleInfo::totalMemoryCount() const {
  return importedMemoryCount() + static_cast<uint32_t>(memories.size());
}

uint32_t WasmModuleInfo::importedMemoryCount() const {
  return countImportsByKind(imports, WasmExternalKind::Memory);
}

uint32_t WasmModuleInfo::totalTagCount() const {
  return importedTagCount() + static_cast<uint32_t>(tags.size());
}

uint32_t WasmModuleInfo::importedTagCount() const {
  return countImportsByKind(imports, WasmExternalKind::Tag);
}

const WasmFuncType &WasmModuleInfo::getTagType(uint32_t tagIndex) const {
  uint32_t numImported = importedTagCount();
  if (tagIndex < numImported) {
    // Find the N-th tag import.
    uint32_t count = 0;
    for (const auto &imp : imports) {
      if (imp.kind == WasmExternalKind::Tag) {
        if (count == tagIndex) {
          assert(imp.tagTypeIndex < types.size() && "type index out of range");
          return types[imp.tagTypeIndex];
        }
        ++count;
      }
    }
    assert(false && "tagIndex out of range for imported tags");
  }
  uint32_t definedIdx = tagIndex - numImported;
  assert(definedIdx < tags.size() && "tagIndex out of range");
  assert(
      tags[definedIdx].typeIndex < types.size() && "type index out of range");
  return types[tags[definedIdx].typeIndex];
}

} // namespace wasm
} // namespace hermes
