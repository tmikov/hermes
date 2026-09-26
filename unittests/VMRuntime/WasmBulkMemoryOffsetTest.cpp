/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#ifdef HERMES_ENABLE_WASM

#include "VMRuntimeTestHelpers.h"
#include "gtest/gtest.h"

#include "hermes/FrontEndDefs/Builtins.h"
#include "hermes/VM/Callable.h"
#include "hermes/VM/JSArrayBuffer.h"
#include "hermes/VM/JSObject.h"
#include "hermes/VM/JSTypedArray.h"

#include <cstring>

using namespace hermes::vm;

namespace {

using WasmBulkMemoryOffsetTest = RuntimeTestFixture;

/// \return \p src evaluated as a script, as its completion value, rooted.
static Handle<> evalExpr(Runtime &runtime, llvh::StringRef src) {
  hermes::hbc::CompileFlags flags;
  auto res =
      runtime.run(src, "file:///wasm-bulk-memory-offset-test.js", flags);
  EXPECT_EQ(ExecutionStatus::RETURNED, res.getStatus());
  if (res == ExecutionStatus::EXCEPTION)
    return Runtime::getUndefinedValue();
  return runtime.makeHandle(*res);
}

/// wasmMemoryFill, wasmMemoryCopy and wasmMemoryInit (lib/VM/JSLib/
/// HermesBuiltin.cpp) all computed their memory destinations (and, for
/// memory.copy, the source too) as "start of the underlying ArrayBuffer +
/// offset", ignoring the linear-memory view's own byte offset, and
/// bounds-checked byte offsets against the view's ELEMENT count
/// (getLength()) rather than its byte length (getByteLength()). This mirrors
/// the bug fixed in wasmDataSegmentInit (see WasmDataSegmentOffsetTest.cpp);
/// this file covers the three bulk-memory builtins that were fixed
/// alongside it.
///
/// Nothing reachable from compiled Wasm exercises a nonzero offset:
/// WasmIRGen always builds the linear-memory view as `new Uint8Array(buffer)`
/// with a single argument, so offset_ is always 0 and the two bugs coincide
/// with correct behavior. These tests build views the way generated code
/// never does -- with a nonzero byteOffset and a length narrower than the
/// whole buffer -- and call the builtins directly through
/// Runtime::getBuiltinCallable, exactly as CallBuiltin bytecode would, to
/// prove the destination (and source) is offset_ + index rather than index.
TEST_F(WasmBulkMemoryOffsetTest, MemoryFillRespectsViewByteOffset) {
  Callable *fillRaw = runtime.getBuiltinCallable(
      hermes::BuiltinMethod::HermesBuiltin_wasmMemoryFill);
  ASSERT_NE(nullptr, fillRaw);
  auto fill = runtime.makeHandle(fillRaw);

  // A 16-byte backing buffer with a view that starts 4 bytes in and is 8
  // bytes long -- neither 0 nor the full buffer, so a destination computed
  // from the buffer's start rather than the view's is observably wrong.
  auto view = evalExpr(runtime, R"JS(
    var buf = new ArrayBuffer(16);
    new Uint8Array(buf, 4, 8);
  )JS");
  ASSERT_TRUE(vmisa<JSTypedArrayBase>(*view));
  auto *arr = vmcast<JSTypedArrayBase>(*view);
  ASSERT_EQ(8u, arr->getByteLength());
  JSArrayBuffer *buffer = arr->getBuffer(runtime);
  ASSERT_EQ(16u, buffer->size());
  std::memset(buffer->getDataBlock(), 0, buffer->size());

  // dest=2, value=0xAA, size=3 into a view whose byte offset is 4: the
  // correct destination is absolute buffer bytes 4+2 .. 4+2+3 = [6, 9).
  auto res = Callable::executeCall4(
      fill,
      runtime,
      Runtime::getUndefinedValue(),
      view.getHermesValue(),
      HermesValue::encodeTrustedNumberValue(2),
      HermesValue::encodeTrustedNumberValue(0xAA),
      HermesValue::encodeTrustedNumberValue(3));
  ASSERT_EQ(ExecutionStatus::RETURNED, res.getStatus());

  const uint8_t *raw = buffer->getDataBlock();
  EXPECT_EQ(0xAA, raw[6]);
  EXPECT_EQ(0xAA, raw[7]);
  EXPECT_EQ(0xAA, raw[8]);
  // The buggy destination -- dest alone, ignoring offset_ -- would have
  // written here instead. It must be untouched.
  EXPECT_EQ(0, raw[2]);
  EXPECT_EQ(0, raw[3]);
  EXPECT_EQ(0, raw[4]);
  for (unsigned i = 0; i < 16; ++i)
    if (i < 6 || i > 8)
      EXPECT_EQ(0, raw[i]) << "byte " << i;
}

TEST_F(WasmBulkMemoryOffsetTest, MemoryCopyRespectsViewByteOffset) {
  Callable *copyRaw = runtime.getBuiltinCallable(
      hermes::BuiltinMethod::HermesBuiltin_wasmMemoryCopy);
  ASSERT_NE(nullptr, copyRaw);
  auto copy = runtime.makeHandle(copyRaw);

  // A 20-byte backing buffer with a view that starts 4 bytes in and is 12
  // bytes long. The view is pre-filled with view[i] = i so a copy's actual
  // source bytes can be identified by value.
  auto view = evalExpr(runtime, R"JS(
    var buf = new ArrayBuffer(20);
    var v = new Uint8Array(buf, 4, 12);
    for (var i = 0; i < 12; ++i) v[i] = i + 1;
    v;
  )JS");
  ASSERT_TRUE(vmisa<JSTypedArrayBase>(*view));
  auto *arr = vmcast<JSTypedArrayBase>(*view);
  ASSERT_EQ(12u, arr->getByteLength());
  JSArrayBuffer *buffer = arr->getBuffer(runtime);
  ASSERT_EQ(20u, buffer->size());

  // dest=0, src=6, size=3 into a view whose byte offset is 4: the correct
  // source is absolute buffer bytes 4+6..4+9 = [10, 13), holding
  // view[6..9) = {7, 8, 9}; the correct destination is absolute buffer
  // bytes 4+0..4+3 = [4, 7).
  auto res = Callable::executeCall4(
      copy,
      runtime,
      Runtime::getUndefinedValue(),
      view.getHermesValue(),
      HermesValue::encodeTrustedNumberValue(0),
      HermesValue::encodeTrustedNumberValue(6),
      HermesValue::encodeTrustedNumberValue(3));
  ASSERT_EQ(ExecutionStatus::RETURNED, res.getStatus());

  const uint8_t *raw = buffer->getDataBlock();
  EXPECT_EQ(7, raw[4]);
  EXPECT_EQ(8, raw[5]);
  EXPECT_EQ(9, raw[6]);
  // The buggy destination -- dest(0) alone, ignoring offset_ -- would have
  // written to buffer bytes [0, 3). Untouched, still zero-initialized.
  EXPECT_EQ(0, raw[0]);
  EXPECT_EQ(0, raw[1]);
  EXPECT_EQ(0, raw[2]);
}

TEST_F(WasmBulkMemoryOffsetTest, MemoryInitRespectsBothViewsByteOffset) {
  Callable *initRaw = runtime.getBuiltinCallable(
      hermes::BuiltinMethod::HermesBuiltin_wasmMemoryInit);
  ASSERT_NE(nullptr, initRaw);
  auto init = runtime.makeHandle(initRaw);

  // Destination: a 16-byte buffer with a view starting 4 bytes in, 8 bytes
  // long. Source data segment: a separate 16-byte buffer with a view
  // starting 3 bytes in, 5 bytes long, pre-filled with known bytes. Both
  // offsets are nonzero and distinct so a bug in either view is caught
  // independently.
  auto argsArr = evalExpr(runtime, R"JS(
    var destBuf = new ArrayBuffer(16);
    var destView = new Uint8Array(destBuf, 4, 8);
    var segBuf = new ArrayBuffer(16);
    var segView = new Uint8Array(segBuf, 3, 5);
    for (var i = 0; i < 5; ++i) segView[i] = 0x10 + i;
    // (heapu8, dataSegs, segIdx, dest, src, size)
    [destView, [segView], 0, 1, 1, 3];
  )JS");
  ASSERT_TRUE(vmisa<JSArray>(*argsArr));

  auto res = Callable::executeCall(
      init,
      runtime,
      Runtime::getUndefinedValue(),
      Runtime::getUndefinedValue(),
      Handle<JSObject>::vmcast(argsArr));
  ASSERT_EQ(ExecutionStatus::RETURNED, res.getStatus());

  // dest=1, src=1, size=3: correct destination is destBuf absolute bytes
  // 4+1..4+4 = [5, 8); correct source is segBuf absolute bytes 3+1..3+4 =
  // [4, 7), holding segView[1..4) = {0x11, 0x12, 0x13}.
  auto destView = evalExpr(runtime, "destView");
  auto *destArr = vmcast<JSTypedArrayBase>(*destView);
  JSArrayBuffer *destBuf = destArr->getBuffer(runtime);
  const uint8_t *destRaw = destBuf->getDataBlock();
  EXPECT_EQ(0x11, destRaw[5]);
  EXPECT_EQ(0x12, destRaw[6]);
  EXPECT_EQ(0x13, destRaw[7]);
  // The buggy destination -- dest(1) alone, ignoring destView's offset_ --
  // would have written to destBuf bytes [1, 4). Untouched, still zero.
  EXPECT_EQ(0, destRaw[1]);
  EXPECT_EQ(0, destRaw[2]);
  EXPECT_EQ(0, destRaw[3]);
}

} // namespace

#endif // HERMES_ENABLE_WASM
