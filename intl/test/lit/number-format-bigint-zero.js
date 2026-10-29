/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes %s | %FileCheck --match-full-lines %s

// NumberFormat formats the BigInt 0n (once a crash in Hermes's old built-in
// Intl, test/hermes/regress-intl-number-format-bigint-input.js).

var nf = new Intl.NumberFormat('en');
print(nf.format(0n), nf.format(-0n));
// CHECK: 0 0
print(JSON.stringify(nf.formatToParts(0n)));
// CHECK-NEXT: [{"type":"integer","value":"0"}]
