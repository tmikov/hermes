/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "NativeHelpers.h"

using namespace facebook;

namespace hermes_intl {

jsi::Object makeNativeHelpers(jsi::Runtime &rt) {
  jsi::Object helpersObject = jsi::Object::create(rt, jsi::Value::null());
#define HERMES_INTL_HELPER(name, ...) \
  HERMES_INTL_ADD_HELPER(rt, helpersObject, name, __VA_ARGS__);
#include "NativeHelpers.def"
#undef HERMES_INTL_HELPER
  return helpersObject;
}

jsi::Object makeSetupDefaults(
    jsi::Runtime &rt,
    const std::vector<std::string> &locales,
    const std::string &timeZone) {
  jsi::Array localesArray(rt, locales.size());
  for (size_t i = 0; i < locales.size(); ++i)
    localesArray.setValueAtIndex(
        rt, i, jsi::String::createFromUtf8(rt, locales[i]));
  jsi::Object defaults = jsi::Object::create(rt, jsi::Value::null());
  defaults.setProperty(rt, "locales", localesArray);
  defaults.setProperty(
      rt, "timeZone", jsi::String::createFromUtf8(rt, timeZone));
  return defaults;
}

} // namespace hermes_intl
