/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#ifndef HERMES_UNITTESTS_VMRUNTIME_STATICHUNITFIXTURES_H
#define HERMES_UNITTESTS_VMRUNTIME_STATICHUNITFIXTURES_H

#include "hermes/VM/StaticHUtils.h"

// Two hand-built SH compilation units, shared by StaticHUnitTest.cpp
// (this directory) and unittests/napi/NapiShUnitTest.cpp so the
// construction exists in exactly one place. Lives under unittests/VMRuntime
// rather than unittests/napi because the contents are pure VM (SHLegacyValue,
// _sh_enter/_sh_leave, SHNativeFuncInfo) with nothing NAPI-specific in them;
// NAPI is built on the VM runtime, not the reverse, so the dependency points
// the right way with NapiShUnitTest.cpp reaching down to this header rather
// than a VM-layer test reaching up into a NAPI-layer test directory.
//
// Everything here has internal linkage on purpose: the two test binaries
// never link against each other, unit indices are process-wide, and each TU
// that includes this header gets its own independent copy of the statics
// below -- which is exactly what a standalone GTest binary needs.
//
// Do not include this from two translation units linked into the SAME test
// binary. Each inclusion gets its own copy of the index variable, and unit
// identity in the VM is the `unit->index` POINTER -- so the second includer
// would silently get a second unit that also believes it is "the test
// unit", rather than sharing the first one.

namespace {

using namespace hermes::vm;

/// A minimal unit. unit_main mirrors what SH.cpp emits for an empty global:
/// enter, leave, return undefined. Doing less corrupts the frame that
/// sh_unit_run set up.
uint32_t g_testUnitIndex = 0;

SHNativeFuncInfo g_testUnitMainInfo = {};

SHLegacyValue testUnitMain(SHRuntime *shr) {
  struct {
    SHLocals head;
  } locals;
  SHLegacyValue *frame = _sh_enter(shr, &locals.head, 1);
  locals.head.count = 0;
  _sh_leave(shr, &locals.head, frame);
  return _sh_ljs_undefined();
}

/// Model this initializer on the one SH.cpp emits (search SH.cpp for
/// "CREATE_THIS_UNIT"); take SHNativeFuncInfo's field values from
/// static_h.h rather than guessing them. Everything not needed by a unit
/// that defines no strings and no properties stays zero.
SHUnit *createTestUnit() {
  auto *unit = static_cast<SHUnit *>(calloc(1, sizeof(SHUnit)));
  unit->index = &g_testUnitIndex;
  unit->unit_main = testUnitMain;
  unit->unit_main_info = &g_testUnitMainInfo;
  unit->unit_name = "StaticHUnitTest.testUnit";
  return unit;
}

uint32_t g_throwingUnitIndex = 0;

SHNativeFuncInfo g_throwingUnitMainInfo = {};

/// A unit whose top-level code throws, which a CommonJS-wrapped module
/// cannot do -- its global only creates a closure.
SHLegacyValue throwingUnitMain(SHRuntime *shr) {
  struct {
    SHLocals head;
  } locals;
  SHLegacyValue *frame = _sh_enter(shr, &locals.head, 1);
  locals.head.count = 0;
  _sh_throw(shr, _sh_ljs_double(42));
  // _sh_throw does not return; the leave is here for shape only.
  _sh_leave(shr, &locals.head, frame);
  return _sh_ljs_undefined();
}

SHUnit *createThrowingUnit() {
  auto *unit = static_cast<SHUnit *>(calloc(1, sizeof(SHUnit)));
  unit->index = &g_throwingUnitIndex;
  unit->unit_main = throwingUnitMain;
  unit->unit_main_info = &g_throwingUnitMainInfo;
  unit->unit_name = "StaticHUnitTest.throwingUnit";
  return unit;
}

} // namespace

#endif // HERMES_UNITTESTS_VMRUNTIME_STATICHUNITFIXTURES_H
