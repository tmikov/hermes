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
#include "hermes/VM/JSWebAssemblyModule.h"
#include "hermes/VM/Runtime.h"
#include "hermes/WasmFrontend/WasmModuleData.h"

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
    data = ab->getDataBlock(runtime);
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
// createWebAssemblyObject
//===----------------------------------------------------------------------===//

void createWebAssemblyObject(Runtime &runtime, MutableHandle<JSObject> result) {
  struct : public Locals {
    PinnedValue<JSObject> wasmObj;
    PinnedValue<NativeConstructor> moduleCons;
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

  result.castAndSetHermesValue<JSObject>(lv.wasmObj.getHermesValue());
}

} // namespace vm
} // namespace hermes
