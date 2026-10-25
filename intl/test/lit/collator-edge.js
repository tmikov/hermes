/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US %s | %FileCheck --match-full-lines %s

// Unusual text through Intl.Collator's compare: unpaired surrogates,
// supplementary characters, NUL, characters whose case mappings change the
// length, and a 1,000,000-character string. compare(x, x) is 0, compare is
// antisymmetric, NUL is not ignored, and nothing crashes.

var compare = new Intl.Collator('en').compare;

var big = 'a'.repeat(1000000);
var texts = [
  '\uD800',
  '\uDC00',
  'a\uDC00b',
  'a\uD800b',
  '\uD801\uDC28', // U+10428 DESERET SMALL LETTER LONG I
  '\uD801\uDC00', // U+10400 DESERET CAPITAL LETTER LONG I
  'a\0b',
  'a\0c',
  'ab',
  '\u00DF',
  'ss',
  '\uFB03',
  'ffi',
  '\u0130',
  'i\u0307',
  '',
  big,
  big + 'b',
];

/// "ok" if compare(x, x) is 0 for every text and compare(x, y) is
/// -compare(y, x) for every pair, or the first case that is not.
function properties() {
  for (var i = 0; i < texts.length; i++) {
    if (compare(texts[i], texts[i]) !== 0) return 'not reflexive: ' + i;
    for (var j = 0; j < texts.length; j++) {
      var xy = compare(texts[i], texts[j]);
      var yx = compare(texts[j], texts[i]);
      if (xy !== -yx) return 'not antisymmetric: ' + i + ' ' + j;
      if (xy !== -1 && xy !== 0 && xy !== 1) return 'not -1, 0 or 1: ' + i + ' ' + j;
    }
  }
  return 'ok';
}
print(properties());
// CHECK: ok

// NUL is a character: the strings differ after it.
print(compare('a\0b', 'a\0c'), compare('a\0c', 'a\0b'));
// CHECK-NEXT: -1 1
// An unpaired surrogate compares as U+FFFD, wherever it is, and an
// unpaired high and an unpaired low surrogate compare equal.
print(
  compare('\uD800', '\uFFFD'),
  compare('\uDC00', '\uFFFD'),
  compare('\uD800', '\uDC00'),
  compare('a\uDC00b', 'a\uFFFDb'),
  compare('a\uD800b', 'a\uDC00b'),
);
// CHECK-NEXT: 0 0 0 0 0
// A surrogate pair is one supplementary character, not two U+FFFDs:
// Deseret small long I sorts before its capital, as lowercase does in
// English, and before U+FFFD U+FFFD. Read as two U+FFFDs, both would be 0.
print(compare('\uD801\uDC28', '\uD801\uDC00'), compare('\uD801\uDC28', '\uFFFD\uFFFD'));
// CHECK-NEXT: -1 -1
// Canonically equivalent strings compare equal: U+0130 is I followed by
// U+0307 COMBINING DOT ABOVE, and U+00E9 is e followed by U+0301 COMBINING
// ACUTE ACCENT.
print(compare('\u0130', 'I\u0307'), compare('\u00E9', 'e\u0301'));
// CHECK-NEXT: 0 0
// ß is not "ss" at variant strength, but is at base strength.
print(compare('\u00DF', 'ss'), new Intl.Collator('en', {sensitivity: 'base'}).compare('\u00DF', 'ss'));
// CHECK-NEXT: 1 0
// The ffi ligature is a compatibility variant of "ffi".
print(compare('\uFB03', 'ffi'), new Intl.Collator('en', {sensitivity: 'base'}).compare('\uFB03', 'ffi'));
// CHECK-NEXT: 1 0
// The long string compares by its content.
print(compare(big, big + 'b'), compare(big + 'b', big + 'a'), compare(big, 'b'));
// CHECK-NEXT: -1 1 -1
