/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//===----------------------------------------------------------------------===//
/// \file
/// Dummy implementation of the Wasm frontend entry points reachable from the
/// VM, to be used by a lean build of the VM, which does not contain the Wasm
/// compiler. Reports errors and returns no-op results.
///
/// This is the counterpart of WasmCompile.cpp, in the same spirit as
/// HBCStub.cpp is the counterpart of the real HBC backend: leanness is
/// expressed by selecting a different translation unit in CMake, never by a
/// compile-time define and never by weak symbols.
///
/// Weak symbols would not work here. Both this file's targets and
/// WebAssembly.cpp end up in the same static archive, and a static link only
/// extracts an archive member in order to resolve an *undefined* symbol. A
/// weak definition living next to the caller resolves the reference on the
/// spot, so the archive member holding the strong definition is never
/// extracted and never gets the chance to override it. Keeping exactly one
/// definition per archive is what forces the linker to pull in the real
/// implementation.
//===----------------------------------------------------------------------===//

#include "hermes/WasmFrontend/WasmCompile.h"

namespace hermes {

bool validateWasmBinary(const uint8_t *buffer, size_t size) {
  return false;
}

std::unique_ptr<WasmModuleData> compileWasmToModuleData(
    const uint8_t *buffer,
    size_t size,
    std::string &errorMsg,
    bool test262) {
  errorMsg = "WebAssembly support not compiled";
  return nullptr;
}

} // namespace hermes
