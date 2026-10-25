/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The collator helpers of NativeHelpers.def.

#include "NativeHelpers.h"
#include "RustCore.h"

#include <memory>

using namespace facebook;

namespace hermes_intl {

namespace {

/// The native state of an Intl.Collator instance: the Rust collator for its
/// resolved options. HiCollator is Send, so the finalizer thread may destroy
/// it; the destructor frees nothing else.
class CollatorState : public jsi::NativeState {
 public:
  /// Make the collator; collator() is null if hi_collator_new fails.
  CollatorState(
      const std::string &locale,
      const std::string &collation,
      const std::string &sensitivity,
      bool ignorePunctuation,
      bool numeric,
      const std::string &caseFirst)
      : collator_(core::collatorNew(
            locale,
            collation,
            sensitivity,
            ignorePunctuation,
            numeric,
            caseFirst)) {}
  CollatorState(const CollatorState &) = delete;
  CollatorState &operator=(const CollatorState &) = delete;
  /// Free the collator.
  ~CollatorState() override;

  /// The collator, or null if it could not be made.
  const HiCollator *collator() const {
    return collator_;
  }

 private:
  /// Owned; freed by the destructor.
  HiCollator *const collator_;
};

CollatorState::~CollatorState() {
  hi_collator_free(collator_);
}

} // namespace

namespace helpers {

std::string collatorLocaleDefaults(jsi::Runtime &, const std::string &locale) {
  if (auto result = core::collatorLocaleDefaults(locale))
    return *result;
  detail::internalError(
      "collatorLocaleDefaults: locale does not parse: " + locale);
}

std::monostate collatorInit(
    jsi::Runtime &rt,
    const jsi::Object &collator,
    const std::string &locale,
    const std::string &collation,
    const std::string &sensitivity,
    const bool &ignorePunctuation,
    const bool &numeric,
    const std::string &caseFirst) {
  if (collator.hasNativeState(rt))
    detail::internalError("collatorInit: the object already has native state");
  // Made before the collator, so an allocation failure leaks nothing.
  auto state = std::make_shared<CollatorState>(
      locale, collation, sensitivity, ignorePunctuation, numeric, caseFirst);
  if (!state->collator())
    detail::internalError(
        "collatorInit: no collator for " + locale + " collation " + collation +
        " sensitivity " + sensitivity + " caseFirst " + caseFirst);
  collator.setNativeState(rt, std::move(state));
  return {};
}

int collatorCompare(
    jsi::Runtime &rt,
    const jsi::Object &collator,
    const std::u16string &x,
    const std::u16string &y) {
  // getNativeState requires native state to be present, so this takes two
  // lookups. hasNativeState<CollatorState> and getNativeState<CollatorState>
  // would take three.
  std::shared_ptr<CollatorState> state;
  if (collator.hasNativeState(rt))
    state =
        std::dynamic_pointer_cast<CollatorState>(collator.getNativeState(rt));
  if (!state)
    detail::internalError("collatorCompare: the object has no collator");
  return core::collatorCompare(state->collator(), x, y);
}

} // namespace helpers

} // namespace hermes_intl
