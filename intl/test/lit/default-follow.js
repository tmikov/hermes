/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// When an existing build directory's HERMES_BUILD_INTL follows a new
// default (hermes_intl_build_option in cmake/modules/HermesIntl.cmake): while
// it still holds the default it was given, and never once the user chose a
// value.
// RUN: rm -rf %t && mkdir -p %t

// A new build directory takes the default and records it; it follows the
// default when it changes, both ways.
// RUN: %cmake -S %S/Inputs/default-follow -B %t/a -DTEST_DEFAULT=OFF 2>&1 | %FileCheck --check-prefix=A1 %s
// A1: Intl plugin off by default: the test says so
// A1: HERMES_BUILD_INTL=OFF recorded=[OFF]
// RUN: %cmake -S %S/Inputs/default-follow -B %t/a -DTEST_DEFAULT=ON 2>&1 | %FileCheck --check-prefix=A2 %s
// A2: HERMES_BUILD_INTL follows its new default, ON
// A2: HERMES_BUILD_INTL=ON recorded=[ON]
// RUN: %cmake -S %S/Inputs/default-follow -B %t/a 2>&1 | %FileCheck --check-prefix=A3 %s
// A3-NOT: follows
// A3: HERMES_BUILD_INTL=ON recorded=[ON]
// RUN: %cmake -S %S/Inputs/default-follow -B %t/a -DTEST_DEFAULT=OFF 2>&1 | %FileCheck --check-prefix=A4 %s
// A4: HERMES_BUILD_INTL follows its new default, OFF
// A4: HERMES_BUILD_INTL=OFF recorded=[OFF]

// A value given on the command line is the user's, even when it equals the
// recorded default, and stays when the default changes later.
// RUN: %cmake -S %S/Inputs/default-follow -B %t/b -DTEST_DEFAULT=OFF 2>&1 | %FileCheck --check-prefix=B1 %s
// B1: HERMES_BUILD_INTL=OFF recorded=[OFF]
// RUN: %cmake -S %S/Inputs/default-follow -B %t/b -DTEST_DEFAULT=ON -DHERMES_BUILD_INTL=OFF 2>&1 | %FileCheck --check-prefix=B2 %s
// B2-NOT: follows
// B2: HERMES_BUILD_INTL=OFF recorded=[]
// RUN: %cmake -S %S/Inputs/default-follow -B %t/b -DTEST_DEFAULT=ON 2>&1 | %FileCheck --check-prefix=B3 %s
// B3-NOT: follows
// B3: HERMES_BUILD_INTL=OFF recorded=[]

// A value given on the first configure is never recorded as a default.
// RUN: %cmake -S %S/Inputs/default-follow -B %t/c -DTEST_DEFAULT=ON -DHERMES_BUILD_INTL=ON 2>&1 | %FileCheck --check-prefix=C1 %s
// C1: HERMES_BUILD_INTL=ON recorded=[]
// RUN: %cmake -S %S/Inputs/default-follow -B %t/c -DTEST_DEFAULT=OFF 2>&1 | %FileCheck --check-prefix=C2 %s
// C2-NOT: follows
// C2: HERMES_BUILD_INTL=ON recorded=[]

// A value changed in the cache by other means (an editor, ccmake) differs
// from the recorded default, so it stays.
// RUN: %cmake -S %S/Inputs/default-follow -B %t/d -DTEST_DEFAULT=ON 2>&1 | %FileCheck --check-prefix=D1 %s
// D1: HERMES_BUILD_INTL=ON recorded=[ON]
// RUN: sed -e "s/^HERMES_BUILD_INTL:BOOL=ON/HERMES_BUILD_INTL:BOOL=OFF/" %t/d/CMakeCache.txt > %t/cache.txt
// RUN: cp %t/cache.txt %t/d/CMakeCache.txt
// RUN: %cmake -S %S/Inputs/default-follow -B %t/d -DTEST_DEFAULT=OFF 2>&1 | %FileCheck --check-prefix=D2 %s
// D2-NOT: follows
// D2: HERMES_BUILD_INTL=OFF recorded=[ON]
// RUN: %cmake -S %S/Inputs/default-follow -B %t/d -DTEST_DEFAULT=ON 2>&1 | %FileCheck --check-prefix=D3 %s
// D3-NOT: follows
// D3: HERMES_BUILD_INTL=OFF recorded=[ON]
