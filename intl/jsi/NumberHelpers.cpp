/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The number data helpers of NativeHelpers.def: the symbols, patterns and
// currency data Intl.NumberFormat formats with. They keep no state.

#include "NativeHelpers.h"
#include "RustCore.h"

using namespace facebook;

namespace hermes_intl::helpers {

std::vector<std::string> numberSymbols(
    jsi::Runtime &,
    const std::string &locale,
    const std::string &nu) {
  if (auto result = core::numberSymbols(locale, nu))
    return std::move(*result);
  detail::internalError("numberSymbols: no data for " + locale + " " + nu);
}

std::vector<std::string> currencyPatterns(
    jsi::Runtime &,
    const std::string &locale,
    const std::string &nu,
    const std::string &currency,
    const std::string &display,
    const bool &accounting) {
  if (auto result =
          core::currencyPatterns(locale, nu, currency, display, accounting))
    return std::move(*result);
  detail::internalError(
      "currencyPatterns: no data for " + locale + " " + nu + " " + currency +
      " " + display);
}

std::vector<std::string> compactCurrencyPatterns(
    jsi::Runtime &,
    const std::string &locale,
    const std::string &nu,
    const std::string &currency,
    const std::string &display,
    const bool &accounting) {
  if (auto result = core::compactCurrencyPatterns(
          locale, nu, currency, display, accounting))
    return std::move(*result);
  detail::internalError(
      "compactCurrencyPatterns: no data for " + locale + " " + nu + " " +
      currency + " " + display);
}

std::vector<std::string> currencySeparators(
    jsi::Runtime &,
    const std::string &locale,
    const std::string &nu,
    const std::string &currency) {
  if (auto result = core::currencySeparators(locale, nu, currency))
    return std::move(*result);
  detail::internalError(
      "currencySeparators: no data for " + locale + " " + nu + " " + currency);
}

std::vector<std::string> compactPatterns(
    jsi::Runtime &,
    const std::string &locale,
    const std::string &nu,
    const bool &long_) {
  if (auto result = core::compactPatterns(locale, nu, long_))
    return std::move(*result);
  detail::internalError("compactPatterns: no data for " + locale + " " + nu);
}

std::vector<std::string> unitPatterns(
    jsi::Runtime &,
    const std::string &locale,
    const std::string &unit,
    const std::string &width) {
  if (auto result = core::unitPatterns(locale, unit, width))
    return std::move(*result);
  detail::internalError(
      "unitPatterns: no data for " + locale + " " + unit + " " + width);
}

int currencyDigits(jsi::Runtime &, const std::string &currency) {
  if (auto result = core::currencyDigits(currency))
    return *result;
  detail::internalError("currencyDigits: not a currency code: " + currency);
}

} // namespace hermes_intl::helpers
