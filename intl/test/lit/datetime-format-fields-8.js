/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: cat %S/Inputs/datetime-fields.js %s > %t.js
// RUN: %hermes -Xintl-default-locale=en-US -Xintl-default-time-zone=UTC %t.js | %FileCheck --match-full-lines %s

// Part 8 of the field-survival check (Inputs/datetime-fields.js): best fit
// keeps every requested field in en, every supported calendar. One
// locale per test, so that lit runs the parts in parallel.

checkFieldSurvival(7, 8);
// CHECK: 3264 configurations, 0 failures
