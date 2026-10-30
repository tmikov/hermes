/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US -Xintl-default-time-zone=UTC %s | %FileCheck --match-full-lines %s

// The month and day widths of the default Intl.DateTimeFormat and of
// {year, month, day: "numeric"}: best fit must pick each locale's numeric
// pattern, not a text month that costs less by BasicFormatMatcher's
// penalties (plan decision 6, refined). The values are Node's (ICU 78.2),
// except two: Node has no und and falls back to en-US, while the plugin
// resolves und itself, whose root pattern is "y-MM-dd"; and tr's yMd is
// "d.M.y" in CLDR 49, where Node's CLDR 48 has "dd.MM.y".

var LOCALES = [
  'en', 'en-GB', 'en-AU', 'en-001', 'en-IN', 'fr', 'de', 'it', 'es', 'pt-BR',
  'ru', 'sv', 'tr', 'nl', 'pl', 'ja', 'zh', 'ko', 'ar', 'he', 'und',
];
for (var i = 0; i < LOCALES.length; i++) {
  var a = new Intl.DateTimeFormat(LOCALES[i]).resolvedOptions();
  var b = new Intl.DateTimeFormat(LOCALES[i], {
    year: 'numeric',
    month: 'numeric',
    day: 'numeric',
  }).resolvedOptions();
  print(LOCALES[i], a.year, a.month, a.day, '|', b.year, b.month, b.day);
}
// CHECK: en numeric numeric numeric | numeric numeric numeric
// CHECK-NEXT: en-GB numeric 2-digit 2-digit | numeric 2-digit 2-digit
// CHECK-NEXT: en-AU numeric 2-digit 2-digit | numeric 2-digit 2-digit
// CHECK-NEXT: en-001 numeric 2-digit 2-digit | numeric 2-digit 2-digit
// CHECK-NEXT: en-IN numeric numeric numeric | numeric numeric numeric
// CHECK-NEXT: fr numeric 2-digit 2-digit | numeric 2-digit 2-digit
// CHECK-NEXT: de numeric numeric numeric | numeric numeric numeric
// CHECK-NEXT: it numeric 2-digit 2-digit | numeric 2-digit 2-digit
// CHECK-NEXT: es numeric numeric numeric | numeric numeric numeric
// CHECK-NEXT: pt-BR numeric 2-digit 2-digit | numeric 2-digit 2-digit
// CHECK-NEXT: ru numeric 2-digit 2-digit | numeric 2-digit 2-digit
// CHECK-NEXT: sv numeric 2-digit 2-digit | numeric 2-digit 2-digit
// CHECK-NEXT: tr numeric numeric numeric | numeric numeric numeric
// CHECK-NEXT: nl numeric numeric numeric | numeric numeric numeric
// CHECK-NEXT: pl numeric 2-digit numeric | numeric 2-digit numeric
// CHECK-NEXT: ja numeric numeric numeric | numeric numeric numeric
// CHECK-NEXT: zh numeric numeric numeric | numeric numeric numeric
// CHECK-NEXT: ko numeric numeric numeric | numeric numeric numeric
// CHECK-NEXT: ar numeric numeric numeric | numeric numeric numeric
// CHECK-NEXT: he numeric numeric numeric | numeric numeric numeric
// CHECK-NEXT: und numeric 2-digit 2-digit | numeric 2-digit 2-digit
