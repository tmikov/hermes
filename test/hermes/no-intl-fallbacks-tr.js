/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: _HERMES_TEST_LOCALE=tr %hermes -Xintl-plugin=false %s | %FileCheck --match-full-lines %s
// Only these Unicode backends take the default locale from
// _HERMES_TEST_LOCALE; Lite and Emscripten use the root case mappings, and
// Java uses its own default locale.
// REQUIRES: unicode_hermes || unicode_cf

// Without the Intl plugin, toLocaleLowerCase and toLocaleUpperCase fall back
// to Hermes's own case mappings, which follow the default locale.

print(typeof Intl);
// CHECK: undefined

print(escape('I'.toLocaleLowerCase()), escape('i'.toLocaleUpperCase()));
// CHECK-NEXT: %u0131 %u0130
