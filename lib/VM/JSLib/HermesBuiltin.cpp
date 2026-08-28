/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "JSLibInternal.h"

#include "hermes/FrontEndDefs/Builtins.h"
#include "hermes/FrontEndDefs/Typeof.h"
#include "hermes/Support/Base64vlq.h"
#include "hermes/VM/Callable.h"
#include "hermes/VM/FastArray.h"
#include "hermes/VM/JSArray.h"
#include "hermes/VM/BigIntPrimitive.h"
#include "hermes/VM/JSArrayBuffer.h"
#include "hermes/VM/JSLib.h"
#include "hermes/VM/JSTypedArray.h"
#include "hermes/VM/JSWebAssemblyMemory.h"
#include "hermes/VM/JSRegExp.h"
#include "hermes/VM/Operations.h"
#include "hermes/VM/PrimitiveBox.h"
#include "hermes/VM/StackFrame-inline.h"
#include "hermes/VM/StringBuilder.h"
#include "hermes/VM/StringView.h"

#include "hermes/Support/Conversions.h"

#include <algorithm>
#include <cmath>
#include <cstring>
#include <random>

namespace hermes {
namespace vm {

/// Set the parent of an object failing silently on any error.
CallResult<HermesValue> silentObjectSetPrototypeOf(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  JSObject *O = dyn_vmcast<JSObject>(args.getArg(0));
  if (!O)
    return HermesValue::encodeUndefinedValue();

  JSObject *parent;
  HermesValue V = args.getArg(1);
  if (V.isNull())
    parent = nullptr;
  else if (V.isObject())
    parent = vmcast<JSObject>(V);
  else
    return HermesValue::encodeUndefinedValue();

  (void)JSObject::setParent(O, runtime, parent);

  // Ignore exceptions.
  runtime.clearThrownValue();

  return HermesValue::encodeUndefinedValue();
}

/// ES6.0 12.2.9.3 Runtime Semantics: GetTemplateObject ( templateLiteral )
/// Given a template literal, return a template object that looks like this:
/// [cookedString0, cookedString1, ..., raw: [rawString0, rawString1]].
/// This object is frozen, as well as the 'raw' object nested inside.
/// We only pass the parts from the template literal that are needed to
/// construct this object. That is, the raw strings and cooked strings.
/// Arguments: \p templateObjID is the unique id associated with the template
/// object. \p dup is a boolean, when it is true, cooked strings are the same as
/// raw strings. Then raw strings are passed. Finally cooked strings are
/// optionally passed if \p dup is true.
CallResult<HermesValue> hermesBuiltinGetTemplateObject(
    void *,
    Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  if (LLVM_UNLIKELY(args.getArgCount() < 3)) {
    return runtime.raiseTypeError("At least three arguments expected");
  }
  if (LLVM_UNLIKELY(!args.getArg(0).isNumber())) {
    return runtime.raiseTypeError("First argument should be a number");
  }
  if (LLVM_UNLIKELY(!args.getArg(1).isBool())) {
    return runtime.raiseTypeError("Second argument should be a bool");
  }

  struct : public Locals {
    PinnedValue<JSArray> rawObj;
    PinnedValue<JSArray> templateObj;
    PinnedValue<> idx;
    PinnedValue<> rawValue;
    PinnedValue<> cookedValue;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  GCScope gcScope{runtime};

  // Try finding the template object in the template object cache.
  uint32_t templateObjID = args.getArg(0).getNumberAs<uint32_t>();

  // Retrieve the code block of the caller to get the cache.
  auto frames = runtime.getStackFrames();
  auto it = frames.begin();
  if (LLVM_UNLIKELY(++it == frames.end()))
    return runtime.raiseTypeError("Cannot be called directly");
  auto callerCB = it->getCalleeCodeBlock();
  if (LLVM_UNLIKELY(!callerCB)) {
    return runtime.raiseTypeError("Cannot be called from native code");
  }
  RuntimeModule *runtimeModule = callerCB->getRuntimeModule();
  JSObject *cachedTemplateObj =
      runtimeModule->findCachedTemplateObject(templateObjID);
  if (cachedTemplateObj) {
    return HermesValue::encodeObjectValue(cachedTemplateObj);
  }

  bool dup = args.getArg(1).getBool();
  if (LLVM_UNLIKELY(!dup && args.getArgCount() % 2 == 1)) {
    return runtime.raiseTypeError(
        "There must be the same number of raw and cooked strings.");
  }
  uint32_t count = dup ? args.getArgCount() - 2 : args.getArgCount() / 2 - 1;

  // Create template object and raw object.
  auto arrRes = JSArray::create(runtime, count, 0);
  if (LLVM_UNLIKELY(arrRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  lv.rawObj = std::move(*arrRes);
  auto arrRes2 = JSArray::create(runtime, count, 0);
  if (LLVM_UNLIKELY(arrRes2 == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  lv.templateObj = std::move(*arrRes2);

  // Set cooked and raw strings as elements in template object and raw object,
  // respectively.
  DefinePropertyFlags dpf = DefinePropertyFlags::getDefaultNewPropertyFlags();
  dpf.writable = 0;
  dpf.configurable = 0;
  uint32_t cookedBegin = dup ? 2 : 2 + count;
  auto marker = gcScope.createMarker();
  for (uint32_t i = 0; i < count; ++i) {
    lv.idx = HermesValue::encodeTrustedNumberValue(i);

    lv.cookedValue = args.getArg(cookedBegin + i);
    auto putRes = JSObject::defineOwnComputedPrimitive(
        lv.templateObj, runtime, lv.idx, dpf, lv.cookedValue);
    assert(
        putRes != ExecutionStatus::EXCEPTION && *putRes &&
        "Failed to set cooked value to template object.");

    lv.rawValue = args.getArg(2 + i);
    putRes = JSObject::defineOwnComputedPrimitive(
        lv.rawObj, runtime, lv.idx, dpf, lv.rawValue);
    assert(
        putRes != ExecutionStatus::EXCEPTION && *putRes &&
        "Failed to set raw value to raw object.");

    gcScope.flushToMarker(marker);
  }

  if (LLVM_UNLIKELY(
          setTemplateObjectProps(runtime, lv.templateObj, lv.rawObj) ==
          ExecutionStatus::EXCEPTION))
    return ExecutionStatus::EXCEPTION;

  // Cache the template object.
  runtimeModule->cacheTemplateObject(templateObjID, lv.templateObj);

  return lv.templateObj.getHermesValue();
}

/// If the first argument is not an object, throw a type error with the second
/// argument as a message.
///
/// \code
///   HermesBuiltin.ensureObject = function(value, errorMessage) {...}
/// \endcode
CallResult<HermesValue> hermesBuiltinEnsureObject(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  if (LLVM_LIKELY(args.getArg(0).isObject()))
    return HermesValue::encodeUndefinedValue();

  return runtime.raiseTypeError(args.getArgHandle(1));
}

/// Perform the GetMethod() abstract operation.
///
/// \code
///   HermesBuiltin.getMethod = function(object, property) {...}
/// \endcode
CallResult<HermesValue> hermesBuiltinGetMethod(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  return getMethod(runtime, args.getArgHandle(0), args.getArgHandle(1))
      .toCallResultHermesValue();
}

/// Throw a type error with the argument as a message.
///
/// \code
///   HermesBuiltin.throwTypeError = function(errorMessage) {...}
/// \endcode
CallResult<HermesValue> hermesBuiltinThrowTypeError(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  return runtime.raiseTypeError(args.getArgHandle(0));
}

/// Check that \p value matches the type flags, throw TypeError if not.
///
/// \code
///   HermesBuiltin.checkedTypeCast = function(value, typeFlags) {...}
/// \endcode
CallResult<HermesValue> hermesBuiltinCheckedTypeCast(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  HermesValue value = args.getArg(0);
  uint16_t flags = static_cast<uint16_t>(args.getArg(1).getNumber());
  if (LLVM_LIKELY(matchTypeOfIs(value, TypeOfIsTypes(flags))))
    return value;
  return runtime.raiseTypeError("Checked cast failed");
}

/// Throw a reference error with the argument as a message.
///
/// \code
///   HermesBuiltin.throwReferenceError = function(errorMessage) {...}
/// \endcode
CallResult<HermesValue> hermesBuiltinThrowReferenceError(
    void *,
    Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  return runtime.raiseReferenceError(args.getArgHandle(0));
}

/// Wasm trap: throws a RuntimeError (currently a generic Error).
/// Called by Wasm-generated IR for the `unreachable` instruction.
/// Takes no meaningful arguments — the trap message is fixed.
CallResult<HermesValue> wasmTrap(void *, Runtime &runtime) {
  return runtime.raiseError("unreachable executed");
}

/// Wasm i32.div_s: signed division with trapping.
/// Traps on division by zero or INT32_MIN / -1 (overflow).
CallResult<HermesValue> wasmI32DivS(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  int32_t a = truncateToInt32(args.getArg(0).getNumber());
  int32_t b = truncateToInt32(args.getArg(1).getNumber());
  if (LLVM_UNLIKELY(b == 0))
    return runtime.raiseError("integer divide by zero");
  if (LLVM_UNLIKELY(a == INT32_MIN && b == -1))
    return runtime.raiseError("integer overflow");
  return HermesValue::encodeTrustedNumberValue(a / b);
}

/// Wasm i32.div_u: unsigned division with trapping.
/// Traps on division by zero.
CallResult<HermesValue> wasmI32DivU(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  uint32_t a = static_cast<uint32_t>(truncateToInt32(args.getArg(0).getNumber()));
  uint32_t b = static_cast<uint32_t>(truncateToInt32(args.getArg(1).getNumber()));
  if (LLVM_UNLIKELY(b == 0))
    return runtime.raiseError("integer divide by zero");
  return HermesValue::encodeTrustedNumberValue(a / b);
}

/// Wasm i32.rem_s: signed remainder with trapping.
/// Traps on division by zero. INT32_MIN % -1 = 0 (not a trap).
CallResult<HermesValue> wasmI32RemS(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  int32_t a = truncateToInt32(args.getArg(0).getNumber());
  int32_t b = truncateToInt32(args.getArg(1).getNumber());
  if (LLVM_UNLIKELY(b == 0))
    return runtime.raiseError("integer divide by zero");
  // Special case: INT32_MIN % -1 = 0.
  // Must handle explicitly because on x86 the idiv instruction traps on
  // INT32_MIN / -1 (which is computed together with the remainder).
  if (LLVM_UNLIKELY(a == INT32_MIN && b == -1))
    return HermesValue::encodeTrustedNumberValue(0);
  return HermesValue::encodeTrustedNumberValue(a % b);
}

/// Wasm i32.rem_u: unsigned remainder with trapping.
/// Traps on division by zero.
CallResult<HermesValue> wasmI32RemU(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  uint32_t a = static_cast<uint32_t>(truncateToInt32(args.getArg(0).getNumber()));
  uint32_t b = static_cast<uint32_t>(truncateToInt32(args.getArg(1).getNumber()));
  if (LLVM_UNLIKELY(b == 0))
    return runtime.raiseError("integer divide by zero");
  return HermesValue::encodeTrustedNumberValue(a % b);
}

/// Wasm i32.clz: count leading zeros.
CallResult<HermesValue> wasmI32Clz(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  uint32_t a =
      static_cast<uint32_t>(truncateToInt32(args.getArg(0).getNumber()));
  // __builtin_clz is undefined for 0, so handle it explicitly.
  uint32_t result = a == 0 ? 32 : __builtin_clz(a);
  return HermesValue::encodeTrustedNumberValue(result);
}

/// Wasm i32.ctz: count trailing zeros.
CallResult<HermesValue> wasmI32Ctz(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  uint32_t a =
      static_cast<uint32_t>(truncateToInt32(args.getArg(0).getNumber()));
  // __builtin_ctz is undefined for 0, so handle it explicitly.
  uint32_t result = a == 0 ? 32 : __builtin_ctz(a);
  return HermesValue::encodeTrustedNumberValue(result);
}

/// Wasm i32.popcnt: population count (number of set bits).
CallResult<HermesValue> wasmI32Popcnt(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  uint32_t a =
      static_cast<uint32_t>(truncateToInt32(args.getArg(0).getNumber()));
  return HermesValue::encodeTrustedNumberValue(__builtin_popcount(a));
}

/// Wasm i32.rotl: rotate left.
CallResult<HermesValue> wasmI32Rotl(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  uint32_t a =
      static_cast<uint32_t>(truncateToInt32(args.getArg(0).getNumber()));
  uint32_t b =
      static_cast<uint32_t>(truncateToInt32(args.getArg(1).getNumber()));
  uint32_t shift = b & 31;
  // Guard against UB: shifting uint32_t by 32 is undefined.
  uint32_t result = shift == 0 ? a : (a << shift) | (a >> (32 - shift));
  return HermesValue::encodeTrustedNumberValue(result);
}

/// Wasm i32.rotr: rotate right.
CallResult<HermesValue> wasmI32Rotr(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  uint32_t a =
      static_cast<uint32_t>(truncateToInt32(args.getArg(0).getNumber()));
  uint32_t b =
      static_cast<uint32_t>(truncateToInt32(args.getArg(1).getNumber()));
  uint32_t shift = b & 31;
  // Guard against UB: shifting uint32_t by 32 is undefined.
  uint32_t result = shift == 0 ? a : (a >> shift) | (a << (32 - shift));
  return HermesValue::encodeTrustedNumberValue(result);
}

/// Wasm i32.trunc_f64_s (also used for i32.trunc_f32_s):
/// Truncate double to signed i32, trapping on NaN or out-of-range.
CallResult<HermesValue> wasmI32TruncF64S(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  double a = args.getArg(0).getNumber();
  if (LLVM_UNLIKELY(std::isnan(a)))
    return runtime.raiseError("invalid conversion to integer");
  // Truncate toward zero.
  double t = std::trunc(a);
  // Signed i32 range: [-2147483648.0, 2147483647.0].
  if (LLVM_UNLIKELY(t < -2147483648.0 || t > 2147483647.0))
    return runtime.raiseError("integer overflow");
  return HermesValue::encodeTrustedNumberValue(static_cast<int32_t>(t));
}

/// Wasm i32.trunc_f64_u (also used for i32.trunc_f32_u):
/// Truncate double to unsigned i32, trapping on NaN or out-of-range.
CallResult<HermesValue> wasmI32TruncF64U(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  double a = args.getArg(0).getNumber();
  if (LLVM_UNLIKELY(std::isnan(a)))
    return runtime.raiseError("invalid conversion to integer");
  // Truncate toward zero.
  double t = std::trunc(a);
  // Unsigned i32 range: [0.0, 4294967295.0].
  if (LLVM_UNLIKELY(t < 0.0 || t > 4294967295.0))
    return runtime.raiseError("integer overflow");
  return HermesValue::encodeTrustedNumberValue(
      static_cast<double>(static_cast<uint32_t>(t)));
}

/// Wasm i32.trunc_sat_f64_s (also used for i32.trunc_sat_f32_s):
/// Saturating truncation to signed i32. NaN -> 0.
CallResult<HermesValue> wasmI32TruncSatF64S(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  double a = args.getArg(0).getNumber();
  if (LLVM_UNLIKELY(std::isnan(a)))
    return HermesValue::encodeTrustedNumberValue(0);
  double t = std::trunc(a);
  if (t < -2147483648.0)
    return HermesValue::encodeTrustedNumberValue(
        static_cast<double>(INT32_MIN));
  if (t > 2147483647.0)
    return HermesValue::encodeTrustedNumberValue(
        static_cast<double>(INT32_MAX));
  return HermesValue::encodeTrustedNumberValue(static_cast<int32_t>(t));
}

/// Wasm i32.trunc_sat_f64_u (also used for i32.trunc_sat_f32_u):
/// Saturating truncation to unsigned i32. NaN -> 0.
CallResult<HermesValue> wasmI32TruncSatF64U(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  double a = args.getArg(0).getNumber();
  if (LLVM_UNLIKELY(std::isnan(a)))
    return HermesValue::encodeTrustedNumberValue(0);
  double t = std::trunc(a);
  if (t < 0.0)
    return HermesValue::encodeTrustedNumberValue(0);
  if (t > 4294967295.0)
    return HermesValue::encodeTrustedNumberValue(
        static_cast<double>(UINT32_MAX));
  return HermesValue::encodeTrustedNumberValue(
      static_cast<double>(static_cast<uint32_t>(t)));
}

/// Wasm i32.reinterpret_f32: bitcast f32 to i32.
/// The input is a double representing an f32 value. We narrow to float,
/// then reinterpret the bits as int32.
CallResult<HermesValue> wasmI32ReinterpretF32(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  float f = static_cast<float>(args.getArg(0).getNumber());
  int32_t bits;
  memcpy(&bits, &f, sizeof(bits));
  return HermesValue::encodeTrustedNumberValue(bits);
}

/// Wasm f32.reinterpret_i32: bitcast i32 to f32.
/// The input is a double representing an i32 value. We truncate to int32,
/// reinterpret as float, then promote back to double.
CallResult<HermesValue> wasmF32ReinterpretI32(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  int32_t bits = truncateToInt32(args.getArg(0).getNumber());
  float f;
  memcpy(&f, &bits, sizeof(f));
  // Use encodeUntrustedNumberValue because reinterpreted bits can produce
  // NaN values whose double representation collides with NaN-boxing tags.
  return HermesValue::encodeUntrustedNumberValue(static_cast<double>(f));
}

/// Wasm f64.copysign(a, b): copy the sign bit of b onto the magnitude of a.
CallResult<HermesValue> wasmF64Copysign(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  double a = args.getArg(0).getNumber();
  double b = args.getArg(1).getNumber();
  return HermesValue::encodeTrustedNumberValue(std::copysign(a, b));
}

/// Wasm f32.copysign(a, b): copy the sign bit of b onto the magnitude of a.
/// In Phase 1, all values are doubles. We narrow to float for the copysign
/// operation, then promote back to double.
CallResult<HermesValue> wasmF32Copysign(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  float a = static_cast<float>(args.getArg(0).getNumber());
  float b = static_cast<float>(args.getArg(1).getNumber());
  return HermesValue::encodeTrustedNumberValue(
      static_cast<double>(std::copysign(a, b)));
}

/// Wasm f64.nearest / f32.nearest: IEEE 754 round-ties-to-even.
CallResult<HermesValue> wasmNearest(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  double val = args.getArg(0).getNumber();
  return HermesValue::encodeTrustedNumberValue(std::nearbyint(val));
}

// ===== i64 split-pair helpers (G.3) =====
//
// Phase 1 represents i64 values as two i32 halves (lo, hi). Arithmetic
// helpers take (retBufI, lo_a, hi_a, lo_b, hi_b) and write the lo/hi
// result to retBufI[0]/retBufI[1], returning 0.

/// Helper to reconstruct a 64-bit value from split lo/hi args.
static int64_t argsToI64(NativeArgs &args, int loIdx, int hiIdx) {
  auto lo = static_cast<uint32_t>(truncateToInt32(args.getArg(loIdx).getNumber()));
  auto hi = static_cast<uint32_t>(truncateToInt32(args.getArg(hiIdx).getNumber()));
  return static_cast<int64_t>(
      (static_cast<uint64_t>(hi) << 32) | static_cast<uint64_t>(lo));
}

/// The Wasm builtins receive their linear-memory view and i64 return buffer
/// as arguments the compiler emits, but those objects are constructed in
/// generated IR through the replaceable globals \c globalThis.Uint32Array /
/// \c ArrayBuffer. Script can override those, so arg0 is untrusted and must
/// not be cast with \c vmcast, which only asserts. \p minByteLength is the
/// number of bytes the caller is about to touch unconditionally (0 to skip).
/// \return the attached view, or nullptr after raising a TypeError.
static JSTypedArrayBase *wasmTypedArrayArg(
    Runtime &runtime,
    HermesValue v,
    uint32_t minByteLength,
    const char *msg) {
  auto *arr = dyn_vmcast<JSTypedArrayBase>(v);
  if (LLVM_UNLIKELY(!arr || !arr->attached(runtime) ||
                    arr->getByteLength() < minByteLength)) {
    runtime.raiseTypeError(msg);
    return nullptr;
  }
  return arr;
}

/// Helper to write i64 result (lo32/hi32) to the return buffer (a Uint32Array).
/// retBuf is arg(0), a JSTypedArrayBase. Writes lo to [0], hi to [1].
static CallResult<HermesValue> writeI64ToRetBuf(
    Runtime &runtime,
    NativeArgs &args,
    int64_t val) {
  auto *retBuf = wasmTypedArrayArg(
      runtime, args.getArg(0), 8, "Wasm i64 return buffer is not a typed array");
  if (LLVM_UNLIKELY(!retBuf))
    return ExecutionStatus::EXCEPTION;
  auto *buf = reinterpret_cast<uint32_t *>(retBuf->data(runtime));
  buf[0] = static_cast<uint32_t>(static_cast<uint64_t>(val) & 0xFFFFFFFF);
  buf[1] = static_cast<uint32_t>(
      (static_cast<uint64_t>(val) >> 32) & 0xFFFFFFFF);
  return HermesValue::encodeTrustedNumberValue(0);
}

/// Helper to write unsigned i64 result to return buffer.
static CallResult<HermesValue> writeU64ToRetBuf(
    Runtime &runtime,
    NativeArgs &args,
    uint64_t val) {
  auto *retBuf = wasmTypedArrayArg(
      runtime, args.getArg(0), 8, "Wasm i64 return buffer is not a typed array");
  if (LLVM_UNLIKELY(!retBuf))
    return ExecutionStatus::EXCEPTION;
  auto *buf = reinterpret_cast<uint32_t *>(retBuf->data(runtime));
  buf[0] = static_cast<uint32_t>(val & 0xFFFFFFFF);
  buf[1] = static_cast<uint32_t>((val >> 32) & 0xFFFFFFFF);
  return HermesValue::encodeTrustedNumberValue(0);
}

/// wasmBigIntToI64(retBufI, bigintVal): Takes a BigInt, extracts the i64
/// value. Writes lo32/hi32 to retBufI[0]/[1], returns 0.
CallResult<HermesValue> wasmBigIntToI64(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  auto val = args.getArg(1);
  if (LLVM_UNLIKELY(!val.isBigInt()))
    return runtime.raiseTypeError("i64 argument must be a BigInt");
  uint64_t bits = val.getBigInt()->truncateToSingleDigit();
  return writeI64ToRetBuf(runtime, args, static_cast<int64_t>(bits));
}

/// wasmI64ToBigInt(lo, hi): Takes lo32/hi32 as Numbers, returns a BigInt.
CallResult<HermesValue> wasmI64ToBigInt(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  int64_t val = argsToI64(args, 0, 1);
  return BigIntPrimitive::fromSigned(runtime, val);
}

/// i64.add
CallResult<HermesValue> wasmI64Add(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  int64_t a = argsToI64(args, 1, 2);
  int64_t b = argsToI64(args, 3, 4);
  return writeI64ToRetBuf(runtime, args, a + b);
}

/// i64.sub
CallResult<HermesValue> wasmI64Sub(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  int64_t a = argsToI64(args, 1, 2);
  int64_t b = argsToI64(args, 3, 4);
  return writeI64ToRetBuf(runtime, args, a - b);
}

/// i64.mul
CallResult<HermesValue> wasmI64Mul(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  int64_t a = argsToI64(args, 1, 2);
  int64_t b = argsToI64(args, 3, 4);
  return writeI64ToRetBuf(runtime, args, a * b);
}

/// i64.div_s: signed division, traps on div by zero and INT64_MIN / -1.
CallResult<HermesValue> wasmI64DivS(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  int64_t a = argsToI64(args, 1, 2);
  int64_t b = argsToI64(args, 3, 4);
  if (LLVM_UNLIKELY(b == 0))
    return runtime.raiseError("integer divide by zero");
  if (LLVM_UNLIKELY(a == INT64_MIN && b == -1))
    return runtime.raiseError("integer overflow");
  return writeI64ToRetBuf(runtime, args, a / b);
}

/// i64.div_u: unsigned division, traps on div by zero.
CallResult<HermesValue> wasmI64DivU(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  uint64_t a = static_cast<uint64_t>(argsToI64(args, 1, 2));
  uint64_t b = static_cast<uint64_t>(argsToI64(args, 3, 4));
  if (LLVM_UNLIKELY(b == 0))
    return runtime.raiseError("integer divide by zero");
  return writeU64ToRetBuf(runtime, args, a / b);
}

/// i64.rem_s: signed remainder, traps on div by zero.
/// INT64_MIN % -1 = 0 (not a trap).
CallResult<HermesValue> wasmI64RemS(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  int64_t a = argsToI64(args, 1, 2);
  int64_t b = argsToI64(args, 3, 4);
  if (LLVM_UNLIKELY(b == 0))
    return runtime.raiseError("integer divide by zero");
  // INT64_MIN % -1 is 0. Must handle explicitly to avoid potential UB
  // on platforms where the division traps (x86 idiv).
  if (LLVM_UNLIKELY(a == INT64_MIN && b == -1))
    return writeI64ToRetBuf(runtime, args, 0);
  return writeI64ToRetBuf(runtime, args, a % b);
}

/// i64.rem_u: unsigned remainder, traps on div by zero.
CallResult<HermesValue> wasmI64RemU(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  uint64_t a = static_cast<uint64_t>(argsToI64(args, 1, 2));
  uint64_t b = static_cast<uint64_t>(argsToI64(args, 3, 4));
  if (LLVM_UNLIKELY(b == 0))
    return runtime.raiseError("integer divide by zero");
  return writeU64ToRetBuf(runtime, args, a % b);
}

/// i64.shl
CallResult<HermesValue> wasmI64Shl(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  uint64_t a = static_cast<uint64_t>(argsToI64(args, 1, 2));
  uint64_t b = static_cast<uint64_t>(argsToI64(args, 3, 4));
  return writeU64ToRetBuf(runtime, args, a << (b & 63));
}

/// i64.shr_s (arithmetic shift right)
CallResult<HermesValue> wasmI64ShrS(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  int64_t a = argsToI64(args, 1, 2);
  uint64_t b = static_cast<uint64_t>(argsToI64(args, 3, 4));
  // C++ arithmetic right shift on signed is implementation-defined but
  // in practice always sign-extends on two's complement platforms.
  return writeI64ToRetBuf(runtime, args, a >> (b & 63));
}

/// i64.shr_u (logical shift right)
CallResult<HermesValue> wasmI64ShrU(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  uint64_t a = static_cast<uint64_t>(argsToI64(args, 1, 2));
  uint64_t b = static_cast<uint64_t>(argsToI64(args, 3, 4));
  return writeU64ToRetBuf(runtime, args, a >> (b & 63));
}

/// i64.rotl
CallResult<HermesValue> wasmI64Rotl(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  uint64_t a = static_cast<uint64_t>(argsToI64(args, 1, 2));
  uint64_t b = static_cast<uint64_t>(argsToI64(args, 3, 4));
  uint64_t shift = b & 63;
  uint64_t result = shift == 0 ? a : (a << shift) | (a >> (64 - shift));
  return writeU64ToRetBuf(runtime, args, result);
}

/// i64.rotr
CallResult<HermesValue> wasmI64Rotr(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  uint64_t a = static_cast<uint64_t>(argsToI64(args, 1, 2));
  uint64_t b = static_cast<uint64_t>(argsToI64(args, 3, 4));
  uint64_t shift = b & 63;
  uint64_t result = shift == 0 ? a : (a >> shift) | (a << (64 - shift));
  return writeU64ToRetBuf(runtime, args, result);
}

/// i64.clz: count leading zeros. Result fits in [0, 64].
CallResult<HermesValue> wasmI64Clz(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  uint64_t a = static_cast<uint64_t>(argsToI64(args, 0, 1));
  uint64_t result;
  if (a == 0) {
    result = 64;
  } else {
    // __builtin_clzll is defined for non-zero values.
    result = __builtin_clzll(a);
  }
  return HermesValue::encodeTrustedNumberValue(static_cast<double>(result));
}

/// i64.ctz: count trailing zeros. Result fits in [0, 64].
CallResult<HermesValue> wasmI64Ctz(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  uint64_t a = static_cast<uint64_t>(argsToI64(args, 0, 1));
  uint64_t result;
  if (a == 0) {
    result = 64;
  } else {
    result = __builtin_ctzll(a);
  }
  return HermesValue::encodeTrustedNumberValue(static_cast<double>(result));
}

/// i64.popcnt: population count. Result fits in [0, 64].
CallResult<HermesValue> wasmI64Popcnt(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  uint64_t a = static_cast<uint64_t>(argsToI64(args, 0, 1));
  return HermesValue::encodeTrustedNumberValue(
      static_cast<double>(__builtin_popcountll(a)));
}

/// i64.trunc_f64_s (also used for i64.trunc_f32_s):
/// Truncate double to signed i64, trapping on NaN or out-of-range.
/// Writes lo/hi to retBufI[0]/[1], returns 0.
CallResult<HermesValue> wasmI64TruncF64S(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  double a = args.getArg(1).getNumber();
  if (LLVM_UNLIKELY(std::isnan(a)))
    return runtime.raiseError("invalid conversion to integer");
  double t = std::trunc(a);
  // Signed i64 range: [-9223372036854775808.0, 9223372036854775807.0].
  // Note: 9223372036854775807.0 is not exactly representable as double;
  // the closest double is 9223372036854775808.0 (2^63). So we check < 2^63.
  if (LLVM_UNLIKELY(t < -9223372036854775808.0 || t >= 9223372036854775808.0))
    return runtime.raiseError("integer overflow");
  return writeI64ToRetBuf(runtime, args, static_cast<int64_t>(t));
}

/// i64.trunc_f64_u (also used for i64.trunc_f32_u):
/// Truncate double to unsigned i64, trapping on NaN or out-of-range.
/// Writes lo/hi to retBufI[0]/[1], returns 0.
CallResult<HermesValue> wasmI64TruncF64U(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  double a = args.getArg(1).getNumber();
  if (LLVM_UNLIKELY(std::isnan(a)))
    return runtime.raiseError("invalid conversion to integer");
  double t = std::trunc(a);
  // Unsigned i64 range: [0.0, 18446744073709551615.0].
  // 18446744073709551615.0 is not exactly representable; closest double is
  // 18446744073709551616.0 (2^64). So we check < 2^64.
  if (LLVM_UNLIKELY(t < 0.0 || t >= 18446744073709551616.0))
    return runtime.raiseError("integer overflow");
  return writeU64ToRetBuf(runtime, args, static_cast<uint64_t>(t));
}

/// i64.trunc_sat_f64_s (also used for i64.trunc_sat_f32_s):
/// Saturating truncation to signed i64. NaN -> 0.
/// Writes lo/hi to retBufI[0]/[1], returns 0.
CallResult<HermesValue> wasmI64TruncSatF64S(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  double a = args.getArg(1).getNumber();
  if (LLVM_UNLIKELY(std::isnan(a)))
    return writeI64ToRetBuf(runtime, args, 0);
  double t = std::trunc(a);
  if (t < -9223372036854775808.0)
    return writeI64ToRetBuf(runtime, args, INT64_MIN);
  if (t >= 9223372036854775808.0)
    return writeI64ToRetBuf(runtime, args, INT64_MAX);
  return writeI64ToRetBuf(runtime, args, static_cast<int64_t>(t));
}

/// i64.trunc_sat_f64_u (also used for i64.trunc_sat_f32_u):
/// Saturating truncation to unsigned i64. NaN -> 0.
/// Writes lo/hi to retBufI[0]/[1], returns 0.
CallResult<HermesValue> wasmI64TruncSatF64U(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  double a = args.getArg(1).getNumber();
  if (LLVM_UNLIKELY(std::isnan(a)))
    return writeU64ToRetBuf(runtime, args, 0);
  double t = std::trunc(a);
  if (t < 0.0)
    return writeU64ToRetBuf(runtime, args, 0);
  if (t >= 18446744073709551616.0)
    return writeU64ToRetBuf(runtime, args, UINT64_MAX);
  return writeU64ToRetBuf(runtime, args, static_cast<uint64_t>(t));
}

/// f64.convert_i64_s: convert signed i64 to f64.
/// Takes split lo/hi args, returns a double.
CallResult<HermesValue> wasmF64ConvertI64S(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  int64_t a = argsToI64(args, 0, 1);
  return HermesValue::encodeTrustedNumberValue(static_cast<double>(a));
}

/// f64.convert_i64_u: convert unsigned i64 to f64.
/// Takes split lo/hi args, returns a double.
CallResult<HermesValue> wasmF64ConvertI64U(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  uint64_t a = static_cast<uint64_t>(argsToI64(args, 0, 1));
  return HermesValue::encodeTrustedNumberValue(static_cast<double>(a));
}

/// f32.convert_i64_s: convert signed i64 to f32.
/// Takes split lo/hi args, returns a double (narrowed to float then widened).
CallResult<HermesValue> wasmF32ConvertI64S(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  int64_t a = argsToI64(args, 0, 1);
  float result = static_cast<float>(a);
  return HermesValue::encodeTrustedNumberValue(static_cast<double>(result));
}

/// f32.convert_i64_u: convert unsigned i64 to f32.
/// Takes split lo/hi args, returns a double (narrowed to float then widened).
CallResult<HermesValue> wasmF32ConvertI64U(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  uint64_t a = static_cast<uint64_t>(argsToI64(args, 0, 1));
  float result = static_cast<float>(a);
  return HermesValue::encodeTrustedNumberValue(static_cast<double>(result));
}

/// i64.reinterpret_f64: bitcast f64 to i64.
/// Takes (retBufI, f64_arg), writes lo/hi to retBufI[0]/[1], returns 0.
CallResult<HermesValue> wasmI64ReinterpretF64(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  double a = args.getArg(1).getNumber();
  uint64_t bits;
  memcpy(&bits, &a, sizeof(bits));
  return writeU64ToRetBuf(runtime, args, bits);
}

/// f64.reinterpret_i64: bitcast i64 to f64.
/// Takes split lo/hi args, returns a double.
CallResult<HermesValue> wasmF64ReinterpretI64(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  uint64_t bits = static_cast<uint64_t>(argsToI64(args, 0, 1));
  double result;
  memcpy(&result, &bits, sizeof(result));
  // Use encodeUntrustedNumberValue because reinterpreted bits can produce
  // NaN values whose double representation collides with NaN-boxing tags.
  return HermesValue::encodeUntrustedNumberValue(result);
}

/// memory.grow helper (H.2).
/// Args: (heapu8View, delta, maxPages, memObj).
/// Creates a new, larger ArrayBuffer and copies the old data into it.
/// \p memObj is the WebAssembly.Memory backing this linear memory, or
/// undefined when there is none (an imported memory). When present, the new
/// buffer is installed on it, so an exported reference to the memory sees the
/// growth instead of holding the old, smaller buffer.
/// Returns the new ArrayBuffer on success, or -1 on failure.
CallResult<HermesValue> wasmMemoryGrow(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();

  // arg0 is the HEAPU8 view (Uint8Array).
  auto *heapu8 = wasmTypedArrayArg(
      runtime, args.getArg(0), 0, "Wasm memory view is not a typed array");
  if (LLVM_UNLIKELY(!heapu8))
    return ExecutionStatus::EXCEPTION;
  auto delta =
      static_cast<uint32_t>(truncateToInt32(args.getArg(1).getNumber()));
  auto maxPages =
      static_cast<uint32_t>(truncateToInt32(args.getArg(2).getNumber()));

  // Get old buffer size.
  JSArrayBuffer *oldBuf = heapu8->getBuffer(runtime);
  uint32_t oldSize = static_cast<uint32_t>(oldBuf->size());
  uint32_t oldPages = oldSize / 65536;

  // Check for overflow and max pages.
  uint64_t newPages64 = static_cast<uint64_t>(oldPages) + delta;
  if (newPages64 > maxPages || newPages64 > 65536) {
    // 65536 pages = 4GB, the maximum Wasm memory.
    return HermesValue::encodeTrustedNumberValue(-1);
  }
  uint32_t newPages = static_cast<uint32_t>(newPages64);
  uint32_t newSize = newPages * 65536;

  // Create a new ArrayBuffer with the larger size.
  struct : public Locals {
    PinnedValue<JSArrayBuffer> newBuf;
    PinnedValue<JSArrayBuffer> oldBufHandle;
    PinnedValue<JSWebAssemblyMemory> memObj;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  lv.oldBufHandle = oldBuf;
  // Pin the Memory before the allocations below: dyn_vmcast yields a raw
  // pointer, and creating the new buffer is a safepoint.
  bool haveMemObj = false;
  if (auto *mem = dyn_vmcast<JSWebAssemblyMemory>(args.getArg(3))) {
    lv.memObj = mem;
    haveMemObj = true;
  }
  lv.newBuf = JSArrayBuffer::create(
      runtime, Handle<JSObject>::vmcast(&runtime.arrayBufferPrototype));

  // Allocate the new data block (zero-initialized for the grown portion).
  if (JSArrayBuffer::createDataBlock(runtime, lv.newBuf, newSize, true) ==
      ExecutionStatus::EXCEPTION) {
    // Allocation failed — return -1 (no trap, just failure).
    runtime.clearThrownValue();
    return HermesValue::encodeTrustedNumberValue(-1);
  }

  // Copy old data to the new buffer.
  if (oldSize > 0) {
    JSArrayBuffer::copyDataBlockBytes(
        runtime, *lv.newBuf, 0, *lv.oldBufHandle, 0, oldSize);
  }

  // Install the grown buffer on the Memory object so exported references
  // follow the growth. The module reloads its own views from the returned
  // buffer.
  if (haveMemObj)
    lv.memObj->setBuffer(runtime, *lv.newBuf);

  return lv.newBuf.getHermesValue();
}

/// Wasm table and segment arrays reach these builtins through values script
/// controls: a table import supplies __wasm_funcs__/__wasm_types__ as plain
/// properties, and the arrays are constructed via globalThis.Array. They are
/// therefore untrusted and must not be cast with vmcast, which only asserts.
/// \return the array, or nullptr after raising a TypeError.
static JSArray *
wasmArrayArg(Runtime &runtime, HermesValue v, const char *msg) {
  auto *arr = dyn_vmcast<JSArray>(v);
  if (LLVM_UNLIKELY(!arr))
    runtime.raiseTypeError(msg);
  return arr;
}

/// Wasm call_indirect helper (J.2).
/// Takes (funcsArr, typesArr, index, expectedTypeIdx).
/// Validates bounds, null/uninitialized entry, and type index.
/// Returns the closure on success, traps on failure.
CallResult<HermesValue> wasmCallIndirect(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();

  // This is the indirect-call hot path, so the arrays are cast without
  // re-checking. That is sound because wasmCheckTableArrays has already
  // validated them once, at instantiation, for every way a table's storage can
  // be produced: an imported __wasm_funcs__/__wasm_types__ pair, the fresh
  // arrays created for a JS-API table import, and the arrays created for a
  // module's own tables. Once validated the arrays live in a VariableScope
  // slot that script cannot reach, and table.grow mutates them in place rather
  // than replacing them, so the invariant holds for the life of the instance.
  // Any new way of populating tableFuncVars_/tableTypeVars_ must call
  // wasmCheckTableArrays too.
  auto *funcsArr = vmcast<JSArray>(args.getArg(0));
  auto *typesArr = vmcast<JSArray>(args.getArg(1));
  int32_t index = truncateToInt32(args.getArg(2).getNumber());
  int32_t expectedTypeIdx = truncateToInt32(args.getArg(3).getNumber());

  // Bounds check.
  uint32_t tableLen = JSArray::getLength(funcsArr, runtime);
  if (LLVM_UNLIKELY(
          index < 0 || static_cast<uint32_t>(index) >= tableLen)) {
    return runtime.raiseError(
        "call_indirect: undefined element");
  }

  // Null/uninitialized check. A never-set entry reads as empty in a sparse
  // array; a WebAssembly.Table (a JS-API table, or a defined table backed by
  // one) initializes its entries to null. Both are an uninitialized funcref
  // for call_indirect, which the spec traps as "uninitialized element".
  auto funcVal = funcsArr->at(runtime, static_cast<uint32_t>(index));
  if (LLVM_UNLIKELY(
          funcVal.isEmpty() || funcVal.unboxToHV(runtime).isNull())) {
    return runtime.raiseError(
        "call_indirect: uninitialized element");
  }

  // Type check.
  auto typeVal = typesArr->at(runtime, static_cast<uint32_t>(index));
  int32_t actualTypeIdx = typeVal.isEmpty()
      ? -1
      : truncateToInt32(typeVal.unboxToHV(runtime).getNumber());
  if (LLVM_UNLIKELY(actualTypeIdx != expectedTypeIdx)) {
    return runtime.raiseError(
        "call_indirect: type mismatch");
  }

  return funcVal.unboxToHV(runtime);
}

/// Wasm exception handling: create an exception object.
/// wasmCreateException(tagIndex, v0, v1, ...):
/// Creates a JSArray [tagIndex, v0, v1, ...] representing a Wasm exception.
/// The tag index at position 0 identifies the exception tag.
/// Payload values follow at positions 1..N.
CallResult<HermesValue> wasmCreateException(void *, Runtime &runtime) {
  struct : public Locals {
    PinnedValue<JSArray> arr;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  // Total elements = 1 (tagIndex) + payload values.
  uint32_t totalElems = args.getArgCount();

  auto arrRes = JSArray::create(runtime, totalElems, totalElems);
  if (LLVM_UNLIKELY(arrRes == ExecutionStatus::EXCEPTION))
    return ExecutionStatus::EXCEPTION;
  lv.arr = std::move(*arrRes);

  // Store tagIndex at position 0, payload values at positions 1..N.
  for (uint32_t i = 0; i < totalElems; ++i) {
    if (LLVM_UNLIKELY(
            JSArray::setElementAt(lv.arr, runtime, i, args.getArgHandle(i)) ==
            ExecutionStatus::EXCEPTION))
      return ExecutionStatus::EXCEPTION;
  }

  return lv.arr.getHermesValue();
}

/// Wasm exception handling: check if a caught value matches a tag.
/// wasmMatchException(caught, tagIndex):
/// If caught is a JSArray and caught[0] === tagIndex, returns the array.
/// Otherwise returns undefined.
CallResult<HermesValue> wasmMatchException(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  HermesValue caught = args.getArg(0);
  // The expected tag is the object identifying it, not a number: Wasm tag
  // identity is nominal, so two tags with the same signature are distinct,
  // and a module-local index means nothing in another module.
  HermesValue expectedTag = args.getArg(1);

  // Check if caught is a JSArray.
  if (!caught.isObject())
    return HermesValue::encodeUndefinedValue();
  auto *obj = vmcast_or_null<JSArray>(caught.getObject(runtime));
  if (!obj)
    return HermesValue::encodeUndefinedValue();

  // Check if element 0 matches the expected tag index.
  auto tagVal = obj->at(runtime, 0);
  if (tagVal.isEmpty())
    return HermesValue::encodeUndefinedValue();
  HermesValue tagHV = tagVal.unboxToHV(runtime);
  // Identity comparison: the same tag object, not merely an equal value.
  if (!tagHV.isObject() || !expectedTag.isObject())
    return HermesValue::encodeUndefinedValue();
  if (tagHV.getObject(runtime) != expectedTag.getObject(runtime))
    return HermesValue::encodeUndefinedValue();

  // Match! Return the array.
  return caught;
}

/// Wasm memory.fill: fill \p size bytes at \p dest with \p value.
/// Args: (heapu8, dest, value, size).
/// Traps on out-of-bounds.
CallResult<HermesValue> wasmMemoryFill(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  auto *heapu8 = wasmTypedArrayArg(
      runtime, args.getArg(0), 0, "Wasm memory view is not a typed array");
  if (LLVM_UNLIKELY(!heapu8))
    return ExecutionStatus::EXCEPTION;
  uint32_t dest =
      static_cast<uint32_t>(truncateToInt32(args.getArg(1).getNumber()));
  uint32_t value =
      static_cast<uint32_t>(truncateToInt32(args.getArg(2).getNumber()));
  uint32_t size =
      static_cast<uint32_t>(truncateToInt32(args.getArg(3).getNumber()));

  uint32_t memSize = static_cast<uint32_t>(heapu8->getLength());
  // Bounds check: dest + size must not exceed memory size.
  // Use uint64_t to avoid overflow.
  if (LLVM_UNLIKELY(
          static_cast<uint64_t>(dest) + size > memSize)) {
    return runtime.raiseError(
        "memory.fill: out of bounds memory access");
  }

  // Perform the fill.
  if (size > 0) {
    JSArrayBuffer *buf = heapu8->getBuffer(runtime);
    uint8_t *data = buf->getDataBlock();
    std::memset(data + dest, static_cast<uint8_t>(value), size);
  }

  return HermesValue::encodeUndefinedValue();
}

/// Wasm memory.copy: copy \p size bytes from \p src to \p dest.
/// Args: (heapu8, dest, src, size).
/// Traps on out-of-bounds. Handles overlapping regions correctly.
CallResult<HermesValue> wasmMemoryCopy(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  auto *heapu8 = wasmTypedArrayArg(
      runtime, args.getArg(0), 0, "Wasm memory view is not a typed array");
  if (LLVM_UNLIKELY(!heapu8))
    return ExecutionStatus::EXCEPTION;
  uint32_t dest =
      static_cast<uint32_t>(truncateToInt32(args.getArg(1).getNumber()));
  uint32_t src =
      static_cast<uint32_t>(truncateToInt32(args.getArg(2).getNumber()));
  uint32_t size =
      static_cast<uint32_t>(truncateToInt32(args.getArg(3).getNumber()));

  uint32_t memSize = static_cast<uint32_t>(heapu8->getLength());
  // Bounds check both regions.
  if (LLVM_UNLIKELY(
          static_cast<uint64_t>(src) + size > memSize ||
          static_cast<uint64_t>(dest) + size > memSize)) {
    return runtime.raiseError(
        "memory.copy: out of bounds memory access");
  }

  // Perform the copy (memmove handles overlapping regions).
  if (size > 0) {
    JSArrayBuffer *buf = heapu8->getBuffer(runtime);
    uint8_t *data = buf->getDataBlock();
    std::memmove(data + dest, data + src, size);
  }

  return HermesValue::encodeUndefinedValue();
}

/// Wasm memory.init: copy bytes from a data segment into linear memory.
/// Args: (heapu8, dataSegs, segIdx, dest, src, size).
/// dataSegs is a JSArray where each element is either a JSTypedArrayBase
/// (Uint8Array of segment data) or null/empty (segment has been dropped).
/// Traps on out-of-bounds or if the segment has been dropped (with n>0).
CallResult<HermesValue> wasmMemoryInit(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  auto *heapu8 = wasmTypedArrayArg(
      runtime, args.getArg(0), 0, "Wasm memory view is not a typed array");
  if (LLVM_UNLIKELY(!heapu8))
    return ExecutionStatus::EXCEPTION;
  auto *arr13 = wasmArrayArg(runtime, args.getArg(1),
      "Wasm data segment array is not an array");
  if (LLVM_UNLIKELY(!arr13))
    return ExecutionStatus::EXCEPTION;
  auto *dataSegs = arr13;
  uint32_t segIdx =
      static_cast<uint32_t>(truncateToInt32(args.getArg(2).getNumber()));
  uint32_t dest =
      static_cast<uint32_t>(truncateToInt32(args.getArg(3).getNumber()));
  uint32_t src =
      static_cast<uint32_t>(truncateToInt32(args.getArg(4).getNumber()));
  uint32_t size =
      static_cast<uint32_t>(truncateToInt32(args.getArg(5).getNumber()));

  // Look up the data segment.
  auto segVal = dataSegs->at(runtime, segIdx);
  bool dropped = segVal.isEmpty() ||
      segVal.unboxToHV(runtime).isNull();

  // If size is 0, check bounds but don't fail on dropped segments.
  // Per spec: memory.init with n=0 succeeds even for dropped segments,
  // as long as s <= segLen and d <= memLen.
  uint32_t segLen = 0;
  JSTypedArrayBase *segArr = nullptr;
  if (!dropped) {
    segArr = wasmTypedArrayArg(
        runtime,
        segVal.unboxToHV(runtime),
        0,
        "Wasm data segment is not a typed array");
    if (LLVM_UNLIKELY(!segArr))
      return ExecutionStatus::EXCEPTION;
    segLen = static_cast<uint32_t>(segArr->getLength());
  }

  // Bounds check against data segment.
  if (LLVM_UNLIKELY(static_cast<uint64_t>(src) + size > segLen)) {
    return runtime.raiseError(
        "memory.init: out of bounds data segment access");
  }

  // Bounds check against linear memory.
  uint32_t memSize = static_cast<uint32_t>(heapu8->getLength());
  if (LLVM_UNLIKELY(static_cast<uint64_t>(dest) + size > memSize)) {
    return runtime.raiseError(
        "memory.init: out of bounds memory access");
  }

  // Perform the copy.
  if (size > 0) {
    JSArrayBuffer *memBuf = heapu8->getBuffer(runtime);
    uint8_t *memData = memBuf->getDataBlock();
    JSArrayBuffer *segBuf = segArr->getBuffer(runtime);
    uint8_t *segData = segBuf->getDataBlock();
    std::memcpy(memData + dest, segData + src, size);
  }

  return HermesValue::encodeUndefinedValue();
}

/// Wasm data.drop: mark a data segment as dropped.
/// Args: (dataSegs, segIdx).
/// Sets the segment entry in the data segments array to null.
CallResult<HermesValue> wasmDataDrop(void *, Runtime &runtime) {
  struct : public Locals {
    PinnedValue<JSArray> dataSegs;
    PinnedValue<> nullVal;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  auto *arr12 = wasmArrayArg(runtime, args.getArg(0),
      "Wasm data segment array is not an array");
  if (LLVM_UNLIKELY(!arr12))
    return ExecutionStatus::EXCEPTION;
  lv.dataSegs = arr12;
  uint32_t segIdx =
      static_cast<uint32_t>(truncateToInt32(args.getArg(1).getNumber()));

  // Set the segment to null to mark it as dropped.
  lv.nullVal = HermesValue::encodeNullValue();
  (void)JSArray::setElementAt(lv.dataSegs, runtime, segIdx, lv.nullVal);

  return HermesValue::encodeUndefinedValue();
}

/// Wasm binary data segment init: bulk-copy from binary data storage blob
/// into linear memory (a typed array).
/// Args: (heapu8, blobOffset, length, dest).
/// Walks the stack to find the caller's RuntimeModule, then copies
/// binaryDataStorage[blobOffset..blobOffset+length] to heapu8[dest..dest+length].
CallResult<HermesValue> wasmDataSegmentInit(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  auto *heapu8 = wasmTypedArrayArg(
      runtime, args.getArg(0), 0, "Wasm memory view is not a typed array");
  if (LLVM_UNLIKELY(!heapu8))
    return ExecutionStatus::EXCEPTION;
  uint32_t blobOffset =
      static_cast<uint32_t>(truncateToInt32(args.getArg(1).getNumber()));
  uint32_t length =
      static_cast<uint32_t>(truncateToInt32(args.getArg(2).getNumber()));
  uint32_t dest =
      static_cast<uint32_t>(truncateToInt32(args.getArg(3).getNumber()));

  if (length == 0)
    return HermesValue::encodeUndefinedValue();

  // Walk stack to get caller's CodeBlock → RuntimeModule.
  auto frames = runtime.getStackFrames();
  auto it = frames.begin();
  if (LLVM_UNLIKELY(++it == frames.end()))
    return runtime.raiseTypeError("Cannot be called directly");
  auto *callerCB = it->getCalleeCodeBlock();
  if (LLVM_UNLIKELY(!callerCB))
    return runtime.raiseTypeError("Cannot be called from native code");
  RuntimeModule *runtimeModule = callerCB->getRuntimeModule();

  auto storage = runtimeModule->getBinaryDataStorage();

  // Bounds check against binary data storage.
  if (LLVM_UNLIKELY(
          static_cast<uint64_t>(blobOffset) + length > storage.size())) {
    return runtime.raiseError(
        "wasmDataSegmentInit: out of bounds binary data access");
  }

  // Bounds check against linear memory.
  uint32_t memSize = static_cast<uint32_t>(heapu8->getLength());
  if (LLVM_UNLIKELY(static_cast<uint64_t>(dest) + length > memSize)) {
    return runtime.raiseError(
        "wasmDataSegmentInit: out of bounds memory access");
  }

  // Perform the bulk copy.
  JSArrayBuffer *memBuf = heapu8->getBuffer(runtime);
  uint8_t *memData = memBuf->getDataBlock();
  std::memcpy(memData + dest, storage.data() + blobOffset, length);

  return HermesValue::encodeUndefinedValue();
}

/// Wasm table.fill: fill \p count entries at \p idx with \p val.
/// Args: (funcsArr, idx, val, count).
/// Traps on out-of-bounds.
CallResult<HermesValue> wasmTableFill(void *, Runtime &runtime) {
  struct : public Locals {
    PinnedValue<JSArray> funcsArr;
    PinnedValue<> val;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  auto *arr11 = wasmArrayArg(runtime, args.getArg(0),
      "Wasm table function array is not an array");
  if (LLVM_UNLIKELY(!arr11))
    return ExecutionStatus::EXCEPTION;
  lv.funcsArr = arr11;
  uint32_t idx =
      static_cast<uint32_t>(truncateToInt32(args.getArg(1).getNumber()));
  lv.val = args.getArg(2);
  uint32_t count =
      static_cast<uint32_t>(truncateToInt32(args.getArg(3).getNumber()));

  uint32_t tableLen = JSArray::getLength(*lv.funcsArr, runtime);
  // Bounds check: idx + count must not exceed table size.
  if (LLVM_UNLIKELY(static_cast<uint64_t>(idx) + count > tableLen)) {
    return runtime.raiseError(
        "table.fill: out of bounds table access");
  }

  // Perform the fill.
  for (uint32_t i = 0; i < count; ++i) {
    (void)JSArray::setElementAt(lv.funcsArr, runtime, idx + i, lv.val);
  }

  return HermesValue::encodeUndefinedValue();
}

/// Wasm table.grow: grow table by delta entries, filling with fillVal.
/// Args: (funcsArr, typesArr, delta, fillVal, maxEntries, actualMax).
/// Returns old size on success, -1 on failure.
CallResult<HermesValue> wasmTableGrow(void *, Runtime &runtime) {
  struct : public Locals {
    PinnedValue<JSArray> funcsArr;
    PinnedValue<JSArray> typesArr;
    PinnedValue<> fillVal;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  auto *arr10 = wasmArrayArg(runtime, args.getArg(0),
      "Wasm table function array is not an array");
  if (LLVM_UNLIKELY(!arr10))
    return ExecutionStatus::EXCEPTION;
  lv.funcsArr = arr10;
  auto *arr9 = wasmArrayArg(runtime, args.getArg(1),
      "Wasm table type array is not an array");
  if (LLVM_UNLIKELY(!arr9))
    return ExecutionStatus::EXCEPTION;
  lv.typesArr = arr9;
  uint32_t delta =
      static_cast<uint32_t>(truncateToInt32(args.getArg(2).getNumber()));
  lv.fillVal = args.getArg(3);
  uint32_t maxEntries =
      static_cast<uint32_t>(truncateToInt32(args.getArg(4).getNumber()));
  // An imported table's own maximum binds too. Link validation only checks
  // that the supplied table's max is <= the declared one, so growing to the
  // declared max would take a shared table past what its owner permits.
  // -1 means the supplied table declares no maximum. Guard on isNumber:
  // this arrives from the table object's metadata, which script can set.
  HermesValue actualMaxVal = args.getArg(5);
  if (actualMaxVal.isNumber()) {
    double actualMaxNum = actualMaxVal.getNumber();
    if (actualMaxNum >= 0) {
      auto actualMax =
          static_cast<uint32_t>(truncateToInt32(actualMaxNum));
      if (actualMax < maxEntries)
        maxEntries = actualMax;
    }
  }

  uint32_t oldLen = JSArray::getLength(*lv.funcsArr, runtime);

  // Largest table this engine will grow to. The spec permits up to 2^32-1
  // entries and requires table.grow to answer -1 when it cannot allocate.
  // maxEntries is UINT32_MAX when the table declares no maximum, so without
  // a limit of our own an enormous delta is not refused but attempted, and
  // the fill loop below runs for billions of iterations, growing indexed
  // storage each time, before any allocation actually fails.
  static constexpr uint64_t kMaxTableEntries = 10'000'000;

  // Check for overflow and max limit.
  uint64_t newLen64 = static_cast<uint64_t>(oldLen) + delta;
  if (newLen64 > maxEntries || newLen64 > kMaxTableEntries) {
    return HermesValue::encodeTrustedNumberValue(-1);
  }
  uint32_t newLen = static_cast<uint32_t>(newLen64);

  // Grow both arrays by setting their length.
  auto res1 = JSArray::setLengthProperty(lv.funcsArr, runtime, newLen);
  if (LLVM_UNLIKELY(res1 == ExecutionStatus::EXCEPTION))
    return ExecutionStatus::EXCEPTION;
  auto res2 = JSArray::setLengthProperty(lv.typesArr, runtime, newLen);
  if (LLVM_UNLIKELY(res2 == ExecutionStatus::EXCEPTION))
    return ExecutionStatus::EXCEPTION;

  // Fill new func entries with fillVal. Type entries remain undefined
  // (no type info for fill entries — they are not callable via
  // call_indirect).
  for (uint32_t i = oldLen; i < newLen; ++i) {
    if (LLVM_UNLIKELY(
            JSArray::setElementAt(lv.funcsArr, runtime, i, lv.fillVal) ==
            ExecutionStatus::EXCEPTION)) {
      // Out of memory part-way through. Put the table back and answer -1,
      // which is how table.grow reports that it could not allocate.
      // Returning normally with an exception pending, as this loop used to,
      // leaves the caller to trip over it somewhere else entirely.
      runtime.clearThrownValue();
      (void)JSArray::setLengthProperty(lv.funcsArr, runtime, oldLen);
      (void)JSArray::setLengthProperty(lv.typesArr, runtime, oldLen);
      return HermesValue::encodeTrustedNumberValue(-1);
    }
  }

  return HermesValue::encodeTrustedNumberValue(oldLen);
}

/// Wasm table.copy: copy \p count entries from src table to dst table.
/// Args: (dstFuncs, srcFuncs, dstTypes, srcTypes, dst, src, count).
/// Traps on out-of-bounds. Handles overlapping regions correctly.
CallResult<HermesValue> wasmTableCopy(void *, Runtime &runtime) {
  struct : public Locals {
    PinnedValue<JSArray> dstFuncs;
    PinnedValue<JSArray> srcFuncs;
    PinnedValue<JSArray> dstTypes;
    PinnedValue<JSArray> srcTypes;
    PinnedValue<> tmpVal;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  auto *arr8 = wasmArrayArg(runtime, args.getArg(0),
      "Wasm table function array is not an array");
  if (LLVM_UNLIKELY(!arr8))
    return ExecutionStatus::EXCEPTION;
  lv.dstFuncs = arr8;
  auto *arr7 = wasmArrayArg(runtime, args.getArg(1),
      "Wasm table function array is not an array");
  if (LLVM_UNLIKELY(!arr7))
    return ExecutionStatus::EXCEPTION;
  lv.srcFuncs = arr7;
  auto *arr6 = wasmArrayArg(runtime, args.getArg(2),
      "Wasm table type array is not an array");
  if (LLVM_UNLIKELY(!arr6))
    return ExecutionStatus::EXCEPTION;
  lv.dstTypes = arr6;
  auto *arr5 = wasmArrayArg(runtime, args.getArg(3),
      "Wasm table type array is not an array");
  if (LLVM_UNLIKELY(!arr5))
    return ExecutionStatus::EXCEPTION;
  lv.srcTypes = arr5;
  uint32_t dst =
      static_cast<uint32_t>(truncateToInt32(args.getArg(4).getNumber()));
  uint32_t src =
      static_cast<uint32_t>(truncateToInt32(args.getArg(5).getNumber()));
  uint32_t count =
      static_cast<uint32_t>(truncateToInt32(args.getArg(6).getNumber()));

  uint32_t dstLen = JSArray::getLength(*lv.dstFuncs, runtime);
  uint32_t srcLen = JSArray::getLength(*lv.srcFuncs, runtime);

  // Bounds check both regions.
  if (LLVM_UNLIKELY(
          static_cast<uint64_t>(dst) + count > dstLen ||
          static_cast<uint64_t>(src) + count > srcLen)) {
    return runtime.raiseError(
        "table.copy: out of bounds table access");
  }

  if (count == 0)
    return HermesValue::encodeUndefinedValue();

  // Handle overlapping copy correctly (like memmove).
  // If dst <= src or different tables, copy forward; otherwise copy backward.
  bool sameTable = lv.dstFuncs.getHermesValue().getRaw() ==
      lv.srcFuncs.getHermesValue().getRaw();
  if (!sameTable || dst <= src) {
    for (uint32_t i = 0; i < count; ++i) {
      auto funcVal = lv.srcFuncs->at(runtime, src + i);
      lv.tmpVal = funcVal.isEmpty() ? HermesValue::encodeEmptyValue()
                                    : funcVal.unboxToHV(runtime);
      (void)JSArray::setElementAt(lv.dstFuncs, runtime, dst + i, lv.tmpVal);

      auto typeVal = lv.srcTypes->at(runtime, src + i);
      lv.tmpVal = typeVal.isEmpty() ? HermesValue::encodeEmptyValue()
                                    : typeVal.unboxToHV(runtime);
      (void)JSArray::setElementAt(lv.dstTypes, runtime, dst + i, lv.tmpVal);
    }
  } else {
    // Copy backward for overlapping same-table copy where dst > src.
    for (uint32_t i = count; i > 0; --i) {
      auto funcVal = lv.srcFuncs->at(runtime, src + i - 1);
      lv.tmpVal = funcVal.isEmpty() ? HermesValue::encodeEmptyValue()
                                    : funcVal.unboxToHV(runtime);
      (void)JSArray::setElementAt(
          lv.dstFuncs, runtime, dst + i - 1, lv.tmpVal);

      auto typeVal = lv.srcTypes->at(runtime, src + i - 1);
      lv.tmpVal = typeVal.isEmpty() ? HermesValue::encodeEmptyValue()
                                    : typeVal.unboxToHV(runtime);
      (void)JSArray::setElementAt(
          lv.dstTypes, runtime, dst + i - 1, lv.tmpVal);
    }
  }

  return HermesValue::encodeUndefinedValue();
}

/// Wasm table.init: copy entries from element segment into a table.
/// Args: (funcsArr, typesArr, elemSegs, segIdx, dst, src, count).
/// elemSegs is a JSArray where each element is either a JSArray of
/// interleaved [func0, typeIdx0, func1, typeIdx1, ...] or null (dropped).
/// Traps on out-of-bounds or if the segment has been dropped (with n>0).
CallResult<HermesValue> wasmTableInit(void *, Runtime &runtime) {
  struct : public Locals {
    PinnedValue<JSArray> funcsArr;
    PinnedValue<JSArray> typesArr;
    PinnedValue<JSArray> elemSegs;
    PinnedValue<> tmpVal;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  auto *arr4 = wasmArrayArg(runtime, args.getArg(0),
      "Wasm table function array is not an array");
  if (LLVM_UNLIKELY(!arr4))
    return ExecutionStatus::EXCEPTION;
  lv.funcsArr = arr4;
  auto *arr3 = wasmArrayArg(runtime, args.getArg(1),
      "Wasm table type array is not an array");
  if (LLVM_UNLIKELY(!arr3))
    return ExecutionStatus::EXCEPTION;
  lv.typesArr = arr3;
  auto *arr2 = wasmArrayArg(runtime, args.getArg(2),
      "Wasm element segment array is not an array");
  if (LLVM_UNLIKELY(!arr2))
    return ExecutionStatus::EXCEPTION;
  lv.elemSegs = arr2;
  uint32_t segIdx =
      static_cast<uint32_t>(truncateToInt32(args.getArg(3).getNumber()));
  uint32_t dst =
      static_cast<uint32_t>(truncateToInt32(args.getArg(4).getNumber()));
  uint32_t src =
      static_cast<uint32_t>(truncateToInt32(args.getArg(5).getNumber()));
  uint32_t count =
      static_cast<uint32_t>(truncateToInt32(args.getArg(6).getNumber()));

  // Look up the element segment.
  auto segVal = lv.elemSegs->at(runtime, segIdx);
  bool dropped = segVal.isEmpty() || segVal.unboxToHV(runtime).isNull();

  // Segment length = number of entries (pairs / 2).
  uint32_t segLen = 0;
  JSArray *segArr = nullptr;
  if (!dropped) {
    // The segment entry is reachable from script-controlled state, so use a
    // checked cast: dyn_vmcast also tolerates a non-pointer value, which
    // getObject() would assert on.
    segArr = wasmArrayArg(
        runtime,
        segVal.unboxToHV(runtime),
        "Wasm element segment entry is not an array");
    if (LLVM_UNLIKELY(!segArr))
      return ExecutionStatus::EXCEPTION;
    // Each element has 2 slots (func, typeIdx), so entries = length / 2.
    segLen = JSArray::getLength(segArr, runtime) / 2;
  }

  // Bounds check against element segment.
  if (LLVM_UNLIKELY(static_cast<uint64_t>(src) + count > segLen)) {
    return runtime.raiseError(
        "table.init: out of bounds element segment access");
  }

  // Bounds check against table.
  uint32_t tableLen = JSArray::getLength(*lv.funcsArr, runtime);
  if (LLVM_UNLIKELY(static_cast<uint64_t>(dst) + count > tableLen)) {
    return runtime.raiseError(
        "table.init: out of bounds table access");
  }

  // Copy entries from segment to table.
  for (uint32_t i = 0; i < count; ++i) {
    // Read func and typeIdx from interleaved segment array.
    auto funcVal = segArr->at(runtime, (src + i) * 2);
    lv.tmpVal = funcVal.isEmpty() ? HermesValue::encodeEmptyValue()
                                  : funcVal.unboxToHV(runtime);
    (void)JSArray::setElementAt(lv.funcsArr, runtime, dst + i, lv.tmpVal);

    auto typeVal = segArr->at(runtime, (src + i) * 2 + 1);
    lv.tmpVal = typeVal.isEmpty() ? HermesValue::encodeEmptyValue()
                                  : typeVal.unboxToHV(runtime);
    (void)JSArray::setElementAt(lv.typesArr, runtime, dst + i, lv.tmpVal);
  }

  return HermesValue::encodeUndefinedValue();
}

/// Wasm elem.drop: mark an element segment as dropped.
/// Args: (elemSegs, segIdx).
/// Sets the segment entry in the element segments array to null.
CallResult<HermesValue> wasmElemDrop(void *, Runtime &runtime) {
  struct : public Locals {
    PinnedValue<JSArray> elemSegs;
    PinnedValue<> nullVal;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  auto *arr1 = wasmArrayArg(runtime, args.getArg(0),
      "Wasm element segment array is not an array");
  if (LLVM_UNLIKELY(!arr1))
    return ExecutionStatus::EXCEPTION;
  lv.elemSegs = arr1;
  uint32_t segIdx =
      static_cast<uint32_t>(truncateToInt32(args.getArg(1).getNumber()));

  // Set the segment to null to mark it as dropped.
  lv.nullVal = HermesValue::encodeNullValue();
  (void)JSArray::setElementAt(lv.elemSegs, runtime, segIdx, lv.nullVal);

  return HermesValue::encodeUndefinedValue();
}



/// Map a structural Wasm function-type string to a stable integer id.
/// call_indirect must compare type identity across module boundaries, and a
/// module-local type index cannot do that: two modules number their type
/// sections independently, so the same signature can get different indices
/// (a spurious trap) and different signatures the same index (a missed trap).
/// The identifier table already uniques strings per Runtime, so its symbol
/// index is exactly the process-wide id needed, with no extra state.
/// wasmInternType(typeString).
CallResult<HermesValue> wasmInternType(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();

  struct : public Locals {
    PinnedValue<StringPrimitive> str;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  auto *strPrim = dyn_vmcast<StringPrimitive>(args.getArg(0));
  if (LLVM_UNLIKELY(!strPrim))
    return runtime.raiseTypeError("Wasm type id must be a string");
  lv.str = strPrim;

  auto symRes = runtime.getIdentifierTable().getSymbolHandleFromPrimitive(
      runtime, createPseudoHandle(lv.str.get()));
  if (LLVM_UNLIKELY(symRes == ExecutionStatus::EXCEPTION))
    return ExecutionStatus::EXCEPTION;

  return HermesValue::encodeTrustedNumberValue(
      symRes->get().unsafeGetIndex());
}
/// \return the prototype for a Wasm LinkError. Without Wasm support the wasm*
/// Runtime fields do not exist and these builtins are unreachable, but they
/// must still compile: Builtins.def numbering is deliberately independent of
/// HERMES_ENABLE_WASM, so the builtins cannot be #ifdef'd out.
static Handle<JSObject> wasmLinkErrorProto(Runtime &runtime) {
#ifdef HERMES_ENABLE_WASM
  return Handle<JSObject>{runtime.wasmLinkErrorPrototype};
#else
  return Handle<JSObject>{runtime.ErrorPrototype};
#endif
}

/// Raise a WebAssembly.LinkError with the ASCII message \p msg.
static ExecutionStatus
raiseWasmLinkError(Runtime &runtime, const char *msg) {
  struct : public Locals {
    PinnedValue<> msgHandle;
    PinnedValue<JSError> err;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  auto strRes =
      StringPrimitive::create(runtime, ASCIIRef(msg, strlen(msg)));
  if (LLVM_UNLIKELY(strRes == ExecutionStatus::EXCEPTION))
    return ExecutionStatus::EXCEPTION;
  lv.msgHandle = *strRes;

  lv.err = JSError::create(runtime, wasmLinkErrorProto(runtime));
  if (LLVM_UNLIKELY(
          JSError::setMessage(lv.err, runtime, lv.msgHandle) ==
          ExecutionStatus::EXCEPTION))
    return ExecutionStatus::EXCEPTION;
  JSError::recordStackTrace(lv.err, runtime, true);
  return runtime.setThrownValue(lv.err.getHermesValue());
}

/// Validate that a table's backing arrays are genuine JSArrays. Called once
/// per table during instantiation, which establishes the invariant that lets
/// wasmCallIndirect -- on the indirect-call hot path -- cast them without
/// re-checking. Both the imported case (__wasm_funcs__/__wasm_types__ are
/// plain properties script controls) and the freshly-created case (the arrays
/// are built via globalThis.Array, which script can replace) go through here.
/// wasmCheckTableArrays(funcsArr, typesArr).
CallResult<HermesValue> wasmCheckTableArrays(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  if (LLVM_UNLIKELY(!dyn_vmcast<JSArray>(args.getArg(0))))
    return raiseWasmLinkError(
        runtime, "table function storage is not an array");
  if (LLVM_UNLIKELY(!dyn_vmcast<JSArray>(args.getArg(1))))
    return raiseWasmLinkError(
        runtime, "table type storage is not an array");
  return HermesValue::encodeUndefinedValue();
}

/// Wasm link error: creates and throws a WebAssembly.LinkError with the
/// given message string. Used by Wasm-generated IR for import type
/// validation at instantiation time.
/// Args: (message).
CallResult<HermesValue> wasmLinkError(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();

  struct : public Locals {
    PinnedValue<> msgHandle;
    PinnedValue<JSError> err;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  lv.msgHandle = args.getArg(0);

  lv.err = JSError::create(runtime, wasmLinkErrorProto(runtime));

  if (LLVM_UNLIKELY(
          JSError::setMessage(lv.err, runtime, lv.msgHandle) ==
          ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }

  JSError::recordStackTrace(lv.err, runtime, true);

  return runtime.setThrownValue(lv.err.getHermesValue());
}

namespace {

CallResult<HermesValue> copyDataPropertiesSlowPath_RJS(
    Runtime &runtime,
    Handle<JSObject> target,
    Handle<JSObject> from,
    Handle<JSObject> excludedItems) {
  struct : public Locals {
    PinnedValue<> nextKeyHandle;
    PinnedValue<> propValueHandle;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  // 5. Let keys be ? from.[[OwnPropertyKeys]]().
  auto cr = JSObject::getOwnPropertyKeys(
      from,
      runtime,
      OwnKeysFlags()
          .plusIncludeSymbols()
          .plusIncludeNonSymbols()
          .plusIncludeNonEnumerable());
  if (LLVM_UNLIKELY(cr == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  auto keys = *cr;

  GCScopeMarkerRAII marker{runtime};
  // 6. For each element nextKey of keys in List order, do
  for (uint32_t nextKeyIdx = 0, endIdx = keys->getEndIndex();
       nextKeyIdx < endIdx;
       ++nextKeyIdx) {
    marker.flush();
    lv.nextKeyHandle = keys->at(runtime, nextKeyIdx).unboxToHV(runtime);
    if (lv.nextKeyHandle->isNumber()) {
      CallResult<PseudoHandle<StringPrimitive>> strRes =
          toString_RJS(runtime, lv.nextKeyHandle);
      if (LLVM_UNLIKELY(strRes == ExecutionStatus::EXCEPTION)) {
        return ExecutionStatus::EXCEPTION;
      }
      lv.nextKeyHandle = strRes->getHermesValue();
    }

    // b. For each element e of excludedItems in List order, do
    //   i. If SameValue(e, nextKey) is true, then
    //     1. Set excluded to true.
    if (excludedItems) {
      assert(
          !excludedItems->isProxyObject() &&
          "internal excludedItems object is a proxy");
      ComputedPropertyDescriptor desc;
      CallResult<bool> cr = JSObject::getOwnComputedPrimitiveDescriptor(
          excludedItems,
          runtime,
          lv.nextKeyHandle,
          JSObject::IgnoreProxy::Yes,
          desc);
      if (LLVM_UNLIKELY(cr == ExecutionStatus::EXCEPTION))
        return ExecutionStatus::EXCEPTION;
      if (*cr)
        continue;
    }

    //   i. Let desc be ? from.[[GetOwnProperty]](nextKey).
    ComputedPropertyDescriptor desc;
    CallResult<bool> crb = JSObject::getOwnComputedDescriptor(
        from, runtime, lv.nextKeyHandle, desc);
    if (LLVM_UNLIKELY(crb == ExecutionStatus::EXCEPTION))
      return ExecutionStatus::EXCEPTION;
    //   ii. If desc is not undefined and desc.[[Enumerable]] is true, then
    // TODO(T141997867), move this special case behavior for host objects to
    // getOwnComputedDescriptor.
    if ((*crb && desc.flags.enumerable) || from->isHostObject()) {
      //     1. Let propValue be ? Get(from, nextKey).
      CallResult<PseudoHandle<>> crv =
          JSObject::getComputed_RJS(from, runtime, lv.nextKeyHandle);
      if (LLVM_UNLIKELY(crv == ExecutionStatus::EXCEPTION))
        return ExecutionStatus::EXCEPTION;
      lv.propValueHandle = std::move(*crv);
      //     2. Perform ! CreateDataProperty(target, nextKey, propValue).
      crb = JSObject::defineOwnComputed(
          target,
          runtime,
          lv.nextKeyHandle,
          DefinePropertyFlags::getDefaultNewPropertyFlags(),
          lv.propValueHandle);
      if (LLVM_UNLIKELY(cr == ExecutionStatus::EXCEPTION))
        return ExecutionStatus::EXCEPTION;
      assert(
          crb != ExecutionStatus::EXCEPTION && *crb &&
          "CreateDataProperty failed");
    }
  }
  return target.getHermesValue();
}

} // namespace

/// \code
///   HermesBuiltin.copyDataProperties =
///         function (target, source, excludedItems) {}
/// \endcode
///
/// Copy all enumerable own properties of object \p source, that are not also
/// properties of \p excludedItems, into \p target, which must be an object, and
/// return \p target. If \p excludedItems is not specified, it is assumed
/// to be empty.
CallResult<HermesValue> hermesBuiltinCopyDataProperties(
    void *,
    Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  struct : public Locals {
    PinnedValue<JSObject> source;
    PinnedValue<> nameHandle;
    PinnedValue<> valueHandle;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  GCScope gcScope{runtime};

  // 1. Assert: Type(target) is Object.
  Handle<JSObject> target = args.dyncastArg<JSObject>(0);
  // To be safe, ignore non-objects.
  if (!target)
    return HermesValue::encodeUndefinedValue();

  // 3. If source is undefined or null, return target.
  Handle<> untypedSource = args.getArgHandle(1);
  if (untypedSource->isNull() || untypedSource->isUndefined())
    return target.getHermesValue();

  // 4. Let from be ! ToObject(source).
  Handle<JSObject> source = untypedSource->isObject()
      ? Handle<JSObject>::vmcast(untypedSource)
      : [&]() {
          lv.source.castAndSetHermesValue<JSObject>(
              *toObject(runtime, untypedSource));
          return Handle<JSObject>{lv.source};
        }();

  // 2. Assert: excludedItems is a List of property keys.
  // In Hermes, excludedItems is represented as a JSObject, created by
  // bytecode emitted by the compiler, whose keys are the excluded
  // propertyKeys
  Handle<JSObject> excludedItems = args.dyncastArg<JSObject>(2);
  assert(
      (!excludedItems || !excludedItems->isProxyObject()) &&
      "excludedItems internal List is a Proxy");

  // We cannot use the fast path if the object is a proxy, host object, or when
  // there could potentially be an accessor defined on the object. This is
  // because in order to use JSObject::forEachOwnPropertyWhile, we must not
  // modify the underlying property map or hidden class. However, if we have an
  // accessor, we cannot guarantee that condition, so we use the slow path.
  if (source->isProxyObject() || source->isHostObject() ||
      source->getClass(runtime)->getMayHaveAccessor()) {
    return copyDataPropertiesSlowPath_RJS(
        runtime, target, source, excludedItems);
  }

  // Process all named properties/symbols.
  bool success = JSObject::forEachOwnPropertyWhile(
      source,
      runtime,
      // indexedCB.
      [&source, &target, &excludedItems, &lv](
          Runtime &runtime, uint32_t index, ComputedPropertyDescriptor desc) {
        if (!desc.flags.enumerable)
          return true;

        lv.nameHandle = HermesValue::encodeTrustedNumberValue(index);

        if (excludedItems) {
          assert(
              !excludedItems->isProxyObject() &&
              "internal excludedItems object is a proxy");
          ComputedPropertyDescriptor xdesc;
          auto cr = JSObject::getOwnComputedPrimitiveDescriptor(
              excludedItems,
              runtime,
              lv.nameHandle,
              JSObject::IgnoreProxy::Yes,
              xdesc);
          if (LLVM_UNLIKELY(cr == ExecutionStatus::EXCEPTION))
            return false;
          if (*cr)
            return true;
        }

        lv.valueHandle = JSObject::getOwnIndexed(
            createPseudoHandle(source.get()), runtime, index);

        if (LLVM_UNLIKELY(
                JSObject::defineOwnComputedPrimitive(
                    target,
                    runtime,
                    lv.nameHandle,
                    DefinePropertyFlags::getDefaultNewPropertyFlags(),
                    lv.valueHandle) == ExecutionStatus::EXCEPTION)) {
          return false;
        }

        return true;
      },
      // namedCB.
      [&source, &target, &excludedItems, &lv](
          Runtime &runtime, SymbolID sym, NamedPropertyDescriptor desc) {
        if (!desc.flags.enumerable)
          return true;
        if (InternalProperty::isInternal(sym))
          return true;

        // Skip excluded items.
        if (excludedItems) {
          auto cr = JSObject::hasNamedOrIndexed(excludedItems, runtime, sym);
          assert(
              cr != ExecutionStatus::EXCEPTION &&
              "hasNamedOrIndex failed, which can only happen with a proxy, "
              "but excludedItems should never be a proxy");
          if (*cr)
            return true;
        }

        SmallHermesValue shv =
            JSObject::getNamedSlotValueUnsafe(*source, runtime, desc);
        lv.valueHandle = shv.unboxToHV(runtime);

        // sym can be an index-like property, so we have to bypass the assert in
        // defineOwnPropertyInternal.
        if (LLVM_UNLIKELY(
                JSObject::defineOwnPropertyInternal(
                    target,
                    runtime,
                    sym,
                    DefinePropertyFlags::getDefaultNewPropertyFlags(),
                    lv.valueHandle) == ExecutionStatus::EXCEPTION)) {
          return false;
        }

        return true;
      });

  if (LLVM_UNLIKELY(!success))
    return ExecutionStatus::EXCEPTION;

  return target.getHermesValue();
}

/// \code
///   HermesBuiltin.copyRestArgsFast = function (from) {}
/// \endcode
/// Same as copyRestArgs, but produces a FastArray. Used by typed-mode rest
/// parameters declared as Array<T>, where the consuming code expects a
/// FastArray rather than an ordinary JSArray.
CallResult<HermesValue> hermesBuiltinCopyRestArgsFast(
    void *,
    Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  struct : public Locals {
    PinnedValue<FastArray> array;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  auto frames = runtime.getStackFrames();
  auto it = frames.begin();
  ++it;
  if (LLVM_UNLIKELY(it == frames.end()))
    return HermesValue::encodeUndefinedValue();

  if (!args.getArg(0).isNumber())
    return HermesValue::encodeUndefinedValue();
  uint32_t from = truncateToUInt32(args.getArg(0).getNumber());

  uint32_t argCount = it->getArgCount();
  uint32_t length = from <= argCount ? argCount - from : 0;

  auto cr = FastArray::create(runtime, length);
  if (LLVM_UNLIKELY(cr == ExecutionStatus::EXCEPTION))
    return ExecutionStatus::EXCEPTION;
  lv.array.castAndSetHermesValue<FastArray>(*cr);

  for (uint32_t i = 0; i != length; ++i) {
    GCScopeMarkerRAII marker{runtime};
    auto valHandle = runtime.makeHandle(it->getArgRef(from));
    if (LLVM_UNLIKELY(
            FastArray::push(lv.array, runtime, valHandle) ==
            ExecutionStatus::EXCEPTION))
      return ExecutionStatus::EXCEPTION;
    ++from;
  }

  return lv.array.getHermesValue();
}

/// \code
///   HermesBuiltin.copyRestArgs = function (from) {}
/// \endcode
/// Copy the callers parameters starting from index \c from (where the first
/// parameter is index 0) into a JSArray.
CallResult<HermesValue> hermesBuiltinCopyRestArgs(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  struct : public Locals {
    PinnedValue<JSArray> array;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  GCScopeMarkerRAII marker{runtime};

  // Obtain the caller's stack frame.
  auto frames = runtime.getStackFrames();
  auto it = frames.begin();
  ++it;
  // Check for the extremely unlikely case where there is no caller frame.
  if (LLVM_UNLIKELY(it == frames.end()))
    return HermesValue::encodeUndefinedValue();

  // "from" should be a number.
  if (!args.getArg(0).isNumber())
    return HermesValue::encodeUndefinedValue();
  uint32_t from = truncateToUInt32(args.getArg(0).getNumber());

  uint32_t argCount = it->getArgCount();
  uint32_t length = from <= argCount ? argCount - from : 0;

  auto cr = JSArray::create(runtime, length, length);
  if (LLVM_UNLIKELY(cr == ExecutionStatus::EXCEPTION))
    return ExecutionStatus::EXCEPTION;
  lv.array = std::move(*cr);
  JSArray::setStorageEndIndex(lv.array, runtime, length);

  for (uint32_t i = 0; i != length; ++i) {
    const auto shv =
        SmallHermesValue::encodeHermesValue(it->getArgRef(from), runtime);
    JSArray::unsafeSetExistingElementAt(*lv.array, runtime, i, shv);
    ++from;
  }

  return lv.array.getHermesValue();
}

/// \code
///   HermesBuiltin.arraySpread = function(target, source, nextIndex) {}
/// /endcode
/// ES9.0 12.2.5.2
/// Iterate the iterable source (as if using a for-of) and copy the values from
/// the spread source into the target array, starting at `nextIndex`.
/// \return the next empty index in the array to use for additional properties.
CallResult<HermesValue> hermesBuiltinArraySpread(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  struct : public Locals {
    PinnedValue<> nextValue;
    PinnedValue<> idxHandle;
    PinnedValue<> nextIndex;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  GCScopeMarkerRAII topMarker{runtime};
  Handle<JSArray> target = args.dyncastArg<JSArray>(0);
  // To be safe, check for non-arrays.
  if (!target) {
    return runtime.raiseTypeError(
        "HermesBuiltin.arraySpread requires an array target");
  }

  Handle<JSArray> arr = args.dyncastArg<JSArray>(1);
  if (arr) {
    // Copying from an array, first check and make sure that
    // `arr[Symbol.iterator]` hasn't been changed by the user.
    NamedPropertyDescriptor desc;
    PseudoHandle<JSObject> propObj = createPseudoHandle(
        JSObject::getNamedDescriptorPredefined(
            arr, runtime, Predefined::SymbolIterator, desc));
    if (LLVM_LIKELY(propObj) && LLVM_LIKELY(!desc.flags.proxyObject)) {
      SmallHermesValue slotValue =
          JSObject::getNamedSlotValueUnsafe(propObj.get(), runtime, desc);
      propObj.invalidate();
      if (LLVM_LIKELY(
              slotValue.isObject() &&
              slotValue.getObject(runtime) == *runtime.arrayPrototypeValues)) {
        auto nextIndex = args.getArg(2).getNumberAs<JSArray::size_type>();
        GCScopeMarkerRAII marker{runtime};
        for (JSArray::size_type i = 0; i < JSArray::getLength(*arr, runtime);
             ++i) {
          marker.flush();
          // Fast path: look up the property in indexed storage.
          lv.nextValue = arr->at(runtime, i).unboxToHV(runtime);
          if (LLVM_UNLIKELY(lv.nextValue->isEmpty())) {
            // Slow path, just run the full getComputed_RJS path.
            // Runs when there is a hole, accessor, non-regular property, etc.
            lv.idxHandle = HermesValue::encodeTrustedNumberValue(i);
            CallResult<PseudoHandle<>> valueRes =
                JSObject::getComputed_RJS(arr, runtime, lv.idxHandle);
            if (LLVM_UNLIKELY(valueRes == ExecutionStatus::EXCEPTION)) {
              return ExecutionStatus::EXCEPTION;
            }
            lv.nextValue = std::move(*valueRes);
          }
          // It is valid to use setElementAt here because we know that
          // `target` was created immediately prior to running the spread
          // and no non-standard properties were added to it,
          // because the only actions that can be performed between array
          // creation and running this spread are DefineOwnProperty calls with
          // standard flags (as well as other spread operations, which do the
          // same thing).
          if (LLVM_UNLIKELY(
                  JSArray::setElementAt(
                      target, runtime, nextIndex, lv.nextValue) ==
                  ExecutionStatus::EXCEPTION))
            return ExecutionStatus::EXCEPTION;
          ++nextIndex;
        }

        if (LLVM_UNLIKELY(
                JSArray::setLengthProperty(target, runtime, nextIndex) ==
                ExecutionStatus::EXCEPTION)) {
          return ExecutionStatus::EXCEPTION;
        }

        return HermesValue::encodeTrustedNumberValue(nextIndex);
      }
    }
  }

  // 3. Let iteratorRecord be ? GetIterator(spreadObj).
  auto iteratorRecordRes = getCheckedIterator(runtime, args.getArgHandle(1));
  if (LLVM_UNLIKELY(iteratorRecordRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  CheckedIteratorRecord iteratorRecord = *iteratorRecordRes;

  lv.nextIndex = args.getArg(2);

  // 4. Repeat,
  for (GCScopeMarkerRAII marker{runtime}; /* nothing */; marker.flush()) {
    // a. Let next be ? IteratorStep(iteratorRecord).
    auto nextRes = iteratorStep(runtime, iteratorRecord);
    if (LLVM_UNLIKELY(nextRes == ExecutionStatus::EXCEPTION)) {
      return ExecutionStatus::EXCEPTION;
    }
    Handle<JSObject> next = *nextRes;

    // b. If next is false, return nextIndex.
    if (!next) {
      return lv.nextIndex.getHermesValue();
    }
    // c. Let nextValue be ? IteratorValue(next).
    auto nextItemRes = JSObject::getNamed_RJS(
        next, runtime, Predefined::getSymbolID(Predefined::value));
    if (LLVM_UNLIKELY(nextItemRes == ExecutionStatus::EXCEPTION)) {
      return ExecutionStatus::EXCEPTION;
    }
    lv.nextValue = std::move(*nextItemRes);

    // d. Let status be CreateDataProperty(array,
    //    ToString(ToUint32(nextIndex)), nextValue).
    // e. Assert: status is true.
    if (LLVM_UNLIKELY(
            JSArray::defineOwnComputed(
                target,
                runtime,
                lv.nextIndex,
                DefinePropertyFlags::getDefaultNewPropertyFlags(),
                lv.nextValue) == ExecutionStatus::EXCEPTION)) {
      return ExecutionStatus::EXCEPTION;
    }

    // f. Let nextIndex be nextIndex + 1.
    lv.nextIndex =
        HermesValue::encodeTrustedNumberValue(lv.nextIndex->getNumber() + 1);
  }
}

/// \code
///   HermesBuiltin.apply = function(fn, argArray, thisVal(opt)) {}
/// /endcode
/// Faster version of Function.prototype.apply which does not use its `this`
/// argument.
/// `argArray` must be a JSArray with no getters.
/// Equivalent to fn.apply(thisVal, argArray) if thisVal is provided.
/// If thisVal is not provided, equivalent to running `new fn` and passing the
/// arguments in argArray.
CallResult<HermesValue> hermesBuiltinApply(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  struct : public Locals {
    PinnedValue<> thisVal;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  GCScopeMarkerRAII marker{runtime};

  Handle<Callable> fn = args.dyncastArg<Callable>(0);
  if (LLVM_UNLIKELY(!fn)) {
    return runtime.raiseTypeErrorForValue(
        args.getArgHandle(0), " is not a function");
  }

  Handle<JSArray> argArray = args.dyncastArg<JSArray>(1);
  if (LLVM_UNLIKELY(!argArray)) {
    return runtime.raiseTypeError("args must be an array");
  }

  uint32_t len = JSArray::getLength(*argArray, runtime);

  bool isConstructor = args.getArgCount() == 2;
  if (isConstructor) {
    auto thisValRes = Callable::createThisForConstruct_RJS(fn, runtime, fn);
    if (LLVM_UNLIKELY(thisValRes == ExecutionStatus::EXCEPTION)) {
      return ExecutionStatus::EXCEPTION;
    }
    lv.thisVal = thisValRes->getHermesValue();
  } else {
    lv.thisVal = args.getArg(2);
  }

  ScopedNativeCallFrame newFrame{
      runtime, len, *fn, isConstructor, lv.thisVal.getHermesValue()};
  if (LLVM_UNLIKELY(newFrame.overflowed()))
    return runtime.raiseStackOverflow(Runtime::StackOverflowKind::NativeStack);

  for (uint32_t i = 0; i < len; ++i) {
    assert(!argArray->at(runtime, i).isEmpty() && "arg array must be dense");
    HermesValue arg = argArray->at(runtime, i).unboxToHV(runtime);
    newFrame->getArgRef(i) = LLVM_UNLIKELY(arg.isEmpty())
        ? HermesValue::encodeUndefinedValue()
        : arg;
  }
  if (isConstructor) {
    auto res = Callable::construct(fn, runtime, lv.thisVal);
    if (LLVM_UNLIKELY(res == ExecutionStatus::EXCEPTION)) {
      return ExecutionStatus::EXCEPTION;
    }
    return res->getHermesValue();
  }
  auto res = Callable::call(fn, runtime);
  if (LLVM_UNLIKELY(res == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  return res->getHermesValue();
}

/// \code
///   HermesBuiltin.applyArguments = function(fn, thisVal, newTarget) {}
/// /endcode
/// Faster version of Function.prototype.apply which copies the arguments
/// from the caller to the callee.
CallResult<HermesValue> hermesBuiltinApplyArguments(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  // Copy 'arguments' from the caller's stack, then call the callee.

  Handle<Callable> fn = args.dyncastArg<Callable>(0);
  if (LLVM_UNLIKELY(!fn)) {
    return runtime.raiseTypeErrorForValue(
        args.getArgHandle(0), " is not a function");
  }

  Handle<> newTarget = args.getArgHandle(2);
  bool isConstructCall = !newTarget->isUndefined();
  assert(
      newTarget->isUndefined() ||
      isConstructor(runtime, *newTarget) &&
          "new.target can only be undefined or a constructor.");

  Handle<> thisHandle = args.getArgHandle(1);

  // Obtain the caller's stack frame.
  auto frames = runtime.getStackFrames();
  auto it = frames.begin();
  ++it;
  // Check for the extremely unlikely case where there is no caller frame.
  if (LLVM_UNLIKELY(it == frames.end()))
    return HermesValue::encodeUndefinedValue();

  uint32_t argCount = it->getArgCount();

  ScopedNativeCallFrame newFrame{
      runtime,
      argCount,
      HermesValue::encodeObjectValue(*fn),
      *newTarget,
      *thisHandle};
  if (LLVM_UNLIKELY(newFrame.overflowed())) {
    return runtime.raiseStackOverflow(Runtime::StackOverflowKind::NativeStack);
  }

  for (uint32_t i = 0; i < argCount; ++i) {
    newFrame->getArgRef(i) = it->getArgRef(i);
  }

  if (isConstructCall) {
    return Callable::construct(fn, runtime, thisHandle)
        .toCallResultHermesValue();
  } else {
    return Callable::call(fn, runtime).toCallResultHermesValue();
  }
}

/// \code
///   HermesBuiltin.applyWithNewTarget = function(fn, argArray, thisVal,
///   newTarget) {}
/// /endcode
/// Perform a construct call on fn, with newTarget being set as the new.target.
/// This is only used in cases where the new.target is *not* implicitly set to
/// fn, as in the case of a new call. Thus, a direct `super` call may result in
/// this function being invoked.
/// `argArray` must be a JSArray with no getters.
CallResult<HermesValue> hermesBuiltinApplyWithNewTarget(
    void *,
    Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  assert(
      args.getArgCount() == 4 &&
      "builtinApplyWithNewTarget expected 4 arguments");
  GCScopeMarkerRAII marker{runtime};

  Handle<Callable> fn = args.dyncastArg<Callable>(0);
  if (LLVM_UNLIKELY(!fn)) {
    return runtime.raiseTypeErrorForValue(
        args.getArgHandle(0), " is not a function");
  }

  Handle<JSArray> argArray = args.dyncastArg<JSArray>(1);
  if (LLVM_UNLIKELY(!argArray)) {
    return runtime.raiseTypeError("args must be an array");
  }

  uint32_t len = JSArray::getLength(*argArray, runtime);
  auto thisVal = args.getArgHandle(2);
  auto newTarget = args.getArgHandle(3);

  ScopedNativeCallFrame newFrame{
      runtime, len, HermesValue::encodeObjectValue(*fn), *newTarget, *thisVal};
  if (LLVM_UNLIKELY(newFrame.overflowed()))
    return runtime.raiseStackOverflow(Runtime::StackOverflowKind::NativeStack);

  for (uint32_t i = 0; i < len; ++i) {
    assert(!argArray->at(runtime, i).isEmpty() && "arg array must be dense");
    HermesValue arg = argArray->at(runtime, i).unboxToHV(runtime);
    newFrame->getArgRef(i) = LLVM_UNLIKELY(arg.isEmpty())
        ? HermesValue::encodeUndefinedValue()
        : arg;
  }
  auto res = Callable::construct(fn, runtime, thisVal);
  if (LLVM_UNLIKELY(res == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  return res->getHermesValue();
}

/// HermesBuiltin.exportAll(exports, source) will copy exported named
/// properties from `source` to `exports`, defining them on `exports` as
/// non-configurable.
/// Note that the default exported property on `source` is ignored,
/// as are non-enumerable properties on `source`.
CallResult<HermesValue> hermesBuiltinExportAll(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  struct : public Locals {
    PinnedValue<> propertyHandle;
    PinnedValue<HiddenClass> sourceClass;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  Handle<JSObject> exports = args.dyncastArg<JSObject>(0);
  if (LLVM_UNLIKELY(!exports)) {
    return runtime.raiseTypeError(
        "exportAll() exports argument must be object");
  }

  Handle<JSObject> source = args.dyncastArg<JSObject>(1);
  if (LLVM_UNLIKELY(!source) || LLVM_UNLIKELY(source->isProxyObject())) {
    return runtime.raiseTypeError(
        "exportAll() source argument must be non-Proxy object");
  }

  auto dpf = DefinePropertyFlags::getDefaultNewPropertyFlags();
  dpf.configurable = 0;

  CallResult<bool> defineRes{ExecutionStatus::EXCEPTION};

  // Iterate the named properties excluding those which use Symbols.
  lv.sourceClass.castAndSetHermesValue<HiddenClass>(
      HermesValue::encodeObjectValue(source->getClass(runtime)));
  bool result = HiddenClass::forEachPropertyWhile(
      lv.sourceClass,
      runtime,
      [&source, &exports, &lv, &dpf, &defineRes](
          Runtime &runtime, SymbolID id, NamedPropertyDescriptor desc) {
        if (!desc.flags.enumerable)
          return true;

        if (id == Predefined::getSymbolID(Predefined::defaultExport)) {
          return true;
        }

        lv.propertyHandle =
            JSObject::getNamedSlotValueUnsafe(*source, runtime, desc)
                .unboxToHV(runtime);
        defineRes = JSObject::defineOwnProperty(
            exports, runtime, id, dpf, lv.propertyHandle);
        if (LLVM_UNLIKELY(defineRes == ExecutionStatus::EXCEPTION)) {
          return false;
        }

        return true;
      });
  if (LLVM_UNLIKELY(!result)) {
    return ExecutionStatus::EXCEPTION;
  }
  return HermesValue::encodeUndefinedValue();
}

CallResult<HermesValue> hermesBuiltinExponentiate(void *ctx, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  struct : public Locals {
    PinnedValue<BigIntPrimitive> lhs;
    PinnedValue<BigIntPrimitive> rhs;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  CallResult<HermesValue> res = toNumeric_RJS(runtime, args.getArgHandle(0));
  if (LLVM_UNLIKELY(res == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  if (LLVM_LIKELY(!res->isBigInt())) {
    double left = res->getNumber();
    // Using ? toNumber() here causes an exception to be raised if args[1] is a
    // BigInt.
    CallResult<HermesValue> res = toNumber_RJS(runtime, args.getArgHandle(1));
    if (LLVM_UNLIKELY(res == ExecutionStatus::EXCEPTION)) {
      return ExecutionStatus::EXCEPTION;
    }
    return HermesValue::encodeTrustedNumberValue(expOp(left, res->getNumber()));
  }

  lv.lhs.castAndSetHermesValue<BigIntPrimitive>(
      HermesValue::encodeBigIntValue(res->getBigInt()));

  // Can't use toBigInt() here as it converts boolean/strings to bigint.
  res = toNumeric_RJS(runtime, args.getArgHandle(1));
  if (LLVM_UNLIKELY(res == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }

  if (!res->isBigInt()) {
    return runtime.raiseTypeErrorForValue(
        "Cannot convert ", args.getArgHandle(1), " to BigInt");
  }

  lv.rhs.castAndSetHermesValue<BigIntPrimitive>(
      HermesValue::encodeBigIntValue(res->getBigInt()));
  return BigIntPrimitive::exponentiate(runtime, lv.lhs, lv.rhs);
}

CallResult<HermesValue> hermesBuiltinInitRegexNamedGroups(
    void *ctx,
    Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  auto *regexp = dyn_vmcast<JSRegExp>(args.getArg(0));
  auto *groupsObj = dyn_vmcast<JSObject>(args.getArg(1));
  regexp->setGroupNameMappings(runtime, groupsObj);
  return HermesValue::encodeUndefinedValue();
}

/// \code
///   HermesBuiltin.setFunctionName = function(F, name, prefix) {}
/// \endcode
/// This implements a subset of ES2025 10.2.9 SetFunctionName. This is only ever
/// used to initialize the names of methods and accessors for object literals
/// and classes.
/// \p F is the function object.
/// \p name is the property key. Only string and symbols are passed.
/// \p prefix is a number: 0 = no prefix, 1 = "get", 2 = "set".
CallResult<HermesValue> hermesBuiltinSetFunctionName(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  struct : public Locals {
    PinnedValue<JSObject> F;
    PinnedValue<StringPrimitive> computedName;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  lv.F = vmcast<JSObject>(args.getArg(0));

  HermesValue nameArg = args.getArg(1);
  int prefix = 0;
  if (args.getArgCount() > 2 && args.getArg(2).isNumber())
    prefix = static_cast<int>(args.getArg(2).getNumber());

  // Do these spec steps out of order for better efficiency.
  ASCIIRef prefixStr{};
  // 5. If prefix is present, then
  // a. Set name to the string-concatenation of prefix, the code unit 0x0020
  // (SPACE), and name.
  if (prefix == 0) {
    prefixStr = createASCIIRef("");
  } else if (prefix == 1) {
    prefixStr = createASCIIRef("get ");
  } else if (prefix == 2) {
    prefixStr = createASCIIRef("set ");
  }
  SafeUInt32 len(prefixStr.size());

  bool needBrackets = false;
  // 2. If name is a Symbol, then
  if (nameArg.isSymbol()) {
    // a. Let description be name's [[Description]] value.
    auto *description = runtime.getStringPrimFromSymbolID(nameArg.getSymbol());
    // b. If description is undefined, set name to the empty String.
    if (description == runtime.strForSymbolNoDescription.get()) {
      lv.computedName = runtime.getPredefinedString(Predefined::emptyString);
    } else {
      // c. Else, set name to the string-concatenation of "[", description, and
      // "]".
      len.add(2);
      lv.computedName = description;
      needBrackets = true;
    }
  } else {
    // name is guaranteed to be a string.
    lv.computedName = vmcast<StringPrimitive>(nameArg);
  }

  len.add(lv.computedName->getStringLength());
  auto builderRes = StringBuilder::createStringBuilder(runtime, len);
  if (LLVM_UNLIKELY(builderRes == ExecutionStatus::EXCEPTION))
    return ExecutionStatus::EXCEPTION;
  auto builder = *builderRes;

  builder.appendASCIIRef(prefixStr);
  if (needBrackets)
    builder.appendCharacter('[');
  builder.appendStringPrim(lv.computedName);
  if (needBrackets)
    builder.appendCharacter(']');
  lv.computedName = *builder.getStringPrimitive();

  // Step 6: Define the "name" property with
  // { writable: false, enumerable: false, configurable: true }.
  DefinePropertyFlags dpf = DefinePropertyFlags::getDefaultNewPropertyFlags();
  dpf.writable = 0;
  dpf.enumerable = 0;

  auto defineRes = JSObject::defineOwnProperty(
      lv.F,
      runtime,
      Predefined::getSymbolID(Predefined::name),
      dpf,
      lv.computedName);
  if (LLVM_UNLIKELY(defineRes == ExecutionStatus::EXCEPTION))
    return ExecutionStatus::EXCEPTION;

  return HermesValue::encodeUndefinedValue();
}

/// \code
///   HermesBuiltin.fastArrayPop = function (array, n) {}
/// \endcode
/// Pop the last \p n elements from a FastArray, returning the topmost popped
/// element or undefined if no element was popped. \p n is expected to be a
/// non-negative integral number; the SHBuiltin caller is responsible for
/// providing it. Values that don't fit in uint32_t are clamped to UINT32_MAX,
/// which will then be clamped to the array length by FastArray::pop.
CallResult<HermesValue> hermesBuiltinFastArrayPop(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  auto arr = Handle<FastArray>::vmcast(args.getArgHandle(0));
  double nDouble = args.getArg(1).getNumber();
  uint32_t n = nDouble >= (double)UINT32_MAX
      ? UINT32_MAX
      : (nDouble > 0 ? (uint32_t)nDouble : 0);
  return FastArray::pop(arr, runtime, n);
}

/// \code
///   HermesBuiltin.fastArraySlice = function (array, n) {}
/// \endcode
/// Return a new FastArray containing the elements of \p array starting at
/// index \p n (i.e., \c array.slice(n) for FastArrays). The result has the
/// same prototype as \p array. \p n is expected to be a non-negative
/// integral number; the IRGen caller is responsible for providing it.
/// Values that exceed the array length yield an empty array.
CallResult<HermesValue> hermesBuiltinFastArraySlice(void *, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  struct : public Locals {
    PinnedValue<FastArray> source;
    PinnedValue<JSObject> prototype;
    PinnedValue<FastArray> result;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  lv.source = vmcast<FastArray>(args.getArg(0));
  double nDouble = args.getArg(1).getNumber();
  assert(
      nDouble >= 0 && nDouble == std::floor(nDouble) &&
      "fastArraySlice: n must be a non-negative integer");
  uint32_t n = nDouble >= (double)UINT32_MAX ? UINT32_MAX : (uint32_t)nDouble;

  uint32_t srcLen = lv.source->getLengthAsUint32(runtime);

  // Reuse the source's prototype so the result has the same Array<T>
  // class shape.
  lv.prototype = lv.source->getParent(runtime);

  // Clamp n to srcLen so FastArray::append's fromIndex is valid even when
  // the IRGen-supplied index exceeds the source length.
  uint32_t effN = std::min(n, srcLen);
  uint32_t resultLen = srcLen - effN;

  auto cr = FastArray::create(runtime, lv.prototype, resultLen);
  if (LLVM_UNLIKELY(cr == ExecutionStatus::EXCEPTION))
    return ExecutionStatus::EXCEPTION;
  lv.result.castAndSetHermesValue<FastArray>(*cr);

  if (LLVM_UNLIKELY(
          FastArray::append(lv.result, runtime, lv.source, effN) ==
          ExecutionStatus::EXCEPTION))
    return ExecutionStatus::EXCEPTION;

  return lv.result.getHermesValue();
}

void createHermesBuiltins(Runtime &runtime) {
  GCScope gcScope{runtime, "createHermesBuiltins", 128};

  struct : public Locals {
    PinnedValue<NativeFunction> method;
  } lv;
  LocalsRAII lraii(runtime, &lv);
  auto marker = gcScope.createMarker();

  auto defineInternMethod = [&](BuiltinMethod::Enum builtinIndex,
                                Predefined::Str symID,
                                NativeFunctionPtr func,
                                uint8_t count = 0) {
    auto methodRes = NativeFunction::create(
        runtime,
        Handle<JSObject>::vmcast(&runtime.functionPrototype),
        Runtime::makeNullHandle<Environment>(),
        nullptr /* context */,
        func,
        Predefined::getSymbolID(symID),
        count,
        Runtime::makeNullHandle<JSObject>());
    lv.method = std::move(*methodRes);
    runtime.registerBuiltin(builtinIndex, *lv.method);
    gcScope.flushToMarker(marker);
  };

  // HermesBuiltin function properties
  namespace P = Predefined;
  namespace B = BuiltinMethod;
  defineInternMethod(
      B::HermesBuiltin_silentSetPrototypeOf,
      P::silentSetPrototypeOf,
      silentObjectSetPrototypeOf,
      2);
  defineInternMethod(
      B::HermesBuiltin_getTemplateObject,
      P::getTemplateObject,
      hermesBuiltinGetTemplateObject);
  defineInternMethod(
      B::HermesBuiltin_ensureObject,
      P::ensureObject,
      hermesBuiltinEnsureObject,
      2);
  defineInternMethod(
      B::HermesBuiltin_getMethod, P::getMethod, hermesBuiltinGetMethod, 2);
  defineInternMethod(
      B::HermesBuiltin_throwTypeError,
      P::throwTypeError,
      hermesBuiltinThrowTypeError,
      1);
  defineInternMethod(
      B::HermesBuiltin_throwReferenceError,
      P::throwReferenceError,
      hermesBuiltinThrowReferenceError,
      1);
  defineInternMethod(
      B::HermesBuiltin_copyDataProperties,
      P::copyDataProperties,
      hermesBuiltinCopyDataProperties,
      3);
  defineInternMethod(
      B::HermesBuiltin_copyRestArgs,
      P::copyRestArgs,
      hermesBuiltinCopyRestArgs,
      1);
  defineInternMethod(
      B::HermesBuiltin_copyRestArgsFast,
      P::copyRestArgsFast,
      hermesBuiltinCopyRestArgsFast,
      1);
  defineInternMethod(
      B::HermesBuiltin_arraySpread,
      P::arraySpread,
      hermesBuiltinArraySpread,
      2);
  defineInternMethod(B::HermesBuiltin_apply, P::apply, hermesBuiltinApply, 2);
  defineInternMethod(
      B::HermesBuiltin_applyArguments,
      P::apply,
      hermesBuiltinApplyArguments,
      2);
  defineInternMethod(
      B::HermesBuiltin_applyWithNewTarget,
      P::applyWithNewTarget,
      hermesBuiltinApplyWithNewTarget,
      4);
  defineInternMethod(
      B::HermesBuiltin_exportAll, P::exportAll, hermesBuiltinExportAll);
  defineInternMethod(
      B::HermesBuiltin_exponentiationOperator,
      P::exponentiationOperator,
      hermesBuiltinExponentiate);
  defineInternMethod(
      B::HermesBuiltin_initRegexNamedGroups,
      P::initRegexNamedGroups,
      hermesBuiltinInitRegexNamedGroups);
  defineInternMethod(
      B::HermesBuiltin_checkedTypeCast,
      P::checkedTypeCast,
      hermesBuiltinCheckedTypeCast,
      2);
  defineInternMethod(
      B::HermesBuiltin_setFunctionName,
      P::setFunctionName,
      hermesBuiltinSetFunctionName,
      3);

  defineInternMethod(
      B::HermesBuiltin_fastArrayPop,
      P::fastArrayPop,
      hermesBuiltinFastArrayPop,
      2);
  defineInternMethod(
      B::HermesBuiltin_fastArraySlice,
      P::fastArraySlice,
      hermesBuiltinFastArraySlice,
      2);

  // Define the 'requireFast' function, which takes a number argument.
  defineInternMethod(
      B::HermesBuiltin_requireFast, P::requireFast, requireFast, 1);

  // Wasm helper builtins.
  defineInternMethod(B::HermesBuiltin_wasmTrap, P::wasmTrap, wasmTrap, 0);
  defineInternMethod(
      B::HermesBuiltin_wasmI32DivS, P::wasmI32DivS, wasmI32DivS, 2);
  defineInternMethod(
      B::HermesBuiltin_wasmI32DivU, P::wasmI32DivU, wasmI32DivU, 2);
  defineInternMethod(
      B::HermesBuiltin_wasmI32RemS, P::wasmI32RemS, wasmI32RemS, 2);
  defineInternMethod(
      B::HermesBuiltin_wasmI32RemU, P::wasmI32RemU, wasmI32RemU, 2);
  defineInternMethod(
      B::HermesBuiltin_wasmI32Clz, P::wasmI32Clz, wasmI32Clz, 1);
  defineInternMethod(
      B::HermesBuiltin_wasmI32Ctz, P::wasmI32Ctz, wasmI32Ctz, 1);
  defineInternMethod(
      B::HermesBuiltin_wasmI32Popcnt, P::wasmI32Popcnt, wasmI32Popcnt, 1);
  defineInternMethod(
      B::HermesBuiltin_wasmI32Rotl, P::wasmI32Rotl, wasmI32Rotl, 2);
  defineInternMethod(
      B::HermesBuiltin_wasmI32Rotr, P::wasmI32Rotr, wasmI32Rotr, 2);
  defineInternMethod(
      B::HermesBuiltin_wasmI32TruncF64S,
      P::wasmI32TruncF64S,
      wasmI32TruncF64S,
      1);
  defineInternMethod(
      B::HermesBuiltin_wasmI32TruncF64U,
      P::wasmI32TruncF64U,
      wasmI32TruncF64U,
      1);
  defineInternMethod(
      B::HermesBuiltin_wasmI32TruncSatF64S,
      P::wasmI32TruncSatF64S,
      wasmI32TruncSatF64S,
      1);
  defineInternMethod(
      B::HermesBuiltin_wasmI32TruncSatF64U,
      P::wasmI32TruncSatF64U,
      wasmI32TruncSatF64U,
      1);
  defineInternMethod(
      B::HermesBuiltin_wasmI32ReinterpretF32,
      P::wasmI32ReinterpretF32,
      wasmI32ReinterpretF32,
      1);
  defineInternMethod(
      B::HermesBuiltin_wasmF32ReinterpretI32,
      P::wasmF32ReinterpretI32,
      wasmF32ReinterpretI32,
      1);
  defineInternMethod(
      B::HermesBuiltin_wasmF64Copysign,
      P::wasmF64Copysign,
      wasmF64Copysign,
      2);
  defineInternMethod(
      B::HermesBuiltin_wasmF32Copysign,
      P::wasmF32Copysign,
      wasmF32Copysign,
      2);
  defineInternMethod(
      B::HermesBuiltin_wasmNearest,
      P::wasmNearest,
      wasmNearest,
      1);
  // i64 helpers (G.3).
  defineInternMethod(
      B::HermesBuiltin_wasmI64Add, P::wasmI64Add, wasmI64Add, 5);
  defineInternMethod(
      B::HermesBuiltin_wasmI64Sub, P::wasmI64Sub, wasmI64Sub, 5);
  defineInternMethod(
      B::HermesBuiltin_wasmI64Mul, P::wasmI64Mul, wasmI64Mul, 5);
  defineInternMethod(
      B::HermesBuiltin_wasmI64DivS, P::wasmI64DivS, wasmI64DivS, 5);
  defineInternMethod(
      B::HermesBuiltin_wasmI64DivU, P::wasmI64DivU, wasmI64DivU, 5);
  defineInternMethod(
      B::HermesBuiltin_wasmI64RemS, P::wasmI64RemS, wasmI64RemS, 5);
  defineInternMethod(
      B::HermesBuiltin_wasmI64RemU, P::wasmI64RemU, wasmI64RemU, 5);
  defineInternMethod(
      B::HermesBuiltin_wasmI64Shl, P::wasmI64Shl, wasmI64Shl, 5);
  defineInternMethod(
      B::HermesBuiltin_wasmI64ShrS, P::wasmI64ShrS, wasmI64ShrS, 5);
  defineInternMethod(
      B::HermesBuiltin_wasmI64ShrU, P::wasmI64ShrU, wasmI64ShrU, 5);
  defineInternMethod(
      B::HermesBuiltin_wasmI64Rotl, P::wasmI64Rotl, wasmI64Rotl, 5);
  defineInternMethod(
      B::HermesBuiltin_wasmI64Rotr, P::wasmI64Rotr, wasmI64Rotr, 5);
  defineInternMethod(
      B::HermesBuiltin_wasmI64Clz, P::wasmI64Clz, wasmI64Clz, 2);
  defineInternMethod(
      B::HermesBuiltin_wasmI64Ctz, P::wasmI64Ctz, wasmI64Ctz, 2);
  defineInternMethod(
      B::HermesBuiltin_wasmI64Popcnt,
      P::wasmI64Popcnt,
      wasmI64Popcnt,
      2);
  // i64 conversion helpers (G.4b).
  defineInternMethod(
      B::HermesBuiltin_wasmI64TruncF64S,
      P::wasmI64TruncF64S,
      wasmI64TruncF64S,
      2);
  defineInternMethod(
      B::HermesBuiltin_wasmI64TruncF64U,
      P::wasmI64TruncF64U,
      wasmI64TruncF64U,
      2);
  defineInternMethod(
      B::HermesBuiltin_wasmI64TruncSatF64S,
      P::wasmI64TruncSatF64S,
      wasmI64TruncSatF64S,
      2);
  defineInternMethod(
      B::HermesBuiltin_wasmI64TruncSatF64U,
      P::wasmI64TruncSatF64U,
      wasmI64TruncSatF64U,
      2);
  // i64 conversion helpers (G.4c): i64→float and reinterpret.
  defineInternMethod(
      B::HermesBuiltin_wasmF64ConvertI64S,
      P::wasmF64ConvertI64S,
      wasmF64ConvertI64S,
      2);
  defineInternMethod(
      B::HermesBuiltin_wasmF64ConvertI64U,
      P::wasmF64ConvertI64U,
      wasmF64ConvertI64U,
      2);
  defineInternMethod(
      B::HermesBuiltin_wasmF32ConvertI64S,
      P::wasmF32ConvertI64S,
      wasmF32ConvertI64S,
      2);
  defineInternMethod(
      B::HermesBuiltin_wasmF32ConvertI64U,
      P::wasmF32ConvertI64U,
      wasmF32ConvertI64U,
      2);
  defineInternMethod(
      B::HermesBuiltin_wasmI64ReinterpretF64,
      P::wasmI64ReinterpretF64,
      wasmI64ReinterpretF64,
      2);
  defineInternMethod(
      B::HermesBuiltin_wasmF64ReinterpretI64,
      P::wasmF64ReinterpretI64,
      wasmF64ReinterpretI64,
      2);
  // Memory helpers (H.2).
  defineInternMethod(
      B::HermesBuiltin_wasmMemoryGrow,
      P::wasmMemoryGrow,
      wasmMemoryGrow,
      3);

  // Table helpers (J.2).
  defineInternMethod(
      B::HermesBuiltin_wasmCallIndirect,
      P::wasmCallIndirect,
      wasmCallIndirect,
      4);

  // Exception handling helpers (L.1).
  defineInternMethod(
      B::HermesBuiltin_wasmCreateException,
      P::wasmCreateException,
      wasmCreateException,
      1);
  defineInternMethod(
      B::HermesBuiltin_wasmMatchException,
      P::wasmMatchException,
      wasmMatchException,
      2);

  // Bulk memory helpers (N.1).
  defineInternMethod(
      B::HermesBuiltin_wasmMemoryFill,
      P::wasmMemoryFill,
      wasmMemoryFill,
      4);
  defineInternMethod(
      B::HermesBuiltin_wasmMemoryCopy,
      P::wasmMemoryCopy,
      wasmMemoryCopy,
      4);
  defineInternMethod(
      B::HermesBuiltin_wasmMemoryInit,
      P::wasmMemoryInit,
      wasmMemoryInit,
      6);
  defineInternMethod(
      B::HermesBuiltin_wasmDataDrop,
      P::wasmDataDrop,
      wasmDataDrop,
      2);
  defineInternMethod(
      B::HermesBuiltin_wasmDataSegmentInit,
      P::wasmDataSegmentInit,
      wasmDataSegmentInit,
      4);

  // BigInt ↔ i64 conversion helpers.
  defineInternMethod(
      B::HermesBuiltin_wasmBigIntToI64,
      P::wasmBigIntToI64,
      wasmBigIntToI64,
      2);
  defineInternMethod(
      B::HermesBuiltin_wasmI64ToBigInt,
      P::wasmI64ToBigInt,
      wasmI64ToBigInt,
      2);

  // Bulk table helpers (N.2).
  defineInternMethod(
      B::HermesBuiltin_wasmTableFill,
      P::wasmTableFill,
      wasmTableFill,
      4);
  defineInternMethod(
      B::HermesBuiltin_wasmTableCopy,
      P::wasmTableCopy,
      wasmTableCopy,
      7);
  defineInternMethod(
      B::HermesBuiltin_wasmTableInit,
      P::wasmTableInit,
      wasmTableInit,
      7);
  defineInternMethod(
      B::HermesBuiltin_wasmElemDrop,
      P::wasmElemDrop,
      wasmElemDrop,
      2);
  defineInternMethod(
      B::HermesBuiltin_wasmTableGrow,
      P::wasmTableGrow,
      wasmTableGrow,
      5);
  defineInternMethod(
      B::HermesBuiltin_wasmLinkError,
      P::wasmLinkError,
      wasmLinkError,
      1);

  defineInternMethod(
      B::HermesBuiltin_wasmInternType,
      P::wasmInternType,
      wasmInternType,
      1);
  defineInternMethod(
      B::HermesBuiltin_wasmCheckTableArrays,
      P::wasmCheckTableArrays,
      wasmCheckTableArrays,
      2);
}

} // namespace vm
} // namespace hermes
