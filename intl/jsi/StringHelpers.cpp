/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The string helpers of NativeHelpers.def.

#include "NativeHelpers.h"
#include "RustCore.h"

using namespace facebook;

namespace hermes_intl::helpers {

std::u16string caseMap(
    jsi::Runtime &,
    const std::u16string &text,
    const std::string &language,
    const bool &upper) {
  if (auto result = core::caseMap(text, language, upper))
    return *std::move(result);
  detail::internalError("caseMap: language does not parse: " + language);
}

} // namespace hermes_intl::helpers
