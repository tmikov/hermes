/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The list format helpers of NativeHelpers.def.

#include "NativeHelpers.h"
#include "RustCore.h"

#include <memory>

using namespace facebook;

namespace hermes_intl {

namespace {

/// The native state of an Intl.ListFormat instance: the Rust list format
/// for one locale, type and style. HiListFormat is Send, so the finalizer
/// thread may destroy it; the destructor frees nothing else.
class ListFormatState : public jsi::NativeState {
 public:
  /// Make the list format; format() is null if hi_list_format_new fails.
  ListFormatState(
      const std::string &locale,
      const std::string &type,
      const std::string &style)
      : format_(core::listFormatNew(locale, type, style)) {}
  ListFormatState(const ListFormatState &) = delete;
  ListFormatState &operator=(const ListFormatState &) = delete;
  /// Free the list format.
  ~ListFormatState() override;

  /// The list format, or null if it could not be made.
  const HiListFormat *format() const {
    return format_;
  }

 private:
  /// Owned; freed by the destructor.
  HiListFormat *const format_;
};

ListFormatState::~ListFormatState() {
  hi_list_format_free(format_);
}

/// The list format of \p object; an internal error naming \p helper if
/// listFormatInit did not initialize it.
const HiListFormat *
formatOf(jsi::Runtime &rt, const jsi::Object &object, const char *helper) {
  // getNativeState requires native state to be present, so this takes two
  // lookups, as collatorCompare does.
  std::shared_ptr<ListFormatState> state;
  if (object.hasNativeState(rt))
    state =
        std::dynamic_pointer_cast<ListFormatState>(object.getNativeState(rt));
  if (!state)
    detail::internalError(
        std::string(helper) + ": the object has no list format");
  return state->format();
}

} // namespace

namespace helpers {

std::monostate listFormatInit(
    jsi::Runtime &rt,
    const jsi::Object &listFormat,
    const std::string &locale,
    const std::string &type,
    const std::string &style) {
  if (listFormat.hasNativeState(rt))
    detail::internalError(
        "listFormatInit: the object already has native state");
  // The state's constructor makes the list format, so it is owned from the
  // moment it exists: neither a failed allocation nor the throw below
  // leaks it.
  auto state = std::make_shared<ListFormatState>(locale, type, style);
  if (!state->format())
    detail::internalError(
        "listFormatInit: no list format for " + locale + " " + type + " " +
        style);
  listFormat.setNativeState(rt, std::move(state));
  return {};
}

std::vector<std::string> listFormatLiterals(
    jsi::Runtime &rt,
    const jsi::Object &listFormat,
    const bool &isPair,
    const std::u16string &last) {
  return core::listFormatLiterals(
      formatOf(rt, listFormat, "listFormatLiterals"), isPair, last);
}

} // namespace helpers

} // namespace hermes_intl
