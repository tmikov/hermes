/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

/*
RUN: TZ=EST+5 LC_ALL=en_US _HERMES_TEST_LOCALE=en_US %hermes -Xintl-plugin=false -O -target=HBC %s \
RUN:            | %FileCheck --match-full-lines -check-prefix US %s
RUN: TZ=EST+5 LC_ALL=tr_TR _HERMES_TEST_LOCALE=tr_TR %hermes -Xintl-plugin=false -O -target=HBC %s \
RUN:            | %FileCheck --match-full-lines -check-prefix TR %s
RUN: TZ=EST+5 LC_ALL=tr_TR _HERMES_TEST_LOCALE=tr_TR %shermes -exec %s -Wx,-Xintl-plugin=false \
RUN:            | %FileCheck --match-full-lines -check-prefix TR %s
TODO(T53144040) Fix LIT tests on Windows
XFAIL: windows
REQUIRES: locale_aware_dates
UNSUPPORTED: intl
*/

// The Intl plugin replaces Date.prototype.toLocaleString,
// toLocaleDateString and toLocaleTimeString with their ECMA-402 versions,
// so these runs turn it off (-Xintl-plugin=false) to test Hermes's own;
// intl/test/lit/to-locale-date-string.js tests the plugin's.

"use strict";

print('toLocaleString');
// US-LABEL: toLocaleString
// TR-LABEL: toLocaleString
print(new Date(112).toLocaleString());
// US-NEXT: Dec 31, 1969{{.+}}7:00:00{{( | )}}PM
// TR-NEXT: 31 Ara 1969{{.+}}19:00:00

print('toLocaleDateString');
// US-LABEL: toLocaleDateString
// TR-LABEL: toLocaleDateString
print(new Date(112).toLocaleDateString());
// US-NEXT: Dec 31, 1969
// TR-NEXT: 31 Ara 1969

print('toLocaleTimeString');
// US-LABEL: toLocaleTimeString
// TR-LABEL: toLocaleTimeString
print(new Date(112).toLocaleTimeString());
// US-NEXT: 7:00:00{{( | )}}PM
// TR-NEXT: 19:00:00

print('Invalid');
// US-LABEL: Invalid
// TR-LABEL: Invalid
print(new Date(NaN).toLocaleTimeString());
// US-NEXT: Invalid Date
// TR-NEXT: Invalid Date
