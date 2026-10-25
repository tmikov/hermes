/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US %s | %FileCheck --match-full-lines %s

// PluralRules freed by the GC. Each Intl.PluralRules has native plural
// rules of its own (plan decision 13), attached when it is constructed and
// freed with it. 2,000 PluralRules, cardinal and ordinal, over 40 locales
// and the four notations, are made; gc() runs every 100 and every tenth is
// kept. The survivors select correctly afterwards. The script ends with
// live PluralRules, so the runtime frees what is left when it is
// destroyed. Under ASan, LeakSanitizer and use-after-free checks cover
// both.

var locales = [
  'en', 'de', 'fr', 'ru', 'ar', 'ja', 'ko', 'zh', 'pl', 'cs',
  'uk', 'he', 'it', 'es', 'pt', 'nl', 'sv', 'da', 'fi', 'nb',
  'tr', 'el', 'hu', 'ro', 'bg', 'hr', 'sr', 'sl', 'sk', 'lt',
  'lv', 'et', 'ga', 'cy', 'is', 'mt', 'hi', 'bn', 'th', 'vi',
];
var notations = ['standard', 'scientific', 'engineering', 'compact'];

/// The type of PluralRules i: it changes every ten, so the survivors
/// (every tenth) have both.
function typeOf(i) {
  return Math.floor(i / 10) % 2 === 0 ? 'cardinal' : 'ordinal';
}

/// The locale of PluralRules i. It changes every 20, so the survivors
/// cover every locale with both types.
function localeOf(i) {
  return locales[Math.floor(i / 20) % locales.length];
}

/// The notation of PluralRules i: it changes every 500, so each notation
/// has 500 PluralRules.
function notationOf(i) {
  return notations[Math.floor(i / 500)];
}

/// The results of the first PluralRules of each kind to be checked, keyed
/// by locale, type and notation. A later one that disagrees is wrong.
var expectedOf = Object.create(null);

/// What pr gives for 0, 1, 2, 3, 5, 11, 22, 1.5 and 1e6, its categories
/// and a range.
function results(pr) {
  return (
    [0, 1, 2, 3, 5, 11, 22, 1.5, 1e6].map(x => pr.select(x)).join(',') +
    '|' +
    pr.resolvedOptions().pluralCategories.join(',') +
    '|' +
    pr.selectRange(1, 2)
  );
}
/// The key of PluralRules i.
function keyOf(i) {
  return localeOf(i) + ' ' + typeOf(i) + ' ' + notationOf(i);
}
/// Whether pr, made as PluralRules i, selects as the first of its kind did.
function check(pr, i) {
  var key = keyOf(i);
  var r = results(pr);
  if (!(key in expectedOf)) expectedOf[key] = r;
  return expectedOf[key] === r;
}

var kept = [];
var keptIndex = [];
var wrong = 0;
for (var i = 0; i < 2000; i++) {
  var pr = new Intl.PluralRules(localeOf(i), {type: typeOf(i), notation: notationOf(i)});
  // Half of them select before they die.
  if (i % 2 === 0 && !check(pr, i)) wrong++;
  if (i % 10 === 0) {
    kept.push(pr);
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
// The survivors cover every locale with both types.
var combinations = new Set();
for (var q = 0; q < keptIndex.length; q++)
  combinations.add(localeOf(keptIndex[q]) + ' ' + typeOf(keptIndex[q]));
print(combinations.size, 'combinations');
// CHECK-NEXT: 80 combinations

// A few of the results, so a wrong category everywhere is caught too.
// CLDR's plural ranges are for cardinals. ICU4X uses them for ordinals
// too, and a pair they do not list (one to two) gets the end's category,
// "two"; ICU4C gives "other".
print(expectedOf['en cardinal standard']);
// CHECK-NEXT: other,one,other,other,other,other,other,other,other|one,other|other
print(expectedOf['en ordinal standard']);
// CHECK-NEXT: other,one,two,few,other,other,two,other,other|one,two,few,other|two
print(expectedOf['ru cardinal standard']);
// CHECK-NEXT: many,one,few,few,many,many,few,other,many|one,few,many,other|few
print(expectedOf['fr cardinal compact']);
// CHECK-NEXT: one,one,other,other,other,other,other,one,many|one,many,other|other
