/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The locale helpers of NativeHelpers.def.

#include "NativeHelpers.h"
#include "RustCore.h"

#include <stdexcept>

using namespace facebook;

namespace hermes_intl::helpers {

std::optional<std::string> canonicalizeLocale(
    jsi::Runtime &,
    const std::string &tag) {
  return core::canonicalizeLocale(tag);
}

std::string addLikelySubtags(jsi::Runtime &, const std::string &tag) {
  if (auto result = core::addLikelySubtags(tag))
    return *result;
  throw std::logic_error("Intl internal error: addLikelySubtags: " + tag);
}

std::string removeLikelySubtags(jsi::Runtime &, const std::string &tag) {
  if (auto result = core::removeLikelySubtags(tag))
    return *result;
  throw std::logic_error("Intl internal error: removeLikelySubtags: " + tag);
}

bool isAvailableLocale(jsi::Runtime &, const std::string &tag) {
  return core::isAvailableLocale(tag);
}

std::string supportedValues(jsi::Runtime &, const std::string &key) {
  HiValueKind kind;
  if (key == "calendar")
    kind = HiValueKind_Calendar;
  else if (key == "collation")
    kind = HiValueKind_Collation;
  else if (key == "currency")
    kind = HiValueKind_Currency;
  else if (key == "numberingSystem")
    kind = HiValueKind_NumberingSystem;
  else if (key == "timeZone")
    kind = HiValueKind_TimeZone;
  else
    throw std::logic_error("Intl internal error: supportedValues: " + key);
  return core::supportedValues(kind);
}

} // namespace hermes_intl::helpers
