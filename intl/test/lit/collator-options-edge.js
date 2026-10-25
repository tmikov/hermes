/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US %s | %FileCheck --match-full-lines %s

// Locales and options with no data or odd values. Each case prints the
// resolved options, or the class of the error. None reaches native code
// with a value it rejects, which would surface as "Exception in
// HostFunction:".

/// The resolved options of new Intl.Collator(locales, options) as JSON, or
/// the class of the error.
function run(locales, options) {
  try {
    print(JSON.stringify(new Intl.Collator(locales, options).resolvedOptions()));
  } catch (e) {
    var message = String(e.message);
    print(message.indexOf('HostFunction') !== -1 ? 'native: ' + message : e.constructor.name);
  }
}

// Locales with no data of their own. "und" is available; "zxx" and "posix"
// fall back to the default locale.
run('und');
// CHECK: {"locale":"und","usage":"sort","sensitivity":"variant","ignorePunctuation":false,"collation":"default","numeric":false,"caseFirst":"false"}
run('zxx');
// CHECK-NEXT: {"locale":"en-US","usage":"sort","sensitivity":"variant","ignorePunctuation":false,"collation":"default","numeric":false,"caseFirst":"false"}
run('posix');
// CHECK-NEXT: {"locale":"en-US","usage":"sort","sensitivity":"variant","ignorePunctuation":false,"collation":"default","numeric":false,"caseFirst":"false"}

// "standard" and "search" are never collations: the keyword is dropped.
run('en-u-co-standard');
// CHECK-NEXT: {"locale":"en","usage":"sort","sensitivity":"variant","ignorePunctuation":false,"collation":"default","numeric":false,"caseFirst":"false"}
run('en-u-co-search');
// CHECK-NEXT: {"locale":"en","usage":"sort","sensitivity":"variant","ignorePunctuation":false,"collation":"default","numeric":false,"caseFirst":"false"}
// CanonicalizeUnicodeLocaleId turns kn's "yes" into "true", which it then
// drops (UTS 35 Annex C), so this is "en-u-kn". kf has no such alias: "yes"
// is not one of its values, and kf keeps its default.
run('en-u-kn-yes');
// CHECK-NEXT: {"locale":"en-u-kn","usage":"sort","sensitivity":"variant","ignorePunctuation":false,"collation":"default","numeric":true,"caseFirst":"false"}
run('en-u-kf-yes');
// CHECK-NEXT: {"locale":"en","usage":"sort","sensitivity":"variant","ignorePunctuation":false,"collation":"default","numeric":false,"caseFirst":"false"}
// A kn with no value is "true", kept in the locale.
run('en-u-kn');
// CHECK-NEXT: {"locale":"en-u-kn","usage":"sort","sensitivity":"variant","ignorePunctuation":false,"collation":"default","numeric":true,"caseFirst":"false"}
// Every locale has the root's emoji and eor collations, Thai too, but no
// phone book collation.
run('th-u-co-emoji');
// CHECK-NEXT: {"locale":"th-u-co-emoji","usage":"sort","sensitivity":"variant","ignorePunctuation":true,"collation":"emoji","numeric":false,"caseFirst":"false"}
run('th-u-co-phonebk');
// CHECK-NEXT: {"locale":"th","usage":"sort","sensitivity":"variant","ignorePunctuation":true,"collation":"default","numeric":false,"caseFirst":"false"}
// The collation option replaces the keyword, which leaves the locale.
run('de-u-co-phonebk', {collation: 'eor'});
// CHECK-NEXT: {"locale":"de","usage":"sort","sensitivity":"variant","ignorePunctuation":false,"collation":"eor","numeric":false,"caseFirst":"false"}
// An unsupported collation option leaves the keyword in place.
run('de-u-co-phonebk', {collation: 'pinyin'});
// CHECK-NEXT: {"locale":"de-u-co-phonebk","usage":"sort","sensitivity":"variant","ignorePunctuation":false,"collation":"phonebk","numeric":false,"caseFirst":"false"}

