/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#pragma once
#ifdef JSI_UNSTABLE
#ifndef HERMES_EXTENSIONS_WORKER_H
#define HERMES_EXTENSIONS_WORKER_H

#include "Extensions.h"

#include <jsi/jsi.h>

namespace facebook {
namespace hermes {

/// Install the Worker constructor on the global object.
/// \param runtime The JSI runtime to install into.
/// \param extensions The precompiled extensions object containing setup
///   functions.
/// \p config carries the host-runtime facts this extension needs; see
/// ExtensionsConfig. The Worker constructor takes raw bytes from JS and runs
/// them through evaluateJavaScript, which decides source-vs-bytecode by
/// content, so it consults config.allowUntrustedBytecodeFromJS before running
/// anything that turns out to be bytecode.
void installWorker(
    jsi::Runtime &rt,
    jsi::Object &extensions,
    const ExtensionsConfig &config);

} // namespace hermes
} // namespace facebook

#endif // HERMES_EXTENSIONS_WORKER_H
#endif // JSI_UNSTABLE
