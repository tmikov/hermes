/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#ifdef _WIN32
#define FAKE_EXPORT __declspec(dllexport)
#else
#define FAKE_EXPORT __attribute__((visibility("default")))
#endif

/// A library fake_intl_needs_dep depends on; the tests never make it
/// findable.
extern "C" FAKE_EXPORT int fake_intl_dep() {
  return 1;
}
