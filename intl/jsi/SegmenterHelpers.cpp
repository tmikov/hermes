/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The segmenter helpers of NativeHelpers.def.

#include "NativeHelpers.h"
#include "RustCore.h"

#include <memory>

using namespace facebook;

namespace hermes_intl {

namespace {

/// The native state of an Intl.Segmenter instance: the Rust segmenter for
/// one granularity and content locale. HiSegmenter is Send, so the finalizer
/// thread may destroy it; the destructor frees nothing else.
class SegmenterState : public jsi::NativeState {
 public:
  /// Make the segmenter; segmenter() is null if hi_segmenter_new fails.
  SegmenterState(const std::string &locale, const std::string &granularity)
      : segmenter_(core::segmenterNew(locale, granularity)) {}
  SegmenterState(const SegmenterState &) = delete;
  SegmenterState &operator=(const SegmenterState &) = delete;
  /// Free the segmenter.
  ~SegmenterState() override;

  /// The segmenter, or null if it could not be made.
  const HiSegmenter *segmenter() const {
    return segmenter_;
  }

 private:
  /// Owned; freed by the destructor.
  HiSegmenter *const segmenter_;
};

SegmenterState::~SegmenterState() {
  hi_segmenter_free(segmenter_);
}

/// The native state of a Segments object: the boundaries of its string, and
/// for word granularity whether each segment is word-like. It does not refer
/// to the segmenter. HiSegments is Send, so the finalizer thread may destroy
/// it; the destructor frees nothing else.
class SegmentsState : public jsi::NativeState {
 public:
  /// Segment \p text with \p segmenter; segments() is null if
  /// hi_segments_new fails.
  SegmentsState(const HiSegmenter *segmenter, const std::u16string &text)
      : segments_(core::segmentsNew(segmenter, text)) {}
  SegmentsState(const SegmentsState &) = delete;
  SegmentsState &operator=(const SegmentsState &) = delete;
  /// Free the segments.
  ~SegmentsState() override;

  /// The segments, or null if they could not be made.
  const HiSegments *segments() const {
    return segments_;
  }

 private:
  /// Owned; freed by the destructor.
  HiSegments *const segments_;
};

SegmentsState::~SegmentsState() {
  hi_segments_free(segments_);
}

/// The native state of type \p State of \p object, or null if it has none
/// or state of another type. getNativeState requires native state to be
/// present, so this takes two lookups, as collatorCompare does.
template <class State>
std::shared_ptr<State> stateOf(jsi::Runtime &rt, const jsi::Object &object) {
  if (!object.hasNativeState(rt))
    return nullptr;
  return std::dynamic_pointer_cast<State>(object.getNativeState(rt));
}

/// The segments of \p object; an internal error naming \p helper if
/// segmentsInit did not initialize it.
const HiSegments *
segmentsOf(jsi::Runtime &rt, const jsi::Object &object, const char *helper) {
  auto state = stateOf<SegmentsState>(rt, object);
  if (!state)
    detail::internalError(std::string(helper) + ": the object has no segments");
  return state->segments();
}

} // namespace

namespace helpers {

std::monostate segmenterInit(
    jsi::Runtime &rt,
    const jsi::Object &segmenter,
    const std::string &locale,
    const std::string &granularity) {
  if (segmenter.hasNativeState(rt))
    detail::internalError("segmenterInit: the object already has native state");
  // The state's constructor makes the segmenter, so it is owned from the
  // moment it exists: neither a failed allocation nor the throw below
  // leaks it.
  auto state = std::make_shared<SegmenterState>(locale, granularity);
  if (!state->segmenter())
    detail::internalError(
        "segmenterInit: no segmenter for " + locale + " " + granularity);
  segmenter.setNativeState(rt, std::move(state));
  return {};
}

std::monostate segmentsInit(
    jsi::Runtime &rt,
    const jsi::Object &segments,
    const jsi::Object &segmenter,
    const std::u16string &string) {
  auto segmenterState = stateOf<SegmenterState>(rt, segmenter);
  if (!segmenterState)
    detail::internalError("segmentsInit: the object has no segmenter");
  if (segments.hasNativeState(rt))
    detail::internalError("segmentsInit: the object already has native state");
  // As in segmenterInit, the state owns the segments from the start.
  auto state =
      std::make_shared<SegmentsState>(segmenterState->segmenter(), string);
  if (!state->segments())
    detail::internalError("segmentsInit: the string is too long");
  segments.setNativeState(rt, std::move(state));
  return {};
}

int segmentsFindBoundary(
    jsi::Runtime &rt,
    const jsi::Object &segments,
    const int &index,
    const bool &after) {
  auto boundary = core::segmentsFindBoundary(
      segmentsOf(rt, segments, "segmentsFindBoundary"), index, after);
  if (!boundary)
    detail::internalError("segmentsFindBoundary: index out of range");
  return *boundary;
}

bool segmentsIsWordLike(
    jsi::Runtime &rt,
    const jsi::Object &segments,
    const int &start) {
  auto wordLike = core::segmentsIsWordLike(
      segmentsOf(rt, segments, "segmentsIsWordLike"), start);
  if (!wordLike)
    detail::internalError(
        "segmentsIsWordLike: not the start of a word segment");
  return *wordLike;
}

} // namespace helpers

} // namespace hermes_intl
