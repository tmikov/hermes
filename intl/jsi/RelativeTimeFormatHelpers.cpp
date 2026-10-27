/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The relative time format helper of NativeHelpers.def: the CLDR patterns
// and relative names Intl.RelativeTimeFormat formats with. It keeps no
// state.

#include "NativeHelpers.h"
#include "RustCore.h"

using namespace facebook;

namespace hermes_intl::helpers {

std::vector<std::string> relativeTimePatterns(
    jsi::Runtime &,
    const std::string &locale,
    const std::string &style) {
  if (auto result = core::relativeTimePatterns(locale, style))
    return std::move(*result);
  detail::internalError(
      "relativeTimePatterns: no data for " + locale + " " + style);
}

} // namespace hermes_intl::helpers
