/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#pragma once

#include <jsi/jsi.h>

namespace facebook {
namespace hermes {

/// Install all JSI extensions (TextEncoder, etc.) into the runtime.
/// The extensions object is passed in from the caller who loaded the bytecode.
/// Facts about the host runtime that extensions need at install time.
///
/// These are read off the live vm::Runtime where the extensions are installed
/// and passed down, rather than each extension reaching back for them: an
/// extension has only a jsi::Runtime, and nothing retains the RuntimeConfig it
/// was built from. Keeping them in one struct means adding the next one costs
/// a field here and a line at the construction site, not a signature change in
/// every layer between.
///
/// Every field defaults to the restrictive answer, so a caller that forgets to
/// set one gets the safe behaviour rather than the permissive one.
struct ExtensionsConfig {
  /// RuntimeConfig's EnableUntrustedBytecodeFromJS. False means an extension
  /// that accepts bytes from JS must refuse them if they are Hermes bytecode:
  /// bytecode is trusted by construction rather than re-validated the way
  /// source is, so letting script supply it crosses a trust boundary. Consumed
  /// by installWorker; see Worker.cpp.
  bool allowUntrustedBytecodeFromJS = false;
};

void installExtensions(
    jsi::Runtime &rt,
    jsi::Object extensions,
    const ExtensionsConfig &config);

} // namespace hermes
} // namespace facebook
