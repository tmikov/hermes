/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: cat %S/Inputs/escape.js %S/Inputs/datetime-locales.js %s > %t.js
// RUN: %hermes -Xintl-default-locale=en-US %t.js | %FileCheck --match-full-lines %s

// Intl.DurationFormat's data for many locales (plan Review Focus 5, the JS
// half; Task 1 checks the time separator of every locale in Rust): for the
// 40 locales of Inputs/datetime-locales.js, each style formats a duration
// with days, hours, minutes, seconds and milliseconds without an
// exception, and formatToParts joins to format. A few results show the
// numbering systems and the separators.

var STYLES = ['long', 'short', 'narrow', 'digital'];
var DURATION = {days: 1, hours: 2, minutes: 3, seconds: 4, milliseconds: 5};
var cases = 0;
var problems = [];
for (var l = 0; l < LOCALES.length; l++) {
  for (var s = 0; s < STYLES.length; s++) {
    var label = LOCALES[l] + ' ' + STYLES[s];
    try {
      var df = new Intl.DurationFormat(LOCALES[l], {style: STYLES[s]});
      var formatted = df.format(DURATION);
      var parts = df.formatToParts(DURATION);
      var joined = '';
      for (var p = 0; p < parts.length; p++) joined += parts[p].value;
      if (formatted === '') problems.push(label + ': empty');
      if (joined !== formatted) problems.push(label + ': ' + formatted + ' / ' + joined);
    } catch (e) {
      problems.push(label + ': ' + e);
    }
    cases++;
  }
}
print(cases, problems.length === 0 ? 'ok' : problems.join('\n'));
// CHECK: 160 ok

/// The digital format of DURATION in locale.
function digital(locale) {
  return escape(new Intl.DurationFormat(locale, {style: 'digital'}).format(DURATION));
}
print(digital('fa'));
// CHECK-NEXT: ۱ روز،\u200f ۲:۰۳:۰۴٫۰۰۵
print(digital('id'), '|', digital('bn'), '|', digital('my'));
// CHECK-NEXT: 1 hr, 2.03.04,005 | ১ দিন, ২:০৩:০৪.০০৫ | ၁ ရက်နှင့် ၂:၀၃:၀၄.၀၀၅
