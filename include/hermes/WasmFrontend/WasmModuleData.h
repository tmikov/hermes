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

namespace hermes {

/// Data stored inside a JSWebAssemblyModule, holding module metadata
/// needed by the JS API. This is a standalone struct with no VM
/// dependencies so it can be populated by the WasmFrontend library and
/// consumed by the VM.
///
/// Subclasses may add additional data (e.g., compiled bytecode for
/// instantiation).
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
};

} // namespace hermes

#endif // HERMES_WASMFRONTEND_WASMMODULEDATA_H
