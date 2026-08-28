/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#ifndef HERMES_WASMFRONTEND_WASMMODULEINFO_H
#define HERMES_WASMFRONTEND_WASMMODULEINFO_H

#include "hermes/WasmFrontend/WasmTypes.h"

#include <optional>

namespace hermes {
namespace wasm {

/// Aggregates all sections of a parsed Wasm module into a single structure.
/// Populated by the binary reader during parsing.
struct WasmModuleInfo {
  /// Type section: function signatures.
  std::vector<WasmFuncType> types;
  /// Import section.
  std::vector<WasmImport> imports;
  /// Function section: defined (non-imported) functions.
  std::vector<WasmFunction> functions;
  /// Table section: defined (non-imported) tables.
  std::vector<WasmTableType> tables;
  /// Memory section: defined (non-imported) memories.
  std::vector<WasmMemoryType> memories;
  /// Global section: defined (non-imported) globals.
  std::vector<WasmGlobal> globals;
  /// Tag section: defined (non-imported) exception tags.
  std::vector<WasmTag> tags;
  /// Export section.
  std::vector<WasmExport> exports;
  /// Start function index (if present).
  std::optional<uint32_t> startFunction;
  /// Element section: table initializers.
  std::vector<WasmElemSegment> elements;
  /// Data section: memory initializers.
  std::vector<WasmDataSegment> dataSegments;
  /// Name custom section (debug names).
  WasmNameSection names;

  /// \return total number of functions (imported + defined).
  uint32_t totalFunctionCount() const;
  /// \return number of imported functions.
  uint32_t importedFunctionCount() const;
  /// \return the type of the function at the given index (handles imports).
  const WasmFuncType &getFunctionType(uint32_t funcIndex) const;
  /// \return the index into \c types of the function at the given index
  ///   (handles imports). Module-local: it names a slot in THIS module's type
  ///   section, so it must not be compared against another module's.
  uint32_t getFunctionTypeIndex(uint32_t funcIndex) const;

  /// \return total number of globals (imported + defined).
  uint32_t totalGlobalCount() const;
  /// \return number of imported globals.
  uint32_t importedGlobalCount() const;

  /// \return total number of tables (imported + defined).
  uint32_t totalTableCount() const;
  /// \return number of imported tables.
  uint32_t importedTableCount() const;

  /// \return total number of memories (imported + defined).
  uint32_t totalMemoryCount() const;
  /// \return number of imported memories.
  uint32_t importedMemoryCount() const;

  /// \return total number of tags (imported + defined).
  uint32_t totalTagCount() const;
  /// \return number of imported tags.
  uint32_t importedTagCount() const;
  /// \return the function type (signature) of the tag at the given index
  ///   (handles imported + defined tags).
  const WasmFuncType &getTagType(uint32_t tagIndex) const;
};

} // namespace wasm
} // namespace hermes

#endif // HERMES_WASMFRONTEND_WASMMODULEINFO_H
