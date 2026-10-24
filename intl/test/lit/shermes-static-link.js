/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// shermes -static-link (macOS only) links the VM and JSI into the
// executable, and the plugin binds to the executable's copy. ld64 exports an
// executable's external symbols by default, and shermes links without
// -dead_strip, so that copy is there to bind to.
// REQUIRES: darwin
// RUN: %shermes -static-link -exec %s -Wx,-Xintl-plugin-path=%intl_plugin | %FileCheck --match-full-lines %s

print(typeof Intl);
// CHECK: object
