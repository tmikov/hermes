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
#include "hermes/VM/JSNativeFunctions.h"
#include "hermes/VM/JSObject.h"
#include "hermes/VM/JSWebAssemblyGlobal.h"

using namespace hermes::vm;

namespace {

using WasmBuiltinTest = RuntimeTestFixture;

/// \return a fresh, unbranded native function. Serves both as the object a
/// brand is stamped onto and as the "closure" such a brand wraps, since
/// wasmSetFuncInfo requires a Callable for each.
///
/// A NativeFunction is NOT the shape of a real export; see makeRealExport.
/// It is kept as one of several object kinds the predicate must handle, not
/// as the representative one.
static Handle<NativeFunction> makeNativeFunction(Runtime &runtime) {
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

/// Evaluate \p src as a script and \return its completion value.
static Handle<> evalExpr(Runtime &runtime, llvh::StringRef src) {
  hermes::hbc::CompileFlags flags;
  auto res = runtime.run(src, "file:///wasm-builtin-test.js", flags);
  EXPECT_EQ(ExecutionStatus::RETURNED, res.getStatus());
  if (res == ExecutionStatus::EXCEPTION)
    return Runtime::getUndefinedValue();
  return runtime.makeHandle(*res);
}

/// \return an ordinary JS closure -- a JSFunction, which is what a real Wasm
/// export is, unlike the NativeFunction above.
static Handle<> makeJSClosure(Runtime &runtime) {
  return evalExpr(runtime, "(function jsClosure() {})");
}

/// \return a REAL WebAssembly Exported Function: a module compiled and
/// instantiated at run time, with its export read off the exports object.
///
/// The only subject in this file with genuine wrapper provenance. WasmIRGen
/// builds each export wrapper with createCreateFunctionInst (WasmIRGen.cpp)
/// and brands it through the generated wasmSetFuncInfo call, so this object is
/// both the right kind -- an ordinary JSFunction closure -- and branded the way
/// production code brands one. Its brand was applied by the module's own
/// generated code, not by this test calling wasmSetFuncInfo.
///
/// What made it necessary: the EARLIER version of these tests built every
/// subject with NativeFunction::create, so a predicate narrowed to
/// `vmisa<NativeFunction>` would have passed the whole file while rejecting
/// every real export. The branded closure in the test below now defeats that
/// mutation on its own; this subject is what keeps the tests anchored to the
/// real object rather than to a reconstruction of it.
///
/// The bytes are `(module (func (export "f") (result i32) (i32.const 42)))`
/// as compiled by wat2wasm -- a small module with an exported function.
static Handle<> makeRealExport(Runtime &runtime) {
  return evalExpr(runtime, R"JS(
    var bytes = new Uint8Array([
        0, 97, 115, 109, 1, 0, 0, 0, 1, 5, 1, 96, 0, 1, 127, 3, 2, 1, 0,
        7, 5, 1, 1, 102, 0, 0, 10, 6, 1, 4, 0, 65, 42, 11]);
    var inst = new WebAssembly.Instance(new WebAssembly.Module(bytes.buffer));
    inst.exports.f;
  )JS");
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

  // Listed in NativeFunctions.def, which is the fourth registration point:
  // it generates the pointer-to-name table getFunctionName consults, and
  // NativeFunction::_snapshotNameImpl uses that for heap-snapshot names. A
  // missing entry is not a dispatch failure, it is an anonymous entry in a
  // snapshot, so nothing else in this file would notice.
  EXPECT_STREQ(
      "wasmIsExportedFunction",
      getFunctionName(vmcast<NativeFunction>(*pred)->getFunctionPtr()));

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

  auto exportedFn = makeNativeFunction(runtime);
  auto closure = makeNativeFunction(runtime);
  auto setRes = Callable::executeCall3(
      setInfo,
      runtime,
      Runtime::getUndefinedValue(),
      exportedFn.getHermesValue(),
      closure.getHermesValue(),
      HermesValue::encodeTrustedNumberValue(7));
  ASSERT_EQ(ExecutionStatus::RETURNED, setRes.getStatus());

  EXPECT_TRUE(ask(exportedFn));

  // The object the predicate actually exists to recognise, and the only
  // subject here with real wrapper provenance: WasmIRGen builds each export
  // wrapper with createCreateFunctionInst and brands it through the generated
  // wasmSetFuncInfo call, so this is a JSFunction closure that became an
  // Exported Function the way production code makes one.
  //
  // The earlier version of this test built every subject with
  // NativeFunction::create, so a predicate narrowed to NativeFunction would
  // have passed it while rejecting every real export. The branded closure
  // below now catches that too; this case is what ties the test to the real
  // object.
  auto realExport = makeRealExport(runtime);
  ASSERT_TRUE(vmisa<JSFunction>(*realExport))
      << "an export wrapper should be an ordinary JS closure";
  ASSERT_FALSE(vmisa<NativeFunction>(*realExport));
  EXPECT_TRUE(ask(realExport)) << "a real module export must be recognised";

  // An unbranded closure of that same kind is not one, so the JSFunction case
  // is not simply "everything callable is true".
  auto jsClosure = makeJSClosure(runtime);
  ASSERT_TRUE(vmisa<JSFunction>(*jsClosure));
  EXPECT_FALSE(ask(jsClosure)) << "an unbranded JS closure is not exported";

  // And a hand-branded closure of that kind IS one: the answer follows the
  // brand, not the object's kind.
  auto brandedJSFn = makeJSClosure(runtime);
  ASSERT_EQ(
      ExecutionStatus::RETURNED,
      Callable::executeCall3(
          setInfo,
          runtime,
          Runtime::getUndefinedValue(),
          brandedJSFn.getHermesValue(),
          jsClosure.getHermesValue(),
          HermesValue::encodeTrustedNumberValue(9))
          .getStatus());
  EXPECT_TRUE(ask(brandedJSFn));

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

  // A real module export and an ordinary closure, not two NativeFunctions:
  // the shape whose rooting matters is the one the funcref paths will hand
  // this builtin.
  auto exportedFn = makeRealExport(runtime);
  auto closure = makeJSClosure(runtime);
  ASSERT_TRUE(vmisa<JSFunction>(*exportedFn));

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

/// wasmMakeGlobal's MODE DISCRIMINATOR and its snapshot validation.
///
/// The builtin used to decide "live" by asking whether argument 2 was
/// callable. An immutable funcref global's snapshot VALUE is an Exported
/// Function -- callable -- so it was read as a getter closure and then
/// refused, because a live global must be mutable. The mode is now
/// isMutable, and argument 2 means the value or the getter accordingly.
///
/// This test calls the builtin directly, with the argument shapes below. A
/// PRIVATE_BUILTIN is reachable from any bytecode emitting a CallBuiltin with
/// its index, so each of them is an answer the builtin owes rather than a
/// hypothetical. What a compiled module produces is covered separately, by
/// e2e-global-ref-export.wat.
TEST_F(WasmBuiltinTest, WasmMakeGlobalModeAndSnapshotValidation) {
  GCScope scope{runtime, "WasmMakeGlobalModeAndSnapshotValidation"};

  Callable *makeRaw = runtime.getBuiltinCallable(
      hermes::BuiltinMethod::HermesBuiltin_wasmMakeGlobal);
  ASSERT_NE(nullptr, makeRaw);
  auto make = runtime.makeHandle(makeRaw);

  // JSWebAssemblyGlobal::ValType, spelled out the way WasmIRGen emits it.
  constexpr double kI32 = 0, kI64 = 1, kF64 = 3, kExternRef = 4, kFuncRef = 5;

  /// Call the builtin. \return the Global it built, or a null handle with the
  /// exception cleared if it refused. A refusal and a non-Global result are
  /// deliberately the same answer here: neither is a usable global.
  auto build = [&](double code,
                   bool isMutable,
                   HermesValue arg2,
                   HermesValue arg3) -> Handle<JSWebAssemblyGlobal> {
    auto res = Callable::executeCall4(
        make,
        runtime,
        Runtime::getUndefinedValue(),
        HermesValue::encodeTrustedNumberValue(code),
        HermesValue::encodeBoolValue(isMutable),
        arg2,
        arg3);
    if (res == ExecutionStatus::EXCEPTION) {
      runtime.clearThrownValue();
      return Runtime::makeNullHandle<JSWebAssemblyGlobal>();
    }
    auto *glob = dyn_vmcast<JSWebAssemblyGlobal>(res->getHermesValue());
    if (!glob)
      return Runtime::makeNullHandle<JSWebAssemblyGlobal>();
    return runtime.makeHandle(glob);
  };

  auto undef = HermesValue::encodeUndefinedValue();
  auto nul = HermesValue::encodeNullValue();
  auto realExport = makeRealExport(runtime);
  auto jsClosure = makeJSClosure(runtime);
  ASSERT_TRUE(vmisa<JSFunction>(*realExport));

  // THE CASE THE OLD DISCRIMINATOR GOT WRONG: an immutable funcref snapshot
  // whose value is callable. It is a snapshot, not a live global, and it
  // holds the export itself.
  {
    auto glob = build(kFuncRef, false, realExport.getHermesValue(), undef);
    ASSERT_TRUE(*glob) << "an immutable funcref snapshot must be built";
    EXPECT_EQ(JSWebAssemblyGlobal::ValType::FuncRef, glob->getValType());
    EXPECT_FALSE(glob->isMutable());
    EXPECT_FALSE(glob->isLive(runtime))
        << "a callable VALUE must not be read as a getter closure";
    EXPECT_EQ(realExport->getRaw(), glob->getValue().getRaw());
  }

  // The same misreading reached an externref global holding any function at
  // all, branded or not. LIVE IMPLIES MUTABLE is what survives here: an
  // immutable global is a snapshot however callable its value is.
  {
    auto glob = build(kExternRef, false, jsClosure.getHermesValue(), undef);
    ASSERT_TRUE(*glob);
    EXPECT_FALSE(glob->isLive(runtime));
    EXPECT_EQ(jsClosure->getRaw(), glob->getValue().getRaw());
  }

  // An externref admits ANY JS value: a check on this path would be a bug.
  {
    auto glob = build(kExternRef, false, undef, undef);
    ASSERT_TRUE(*glob);
    EXPECT_TRUE(glob->getValue().isUndefined());

    glob = build(kExternRef, false, nul, undef);
    ASSERT_TRUE(*glob);
    EXPECT_TRUE(glob->getValue().isNull());

    glob = build(
        kExternRef, false, HermesValue::encodeTrustedNumberValue(7), undef);
    ASSERT_TRUE(*glob);
    EXPECT_EQ(7.0, glob->getValue().getNumber());
  }

  // A funcref admits null or an Exported Function, and nothing else. The
  // brand check is isWasmExportedFunction, the same one the JS API uses, so
  // an unbranded closure of the very kind a real export is fails it.
  {
    auto glob = build(kFuncRef, false, nul, undef);
    ASSERT_TRUE(*glob);
    EXPECT_TRUE(glob->getValue().isNull());

    EXPECT_FALSE(*build(kFuncRef, false, jsClosure.getHermesValue(), undef))
        << "a plain JS closure is a host reference, not a funcref";
    EXPECT_FALSE(*build(kFuncRef, false, undef, undef))
        << "undefined is not a funcref";
    EXPECT_FALSE(*build(
        kFuncRef, false, HermesValue::encodeTrustedNumberValue(0), undef));
  }

  // The numeric validation is unchanged: a Number for a numeric type, a
  // BigInt for i64, and neither in the other's place.
  {
    auto glob =
        build(kF64, false, HermesValue::encodeTrustedNumberValue(1.5), undef);
    ASSERT_TRUE(*glob);
    EXPECT_EQ(1.5, glob->getValue().getNumber());

    EXPECT_FALSE(*build(kI32, false, jsClosure.getHermesValue(), undef));
    EXPECT_FALSE(*build(
        kI64, false, HermesValue::encodeTrustedNumberValue(1), undef));
  }

  // A MUTABLE global is live: arguments 2 and 3 are the two closures.
  {
    auto glob = build(
        kI32, true, jsClosure.getHermesValue(), realExport.getHermesValue());
    ASSERT_TRUE(*glob);
    EXPECT_TRUE(glob->isMutable());
    EXPECT_TRUE(glob->isLive(runtime));

    // ...and both must be there. A mutable snapshot is what this builtin
    // exists to refuse -- its writes would go nowhere -- even though mutable
    // snapshots are perfectly legal when the public constructor builds one.
    EXPECT_FALSE(*build(
        kI32, true, HermesValue::encodeTrustedNumberValue(1), undef))
        << "a mutable global must be live";
    EXPECT_FALSE(*build(kI32, true, jsClosure.getHermesValue(), undef))
        << "a live global needs a setter";
  }

  // The range check. It bounds the type code with an ordering comparison,
  // which -Wswitch cannot flag, so it is asserted rather than trusted: 5 is
  // the highest enumerator and 6 is not one.
  {
    EXPECT_TRUE(*build(kFuncRef, false, nul, undef));
    EXPECT_FALSE(*build(6, false, HermesValue::encodeTrustedNumberValue(0),
                        undef));
    EXPECT_FALSE(*build(-1, false, HermesValue::encodeTrustedNumberValue(0),
                        undef));
  }
}

/// wasmGlobalSet called directly: strict validation, and the ORDER it happens
/// in.
///
/// This builtin refuses where the public `.value` setter coerces, and its
/// numeric refusal is reachable from ordinary compiled Wasm rather than only
/// from handcrafted bytecode. The chain, in WasmIRGen.cpp: a module's return
/// buffer is built by calling globalThis.Float64Array, which script can
/// replace; an f32/f64 result is read out of it with an ordinary property
/// load and pushed with NO coercion; and a following global.set on an
/// imported mutable global pops that value and hands it straight to this
/// builtin. A replacement whose elements read back as strings is therefore
/// enough to deliver a non-Number here.
///
/// Each refusal below is checked for leaving the destination unchanged, and
/// the ones aimed at a live global are checked for not having run the setter
/// CLOSURE. That second check is what a .wat test cannot make: a compiled
/// module's setter closure is generated code, so nothing on the JS side can
/// count its invocations. "Validation before any closure invocation" is the
/// property that keeps a refused write out of the module's frame slot, and a
/// call count is direct evidence of it where an unchanged slot is only
/// consistent with it.
///
/// The funcref cases are here as well as in
/// e2e-global-ref-internal-setter.wat for a different reason: the .wat cases
/// reach this builtin only because export-wrapper parameter conversion does
/// not exist yet. When it does, those arguments are intercepted at the
/// `(param funcref)` boundary and the .wat checks change meaning. These do
/// not depend on that.
TEST_F(WasmBuiltinTest, WasmGlobalSetValidatesBeforeStoringOrCalling) {
  GCScope scope{runtime, "WasmGlobalSetValidatesBeforeStoringOrCalling"};

  Callable *setRaw = runtime.getBuiltinCallable(
      hermes::BuiltinMethod::HermesBuiltin_wasmGlobalSet);
  ASSERT_NE(nullptr, setRaw);
  auto globalSet = runtime.makeHandle(setRaw);

  Callable *makeRaw = runtime.getBuiltinCallable(
      hermes::BuiltinMethod::HermesBuiltin_wasmMakeGlobal);
  ASSERT_NE(nullptr, makeRaw);
  auto make = runtime.makeHandle(makeRaw);

  constexpr double kF64 = 3, kExternRef = 4, kFuncRef = 5;

  auto realExport = makeRealExport(runtime);
  auto jsClosure = makeJSClosure(runtime);

  // A live global's storage, with a setter that COUNTS its invocations. Both
  // things the assertions below need -- what was stored and how many times
  // the closure ran -- are read back out of this object.
  evalExpr(runtime, R"JS(
    var __st = {slot: 0, calls: 0};
    var __get = function () { return __st.slot; };
    var __set = function (v) { __st.calls++; __st.slot = v; };
    0;
  )JS");
  auto getter = evalExpr(runtime, "__get");
  auto setter = evalExpr(runtime, "__set");

  auto reset = [&]() { evalExpr(runtime, "__st.slot = 0; __st.calls = 0; 0"); };
  auto calls = [&]() {
    return evalExpr(runtime, "__st.calls").getHermesValue().getNumber();
  };
  auto slot = [&]() { return evalExpr(runtime, "__st.slot").getHermesValue(); };

  /// A LIVE global of type \p code, whose closures are the counting pair
  /// above. wasmMakeGlobal reads its mode from isMutable, so `true` is what
  /// makes arguments 2 and 3 the getter and the setter.
  auto makeLive = [&](double code) -> Handle<JSWebAssemblyGlobal> {
    auto res = Callable::executeCall4(
        make,
        runtime,
        Runtime::getUndefinedValue(),
        HermesValue::encodeTrustedNumberValue(code),
        HermesValue::encodeBoolValue(true),
        getter.getHermesValue(),
        setter.getHermesValue());
    EXPECT_EQ(ExecutionStatus::RETURNED, res.getStatus());
    if (res == ExecutionStatus::EXCEPTION) {
      runtime.clearThrownValue();
      return Runtime::makeNullHandle<JSWebAssemblyGlobal>();
    }
    return runtime.makeHandle(
        vmcast<JSWebAssemblyGlobal>(res->getHermesValue()));
  };

  /// A SNAPSHOT global, built through the public constructor because
  /// wasmMakeGlobal refuses a mutable snapshot -- one of its writes would go
  /// nowhere -- while the constructor takes mutability from the descriptor.
  auto makeSnapshot = [&](const char *js) -> Handle<JSWebAssemblyGlobal> {
    return runtime.makeHandle(
        vmcast<JSWebAssemblyGlobal>(evalExpr(runtime, js).getHermesValue()));
  };

  /// Call the builtin. \return true if it REFUSED the write.
  auto callSet = [&](Handle<JSWebAssemblyGlobal> glob, HermesValue v) -> bool {
    auto res = Callable::executeCall2(
        globalSet,
        runtime,
        Runtime::getUndefinedValue(),
        glob.getHermesValue(),
        v);
    if (res == ExecutionStatus::EXCEPTION) {
      runtime.clearThrownValue();
      return true;
    }
    return false;
  };

  auto str37 = evalExpr(runtime, "'3.7'");

  // THE REFUSAL A REPLACED Float64Array DELIVERS, against a live global.
  {
    auto g = makeLive(kF64);
    ASSERT_TRUE(*g);
    reset();
    EXPECT_TRUE(callSet(g, str37.getHermesValue()))
        << "a string must not satisfy an f64 global";
    EXPECT_EQ(0.0, calls()) << "a refusal must not reach the setter closure";
    EXPECT_EQ(0.0, slot().getNumber()) << "and must not store";

    // The accepted write DOES run it, so the zero above is a property of the
    // refusal rather than of a closure that never works.
    EXPECT_FALSE(callSet(g, HermesValue::encodeTrustedNumberValue(1.5)));
    EXPECT_EQ(1.0, calls());
    EXPECT_EQ(1.5, slot().getNumber());
  }

  // The same refusal with a SNAPSHOT destination, where the storage funnel
  // rather than a closure would have done the store.
  {
    auto g = makeSnapshot(
        "new WebAssembly.Global({value: 'f64', mutable: true}, 1.5)");
    EXPECT_TRUE(callSet(g, str37.getHermesValue()));
    EXPECT_EQ(1.5, g->getValue().getNumber());
  }

  // i64 takes a BigInt and refuses a Number, and the stored BigInt is wrapped
  // to 64 bits. Compared in JS with ===, so the Number 5 does not pass for
  // 5n.
  {
    auto g = makeSnapshot(
        "new WebAssembly.Global({value: 'i64', mutable: true}, 7n)");
    EXPECT_TRUE(callSet(g, HermesValue::encodeTrustedNumberValue(5)));
    auto isSeven = evalExpr(runtime, "(function (v) { return v === 7n; })");
    auto kept = Callable::executeCall1(
        Handle<Callable>::vmcast(isSeven),
        runtime,
        Runtime::getUndefinedValue(),
        g->getValue());
    ASSERT_EQ(ExecutionStatus::RETURNED, kept.getStatus());
    EXPECT_TRUE(kept->get().getBool()) << "a refused write must not store";

    auto big = evalExpr(runtime, "2n ** 100n + 5n");
    EXPECT_FALSE(callSet(g, big.getHermesValue()));
    auto isFive = evalExpr(runtime, "(function (v) { return v === 5n; })");
    auto wrapped = Callable::executeCall1(
        Handle<Callable>::vmcast(isFive),
        runtime,
        Runtime::getUndefinedValue(),
        g->getValue());
    ASSERT_EQ(ExecutionStatus::RETURNED, wrapped.getStatus());
    EXPECT_TRUE(wrapped->get().getBool()) << "2n**100n + 5n wraps to 5n";
  }

  // funcref: null or an Exported Function, validated before the closure runs.
  {
    auto g = makeLive(kFuncRef);
    ASSERT_TRUE(*g);
    reset();
    EXPECT_TRUE(callSet(g, jsClosure.getHermesValue()))
        << "a plain JS closure is a host reference, not a funcref";
    EXPECT_TRUE(callSet(g, HermesValue::encodeUndefinedValue()));
    EXPECT_TRUE(callSet(g, HermesValue::encodeTrustedNumberValue(5)));
    EXPECT_TRUE(callSet(g, str37.getHermesValue()));
    EXPECT_EQ(0.0, calls()) << "no refusal may reach the setter closure";

    EXPECT_FALSE(callSet(g, realExport.getHermesValue()));
    EXPECT_EQ(1.0, calls());
    EXPECT_EQ(realExport->getRaw(), slot().getRaw());
    EXPECT_FALSE(callSet(g, HermesValue::encodeNullValue()));
    EXPECT_EQ(2.0, calls());
    EXPECT_TRUE(slot().isNull());
  }

  // The same rules with a snapshot destination, which is the funnel's arm.
  {
    auto g = makeSnapshot(
        "new WebAssembly.Global({value: 'anyfunc', mutable: true}, null)");
    EXPECT_TRUE(callSet(g, jsClosure.getHermesValue()));
    EXPECT_TRUE(g->getValue().isNull()) << "a refused write must not store";
    EXPECT_FALSE(callSet(g, realExport.getHermesValue()));
    EXPECT_EQ(realExport->getRaw(), g->getValue().getRaw());
  }

  // An externref admits any JS value, so none of these may be refused, and
  // nothing is coerced: the string arrives as the string it is.
  {
    auto g = makeLive(kExternRef);
    ASSERT_TRUE(*g);
    reset();
    EXPECT_FALSE(callSet(g, HermesValue::encodeUndefinedValue()));
    EXPECT_FALSE(callSet(g, HermesValue::encodeNullValue()));
    EXPECT_FALSE(callSet(g, HermesValue::encodeTrustedNumberValue(7)));
    EXPECT_FALSE(callSet(g, jsClosure.getHermesValue()));
    EXPECT_FALSE(callSet(g, str37.getHermesValue()));
    EXPECT_EQ(5.0, calls());
    EXPECT_EQ(str37->getRaw(), slot().getRaw());
  }

  // The entry guards, which are this builtin's own and not the compiler's: a
  // PRIVATE_BUILTIN is reachable from any bytecode emitting a CallBuiltin
  // with its index.
  {
    auto imm = makeSnapshot("new WebAssembly.Global({value: 'i32'}, 1)");
    EXPECT_TRUE(callSet(imm, HermesValue::encodeTrustedNumberValue(2)))
        << "an immutable global must be refused";
    EXPECT_EQ(1.0, imm->getValue().getNumber());

    auto res = Callable::executeCall2(
        globalSet,
        runtime,
        Runtime::getUndefinedValue(),
        jsClosure.getHermesValue(),
        HermesValue::encodeTrustedNumberValue(2));
    EXPECT_EQ(ExecutionStatus::EXCEPTION, res.getStatus())
        << "an object that is not a WebAssembly.Global must be refused";
    if (res == ExecutionStatus::EXCEPTION)
      runtime.clearThrownValue();
  }
}

} // namespace

#endif // HERMES_ENABLE_WASM
