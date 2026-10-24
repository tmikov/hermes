/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The binary contract with a shared-library host: the plugin binds to
// libhermesvm's JSI, a jsi::JSError thrown in the plugin reaches JS as that
// error, and NativeState works across the boundary.
// RUN: %intl_shared_host %intl_fixture_dir/libfake_intl_good.so %s | %FileCheck --match-full-lines %s

var out = [];
try {
  __fakeIntlThrow();
} catch (e) {
  out.push(e.message);
}
var o = {};
out.push(String(__fakeIntlRead(o)));
__fakeIntlAttach(o);
out.push(String(__fakeIntlRead(o)));
out.push(JSON.stringify(__fakeIntlPlugin.locales));
out.join(' ');
// CHECK: fake JSError -1 42 ["x-detected-1"]
