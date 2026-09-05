/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "JSLibInternal.h"

#include "hermes/VM/Operations.h"
#include "hermes/VM/StringView.h"

#include "llvh/Support/raw_ostream.h"

namespace hermes {
namespace vm {

ExecutionStatus printArgsToStream(
    Runtime &runtime,
    llvh::raw_ostream &os,
    unsigned firstArg) {
  NativeArgs args = runtime.getCurrentFrame().getNativeArgs();
  GCScope scope(runtime);
  auto marker = scope.createMarker();
  bool first = true;

  struct : public Locals {
    PinnedValue<StringPrimitive> strHandle;
  } lv;
  LocalsRAII lraii(runtime, &lv);

  for (unsigned i = firstArg, e = args.getArgCount(); i < e; ++i) {
    scope.flushToMarker(marker);
    auto res = toString_RJS(runtime, args.getArgHandle(i));
    if (res == ExecutionStatus::EXCEPTION)
      return ExecutionStatus::EXCEPTION;

    if (!first)
      os << " ";
    SmallU16String<32> tmp;
    lv.strHandle.castAndSetHermesValue<StringPrimitive>(res->getHermesValue());
    os << StringPrimitive::createStringView(runtime, lv.strHandle)
              .getUTF16Ref(tmp);
    first = false;
  }

  os << "\n";
  os.flush();
  return ExecutionStatus::RETURNED;
}

/// Convert all arguments to string and print them followed by new line.
CallResult<HermesValue> print(void *, Runtime &runtime) {
  if (LLVM_UNLIKELY(
          printArgsToStream(runtime, llvh::outs(), 0) ==
          ExecutionStatus::EXCEPTION))
    return ExecutionStatus::EXCEPTION;
  return HermesValue::encodeUndefinedValue();
}

} // namespace vm
} // namespace hermes
