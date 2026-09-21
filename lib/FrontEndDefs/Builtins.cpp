/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/FrontEndDefs/Builtins.h"

#include <cassert>

namespace hermes {

static const char *builtinName[] = {
#define NORMAL_OBJECT(object)
#define NORMAL_METHOD(object, name) BUILTIN_METHOD(object, name)
#define BUILTIN_METHOD(object, name) #object "." #name,
#define PRIVATE_BUILTIN(name) BUILTIN_METHOD(HermesBuiltin, name)
#define JS_BUILTIN(name) BUILTIN_METHOD(HermesBuiltin, name)
#ifndef HERMES_ENABLE_WASM
// Without Wasm these 71 ids can still appear in bytecode -- Builtins.def
// numbering does not change with the build flag -- but nothing in this build
// can run them, so their only remaining use is being printed. One shared name
// costs one string instead of 71; see the note in getBuiltinMethodName for
// what that gives up.
#define WASM_BUILTIN(name) "HermesBuiltin.<wasm, not built>",
#endif
#include "hermes/FrontEndDefs/Builtins.def"
};

/// \return a printable name for \p method.
///
/// In a build without Wasm every wasm builtin id shares one name. Such a build
/// can still be handed bytecode that a Wasm-enabled compiler produced -- the
/// ids are stable across the flag on purpose -- so a disassembly of that
/// bytecode names those 71 opcodes only as a group, and since the
/// disassembler prints the name rather than the operand, it no longer
/// distinguishes them at all. That is a debugging nicety, not correctness:
/// the id is still exact in the bytecode, and nothing in this build can
/// execute these anyway -- every one of them resolves to a fatal stub.
const char *getBuiltinMethodName(int method) {
  assert(
      method >= 0 && method < BuiltinMethod::_count &&
      "invalid builtin method index");
  return builtinName[method];
}

} // namespace hermes
