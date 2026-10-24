/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The plugin binds to JSI in a static host (the hermes CLI, which exports its
// JSI) and in a shared-library host (hermes-jsi-shared, which gets JSI from
// libhermesvm), each finding it by default resolution in the build tree.

// RUN: %hermes_default %s | %FileCheck --match-full-lines %s
// RUN: %hermes_jsi_shared --eval "Object.prototype.toString.call(Intl)" --print | %FileCheck --match-full-lines %s
print(Object.prototype.toString.call(Intl));
// CHECK: [object Intl]
