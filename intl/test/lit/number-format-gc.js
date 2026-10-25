/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US %s | %FileCheck --match-full-lines %s

// NumberFormats freed by the GC, and the internal plural-rules objects
// their plural forms come from. Formatters do not hold those objects: they
// live only in the internal cache, 32 entries keyed by locale (plan
// decision 16), and a formatter looks its locale's object up when it
// formats. 2,000 NumberFormats use the currency name and unit styles, both
// plural-dependent, over 40 locales, so the cache is emptied again and
// again. The test checks that the objects the cache drops are freed with
// their native state safely while formatters of their locales live on and
// get new ones. gc() runs every 100 formatters and every tenth formatter
// is kept; the survivors format correctly afterwards. The script ends with
// live formatters, so the runtime frees what is left when it is destroyed.
// Under ASan, LeakSanitizer and use-after-free checks cover both.

var locales = [
  'en', 'de', 'fr', 'ru', 'ar', 'ja', 'ko', 'zh', 'pl', 'cs',
  'uk', 'he', 'it', 'es', 'pt', 'nl', 'sv', 'da', 'fi', 'nb',
  'tr', 'el', 'hu', 'ro', 'bg', 'hr', 'sr', 'sl', 'sk', 'lt',
  'lv', 'et', 'ga', 'cy', 'is', 'mt', 'hi', 'bn', 'th', 'vi',
];

/// The style of formatter i: 0, the currency name style, or 1, the long
/// unit style. It changes every ten formatters, so the survivors (every
/// tenth) have both.
function styleOf(i) {
  return Math.floor(i / 10) % 2;
}

/// The locale of formatter i. It changes every 20 formatters, so 32
/// locales, and an emptied cache, come every 640 formatters, and the
/// survivors cover every locale with both styles.
function localeOf(i) {
  return locales[Math.floor(i / 20) % locales.length];
}

/// The options of formatter i.
function optionsOf(i) {
  return styleOf(i) === 0
    ? {style: 'currency', currency: 'EUR', currencyDisplay: 'name'}
    : {style: 'unit', unit: 'kilometer', unitDisplay: 'long'};
}

/// The results of the first formatter of each locale and style to be
/// checked, keyed by localeOf(i) + ' ' + styleOf(i). A later formatter that
/// disagrees is wrong.
var expectedOf = Object.create(null);

/// What nf gives for 1, 2, 5 and 1.5.
function results(nf) {
  return [1, 2, 5, 1.5].map(x => nf.format(x)).join('|');
}
/// Whether nf, made as formatter i, formats as the first of its kind did.
function check(nf, i) {
  var key = localeOf(i) + ' ' + styleOf(i);
  var r = results(nf);
  if (!(key in expectedOf)) expectedOf[key] = r;
  return expectedOf[key] === r;
}

var kept = [];
var keptIndex = [];
var wrong = 0;
for (var i = 0; i < 2000; i++) {
  var nf = new Intl.NumberFormat(localeOf(i), optionsOf(i));
  // Half of them format before they die.
  if (i % 2 === 0 && !check(nf, i)) wrong++;
  if (i % 10 === 0) {
    kept.push(nf);
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
// The survivors cover every locale with both styles.
var combinations = new Set();
for (var q = 0; q < keptIndex.length; q++)
  combinations.add(localeOf(keptIndex[q]) + ' ' + styleOf(keptIndex[q]));
print(combinations.size, 'combinations');
// CHECK-NEXT: 80 combinations

// A few of the results, so a wrong plural form everywhere is caught too.
print(expectedOf['en 0']);
// CHECK-NEXT: 1.00 euros|2.00 euros|5.00 euros|1.50 euros
print(expectedOf['en 1']);
// CHECK-NEXT: 1 kilometer|2 kilometers|5 kilometers|1.5 kilometers
print(expectedOf['ru 1']);
// CHECK-NEXT: 1 километр|2 километра|5 километров|1,5 километра

// The survivors' bound formats outlive a collection too.
var formats = [];
for (var m = 0; m < kept.length; m++) formats.push(kept[m].format);
kept = null;
gc();
print(formats[1](1), formats.length);
// CHECK-NEXT: 1 kilometer 200
