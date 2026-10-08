/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/VM/JSTypedArray.h"
#include "hermes/VM/Runtime.h"
#include "hermes/VM/StaticHUtils.h"
#include "hermes/VM/static_h.h"

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <mutex>

using namespace hermes;
using namespace hermes::vm;

extern "C" void _sh_wasm_data_segment_init(
    SHRuntime *shr,
    SHUnit *unit,
    SHLegacyValue heapu8,
    uint32_t blobOffset,
    uint32_t length,
    uint32_t dest) {
  Runtime &runtime = getRuntime(shr);

  // Validate the view before the zero-length short-circuit, so that a bad
  // view is rejected regardless of length -- matching wasmTypedArrayArg,
  // which the builtin calls before it checks length.
  auto *arr = dyn_vmcast<JSTypedArrayBase>(HermesValue::fromRaw(heapu8.raw));
  if (LLVM_UNLIKELY(!arr || !arr->attached(runtime))) {
    (void)runtime.raiseTypeError("Wasm memory view is not a typed array");
    _sh_throw_current(shr);
  }

  if (length == 0)
    return;

  if (LLVM_UNLIKELY(
          (uint64_t)blobOffset + length > unit->binary_data_size)) {
    (void)runtime.raiseError(
        "wasmDataSegmentInit: out of bounds binary data access");
    _sh_throw_current(shr);
  }

  if (LLVM_UNLIKELY((uint64_t)dest + length > arr->getByteLength())) {
    (void)runtime.raiseError(
        "wasmDataSegmentInit: out of bounds memory access");
    _sh_throw_current(shr);
  }

  // Nothing between here and the memcpy allocates, so the raw data pointer
  // cannot be invalidated by a GC.
  uint8_t *memData = arr->data(runtime);
  std::memcpy(memData + dest, unit->binary_data + blobOffset, length);
}

namespace {
/// Head of the registry. In .bss, so NULL before any static constructor runs.
SHWasmUnitReg *s_wasmUnits = nullptr;

/// Guards insertion AND lookup: a dlopen on another thread can push while a
/// lookup walks the list. Function-local so it is initialized on first use,
/// which is valid from a static constructor.
std::mutex &wasmUnitsMutex() {
  static std::mutex m;
  return m;
}

/// NULL equals only NULL; otherwise by contents.
bool sameCodegenConfig(const char *a, const char *b) {
  if (!a || !b)
    return a == b;
  return std::strcmp(a, b) == 0;
}
} // namespace

extern "C" void _sh_wasm_register_unit(SHWasmUnitReg *reg) {
  std::lock_guard<std::mutex> lock(wasmUnitsMutex());
  for (SHWasmUnitReg *p = s_wasmUnits; p; p = p->next) {
    if (std::strcmp(p->name, reg->name) == 0) {
      fprintf(
          stderr,
          "SH: duplicate Wasm unit registration for \"%s\"\n",
          reg->name);
      abort();
    }
    // One creator, one configuration: the native lookup finds a unit's
    // registration by its creator, and two answers would make the check it
    // performs depend on list order.
    if (p->creator == reg->creator &&
        !sameCodegenConfig(p->codegen_config, reg->codegen_config)) {
      fprintf(
          stderr,
          "SH: conflicting Wasm unit registrations \"%s\" and \"%s\": one "
          "creator, two codegen configurations\n",
          p->name,
          reg->name);
      abort();
    }
  }
  reg->next = s_wasmUnits;
  s_wasmUnits = reg;
}

extern "C" SHUnitCreator _sh_wasm_find_unit(const char *name) {
  // The lock is released before returning, so the caller runs the unit --
  // which re-enters the VM -- outside it.
  std::lock_guard<std::mutex> lock(wasmUnitsMutex());
  for (SHWasmUnitReg *p = s_wasmUnits; p; p = p->next) {
    if (std::strcmp(p->name, name) == 0)
      return p->creator;
  }
  return nullptr;
}

extern "C" const SHWasmUnitReg *_sh_wasm_find_unit_reg(SHUnitCreator creator) {
  // Same lock discipline as _sh_wasm_find_unit: released before the caller
  // runs anything.
  std::lock_guard<std::mutex> lock(wasmUnitsMutex());
  for (SHWasmUnitReg *p = s_wasmUnits; p; p = p->next) {
    if (p->creator == creator)
      return p;
  }
  return nullptr;
}
