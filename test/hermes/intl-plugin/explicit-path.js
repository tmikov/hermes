/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-plugin-path=%intl_fixture_dir/libfake_intl_good.so %s | %FileCheck --match-full-lines %s
// RUN: %hermes -Xintl-plugin-path=%intl_fixture_dir/libfake_intl_good.so -Xintl-default-locale=fr-FR -Xintl-default-locale=de -Xintl-default-time-zone=Europe/Paris %s | %FileCheck --match-full-lines --check-prefix=EXPLICIT %s
// RUN: %hermes -Xintl-plugin=false -Xintl-plugin-path=%intl_fixture_dir/libfake_intl_good.so %s | %FileCheck --match-full-lines --check-prefix=DISABLED %s

// An explicit IntlPluginPath loads that library; disabling the plugin wins
// over the path. The fake also checks the binary contract: a jsi::JSError
// thrown in the plugin reaches JS as that error, and NativeState works
// across the boundary.

print(
  typeof __fakeIntlPlugin === 'object'
    ? JSON.stringify(__fakeIntlPlugin)
    : 'no plugin',
);
// CHECK: {"locales":["x-detected-1"],"timeZone":"Fake/Detected","detections":1}
// EXPLICIT: {"locales":["fr-FR","de"],"timeZone":"Europe/Paris","detections":0}
// DISABLED: no plugin
if (typeof __fakeIntlThrow === 'function') {
  try {
    __fakeIntlThrow();
  } catch (e) {
    print(e.message);
  }
  var o = {};
  print(__fakeIntlRead(o));
  __fakeIntlAttach(o);
  print(__fakeIntlRead(o));
}
// CHECK-NEXT: fake JSError
// CHECK-NEXT: -1
// CHECK-NEXT: 42
