/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// REQUIRES: jsi_unstable
// RUN: %hermes -Xintl-plugin-path=%intl_fixture_dir/libfake_intl_good.so %s | %FileCheck --match-full-lines %s

// A Worker gets its parent's plugin and the defaults the parent's plugin
// resolved, without detecting again: the fake counts detections per process,
// and the worker still sees 1.

// Hermes does not keep function source, so the shared helper is a string.
var DESCRIBE =
  "function describe() { return typeof __fakeIntlPlugin === 'object' ? " +
  "JSON.stringify(__fakeIntlPlugin) : 'no plugin'; }";
(0, eval)(DESCRIBE);
print('parent ' + describe());
var worker = new Worker(DESCRIBE + "\npostMessage('worker ' + describe());");
worker.onmessage = function (msg) {
  print(msg);
  worker.terminate();
};
// CHECK: parent {"locales":["x-detected-1"],"timeZone":"Fake/Detected","detections":1}
// CHECK-NEXT: worker {"locales":["x-detected-1"],"timeZone":"Fake/Detected","detections":1}
