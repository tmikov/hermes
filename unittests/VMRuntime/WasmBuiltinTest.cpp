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
#include "hermes/VM/JSObject.h"

using namespace hermes::vm;

namespace {

using WasmBuiltinTest = RuntimeTestFixture;

/// \return a fresh, unbranded native function. Serves both as the object a
/// brand is stamped onto and as the "closure" such a brand wraps, since
/// wasmSetFuncInfo requires a Callable for each.
static Handle<NativeFunction> makeFunction(Runtime &runtime) {
  return NativeFunction::create(
      runtime,
      runtime.functionPrototype,
      Runtime::makeNullHandle<Environment>(),
      nullptr,
      nullptr,
      Predefined::getSymbolID(Predefined::emptyString),
      0,
      Runtime::makeNullHandle<JSObject>());
}

/// The wasmIsExportedFunction builtin: the WebAssembly Exported Function brand
/// asked as a question, for generated IR that cannot call the C++ predicate.
///
/// Nothing emits a call to it yet -- the funcref paths that will are later
/// work -- so this is the only thing exercising it. That makes two properties
/// worth asserting separately: that it is REGISTERED (an appended builtin that
/// nobody added to createHermesBuiltins compiles and links perfectly happily),
/// and that it ANSWERS correctly.
TEST_F(WasmBuiltinTest, WasmIsExportedFunctionPredicate) {
  GCScope scope{runtime, "WasmIsExportedFunctionPredicate"};

  Callable *predRaw = runtime.getBuiltinCallable(
      hermes::BuiltinMethod::HermesBuiltin_wasmIsExportedFunction);
  ASSERT_NE(nullptr, predRaw);
  auto pred = runtime.makeHandle(predRaw);

  // Registered with arity 1, which is what the compiler will emit against.
  auto lenRes = JSObject::getNamed_RJS(
      Handle<JSObject>::vmcast(pred),
      runtime,
      Predefined::getSymbolID(Predefined::length));
  ASSERT_EQ(ExecutionStatus::RETURNED, lenRes.getStatus());
  ASSERT_TRUE(lenRes->get().isNumber());
  EXPECT_EQ(1.0, lenRes->get().getNumber());

  /// Call the builtin on \p arg. The result must be a boolean: a predicate
  /// that answered anything else would be a value generated IR would then
  /// branch on incorrectly.
  auto ask = [&](Handle<> arg) -> bool {
    auto res = Callable::executeCall1(
        pred, runtime, Runtime::getUndefinedValue(), *arg);
    EXPECT_EQ(ExecutionStatus::RETURNED, res.getStatus());
    if (res == ExecutionStatus::EXCEPTION)
      return false;
    EXPECT_TRUE(res->get().isBool());
    return res->get().isBool() && res->get().getBool();
  };

  // A function carrying the brand wasmSetFuncInfo stamps -- the positive case,
  // and the reason the builtin delegates to isWasmExportedFunction rather than
  // re-deriving the brand: the same stamp the funnel reads must be the stamp
  // this answers to.
  Callable *setInfoRaw = runtime.getBuiltinCallable(
      hermes::BuiltinMethod::HermesBuiltin_wasmSetFuncInfo);
  ASSERT_NE(nullptr, setInfoRaw);
  auto setInfo = runtime.makeHandle(setInfoRaw);

  auto exportedFn = makeFunction(runtime);
  auto closure = makeFunction(runtime);
  auto setRes = Callable::executeCall3(
      setInfo,
      runtime,
      Runtime::getUndefinedValue(),
      exportedFn.getHermesValue(),
      closure.getHermesValue(),
      HermesValue::encodeTrustedNumberValue(7));
  ASSERT_EQ(ExecutionStatus::RETURNED, setRes.getStatus());

  EXPECT_TRUE(ask(exportedFn));

  // Everything else is false rather than an error. The builtin is a
  // PRIVATE_BUILTIN, reachable from any bytecode emitting a CallBuiltin with
  // its index, so every argument shape has to have an answer.
  EXPECT_FALSE(ask(closure)) << "an unbranded function is not exported";
  EXPECT_FALSE(ask(Runtime::getNullValue()));
  EXPECT_FALSE(ask(Runtime::getUndefinedValue()));
  EXPECT_FALSE(
      ask(runtime.makeHandle(HermesValue::encodeTrustedNumberValue(1))));
  EXPECT_FALSE(ask(runtime.makeHandle(HermesValue::encodeBoolValue(true))));

  auto plainObj = runtime.makeHandle(JSObject::create(runtime));
  EXPECT_FALSE(ask(plainObj));

  // A missing argument reads as undefined, so a zero-argument call answers
  // false rather than reading a register that is not there.
  auto zeroRes =
      Callable::executeCall0(pred, runtime, Runtime::getUndefinedValue());
  ASSERT_EQ(ExecutionStatus::RETURNED, zeroRes.getStatus());
  ASSERT_TRUE(zeroRes->get().isBool());
  EXPECT_FALSE(zeroRes->get().getBool());

  // Half a brand is not a brand. The internal closure property is what the
  // brand IS, but wasmSetFuncInfo writes the type id first and the closure
  // last precisely so that carrying the closure implies carrying the id; an
  // object carrying only the closure must not pass, or a later reader would
  // take `undefined` for a signature id.
  auto halfBranded = runtime.makeHandle(JSObject::create(runtime));
  DefinePropertyFlags dpf = DefinePropertyFlags::getNewNonEnumerableFlags();
  dpf.writable = 0;
  dpf.configurable = 0;
  auto defRes = JSObject::defineOwnProperty(
      halfBranded,
      runtime,
      Predefined::getSymbolID(Predefined::InternalPropertyWasmFuncClosure),
      dpf,
      closure);
  ASSERT_EQ(ExecutionStatus::RETURNED, defRes.getStatus());
  ASSERT_TRUE(*defRes);
  EXPECT_FALSE(ask(halfBranded))
      << "a closure without a type id is not a brand";
}

/// The same predicate, but with handle sanitization asked for at rate 1.0
/// instead of the shared fixture's 0.01: the heap moves after every
/// allocation, so a stale pointer held across one is a use-after-free rather
/// than a 1-in-100 chance of one.
///
/// The rate is ignored unless the build has HERMESVM_SANITIZE_HANDLES on, so
/// this test is only stronger than the one above in such a build -- it is not
/// evidence of anything in the default build.
class WasmBuiltinSanitizeTest : public RuntimeTestFixtureBase {
 public:
  WasmBuiltinSanitizeTest()
      : RuntimeTestFixtureBase(
            RuntimeConfig::Builder()
                .withGCConfig(
                    GCConfig::Builder(kTestGCConfigBuilder)
                        .withSanitizeConfig(GCSanitizeConfig::Builder()
                                                .withSanitizeRate(1.0)
                                                .build())
                        .build())
                .build()) {}
};

/// The predicate ALLOCATES: isWasmExportedFunction reaches
/// HiddenClass::findPropertyNoMap, which initializes a missing property map.
/// So each call is a safepoint, and the branded function -- the caller's
/// argument, which the builtin does not root itself -- has to survive it.
TEST_F(WasmBuiltinSanitizeTest, WasmIsExportedFunctionMovesTheHeap) {
  GCScope scope{runtime, "WasmIsExportedFunctionMovesTheHeap"};

  auto pred = runtime.makeHandle(runtime.getBuiltinCallable(
      hermes::BuiltinMethod::HermesBuiltin_wasmIsExportedFunction));
  auto setInfo = runtime.makeHandle(runtime.getBuiltinCallable(
      hermes::BuiltinMethod::HermesBuiltin_wasmSetFuncInfo));
  ASSERT_TRUE(*pred);
  ASSERT_TRUE(*setInfo);

  auto exportedFn = makeFunction(runtime);
  auto closure = makeFunction(runtime);
  ASSERT_EQ(
      ExecutionStatus::RETURNED,
      Callable::executeCall3(
          setInfo,
          runtime,
          Runtime::getUndefinedValue(),
          exportedFn.getHermesValue(),
          closure.getHermesValue(),
          HermesValue::encodeTrustedNumberValue(7))
          .getStatus());

  // Repeat, allocating in between, so the answer has to survive the heap
  // moving under it rather than being read once from a settled heap. The
  // first call is also the one that materializes the property map the
  // predicate's allocation comes from, so a later call exercises the already
  // -mapped path too.
  for (unsigned i = 0; i < 8; ++i) {
    GCScopeMarkerRAII marker{runtime};
    auto garbage = runtime.makeHandle(JSObject::create(runtime));
    (void)garbage;

    auto yes = Callable::executeCall1(
        pred,
        runtime,
        Runtime::getUndefinedValue(),
        exportedFn.getHermesValue());
    ASSERT_EQ(ExecutionStatus::RETURNED, yes.getStatus());
    ASSERT_TRUE(yes->get().isBool());
    EXPECT_TRUE(yes->get().getBool()) << "iteration " << i;

    auto no = Callable::executeCall1(
        pred, runtime, Runtime::getUndefinedValue(), closure.getHermesValue());
    ASSERT_EQ(ExecutionStatus::RETURNED, no.getStatus());
    ASSERT_TRUE(no->get().isBool());
    EXPECT_FALSE(no->get().getBool()) << "iteration " << i;
  }
}

} // namespace

#endif // HERMES_ENABLE_WASM
