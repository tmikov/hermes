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
#include "hermes/BCGen/HBC/BCProvider.h"
#include "hermes/Support/Conversions.h"
#include "hermes/Support/MemoryBuffer.h"
#include "hermes/Support/UTF8.h"
#include "hermes/WasmFrontend/WasmCompile.h"
#include "hermes/WasmFrontend/WasmModuleData.h"

#include <cmath>

namespace hermes {
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
// Shared helpers for HBC detection and module info extraction
//===----------------------------------------------------------------------===//

/// Extract export/import descriptors from the JS module info object returned
/// by running the top-level Wasm bytecode. Populates the WasmModuleData
/// exportDescs and importDescs vectors.
static ExecutionStatus extractDescriptorsFromModuleInfo(
    Runtime &runtime,
    Handle<JSObject> moduleInfoObj,
    WasmModuleData &moduleData) {
  struct : public Locals {
    PinnedValue<> arr;
    PinnedValue<> elem;
    PinnedValue<> prop;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  GCScopeMarkerRAII marker{runtime};

  // Extract exportDescs.
  {
    auto res = JSObject::getNamed_RJS(
        moduleInfoObj, runtime,
        Predefined::getSymbolID(Predefined::exportDescs));
    if (LLVM_UNLIKELY(res == ExecutionStatus::EXCEPTION))
      return ExecutionStatus::EXCEPTION;
    lv.arr = std::move(*res);
  }

  if (lv.arr->isObject()) {
    auto lengthRes = JSObject::getNamed_RJS(
        Handle<JSObject>::vmcast(&lv.arr), runtime,
        Predefined::getSymbolID(Predefined::length));
    if (LLVM_UNLIKELY(lengthRes == ExecutionStatus::EXCEPTION))
      return ExecutionStatus::EXCEPTION;
    uint32_t len = 0;
    if (lengthRes->getHermesValue().isNumber())
      len = static_cast<uint32_t>(lengthRes->getHermesValue().getDouble());

    moduleData.exportDescs.resize(len);
    for (uint32_t i = 0; i < len; ++i) {
      marker.flush();
      auto elemRes = JSObject::getComputed_RJS(
          Handle<JSObject>::vmcast(&lv.arr), runtime,
          runtime.makeHandle(HermesValue::encodeTrustedNumberValue(i)));
      if (LLVM_UNLIKELY(elemRes == ExecutionStatus::EXCEPTION))
        return ExecutionStatus::EXCEPTION;
      lv.elem = std::move(*elemRes);

      if (!lv.elem->isObject())
        continue;

      // Get 'name'.
      auto nameRes = JSObject::getNamed_RJS(
          Handle<JSObject>::vmcast(&lv.elem), runtime,
          Predefined::getSymbolID(Predefined::name));
      if (LLVM_UNLIKELY(nameRes == ExecutionStatus::EXCEPTION))
        return ExecutionStatus::EXCEPTION;
      lv.prop = std::move(*nameRes);
      if (auto *str = dyn_vmcast<StringPrimitive>(*lv.prop)) {
        llvh::SmallVector<char16_t, 32> buf;
        str->appendUTF16String(buf);
        std::string utf8;
        for (auto ch : buf)
          utf8.push_back(static_cast<char>(ch));
        moduleData.exportDescs[i].name = std::move(utf8);
      }

      // Get 'kind'.
      auto kindRes = JSObject::getNamed_RJS(
          Handle<JSObject>::vmcast(&lv.elem), runtime,
          Predefined::getSymbolID(Predefined::kind));
      if (LLVM_UNLIKELY(kindRes == ExecutionStatus::EXCEPTION))
        return ExecutionStatus::EXCEPTION;
      lv.prop = std::move(*kindRes);
      if (auto *str = dyn_vmcast<StringPrimitive>(*lv.prop)) {
        llvh::SmallVector<char16_t, 32> buf;
        str->appendUTF16String(buf);
        std::string utf8;
        for (auto ch : buf)
          utf8.push_back(static_cast<char>(ch));
        moduleData.exportDescs[i].kind = std::move(utf8);
      }
    }
  }

  // Extract importDescs.
  {
    auto res = JSObject::getNamed_RJS(
        moduleInfoObj, runtime,
        Predefined::getSymbolID(Predefined::importDescs));
    if (LLVM_UNLIKELY(res == ExecutionStatus::EXCEPTION))
      return ExecutionStatus::EXCEPTION;
    lv.arr = std::move(*res);
  }

  if (lv.arr->isObject()) {
    auto lengthRes = JSObject::getNamed_RJS(
        Handle<JSObject>::vmcast(&lv.arr), runtime,
        Predefined::getSymbolID(Predefined::length));
    if (LLVM_UNLIKELY(lengthRes == ExecutionStatus::EXCEPTION))
      return ExecutionStatus::EXCEPTION;
    uint32_t len = 0;
    if (lengthRes->getHermesValue().isNumber())
      len = static_cast<uint32_t>(lengthRes->getHermesValue().getDouble());

    moduleData.importDescs.resize(len);
    for (uint32_t i = 0; i < len; ++i) {
      marker.flush();
      auto elemRes = JSObject::getComputed_RJS(
          Handle<JSObject>::vmcast(&lv.arr), runtime,
          runtime.makeHandle(HermesValue::encodeTrustedNumberValue(i)));
      if (LLVM_UNLIKELY(elemRes == ExecutionStatus::EXCEPTION))
        return ExecutionStatus::EXCEPTION;
      lv.elem = std::move(*elemRes);

      if (!lv.elem->isObject())
        continue;

      // Get 'module'.
      auto modRes = JSObject::getNamed_RJS(
          Handle<JSObject>::vmcast(&lv.elem), runtime,
          Predefined::getSymbolID(Predefined::module));
      if (LLVM_UNLIKELY(modRes == ExecutionStatus::EXCEPTION))
        return ExecutionStatus::EXCEPTION;
      lv.prop = std::move(*modRes);
      if (auto *str = dyn_vmcast<StringPrimitive>(*lv.prop)) {
        llvh::SmallVector<char16_t, 32> buf;
        str->appendUTF16String(buf);
        std::string utf8;
        for (auto ch : buf)
          utf8.push_back(static_cast<char>(ch));
        moduleData.importDescs[i].module = std::move(utf8);
      }

      // Get 'name'.
      auto nameRes = JSObject::getNamed_RJS(
          Handle<JSObject>::vmcast(&lv.elem), runtime,
          Predefined::getSymbolID(Predefined::name));
      if (LLVM_UNLIKELY(nameRes == ExecutionStatus::EXCEPTION))
        return ExecutionStatus::EXCEPTION;
      lv.prop = std::move(*nameRes);
      if (auto *str = dyn_vmcast<StringPrimitive>(*lv.prop)) {
        llvh::SmallVector<char16_t, 32> buf;
        str->appendUTF16String(buf);
        std::string utf8;
        for (auto ch : buf)
          utf8.push_back(static_cast<char>(ch));
        moduleData.importDescs[i].name = std::move(utf8);
      }

      // Get 'kind'.
      auto kindRes = JSObject::getNamed_RJS(
          Handle<JSObject>::vmcast(&lv.elem), runtime,
          Predefined::getSymbolID(Predefined::kind));
      if (LLVM_UNLIKELY(kindRes == ExecutionStatus::EXCEPTION))
        return ExecutionStatus::EXCEPTION;
      lv.prop = std::move(*kindRes);
      if (auto *str = dyn_vmcast<StringPrimitive>(*lv.prop)) {
        llvh::SmallVector<char16_t, 32> buf;
        str->appendUTF16String(buf);
        std::string utf8;
        for (auto ch : buf)
          utf8.push_back(static_cast<char>(ch));
        moduleData.importDescs[i].kind = std::move(utf8);
      }
    }
  }

  return ExecutionStatus::RETURNED;
}

/// How a byte buffer handed to a WebAssembly entry point is interpreted.
enum class WasmBytesMode {
  /// Spec entry (Module/compile/instantiate). Treat as .wasm unless the
  /// content-sniffing gate is on AND the bytes are .hbc, which additionally
  /// requires the untrusted-bytecode gate; a detected-but-ungated .hbc is
  /// refused with a CompileError.
  SpecEntry,
  /// Trusted bytecode from the embedder (fromHermesURL). Always loaded as
  /// bytecode; never sniffed; not gated.
  TrustedBytecode,
  /// Explicit untrusted bytecode from JS (fromHermesBytecode). Always loaded
  /// as bytecode; the caller has already checked the untrusted gate.
  UntrustedBytecode,
};

/// Shared helper: create a WasmModuleData from raw bytes.
/// \p mode says how the bytes are to be interpreted; the bytes are never
/// content-sniffed unless \p mode is SpecEntry and the embedder has opted
/// into sniffing. Compiles/loads accordingly, runs the lightweight top-level
/// to extract descriptors, and returns a populated WasmModuleData.
/// Returns nullptr on error (errorMsg is set, or an exception is thrown).
static std::unique_ptr<WasmModuleData> createModuleFromBytes(
    Runtime &runtime,
    const uint8_t *data,
    size_t size,
    WasmBytesMode mode,
    std::string &errorMsg) {
  std::shared_ptr<hbc::BCProviderBase> bcProvider;

  bool loadAsBytecode = false;
  switch (mode) {
    case WasmBytesMode::TrustedBytecode:
    case WasmBytesMode::UntrustedBytecode:
      loadAsBytecode = true;
      break;
    case WasmBytesMode::SpecEntry:
      // Only ever treat spec-entry bytes as bytecode when the embedder has
      // explicitly opted into BOTH sniffing and untrusted bytecode; otherwise
      // the bytes are .wasm (or a refused .hbc).
      if (runtime.enableWasmBytecodeContentSniffing &&
          hbc::BCProviderFromBuffer::isBytecodeStream(
              llvh::ArrayRef<uint8_t>(data, size))) {
        if (!runtime.enableUntrustedBytecodeFromJS) {
          errorMsg =
              "refusing to load Hermes bytecode: untrusted bytecode from JS "
              "is disabled";
          return nullptr;
        }
        loadAsBytecode = true;
      } else {
        loadAsBytecode = false;
      }
      break;
  }

  if (loadAsBytecode) {
    // Precompiled HBC path — load directly.
    auto llvmBuf = llvh::MemoryBuffer::getMemBufferCopy(
        llvh::StringRef(reinterpret_cast<const char *>(data), size));
    auto ret = hbc::BCProviderFromBuffer::createBCProviderFromBuffer(
        std::make_unique<OwnedMemoryBuffer>(std::move(llvmBuf)));
    if (!ret.first) {
      errorMsg = ret.second.empty()
          ? "invalid HBC bytecode" : std::string(ret.second);
      return nullptr;
    }
    bcProvider = std::shared_ptr<hbc::BCProviderBase>(std::move(ret.first));
  } else {
    // .wasm path — compile to HBC first.
    auto compiledData = hermes::compileWasmToModuleData(
        data, size, errorMsg, runtime.test262);
    if (!compiledData) {
      return nullptr;
    }
    bcProvider = compiledData->bytecodeProvider;
  }

  // Run the lightweight top-level to extract descriptors.
  auto bcCopy = bcProvider;
  auto runRes = runtime.runBytecode(
      std::move(bcCopy),
      RuntimeModuleFlags{},
      "wasm-module",
      Runtime::makeNullHandle<Environment>());

  if (LLVM_UNLIKELY(runRes == ExecutionStatus::EXCEPTION)) {
    errorMsg = "failed to run Wasm module top-level";
    return nullptr;
  }

  if (!runRes->isObject()) {
    errorMsg = "Wasm module top-level did not return an object";
    return nullptr;
  }

  auto moduleData = std::make_unique<WasmModuleData>();
  moduleData->bytecodeProvider = bcProvider;

  struct : public Locals {
    PinnedValue<JSObject> moduleInfoObj;
  } lv;
  LocalsRAII lraii(runtime, &lv);
  lv.moduleInfoObj.castAndSetHermesValue<JSObject>(*runRes);

  if (LLVM_UNLIKELY(
          extractDescriptorsFromModuleInfo(
              runtime, lv.moduleInfoObj, *moduleData) ==
          ExecutionStatus::EXCEPTION)) {
    errorMsg = "failed to extract descriptors from module info";
    return nullptr;
  }

  return moduleData;
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

/// WebAssembly.compile(bytes) — compile a Wasm binary or load .hbc
/// asynchronously. Since Hermes doesn't do async compilation, this is
/// synchronous compilation wrapped in a resolved Promise. On error, returns
/// a rejected Promise.
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

  // Spec entry: the bytes are .wasm unless the embedder gates say otherwise.
  std::string errorMsg;
  auto moduleData = createModuleFromBytes(
      runtime, data, size, WasmBytesMode::SpecEntry, errorMsg);
  if (!moduleData) {
    // Create a CompileError and return a rejected Promise.
    if (runtime.getThrownValue().isEmpty()) {
      raiseCompileError(runtime, errorMsg.c_str());
    }
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
    PinnedValue<> moduleInfoObj;
    PinnedValue<> instantiateFn;
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

  // Run the compiled bytecode top-level to get the module info object.
  // The top-level returns {instantiate, exportDescs, importDescs}.
  auto bcProvider = moduleData->bytecodeProvider;
  auto runRes = runtime.runBytecode(
      std::move(bcProvider),
      RuntimeModuleFlags{},
      "wasm-module",
      Runtime::makeNullHandle<Environment>());

  if (LLVM_UNLIKELY(runRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }

  lv.moduleInfoObj = std::move(*runRes);

  if (!lv.moduleInfoObj->isObject()) {
    raiseLinkError(
        runtime, "WebAssembly instantiation failed: unexpected result");
    return ExecutionStatus::EXCEPTION;
  }

  // Extract the instantiate closure from the module info object.
  {
    auto instRes = JSObject::getNamed_RJS(
        Handle<JSObject>::vmcast(&lv.moduleInfoObj),
        runtime,
        Predefined::getSymbolID(Predefined::instantiate));
    if (LLVM_UNLIKELY(instRes == ExecutionStatus::EXCEPTION)) {
      return ExecutionStatus::EXCEPTION;
    }
    lv.instantiateFn = std::move(*instRes);
  }

  if (!vmisa<Callable>(*lv.instantiateFn)) {
    raiseLinkError(
        runtime,
        "WebAssembly instantiation failed: instantiate is not callable");
    return ExecutionStatus::EXCEPTION;
  }

  // Call instantiate(imports) to perform initialization and get the exports
  // object. The import object is passed as an argument rather than through a
  // global: a global is observable and replaceable by any script running
  // during instantiation -- an import-object getter or a Proxy trap -- and it
  // makes instantiating one module twice with different imports impossible to
  // express.
  auto callRes = Callable::executeCall1(
      Handle<Callable>::vmcast(&lv.instantiateFn),
      runtime,
      Runtime::getUndefinedValue(),
      hasImports ? importObj.getHermesValue()
                 : HermesValue::encodeUndefinedValue());

  if (LLVM_UNLIKELY(callRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }

  if (!callRes->getHermesValue().isObject()) {
    raiseLinkError(
        runtime, "WebAssembly instantiation failed: unexpected result");
    return ExecutionStatus::EXCEPTION;
  }

  lv.exportsObj.castAndSetHermesValue<JSObject>(callRes->getHermesValue());

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

  // Spec entry: the bytes are .wasm unless the embedder gates say otherwise.
  std::string errorMsg;
  auto moduleData = createModuleFromBytes(
      runtime, data, size, WasmBytesMode::SpecEntry, errorMsg);
  if (!moduleData) {
    if (runtime.getThrownValue().isEmpty()) {
      raiseCompileError(runtime, errorMsg.c_str());
    }
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

/// Interpret \p data / \p size according to \p mode and wrap the result in a
/// new WebAssembly.Module JS object. On failure raises a CompileError (unless
/// an exception is already pending) and returns EXCEPTION. Shared by the
/// Module constructor and the explicit Module.fromXXX entry points, which
/// differ only in the mode they pass. The returned value is unrooted --
/// callers must return it directly, not store it.
static CallResult<HermesValue> createAndBuildModule(
    Runtime &runtime,
    const uint8_t *data,
    size_t size,
    WasmBytesMode mode) {
  std::string errorMsg;
  auto moduleData = createModuleFromBytes(runtime, data, size, mode, errorMsg);
  if (!moduleData) {
    if (runtime.getThrownValue().isEmpty()) {
      raiseCompileError(runtime, errorMsg.c_str());
    }
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

/// new WebAssembly.Module(bytes) — compile a Wasm binary module or load
/// a precompiled .hbc module.
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

  // Spec entry: the bytes are .wasm unless the embedder gates say otherwise.
  return createAndBuildModule(runtime, data, size, WasmBytesMode::SpecEntry);
}

/// WebAssembly.Module.fromHermesBytecode(bytes) — load a precompiled Hermes
/// bytecode module. The caller declares that the bytes are .hbc, so nothing
/// is inferred from their content. Gated by EnableUntrustedBytecodeFromJS:
/// the bytes come from JS, and bytecode runs with full VM authority without
/// the checks the compiler would otherwise guarantee.
static CallResult<HermesValue>
wasmModuleFromHermesBytecode(void *context, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();

  if (!runtime.enableUntrustedBytecodeFromJS) {
    return runtime.raiseTypeError(
        "WebAssembly.Module.fromHermesBytecode() is disabled "
        "(EnableUntrustedBytecodeFromJS is off)");
  }

  const uint8_t *data = nullptr;
  size_t size = 0;
  if (!extractBufferSourceBytes(runtime, args.getArgHandle(0), data, size)) {
    return runtime.raiseTypeError(
        "WebAssembly.Module.fromHermesBytecode(): argument must be an "
        "ArrayBuffer or typed array");
  }

  // The caller declares the bytes to be Hermes bytecode, so they are loaded
  // as bytecode without sniffing. The untrusted gate was checked above.
  return createAndBuildModule(
      runtime, data, size, WasmBytesMode::UntrustedBytecode);
}

/// WebAssembly.Module.fromHermesURL(url) — resolve \p url to trusted Hermes
/// bytecode via the embedder-installed resolver and load it. The bytes never
/// pass through JS, so there is nothing for JS to falsify: this entry point is
/// not config-gated, and its authorization is simply that the embedder
/// installed a resolver which produced bytes for the URL. Never sniffs, never
/// compiles .wasm.
static CallResult<HermesValue>
wasmModuleFromHermesURL(void *context, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();

  struct : public Locals {
    PinnedValue<StringPrimitive> urlStr;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  // ToString(url). This may allocate and may run user JS (a toString method),
  // so the result is pinned before anything else happens.
  auto strRes = toString_RJS(runtime, args.getArgHandle(0));
  if (LLVM_UNLIKELY(strRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  lv.urlStr = std::move(*strRes);

  llvh::SmallVector<char16_t, 64> urlBuf;
  lv.urlStr->appendUTF16String(urlBuf);
  std::string url;
  convertUTF16ToUTF8WithReplacements(url, urlBuf);

  // A copy, not a reference into the Runtime member: this calls out to
  // embedder code, and the member must not be the only thing keeping the
  // callable alive while it runs. Installation is one-shot today, so nothing
  // can reassign it -- but that invariant lives in the API layer, and this
  // copy makes the VM side not depend on it.
  auto resolver = runtime.getWasmModuleResolver();
  std::string bytecode;
  std::string resolverError;
  if (!resolver || !resolver(url, bytecode, resolverError)) {
    // Name the URL, and the reason if the embedder offered one; without both,
    // "no module for URL" cannot distinguish a missing resolver from one that
    // declined from a registry miss.
    std::string msg = "WebAssembly.Module.fromHermesURL: no module for URL '";
    msg += url;
    msg += '\'';
    if (!resolver) {
      msg += ": no Wasm module resolver installed";
    } else if (!resolverError.empty()) {
      msg += ": ";
      msg += resolverError;
    }
    return runtime.raiseTypeError(llvh::StringRef(msg));
  }

  // The embedder declares these bytes to be Hermes bytecode and is trusted to
  // do so, so they are loaded as bytecode without sniffing.
  return createAndBuildModule(
      runtime,
      reinterpret_cast<const uint8_t *>(bytecode.data()),
      bytecode.size(),
      WasmBytesMode::TrustedBytecode);
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

  // Read "maximum" property (optional). UINT32_MAX is the "no explicit
  // maximum" sentinel; growth is capped at 65536 pages regardless.
  uint32_t maxPages = UINT32_MAX;
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

  // NOTHING IS PUBLISHED ON THE MEMORY. It used to carry three ordinary,
  // writable, enumerable own properties -- __wasm_type__, __wasm_min__ and
  // __wasm_max__ -- which the link path read to decide whether a memory
  // import was satisfied. That had three consequences, all closed here:
  //
  //   * a plain object literal carrying those names described itself as a
  //     memory, and so did any object INHERITING from a real one;
  //   * __wasm_min__ was a snapshot of the size at construction that grow()
  //     never updated, so a memory grown from one page to two still claimed
  //     a minimum of one and failed to satisfy a (memory 2) import (H7);
  //   * the writes went through putNamed_RJS, which walks the prototype
  //     chain, so a setter on WebAssembly.Memory.prototype ran arbitrary user
  //     JS inside this constructor on a half-built object (H2).
  //
  // The link path now reads buffer_ and maxPages_ through the wasmLinkMemory
  // builtin, whose dyn_vmcast is the brand check that replaced the
  // __wasm_type__ string comparison, and whose size comes from the buffer --
  // so it cannot go stale after a grow. A WebAssembly.Memory now has no own
  // properties at all, which is also what the spec requires of it.

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

/// Fetch a Table's three parallel backing arrays: the internal closures that
/// call_indirect calls, their interned type ids that it checks, and the
/// Exported Functions that every JS boundary crossing sees. Every method that
/// touches slots needs all three, because a slot is the triple and not any one
/// of them.
///
/// The constructor creates all three together and nothing ever clears them, so
/// a table missing one is not a state script can reach; the check is here
/// because the fields are nullable and a half-built table must not be written
/// through, rather than because it is expected to fire.
/// \return false with a TypeError pending if any array is missing.
static bool wasmTableArrays(
    Runtime &runtime,
    Handle<JSWebAssemblyTable> tbl,
    PinnedValue<JSArray> &funcsArr,
    PinnedValue<JSArray> &typesArr,
    PinnedValue<JSArray> &exportedArr) {
  JSArray *funcs = tbl->getElements(runtime);
  JSArray *types = tbl->getTypes(runtime);
  JSArray *exported = tbl->getExported(runtime);
  if (LLVM_UNLIKELY(!funcs || !types || !exported)) {
    (void)runtime.raiseTypeError("WebAssembly.Table has no backing storage");
    return false;
  }
  funcsArr = funcs;
  typesArr = types;
  exportedArr = exported;
  return true;
}

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
    PinnedValue<JSArray> typesArr;
    PinnedValue<JSArray> exportedArr;
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

  // Read "maximum" property (optional). UINT32_MAX means no explicit
  // maximum; 0 is a real maximum that forbids all growth. The distinction
  // is observable: importing a {maximum: 0} table as (table 0 0 funcref)
  // must link, and its metadata must say 0, not "unbounded".
  uint32_t maxSize = UINT32_MAX;
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
  lv.tbl->setElements(runtime, *lv.arr);

  // The parallel type-id array. A slot's entry is the interned Wasm type of
  // the function in it; an empty entry means "no interned type", which is what
  // makes call_indirect refuse the slot.
  auto typesRes = JSArray::create(runtime, initialSize, initialSize);
  if (LLVM_UNLIKELY(typesRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  lv.typesArr = std::move(*typesRes);
  lv.tbl->setTypes(runtime, *lv.typesArr);

  // The parallel Exported Function array. This is what table.get and
  // Table.prototype.get hand out.
  auto exportedRes = JSArray::create(runtime, initialSize, initialSize);
  if (LLVM_UNLIKELY(exportedRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  lv.exportedArr = std::move(*exportedRes);
  lv.tbl->setExported(runtime, *lv.exportedArr);

  // Clear every slot through the slot funnel, the one writer of a table array.
  // Doing it by hand would produce the same three values today, but it would
  // be a second definition of "an empty slot" sitting next to the funnel's and
  // free to drift from it; and the spec's initial value for a funcref table is
  // exactly DefaultValue(funcref), which is the null this passes.
  //
  // NO TEST CAN SEE THIS LOOP, and that is now a property of the design rather
  // than a gap. While the backing arrays were published, an explicit null and
  // a never-written hole were distinguishable through `Object.keys` and `in`.
  // They are not reachable any more, and every remaining reader maps the two
  // to the same answer: wasmCallIndirect reports "uninitialized element" for
  // either, and wasmTableGetSlot and Table.prototype.get both return null for
  // either. Deleting the loop outright leaves the whole suite green -- checked,
  // not assumed. It is kept because "the funnel is the only writer" is an
  // invariant later changes lean on, and because an externref table's arrays
  // are holes throughout, so a reader that ever starts telling the two apart
  // would be wrong about externref tables as well. Anyone removing it for its
  // instantiate-time cost should say so as a deliberate trade, not as a
  // cleanup covered by tests.
  {
    GCScopeMarkerRAII marker{runtime};
    for (uint32_t i = 0; i < initialSize; ++i) {
      marker.flush();
      if (LLVM_UNLIKELY(
              setWasmTableSlot(
                  runtime,
                  lv.arr,
                  lv.typesArr,
                  lv.exportedArr,
                  i,
                  Runtime::getNullValue(),
                  /* isFuncRef */ true) == ExecutionStatus::EXCEPTION))
        return ExecutionStatus::EXCEPTION;
    }
  }

  // NOTHING IS PUBLISHED ON THE TABLE. It used to carry six own properties --
  // __wasm_funcs__/__wasm_types__/__wasm_exported__ (the backing storage) and
  // __wasm_type__/__wasm_min__/__wasm_max__ (the metadata the link path
  // compared against) -- all of them ordinary, writable and enumerable. That
  // published the entire linking ABI to script: the arrays could be read,
  // frozen and handed to another module, the internal closures in them could
  // be called directly (which aborts the VM), the type ids could be forged to
  // defeat call_indirect's check, and an object literal carrying the same six
  // names linked as a table. A WebAssembly.Table now has no own properties at
  // all, which is also what the spec requires of it.
  //
  // The link path reads elements_/types_/exported_ and maxSize_ through the
  // wasmLinkTable builtin, whose dyn_vmcast is the brand check that replaced
  // the __wasm_type__ string comparison. The current size is the storage's
  // length, so it needs no snapshot and cannot go stale after a grow.

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
    PinnedValue<JSWebAssemblyTable> tblHandle;
    PinnedValue<JSArray> arr;
    PinnedValue<JSArray> typesArr;
    PinnedValue<JSArray> exportedArr;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  // Pin the table BEFORE anything that can run JS. toNumber_RJS below calls a
  // user valueOf, which can allocate and move the table; the raw `tbl` would
  // be stale from that point on. The set and grow methods already do this.
  lv.tblHandle = tbl;

  lv.indexVal = args.getArg(0);
  auto indexRes = toNumber_RJS(runtime, lv.indexVal);
  if (LLVM_UNLIKELY(indexRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  double indexDbl = indexRes->getDouble();

  // The length is the closure array's, but the value handed out is the
  // EXPORTED FUNCTION. Returning elements_[i] hands script the internal
  // closure, whose calling convention is the internal one -- an i64 as a
  // lo/hi pair, results through a return buffer -- so calling it with the
  // spec-legal 5n read a BigInt as a double and aborted the VM.
  //
  // This does not need the type array, but it goes through the same helper as
  // set and grow so that a table missing any of its storage reports that, and
  // reports it the same way. It used to answer "index out of bounds", which
  // named the one thing that was not wrong.
  if (LLVM_UNLIKELY(!wasmTableArrays(
          runtime, lv.tblHandle, lv.arr, lv.typesArr, lv.exportedArr)))
    return ExecutionStatus::EXCEPTION;

  uint32_t len = JSArray::getLength(*lv.arr, runtime);
  if (indexDbl < 0 || indexDbl >= len ||
      indexDbl != std::floor(indexDbl)) {
    return runtime.raiseRangeError(
        "WebAssembly.Table.prototype.get: index out of bounds");
  }
  uint32_t index = static_cast<uint32_t>(indexDbl);

  // The SAME read `table.get` uses, deliberately: see readWasmTableSlot. On a
  // WebAssembly.Table the empty case cannot arise -- the constructor clears
  // every slot through the funnel and grow fills the new ones -- but sharing
  // the read is what stops the two ever disagreeing, and it is what gives that
  // mapping a reachable test (an externref table's storage is holes).
  return readWasmTableSlot(runtime, *lv.exportedArr, index);
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
    PinnedValue<JSArray> typesArr;
    PinnedValue<JSArray> exportedArr;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  // Pin the table BEFORE anything that can run JS: toNumber_RJS below calls a
  // user valueOf, which can allocate and move the table.
  lv.tblHandle = tbl;

  lv.indexVal = args.getArg(0);
  auto indexRes = toNumber_RJS(runtime, lv.indexVal);
  if (LLVM_UNLIKELY(indexRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  double indexDbl = indexRes->getDouble();

  // ToWebAssemblyValue for funcref admits null and an Exported Function, and
  // nothing else: every other value takes the spec's host-reference branch,
  // whose type does not match `ref null func`, so it is a TypeError. A plain
  // JS function is therefore refused -- it is a host reference, not a funcref
  // -- and so is `undefined`.
  //
  // OMITTING the value is a different thing from passing `undefined`. WebIDL
  // declares it `optional any value` with no default, so an absent argument is
  // DefaultValue(funcref), which is null, while an explicit `undefined` is an
  // ordinary value that fails the check. wpt wasm/jsapi/table/get-set.any.js
  // pins both halves.
  //
  // The funnel would refuse the same values, but this check is not redundant:
  // the spec converts the value BEFORE the write, so an out-of-range index and
  // a bad value must report the value's TypeError rather than the index's
  // RangeError, and the message can name the method that the caller actually
  // called.
  if (args.getArgCount() >= 2)
    lv.funcVal = args.getArg(1);
  else
    lv.funcVal = HermesValue::encodeNullValue();
  if (!lv.funcVal->isNull() && !isWasmExportedFunction(runtime, lv.funcVal)) {
    return runtime.raiseTypeError(
        "WebAssembly.Table.prototype.set: value must be null or a "
        "WebAssembly exported function");
  }

  if (LLVM_UNLIKELY(!wasmTableArrays(
          runtime, lv.tblHandle, lv.arr, lv.typesArr, lv.exportedArr)))
    return ExecutionStatus::EXCEPTION;

  uint32_t len = JSArray::getLength(*lv.arr, runtime);
  if (indexDbl < 0 || indexDbl >= len ||
      indexDbl != std::floor(indexDbl)) {
    return runtime.raiseRangeError(
        "WebAssembly.Table.prototype.set: index out of bounds");
  }
  uint32_t index = static_cast<uint32_t>(indexDbl);

  // Through the funnel, so the closure and the interned type id are DERIVED
  // from the Exported Function rather than guessed at here. Writing the
  // wrapper and leaving the type id alone -- which is what this method used to
  // do -- left the slot's old signature in place, and call_indirect then
  // called the new function through it.
  //
  // A JS-API table is always funcref: the constructor accepts only "anyfunc"
  // and "funcref".
  if (LLVM_UNLIKELY(
          setWasmTableSlot(
              runtime,
              lv.arr,
              lv.typesArr,
              lv.exportedArr,
              index,
              lv.funcVal,
              /* isFuncRef */ true) == ExecutionStatus::EXCEPTION))
    return ExecutionStatus::EXCEPTION;

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
    PinnedValue<JSArray> arr;
    PinnedValue<JSArray> typesArr;
    PinnedValue<JSArray> exportedArr;
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

  if (LLVM_UNLIKELY(!wasmTableArrays(
          runtime, lv.tblHandle, lv.arr, lv.typesArr, lv.exportedArr)))
    return ExecutionStatus::EXCEPTION;
  uint32_t oldLen = JSArray::getLength(*lv.arr, runtime);

  uint64_t newLen64 = static_cast<uint64_t>(oldLen) + delta;
  uint32_t maxSize = lv.tblHandle->getMaxSize();
  // maxSize is UINT32_MAX when no maximum was declared, which cannot be
  // exceeded without also failing the 2^32-1 overflow check below.
  if (newLen64 > maxSize) {
    return runtime.raiseRangeError(
        "WebAssembly.Table.prototype.grow: would exceed maximum");
  }
  if (newLen64 > 0xFFFFFFFF) {
    return runtime.raiseRangeError(
        "WebAssembly.Table.prototype.grow: would exceed maximum");
  }
  // Largest table this engine will grow to; mirrors kMaxTableEntries in
  // wasmTableGrow. Without it a huge delta under no maximum is not
  // refused but attempted, filling entries for billions of iterations.
  static constexpr uint64_t kMaxTableEntries = 10'000'000;
  if (newLen64 > kMaxTableEntries) {
    return runtime.raiseRangeError(
        "WebAssembly.Table.prototype.grow: table too large");
  }
  uint32_t newLen = static_cast<uint32_t>(newLen64);

  // Grow all three IN PLACE. A module importing this table shares these very
  // array objects; replacing them here would silently disconnect the two,
  // which is exactly what the Wasm-side table.grow avoids by growing the
  // shared arrays the same way. Leaving any one of the three short
  // desynchronizes the triple, and a later write to a grown slot would land in
  // some arrays and extend others.
  //
  // The `bool` half of each CallResult is deliberately discarded here, unlike
  // the element writes in the funnel, where discarding it is the whole defect.
  // The difference is that a refused LENGTH write is caught downstream: it
  // leaves that array short, the fill loop below then writes an index the
  // array will not take, and the checked element store turns that into the
  // rollback and the RangeError. When delta is 0 no fill loop runs, but then
  // there is also nothing to write.
  //
  // NOTHING PINS THAT ANY MORE, and the reason is worth stating rather than
  // leaving to be rediscovered. It used to be pinned by three frozen-array
  // cases in e2e-table-js-methods.wat, which froze a table's backing arrays
  // through the __wasm_funcs__ publication; those publications are deleted, a
  // WebAssembly.Table's storage is internal fields script cannot name, and so
  // no JS-API table can be handed an array that refuses writes. The rollback
  // below is unreachable from this method and untested. The equivalent
  // downstream-catch on the WASM side IS reachable and is tested -- an
  // externref table's arrays come from globalThis.Array; see wasmTableGrow and
  // the frozen-storage cases in e2e-table-slot-invariant.wat.
  {
    auto lenRes = JSArray::setLengthProperty(lv.arr, runtime, newLen);
    if (LLVM_UNLIKELY(lenRes == ExecutionStatus::EXCEPTION))
      return ExecutionStatus::EXCEPTION;
    lenRes = JSArray::setLengthProperty(lv.typesArr, runtime, newLen);
    if (LLVM_UNLIKELY(lenRes == ExecutionStatus::EXCEPTION))
      return ExecutionStatus::EXCEPTION;
    lenRes = JSArray::setLengthProperty(lv.exportedArr, runtime, newLen);
    if (LLVM_UNLIKELY(lenRes == ExecutionStatus::EXCEPTION))
      return ExecutionStatus::EXCEPTION;
  }

  // Clear the new entries through the slot funnel, like every other write to
  // a table array. The fill value is null because this method does not accept
  // the spec's optional second argument, so it is always
  // DefaultValue(funcref).
  GCScopeMarkerRAII marker{runtime};
  for (uint32_t i = oldLen; i < newLen; ++i) {
    marker.flush();
    if (LLVM_UNLIKELY(
            setWasmTableSlot(
                runtime,
                lv.arr,
                lv.typesArr,
                lv.exportedArr,
                i,
                Runtime::getNullValue(),
                /* isFuncRef */ true) == ExecutionStatus::EXCEPTION)) {
      // Could not write part-way through -- out of memory, or one of the
      // arrays refusing writes because script froze it. Put the table back to
      // its old length and report it the way the spec reports a failed grow,
      // as a RangeError; returning with a foreign exception pending would
      // leave a table grown but not filled.
      //
      // Best-effort cleanup on a path that is already reporting failure: an
      // array that refuses to shrink is one that never grew, so ignoring the
      // result cannot leave the table longer than it started.
      runtime.clearThrownValue();
      (void)JSArray::setLengthProperty(lv.arr, runtime, oldLen);
      (void)JSArray::setLengthProperty(lv.typesArr, runtime, oldLen);
      (void)JSArray::setLengthProperty(lv.exportedArr, runtime, oldLen);
      return runtime.raiseRangeError(
          "WebAssembly.Table.prototype.grow: could not allocate");
    }
  }

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
  // An i64 global takes a BigInt, not a Number: a double cannot represent
  // every i64 exactly, and the spec defines Global.prototype.value as a
  // BigInt for i64.
  double initValue = 0.0;
  int64_t initI64 = 0;
  if (valType == JSWebAssemblyGlobal::ValType::I64) {
    if (args.getArgCount() >= 2) {
      lv.initVal = args.getArg(1);
      if (!lv.initVal->isBigInt()) {
        return runtime.raiseTypeError(
            "WebAssembly.Global(): an i64 global requires a BigInt value");
      }
      initI64 = static_cast<int64_t>(
          lv.initVal->getBigInt()->truncateToSingleDigit());
    }
  } else if (args.getArgCount() >= 2) {
    lv.initVal = args.getArg(1);
    auto initRes = toNumber_RJS(runtime, lv.initVal);
    if (LLVM_UNLIKELY(initRes == ExecutionStatus::EXCEPTION)) {
      return ExecutionStatus::EXCEPTION;
    }
    initValue = initRes->getDouble();
  }

  // Create the Global object.
  Handle<JSObject> globalPrototype{runtime.wasmGlobalPrototype};
  lv.glob = JSWebAssemblyGlobal::create(runtime, globalPrototype);
  lv.glob->setValType(valType);
  lv.glob->setMutable(isMutable);
  lv.glob->setI64Value(initI64);
  // The type must already be set: setWasmGlobalNumber coerces to it, and it
  // is the one place value_ is written, so an i32 global never holds a
  // fractional double however it was constructed. i64 keeps initValue at 0
  // and carries its value in i64Value_ above.
  if (valType != JSWebAssemblyGlobal::ValType::I64)
    setWasmGlobalNumber(lv.glob.get(), initValue);

  // NOTHING IS PUBLISHED ON THE GLOBAL. It used to carry one ordinary,
  // writable, enumerable own property, __wasm_type__, holding a string such
  // as "global:i32:const" that the link path compared against the importing
  // module's declaration. Because that was the whole of the check, an object
  // literal carrying the right string and a `value` LINKED as a global and
  // handed the module its own `value` -- the only one of the three kinds
  // where a plain forgery succeeded outright. The write also went through
  // putNamed_RJS, which walks the prototype chain, so a setter on
  // WebAssembly.Global.prototype ran user JS inside this constructor (H2).
  //
  // The link path now reads valType_/mutable_ and the value itself through
  // the wasmLinkGlobal builtin, whose dyn_vmcast is the brand check that
  // replaced the string comparison. A WebAssembly.Global now has no own
  // properties at all, which is also what the spec requires of it.

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

  if (glob->getValType() == JSWebAssemblyGlobal::ValType::I64) {
    // Exact, and a BigInt as the spec requires. Returning the low 32 bits as
    // a Number would silently discard the upper half.
    return BigIntPrimitive::fromSigned(runtime, glob->getI64Value());
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
  if (glob->getValType() == JSWebAssemblyGlobal::ValType::I64) {
    if (!lv.newVal->isBigInt()) {
      return runtime.raiseTypeError(
          "WebAssembly.Global.prototype.value: an i64 global requires a "
          "BigInt value");
    }
    glob->setI64Value(
        static_cast<int64_t>(lv.newVal->getBigInt()->truncateToSingleDigit()));
    return HermesValue::encodeUndefinedValue();
  }
  auto numRes = toNumber_RJS(runtime, lv.newVal);
  if (LLVM_UNLIKELY(numRes == ExecutionStatus::EXCEPTION)) {
    return ExecutionStatus::EXCEPTION;
  }
  // toNumber_RJS is a safepoint and `glob` is a raw pointer, so re-derive it
  // rather than trusting the one taken before the call.
  glob = vmcast<JSWebAssemblyGlobal>(args.getThisArg());
  setWasmGlobalNumber(glob, numRes->getDouble());
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

  defineMethod(
      runtime,
      lv.moduleCons,
      Predefined::getSymbolID(Predefined::fromHermesBytecode),
      nullptr,
      wasmModuleFromHermesBytecode,
      1);

  defineMethod(
      runtime,
      lv.moduleCons,
      Predefined::getSymbolID(Predefined::fromHermesURL),
      nullptr,
      wasmModuleFromHermesURL,
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
