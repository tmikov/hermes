/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#ifndef HERMES_WASMFRONTEND_WASMCOMPILE_H
#define HERMES_WASMFRONTEND_WASMCOMPILE_H

#include "hermes/WasmFrontend/WasmModuleData.h"

#include <cstddef>
#include <cstdint>
#include <memory>
#include <string>

namespace hermes {

class Module;

/// Compile a Wasm binary module to Hermes IR.
/// \param buffer The raw .wasm bytes.
/// \param size Size in bytes.
/// \param M The Hermes IR module to populate.
/// \param errorMsg [out] Error message on failure.
/// \returns true on success.
bool compileWasmModule(
    const uint8_t *buffer,
    size_t size,
    Module &M,
    std::string &errorMsg);

/// Validate a Wasm binary module without compiling it.
/// \param buffer The raw .wasm bytes.
/// \param size Size in bytes.
/// \returns true if the module is valid.
bool validateWasmBinary(const uint8_t *buffer, size_t size);

/// Compile a Wasm binary module and produce a WasmModuleData suitable for
/// storing in a JSWebAssemblyModule. This parses the module, compiles to
/// Hermes IR, generates bytecode, and populates export/import descriptors.
/// \param buffer The raw .wasm bytes.
/// \param size Size in bytes.
/// \param errorMsg [out] Error message on failure.
/// \returns a WasmModuleData on success, nullptr on failure.
std::unique_ptr<WasmModuleData> compileWasmToModuleData(
    const uint8_t *buffer,
    size_t size,
    std::string &errorMsg);

} // namespace hermes

#endif // HERMES_WASMFRONTEND_WASMCOMPILE_H
