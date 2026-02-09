/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/WasmFrontend/WasmCompile.h"

#include "hermes/WasmFrontend/BinaryReaderHermesIRGen.h"
#include "hermes/WasmFrontend/WasmModuleInfo.h"

#include "wabt/binary-reader.h"

#include "llvh/Support/raw_ostream.h"

namespace hermes {

bool compileWasmModule(
    const uint8_t *buffer,
    size_t size,
    Module &M,
    std::string &errorMsg) {
  // Parse the Wasm binary into our module info structure.
  wasm::WasmModuleInfo moduleInfo;
  wasm::BinaryReaderHermesIRGen reader(moduleInfo);
  wabt::ReadBinaryOptions options;
  options.read_debug_names = true;
  wabt::Result result =
      wabt::ReadBinary(buffer, size, &reader, options);
  if (!wabt::Succeeded(result)) {
    errorMsg = "Failed to parse Wasm binary";
    return false;
  }

  // Print a summary of the parsed module.
  llvh::outs() << "Wasm module parsed successfully.\n";
  if (!moduleInfo.names.moduleName.empty()) {
    llvh::outs() << "  Module name: " << moduleInfo.names.moduleName
                 << "\n";
  }
  llvh::outs() << "  Types: " << moduleInfo.types.size() << "\n";
  llvh::outs() << "  Imports: " << moduleInfo.imports.size() << "\n";
  llvh::outs() << "  Functions: " << moduleInfo.totalFunctionCount()
               << " (" << moduleInfo.importedFunctionCount() << " imported, "
               << moduleInfo.functions.size() << " defined)\n";
  llvh::outs() << "  Tables: " << moduleInfo.totalTableCount() << "\n";
  llvh::outs() << "  Memories: " << moduleInfo.totalMemoryCount() << "\n";
  llvh::outs() << "  Globals: " << moduleInfo.totalGlobalCount()
               << " (" << moduleInfo.importedGlobalCount() << " imported, "
               << moduleInfo.globals.size() << " defined)\n";
  llvh::outs() << "  Exports: " << moduleInfo.exports.size() << "\n";
  if (moduleInfo.startFunction.has_value()) {
    llvh::outs() << "  Start function: " << *moduleInfo.startFunction
                 << "\n";
  }
  llvh::outs() << "  Element segments: " << moduleInfo.elements.size()
               << "\n";
  llvh::outs() << "  Data segments: " << moduleInfo.dataSegments.size()
               << "\n";

  // List exports.
  for (const auto &exp : moduleInfo.exports) {
    const char *kindStr = "unknown";
    switch (exp.kind) {
      case wasm::WasmExternalKind::Function:
        kindStr = "func";
        break;
      case wasm::WasmExternalKind::Table:
        kindStr = "table";
        break;
      case wasm::WasmExternalKind::Memory:
        kindStr = "memory";
        break;
      case wasm::WasmExternalKind::Global:
        kindStr = "global";
        break;
    }
    llvh::outs() << "  Export: " << exp.name << " (" << kindStr << " "
                 << exp.index << ")\n";
  }

  // List function names from the name section.
  for (uint32_t i = 0; i < moduleInfo.names.functionNames.size(); ++i) {
    if (!moduleInfo.names.functionNames[i].empty()) {
      llvh::outs() << "  Function " << i << " name: "
                   << moduleInfo.names.functionNames[i] << "\n";
    }
  }

  // TODO: Drive WasmIRGen and BCGen here in future steps.
  return true;
}

} // namespace hermes
