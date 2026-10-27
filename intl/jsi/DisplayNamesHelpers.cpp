/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The display name helpers of NativeHelpers.def: the single names and the
// locale display patterns Intl.DisplayNames composes its results from. They
// keep no state.

#include "NativeHelpers.h"
#include "RustCore.h"

using namespace facebook;

namespace hermes_intl::helpers {

std::optional<std::string> displayName(
    jsi::Runtime &,
    const std::string &locale,
    const std::string &kind,
    const std::string &code,
    const std::string &style) {
  if (auto result = core::displayName(locale, kind, code, style))
    return std::move(*result);
  detail::internalError(
      "displayName: bad input " + locale + " " + kind + " " + code + " " +
      style);
}

std::vector<std::string> localeDisplayPatterns(
    jsi::Runtime &,
    const std::string &locale) {
  if (auto result = core::localeDisplayPatterns(locale))
    return std::move(*result);
  detail::internalError("localeDisplayPatterns: bad locale " + locale);
}

} // namespace hermes_intl::helpers
