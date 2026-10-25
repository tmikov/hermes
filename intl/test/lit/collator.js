/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US %s | %FileCheck --match-full-lines %s

// The basic surface of Intl.Collator: the resolved options, a collation from
// the tag, the call without new, supportedLocalesOf, compare and
// @@toStringTag.

// The default collator, in the default locale.
print(JSON.stringify(new Intl.Collator().resolvedOptions()));
// CHECK: {"locale":"en-US","usage":"sort","sensitivity":"variant","ignorePunctuation":false,"collation":"default","numeric":false,"caseFirst":"false"}

// A supported collation in the tag is kept in the locale.
var phonebk = new Intl.Collator('de-u-co-phonebk').resolvedOptions();
print(phonebk.locale, phonebk.collation);
// CHECK-NEXT: de-u-co-phonebk phonebk

// Called without new, Intl.Collator still makes a Collator.
var called = Intl.Collator('de');
print(
  called instanceof Intl.Collator,
  Object.getPrototypeOf(called) === Intl.Collator.prototype,
  called.resolvedOptions().locale,
);
// CHECK-NEXT: true true de

print(JSON.stringify(Intl.Collator.supportedLocalesOf(['de', 'xx', 'zh-Hant-TW'])));
// CHECK-NEXT: ["de","zh-Hant-TW"]

// German sorts a with umlaut after a and before b, and lowercase first.
print(JSON.stringify(['b', 'a', 'B', 'ä'].sort(new Intl.Collator('de').compare)));
// CHECK-NEXT: ["a","ä","b","B"]

// The getter returns the same bound function each time.
var collator = new Intl.Collator();
print(collator.compare === collator.compare);
// CHECK-NEXT: true

print(Object.prototype.toString.call(new Intl.Collator()));
// CHECK-NEXT: [object Intl.Collator]

// Every collation Intl.supportedValuesOf lists is resolved for one of the
// locales of test262's collations-accepted-by-Collator.js; the first such
// locale is printed.
var locales = ['en', 'ar', 'de', 'es', 'hi', 'ko', 'ln', 'si', 'sv', 'zh'];
var collations = Intl.supportedValuesOf('collation');
for (var i = 0; i < collations.length; i++) {
  var found = 'none';
  for (var j = 0; j < locales.length; j++) {
    var c = new Intl.Collator(locales[j], {collation: collations[i]});
    if (c.resolvedOptions().collation === collations[i]) {
      found = locales[j];
      break;
    }
  }
  print(collations[i], found);
}
// CHECK-NEXT: compat ar
// CHECK-NEXT: dict si
// CHECK-NEXT: emoji en
// CHECK-NEXT: eor en
// CHECK-NEXT: phonebk de
// CHECK-NEXT: pinyin zh
// CHECK-NEXT: stroke zh
// CHECK-NEXT: trad es
// CHECK-NEXT: unihan ko
// CHECK-NEXT: zhuyin zh
