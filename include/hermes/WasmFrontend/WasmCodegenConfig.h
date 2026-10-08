/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#ifndef HERMES_WASMFRONTEND_WASMCODEGENCONFIG_H
#define HERMES_WASMFRONTEND_WASMCODEGENCONFIG_H

#include "hermes/BCGen/HBC/BytecodeVersion.h"
#include "hermes/WasmFrontend/WasmCodegenVersion.h"

#include <string>

namespace hermes {

/// Everything about a compiler that changes the code the Wasm frontend
/// generates, as an opaque, self-describing string.
///
/// One function serves both sides of every comparison made with it. A
/// runtime describes itself with it -- the Wasm cache keys on the result, and
/// a natively compiled unit must match it before it runs -- and the SH
/// backend stamps it into every Wasm unit's registration from the emitting
/// compiler's own constants. Two copies of this format would be two things
/// that could disagree about a unit, which nothing could then load.
///
/// Legible on purpose: it ends up in cache keys and error messages, and "why
/// did this miss?" is a question someone will ask of a hexdump. Contains
/// only [A-Za-z0-9;=-], so it needs no escaping inside a C string literal.
///
/// Anything added here that affects codegen MUST also be set by every
/// caller, or a cache will serve code built under different rules.
inline std::string wasmCodegenConfigString(bool test262) {
  std::string out("hermes-wasm;bc=");
  out += std::to_string(hbc::BYTECODE_VERSION);
  out += ";cg=";
  out += std::to_string(WASM_CODEGEN_VERSION);
  out += ";t262=";
  out += test262 ? '1' : '0';
  return out;
}

} // namespace hermes

#endif // HERMES_WASMFRONTEND_WASMCODEGENCONFIG_H
