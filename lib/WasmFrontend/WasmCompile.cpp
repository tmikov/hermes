/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/WasmFrontend/WasmCompile.h"

#include "hermes/BCGen/HBC/BCProviderFromSrc.h"
#include "hermes/BCGen/HBC/HBC.h"
#include "hermes/IR/IR.h"
#include "hermes/Optimizer/PassManager/Pipeline.h"
#include "hermes/WasmFrontend/BinaryReaderHermesIRGen.h"
#include "hermes/WasmFrontend/WasmModuleInfo.h"
#include "hermes/WasmIRGen/WasmIRGen.h"

// wabt headers use #if on macros that may not be defined, triggering -Wundef.
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wundef"
#include "wabt/binary-reader.h"
#include "wabt/binary-reader-nop.h"
#pragma GCC diagnostic pop

namespace hermes {

bool compileWasmModule(
    const uint8_t *buffer,
    size_t size,
    Module &M,
    std::string &errorMsg) {
  // Parse the Wasm binary and generate Hermes IR in a single pass.
  // The BinaryReaderHermesIRGen populates WasmModuleInfo during module-level
  // sections and dispatches function body callbacks to WasmIRGen for IR
  // generation.
  wasm::WasmModuleInfo moduleInfo;
  wasm::WasmIRGen irgen(M, moduleInfo);
  wasm::BinaryReaderHermesIRGen reader(moduleInfo);
  reader.setIRGen(&irgen);

  wabt::ReadBinaryOptions options;
  options.read_debug_names = true;
  options.features.enable_exceptions();
  wabt::Result result =
      wabt::ReadBinary(buffer, size, &reader, options);
  if (!wabt::Succeeded(result)) {
    errorMsg = "Failed to parse Wasm binary";
    return false;
  }

  return true;
}

/// Convert WasmExternalKind to the JS API string name.
static const char *externalKindName(wasm::WasmExternalKind kind) {
  switch (kind) {
    case wasm::WasmExternalKind::Function:
      return "function";
    case wasm::WasmExternalKind::Table:
      return "table";
    case wasm::WasmExternalKind::Memory:
      return "memory";
    case wasm::WasmExternalKind::Global:
      return "global";
    case wasm::WasmExternalKind::Tag:
      return "tag";
  }
  return "unknown";
}

std::unique_ptr<WasmModuleData> compileWasmToModuleData(
    const uint8_t *buffer,
    size_t size,
    std::string &errorMsg) {
  // Full compilation: parse → IR → optimize → bytecode.
  auto context = std::make_shared<Context>();
  auto M = std::make_shared<Module>(context);

  wasm::WasmModuleInfo moduleInfo;
  wasm::WasmIRGen irgen(*M, moduleInfo);
  wasm::BinaryReaderHermesIRGen reader(moduleInfo);
  reader.setIRGen(&irgen);

  wabt::ReadBinaryOptions options;
  options.read_debug_names = true;
  options.features.enable_exceptions();
  wabt::Result result = wabt::ReadBinary(buffer, size, &reader, options);
  if (!wabt::Succeeded(result)) {
    errorMsg = "invalid Wasm binary";
    return nullptr;
  }

  // Run the optimization pipeline.
  runFullOptimizationPasses(*M);

  // Generate bytecode.
  BytecodeGenerationOptions genOptions{OutputFormatKind::Execute};
  genOptions.optimizationEnabled = true;
  genOptions.staticBuiltinsEnabled = context->getStaticBuiltinOptimization();

  auto BM = hbc::generateBytecodeModule(
      M.get(), M->getTopLevelFunction(), genOptions);
  if (!BM) {
    errorMsg = "bytecode generation failed";
    return nullptr;
  }

  auto provider = hbc::BCProviderFromSrc::createFromBytecodeModule(
      std::move(BM),
      hbc::BCProviderFromSrc::CompilationData{genOptions, M, nullptr});

  auto data = std::make_unique<WasmModuleData>();
  data->bytecodeProvider = std::move(provider);

  // Populate export descriptors.
  for (const auto &exp : moduleInfo.exports) {
    data->exportDescs.push_back({exp.name, externalKindName(exp.kind)});
  }

  // Populate import descriptors.
  for (const auto &imp : moduleInfo.imports) {
    data->importDescs.push_back(
        {imp.moduleName, imp.fieldName, externalKindName(imp.kind)});
  }

  return data;
}

bool validateWasmBinary(const uint8_t *buffer, size_t size) {
  // Use a silent reader that suppresses error messages (OnError returns true
  // = "handled", preventing wabt from printing to stderr).
  class SilentReader : public wabt::BinaryReaderNop {
   public:
    bool OnError(const wabt::Error &) override {
      return true;
    }
  };

  SilentReader reader;
  wabt::ReadBinaryOptions options;
  options.features.enable_exceptions();
  wabt::Result result = wabt::ReadBinary(buffer, size, &reader, options);
  return wabt::Succeeded(result);
}

} // namespace hermes
