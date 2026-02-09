/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/WasmFrontend/WasmCompile.h"

#include "hermes/WasmFrontend/BinaryReaderHermesIRGen.h"
#include "hermes/WasmFrontend/WasmModuleInfo.h"
#include "hermes/WasmIRGen/WasmIRGen.h"

#include "wabt/binary-reader.h"

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
  wabt::Result result =
      wabt::ReadBinary(buffer, size, &reader, options);
  if (!wabt::Succeeded(result)) {
    errorMsg = "Failed to parse Wasm binary";
    return false;
  }

  return true;
}

} // namespace hermes