// Options that are not Unicode type values, or not among the values.
run('en', {collation: ''});
// CHECK-NEXT: RangeError
run('en', {collation: 'abcdefghi'});
// CHECK-NEXT: RangeError
run('en', {caseFirst: 'FALSE'});
// CHECK-NEXT: RangeError
run('en', {sensitivity: 'Base'});
// CHECK-NEXT: RangeError
run('en', {usage: 'SORT'});
// CHECK-NEXT: RangeError
// numeric is converted with ToBoolean, so "false" is true.
run('en', {numeric: 'false'});
// CHECK-NEXT: {"locale":"en","usage":"sort","sensitivity":"variant","ignorePunctuation":false,"collation":"default","numeric":true,"caseFirst":"false"}
// ignorePunctuation is converted with ToBoolean too.
run('en', {ignorePunctuation: 'no'});
// CHECK-NEXT: {"locale":"en","usage":"sort","sensitivity":"variant","ignorePunctuation":true,"collation":"default","numeric":false,"caseFirst":"false"}
// Options that are not objects are converted with ToObject.
run('en', 5);
// CHECK-NEXT: {"locale":"en","usage":"sort","sensitivity":"variant","ignorePunctuation":false,"collation":"default","numeric":false,"caseFirst":"false"}
run('en', null);
// CHECK-NEXT: TypeError

// Danish and Maltese sort uppercase first.
run('da');
// CHECK-NEXT: {"locale":"da","usage":"sort","sensitivity":"variant","ignorePunctuation":false,"collation":"default","numeric":false,"caseFirst":"upper"}
run('mt');
// CHECK-NEXT: {"locale":"mt","usage":"sort","sensitivity":"variant","ignorePunctuation":false,"collation":"default","numeric":false,"caseFirst":"upper"}
run('da', {caseFirst: 'false'});
// CHECK-NEXT: {"locale":"da","usage":"sort","sensitivity":"variant","ignorePunctuation":false,"collation":"default","numeric":false,"caseFirst":"false"}
// Thai ignores punctuation by default.
run('th');
// CHECK-NEXT: {"locale":"th","usage":"sort","sensitivity":"variant","ignorePunctuation":true,"collation":"default","numeric":false,"caseFirst":"false"}
run('th-TH');
// CHECK-NEXT: {"locale":"th-TH","usage":"sort","sensitivity":"variant","ignorePunctuation":true,"collation":"default","numeric":false,"caseFirst":"false"}
run('th', {ignorePunctuation: false});
// CHECK-NEXT: {"locale":"th","usage":"sort","sensitivity":"variant","ignorePunctuation":false,"collation":"default","numeric":false,"caseFirst":"false"}

// usage "search" takes its default sensitivity from the search data.
run('de', {usage: 'search'});
// CHECK-NEXT: {"locale":"de","usage":"search","sensitivity":"variant","ignorePunctuation":false,"collation":"default","numeric":false,"caseFirst":"false"}

// The options that change the comparison do change it.
print(
  new Intl.Collator('en', {sensitivity: 'base'}).compare('a', 'á'),
  new Intl.Collator('en', {sensitivity: 'accent'}).compare('a', 'á'),
  new Intl.Collator('en', {sensitivity: 'accent'}).compare('a', 'A'),
  new Intl.Collator('en', {sensitivity: 'case'}).compare('a', 'á'),
  new Intl.Collator('en', {sensitivity: 'case'}).compare('a', 'A'),
  new Intl.Collator('en', {sensitivity: 'variant'}).compare('a', 'á'),
);
// CHECK-NEXT: 0 -1 0 0 -1 -1
print(
  new Intl.Collator('en').compare('2', '10'),
  new Intl.Collator('en', {numeric: true}).compare('2', '10'),
  new Intl.Collator('en-u-kn').compare('2', '10'),
);
// CHECK-NEXT: 1 -1 -1
print(
  new Intl.Collator('en').compare('a', 'A'),
  new Intl.Collator('en', {caseFirst: 'upper'}).compare('a', 'A'),
  new Intl.Collator('en-u-kf-upper').compare('a', 'A'),
  new Intl.Collator('da').compare('a', 'A'),
);
// CHECK-NEXT: -1 1 1 1
print(
  new Intl.Collator('en').compare('a-b', 'ab'),
  new Intl.Collator('en', {ignorePunctuation: true}).compare('a-b', 'ab'),
  new Intl.Collator('th').compare('a-b', 'ab'),
);
// CHECK-NEXT: -1 0 0
// In the phone book collation, a with umlaut sorts as "ae".
print(
  new Intl.Collator('de').compare('ä', 'ad'),
  new Intl.Collator('de-u-co-phonebk').compare('ä', 'ad'),
  new Intl.Collator('de', {collation: 'phonebk'}).compare('ä', 'ad'),
);
// CHECK-NEXT: -1 1 1
