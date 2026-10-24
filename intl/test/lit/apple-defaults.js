/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// CoreFoundation detection with a per-process language override and TZ:
// the first preferred language that is usable becomes the default locale
// (sr-Latn-RS is available), and the zone becomes its primary identifier.
// REQUIRES: darwin
// RUN: env TZ=Asia/Calcutta %intl_defaults_probe -AppleLanguages '(sr-Latn-RS, de)' | %FileCheck --match-full-lines %s
// CHECK: sr-Latn-RS
// CHECK-NEXT: Asia/Kolkata
