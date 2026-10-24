/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The tools that link the VM statically and create runtimes find the plugin
// in <build>/lib by default resolution. It binds to the JSI code they
// export (hermes_export_dynamic_symbols in cmake/modules/Hermes.cmake);
// without those exports, loading it is fatal.

// RUN: %hermes_default %s | %FileCheck --match-full-lines %s
// RUN: %hermesc -emit-binary -out=%t.hbc %s && %hvm %t.hbc | %FileCheck --match-full-lines %s
// CHECK: [object Intl]

print(Object.prototype.toString.call(Intl));
