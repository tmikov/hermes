/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The plural rules helpers of NativeHelpers.def.

#include "NativeHelpers.h"
#include "RustCore.h"

#include <memory>

using namespace facebook;

namespace hermes_intl {

namespace {

/// The native state of an Intl.PluralRules instance, or of the internal
/// plural rules object of an Intl.NumberFormat: the Rust plural rules for
/// one locale and type. HiPluralRules is Send, so the finalizer thread may
/// destroy it; the destructor frees nothing else.
class PluralRulesState : public jsi::NativeState {
 public:
  /// Make the plural rules; rules() is null if hi_plural_rules_new fails.
  PluralRulesState(const std::string &locale, bool ordinal)
      : rules_(core::pluralRulesNew(locale, ordinal)) {}
  PluralRulesState(const PluralRulesState &) = delete;
  PluralRulesState &operator=(const PluralRulesState &) = delete;
  /// Free the plural rules.
  ~PluralRulesState() override;

  /// The plural rules, or null if they could not be made.
  const HiPluralRules *rules() const {
    return rules_;
  }

 private:
  /// Owned; freed by the destructor.
  HiPluralRules *const rules_;
};

PluralRulesState::~PluralRulesState() {
  hi_plural_rules_free(rules_);
}

/// The plural rules of \p object; an internal error naming \p helper if
/// pluralRulesInit did not initialize it.
const HiPluralRules *
rulesOf(jsi::Runtime &rt, const jsi::Object &object, const char *helper) {
  // getNativeState requires native state to be present, so this takes two
  // lookups, as collatorCompare does.
  std::shared_ptr<PluralRulesState> state;
  if (object.hasNativeState(rt))
    state =
        std::dynamic_pointer_cast<PluralRulesState>(object.getNativeState(rt));
  if (!state)
    detail::internalError(
        std::string(helper) + ": the object has no plural rules");
  return state->rules();
}

} // namespace

namespace helpers {

std::monostate pluralRulesInit(
    jsi::Runtime &rt,
    const jsi::Object &pluralRules,
    const std::string &locale,
    const bool &ordinal) {
  if (pluralRules.hasNativeState(rt))
    detail::internalError(
        "pluralRulesInit: the object already has native state");
  // The state's constructor makes the rules, so they are owned from the
  // moment they exist: neither a failed allocation nor the throw below
  // leaks them.
  auto state = std::make_shared<PluralRulesState>(locale, ordinal);
  if (!state->rules())
    detail::internalError(
        "pluralRulesInit: no plural rules for " + locale +
        (ordinal ? " ordinal" : " cardinal"));
  pluralRules.setNativeState(rt, std::move(state));
  return {};
}

std::string pluralSelect(
    jsi::Runtime &rt,
    const jsi::Object &pluralRules,
    const std::string &decimal,
    const int &exponent) {
  if (auto result = core::pluralRulesSelect(
          rulesOf(rt, pluralRules, "pluralSelect"), decimal, exponent))
    return *result;
  detail::internalError("pluralSelect: not a plain decimal: " + decimal);
}

std::string pluralSelectRange(
    jsi::Runtime &rt,
    const jsi::Object &pluralRules,
    const std::string &startCategory,
    const std::string &endCategory) {
  if (auto result = core::pluralRulesSelectRange(
          rulesOf(rt, pluralRules, "pluralSelectRange"),
          startCategory,
          endCategory))
    return *result;
  detail::internalError(
      "pluralSelectRange: not a category: " + startCategory + " " +
      endCategory);
}

std::vector<std::string> pluralCategories(
    jsi::Runtime &rt,
    const jsi::Object &pluralRules) {
  return core::pluralRulesCategories(
      rulesOf(rt, pluralRules, "pluralCategories"));
}

} // namespace helpers

} // namespace hermes_intl
