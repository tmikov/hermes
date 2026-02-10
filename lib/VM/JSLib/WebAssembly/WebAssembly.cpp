/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//===----------------------------------------------------------------------===//
/// \file
/// Initialize the WebAssembly namespace object, its error types
/// (CompileError, LinkError, RuntimeError), and static methods
/// (validate, compile, instantiate).
//===----------------------------------------------------------------------===//

#include "../JSLibInternal.h"

#include "hermes/VM/JSArrayBuffer.h"
#include "hermes/VM/JSError.h"
#include "hermes/VM/JSTypedArray.h"
#include "hermes/VM/Runtime.h"

namespace hermes {

/// Weak declaration of validateWasmBinary from WasmFrontend.
/// When the WasmFrontend library is linked (full VM), this resolves to the
/// real implementation. When it's not linked (lean VM), it resolves to the
/// weak default which returns false.
__attribute__((__weak__)) bool
validateWasmBinary(const uint8_t *buffer, size_t size) {
  return false;
}

namespace vm {

/// Constructor function for WebAssembly error types. Works the same as
/// NativeError constructors in Error.cpp — creates a JSError with the
/// appropriate prototype.
/// \p context is a pointer to the pair of Runtime fields for the constructor
/// and prototype of this specific error type.
static CallResult<HermesValue>
wasmErrorConstructor(void *context, Runtime &runtime) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();

  // context encodes the error type index (0=CompileError, 1=LinkError,
  // 2=RuntimeError).
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

  // If this is not a construct call, or it is a construct call and new.target
  // is the error constructor, then we know what parent to use.
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

  // Record the stack trace, skipping this entry.
  JSError::recordStackTrace(lv.self, runtime, true);

  // Set the message property if the first argument is not undefined.
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

/// Create a WebAssembly error constructor and its prototype.
/// The prototype inherits from Error.prototype.
/// The constructor is NOT registered as a global — it will be a property
/// of the WebAssembly namespace object.
/// \param name the predefined string ID for the error name (e.g.,
///   Predefined::CompileError)
/// \param typeIndex the index used to dispatch in wasmErrorConstructor
///   (0=CompileError, 1=LinkError, 2=RuntimeError)
/// \param prototypeOut output: receives the prototype
/// \param constructorOut output: receives the constructor
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

  // Set the 'name' property on the prototype.
  auto defaultName = runtime.getPredefinedString(name);
  lv.nameValue = HermesValue::encodeStringValue(defaultName);
  defineProperty(
      runtime,
      prototype,
      Predefined::getSymbolID(Predefined::name),
      lv.nameValue);

  // Set the 'message' property on the prototype (empty string).
  defineProperty(
      runtime,
      prototype,
      Predefined::getSymbolID(Predefined::message),
      runtime.getPredefinedStringHandle(Predefined::emptyString));

  // Create the constructor. Use the typeIndex as the context pointer.
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

/// Extract raw bytes from a BufferSource argument (ArrayBuffer or TypedArray).
/// \param arg the JS argument value.
/// \param[out] data pointer to the first byte.
/// \param[out] size number of bytes.
/// \returns true if extraction succeeded, false if the argument is not a valid
///   BufferSource (caller should throw TypeError).
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

/// WebAssembly.validate(bytes) — validate a Wasm binary module.
/// Returns true if the bytes form a valid Wasm module, false otherwise.
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

void createWebAssemblyObject(Runtime &runtime, MutableHandle<JSObject> result) {
  struct : public Locals {
    PinnedValue<JSObject> wasmObj;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  // Create the WebAssembly namespace object.
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

  // Forward-declare prototypes (inheriting from Error.prototype).
  // This was already done in GlobalObject.cpp's initGlobalObject.

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

  // Register error constructors as properties of the WebAssembly object.
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

  result.castAndSetHermesValue<JSObject>(lv.wasmObj.getHermesValue());
}

} // namespace vm
} // namespace hermes
