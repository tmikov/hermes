/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//===----------------------------------------------------------------------===//
/// \file
/// Initialize the WebAssembly namespace object, its error types
/// (CompileError, LinkError, RuntimeError), the Module constructor,
/// and static methods (validate).
//===----------------------------------------------------------------------===//

#include "../JSLibInternal.h"

#include "hermes/VM/JSArray.h"
#include "hermes/VM/JSArrayBuffer.h"
#include "hermes/VM/JSError.h"
#include "hermes/VM/JSTypedArray.h"
#include "hermes/VM/JSWebAssemblyException.h"
#include "hermes/VM/JSWebAssemblyGlobal.h"
#include "hermes/VM/JSWebAssemblyInstance.h"
#include "hermes/VM/JSWebAssemblyTag.h"
#include "hermes/VM/JSWebAssemblyMemory.h"
#include "hermes/VM/JSWebAssemblyModule.h"
#include "hermes/VM/JSWebAssemblyTable.h"
#include "hermes/VM/Runtime.h"
#include "hermes/VM/RuntimeModule.h"
#include "hermes/WasmFrontend/WasmModuleData.h"

#include <cmath>

namespace hermes {

/// Weak declaration of validateWasmBinary from WasmFrontend.
/// When the WasmFrontend library is linked (full VM), this resolves to the
/// real implementation. When it's not linked (lean VM), it resolves to the
/// weak default which returns false.
__attribute__((__weak__)) bool
validateWasmBinary(const uint8_t *buffer, size_t size) {
  return false;
}

/// Weak declaration of compileWasmToModuleData from WasmFrontend.
/// Returns nullptr in the lean VM (wasm not supported).
__attribute__((__weak__)) std::unique_ptr<WasmModuleData>
compileWasmToModuleData(
    const uint8_t *buffer,
    size_t size,
    std::string &errorMsg) {
  errorMsg = "WebAssembly support not compiled";
  return nullptr;
}

namespace vm {

//===----------------------------------------------------------------------===//
// WebAssembly error types
//===----------------------------------------------------------------------===//

/// Constructor function for WebAssembly error types. Works the same as
/// NativeError constructors in Error.cpp — creates a JSError with the
/// appropriate prototype.
static CallResult<HermesValue>
wasmErrorConstructor(void *context, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();

  auto typeIndex = reinterpret_cast<uintptr_t>(context);

  const PinnedValue<NativeConstructor> *errConstructor;
  const PinnedValue<JSObject> *errPrototype;
  switch (typeIndex) {
    case 0:
      errConstructor = &runtime.wasmCompileErrorConstructor;
      errPrototype = &runtime.wasmCompileErrorPrototype;
      break;
    case 1:
      errConstructor = &runtime.wasmLinkErrorConstructor;
      errPrototype = &runtime.wasmLinkErrorPrototype;
      break;
    case 2:
      errConstructor = &runtime.wasmRuntimeErrorConstructor;
      errPrototype = &runtime.wasmRuntimeErrorPrototype;
      break;
    default:
      llvm_unreachable("Invalid Wasm error type index");
  }

  struct : public Locals {
    PinnedValue<JSObject> selfParent;
    PinnedValue<JSError> self;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  if (LLVM_LIKELY(
          !args.isConstructorCall() ||
          (args.getNewTarget().getRaw() ==
           errConstructor->getHermesValue().getRaw()))) {
    lv.selfParent = *errPrototype;
  } else {
    CallResult<PseudoHandle<JSObject>> thisParentRes =
        NativeConstructor::parentForNewThis_RJS(
            runtime,
            Handle<Callable>::vmcast(&args.getNewTarget()),
            *errPrototype);
    if (LLVM_UNLIKELY(thisParentRes == ExecutionStatus::EXCEPTION)) {
      return ExecutionStatus::EXCEPTION;
    }
    lv.selfParent = std::move(*thisParentRes);
  }
  lv.self = JSError::create(runtime, lv.selfParent);

  JSError::recordStackTrace(lv.self, runtime, true);

  auto message = args.getArgHandle(0);
  if (!message->isUndefined()) {
    if (LLVM_UNLIKELY(
            JSError::setMessage(lv.self, runtime, message) ==
            ExecutionStatus::EXCEPTION)) {
      return ExecutionStatus::EXCEPTION;
    }
  }

  // Handle the options.cause property (ES2023 error cause proposal).
  if (auto options = Handle<JSObject>::dyn_vmcast(args.getArgHandle(1))) {
    GCScopeMarkerRAII marker{runtime};
    auto causeName = Predefined::getSymbolID(Predefined::cause);
    auto hasRes = JSObject::hasNamed(options, runtime, causeName);
    if (LLVM_UNLIKELY(hasRes == ExecutionStatus::EXCEPTION)) {
      return ExecutionStatus::EXCEPTION;
    }
    if (LLVM_UNLIKELY(*hasRes)) {
      auto causeRes = JSObject::getNamed_RJS(
          options, runtime, causeName, PropOpFlags().plusThrowOnError());
      if (LLVM_UNLIKELY(causeRes == ExecutionStatus::EXCEPTION)) {
        return ExecutionStatus::EXCEPTION;
      }
      struct : public Locals {
        PinnedValue<> cause;
      } lv2;
      LocalsRAII lraii2(runtime, &lv2);
      lv2.cause = std::move(*causeRes);
      if (LLVM_UNLIKELY(
              JSObject::defineOwnProperty(
                  lv.self,
                  runtime,
                  Predefined::getSymbolID(Predefined::cause),
                  DefinePropertyFlags::getNewNonEnumerableFlags(),
                  lv2.cause,
                  PropOpFlags().plusThrowOnError()) ==
              ExecutionStatus::EXCEPTION)) {
        return ExecutionStatus::EXCEPTION;
      }
    }
  }

  return lv.self.getHermesValue();
}

static void createWasmErrorType(
    Runtime &runtime,
    Predefined::Str name,
    uintptr_t typeIndex,
    PinnedValue<JSObject> &prototypeOut,
    PinnedValue<NativeConstructor> &constructorOut) {
  struct : public Locals {
    PinnedValue<> nameValue;
    PinnedValue<NativeConstructor> cons;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  Handle<JSObject> prototype{prototypeOut};

  auto defaultName = runtime.getPredefinedString(name);
  lv.nameValue = HermesValue::encodeStringValue(defaultName);
  defineProperty(
      runtime,
      prototype,
      Predefined::getSymbolID(Predefined::name),
      lv.nameValue);

  defineProperty(
      runtime,
      prototype,
      Predefined::getSymbolID(Predefined::message),
      runtime.getPredefinedStringHandle(Predefined::emptyString));

  lv.cons = NativeConstructor::create(
      runtime,
      Handle<JSObject>::vmcast(&runtime.ErrorConstructor),
      reinterpret_cast<void *>(typeIndex),
      wasmErrorConstructor,
      1);

  auto st = Callable::defineNameLengthAndPrototype(
      lv.cons,
      runtime,
      Predefined::getSymbolID(name),
      1,
      prototype,
      Callable::WritablePrototype::No);
  (void)st;
  assert(
      st != ExecutionStatus::EXCEPTION && "defineNameLengthAndPrototype failed");

  constructorOut.castAndSetHermesValue<NativeConstructor>(
      lv.cons.getHermesValue());
}

//===----------------------------------------------------------------------===//
// Helpers
//===----------------------------------------------------------------------===//

/// Extract raw bytes from a BufferSource argument (ArrayBuffer or TypedArray).
static bool extractBufferSourceBytes(
    Runtime &runtime,
    Handle<> arg,
    const uint8_t *&data,
    size_t &size) {
  if (auto *ab = dyn_vmcast<JSArrayBuffer>(*arg)) {
    if (!ab->attached()) {
      return false;
    }
    data = ab->getDataBlock();
    size = ab->size();
    return true;
  }
  if (auto *ta = dyn_vmcast<JSTypedArrayBase>(*arg)) {
    if (!ta->attached(runtime)) {
      return false;
    }
    data = ta->data(runtime);
    size = ta->getByteLength();
    return true;
  }
  return false;
}

/// Raise a WebAssembly.CompileError with the given message.
static ExecutionStatus
raiseCompileError(Runtime &runtime, const char *msg) {
  struct : public Locals {
    PinnedValue<> msgStr;
    PinnedValue<JSError> err;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  auto strRes = StringPrimitive::create(runtime, ASCIIRef(msg, strlen(msg)));
  if (LLVM_UNLIKELY(strRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  lv.msgStr = std::move(*strRes);

  lv.err = JSError::create(
      runtime, Handle<JSObject>{runtime.wasmCompileErrorPrototype});

  if (LLVM_UNLIKELY(
          JSError::setMessage(lv.err, runtime, lv.msgStr) ==
          ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }

  JSError::recordStackTrace(lv.err, runtime, true);

  return runtime.setThrownValue(lv.err.getHermesValue());
}

/// Raise a WebAssembly.LinkError with the given message.
static ExecutionStatus
raiseLinkError(Runtime &runtime, const char *msg) {
  struct : public Locals {
    PinnedValue<> msgStr;
    PinnedValue<JSError> err;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  auto strRes = StringPrimitive::create(runtime, ASCIIRef(msg, strlen(msg)));
  if (LLVM_UNLIKELY(strRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  lv.msgStr = std::move(*strRes);

  lv.err = JSError::create(
      runtime, Handle<JSObject>{runtime.wasmLinkErrorPrototype});

  if (LLVM_UNLIKELY(
          JSError::setMessage(lv.err, runtime, lv.msgStr) ==
          ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }

  JSError::recordStackTrace(lv.err, runtime, true);

  return runtime.setThrownValue(lv.err.getHermesValue());
}

/// Look up globalThis.Promise.resolve and call it with \p value.
/// Returns the resolved Promise object.
static CallResult<HermesValue>
callPromiseResolve(Runtime &runtime, HermesValue value) {
  struct : public Locals {
    PinnedValue<> promiseCons;
    PinnedValue<> resolveFn;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  // Get globalThis.Promise.
  auto promiseRes = JSObject::getNamed_RJS(
      runtime.getGlobal(),
      runtime,
      Predefined::getSymbolID(Predefined::Promise));
  if (LLVM_UNLIKELY(promiseRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  lv.promiseCons = std::move(*promiseRes);

  if (!vmisa<Callable>(*lv.promiseCons)) {
    return runtime.raiseTypeError("Promise is not a constructor");
  }

  // Get Promise.resolve.
  auto resolveRes = JSObject::getNamed_RJS(
      Handle<JSObject>::vmcast(&lv.promiseCons),
      runtime,
      Predefined::getSymbolID(Predefined::resolve));
  if (LLVM_UNLIKELY(resolveRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  lv.resolveFn = std::move(*resolveRes);

  if (!vmisa<Callable>(*lv.resolveFn)) {
    return runtime.raiseTypeError("Promise.resolve is not callable");
  }

  // Call Promise.resolve(value).
  auto callRes = Callable::executeCall1(
      Handle<Callable>::vmcast(&lv.resolveFn),
      runtime,
      lv.promiseCons,
      value);
  if (LLVM_UNLIKELY(callRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  return callRes->getHermesValue();
}

/// Look up globalThis.Promise.reject and call it with \p error.
/// Returns the rejected Promise object.
static CallResult<HermesValue>
callPromiseReject(Runtime &runtime, HermesValue error) {
  struct : public Locals {
    PinnedValue<> promiseCons;
    PinnedValue<> rejectFn;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  // Get globalThis.Promise.
  auto promiseRes = JSObject::getNamed_RJS(
      runtime.getGlobal(),
      runtime,
      Predefined::getSymbolID(Predefined::Promise));
  if (LLVM_UNLIKELY(promiseRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  lv.promiseCons = std::move(*promiseRes);

  if (!vmisa<Callable>(*lv.promiseCons)) {
    return runtime.raiseTypeError("Promise is not a constructor");
  }

  // Get Promise.reject.
  auto rejectRes = JSObject::getNamed_RJS(
      Handle<JSObject>::vmcast(&lv.promiseCons),
      runtime,
      Predefined::getSymbolID(Predefined::reject));
  if (LLVM_UNLIKELY(rejectRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  lv.rejectFn = std::move(*rejectRes);

  if (!vmisa<Callable>(*lv.rejectFn)) {
    return runtime.raiseTypeError("Promise.reject is not callable");
  }

  // Call Promise.reject(error).
  auto callRes = Callable::executeCall1(
      Handle<Callable>::vmcast(&lv.rejectFn),
      runtime,
      lv.promiseCons,
      error);
  if (LLVM_UNLIKELY(callRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  return callRes->getHermesValue();
}

//===----------------------------------------------------------------------===//
// WebAssembly.validate
//===----------------------------------------------------------------------===//

static CallResult<HermesValue>
wasmValidate(void *context, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();

  const uint8_t *data = nullptr;
  size_t size = 0;
  if (!extractBufferSourceBytes(runtime, args.getArgHandle(0), data, size)) {
    return runtime.raiseTypeError(
        "WebAssembly.validate(): argument must be an ArrayBuffer or "
        "typed array");
  }

  bool valid = hermes::validateWasmBinary(data, size);
  return HermesValue::encodeBoolValue(valid);
}

//===----------------------------------------------------------------------===//
// WebAssembly.compile
//===----------------------------------------------------------------------===//

/// WebAssembly.compile(bytes) — compile a Wasm binary asynchronously.
/// Since Hermes doesn't do async compilation, this is synchronous compilation
/// wrapped in a resolved Promise. On error, returns a rejected Promise.
static CallResult<HermesValue>
wasmCompile(void *context, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();

  const uint8_t *data = nullptr;
  size_t size = 0;
  if (!extractBufferSourceBytes(runtime, args.getArgHandle(0), data, size)) {
    return runtime.raiseTypeError(
        "WebAssembly.compile(): argument must be an ArrayBuffer or "
        "typed array");
  }

  // Compile the Wasm binary.
  std::string errorMsg;
  auto moduleData = hermes::compileWasmToModuleData(data, size, errorMsg);
  if (!moduleData) {
    // Create a CompileError and return a rejected Promise.
    raiseCompileError(runtime, errorMsg.c_str());
    HermesValue thrownError = runtime.getThrownValue();
    runtime.clearThrownValue();
    return callPromiseReject(runtime, thrownError);
  }

  struct : public Locals {
    PinnedValue<JSWebAssemblyModule> mod;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  Handle<JSObject> prototype{runtime.wasmModulePrototype};
  lv.mod = JSWebAssemblyModule::create(runtime, prototype);
  lv.mod->setModuleData(std::move(moduleData));

  return callPromiseResolve(runtime, lv.mod.getHermesValue());
}

//===----------------------------------------------------------------------===//
// WebAssembly.instantiate
//===----------------------------------------------------------------------===//

/// Helper: instantiate a module (shared by wasmInstantiate and Instance ctor).
/// Returns the Instance HermesValue on success.
static CallResult<HermesValue>
instantiateModuleImpl(Runtime &runtime, JSWebAssemblyModule *mod, Handle<> importObj) {
  auto *moduleData = mod->getModuleData();
  if (!moduleData) {
    raiseLinkError(runtime, "module has no compiled data");
    return ExecutionStatus::EXCEPTION;
  }

  if (!moduleData->bytecodeProvider) {
    raiseLinkError(runtime, "module was not fully compiled");
    return ExecutionStatus::EXCEPTION;
  }

  struct : public Locals {
    PinnedValue<JSWebAssemblyInstance> inst;
    PinnedValue<JSObject> exportsObj;
    PinnedValue<> result;
    PinnedValue<> oldImports;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  // Validate imports: if the module has imports, the second argument must
  // be an object providing them.
  bool hasImports = !moduleData->importDescs.empty();

  if (hasImports && !importObj->isObject()) {
    raiseLinkError(
        runtime,
        "module has imports but no import object provided");
    return ExecutionStatus::EXCEPTION;
  }

  // Set globalThis.__wasm_imports__ to the import object so the compiled
  // Wasm top-level function can resolve imports from it.
  auto wasmImportsSymbol =
      Predefined::getSymbolID(Predefined::__wasm_imports__);

  // Save the previous value (if any) to restore it later.
  auto prevRes = JSObject::getNamed_RJS(
      runtime.getGlobal(), runtime, wasmImportsSymbol);
  if (LLVM_UNLIKELY(prevRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  lv.oldImports = std::move(*prevRes);

  if (hasImports) {
    auto putRes = JSObject::putNamed_RJS(
        runtime.getGlobal(),
        runtime,
        wasmImportsSymbol,
        importObj);
    if (LLVM_UNLIKELY(putRes == ExecutionStatus::EXCEPTION)) {
      return ExecutionStatus::EXCEPTION;
    }
  }

  // Run the compiled bytecode.
  auto bcProvider = moduleData->bytecodeProvider;
  auto runRes = runtime.runBytecode(
      std::move(bcProvider),
      RuntimeModuleFlags{},
      "wasm-module",
      Runtime::makeNullHandle<Environment>());

  // Restore the old __wasm_imports__ value regardless of success/failure.
  {
    auto restoreRes = JSObject::putNamed_RJS(
        runtime.getGlobal(),
        runtime,
        wasmImportsSymbol,
        lv.oldImports);
    (void)restoreRes;
  }

  if (LLVM_UNLIKELY(runRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }

  lv.result = std::move(*runRes);

  if (!lv.result->isObject()) {
    raiseLinkError(
        runtime, "WebAssembly instantiation failed: unexpected result");
    return ExecutionStatus::EXCEPTION;
  }

  lv.exportsObj.castAndSetHermesValue<JSObject>(lv.result.getHermesValue());

  // Freeze the exports object.
  if (LLVM_UNLIKELY(
          JSObject::freeze(lv.exportsObj, runtime) ==
          ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }

  // Create the Instance object.
  Handle<JSObject> instancePrototype{runtime.wasmInstancePrototype};
  lv.inst = JSWebAssemblyInstance::create(runtime, instancePrototype);

  // Define the "exports" property as non-writable, non-configurable,
  // enumerable (per the WebAssembly spec).
  DefinePropertyFlags exportsDpf{};
  exportsDpf.setWritable = 1;
  exportsDpf.writable = 0;
  exportsDpf.setConfigurable = 1;
  exportsDpf.configurable = 0;
  exportsDpf.setEnumerable = 1;
  exportsDpf.enumerable = 1;
  exportsDpf.setValue = 1;

  auto defRes = JSObject::defineOwnProperty(
      lv.inst,
      runtime,
      Predefined::getSymbolID(Predefined::exports),
      exportsDpf,
      lv.exportsObj);
  if (LLVM_UNLIKELY(defRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }

  return lv.inst.getHermesValue();
}

/// WebAssembly.instantiate(bytesOrModule, importObject) — two overloads:
/// 1. instantiate(bytes, imports) → Promise<{module, instance}>
/// 2. instantiate(module, imports) → Promise<instance>
static CallResult<HermesValue>
wasmInstantiate(void *context, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();

  struct : public Locals {
    PinnedValue<JSWebAssemblyModule> mod;
    PinnedValue<> importArg;
    PinnedValue<> instanceVal;
    PinnedValue<JSObject> resultObj;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  lv.importArg = args.getArg(1);

  // Check if the first argument is a Module (overload 2).
  if (auto *existingMod = dyn_vmcast<JSWebAssemblyModule>(args.getArg(0))) {
    // Overload 2: instantiate(module, imports) → Promise<instance>
    auto instRes = instantiateModuleImpl(runtime, existingMod, lv.importArg);
    if (LLVM_UNLIKELY(instRes == ExecutionStatus::EXCEPTION)) {
      HermesValue thrownError = runtime.getThrownValue();
      runtime.clearThrownValue();
      return callPromiseReject(runtime, thrownError);
    }
    return callPromiseResolve(runtime, *instRes);
  }

  // Overload 1: instantiate(bytes, imports) → Promise<{module, instance}>
  const uint8_t *data = nullptr;
  size_t size = 0;
  if (!extractBufferSourceBytes(runtime, args.getArgHandle(0), data, size)) {
    return runtime.raiseTypeError(
        "WebAssembly.instantiate(): first argument must be a "
        "WebAssembly.Module, ArrayBuffer, or typed array");
  }

  // Compile the Wasm binary.
  std::string errorMsg;
  auto moduleData = hermes::compileWasmToModuleData(data, size, errorMsg);
  if (!moduleData) {
    raiseCompileError(runtime, errorMsg.c_str());
    HermesValue thrownError = runtime.getThrownValue();
    runtime.clearThrownValue();
    return callPromiseReject(runtime, thrownError);
  }

  Handle<JSObject> prototype{runtime.wasmModulePrototype};
  lv.mod = JSWebAssemblyModule::create(runtime, prototype);
  lv.mod->setModuleData(std::move(moduleData));

  // Instantiate the compiled module.
  auto instRes = instantiateModuleImpl(
      runtime, vmcast<JSWebAssemblyModule>(*lv.mod), lv.importArg);
  if (LLVM_UNLIKELY(instRes == ExecutionStatus::EXCEPTION)) {
    HermesValue thrownError = runtime.getThrownValue();
    runtime.clearThrownValue();
    return callPromiseReject(runtime, thrownError);
  }
  lv.instanceVal = *instRes;

  // Create the result object {module, instance}.
  lv.resultObj = JSObject::create(runtime);

  auto putRes = JSObject::putNamed_RJS(
      lv.resultObj,
      runtime,
      Predefined::getSymbolID(Predefined::module),
      lv.mod);
  if (LLVM_UNLIKELY(putRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }

  putRes = JSObject::putNamed_RJS(
      lv.resultObj,
      runtime,
      Predefined::getSymbolID(Predefined::instance),
      lv.instanceVal);
  if (LLVM_UNLIKELY(putRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }

  return callPromiseResolve(runtime, lv.resultObj.getHermesValue());
}

//===----------------------------------------------------------------------===//
// WebAssembly.Module
//===----------------------------------------------------------------------===//

/// new WebAssembly.Module(bytes) — compile a Wasm binary module.
static CallResult<HermesValue>
wasmModuleConstructor(void *context, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();

  if (!args.isConstructorCall()) {
    return runtime.raiseTypeError(
        "WebAssembly.Module() must be called with 'new'");
  }

  const uint8_t *data = nullptr;
  size_t size = 0;
  if (!extractBufferSourceBytes(runtime, args.getArgHandle(0), data, size)) {
    return runtime.raiseTypeError(
        "WebAssembly.Module(): argument must be an ArrayBuffer or "
        "typed array");
  }

  // Compile the Wasm binary.
  std::string errorMsg;
  auto moduleData = hermes::compileWasmToModuleData(data, size, errorMsg);
  if (!moduleData) {
    raiseCompileError(runtime, errorMsg.c_str());
    return ExecutionStatus::EXCEPTION;
  }

  struct : public Locals {
    PinnedValue<JSWebAssemblyModule> mod;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  Handle<JSObject> prototype{runtime.wasmModulePrototype};
  lv.mod = JSWebAssemblyModule::create(runtime, prototype);
  lv.mod->setModuleData(std::move(moduleData));

  return lv.mod.getHermesValue();
}

/// Return the Predefined string for an export/import kind.
static Predefined::Str kindToPredefined(const std::string &kind) {
  if (kind == "function")
    return Predefined::function;
  if (kind == "table")
    return Predefined::table;
  if (kind == "memory")
    return Predefined::memory;
  if (kind == "global")
    return Predefined::global;
  if (kind == "tag")
    return Predefined::tag;
  return Predefined::function;
}

/// WebAssembly.Module.exports(module) — return export descriptors.
/// Each descriptor is {name: string, kind: string}.
static CallResult<HermesValue>
wasmModuleExports(void *context, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();

  auto *mod = dyn_vmcast<JSWebAssemblyModule>(args.getArg(0));
  if (!mod) {
    return runtime.raiseTypeError(
        "WebAssembly.Module.exports(): argument is not a "
        "WebAssembly.Module");
  }

  auto *moduleData = mod->getModuleData();
  if (!moduleData) {
    return runtime.raiseTypeError(
        "WebAssembly.Module.exports(): module has no data");
  }

  struct : public Locals {
    PinnedValue<JSArray> arr;
    PinnedValue<JSObject> desc;
    PinnedValue<> strVal;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  auto arrRes = JSArray::create(runtime, moduleData->exportDescs.size(), 0);
  if (LLVM_UNLIKELY(arrRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  lv.arr = std::move(*arrRes);

  GCScopeMarkerRAII marker{runtime};
  for (uint32_t i = 0, e = moduleData->exportDescs.size(); i < e; ++i) {
    marker.flush();
    const auto &exp = moduleData->exportDescs[i];

    lv.desc = JSObject::create(runtime);

    // Set 'name' property.
    auto nameRes = StringPrimitive::create(
        runtime, ASCIIRef(exp.name.data(), exp.name.size()));
    if (LLVM_UNLIKELY(nameRes == ExecutionStatus::EXCEPTION)) {
      return ExecutionStatus::EXCEPTION;
    }
    lv.strVal = std::move(*nameRes);
    auto putRes = JSObject::putNamed_RJS(
        lv.desc,
        runtime,
        Predefined::getSymbolID(Predefined::name),
        lv.strVal);
    if (LLVM_UNLIKELY(putRes == ExecutionStatus::EXCEPTION)) {
      return ExecutionStatus::EXCEPTION;
    }

    // Set 'kind' property.
    lv.strVal = HermesValue::encodeStringValue(
        runtime.getPredefinedString(kindToPredefined(exp.kind)));
    putRes = JSObject::putNamed_RJS(
        lv.desc,
        runtime,
        Predefined::getSymbolID(Predefined::kind),
        lv.strVal);
    if (LLVM_UNLIKELY(putRes == ExecutionStatus::EXCEPTION)) {
      return ExecutionStatus::EXCEPTION;
    }

    (void)JSArray::setElementAt(lv.arr, runtime, i, lv.desc);
  }

  if (LLVM_UNLIKELY(
          JSArray::setLengthProperty(
              lv.arr, runtime, moduleData->exportDescs.size()) ==
          ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }

  return lv.arr.getHermesValue();
}

/// WebAssembly.Module.imports(module) — return import descriptors.
/// Each descriptor is {module: string, name: string, kind: string}.
static CallResult<HermesValue>
wasmModuleImports(void *context, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();

  auto *mod = dyn_vmcast<JSWebAssemblyModule>(args.getArg(0));
  if (!mod) {
    return runtime.raiseTypeError(
        "WebAssembly.Module.imports(): argument is not a "
        "WebAssembly.Module");
  }

  auto *moduleData = mod->getModuleData();
  if (!moduleData) {
    return runtime.raiseTypeError(
        "WebAssembly.Module.imports(): module has no data");
  }

  struct : public Locals {
    PinnedValue<JSArray> arr;
    PinnedValue<JSObject> desc;
    PinnedValue<> strVal;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  auto arrRes = JSArray::create(runtime, moduleData->importDescs.size(), 0);
  if (LLVM_UNLIKELY(arrRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  lv.arr = std::move(*arrRes);

  GCScopeMarkerRAII marker{runtime};
  for (uint32_t i = 0, e = moduleData->importDescs.size(); i < e; ++i) {
    marker.flush();
    const auto &imp = moduleData->importDescs[i];

    lv.desc = JSObject::create(runtime);

    // Set 'module' property.
    auto modRes = StringPrimitive::create(
        runtime, ASCIIRef(imp.module.data(), imp.module.size()));
    if (LLVM_UNLIKELY(modRes == ExecutionStatus::EXCEPTION)) {
      return ExecutionStatus::EXCEPTION;
    }
    lv.strVal = std::move(*modRes);
    auto putRes = JSObject::putNamed_RJS(
        lv.desc,
        runtime,
        Predefined::getSymbolID(Predefined::module),
        lv.strVal);
    if (LLVM_UNLIKELY(putRes == ExecutionStatus::EXCEPTION)) {
      return ExecutionStatus::EXCEPTION;
    }

    // Set 'name' property.
    auto nameRes = StringPrimitive::create(
        runtime, ASCIIRef(imp.name.data(), imp.name.size()));
    if (LLVM_UNLIKELY(nameRes == ExecutionStatus::EXCEPTION)) {
      return ExecutionStatus::EXCEPTION;
    }
    lv.strVal = std::move(*nameRes);
    putRes = JSObject::putNamed_RJS(
        lv.desc,
        runtime,
        Predefined::getSymbolID(Predefined::name),
        lv.strVal);
    if (LLVM_UNLIKELY(putRes == ExecutionStatus::EXCEPTION)) {
      return ExecutionStatus::EXCEPTION;
    }

    // Set 'kind' property.
    lv.strVal = HermesValue::encodeStringValue(
        runtime.getPredefinedString(kindToPredefined(imp.kind)));
    putRes = JSObject::putNamed_RJS(
        lv.desc,
        runtime,
        Predefined::getSymbolID(Predefined::kind),
        lv.strVal);
    if (LLVM_UNLIKELY(putRes == ExecutionStatus::EXCEPTION)) {
      return ExecutionStatus::EXCEPTION;
    }

    (void)JSArray::setElementAt(lv.arr, runtime, i, lv.desc);
  }

  if (LLVM_UNLIKELY(
          JSArray::setLengthProperty(
              lv.arr, runtime, moduleData->importDescs.size()) ==
          ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }

  return lv.arr.getHermesValue();
}

//===----------------------------------------------------------------------===//
// WebAssembly.Instance
//===----------------------------------------------------------------------===//

/// new WebAssembly.Instance(module, importObject) — instantiate a compiled
/// Wasm module with the given import object.
static CallResult<HermesValue>
wasmInstanceConstructor(void *context, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();

  if (!args.isConstructorCall()) {
    return runtime.raiseTypeError(
        "WebAssembly.Instance() must be called with 'new'");
  }

  // First argument must be a WebAssembly.Module.
  auto *mod = dyn_vmcast<JSWebAssemblyModule>(args.getArg(0));
  if (!mod) {
    return runtime.raiseTypeError(
        "WebAssembly.Instance(): first argument must be a "
        "WebAssembly.Module");
  }

  struct : public Locals {
    PinnedValue<> importObj;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  lv.importObj = args.getArg(1);
  return instantiateModuleImpl(runtime, mod, lv.importObj);
}

//===----------------------------------------------------------------------===//
// WebAssembly.Memory
//===----------------------------------------------------------------------===//

/// new WebAssembly.Memory({initial: N, maximum: M}) — create a linear memory.
static CallResult<HermesValue>
wasmMemoryConstructor(void *context, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();

  if (!args.isConstructorCall()) {
    return runtime.raiseTypeError(
        "WebAssembly.Memory() must be called with 'new'");
  }

  // The first argument must be an object with an "initial" property.
  auto optionsHandle = args.getArgHandle(0);
  if (!optionsHandle->isObject()) {
    return runtime.raiseTypeError(
        "WebAssembly.Memory(): argument must be a memory descriptor object");
  }

  struct : public Locals {
    PinnedValue<JSObject> options;
    PinnedValue<> initialVal;
    PinnedValue<> maximumVal;
    PinnedValue<JSWebAssemblyMemory> mem;
    PinnedValue<JSArrayBuffer> buf;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  lv.options.castAndSetHermesValue<JSObject>(optionsHandle.getHermesValue());

  // Read "initial" property (required).
  auto initialRes = JSObject::getNamed_RJS(
      lv.options,
      runtime,
      Predefined::getSymbolID(Predefined::initial));
  if (LLVM_UNLIKELY(initialRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  lv.initialVal = std::move(*initialRes);

  if (lv.initialVal->isUndefined()) {
    return runtime.raiseTypeError(
        "WebAssembly.Memory(): 'initial' is required");
  }

  auto initialRes2 = toNumber_RJS(runtime, lv.initialVal);
  if (LLVM_UNLIKELY(initialRes2 == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  double initialDbl = initialRes2->getDouble();
  if (initialDbl < 0 || initialDbl > 65536 ||
      initialDbl != std::floor(initialDbl)) {
    return runtime.raiseRangeError(
        "WebAssembly.Memory(): 'initial' must be a non-negative integer "
        "<= 65536");
  }
  uint32_t initialPages = static_cast<uint32_t>(initialDbl);

  // Read "maximum" property (optional).
  uint32_t maxPages = 65536; // Default: no explicit maximum (Wasm max).
  auto maximumRes = JSObject::getNamed_RJS(
      lv.options,
      runtime,
      Predefined::getSymbolID(Predefined::maximum));
  if (LLVM_UNLIKELY(maximumRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  lv.maximumVal = std::move(*maximumRes);

  if (!lv.maximumVal->isUndefined()) {
    auto maximumRes2 = toNumber_RJS(runtime, lv.maximumVal);
    if (LLVM_UNLIKELY(maximumRes2 == ExecutionStatus::EXCEPTION)) {
      return ExecutionStatus::EXCEPTION;
    }
    double maxDbl = maximumRes2->getDouble();
    if (maxDbl < 0 || maxDbl > 65536 || maxDbl != std::floor(maxDbl)) {
      return runtime.raiseRangeError(
          "WebAssembly.Memory(): 'maximum' must be a non-negative integer "
          "<= 65536");
    }
    maxPages = static_cast<uint32_t>(maxDbl);
    if (initialPages > maxPages) {
      return runtime.raiseRangeError(
          "WebAssembly.Memory(): 'initial' must not exceed 'maximum'");
    }
  }

  // Create the Memory object.
  Handle<JSObject> memPrototype{runtime.wasmMemoryPrototype};
  lv.mem = JSWebAssemblyMemory::create(runtime, memPrototype);
  lv.mem->setMaxPages(maxPages);

  // Create the backing ArrayBuffer.
  uint32_t byteLength = initialPages * 65536;
  lv.buf = JSArrayBuffer::create(
      runtime, Handle<JSObject>::vmcast(&runtime.arrayBufferPrototype));

  if (LLVM_UNLIKELY(
          JSArrayBuffer::createDataBlock(runtime, lv.buf, byteLength) ==
          ExecutionStatus::EXCEPTION)) {
    return runtime.raiseRangeError(
        "WebAssembly.Memory(): could not allocate memory");
  }

  lv.mem->setBuffer(runtime, *lv.buf);

  return lv.mem.getHermesValue();
}

/// WebAssembly.Memory.prototype.buffer getter — returns the ArrayBuffer.
static CallResult<HermesValue>
wasmMemoryBufferGetter(void *context, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();

  auto *mem = dyn_vmcast<JSWebAssemblyMemory>(args.getThisArg());
  if (!mem) {
    return runtime.raiseTypeError(
        "WebAssembly.Memory.prototype.buffer: 'this' is not a "
        "WebAssembly.Memory");
  }

  JSArrayBuffer *buf = mem->getBuffer(runtime);
  if (!buf) {
    return HermesValue::encodeUndefinedValue();
  }

  return HermesValue::encodeObjectValue(buf);
}

/// WebAssembly.Memory.prototype.grow(delta) — grow the memory.
static CallResult<HermesValue>
wasmMemoryGrowMethod(void *context, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();

  auto *mem = dyn_vmcast<JSWebAssemblyMemory>(args.getThisArg());
  if (!mem) {
    return runtime.raiseTypeError(
        "WebAssembly.Memory.prototype.grow: 'this' is not a "
        "WebAssembly.Memory");
  }

  struct : public Locals {
    PinnedValue<> deltaVal;
    PinnedValue<JSArrayBuffer> oldBuf;
    PinnedValue<JSArrayBuffer> newBuf;
    PinnedValue<JSWebAssemblyMemory> memHandle;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  lv.memHandle = mem;

  // Convert delta argument to number.
  lv.deltaVal = args.getArg(0);
  auto deltaRes = toNumber_RJS(runtime, lv.deltaVal);
  if (LLVM_UNLIKELY(deltaRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  double deltaDbl = deltaRes->getDouble();
  if (deltaDbl < 0 || deltaDbl > 65536 ||
      deltaDbl != std::floor(deltaDbl)) {
    return runtime.raiseRangeError(
        "WebAssembly.Memory.prototype.grow: invalid delta");
  }
  uint32_t delta = static_cast<uint32_t>(deltaDbl);

  // Get old buffer info.
  JSArrayBuffer *oldBufPtr = lv.memHandle->getBuffer(runtime);
  if (!oldBufPtr) {
    return runtime.raiseTypeError(
        "WebAssembly.Memory.prototype.grow: memory has no buffer");
  }
  lv.oldBuf = oldBufPtr;

  uint32_t oldSize = static_cast<uint32_t>(lv.oldBuf->size());
  uint32_t oldPages = oldSize / 65536;

  // Check growth limits.
  uint32_t maxPages = lv.memHandle->getMaxPages();
  uint64_t newPages64 = static_cast<uint64_t>(oldPages) + delta;
  if (newPages64 > maxPages || newPages64 > 65536) {
    return runtime.raiseRangeError(
        "WebAssembly.Memory.prototype.grow: would exceed maximum");
  }
  uint32_t newPages = static_cast<uint32_t>(newPages64);
  uint32_t newSize = newPages * 65536;

  // Create a new ArrayBuffer with the larger size.
  lv.newBuf = JSArrayBuffer::create(
      runtime, Handle<JSObject>::vmcast(&runtime.arrayBufferPrototype));

  if (LLVM_UNLIKELY(
          JSArrayBuffer::createDataBlock(runtime, lv.newBuf, newSize, true) ==
          ExecutionStatus::EXCEPTION)) {
    return runtime.raiseRangeError(
        "WebAssembly.Memory.prototype.grow: allocation failed");
  }

  // Copy old data to the new buffer.
  if (oldSize > 0) {
    JSArrayBuffer::copyDataBlockBytes(
        runtime, *lv.newBuf, 0, *lv.oldBuf, 0, oldSize);
  }

  // Update the Memory object's buffer reference.
  lv.memHandle->setBuffer(runtime, *lv.newBuf);

  // Return the old page count.
  return HermesValue::encodeTrustedNumberValue(oldPages);
}

//===----------------------------------------------------------------------===//
// WebAssembly.Table
//===----------------------------------------------------------------------===//

/// new WebAssembly.Table({element: "anyfunc", initial: N, maximum: M}) —
/// create a table for function references.
static CallResult<HermesValue>
wasmTableConstructor(void *context, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();

  if (!args.isConstructorCall()) {
    return runtime.raiseTypeError(
        "WebAssembly.Table() must be called with 'new'");
  }

  auto optionsHandle = args.getArgHandle(0);
  if (!optionsHandle->isObject()) {
    return runtime.raiseTypeError(
        "WebAssembly.Table(): argument must be a table descriptor object");
  }

  struct : public Locals {
    PinnedValue<JSObject> options;
    PinnedValue<> elementVal;
    PinnedValue<> initialVal;
    PinnedValue<> maximumVal;
    PinnedValue<JSWebAssemblyTable> tbl;
    PinnedValue<JSArray> arr;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  lv.options.castAndSetHermesValue<JSObject>(optionsHandle.getHermesValue());

  // Read "element" property (required).
  auto elementRes = JSObject::getNamed_RJS(
      lv.options,
      runtime,
      Predefined::getSymbolID(Predefined::element));
  if (LLVM_UNLIKELY(elementRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  lv.elementVal = std::move(*elementRes);

  // The element type must be "anyfunc" (or "funcref", which is equivalent).
  if (!lv.elementVal->isString()) {
    return runtime.raiseTypeError(
        "WebAssembly.Table(): 'element' must be a string");
  }
  auto *elemStr = lv.elementVal->getString();
  bool isAnyfunc = elemStr->equals(
      runtime.getPredefinedString(Predefined::anyfunc));
  // Also accept "funcref" as an alias for "anyfunc" per the spec.
  if (!isAnyfunc) {
    auto funcrefRes = StringPrimitive::create(
        runtime, ASCIIRef("funcref", 7));
    if (LLVM_UNLIKELY(funcrefRes == ExecutionStatus::EXCEPTION)) {
      return ExecutionStatus::EXCEPTION;
    }
    auto *funcrefStr = vmcast<StringPrimitive>(*funcrefRes);
    if (!elemStr->equals(funcrefStr)) {
      return runtime.raiseTypeError(
          "WebAssembly.Table(): 'element' must be 'anyfunc' or 'funcref'");
    }
  }

  // Read "initial" property (required).
  auto initialRes = JSObject::getNamed_RJS(
      lv.options,
      runtime,
      Predefined::getSymbolID(Predefined::initial));
  if (LLVM_UNLIKELY(initialRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  lv.initialVal = std::move(*initialRes);

  if (lv.initialVal->isUndefined()) {
    return runtime.raiseTypeError(
        "WebAssembly.Table(): 'initial' is required");
  }

  auto initialRes2 = toNumber_RJS(runtime, lv.initialVal);
  if (LLVM_UNLIKELY(initialRes2 == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  double initialDbl = initialRes2->getDouble();
  if (initialDbl < 0 || initialDbl > 0xFFFFFFFF ||
      initialDbl != std::floor(initialDbl)) {
    return runtime.raiseRangeError(
        "WebAssembly.Table(): 'initial' must be a non-negative integer");
  }
  uint32_t initialSize = static_cast<uint32_t>(initialDbl);

  // Read "maximum" property (optional).
  uint32_t maxSize = 0; // 0 means no explicit maximum.
  auto maximumRes = JSObject::getNamed_RJS(
      lv.options,
      runtime,
      Predefined::getSymbolID(Predefined::maximum));
  if (LLVM_UNLIKELY(maximumRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  lv.maximumVal = std::move(*maximumRes);

  if (!lv.maximumVal->isUndefined()) {
    auto maximumRes2 = toNumber_RJS(runtime, lv.maximumVal);
    if (LLVM_UNLIKELY(maximumRes2 == ExecutionStatus::EXCEPTION)) {
      return ExecutionStatus::EXCEPTION;
    }
    double maxDbl = maximumRes2->getDouble();
    if (maxDbl < 0 || maxDbl > 0xFFFFFFFF ||
        maxDbl != std::floor(maxDbl)) {
      return runtime.raiseRangeError(
          "WebAssembly.Table(): 'maximum' must be a non-negative integer");
    }
    maxSize = static_cast<uint32_t>(maxDbl);
    if (initialSize > maxSize) {
      return runtime.raiseRangeError(
          "WebAssembly.Table(): 'initial' must not exceed 'maximum'");
    }
  }

  // Create the Table object.
  Handle<JSObject> tablePrototype{runtime.wasmTablePrototype};
  lv.tbl = JSWebAssemblyTable::create(runtime, tablePrototype);
  lv.tbl->setMaxSize(maxSize);

  // Create the backing JSArray with initialSize entries (all null).
  auto arrRes = JSArray::create(runtime, initialSize, initialSize);
  if (LLVM_UNLIKELY(arrRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  lv.arr = std::move(*arrRes);

  // Initialize all entries to null.
  GCScopeMarkerRAII marker{runtime};
  for (uint32_t i = 0; i < initialSize; ++i) {
    marker.flush();
    (void)JSArray::setElementAt(
        lv.arr,
        runtime,
        i,
        runtime.makeHandle(HermesValue::encodeNullValue()));
  }

  lv.tbl->setElements(runtime, *lv.arr);

  return lv.tbl.getHermesValue();
}

/// WebAssembly.Table.prototype.length getter.
static CallResult<HermesValue>
wasmTableLengthGetter(void *context, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();

  auto *tbl = dyn_vmcast<JSWebAssemblyTable>(args.getThisArg());
  if (!tbl) {
    return runtime.raiseTypeError(
        "WebAssembly.Table.prototype.length: 'this' is not a "
        "WebAssembly.Table");
  }

  JSArray *arr = tbl->getElements(runtime);
  if (!arr) {
    return HermesValue::encodeTrustedNumberValue(0);
  }

  return HermesValue::encodeTrustedNumberValue(
      JSArray::getLength(arr, runtime));
}

/// WebAssembly.Table.prototype.get(index) — return the element at index.
static CallResult<HermesValue>
wasmTableGetMethod(void *context, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();

  auto *tbl = dyn_vmcast<JSWebAssemblyTable>(args.getThisArg());
  if (!tbl) {
    return runtime.raiseTypeError(
        "WebAssembly.Table.prototype.get: 'this' is not a "
        "WebAssembly.Table");
  }

  struct : public Locals {
    PinnedValue<> indexVal;
    PinnedValue<JSArray> arr;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  lv.indexVal = args.getArg(0);
  auto indexRes = toNumber_RJS(runtime, lv.indexVal);
  if (LLVM_UNLIKELY(indexRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  double indexDbl = indexRes->getDouble();

  JSArray *arrPtr = tbl->getElements(runtime);
  if (!arrPtr) {
    return runtime.raiseRangeError(
        "WebAssembly.Table.prototype.get: index out of bounds");
  }
  lv.arr = arrPtr;

  uint32_t len = JSArray::getLength(*lv.arr, runtime);
  if (indexDbl < 0 || indexDbl >= len ||
      indexDbl != std::floor(indexDbl)) {
    return runtime.raiseRangeError(
        "WebAssembly.Table.prototype.get: index out of bounds");
  }
  uint32_t index = static_cast<uint32_t>(indexDbl);

  HermesValue elem = lv.arr->at(runtime, index).unboxToHV(runtime);
  return elem;
}

/// WebAssembly.Table.prototype.set(index, value) — set the element.
static CallResult<HermesValue>
wasmTableSetMethod(void *context, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();

  auto *tbl = dyn_vmcast<JSWebAssemblyTable>(args.getThisArg());
  if (!tbl) {
    return runtime.raiseTypeError(
        "WebAssembly.Table.prototype.set: 'this' is not a "
        "WebAssembly.Table");
  }

  struct : public Locals {
    PinnedValue<> indexVal;
    PinnedValue<> funcVal;
    PinnedValue<JSWebAssemblyTable> tblHandle;
    PinnedValue<JSArray> arr;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  lv.tblHandle = tbl;

  lv.indexVal = args.getArg(0);
  auto indexRes = toNumber_RJS(runtime, lv.indexVal);
  if (LLVM_UNLIKELY(indexRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  double indexDbl = indexRes->getDouble();

  JSArray *arrPtr = lv.tblHandle->getElements(runtime);
  if (!arrPtr) {
    return runtime.raiseRangeError(
        "WebAssembly.Table.prototype.set: index out of bounds");
  }
  lv.arr = arrPtr;

  uint32_t len = JSArray::getLength(*lv.arr, runtime);
  if (indexDbl < 0 || indexDbl >= len ||
      indexDbl != std::floor(indexDbl)) {
    return runtime.raiseRangeError(
        "WebAssembly.Table.prototype.set: index out of bounds");
  }
  uint32_t index = static_cast<uint32_t>(indexDbl);

  // The value must be null or a callable function.
  lv.funcVal = args.getArg(1);
  if (!lv.funcVal->isNull()) {
    if (!dyn_vmcast<Callable>(*lv.funcVal)) {
      return runtime.raiseTypeError(
          "WebAssembly.Table.prototype.set: value must be null or a "
          "function");
    }
  }

  (void)JSArray::setElementAt(lv.arr, runtime, index, lv.funcVal);

  return HermesValue::encodeUndefinedValue();
}

/// WebAssembly.Table.prototype.grow(delta) — grow the table.
static CallResult<HermesValue>
wasmTableGrowMethod(void *context, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();

  auto *tbl = dyn_vmcast<JSWebAssemblyTable>(args.getThisArg());
  if (!tbl) {
    return runtime.raiseTypeError(
        "WebAssembly.Table.prototype.grow: 'this' is not a "
        "WebAssembly.Table");
  }

  struct : public Locals {
    PinnedValue<> deltaVal;
    PinnedValue<JSWebAssemblyTable> tblHandle;
    PinnedValue<JSArray> oldArr;
    PinnedValue<JSArray> newArr;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  lv.tblHandle = tbl;

  lv.deltaVal = args.getArg(0);
  auto deltaRes = toNumber_RJS(runtime, lv.deltaVal);
  if (LLVM_UNLIKELY(deltaRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  double deltaDbl = deltaRes->getDouble();
  if (deltaDbl < 0 || deltaDbl > 0xFFFFFFFF ||
      deltaDbl != std::floor(deltaDbl)) {
    return runtime.raiseRangeError(
        "WebAssembly.Table.prototype.grow: invalid delta");
  }
  uint32_t delta = static_cast<uint32_t>(deltaDbl);

  JSArray *oldArrPtr = lv.tblHandle->getElements(runtime);
  uint32_t oldLen = 0;
  if (oldArrPtr) {
    lv.oldArr = oldArrPtr;
    oldLen = JSArray::getLength(*lv.oldArr, runtime);
  }

  uint64_t newLen64 = static_cast<uint64_t>(oldLen) + delta;
  uint32_t maxSize = lv.tblHandle->getMaxSize();
  if (maxSize > 0 && newLen64 > maxSize) {
    return runtime.raiseRangeError(
        "WebAssembly.Table.prototype.grow: would exceed maximum");
  }
  if (newLen64 > 0xFFFFFFFF) {
    return runtime.raiseRangeError(
        "WebAssembly.Table.prototype.grow: would exceed maximum");
  }
  uint32_t newLen = static_cast<uint32_t>(newLen64);

  // Create a new array with the larger size.
  auto arrRes = JSArray::create(runtime, newLen, newLen);
  if (LLVM_UNLIKELY(arrRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  lv.newArr = std::move(*arrRes);

  GCScopeMarkerRAII marker{runtime};

  // Copy old entries.
  for (uint32_t i = 0; i < oldLen; ++i) {
    marker.flush();
    HermesValue elem = lv.oldArr->at(runtime, i).unboxToHV(runtime);
    (void)JSArray::setElementAt(
        lv.newArr, runtime, i, runtime.makeHandle(elem));
  }

  // Initialize new entries to null.
  for (uint32_t i = oldLen; i < newLen; ++i) {
    marker.flush();
    (void)JSArray::setElementAt(
        lv.newArr,
        runtime,
        i,
        runtime.makeHandle(HermesValue::encodeNullValue()));
  }

  lv.tblHandle->setElements(runtime, *lv.newArr);

  return HermesValue::encodeTrustedNumberValue(oldLen);
}

//===----------------------------------------------------------------------===//
// WebAssembly.Global
//===----------------------------------------------------------------------===//

/// WebAssembly.Global constructor.
static CallResult<HermesValue>
wasmGlobalConstructor(void *context, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();

  if (!args.isConstructorCall()) {
    return runtime.raiseTypeError(
        "WebAssembly.Global() must be called with 'new'");
  }

  auto descriptorHandle = args.getArgHandle(0);
  if (!descriptorHandle->isObject()) {
    return runtime.raiseTypeError(
        "WebAssembly.Global(): argument must be a global descriptor object");
  }

  struct : public Locals {
    PinnedValue<JSObject> descriptor;
    PinnedValue<> valueTypeVal;
    PinnedValue<> mutableVal;
    PinnedValue<> initVal;
    PinnedValue<JSWebAssemblyGlobal> glob;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  lv.descriptor.castAndSetHermesValue<JSObject>(
      descriptorHandle.getHermesValue());

  // Read "value" property (required) — the type string.
  auto valueTypeRes = JSObject::getNamed_RJS(
      lv.descriptor,
      runtime,
      Predefined::getSymbolID(Predefined::value));
  if (LLVM_UNLIKELY(valueTypeRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  lv.valueTypeVal = std::move(*valueTypeRes);

  if (!lv.valueTypeVal->isString()) {
    return runtime.raiseTypeError(
        "WebAssembly.Global(): 'value' must be a string");
  }

  // Parse the value type string by comparing against known type names.
  auto *typeStr = lv.valueTypeVal->getString();
  JSWebAssemblyGlobal::ValType valType;

  // Helper to create a comparison string and check equality.
  auto matchStr = [&](const char *s, size_t len) -> bool {
    auto res = StringPrimitive::create(runtime, ASCIIRef(s, len));
    if (LLVM_UNLIKELY(res == ExecutionStatus::EXCEPTION))
      return false;
    return typeStr->equals(vmcast<StringPrimitive>(*res));
  };

  if (matchStr("i32", 3)) {
    valType = JSWebAssemblyGlobal::ValType::I32;
  } else if (matchStr("i64", 3)) {
    valType = JSWebAssemblyGlobal::ValType::I64;
  } else if (matchStr("f32", 3)) {
    valType = JSWebAssemblyGlobal::ValType::F32;
  } else if (matchStr("f64", 3)) {
    valType = JSWebAssemblyGlobal::ValType::F64;
  } else {
    return runtime.raiseTypeError(
        "WebAssembly.Global(): 'value' must be "
        "'i32', 'i64', 'f32', or 'f64'");
  }

  // Read "mutable" property (optional, defaults to false).
  auto mutableRes = JSObject::getNamed_RJS(
      lv.descriptor,
      runtime,
      Predefined::getSymbolID(Predefined::mutable_));
  if (LLVM_UNLIKELY(mutableRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  lv.mutableVal = std::move(*mutableRes);
  bool isMutable = toBoolean(lv.mutableVal.getHermesValue());

  // Read initial value (second argument, optional, defaults to 0).
  double initValue = 0.0;
  if (args.getArgCount() >= 2) {
    lv.initVal = args.getArg(1);
    auto initRes = toNumber_RJS(runtime, lv.initVal);
    if (LLVM_UNLIKELY(initRes == ExecutionStatus::EXCEPTION)) {
      return ExecutionStatus::EXCEPTION;
    }
    initValue = initRes->getDouble();

    // For integer types, truncate to the appropriate range.
    if (valType == JSWebAssemblyGlobal::ValType::I32) {
      initValue = static_cast<double>(
          static_cast<int32_t>(static_cast<int64_t>(initValue)));
    } else if (valType == JSWebAssemblyGlobal::ValType::F32) {
      initValue = static_cast<double>(static_cast<float>(initValue));
    }
    // i64 and f64 keep the full double value.
  }

  // Create the Global object.
  Handle<JSObject> globalPrototype{runtime.wasmGlobalPrototype};
  lv.glob = JSWebAssemblyGlobal::create(runtime, globalPrototype);
  lv.glob->setValType(valType);
  lv.glob->setMutable(isMutable);
  lv.glob->setValue(initValue);

  return lv.glob.getHermesValue();
}

/// WebAssembly.Global.prototype.value getter.
static CallResult<HermesValue>
wasmGlobalValueGetter(void *context, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();

  auto *glob = dyn_vmcast<JSWebAssemblyGlobal>(args.getThisArg());
  if (!glob) {
    return runtime.raiseTypeError(
        "WebAssembly.Global.prototype.value: 'this' is not a "
        "WebAssembly.Global");
  }

  return HermesValue::encodeTrustedNumberValue(glob->getValue());
}

/// WebAssembly.Global.prototype.value setter.
static CallResult<HermesValue>
wasmGlobalValueSetter(void *context, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();

  auto *glob = dyn_vmcast<JSWebAssemblyGlobal>(args.getThisArg());
  if (!glob) {
    return runtime.raiseTypeError(
        "WebAssembly.Global.prototype.value: 'this' is not a "
        "WebAssembly.Global");
  }

  if (!glob->isMutable()) {
    return runtime.raiseTypeError(
        "WebAssembly.Global.prototype.value: cannot set an immutable global");
  }

  struct : public Locals {
    PinnedValue<> newVal;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  lv.newVal = args.getArg(0);
  auto numRes = toNumber_RJS(runtime, lv.newVal);
  if (LLVM_UNLIKELY(numRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  double val = numRes->getDouble();

  // Truncate for integer/float types.
  auto valType = glob->getValType();
  if (valType == JSWebAssemblyGlobal::ValType::I32) {
    val = static_cast<double>(
        static_cast<int32_t>(static_cast<int64_t>(val)));
  } else if (valType == JSWebAssemblyGlobal::ValType::F32) {
    val = static_cast<double>(static_cast<float>(val));
  }

  glob->setValue(val);
  return HermesValue::encodeUndefinedValue();
}

/// WebAssembly.Global.prototype.valueOf() — same as value getter.
static CallResult<HermesValue>
wasmGlobalValueOfMethod(void *context, Runtime &runtime) {
  return wasmGlobalValueGetter(context, runtime);
}

//===----------------------------------------------------------------------===//
// WebAssembly.Tag
//===----------------------------------------------------------------------===//

/// Parse a Wasm value type string ("i32", "i64", "f32", "f64") into a
/// JSWebAssemblyTag::ValType. Returns true on success.
static bool parseValTypeString(
    Runtime &runtime,
    StringPrimitive *str,
    JSWebAssemblyTag::ValType &result) {
  auto matchStr = [&](const char *s, size_t len) -> bool {
    auto res = StringPrimitive::create(runtime, ASCIIRef(s, len));
    if (LLVM_UNLIKELY(res == ExecutionStatus::EXCEPTION))
      return false;
    return str->equals(vmcast<StringPrimitive>(*res));
  };

  if (matchStr("i32", 3)) {
    result = JSWebAssemblyTag::ValType::I32;
    return true;
  }
  if (matchStr("i64", 3)) {
    result = JSWebAssemblyTag::ValType::I64;
    return true;
  }
  if (matchStr("f32", 3)) {
    result = JSWebAssemblyTag::ValType::F32;
    return true;
  }
  if (matchStr("f64", 3)) {
    result = JSWebAssemblyTag::ValType::F64;
    return true;
  }
  return false;
}

/// WebAssembly.Tag constructor.
/// new WebAssembly.Tag({parameters: ['i32', 'f64']})
static CallResult<HermesValue>
wasmTagConstructor(void *context, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();

  if (!args.isConstructorCall()) {
    return runtime.raiseTypeError(
        "WebAssembly.Tag() must be called with 'new'");
  }

  auto descriptorHandle = args.getArgHandle(0);
  if (!descriptorHandle->isObject()) {
    return runtime.raiseTypeError(
        "WebAssembly.Tag(): argument must be a tag type object");
  }

  struct : public Locals {
    PinnedValue<JSObject> descriptor;
    PinnedValue<> paramsVal;
    PinnedValue<JSObject> paramsObj;
    PinnedValue<> lenVal;
    PinnedValue<> elemVal;
    PinnedValue<JSWebAssemblyTag> tag;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  lv.descriptor.castAndSetHermesValue<JSObject>(
      descriptorHandle.getHermesValue());

  // Read "parameters" property (required).
  auto paramsRes = JSObject::getNamed_RJS(
      lv.descriptor,
      runtime,
      Predefined::getSymbolID(Predefined::parameters));
  if (LLVM_UNLIKELY(paramsRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  lv.paramsVal = std::move(*paramsRes);

  if (!lv.paramsVal->isObject()) {
    return runtime.raiseTypeError(
        "WebAssembly.Tag(): 'parameters' must be an iterable");
  }
  lv.paramsObj.castAndSetHermesValue<JSObject>(lv.paramsVal.getHermesValue());

  // Get the length of the parameters array.
  auto lenRes = JSObject::getNamed_RJS(
      lv.paramsObj,
      runtime,
      Predefined::getSymbolID(Predefined::length));
  if (LLVM_UNLIKELY(lenRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  lv.lenVal = std::move(*lenRes);

  if (!lv.lenVal->isNumber()) {
    return runtime.raiseTypeError(
        "WebAssembly.Tag(): 'parameters' must be an array-like object");
  }

  uint32_t paramCount = static_cast<uint32_t>(lv.lenVal->getNumber());
  std::vector<JSWebAssemblyTag::ValType> paramTypes;
  paramTypes.reserve(paramCount);

  for (uint32_t i = 0; i < paramCount; ++i) {
    GCScopeMarkerRAII marker{runtime};
    auto elemRes = JSObject::getComputed_RJS(
        lv.paramsObj,
        runtime,
        runtime.makeHandle(
            HermesValue::encodeTrustedNumberValue(static_cast<double>(i))));
    if (LLVM_UNLIKELY(elemRes == ExecutionStatus::EXCEPTION)) {
      return ExecutionStatus::EXCEPTION;
    }
    lv.elemVal = std::move(*elemRes);

    if (!lv.elemVal->isString()) {
      return runtime.raiseTypeError(
          "WebAssembly.Tag(): parameter type must be a string");
    }

    JSWebAssemblyTag::ValType vt;
    if (!parseValTypeString(runtime, lv.elemVal->getString(), vt)) {
      return runtime.raiseTypeError(
          "WebAssembly.Tag(): parameter type must be "
          "'i32', 'i64', 'f32', or 'f64'");
    }
    paramTypes.push_back(vt);
  }

  // Create the Tag object.
  Handle<JSObject> tagPrototype{runtime.wasmTagPrototype};
  lv.tag = JSWebAssemblyTag::create(runtime, tagPrototype);
  lv.tag->setParameters(std::move(paramTypes));

  return lv.tag.getHermesValue();
}

//===----------------------------------------------------------------------===//
// WebAssembly.Exception
//===----------------------------------------------------------------------===//

/// WebAssembly.Exception constructor.
/// new WebAssembly.Exception(tag, [v0, v1, ...])
static CallResult<HermesValue>
wasmExceptionConstructor(void *context, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();

  if (!args.isConstructorCall()) {
    return runtime.raiseTypeError(
        "WebAssembly.Exception() must be called with 'new'");
  }

  // First arg must be a WebAssembly.Tag.
  auto *tag = dyn_vmcast<JSWebAssemblyTag>(args.getArg(0));
  if (!tag) {
    return runtime.raiseTypeError(
        "WebAssembly.Exception(): first argument must be a WebAssembly.Tag");
  }

  struct : public Locals {
    PinnedValue<JSWebAssemblyTag> tagHandle;
    PinnedValue<JSObject> payloadObj;
    PinnedValue<> lenVal;
    PinnedValue<> elemVal;
    PinnedValue<JSArray> arr;
    PinnedValue<> numVal;
    PinnedValue<JSWebAssemblyException> exc;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  lv.tagHandle = tag;

  const auto &paramTypes = lv.tagHandle->getParameters();
  uint32_t paramCount = paramTypes.size();

  // Second arg must be an iterable/array-like with matching length.
  auto payloadHandle = args.getArgHandle(1);
  if (!payloadHandle->isObject()) {
    return runtime.raiseTypeError(
        "WebAssembly.Exception(): second argument must be an array-like");
  }
  lv.payloadObj.castAndSetHermesValue<JSObject>(
      payloadHandle.getHermesValue());

  // Create the payload array.
  auto arrRes = JSArray::create(runtime, paramCount, paramCount);
  if (LLVM_UNLIKELY(arrRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  lv.arr = std::move(*arrRes);

  // Copy and coerce payload values according to the tag's parameter types.
  for (uint32_t i = 0; i < paramCount; ++i) {
    GCScopeMarkerRAII marker{runtime};
    auto elemRes = JSObject::getComputed_RJS(
        lv.payloadObj,
        runtime,
        runtime.makeHandle(
            HermesValue::encodeTrustedNumberValue(static_cast<double>(i))));
    if (LLVM_UNLIKELY(elemRes == ExecutionStatus::EXCEPTION)) {
      return ExecutionStatus::EXCEPTION;
    }
    lv.elemVal = std::move(*elemRes);

    // Coerce to number.
    lv.numVal = lv.elemVal.getHermesValue();
    auto numRes = toNumber_RJS(runtime, lv.numVal);
    if (LLVM_UNLIKELY(numRes == ExecutionStatus::EXCEPTION)) {
      return ExecutionStatus::EXCEPTION;
    }
    double val = numRes->getDouble();

    // Truncate based on type.
    switch (paramTypes[i]) {
      case JSWebAssemblyTag::ValType::I32:
        val = static_cast<double>(
            static_cast<int32_t>(static_cast<int64_t>(val)));
        break;
      case JSWebAssemblyTag::ValType::F32:
        val = static_cast<double>(static_cast<float>(val));
        break;
      case JSWebAssemblyTag::ValType::I64:
      case JSWebAssemblyTag::ValType::F64:
        // Keep full double precision.
        break;
    }

    lv.numVal = HermesValue::encodeTrustedNumberValue(val);
    (void)JSArray::setElementAt(lv.arr, runtime, i, lv.numVal);
  }

  // Create the Exception object.
  Handle<JSObject> excPrototype{runtime.wasmExceptionPrototype};
  lv.exc = JSWebAssemblyException::create(runtime, excPrototype);
  lv.exc->setTag(runtime, *lv.tagHandle);
  lv.exc->setPayload(runtime, *lv.arr);

  return lv.exc.getHermesValue();
}

/// WebAssembly.Exception.prototype.is(tag)
/// Returns true if this exception's tag identity-matches the given tag.
static CallResult<HermesValue>
wasmExceptionIsMethod(void *context, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();

  auto *exc = dyn_vmcast<JSWebAssemblyException>(args.getThisArg());
  if (!exc) {
    return runtime.raiseTypeError(
        "WebAssembly.Exception.prototype.is: 'this' is not a "
        "WebAssembly.Exception");
  }

  auto *tag = dyn_vmcast<JSWebAssemblyTag>(args.getArg(0));
  if (!tag) {
    return runtime.raiseTypeError(
        "WebAssembly.Exception.prototype.is: argument must be a "
        "WebAssembly.Tag");
  }

  // Identity check: the exception's tag must be the same object.
  bool matches = (exc->getTag(runtime) == tag);
  return HermesValue::encodeBoolValue(matches);
}

/// WebAssembly.Exception.prototype.getArg(tag, index)
/// Returns the payload value at the given index if the tag matches.
static CallResult<HermesValue>
wasmExceptionGetArgMethod(void *context, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();

  auto *exc = dyn_vmcast<JSWebAssemblyException>(args.getThisArg());
  if (!exc) {
    return runtime.raiseTypeError(
        "WebAssembly.Exception.prototype.getArg: 'this' is not a "
        "WebAssembly.Exception");
  }

  auto *tag = dyn_vmcast<JSWebAssemblyTag>(args.getArg(0));
  if (!tag) {
    return runtime.raiseTypeError(
        "WebAssembly.Exception.prototype.getArg: first argument must be a "
        "WebAssembly.Tag");
  }

  // Tag must identity-match.
  if (exc->getTag(runtime) != tag) {
    return runtime.raiseTypeError(
        "WebAssembly.Exception.prototype.getArg: tag does not match");
  }

  struct : public Locals {
    PinnedValue<> indexVal;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  lv.indexVal = args.getArg(1);
  auto indexRes = toNumber_RJS(runtime, lv.indexVal);
  if (LLVM_UNLIKELY(indexRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  double indexD = indexRes->getDouble();
  uint32_t index = static_cast<uint32_t>(indexD);

  if (static_cast<double>(index) != indexD ||
      index >= tag->getParameters().size()) {
    return runtime.raiseRangeError("WebAssembly.Exception.prototype.getArg: "
                                   "index out of range");
  }

  auto *payload = exc->getPayload(runtime);
  if (!payload) {
    return HermesValue::encodeUndefinedValue();
  }

  auto val = payload->at(runtime, index);
  if (val.isEmpty()) {
    return HermesValue::encodeUndefinedValue();
  }
  return val.unboxToHV(runtime);
}

//===----------------------------------------------------------------------===//
// createWebAssemblyObject
//===----------------------------------------------------------------------===//

void createWebAssemblyObject(Runtime &runtime, MutableHandle<JSObject> result) {
  struct : public Locals {
    PinnedValue<JSObject> wasmObj;
    PinnedValue<NativeConstructor> moduleCons;
    PinnedValue<NativeConstructor> instanceCons;
    PinnedValue<NativeConstructor> memoryCons;
    PinnedValue<NativeConstructor> tableCons;
    PinnedValue<NativeConstructor> globalCons;
    PinnedValue<NativeConstructor> tagCons;
    PinnedValue<NativeConstructor> exceptionCons;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  lv.wasmObj = JSObject::create(runtime);

  // Set @@toStringTag to "WebAssembly".
  {
    auto dpf = DefinePropertyFlags::getDefaultNewPropertyFlags();
    dpf.writable = 0;
    dpf.enumerable = 0;

    defineProperty(
        runtime,
        lv.wasmObj,
        Predefined::getSymbolID(Predefined::SymbolToStringTag),
        runtime.getPredefinedStringHandle(Predefined::WebAssembly),
        dpf);
  }

  // Create error types and register them as properties of WebAssembly.
  createWasmErrorType(
      runtime,
      Predefined::CompileError,
      0,
      runtime.wasmCompileErrorPrototype,
      runtime.wasmCompileErrorConstructor);
  createWasmErrorType(
      runtime,
      Predefined::LinkError,
      1,
      runtime.wasmLinkErrorPrototype,
      runtime.wasmLinkErrorConstructor);
  createWasmErrorType(
      runtime,
      Predefined::RuntimeError,
      2,
      runtime.wasmRuntimeErrorPrototype,
      runtime.wasmRuntimeErrorConstructor);

  DefinePropertyFlags dpf = DefinePropertyFlags::getNewNonEnumerableFlags();

  auto res = JSObject::defineOwnProperty(
      lv.wasmObj,
      runtime,
      Predefined::getSymbolID(Predefined::CompileError),
      dpf,
      runtime.wasmCompileErrorConstructor);
  (void)res;
  assert(res != ExecutionStatus::EXCEPTION && *res);

  res = JSObject::defineOwnProperty(
      lv.wasmObj,
      runtime,
      Predefined::getSymbolID(Predefined::LinkError),
      dpf,
      runtime.wasmLinkErrorConstructor);
  (void)res;
  assert(res != ExecutionStatus::EXCEPTION && *res);

  res = JSObject::defineOwnProperty(
      lv.wasmObj,
      runtime,
      Predefined::getSymbolID(Predefined::RuntimeError),
      dpf,
      runtime.wasmRuntimeErrorConstructor);
  (void)res;
  assert(res != ExecutionStatus::EXCEPTION && *res);

  // Register static methods.
  defineMethod(
      runtime,
      lv.wasmObj,
      Predefined::getSymbolID(Predefined::validate),
      nullptr,
      wasmValidate,
      1);

  defineMethod(
      runtime,
      lv.wasmObj,
      Predefined::getSymbolID(Predefined::compile),
      nullptr,
      wasmCompile,
      1);

  defineMethod(
      runtime,
      lv.wasmObj,
      Predefined::getSymbolID(Predefined::instantiate),
      nullptr,
      wasmInstantiate,
      1);

  // --- WebAssembly.Module constructor ---
  Handle<JSObject> modulePrototype{runtime.wasmModulePrototype};

  // Set @@toStringTag on the Module prototype.
  {
    auto tagDpf = DefinePropertyFlags::getDefaultNewPropertyFlags();
    tagDpf.writable = 0;
    tagDpf.enumerable = 0;

    defineProperty(
        runtime,
        modulePrototype,
        Predefined::getSymbolID(Predefined::SymbolToStringTag),
        runtime.getPredefinedStringHandle(Predefined::Module),
        tagDpf);
  }

  lv.moduleCons = NativeConstructor::create(
      runtime,
      Handle<JSObject>::vmcast(&runtime.functionPrototype),
      nullptr,
      wasmModuleConstructor,
      1);

  auto st = Callable::defineNameLengthAndPrototype(
      lv.moduleCons,
      runtime,
      Predefined::getSymbolID(Predefined::Module),
      1,
      modulePrototype,
      Callable::WritablePrototype::No);
  (void)st;
  assert(
      st != ExecutionStatus::EXCEPTION &&
      "defineNameLengthAndPrototype failed");

  runtime.wasmModuleConstructor.castAndSetHermesValue<NativeConstructor>(
      lv.moduleCons.getHermesValue());

  // Register Module.exports and Module.imports as static methods on the
  // constructor (not on the prototype).
  defineMethod(
      runtime,
      lv.moduleCons,
      Predefined::getSymbolID(Predefined::exports),
      nullptr,
      wasmModuleExports,
      1);

  defineMethod(
      runtime,
      lv.moduleCons,
      Predefined::getSymbolID(Predefined::imports),
      nullptr,
      wasmModuleImports,
      1);

  // Register Module constructor as a property of WebAssembly.
  res = JSObject::defineOwnProperty(
      lv.wasmObj,
      runtime,
      Predefined::getSymbolID(Predefined::Module),
      dpf,
      runtime.wasmModuleConstructor);
  (void)res;
  assert(res != ExecutionStatus::EXCEPTION && *res);

  // --- WebAssembly.Instance constructor ---
  Handle<JSObject> instancePrototype{runtime.wasmInstancePrototype};

  // Set @@toStringTag on the Instance prototype.
  {
    auto tagDpf = DefinePropertyFlags::getDefaultNewPropertyFlags();
    tagDpf.writable = 0;
    tagDpf.enumerable = 0;

    defineProperty(
        runtime,
        instancePrototype,
        Predefined::getSymbolID(Predefined::SymbolToStringTag),
        runtime.getPredefinedStringHandle(Predefined::Instance),
        tagDpf);
  }

  lv.instanceCons = NativeConstructor::create(
      runtime,
      Handle<JSObject>::vmcast(&runtime.functionPrototype),
      nullptr,
      wasmInstanceConstructor,
      1);

  st = Callable::defineNameLengthAndPrototype(
      lv.instanceCons,
      runtime,
      Predefined::getSymbolID(Predefined::Instance),
      1,
      instancePrototype,
      Callable::WritablePrototype::No);
  (void)st;
  assert(
      st != ExecutionStatus::EXCEPTION &&
      "defineNameLengthAndPrototype failed");

  runtime.wasmInstanceConstructor.castAndSetHermesValue<NativeConstructor>(
      lv.instanceCons.getHermesValue());

  // Register Instance constructor as a property of WebAssembly.
  res = JSObject::defineOwnProperty(
      lv.wasmObj,
      runtime,
      Predefined::getSymbolID(Predefined::Instance),
      dpf,
      runtime.wasmInstanceConstructor);
  (void)res;
  assert(res != ExecutionStatus::EXCEPTION && *res);

  // --- WebAssembly.Memory constructor ---
  Handle<JSObject> memoryPrototype{runtime.wasmMemoryPrototype};

  // Set @@toStringTag on the Memory prototype.
  {
    auto tagDpf = DefinePropertyFlags::getDefaultNewPropertyFlags();
    tagDpf.writable = 0;
    tagDpf.enumerable = 0;

    defineProperty(
        runtime,
        memoryPrototype,
        Predefined::getSymbolID(Predefined::SymbolToStringTag),
        runtime.getPredefinedStringHandle(Predefined::Memory),
        tagDpf);
  }

  // Define "buffer" as a getter on the prototype.
  defineAccessor(
      runtime,
      memoryPrototype,
      Predefined::getSymbolID(Predefined::buffer),
      nullptr,
      wasmMemoryBufferGetter,
      nullptr,
      false,
      true);

  // Define "grow" as a method on the prototype.
  defineMethod(
      runtime,
      memoryPrototype,
      Predefined::getSymbolID(Predefined::grow),
      nullptr,
      wasmMemoryGrowMethod,
      1);

  lv.memoryCons = NativeConstructor::create(
      runtime,
      Handle<JSObject>::vmcast(&runtime.functionPrototype),
      nullptr,
      wasmMemoryConstructor,
      1);

  st = Callable::defineNameLengthAndPrototype(
      lv.memoryCons,
      runtime,
      Predefined::getSymbolID(Predefined::Memory),
      1,
      memoryPrototype,
      Callable::WritablePrototype::No);
  (void)st;
  assert(
      st != ExecutionStatus::EXCEPTION &&
      "defineNameLengthAndPrototype failed");

  runtime.wasmMemoryConstructor.castAndSetHermesValue<NativeConstructor>(
      lv.memoryCons.getHermesValue());

  // Register Memory constructor as a property of WebAssembly.
  res = JSObject::defineOwnProperty(
      lv.wasmObj,
      runtime,
      Predefined::getSymbolID(Predefined::Memory),
      dpf,
      runtime.wasmMemoryConstructor);
  (void)res;
  assert(res != ExecutionStatus::EXCEPTION && *res);

  // --- WebAssembly.Table constructor ---
  Handle<JSObject> tablePrototype{runtime.wasmTablePrototype};

  // Set @@toStringTag on the Table prototype.
  {
    auto tagDpf = DefinePropertyFlags::getDefaultNewPropertyFlags();
    tagDpf.writable = 0;
    tagDpf.enumerable = 0;

    defineProperty(
        runtime,
        tablePrototype,
        Predefined::getSymbolID(Predefined::SymbolToStringTag),
        runtime.getPredefinedStringHandle(Predefined::Table),
        tagDpf);
  }

  // Define "length" as a getter on the prototype.
  defineAccessor(
      runtime,
      tablePrototype,
      Predefined::getSymbolID(Predefined::length),
      nullptr,
      wasmTableLengthGetter,
      nullptr,
      false,
      true);

  // Define "get" as a method on the prototype.
  defineMethod(
      runtime,
      tablePrototype,
      Predefined::getSymbolID(Predefined::get),
      nullptr,
      wasmTableGetMethod,
      1);

  // Define "set" as a method on the prototype.
  defineMethod(
      runtime,
      tablePrototype,
      Predefined::getSymbolID(Predefined::set),
      nullptr,
      wasmTableSetMethod,
      2);

  // Define "grow" as a method on the prototype.
  defineMethod(
      runtime,
      tablePrototype,
      Predefined::getSymbolID(Predefined::grow),
      nullptr,
      wasmTableGrowMethod,
      1);

  lv.tableCons = NativeConstructor::create(
      runtime,
      Handle<JSObject>::vmcast(&runtime.functionPrototype),
      nullptr,
      wasmTableConstructor,
      1);

  st = Callable::defineNameLengthAndPrototype(
      lv.tableCons,
      runtime,
      Predefined::getSymbolID(Predefined::Table),
      1,
      tablePrototype,
      Callable::WritablePrototype::No);
  (void)st;
  assert(
      st != ExecutionStatus::EXCEPTION &&
      "defineNameLengthAndPrototype failed");

  runtime.wasmTableConstructor.castAndSetHermesValue<NativeConstructor>(
      lv.tableCons.getHermesValue());

  // Register Table constructor as a property of WebAssembly.
  res = JSObject::defineOwnProperty(
      lv.wasmObj,
      runtime,
      Predefined::getSymbolID(Predefined::Table),
      dpf,
      runtime.wasmTableConstructor);
  (void)res;
  assert(res != ExecutionStatus::EXCEPTION && *res);

  // --- WebAssembly.Global constructor ---
  Handle<JSObject> globalPrototype{runtime.wasmGlobalPrototype};

  // Set @@toStringTag on the Global prototype.
  {
    auto tagDpf = DefinePropertyFlags::getDefaultNewPropertyFlags();
    tagDpf.writable = 0;
    tagDpf.enumerable = 0;

    defineProperty(
        runtime,
        globalPrototype,
        Predefined::getSymbolID(Predefined::SymbolToStringTag),
        runtime.getPredefinedStringHandle(Predefined::Global),
        tagDpf);
  }

  // Define "value" as a getter/setter on the prototype.
  defineAccessor(
      runtime,
      globalPrototype,
      Predefined::getSymbolID(Predefined::value),
      nullptr,
      wasmGlobalValueGetter,
      wasmGlobalValueSetter,
      false,
      true);

  // Define "valueOf" as a method on the prototype.
  defineMethod(
      runtime,
      globalPrototype,
      Predefined::getSymbolID(Predefined::valueOf),
      nullptr,
      wasmGlobalValueOfMethod,
      0);

  lv.globalCons = NativeConstructor::create(
      runtime,
      Handle<JSObject>::vmcast(&runtime.functionPrototype),
      nullptr,
      wasmGlobalConstructor,
      1);

  st = Callable::defineNameLengthAndPrototype(
      lv.globalCons,
      runtime,
      Predefined::getSymbolID(Predefined::Global),
      1,
      globalPrototype,
      Callable::WritablePrototype::No);
  (void)st;
  assert(
      st != ExecutionStatus::EXCEPTION &&
      "defineNameLengthAndPrototype failed");

  runtime.wasmGlobalConstructor.castAndSetHermesValue<NativeConstructor>(
      lv.globalCons.getHermesValue());

  // Register Global constructor as a property of WebAssembly.
  res = JSObject::defineOwnProperty(
      lv.wasmObj,
      runtime,
      Predefined::getSymbolID(Predefined::Global),
      dpf,
      runtime.wasmGlobalConstructor);
  (void)res;
  assert(res != ExecutionStatus::EXCEPTION && *res);

  // --- WebAssembly.Tag constructor ---
  Handle<JSObject> tagPrototype{runtime.wasmTagPrototype};

  // Set @@toStringTag on the Tag prototype.
  {
    auto tagDpf = DefinePropertyFlags::getDefaultNewPropertyFlags();
    tagDpf.writable = 0;
    tagDpf.enumerable = 0;

    defineProperty(
        runtime,
        tagPrototype,
        Predefined::getSymbolID(Predefined::SymbolToStringTag),
        runtime.getPredefinedStringHandle(Predefined::Tag),
        tagDpf);
  }

  lv.tagCons = NativeConstructor::create(
      runtime,
      Handle<JSObject>::vmcast(&runtime.functionPrototype),
      nullptr,
      wasmTagConstructor,
      1);

  st = Callable::defineNameLengthAndPrototype(
      lv.tagCons,
      runtime,
      Predefined::getSymbolID(Predefined::Tag),
      1,
      tagPrototype,
      Callable::WritablePrototype::No);
  (void)st;
  assert(
      st != ExecutionStatus::EXCEPTION &&
      "defineNameLengthAndPrototype failed");

  runtime.wasmTagConstructor.castAndSetHermesValue<NativeConstructor>(
      lv.tagCons.getHermesValue());

  // Register Tag constructor as a property of WebAssembly.
  res = JSObject::defineOwnProperty(
      lv.wasmObj,
      runtime,
      Predefined::getSymbolID(Predefined::Tag),
      dpf,
      runtime.wasmTagConstructor);
  (void)res;
  assert(res != ExecutionStatus::EXCEPTION && *res);

  // --- WebAssembly.Exception constructor ---
  Handle<JSObject> exceptionPrototype{runtime.wasmExceptionPrototype};

  // Set @@toStringTag on the Exception prototype.
  {
    auto tagDpf = DefinePropertyFlags::getDefaultNewPropertyFlags();
    tagDpf.writable = 0;
    tagDpf.enumerable = 0;

    defineProperty(
        runtime,
        exceptionPrototype,
        Predefined::getSymbolID(Predefined::SymbolToStringTag),
        runtime.getPredefinedStringHandle(Predefined::Exception),
        tagDpf);
  }

  // Define "is" method on Exception prototype.
  defineMethod(
      runtime,
      exceptionPrototype,
      Predefined::getSymbolID(Predefined::is),
      nullptr,
      wasmExceptionIsMethod,
      1);

  // Define "getArg" method on Exception prototype.
  defineMethod(
      runtime,
      exceptionPrototype,
      Predefined::getSymbolID(Predefined::getArg),
      nullptr,
      wasmExceptionGetArgMethod,
      2);

  lv.exceptionCons = NativeConstructor::create(
      runtime,
      Handle<JSObject>::vmcast(&runtime.functionPrototype),
      nullptr,
      wasmExceptionConstructor,
      2);

  st = Callable::defineNameLengthAndPrototype(
      lv.exceptionCons,
      runtime,
      Predefined::getSymbolID(Predefined::Exception),
      2,
      exceptionPrototype,
      Callable::WritablePrototype::No);
  (void)st;
  assert(
      st != ExecutionStatus::EXCEPTION &&
      "defineNameLengthAndPrototype failed");

  runtime.wasmExceptionConstructor.castAndSetHermesValue<NativeConstructor>(
      lv.exceptionCons.getHermesValue());

  // Register Exception constructor as a property of WebAssembly.
  res = JSObject::defineOwnProperty(
      lv.wasmObj,
      runtime,
      Predefined::getSymbolID(Predefined::Exception),
      dpf,
      runtime.wasmExceptionConstructor);
  (void)res;
  assert(res != ExecutionStatus::EXCEPTION && *res);

  result.castAndSetHermesValue<JSObject>(lv.wasmObj.getHermesValue());
}

} // namespace vm
} // namespace hermes
