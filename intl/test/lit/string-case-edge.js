/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US %s | %FileCheck --match-full-lines %s

// Unusual text through toLocaleUpperCase, toLocaleLowerCase and
// localeCompare: unpaired surrogates, supplementary characters, NUL,
// mappings that change the length, and a 1,000,000-character string.
// Unpaired surrogates stay where they were, supplementary characters are
// mapped, and nothing crashes.

/// The code units of s in hex, so that unpaired surrogates can be printed.
function hex(s) {
  var out = [];
  for (var i = 0; i < s.length; i++) out.push(s.charCodeAt(i).toString(16));
  return out.join(' ');
}

// Unpaired surrogates, alone, between letters, and at either end, are copied
// through unchanged.
print(hex('\uD800'.toLocaleUpperCase()), '|', hex('\uDC00'.toLocaleLowerCase()));
// CHECK: d800 | dc00
print(hex('a\uD800b'.toLocaleUpperCase()), '|', hex('A\uDC00B'.toLocaleLowerCase()));
// CHECK-NEXT: 41 d800 42 | 61 dc00 62
print(hex('\uDC00i\uD800'.toLocaleUpperCase('tr')));
// CHECK-NEXT: dc00 130 d800
// A low surrogate before a high one is two unpaired surrogates, not a pair.
print(hex('a\uDC00\uD800b'.toLocaleUpperCase()));
// CHECK-NEXT: 41 dc00 d800 42
// A surrogate next to a supplementary character: the pair is mapped, the
// unpaired one is kept.
print(hex('\uD800𐐨\uDC00'.toLocaleUpperCase()));
// CHECK-NEXT: d800 d801 dc00 dc00

// Supplementary characters are mapped: Deseret long I.
print(hex('𐐨'.toLocaleUpperCase()), '|', hex('𐐀'.toLocaleLowerCase()));
// CHECK-NEXT: d801 dc00 | d801 dc28

// NUL is a character: it is kept, and the letters after it are mapped.
print(hex('a\0b'.toLocaleUpperCase()), '|', hex('A\0B'.toLocaleLowerCase('tr')));
// CHECK-NEXT: 41 0 42 | 61 0 62

// Mappings that change the length.
print('ß'.toLocaleUpperCase(), 'ﬃ'.toLocaleUpperCase(), 'ŉ'.toLocaleUpperCase());
// CHECK-NEXT: SS FFI ʼN
print(hex('İ'.toLocaleLowerCase()), '|', hex('İ'.toLocaleLowerCase('tr')));
// CHECK-NEXT: 69 307 | 69
// Turkish and Azerbaijani drop a combining dot above after I.
print(hex('İ'.toLocaleLowerCase('az')), '|', hex('İ'.toLocaleLowerCase('en')));
// CHECK-NEXT: 69 | 69 307
// Final sigma, which depends on the context.
print('ΑΣ ΑΣΑ'.toLocaleLowerCase());
// CHECK-NEXT: ας ασα

// The empty string.
print(JSON.stringify(''.toLocaleUpperCase('tr')), JSON.stringify(''.toLocaleLowerCase()));
// CHECK-NEXT: "" ""

// A 1,000,000-character string, with an unpaired surrogate in the middle and
// one at the end.
var big = 'i'.repeat(500000) + '\uD800' + 'i'.repeat(499998) + '\uDC00';
var upper = big.toLocaleUpperCase('tr');
print(
  upper.length,
  upper === 'İ'.repeat(500000) + '\uD800' + 'İ'.repeat(499998) + '\uDC00',
);
// CHECK-NEXT: 1000000 true
var lower = upper.toLocaleLowerCase('tr');
print(lower === big);
// CHECK-NEXT: true
// Without a tailoring, the dotted capital I lowercases to two code units.
print(upper.toLocaleLowerCase().length);
// CHECK-NEXT: 1999998

// localeCompare on the same texts: reflexive, antisymmetric, NUL counts.
var texts = ['\uD800', 'a\uDC00b', '𐐨', 'a\0b', 'a\0c', 'ß', 'ss', big, ''];
var bad = 'ok';
for (var i = 0; i < texts.length; i++) {
  if (texts[i].localeCompare(texts[i]) !== 0) bad = 'not reflexive: ' + i;
  for (var j = 0; j < texts.length; j++) {
    if (texts[i].localeCompare(texts[j]) !== -texts[j].localeCompare(texts[i]))
      bad = 'not antisymmetric: ' + i + ' ' + j;
  }
}
print(bad);
// CHECK-NEXT: ok
print('a\0b'.localeCompare('a\0c'), 'a\0c'.localeCompare('a\0b', 'de'));
// CHECK-NEXT: -1 1
