/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The Hermes Intl plugin's ECMA-402 implementation, following the draft
// https://tc39.es/ecma402/ dated October 24, 2026. 00-header.js and
// 99-footer.js wrap the other files (concatenated in name order) in one
// function scope, so nothing defined here is reachable from user code. The
// completion value of the compiled bytecode is the setup function in
// 99-footer.js, which intl/jsi/Plugin.cpp calls once per runtime.
//
// Follow the rules in "Writing plugin JS" in intl/doc/Development.md:
// captured intrinsics only, no spread or for-of, listPush instead of indexed
// stores, null-prototype descriptors.
(function () {
'use strict';

/// The native helpers (intl/jsi/NativeHelpers.def); set by setup.
var native;
/// {locales, timeZone}: the defaults the loader resolved; set by setup.
var hostDefaults;
