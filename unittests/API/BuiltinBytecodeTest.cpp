/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/CompileJS.h"
#include "hermes/hermes.h"

#include <gtest/gtest.h>
#include <jsi/threadsafe.h>

using namespace facebook;
using namespace facebook::hermes;

namespace {

/// Compile \p source to Hermes bytecode.
std::shared_ptr<const jsi::Buffer> compile(const std::string &source) {
  std::string bytecode;
  EXPECT_TRUE(::hermes::compileJS(source, bytecode));
  return std::make_shared<jsi::StringBuffer>(std::move(bytecode));
}

/// Function.prototype.toString applied to \p fn.
std::string functionSource(jsi::Runtime &rt, const jsi::Value &fn) {
  auto toString = rt.global()
                      .getPropertyAsObject(rt, "Function")
                      .getPropertyAsObject(rt, "prototype")
                      .getPropertyAsFunction(rt, "toString");
  return toString.callWithThis(rt, fn.asObject(rt)).getString(rt).utf8(rt);
}

TEST(BuiltinBytecodeTest, ReturnsTheCompletionValue) {
  auto rt = makeHermesRuntime();
  auto *builtin = jsi::castInterface<IHermesBuiltinBytecode>(rt.get());
  ASSERT_NE(builtin, nullptr);
  jsi::Value fn = builtin->runBuiltinBytecode(
      compile("(function () { return 41 + 1; })"), "test.js");
  EXPECT_EQ(fn.asObject(*rt).asFunction(*rt).call(*rt).getNumber(), 42);
}

TEST(BuiltinBytecodeTest, FunctionsPrintAsNativeCode) {
  auto rt = makeHermesRuntime();
  auto *builtin = jsi::castInterface<IHermesBuiltinBytecode>(rt.get());
  ASSERT_NE(builtin, nullptr);
  const char *src = "(function setup(a) { return a; })";
  jsi::Value viaBuiltin = builtin->runBuiltinBytecode(compile(src), "b.js");
  EXPECT_NE(
      functionSource(*rt, viaBuiltin).find("[native code]"), std::string::npos);
  // The same bytecode through evaluateJavaScript is ordinary user code.
  jsi::Value viaEval = rt->evaluateJavaScript(compile(src), "e.js");
  EXPECT_EQ(
      functionSource(*rt, viaEval).find("[native code]"), std::string::npos);
}

TEST(BuiltinBytecodeTest, ThrowsJSErrorWhenTheCodeThrows) {
  auto rt = makeHermesRuntime();
  auto *builtin = jsi::castInterface<IHermesBuiltinBytecode>(rt.get());
  ASSERT_NE(builtin, nullptr);
  EXPECT_THROW(
      builtin->runBuiltinBytecode(
          compile("throw new TypeError('boom')"), "t.js"),
      jsi::JSError);
}

TEST(BuiltinBytecodeTest, RejectsSourceText) {
  auto rt = makeHermesRuntime();
  auto *builtin = jsi::castInterface<IHermesBuiltinBytecode>(rt.get());
  ASSERT_NE(builtin, nullptr);
  EXPECT_THROW(
      builtin->runBuiltinBytecode(
          std::make_shared<jsi::StringBuffer>("1 + 1"), "s.js"),
      jsi::JSINativeException);
}

TEST(BuiltinBytecodeTest, ThreadSafeRuntimeForwardsTheInterface) {
  auto rt = makeThreadSafeHermesRuntime();
  EXPECT_NE(jsi::castInterface<IHermesBuiltinBytecode>(rt.get()), nullptr);
}

} // namespace
