/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: cat %S/Inputs/strict.js %intl_js/01-intrinsics.js %intl_js/02-shapes.js %intl_js/03-locales.js %intl_js/04-options.js %intl_js/10-Intl.js %s > %t.js
// RUN: %hermes_default -Xintl-plugin=false %t.js | %FileCheck --match-full-lines %s
// RUN: %hermesc -O -emit-binary -out=%t.hbc %t.js && %hermes_default -Xintl-plugin=false -b %t.hbc | %FileCheck --match-full-lines %s

// Intl.supportedValuesOf (ECMA-402 8.3.2) with stub natives whose lists hold
// what the real data does not: calendar aliases, which step 2.b drops, and
// stray spaces, which splitting skips.

// Stubs for what the plugin's C++ passes to setup.
var native = {
  canonicalizeLocale: function (tag) {
    if (!/^[a-z]{2,3}(-[A-Za-z0-9]{1,8})*$/.test(tag)) return undefined;
    // Two UTS 35 calendar aliases.
    return tag
      .replace(/-u-ca-islamicc(?=-|$)/, '-u-ca-islamic-civil')
      .replace(/-u-ca-ethiopic-amete-alem(?=-|$)/, '-u-ca-ethioaa');
  },
  isAvailableLocale: function (tag) {
    return tag === 'en';
  },
  supportedValues: function (key) {
    if (key === 'calendar')
      return 'ethioaa ethiopic-amete-alem gregory islamic-civil islamicc iso8601';
    return ' ' + key + '-a  ' + key + '-b ';
  },
};
var hostDefaults = {locales: ['en'], timeZone: 'UTC'};
installIntl();

print(JSON.stringify(globalThis.Intl.supportedValuesOf('calendar')));
// CHECK: ["ethioaa","gregory","islamic-civil","iso8601"]
print(JSON.stringify(globalThis.Intl.supportedValuesOf('currency')));
// CHECK-NEXT: ["currency-a","currency-b"]
print(JSON.stringify(globalThis.Intl.supportedValuesOf('timeZone')));
// CHECK-NEXT: ["timeZone-a","timeZone-b"]
