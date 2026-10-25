/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// See 00-header.js. ECMA-402 draft of October 24, 2026.

/// Called once per runtime by intl/jsi/Plugin.cpp, before any user code,
/// with the native helpers and the resolved defaults. Design spec "What
/// setup does"; it is not an ECMA-402 algorithm.
return function setup(nativeHelpers, defaults) {
  // 1. Keep the native helpers and the defaults in the closure.
  native = nativeHelpers;
  hostDefaults = defaults;
  // 2. Define globalThis.Intl.
  installIntl();
  // 3. Replace the locale-sensitive built-ins with their ECMA-402 versions.
  installBuiltinOverrides();
};
})();
