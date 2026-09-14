/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/VM/StaticHUtils.h"

#include "StaticHUnitFixtures.h"
#include "VMRuntimeTestHelpers.h"
#include "gtest/gtest.h"

using namespace hermes::vm;

namespace {

TEST(StaticHUnitTest, StartsEmpty) {
  auto rt = Runtime::create(kTestRTConfig);
  EXPECT_EQ(nullptr, rt->units);
  EXPECT_EQ(0u, rt->units_size);
}

TEST(StaticHUnitTest, GrowsAndNullInitializes) {
  auto rt = Runtime::create(kTestRTConfig);
  ASSERT_TRUE(shUnitEnsureCapacity(*rt, 900));
  EXPECT_GT(rt->units_size, 900u);
  // Every new slot must read as "not registered in this runtime"; realloc
  // does not do this and the lookup depends on it.
  for (uint32_t i = 0; i <= 900; ++i)
    EXPECT_EQ(nullptr, rt->units[i]) << "slot " << i;
}

TEST(StaticHUnitTest, PreservesExistingEntriesAcrossGrowth) {
  auto rt = Runtime::create(kTestRTConfig);
  ASSERT_TRUE(shUnitEnsureCapacity(*rt, 1));
  SHUnit *marker = reinterpret_cast<SHUnit *>(0x1234);
  rt->units[1] = marker;
  ASSERT_TRUE(shUnitEnsureCapacity(*rt, 5000));
  EXPECT_EQ(marker, rt->units[1]);
  EXPECT_EQ(nullptr, rt->units[5000]);
  rt->units[1] = nullptr; // not a real unit; do not let teardown see it
}

TEST(StaticHUnitTest, AlreadyLargeEnoughIsANoOp) {
  auto rt = Runtime::create(kTestRTConfig);
  ASSERT_TRUE(shUnitEnsureCapacity(*rt, 100));
  uint32_t size = rt->units_size;
  SHUnit **buf = rt->units;
  ASSERT_TRUE(shUnitEnsureCapacity(*rt, 100));
  EXPECT_EQ(size, rt->units_size);
  EXPECT_EQ(buf, rt->units);
}

/// The second-runtime hazard. Unit indices are process-wide (a
/// function-local static in _sh_unit_init), the arrays are per-runtime, so a
/// unit assigned a high index by one runtime and then initialized in another
/// reads past the end of the second runtime's array unless capacity is
/// ensured unconditionally. Registering the HIGH index first in B is the
/// point: ascending registration passes even with the check in the wrong
/// place.
TEST(StaticHUnitTest, CapacityIsPerRuntimeAndHighIndexFirstWorks) {
  auto a = Runtime::create(kTestRTConfig);
  auto b = Runtime::create(kTestRTConfig);
  ASSERT_TRUE(shUnitEnsureCapacity(*a, 900));
  EXPECT_EQ(0u, b->units_size);
  ASSERT_TRUE(shUnitEnsureCapacity(*b, 900));
  EXPECT_GT(b->units_size, 900u);
  EXPECT_EQ(nullptr, b->units[900]);
}

TEST(StaticHUnitTest, MallocSizeCountsTheBackingStore) {
  auto rt = Runtime::create(kTestRTConfig);
  size_t before = rt->mallocSize();
  ASSERT_TRUE(shUnitEnsureCapacity(*rt, 4095));
  size_t after = rt->mallocSize();
  EXPECT_GE(after - before, 4096u * sizeof(SHUnit *));
}

/// The second-runtime hazard, through the real entry point. Runtime A
/// assigns the index; runtime B then sees an already-assigned index and
/// must still grow its own array before the lookup. With the capacity call
/// left inside `if (!*unit->index)`, this reads out of bounds.
TEST(StaticHUnitTest, InitInASecondRuntimeGrowsThatRuntimesArray) {
  auto a = Runtime::create(kTestRTConfig);
  SHLegacyValue v;
  ASSERT_TRUE(_sh_unit_init_guarded(getSHRuntime(*a), createTestUnit, &v));
  ASSERT_NE(0u, g_testUnitIndex);

  auto b = Runtime::create(kTestRTConfig);
  EXPECT_EQ(0u, b->units_size);
  ASSERT_TRUE(_sh_unit_init_guarded(getSHRuntime(*b), createTestUnit, &v));
  EXPECT_GT(b->units_size, g_testUnitIndex);
  EXPECT_NE(nullptr, b->units[g_testUnitIndex]);
}

TEST(StaticHUnitTest, GuardedInitReportsAThrowingUnit) {
  auto rt = Runtime::create(kTestRTConfig);
  SHLegacyValue resOrExc = _sh_ljs_undefined();
  EXPECT_FALSE(
      _sh_unit_init_guarded(getSHRuntime(*rt), createThrowingUnit, &resOrExc));
  // The thrown value comes back in resOrExc, already extracted and cleared
  // from the runtime by _sh_catch -- which is why the NAPI wrapper must
  // NOT ask the runtime for it again.
  EXPECT_EQ(42, _sh_ljs_get_double(resOrExc));
}

} // namespace
