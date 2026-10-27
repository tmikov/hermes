/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US %s | %FileCheck --match-full-lines %s

// ListFormats freed by the GC. Each Intl.ListFormat has a native list
// formatter of its own (plan decision 10), attached when it is constructed
// and freed with it. 2,000 ListFormats over 20 locales, the three types
// and the three styles are made; gc() runs every 100 and every tenth is
// kept. The survivors format correctly afterwards. The script ends with
// live ListFormats, so the runtime frees what is left when it is
// destroyed. Under ASan, LeakSanitizer and use-after-free checks cover
// both.

var locales = [
  'en', 'de', 'fr', 'es', 'he', 'ar', 'ja', 'zh', 'ru', 'ti',
  'mn', 'it', 'pt', 'nl', 'sv', 'pl', 'tr', 'hi', 'ko', 'th',
];
var types = ['conjunction', 'disjunction', 'unit'];
var styles = ['long', 'short', 'narrow'];

/// The locale of ListFormat i: it changes every 10, so the survivors (every
/// tenth) cover every locale.
function localeOf(i) {
  return locales[Math.floor(i / 10) % locales.length];
}
/// The type of ListFormat i: it changes every 200.
function typeOf(i) {
  return types[Math.floor(i / 200) % types.length];
}
/// The style of ListFormat i: it changes every 600.
function styleOf(i) {
  return styles[Math.floor(i / 600) % styles.length];
}
/// The key of ListFormat i.
function keyOf(i) {
  return localeOf(i) + ' ' + typeOf(i) + ' ' + styleOf(i);
}

/// The results of the first ListFormat of each kind to be checked, keyed
/// by locale, type and style. A later one that disagrees is wrong.
var expectedOf = Object.create(null);

/// What lf gives for lists of one to five elements, the last of which
/// chooses a conditional pattern in es and he.
function results(lf) {
  return [
    lf.format(['a']),
    lf.format(['a', 'Ibiza']),
    lf.format(['a', 'b', 'hielo']),
    JSON.stringify(lf.formatToParts(['a', 'b', 'c', 'ocho'])),
    lf.format(['a', 'b', 'c', 'd', 'בננה']),
  ].join('|');
}
/// Whether lf, made as ListFormat i, formats as the first of its kind did.
function check(lf, i) {
  var key = keyOf(i);
  var r = results(lf);
  if (!(key in expectedOf)) expectedOf[key] = r;
  return expectedOf[key] === r;
}

var kept = [];
var keptIndex = [];
var wrong = 0;
for (var i = 0; i < 2000; i++) {
  var lf = new Intl.ListFormat(localeOf(i), {type: typeOf(i), style: styleOf(i)});
  // Half of them format before they die.
  if (i % 2 === 0 && !check(lf, i)) wrong++;
  if (i % 10 === 0) {
    kept.push(lf);
    keptIndex.push(i);
  }
  if (i % 100 === 99) gc();
}
gc();

for (var k = 0; k < kept.length; k++) {
  if (!check(kept[k], keptIndex[k])) wrong++;
  var ro = kept[k].resolvedOptions();
  if (ro.locale !== localeOf(keptIndex[k]) || ro.type !== typeOf(keptIndex[k]) || ro.style !== styleOf(keptIndex[k]))
    wrong++;
}
print(kept.length, 'kept,', wrong, 'wrong');
// CHECK: 200 kept, 0 wrong
// The survivors cover every locale, type and style: 20 x 3 x 3.
var combinations = new Set();
for (var q = 0; q < keptIndex.length; q++) combinations.add(keyOf(keptIndex[q]));
print(combinations.size, 'combinations');
// CHECK-NEXT: 180 combinations

// A few of the results, so a wrong literal everywhere is caught too.
print(expectedOf['en conjunction long']);
// CHECK-NEXT: a|a and Ibiza|a, b, and hielo|[{"type":"element","value":"a"},{"type":"literal","value":", "},{"type":"element","value":"b"},{"type":"literal","value":", "},{"type":"element","value":"c"},{"type":"literal","value":", and "},{"type":"element","value":"ocho"}]|a, b, c, d, and בננה
print(expectedOf['es conjunction long']);
// CHECK-NEXT: a|a e Ibiza|a, b y hielo|[{"type":"element","value":"a"},{"type":"literal","value":", "},{"type":"element","value":"b"},{"type":"literal","value":", "},{"type":"element","value":"c"},{"type":"literal","value":" y "},{"type":"element","value":"ocho"}]|a, b, c, d y בננה
print(expectedOf['es disjunction short']);
// CHECK-NEXT: a|a o Ibiza|a, b o hielo|[{"type":"element","value":"a"},{"type":"literal","value":", "},{"type":"element","value":"b"},{"type":"literal","value":", "},{"type":"element","value":"c"},{"type":"literal","value":" u "},{"type":"element","value":"ocho"}]|a, b, c, d o בננה
