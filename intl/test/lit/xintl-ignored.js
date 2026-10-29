/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// -Xintl (RuntimeConfig's Intl field) is deprecated and ignored: it enabled
// Hermes's old built-in Intl. Either way, Intl is the plugin's.

// RUN: %hermes -Xintl %s | %FileCheck --match-full-lines %s
// RUN: %hermes -Xintl=false %s | %FileCheck --match-full-lines %s
// RUN: %shermes -exec %s -Wx,-Xintl | %FileCheck --match-full-lines %s
// RUN: %shermes -exec %s -Wx,-Xintl=false | %FileCheck --match-full-lines %s

// CHECK: object function

print(typeof Intl, typeof Intl.Segmenter);
