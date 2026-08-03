/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "node_api.h"

namespace {

/// Entry point for the exported `parse` function.
napi_value parse(napi_env env, napi_callback_info info) {
  napi_throw_error(env, nullptr, "hermes-parser-native: not implemented");
  return nullptr;
}

/// Module initializer. Registers `parse` on the exports object.
napi_value init(napi_env env, napi_value exports) {
  napi_value fn;
  if (napi_create_function(env, "parse", NAPI_AUTO_LENGTH, parse, nullptr,
                           &fn) != napi_ok) {
    return nullptr;
  }
  if (napi_set_named_property(env, exports, "parse", fn) != napi_ok) {
    return nullptr;
  }
  return exports;
}

} // namespace

NAPI_MODULE(NODE_GYP_MODULE_NAME, init)
