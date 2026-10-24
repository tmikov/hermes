/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// REQUIRES: jsi_unstable
// The old built-in Intl would make `typeof Intl` differ for other reasons.
// UNSUPPORTED: intl
// RUN: %hermes -Xintl-plugin=false -Xintl-plugin-path=%intl_fixture_dir/libfake_intl_good.so %s | %FileCheck --match-full-lines %s

// A parent without a plugin has workers without one, even when the default
// plugin (the real libhermesintl in the build tree) would be found.

// Hermes does not keep function source, so the shared helper is a string.
var DESCRIBE =
  "function describe() { return typeof __fakeIntlPlugin + ' ' + typeof Intl; }";
(0, eval)(DESCRIBE);
print('parent ' + describe());
var worker = new Worker(DESCRIBE + "\npostMessage('worker ' + describe());");
worker.onmessage = function (msg) {
  print(msg);
  worker.terminate();
};
// CHECK: parent undefined undefined
// CHECK-NEXT: worker undefined undefined
