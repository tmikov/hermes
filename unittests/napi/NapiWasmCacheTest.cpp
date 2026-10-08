/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "NapiTestFixture.h"
#include "hermes/VM/static_h.h"
#include "hermes_napi.h"

#include <cstddef>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>

namespace {

using hermes::napi::NapiTestFixture;

/// A recording cache. Serves whatever it was last told to store.
struct FakeCache {
  int lookups = 0;
  int stores = 0;
  int discards = 0;
  int outstandingTokens = 0;
  std::string lastConfig;
  std::vector<uint8_t> stored;
  bool serve = false;
  int nativeCalls = 0;
  /// What lookup_native answers.
  SHUnitCreator nativeAnswer = nullptr;
  /// "native" and "lookup" in call order, to pin which tier is asked first.
  std::vector<std::string> order;
  int acceptedCalls = 0;
  /// When set, a hit hands Hermes a heap copy of `stored` with a finalizer
  /// that frees it -- a buffer that really is gone once finalized.
  bool useFinalizer = false;
  /// "lookup", "finalize", "accepted", "store", "discard", in call order.
  std::vector<std::string> events;

  static void finalize(const uint8_t *data, size_t, void *hint) {
    static_cast<FakeCache *>(hint)->events.push_back("finalize");
    delete[] data;
  }

  static void accepted(void *ctx, void *) {
    auto *self = static_cast<FakeCache *>(ctx);
    self->acceptedCalls++;
    self->events.push_back("accepted");
  }

  static SHUnitCreator lookupNative(void *ctx, const uint8_t *, size_t) {
    auto *self = static_cast<FakeCache *>(ctx);
    self->nativeCalls++;
    self->order.push_back("native");
    return self->nativeAnswer;
  }

  static bool lookup(
      void *ctx, const uint8_t *, size_t, const uint8_t *config,
      size_t configSize, const uint8_t **hbc, size_t *hbcSize,
      void (**finalizeCb)(const uint8_t *, size_t, void *),
      void **finalizeHint, void **storeToken) {
    auto *self = static_cast<FakeCache *>(ctx);
    self->lookups++;
    self->order.push_back("lookup");
    self->events.push_back("lookup");
    self->lastConfig.assign(
        reinterpret_cast<const char *>(config), configSize);
    // A token is produced on every path, hit or miss.
    self->outstandingTokens++;
    *storeToken = self;
    if (!self->serve || self->stored.empty())
      return false;
    if (self->useFinalizer) {
      auto *copy = new uint8_t[self->stored.size()];
      std::memcpy(copy, self->stored.data(), self->stored.size());
      *hbc = copy;
      *finalizeCb = &FakeCache::finalize;
      *finalizeHint = self;
    } else {
      *hbc = self->stored.data();
      *finalizeCb = nullptr;
      *finalizeHint = nullptr;
    }
    *hbcSize = self->stored.size();
    return true;
  }

  static void store(
      void *ctx, void *, const uint8_t *hbc, size_t hbcSize) {
    auto *self = static_cast<FakeCache *>(ctx);
    self->stores++;
    self->events.push_back("store");
    self->outstandingTokens--;
    self->stored.assign(hbc, hbc + hbcSize);
  }

  static void discard(void *ctx, void *) {
    auto *self = static_cast<FakeCache *>(ctx);
    self->discards++;
    self->events.push_back("discard");
    self->outstandingTokens--;
  }

