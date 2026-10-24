/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#pragma once

#include "hermes/Public/RuntimeConfig.h"

#include <jsi/jsi.h>

namespace facebook::hermes {

/// Install the Intl plugin into \p rt as \p config asks, and record the
/// outcome on \p rt for makeWorkerRuntimeConfig. Follows the "Loader" section
/// of doc/superpowers/specs/2026-10-22-intl-plugin-design.md: every row of
/// its error table marked fatal calls hermes_fatal.
void installIntlPlugin(
    jsi::Runtime &rt,
    const ::hermes::vm::RuntimeConfig &config);

/// The RuntimeConfig for a Worker started by \p parent: the parent's plugin
/// (or none), the default locales and time zone the parent's plugin actually
/// received, and the parent's Intl flag. Everything else is the default.
::hermes::vm::RuntimeConfig makeWorkerRuntimeConfig(jsi::Runtime &parent);

/// How many times this process has resolved the default plugin (0 or 1).
/// For tests.
unsigned intlDefaultResolutionsForTests();

} // namespace facebook::hermes
