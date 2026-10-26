/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The DateTimeFormat helpers of NativeHelpers.def: the CLDR patterns, the
// flexible day periods and time zone name lookup, which keep no state; and
// the native state of each formatter, which gives the local time and the
// names of an instant.

#include "NativeHelpers.h"
#include "RustCore.h"

#include <memory>

using namespace facebook;

namespace hermes_intl {

namespace {

/// The native state of an Intl.DateTimeFormat instance: the Rust calendar,
/// time zone rules and names of one formatter. HiDateTimeFormat is Send, so
/// the finalizer thread may destroy it; the destructor frees nothing else.
class DateTimeFormatState : public jsi::NativeState {
 public:
  /// Make the formatter's state; format() is null if
  /// hi_date_time_format_new fails.
  DateTimeFormatState(
      const std::string &locale,
      const std::string &calendar,
      const std::string &timeZone,
      const std::string &symbols)
      : format_(core::dateTimeFormatNew(locale, calendar, timeZone, symbols)) {}
  DateTimeFormatState(const DateTimeFormatState &) = delete;
  DateTimeFormatState &operator=(const DateTimeFormatState &) = delete;
  /// Free the formatter's state.
  ~DateTimeFormatState() override;

  /// The formatter's state, or null if it could not be made.
  const HiDateTimeFormat *format() const {
    return format_;
  }

 private:
  /// Owned; freed by the destructor.
  HiDateTimeFormat *const format_;
};

DateTimeFormatState::~DateTimeFormatState() {
  hi_date_time_format_free(format_);
}

/// The formatter state of \p object; an internal error naming \p helper if
/// dateTimeFormatInit did not initialize it.
const HiDateTimeFormat *
formatOf(jsi::Runtime &rt, const jsi::Object &object, const char *helper) {
  // getNativeState requires native state to be present, so this takes two
  // lookups, as collatorCompare does.
  std::shared_ptr<DateTimeFormatState> state;
  if (object.hasNativeState(rt))
    state = std::dynamic_pointer_cast<DateTimeFormatState>(
        object.getNativeState(rt));
  if (!state)
    detail::internalError(
        std::string(helper) + ": the object has no DateTimeFormat state");
  return state->format();
}

} // namespace

namespace helpers {

std::vector<std::string> dateTimePatterns(
    jsi::Runtime &,
    const std::string &locale,
    const std::string &calendar) {
  if (auto result = core::dateTimePatterns(locale, calendar))
    return std::move(*result);
  detail::internalError(
      "dateTimePatterns: no data for " + locale + " " + calendar);
}

std::vector<std::string> dayPeriods(jsi::Runtime &, const std::string &locale) {
  if (auto result = core::dayPeriods(locale))
    return std::move(*result);
  detail::internalError("dayPeriods: no data for " + locale);
}

std::optional<std::string> canonicalizeTimeZone(
    jsi::Runtime &,
    const std::string &name) {
  return core::canonicalizeTimeZone(name);
}

std::monostate dateTimeFormatInit(
    jsi::Runtime &rt,
    const jsi::Object &dateTimeFormat,
    const std::string &locale,
    const std::string &calendar,
    const std::string &timeZone,
    const std::string &symbols) {
  if (dateTimeFormat.hasNativeState(rt))
    detail::internalError(
        "dateTimeFormatInit: the object already has native state");
  // The state's constructor makes the Rust object, so it is owned from the
  // moment it exists: neither a failed allocation nor the throw below leaks
  // it.
  auto state = std::make_shared<DateTimeFormatState>(
      locale, calendar, timeZone, symbols);
  if (!state->format())
    detail::internalError(
        "dateTimeFormatInit: cannot make a formatter for " + locale + " " +
        calendar + " " + timeZone + " [" + symbols + "]");
  dateTimeFormat.setNativeState(rt, std::move(state));
  return {};
}

std::vector<std::string> dateTimeFormatLocalTime(
    jsi::Runtime &rt,
    const jsi::Object &dateTimeFormat,
    const double &epochMs) {
  std::vector<std::string> result;
  switch (core::dateTimeFormatLocalTime(
      formatOf(rt, dateTimeFormat, "dateTimeFormatLocalTime"),
      epochMs,
      result)) {
    case HiLocalTimeStatus_Ok:
      return result;
    case HiLocalTimeStatus_NotATimeValue:
      detail::internalError(
          "dateTimeFormatLocalTime: not a time value: " +
          std::to_string(epochMs));
    case HiLocalTimeStatus_FormatFailed:
      break;
  }
  detail::internalError(
      "dateTimeFormatLocalTime: ICU4X failed to format a name at " +
      std::to_string(epochMs));
}

} // namespace helpers

} // namespace hermes_intl
