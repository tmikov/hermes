/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#ifndef HERMES_VM_STATICH_UTILS_H
#define HERMES_VM_STATICH_UTILS_H

#include "hermes/VM/Runtime.h"
#include "hermes/VM/static_h.h"

namespace hermes::vm {

struct AddPropertyCacheEntry;

inline Runtime &getRuntime(SHRuntime *shr) {
  return *static_cast<Runtime *>(shr);
}
inline SHRuntime *getSHRuntime(Runtime &runtime) {
  return static_cast<SHRuntime *>(&runtime);
}

/// Free the \p unit, and all associated data.
void sh_unit_done(Runtime &runtime, SHUnit *unit);

/// Ensure \p runtime's unit array can be indexed by \p index, growing it and
/// null-initializing the new slots.
///
/// Called unconditionally before every lookup and store, NOT only when an
/// index is first assigned. Unit indices are process-wide and the arrays are
/// not, so a unit that was assigned index 900 by one runtime and is then
/// initialized in another would otherwise index past the second runtime's
/// array.
///
/// \return false on integer overflow or allocation failure, with the
/// existing array untouched.
bool shUnitEnsureCapacity(Runtime &runtime, uint32_t index);

/// Calculate the size of allocated memory not tracked by GC.
size_t sh_unit_additional_memory_size(const SHUnit *unit);

/// Mark the non-weak roots owned by this unit.
void sh_unit_mark_roots(
    SHUnit *unit,
    RootAcceptorWithNames &acceptor,
    bool markLongLived);

/// Mark the short lived weak roots owned by this unit.
void sh_unit_mark_weak_roots(
    SHUnit *unit,
    WeakRootAcceptor &acceptor,
    bool markLongLived);

} // namespace hermes::vm

#endif // HERMES_VM_STATICH_UTILS_H
