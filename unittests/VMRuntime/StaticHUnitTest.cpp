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

#include <chrono>
#include <string>
#include <vector>

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

// --- DISABLED_FullGCPauseVersusUnitCount ------------------------------

/// Symbol count per synthesized unit, picked to be "realistic" per the
/// design's own worry: a full GC walks each unit's whole symbols array, and
/// a module with a couple hundred top-level bindings and property names is
/// an ordinary size, not a worst case.
constexpr uint32_t kBenchSymbolCount = 200;
constexpr uint32_t kBenchReadCacheCount = 8;
constexpr uint32_t kBenchWriteCacheCount = 8;
constexpr uint32_t kBenchPrivateNameCacheCount = 4;

/// One shared ASCII pool and one shared strings table of kBenchSymbolCount
/// entries. The cost under test is the GC walking each unit's OWN symbols
/// and property-cache arrays; that walk does not care where the underlying
/// strings came from, so every synthesized unit below points its
/// ascii_pool/strings at this single shared table.
struct BenchStringTable {
  std::string asciiPool;
  std::vector<uint32_t> strings; // triples of (offset, length, hash)

  BenchStringTable() {
    for (uint32_t i = 0; i < kBenchSymbolCount; ++i) {
      std::string name = "gcBenchSym" + std::to_string(i);
      strings.push_back(static_cast<uint32_t>(asciiPool.size()));
      strings.push_back(static_cast<uint32_t>(name.size()));
      strings.push_back(0); // 0 means "compute the hash lazily"
      asciiPool += name;
    }
  }
};

/// A trivial unit body, modeled on StaticHUnitFixtures.h's testUnitMain:
/// enter, leave, return undefined. What is under test is unit
/// registration and GC scanning, not what the unit's code does.
SHLegacyValue benchUnitMain(SHRuntime *shr) {
  struct {
    SHLocals head;
  } locals;
  SHLegacyValue *frame = _sh_enter(shr, &locals.head, 1);
  locals.head.count = 0;
  _sh_leave(shr, &locals.head, frame);
  return _sh_ljs_undefined();
}

/// Backing storage for one synthesized unit: everything that must be
/// DISTINCT per unit -- the index variable _sh_unit_init writes through,
/// the symbols array sh_unit_init_symbols fills, and the property caches
/// the GC writes through -- plus the SHUnit struct itself. Only ascii_pool
/// and strings (see BenchStringTable) are shared across every unit.
struct BenchUnit {
  std::vector<SHSymbolID> symbols = std::vector<SHSymbolID>(kBenchSymbolCount);
  std::vector<SHReadPropertyCacheEntry> readCache =
      std::vector<SHReadPropertyCacheEntry>(kBenchReadCacheCount);
  std::vector<SHWritePropertyCacheEntry> writeCache =
      std::vector<SHWritePropertyCacheEntry>(kBenchWriteCacheCount);
  std::vector<SHPrivateNameCacheEntry> privateNameCache =
      std::vector<SHPrivateNameCacheEntry>(kBenchPrivateNameCacheCount);
  SHNativeFuncInfo mainInfo{};
  SHUnit *unit = static_cast<SHUnit *>(calloc(1, sizeof(SHUnit)));
};

/// SHUnitCreator is `SHUnit *(*)(void)` -- no state parameter, so it cannot
/// close over which of the N units to build. This file-scope pair stands in
/// for that: the caller points g_benchUnits at the vector and sets
/// g_benchNextUnit before each _sh_unit_init call, and the creator just
/// reads them back.
std::vector<BenchUnit> *g_benchUnits = nullptr;
uint32_t g_benchNextUnit = 0;

SHUnit *benchUnitCreator() {
  return (*g_benchUnits)[g_benchNextUnit].unit;
}

/// The design's open question: a full GC visits every initialized unit and
/// scans its whole symbol and property-cache arrays, so pause time should
/// grow with unit COUNT rather than with live data. Measured rather than
/// argued about.
///
/// Run by hand with --gtest_also_run_disabled_tests; this is a measurement,
/// not an assertion, which is also why it is DISABLED_.
TEST(StaticHUnitTest, DISABLED_FullGCPauseVersusUnitCount) {
  BenchStringTable table;

  for (uint32_t n : {1u, 100u, 1000u}) {
    // Both vectors are declared before `rt` so they outlive it: `rt`'s
    // destructor tears down every unit it still has registered, and must
    // not do so after the arrays those units point into are gone.
    std::vector<uint32_t> indices(n, 0);
    std::vector<BenchUnit> units(n);
    for (uint32_t i = 0; i < n; ++i) {
      SHUnit *unit = units[i].unit;
      unit->index = &indices[i];
      unit->num_symbols = kBenchSymbolCount;
      unit->ascii_pool = table.asciiPool.data();
      unit->strings = table.strings.data();
      unit->symbols = units[i].symbols.data();
      unit->num_read_prop_cache_entries = kBenchReadCacheCount;
      unit->read_prop_cache = units[i].readCache.data();
      unit->num_write_prop_cache_entries = kBenchWriteCacheCount;
      unit->write_prop_cache = units[i].writeCache.data();
      unit->num_private_name_cache_entries = kBenchPrivateNameCacheCount;
      unit->private_name_cache = units[i].privateNameCache.data();
      unit->unit_main = benchUnitMain;
      unit->unit_main_info = &units[i].mainInfo;
      unit->unit_name = "StaticHUnitTest.benchUnit";
    }

    auto rt = Runtime::create(kTestRTConfig);
    g_benchUnits = &units;
    for (uint32_t i = 0; i < n; ++i) {
      g_benchNextUnit = i;
      // The guarded form, not _sh_unit_init directly: it opens the
      // GCScope that _sh_ljs_create_closure's own GCScopeMarkerRAII
      // needs to nest under. Calling _sh_unit_init bare, with no
      // enclosing GCScope, segfaults in GCScope::createMarker on a null
      // "current scope" -- found by running this benchmark, not guessed.
      SHLegacyValue resOrExc;
      ASSERT_TRUE(_sh_unit_init_guarded(
          getSHRuntime(*rt), benchUnitCreator, &resOrExc));
    }
    g_benchUnits = nullptr;

    auto start = std::chrono::steady_clock::now();
    rt->collect("benchmark");
    auto ms = std::chrono::duration<double, std::milli>(
                  std::chrono::steady_clock::now() - start)
                  .count();
    llvh::errs() << n << " units: " << ms << " ms\n";
  }
}

} // namespace
