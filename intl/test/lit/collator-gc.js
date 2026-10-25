/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US %s | %FileCheck --match-full-lines %s

// Collators freed by the GC. 2,000 collators are made with gc() every 100,
// every tenth is kept, and the survivors compare correctly afterwards. The
// script ends with live collators, so the runtime frees their native state
// when it is destroyed. Under ASan, LeakSanitizer and use-after-free checks
// cover both.

/// The options of collator i: four sensitivities, each with and without
/// numeric ordering. They change every ten collators, so the survivors
/// (every tenth) cover all eight combinations, and every locale with each.
function optionsOf(i) {
  var sensitivities = ['base', 'accent', 'case', 'variant'];
  var group = Math.floor(i / 10);
  return {sensitivity: sensitivities[group % 4], numeric: Math.floor(group / 4) % 2 === 1};
}

/// The locale of collator i.
function localeOf(i) {
  return locales[Math.floor(i / 80) % 4];
}

/// What collator i must give for the comparisons in check(): "a" against
/// "A", "a" against "á", and "2" against "10".
function expected(i) {
  var o = optionsOf(i);
  var caseDiffers = o.sensitivity === 'case' || o.sensitivity === 'variant';
  var accentDiffers = o.sensitivity === 'accent' || o.sensitivity === 'variant';
  return [caseDiffers ? -1 : 0, accentDiffers ? -1 : 0, o.numeric ? -1 : 1].join();
}

/// Whether collator c, made as collator i, compares as it should.
function check(c, i) {
  var compare = c.compare;
  return [compare('a', 'A'), compare('a', 'á'), compare('2', '10')].join() === expected(i);
}

var locales = ['en', 'de', 'sv', 'fr'];
var kept = [];
var keptIndex = [];
var wrong = 0;
for (var i = 0; i < 2000; i++) {
  var c = new Intl.Collator(localeOf(i), optionsOf(i));
  // Half of them get their bound compare before they die.
  if (i % 2 === 0 && !check(c, i)) wrong++;
  if (i % 10 === 0) {
    kept.push(c);
    keptIndex.push(i);
  }
  if (i % 100 === 99) gc();
}
gc();

for (var k = 0; k < kept.length; k++) {
  if (!check(kept[k], keptIndex[k])) wrong++;
  if (kept[k].resolvedOptions().locale !== localeOf(keptIndex[k])) wrong++;
}
print(kept.length, 'kept,', wrong, 'wrong');
// CHECK: 200 kept, 0 wrong
// The survivors cover every locale with every sensitivity, with and without
// numeric ordering.
var combinations = new Set();
for (var q = 0; q < keptIndex.length; q++)
  combinations.add(localeOf(keptIndex[q]) + ' ' + JSON.stringify(optionsOf(keptIndex[q])));
print(combinations.size, 'combinations');
// CHECK-NEXT: 32 combinations

// The survivors' bound compares outlive a collection too.
var compares = [];
for (var m = 0; m < kept.length; m++) compares.push(kept[m].compare);
kept = null;
gc();
var sorted = ['b', 'a', 'c'].sort(compares[0]);
print(sorted.join(), compares.length);
// CHECK-NEXT: a,b,c 200
