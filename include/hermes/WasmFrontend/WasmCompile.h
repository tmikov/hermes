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

namespace wasm {
struct WasmModuleInfo;
} // namespace wasm

/// Compile a Wasm binary module to Hermes IR. This is the single parse
/// implementation for both `hermesc --wasm` and `new WebAssembly.Module()`:
/// it validates the module before doing anything else, so there is no way
/// to reach IRGen with a module the Wasm engine would reject.
/// Validates, parses the Wasm binary, generates Hermes IR, and appends data
/// segment bytes to the Module's binary data storage.
/// \param buffer The raw .wasm bytes.
/// \param size Size in bytes.
/// \param M The Hermes IR module to populate.
/// \param moduleInfo [out] Aggregated module sections, populated by the
///     reader. A true out-param: it is cleared on entry, so passing a reused
///     object is safe and the caller needs no precondition. Left holding
///     whatever the reader managed to record when the module is rejected.
/// \param errorMsg [out] Error message on failure.
/// \returns true on success.
bool compileWasmModule(
    const uint8_t *buffer,
    size_t size,
    Module &M,
    wasm::WasmModuleInfo &moduleInfo,
    std::string &errorMsg);

/// Validate a Wasm binary module without compiling it, using wabt's semantic
/// validator (`wabt::ValidateModule`).
/// \param buffer The raw .wasm bytes.
/// \param size Size in bytes.
/// \param errorMsg [out] Diagnostic text describing why the module is
///     invalid, formatted by wabt's error formatter. Untouched on success.
/// \returns true if the module is valid.
bool validateWasmBinary(
    const uint8_t *buffer,
    size_t size,
    std::string &errorMsg);

/// Convenience overload for callers that only need the pass/fail result,
/// e.g. `WebAssembly.validate()`.
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
/// \param test262 Whether to enable strict bounds checking for spec tests.
/// \returns a WasmModuleData on success, nullptr on failure.
std::unique_ptr<WasmModuleData> compileWasmToModuleData(
    const uint8_t *buffer,
    size_t size,
    std::string &errorMsg,
    bool test262 = false);

} // namespace hermes

#endif // HERMES_WASMFRONTEND_WASMCOMPILE_H
