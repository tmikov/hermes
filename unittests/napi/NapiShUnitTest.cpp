/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "../VMRuntime/StaticHUnitFixtures.h"
#include "NapiTestFixture.h"

namespace {
using namespace hermes::napi;

class NapiShUnitTest : public NapiTestFixture {
 protected:
  void openScope() {
    napi_handle_scope scope;
    ASSERT_EQ(napi_open_handle_scope(env_, &scope), napi_ok);
    scope_ = scope;
  }
  void closeScope() {
    ASSERT_EQ(napi_close_handle_scope(env_, scope_), napi_ok);
  }

  napi_handle_scope scope_ = nullptr;
};

TEST_F(NapiShUnitTest, InitReturnsTheUnitsCompletionValue) {
  openScope();
  napi_value result = nullptr;
  ASSERT_EQ(napi_ok, hermes_init_sh_unit(env_, createTestUnit, &result));
  ASSERT_NE(nullptr, result);
  napi_valuetype type;
  ASSERT_EQ(napi_ok, napi_typeof(env_, result, &type));
  EXPECT_EQ(napi_undefined, type); // the minimal unit returns undefined
  closeScope();
}

/// The half no bundled module can reach: the wrapper means a module's
/// global only creates a closure, so a throwing module body throws when
/// the loader CALLS it, after this function has returned.
TEST_F(NapiShUnitTest, InitReportsAThrowingUnitAsAPendingException) {
  openScope();
  napi_value result = nullptr;
  EXPECT_EQ(
      napi_pending_exception,
      hermes_init_sh_unit(env_, createThrowingUnit, &result));
  bool pending = false;
  ASSERT_EQ(napi_ok, napi_is_exception_pending(env_, &pending));
  EXPECT_TRUE(pending);
  napi_value exc = nullptr;
  ASSERT_EQ(napi_ok, napi_get_and_clear_last_exception(env_, &exc));
  double value = 0;
  ASSERT_EQ(napi_ok, napi_get_value_double(env_, exc, &value));
  EXPECT_EQ(42, value);
  closeScope();
}

TEST_F(NapiShUnitTest, InitRejectsNullArguments) {
  openScope();
  napi_value result = nullptr;
  EXPECT_EQ(napi_invalid_arg, hermes_init_sh_unit(env_, nullptr, &result));
  EXPECT_EQ(
      napi_invalid_arg, hermes_init_sh_unit(env_, createTestUnit, nullptr));
  closeScope();
}

} // namespace
