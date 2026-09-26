/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#ifndef HERMES_WASMFRONTEND_WASMMODULEDATA_H
#define HERMES_WASMFRONTEND_WASMMODULEDATA_H

#include <memory>
#include <string>
#include <vector>

/// Mirrored rather than included: this struct is shared with the Wasm
/// frontend and must not depend on VM headers. SHUnit is declared at global
/// scope in static_h.h, so this declaration must be too -- declaring it
/// inside namespace hermes would name a different type.
struct SHUnit;

namespace hermes {

namespace hbc {
class BCProviderBase;
} // namespace hbc

/// Creator for a natively compiled SH unit; the same type as SHUnitCreator
/// in static_h.h, so no cast is needed to convert between them.
using SHUnitCreatorFn = ::SHUnit *(*)();

/// Data stored inside a JSWebAssemblyModule, holding module metadata
/// needed by the JS API. This is a standalone struct with no VM
/// dependencies so it can be populated by the WasmFrontend library and
/// consumed by the VM.
struct WasmModuleData {
  /// Export descriptor for WebAssembly.Module.exports().
  struct ExportDesc {
    std::string name;
    /// One of "function", "table", "memory", "global", "tag".
    std::string kind;
  };

  /// Import descriptor for WebAssembly.Module.imports().
  struct ImportDesc {
    std::string module;
    std::string name;
    /// One of "function", "table", "memory", "global", "tag".
    std::string kind;
  };

  virtual ~WasmModuleData() = default;

  /// Export descriptors populated during compilation.
  std::vector<ExportDesc> exportDescs;
  /// Import descriptors populated during compilation.
  std::vector<ImportDesc> importDescs;
  /// Compiled bytecode provider, set during full compilation.
  /// Null when only metadata was parsed (e.g., Module.exports/imports).
  std::shared_ptr<hbc::BCProviderBase> bytecodeProvider;
  /// Creator for a natively compiled unit, set instead of bytecodeProvider
  /// when the module came from a linked SH unit. Exactly one of the two is
  /// non-null in a fully compiled module.
  SHUnitCreatorFn unitCreator = nullptr;
};

} // namespace hermes

#endif // HERMES_WASMFRONTEND_WASMMODULEDATA_H
