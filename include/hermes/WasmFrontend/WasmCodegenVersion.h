/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#ifndef HERMES_WASMFRONTEND_WASMCODEGENVERSION_H
#define HERMES_WASMFRONTEND_WASMCODEGENVERSION_H

#include <cstdint>

/*
 * This file should *only* contain the version number constant definition,
 * to enable tooling to determine whether the version has been updated.
 */

namespace hermes {

/// Identifies the code this Wasm frontend generates, for embedders that
/// cache a compile and must not serve the result to a later, different
/// compiler. It travels in the codegen config handed to the cache hooks.
///
/// BUMP THIS whenever a change to the Wasm frontend alters the bytecode
/// produced from identical input -- a lowering fix, a new instruction
/// encoding, a change of semantics. BYTECODE_VERSION does not cover it:
/// that tracks the bytecode FORMAT, so a codegen fix emitting
/// different-but-same-format bytecode leaves it untouched, and a cache
/// keyed only on that would serve the old, wrong bytecode.
///
/// Updated: Sep 7, 2026
const static uint32_t WASM_CODEGEN_VERSION = 1;

} // namespace hermes

#endif // HERMES_WASMFRONTEND_WASMCODEGENVERSION_H
