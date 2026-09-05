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
#include "wabt/binary-reader-ir.h"
#include "wabt/binary-reader-nop.h"
#include "wabt/validator.h"
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
  options.features.enable_extended_const();
  wabt::Result result =
      wabt::ReadBinary(buffer, size, &reader, options);
  if (!wabt::Succeeded(result)) {
    // The IRGen refuses some malformed modules that wabt's structural read
    // accepts (an export naming an index that does not exist, for one), and
    // it says which. Prefer that over the generic message, which would send
    // the reader looking for a truncated file.
    errorMsg = irgen.getErrorMessage().empty()
        ? "Failed to parse Wasm binary"
        : irgen.getErrorMessage().str();
    return false;
  }

  // Append all data segment bytes to the binary data storage blob on the
  // IR Module. generateBytecodeModule() will transfer this to the
  // BytecodeModule.
  for (const auto &seg : moduleInfo.dataSegments) {
    M.appendBinaryData(llvh::ArrayRef<uint8_t>(seg.data));
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
    std::string &errorMsg,
    bool test262) {
  // Validate the module first. The Wasm spec requires that
  // new WebAssembly.Module() reject semantically invalid modules with a
  // CompileError. Our compile path (ReadBinary + BinaryReaderHermesIRGen) only
  // does structural parsing, so we run WABT's full semantic validator first.
  if (!validateWasmBinary(buffer, size)) {
    errorMsg = "Wasm module validation failed";
    return nullptr;
  }

  // Full compilation: parse → IR → optimize → bytecode.
  CodeGenerationSettings codeGenOpts;
  codeGenOpts.test262 = test262;
  auto context = std::make_shared<Context>(std::move(codeGenOpts));
  auto M = std::make_shared<Module>(context);

  wasm::WasmModuleInfo moduleInfo;
  wasm::WasmIRGen irgen(*M, moduleInfo);
  wasm::BinaryReaderHermesIRGen reader(moduleInfo);
  reader.setIRGen(&irgen);

  wabt::ReadBinaryOptions options;
  options.read_debug_names = true;
  options.features.enable_exceptions();
  options.features.enable_extended_const();
  wabt::Result result = wabt::ReadBinary(buffer, size, &reader, options);
  if (!wabt::Succeeded(result)) {
    errorMsg = irgen.getErrorMessage().empty()
        ? "invalid Wasm binary"
        : irgen.getErrorMessage().str();
    return nullptr;
  }

  // Run the optimization pipeline.
  runFullOptimizationPasses(*M);

  // Append all data segment bytes to the binary data storage blob on the
  // IR Module. generateBytecodeModule() will transfer this to the
  // BytecodeModule. The segments are appended in order, matching the offsets
  // computed during IR generation in WasmIRGen::finalizeModule().
  for (const auto &seg : moduleInfo.dataSegments) {
    M->appendBinaryData(llvh::ArrayRef<uint8_t>(seg.data));
  }

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
  wabt::Module module;
  wabt::Errors errors;
  wabt::ReadBinaryOptions readOptions;
  readOptions.features.enable_exceptions();
  readOptions.features.enable_extended_const();

  wabt::Result readResult = wabt::ReadBinaryIr(
      "<validate>", buffer, size, readOptions, &errors, &module);
  if (wabt::Failed(readResult))
    return false;

  wabt::ValidateOptions validateOptions(readOptions.features);
  wabt::Result validateResult =
      wabt::ValidateModule(&module, &errors, validateOptions);
  return wabt::Succeeded(validateResult);
}

} // namespace hermes
