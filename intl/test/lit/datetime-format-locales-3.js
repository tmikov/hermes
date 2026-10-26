/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: cat %S/Inputs/datetime-locales.js %s > %t.js
// RUN: %hermes -Xintl-default-locale=en-US -Xintl-default-time-zone=UTC %t.js | %FileCheck --match-full-lines %s

// Part 3 of datetime-format-locales.js's check of the draft's 14 required
// field sets with formatMatcher "basic" (plan Review Focus 5): locales 20
// to 29 of LOCALES (ta to sw), every supported calendar.

checkBasicSets(20, 30);
// CHECK: 2240 basic cases: ok true
