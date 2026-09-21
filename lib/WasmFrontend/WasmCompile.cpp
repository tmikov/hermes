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
#include "wabt/error-formatter.h"
#include "wabt/validator.h"
#pragma GCC diagnostic pop

namespace hermes {

namespace {

/// The single definition of the wabt feature set the Wasm frontend enables.
/// Used both to configure the structural reader and to configure
/// `validateWasmBinary`'s semantic validator, so the two can never drift
/// apart and disagree about what is a legal module.
wabt::Features wasmFeatures() {
  wabt::Features features;
  features.enable_exceptions();
  features.enable_extended_const();
  return features;
}

} // namespace

bool compileWasmModule(
    const uint8_t *buffer,
    size_t size,
    Module &M,
    wasm::WasmModuleInfo &moduleInfo,
    std::string &errorMsg) {
  // Start from empty. The reader APPENDS to every section vector as the
  // callbacks fire and never clears them, so a caller that reused a
  // WasmModuleInfo would silently compile this module with the previous
  // one's globals, exports and data segments still in place -- and the
  // export/import descriptors built from it afterwards would describe both.
  // Owning that here makes the parameter a true out-param rather than a
  // precondition every caller has to know about.
  moduleInfo = wasm::WasmModuleInfo{};

  // Reject a semantically invalid module before doing anything else, so
  // there is no way to reach IRGen -- from `hermesc --wasm` or from
  // `new WebAssembly.Module()` -- with a module the Wasm engine itself
  // would refuse. See H19.
  if (!validateWasmBinary(buffer, size, errorMsg)) {
    return false;
  }

  // Parse the Wasm binary and generate Hermes IR in a single pass.
  // The BinaryReaderHermesIRGen populates WasmModuleInfo during module-level
  // sections and dispatches function body callbacks to WasmIRGen for IR
  // generation.
  wasm::WasmIRGen irgen(M, moduleInfo);
  wasm::BinaryReaderHermesIRGen reader(moduleInfo);
  reader.setIRGen(&irgen);

  wabt::ReadBinaryOptions options;
  options.read_debug_names = true;
  options.features = wasmFeatures();
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
  // Full compilation: validate → parse → IR → optimize → bytecode.
  // compileWasmModule() does the validate + parse + IR part; it is the same
  // implementation `hermesc --wasm` uses, so both entry points agree on what
  // counts as a valid module.
  CodeGenerationSettings codeGenOpts;
  codeGenOpts.test262 = test262;
  auto context = std::make_shared<Context>(std::move(codeGenOpts));
  auto M = std::make_shared<Module>(context);

  wasm::WasmModuleInfo moduleInfo;
  if (!compileWasmModule(buffer, size, *M, moduleInfo, errorMsg)) {
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

bool validateWasmBinary(
    const uint8_t *buffer,
    size_t size,
    std::string &errorMsg) {
  wabt::Module module;
  wabt::Errors errors;
  wabt::ReadBinaryOptions readOptions;
  readOptions.features = wasmFeatures();

  wabt::Result result = wabt::ReadBinaryIr(
      "<validate>", buffer, size, readOptions, &errors, &module);
  if (wabt::Succeeded(result)) {
    wabt::ValidateOptions validateOptions(readOptions.features);
    result = wabt::ValidateModule(&module, &errors, validateOptions);
  }
  if (wabt::Failed(result)) {
    errorMsg =
        wabt::FormatErrorsToString(errors, wabt::Location::Type::Binary);
    return false;
  }
  return true;
}

bool validateWasmBinary(const uint8_t *buffer, size_t size) {
  std::string errorMsg;
  return validateWasmBinary(buffer, size, errorMsg);
}

} // namespace hermes
