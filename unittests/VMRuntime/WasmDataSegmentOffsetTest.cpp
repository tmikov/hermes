/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#ifdef HERMES_ENABLE_WASM

#include "VMRuntimeTestHelpers.h"
#include "gtest/gtest.h"

#include "hermes/VM/JSArrayBuffer.h"
#include "hermes/VM/JSTypedArray.h"
#include "hermes/VM/StaticHUtils.h"
#include "hermes/VM/static_h.h"

#include <cstring>

using namespace hermes::vm;

namespace {

using WasmDataSegmentOffsetTest = RuntimeTestFixture;

/// \return \p src evaluated as a script, as its completion value, rooted.
static Handle<> evalExpr(Runtime &runtime, llvh::StringRef src) {
  hermes::hbc::CompileFlags flags;
  auto res = runtime.run(src, "file:///wasm-data-segment-offset-test.js",
                          flags);
  EXPECT_EQ(ExecutionStatus::RETURNED, res.getStatus());
  if (res == ExecutionStatus::EXCEPTION)
    return Runtime::getUndefinedValue();
  return runtime.makeHandle(*res);
}

/// _sh_wasm_data_segment_init, the native/SH counterpart of the
/// wasmDataSegmentInit builtin (lib/VM/JSLib/HermesBuiltin.cpp), copies binary
/// data into a Wasm linear-memory view. Both implementations used to compute
/// the destination as "start of the underlying ArrayBuffer + dest", ignoring
/// the view's own byte offset, and bounds-checked against the view's ELEMENT
/// count rather than its byte length.
///
/// Nothing reachable from compiled Wasm exercises a nonzero offset: WasmIRGen
/// (WasmIRGen.cpp) always builds the memory view as `new Uint8Array(buffer)`,
/// so offset_ is always 0 and the two bugs coincide with correct behavior.
/// This test builds the view the way generated code never does -- with a
/// nonzero byteOffset and length narrower than the whole buffer -- and calls
/// the SH entry point directly, exactly as generated code would, to prove the
/// destination is offset_ + dest rather than dest.
TEST_F(WasmDataSegmentOffsetTest, RespectsViewByteOffset) {
  GCScope scope{runtime, "RespectsViewByteOffset"};

  // A 16-byte backing buffer with a view that starts 4 bytes in and is 8
  // bytes long -- neither 0 nor the full buffer, so a destination computed
  // from the buffer's start rather than the view's is observably wrong.
  auto heapu8 = evalExpr(runtime, R"JS(
    var buf = new ArrayBuffer(16);
    new Uint8Array(buf, 4, 8);
  )JS");
  ASSERT_TRUE(vmisa<JSTypedArrayBase>(*heapu8));
  auto *arr = vmcast<JSTypedArrayBase>(*heapu8);
  ASSERT_EQ(8u, arr->getByteLength());
  JSArrayBuffer *buffer = arr->getBuffer(runtime);
  ASSERT_EQ(16u, buffer->size());

  // Zero the whole backing store explicitly, so the assertions below are
  // about what the call wrote rather than about the buffer's initial state.
  std::memset(buffer->getDataBlock(), 0, buffer->size());

  static const unsigned char kBlob[4] = {0xAA, 0xBB, 0xCC, 0xDD};
  // Only binary_data and binary_data_size are read by
  // _sh_wasm_data_segment_init; every other field of SHUnit is unused on
  // this path, so a stack instance with those two fields set is enough.
  SHUnit unit = {};
  unit.binary_data = kBlob;
  unit.binary_data_size = sizeof(kBlob);

  // dest=2 into a view whose byte offset is 4: the correct destination is
  // absolute buffer byte 4+2=6, not byte 2.
  _sh_wasm_data_segment_init(
      getSHRuntime(runtime),
      &unit,
      SHLegacyValue{(*heapu8).getRaw()},
      /*blobOffset=*/0,
      /*length=*/4,
      /*dest=*/2);

  const uint8_t *raw = buffer->getDataBlock();
  // The correct destination: offset_(4) + dest(2) = buffer byte 6..9.
  EXPECT_EQ(0xAA, raw[6]);
  EXPECT_EQ(0xBB, raw[7]);
  EXPECT_EQ(0xCC, raw[8]);
  EXPECT_EQ(0xDD, raw[9]);
  // The buggy destination -- dest alone, ignoring offset_ -- would have
  // written here instead. It must be untouched.
  EXPECT_EQ(0, raw[2]);
  EXPECT_EQ(0, raw[3]);
  EXPECT_EQ(0, raw[4]);
  EXPECT_EQ(0, raw[5]);
  // Nothing outside either candidate destination was touched either.
  EXPECT_EQ(0, raw[0]);
  EXPECT_EQ(0, raw[1]);
  for (unsigned i = 10; i < 16; ++i)
    EXPECT_EQ(0, raw[i]) << "byte " << i;
}

} // namespace

#endif // HERMES_ENABLE_WASM