  hermes_wasm_cache_callbacks callbacks() {
    hermes_wasm_cache_callbacks cbs{};
    cbs.struct_size = sizeof(cbs);
    cbs.ctx = this;
    cbs.lookup = &FakeCache::lookup;
    cbs.store = &FakeCache::store;
    cbs.discard = &FakeCache::discard;
    cbs.lookup_native = &FakeCache::lookupNative;
    cbs.accepted = &FakeCache::accepted;
    return cbs;
  }
};

// Everything below drives a real WebAssembly.Module compile, so it needs a
// build that has the Wasm frontend. HERMES_ENABLE_WASM defaults to OFF while
// this file is compiled either way, so without this guard a default build
// runs eight tests whose WebAssembly global does not exist and fails all
// eight. The lit suite has had the equivalent since the beginning, as the
// `wasm` feature in test/lit.cfg.
#ifdef HERMES_ENABLE_WASM

/// wat2wasm output for:
///   (module (func (export "add") (param i32 i32) (result i32)
///     (i32.add (local.get 0) (local.get 1))))
static const uint8_t kAdd[] = {
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x07, 0x01, 0x60,
    0x02, 0x7f, 0x7f, 0x01, 0x7f, 0x03, 0x02, 0x01, 0x00, 0x07, 0x07, 0x01,
    0x03, 0x61, 0x64, 0x64, 0x00, 0x00, 0x0a, 0x09, 0x01, 0x07, 0x00, 0x20,
    0x00, 0x20, 0x01, 0x6a, 0x0b};

/// Compile kAdd through the JS API and return whether it succeeded.
static bool compileAdd(napi_env env) {
  napi_handle_scope scope = nullptr;
  EXPECT_EQ(napi_ok, napi_open_handle_scope(env, &scope));
  napi_value global = nullptr, result = nullptr;
  EXPECT_EQ(napi_ok, napi_get_global(env, &global));
  // Build the module from a literal byte array so the test needs no fixtures.
  std::string src = "(function(){var b=new Uint8Array([";
  for (size_t i = 0; i < sizeof(kAdd); ++i)
    src += std::to_string(kAdd[i]) + ",";
  src += "]); var m=new WebAssembly.Module(b);"
         "return new WebAssembly.Instance(m).exports.add(2,3);})()";
  napi_value script = nullptr;
  EXPECT_EQ(
      napi_ok,
      napi_create_string_utf8(
          env, src.c_str(), NAPI_AUTO_LENGTH, &script));
  napi_status st = napi_run_script(env, script, &result);
  bool ok = false;
  if (st == napi_ok) {
    int32_t v = 0;
    EXPECT_EQ(napi_ok, napi_get_value_int32(env, result, &v));
    ok = (v == 5);
  } else {
    napi_value ignored = nullptr;
    napi_get_and_clear_last_exception(env, &ignored);
  }
  EXPECT_EQ(napi_ok, napi_close_handle_scope(env, scope));
  return ok;
}

/// new WebAssembly.Module over \p bytes; true if it compiled. Leaves no
/// exception pending.
static bool compileBytes(napi_env env, const uint8_t *bytes, size_t size) {
  napi_handle_scope scope = nullptr;
  EXPECT_EQ(napi_ok, napi_open_handle_scope(env, &scope));
  std::string src = "(function(){var b=new Uint8Array([";
  for (size_t i = 0; i < size; ++i)
    src += std::to_string(bytes[i]) + ",";
  src += "]); new WebAssembly.Module(b); return true;})()";
  napi_value script = nullptr, result = nullptr;
  EXPECT_EQ(
      napi_ok,
      napi_create_string_utf8(env, src.c_str(), NAPI_AUTO_LENGTH, &script));
  bool ok = napi_run_script(env, script, &result) == napi_ok;
  if (!ok) {
    napi_value ignored = nullptr;
    napi_get_and_clear_last_exception(env, &ignored);
  }
  EXPECT_EQ(napi_ok, napi_close_handle_scope(env, scope));
  return ok;
}

/// The right magic, a version Hermes does not know, nothing after it.
static const uint8_t kGarbage[] =
    {0x00, 0x61, 0x73, 0x6d, 0x09, 0x00, 0x00, 0x00};

/// Never called: a creator the native tier offers but Hermes must refuse. If
/// Hermes ever ran it, the test process aborts, which is the failure.
static SHUnit *creatorThatMustNotRun(void) {
  ::abort();
}

TEST_F(NapiTestFixture, WasmCache_NativeTierIsAskedFirst) {
  FakeCache cache;
  auto cbs = cache.callbacks();
  ASSERT_EQ(napi_ok, hermes_set_wasm_cache(env_, &cbs));
  EXPECT_TRUE(compileAdd(env_));
  EXPECT_EQ(1, cache.nativeCalls);
  EXPECT_EQ((std::vector<std::string>{"native", "lookup"}), cache.order);
  EXPECT_EQ(1, cache.stores);
}

TEST_F(NapiTestFixture, WasmCache_UnregisteredCreatorFallsThrough) {
  FakeCache cache;
  cache.nativeAnswer = &creatorThatMustNotRun;
  auto cbs = cache.callbacks();
  ASSERT_EQ(napi_ok, hermes_set_wasm_cache(env_, &cbs));
  EXPECT_TRUE(compileAdd(env_));
  EXPECT_EQ(1, cache.nativeCalls);
  EXPECT_EQ(1, cache.lookups) << "a refused creator is a miss";
  EXPECT_EQ(1, cache.stores);
}

static SHUnit *mismatchedCreator(void) {
  ::abort();
}
static SHUnit *configlessCreator(void) {
  ::abort();
}

TEST_F(NapiTestFixture, WasmCache_MismatchedRegistrationFallsThrough) {
  // Registered once per process: the registry has no unregistration and
  // aborts on a duplicate name.
  static SHWasmUnitReg reg = {
      "napitest_mismatch",
      mismatchedCreator,
      "hermes-wasm;bc=0;cg=0;t262=0",
      nullptr};
  static bool registered = (_sh_wasm_register_unit(&reg), true);
  (void)registered;

  FakeCache cache;
  cache.nativeAnswer = &mismatchedCreator;
  auto cbs = cache.callbacks();
  ASSERT_EQ(napi_ok, hermes_set_wasm_cache(env_, &cbs));
  EXPECT_TRUE(compileAdd(env_));
  EXPECT_EQ(1, cache.lookups);
  EXPECT_EQ(1, cache.stores);
}

TEST_F(NapiTestFixture, WasmCache_ConfiglessRegistrationFallsThrough) {
  static SHWasmUnitReg reg = {
      "napitest_configless", configlessCreator, nullptr, nullptr};
  static bool registered = (_sh_wasm_register_unit(&reg), true);
  (void)registered;

  FakeCache cache;
  cache.nativeAnswer = &configlessCreator;
  auto cbs = cache.callbacks();
  ASSERT_EQ(napi_ok, hermes_set_wasm_cache(env_, &cbs));
  EXPECT_TRUE(compileAdd(env_));
  EXPECT_EQ(1, cache.lookups);
}

TEST_F(NapiTestFixture, WasmCache_NullNativeAnswerIsTheBytecodePath) {
  FakeCache cache;
  auto cbs = cache.callbacks();
  cbs.lookup_native = nullptr; // no native tier at all
  ASSERT_EQ(napi_ok, hermes_set_wasm_cache(env_, &cbs));
  EXPECT_TRUE(compileAdd(env_));
  EXPECT_EQ(0, cache.nativeCalls);
  EXPECT_EQ(1, cache.lookups);
}

TEST_F(NapiTestFixture, WasmCache_MissCompilesAndStores) {
  FakeCache cache;
  auto cbs = cache.callbacks();
  ASSERT_EQ(napi_ok, hermes_set_wasm_cache(env_, &cbs));

  EXPECT_TRUE(compileAdd(env_));
  EXPECT_EQ(1, cache.lookups);
  EXPECT_EQ(1, cache.stores);
  EXPECT_EQ(0, cache.discards);
  EXPECT_EQ(0, cache.outstandingTokens);
  EXPECT_FALSE(cache.stored.empty());

  // The codegen config actually reaches the embedder, and carries what a
  // cache must key on. Asserted by content rather than by exact string: the
  // blob is deliberately extensible, so pinning it whole would fail on every
  // future addition for no reason.
  EXPECT_NE(std::string::npos, cache.lastConfig.find("hermes-wasm"));
  EXPECT_NE(std::string::npos, cache.lastConfig.find("bc="));
  EXPECT_NE(std::string::npos, cache.lastConfig.find("cg="));
  EXPECT_NE(std::string::npos, cache.lastConfig.find("t262=0"));
}

TEST_F(NapiTestFixture, WasmCache_HitDiscardsTokenAndDoesNotStore) {
  FakeCache cache;
  auto cbs = cache.callbacks();
  ASSERT_EQ(napi_ok, hermes_set_wasm_cache(env_, &cbs));

  EXPECT_TRUE(compileAdd(env_));   // populates
  cache.serve = true;
  EXPECT_TRUE(compileAdd(env_));   // served

  EXPECT_EQ(2, cache.lookups);
  EXPECT_EQ(1, cache.stores);
  EXPECT_EQ(1, cache.discards);
  EXPECT_EQ(0, cache.outstandingTokens);
}

TEST_F(NapiTestFixture, WasmCache_RejectedHitFallsBackAndStores) {
  FakeCache cache;
  auto cbs = cache.callbacks();
  ASSERT_EQ(napi_ok, hermes_set_wasm_cache(env_, &cbs));

  // Serve bytes that are not bytecode at all.
  cache.stored = {0xde, 0xad, 0xbe, 0xef, 0x00, 0x11, 0x22, 0x33};
  cache.serve = true;

  EXPECT_TRUE(compileAdd(env_)) << "a bad hit must fall back to compiling";
  EXPECT_EQ(1, cache.lookups);
  EXPECT_EQ(1, cache.stores) << "the fallback compile must overwrite the entry";
  EXPECT_EQ(0, cache.outstandingTokens);
}

TEST_F(NapiTestFixture, WasmCache_HitSkipsCompilationEntirely) {
  FakeCache cache;
  auto cbs = cache.callbacks();
  ASSERT_EQ(napi_ok, hermes_set_wasm_cache(env_, &cbs));

  EXPECT_TRUE(compileAdd(env_)); // populates cache.stored
  cache.serve = true;

  // Hand WebAssembly.Module bytes that could never compile, while the cache
  // serves bytecode for a module that works. If the result works, the
  // compile was genuinely skipped -- a fake that only counted calls could
  // not distinguish that from a compile that happened anyway.
  napi_handle_scope scope = nullptr;
  ASSERT_EQ(napi_ok, napi_open_handle_scope(env_, &scope));
  const char *src =
      "(function(){var b=new Uint8Array([0,97,115,109,9,9,9,9,1,2,3]);"
      "var m=new WebAssembly.Module(b);"
      "return new WebAssembly.Instance(m).exports.add(2,3);})()";
  napi_value script = nullptr, result = nullptr;
  ASSERT_EQ(napi_ok,
            napi_create_string_utf8(env_, src, NAPI_AUTO_LENGTH, &script));
  ASSERT_EQ(napi_ok, napi_run_script(env_, script, &result));
  int32_t v = 0;
  ASSERT_EQ(napi_ok, napi_get_value_int32(env_, result, &v));
  EXPECT_EQ(5, v) << "the cached module should have been used verbatim";
  EXPECT_EQ(napi_ok, napi_close_handle_scope(env_, scope));
}

TEST_F(NapiTestFixture, WasmCache_NoHooksStillCompiles) {
  EXPECT_TRUE(compileAdd(env_));
}

TEST_F(NapiTestFixture, WasmCache_TooSmallStructIsRejected) {
  FakeCache cache;
  auto cbs = cache.callbacks();
  // Claim a struct_size that predates some of the required callbacks, as an
  // older caller compiled against a smaller version of the struct would.
  cbs.struct_size = offsetof(hermes_wasm_cache_callbacks, lookup);
  EXPECT_EQ(napi_invalid_arg, hermes_set_wasm_cache(env_, &cbs));

  // No cache got installed, so a compile still runs uncached.
  EXPECT_TRUE(compileAdd(env_));
  EXPECT_EQ(0, cache.lookups);
}

TEST_F(NapiTestFixture, WasmCache_TrulyTruncatedStructIsNotReadPastItsEnd) {
  // The test above proves the SIZE is rejected, and nothing more: it hands
  // over a fully allocated struct that merely claims to be small, so reading
  // a callback field out of it is harmless whether or not the size is
  // checked first. It therefore passes with or without the fix it was
  // written for, which was about reading past the end of a caller's memory.
  //
  // An old caller's struct really is short. Allocate exactly what such a
  // caller would own -- up through struct_size and ctx, and not one byte
  // more -- so that touching `lookup` before validating the size is a
  // genuine out-of-bounds read, which AddressSanitizer fails the test for.
  const size_t oldStructSize = offsetof(hermes_wasm_cache_callbacks, lookup);
  void *shortAlloc = ::malloc(oldStructSize);
  ASSERT_NE(nullptr, shortAlloc);
  std::memset(shortAlloc, 0, oldStructSize);
  // struct_size is the first member, so it is inside even this allocation --
  // which is the whole reason the ABI can put it there and nowhere else.
  std::memcpy(shortAlloc, &oldStructSize, sizeof(size_t));

  auto *asCallbacks =
      static_cast<const hermes_wasm_cache_callbacks *>(shortAlloc);
  EXPECT_EQ(napi_invalid_arg, hermes_set_wasm_cache(env_, asCallbacks));

  ::free(shortAlloc);
}

TEST_F(NapiTestFixture, WasmCache_LargerStructIsAccepted) {
  // The other half of the same check, and untested until now: a NEWER caller,
  // built against a later header that appended fields, reports a larger
  // struct_size and must be accepted -- only the fields this version knows
  // about are ever read. That is what makes the ABI extensible, and a future
  // tightening back to exact equality would break every such caller with
  // nothing here to notice.
  struct Extended {
    hermes_wasm_cache_callbacks base;
    // Stands in for whatever a later version appends.
    void (*somethingAddedLater)(void *) = nullptr;
  };
  FakeCache cache;
  Extended ext{};
  ext.base = cache.callbacks();
  ext.base.struct_size = sizeof(Extended);
  ASSERT_GT(sizeof(Extended), sizeof(hermes_wasm_cache_callbacks));

  EXPECT_EQ(napi_ok, hermes_set_wasm_cache(env_, &ext.base));

  // Accepted AND installed: the known fields were read and used, rather than
  // the call merely being tolerated.
  EXPECT_TRUE(compileAdd(env_));
  EXPECT_EQ(1, cache.lookups);
}

/// The callbacks struct exactly as a caller compiled before lookup_native
/// existed owns it: allocated to the old size and not one byte more, so a
/// read of any field appended since is a heap overflow AddressSanitizer
/// fails the test for. Installs, and serves a miss/store round trip and an
/// accepted-hit round trip, with the new slots never read.
TEST_F(NapiTestFixture, WasmCache_OldSizedStructInstallsAndRoundTrips) {
  FakeCache cache;
  hermes_wasm_cache_callbacks full = cache.callbacks();
  const size_t oldSize = offsetof(hermes_wasm_cache_callbacks, lookup_native);
  full.struct_size = oldSize;
  void *oldAlloc = ::malloc(oldSize);
  ASSERT_NE(nullptr, oldAlloc);
  std::memcpy(oldAlloc, &full, oldSize);
  auto *old = static_cast<const hermes_wasm_cache_callbacks *>(oldAlloc);

  ASSERT_EQ(napi_ok, hermes_set_wasm_cache(env_, old));
  EXPECT_TRUE(compileAdd(env_)); // miss, compile, store
  cache.serve = true;
  EXPECT_TRUE(compileAdd(env_)); // accepted hit, discard
  EXPECT_EQ(2, cache.lookups);
  EXPECT_EQ(1, cache.stores);
  EXPECT_EQ(1, cache.discards);
  EXPECT_EQ(0, cache.outstandingTokens);

  ASSERT_EQ(napi_ok, hermes_set_wasm_cache(env_, nullptr));
  ::free(oldAlloc);
}

TEST_F(NapiTestFixture, WasmCache_AcceptedHitOrderIsFinalizeAcceptedDiscard) {
  FakeCache cache;
  auto cbs = cache.callbacks();
  ASSERT_EQ(napi_ok, hermes_set_wasm_cache(env_, &cbs));
  EXPECT_TRUE(compileAdd(env_)); // populates
  cache.serve = true;
  cache.useFinalizer = true;
  cache.events.clear();
  EXPECT_TRUE(compileAdd(env_));
  EXPECT_EQ(
      (std::vector<std::string>{"lookup", "finalize", "accepted", "discard"}),
      cache.events);
}

TEST_F(NapiTestFixture, WasmCache_RejectedHitIsNeverAccepted) {
  FakeCache cache;
  cache.stored = {0xde, 0xad, 0xbe, 0xef, 0x00, 0x11, 0x22, 0x33};
  cache.serve = true;
  auto cbs = cache.callbacks();
  ASSERT_EQ(napi_ok, hermes_set_wasm_cache(env_, &cbs));
  EXPECT_TRUE(compileAdd(env_)); // rejected hit, recompiled, stored
  EXPECT_EQ(0, cache.acceptedCalls);
  EXPECT_EQ(1, cache.stores);
}

TEST_F(NapiTestFixture, WasmCache_RejectedHitThenFailedCompileIsABareDiscard) {
  // Bytes that do not compile, and a cache entry for them that Hermes
  // refuses: no store, no accepted, one discard.
  FakeCache cache;
  cache.stored = {0xde, 0xad, 0xbe, 0xef, 0x00, 0x11, 0x22, 0x33};
  cache.serve = true;
  auto cbs = cache.callbacks();
  ASSERT_EQ(napi_ok, hermes_set_wasm_cache(env_, &cbs));
  EXPECT_FALSE(compileBytes(env_, kGarbage, sizeof(kGarbage)));
  EXPECT_EQ(0, cache.acceptedCalls);
  EXPECT_EQ(0, cache.stores);
  EXPECT_EQ(1, cache.discards);
  EXPECT_EQ(0, cache.outstandingTokens);
}

/// A caller whose struct reaches lookup_native but stops before accepted.
TEST_F(NapiTestFixture, WasmCache_StructWithoutAcceptedIsNotReadPastItsEnd) {
  FakeCache cache;
  hermes_wasm_cache_callbacks full = cache.callbacks();
  const size_t size = offsetof(hermes_wasm_cache_callbacks, accepted);
  full.struct_size = size;
  void *alloc = ::malloc(size);
  ASSERT_NE(nullptr, alloc);
  std::memcpy(alloc, &full, size);
  ASSERT_EQ(
      napi_ok,
      hermes_set_wasm_cache(
          env_, static_cast<const hermes_wasm_cache_callbacks *>(alloc)));
  EXPECT_TRUE(compileAdd(env_));
  cache.serve = true;
  EXPECT_TRUE(compileAdd(env_));
  EXPECT_EQ(0, cache.acceptedCalls);
  EXPECT_EQ(1, cache.discards);
  ASSERT_EQ(napi_ok, hermes_set_wasm_cache(env_, nullptr));
  ::free(alloc);
}

#else

// With no Wasm support there is nothing to install hooks into, and
// hermes_set_wasm_cache says so rather than reporting a success it cannot
// deliver. Worth pinning precisely because it is the branch nobody runs: an
// embedder that got napi_ok here would wire up a cache, see no callback ever
// fire, and have nothing to explain why.
TEST_F(NapiTestFixture, WasmCache_RefusedWithoutWasmSupport) {
  FakeCache cache;
  auto cbs = cache.callbacks();
  EXPECT_EQ(napi_generic_failure, hermes_set_wasm_cache(env_, &cbs));
}

#endif

} // namespace
