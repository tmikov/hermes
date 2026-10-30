/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: cat %S/Inputs/escape.js %s > %t.js
// RUN: %hermes -Xintl-default-locale=en-US -Xintl-default-time-zone=UTC %t.js | %FileCheck --match-full-lines %s

// The available locales are those ICU4X 2.4's compiled data covers
// (core/data/locales.txt). CLDR 49 dropped ht, so it is not available:
// LookupMatchingLocaleByPrefix finds no prefix of it, and ResolveLocale
// takes the default locale. ICU4X 2.4 added locales such as ckb and kbd,
// which resolve to themselves.

var CONSTRUCTORS = [
  'Collator', 'DateTimeFormat', 'DisplayNames', 'DurationFormat',
  'ListFormat', 'NumberFormat', 'PluralRules', 'RelativeTimeFormat',
  'Segmenter',
];
var NEW = ['ady', 'ckb', 'ckb-IR', 'cop', 'kbd', 'suz-Sunu'];
for (var i = 0; i < CONSTRUCTORS.length; i++) {
  var c = Intl[CONSTRUCTORS[i]];
  print(
    CONSTRUCTORS[i],
    JSON.stringify(c.supportedLocalesOf(['ht'])),
    JSON.stringify(c.supportedLocalesOf(['ht', 'fr'])),
    JSON.stringify(c.supportedLocalesOf(NEW)) === JSON.stringify(NEW),
  );
}
// CHECK: Collator [] ["fr"] true
// CHECK-NEXT: DateTimeFormat [] ["fr"] true
// CHECK-NEXT: DisplayNames [] ["fr"] true
// CHECK-NEXT: DurationFormat [] ["fr"] true
// CHECK-NEXT: ListFormat [] ["fr"] true
// CHECK-NEXT: NumberFormat [] ["fr"] true
// CHECK-NEXT: PluralRules [] ["fr"] true
// CHECK-NEXT: RelativeTimeFormat [] ["fr"] true
// CHECK-NEXT: Segmenter [] ["fr"] true

// ht resolves to the default locale and formats as it does; canonicalizing
// it does not depend on the data.
var date = Date.UTC(2026, 9, 30, 12, 0, 0);
var htNumber = new Intl.NumberFormat('ht');
var htDate = new Intl.DateTimeFormat('ht', {dateStyle: 'long'});
print(
  htNumber.resolvedOptions().locale,
  htNumber.format(1234567.891) === new Intl.NumberFormat().format(1234567.891),
  htNumber.format(1234567.891),
);
// CHECK-NEXT: en-US true 1,234,567.891
print(
  htDate.resolvedOptions().locale,
  htDate.format(date) === new Intl.DateTimeFormat(undefined, {dateStyle: 'long'}).format(date),
  htDate.format(date),
);
// CHECK-NEXT: en-US true October 30, 2026
print(
  new Intl.PluralRules('ht').resolvedOptions().locale,
  JSON.stringify(new Intl.PluralRules('ht').resolvedOptions().pluralCategories),
  JSON.stringify(Intl.getCanonicalLocales('ht')),
);
// CHECK-NEXT: en-US ["one","other"] ["ht"]
// A later requested locale that is available is chosen.
var htFr = new Intl.NumberFormat(['ht', 'fr']);
print(htFr.resolvedOptions().locale, escape(htFr.format(1234567.891)));
// CHECK-NEXT: fr 1\u202f234\u202f567,891

// New locales resolve to themselves and format numbers and dates with
// their own CLDR 49 data: ckb's default numbering system is arab, and
// ckb-IR's calendar is persian.
var LOCALES = ['ckb', 'ckb-IR', 'kbd'];
for (var j = 0; j < LOCALES.length; j++) {
  var nf = new Intl.NumberFormat(LOCALES[j]);
  var df = new Intl.DateTimeFormat(LOCALES[j], {dateStyle: 'long'});
  print(
    nf.resolvedOptions().locale,
    nf.resolvedOptions().numberingSystem,
    escape(nf.format(1234567.891)),
    df.resolvedOptions().locale,
    df.resolvedOptions().calendar,
    escape(df.format(date)),
  );
}
// CHECK-NEXT: ckb arab ١٬٢٣٤٬٥٦٧٫٨٩١ ckb gregory ٣٠ی تشرینی یەکەمی ٢٠٢٦
// CHECK-NEXT: ckb-IR arab ١٬٢٣٤٬٥٦٧٫٨٩١ ckb-IR persian ٨ی گەڵاڕێزانی ١٤٠٥ AP
// CHECK-NEXT: kbd latn 1\u00a0234\u00a0567,891 kbd gregory 2026 жэпуэгъуэм 30
