/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US %s | %FileCheck --match-full-lines %s

// Segmenters, Segments objects and iterators freed by the GC. Each
// Intl.Segmenter has a native segmenter of its own, and each Segments object
// that reaches native code has native boundaries of its own (plan
// decisions 9 and 10), freed with it. 2,000 Segmenters, each with a
// Segments object, alternate the three granularities over 10 locales; some
// are iterated halfway, some are asked containing first, and some never
// reach native code. gc() runs every 100, and every tenth Segmenter, with
// its Segments object and its iterator, is kept. The survivors give the
// same results afterwards. An iterator kept after its Segments object and
// Segmenter become unreachable still works. The script ends with live
// objects, so the runtime frees what is left when it is destroyed. Under
// ASan, LeakSanitizer and use-after-free checks cover both.

var locales = ['en', 'de', 'fr', 'ja', 'zh', 'th', 'el', 'hi', 'ar', 'ko'];
var granularities = ['grapheme', 'word', 'sentence'];
var text = 'Hello, world! \u65e5\u672c\u8a9e\u3067\u3059. \u0e17\u0e38\u0e01\u0e2a\u0e2d\u0e07. Ok?';

/// The locale of Segmenter i: it changes every 10, so the survivors (every
/// tenth) cover every locale.
function localeOf(i) {
  return locales[Math.floor(i / 10) % locales.length];
}
/// The granularity of Segmenter i: it changes with every Segmenter.
function granularityOf(i) {
  return granularities[i % granularities.length];
}
/// The key of Segmenter i.
function keyOf(i) {
  return localeOf(i) + ' ' + granularityOf(i);
}

/// The segments of the iterator it from where it stands, as
/// segment@index, with * for a word-like segment.
function rest(it) {
  var out = [];
  for (var r = it.next(); !r.done; r = it.next())
    out.push(r.value.segment + '@' + r.value.index + (r.value.isWordLike ? '*' : ''));
  return out.join('|');
}

/// s with every code unit outside printable ASCII as \uXXXX.
function esc(s) {
  var out = '';
  for (var i = 0; i < s.length; i++) {
    var c = s.charCodeAt(i);
    out += c < 0x20 || c > 0x7e ? '\\u' + (c + 0x10000).toString(16).substring(1) : s[i];
  }
  return out;
}

/// The results of the first Segmenter of each kind to be checked, keyed by
/// locale and granularity. A later one that disagrees is wrong.
var expectedOf = Object.create(null);
/// Whether results r of Segmenter i agree with the first of its kind.
function check(r, i) {
  var key = keyOf(i);
  if (!(key in expectedOf)) expectedOf[key] = r;
  return expectedOf[key] === r;
}

var kept = [];
var wrong = 0;
for (var i = 0; i < 2000; i++) {
  var segmenter = new Intl.Segmenter(localeOf(i), {granularity: granularityOf(i)});
  var segments = segmenter.segment(text);
  var it = segments[Symbol.iterator]();
  // Two in five are iterated halfway; one in five is asked containing
  // first; the rest never reach native code before they are kept or die.
  if (i % 5 < 2) {
    for (var n = 0; n < 4; n++) it.next();
  } else if (i % 5 === 2) {
    if (segments.containing(14).index !== 14) wrong++;
  }
  if (i % 10 === 0) kept.push({i: i, segmenter: segmenter, segments: segments, it: it});
  if (i % 100 === 99) gc();
}
gc();

for (var k = 0; k < kept.length; k++) {
  var e = kept[k];
  // The kept iterator went 4 segments in (i % 10 === 0, so i % 5 === 0).
  var fromIt = rest(e.it);
  var whole = rest(e.segments[Symbol.iterator]());
  var again = rest(e.segmenter.segment(text)[Symbol.iterator]());
  if (whole !== again || whole.indexOf(fromIt) === -1 || fromIt === whole) wrong++;
  if (!check(whole, e.i)) wrong++;
  if (e.segments.containing(text.length - 1).segment.slice(-1) !== '?') wrong++;
  var ro = e.segmenter.resolvedOptions();
  if (ro.locale !== localeOf(e.i) || ro.granularity !== granularityOf(e.i)) wrong++;
}
print(kept.length, 'kept,', wrong, 'wrong');
// CHECK: 200 kept, 0 wrong
// The survivors cover every locale and granularity: 10 x 3.
var combinations = new Set();
for (var q = 0; q < kept.length; q++) combinations.add(keyOf(kept[q].i));
print(combinations.size, 'combinations');
// CHECK-NEXT: 30 combinations

// An iterator outlives its Segments object and Segmenter: it keeps the
// Segments object, whose native boundaries do not depend on the
// Segmenter's native state.
function orphan(granularity, s) {
  var it = new Intl.Segmenter('en', {granularity: granularity}).segment(s)[Symbol.iterator]();
  it.next();
  return it;
}
var orphans = [orphan('word', 'one two three'), orphan('grapheme', 'abc'), orphan('sentence', 'A. B. C.')];
function lateOrphan() {
  return new Intl.Segmenter('en', {granularity: 'word'}).segment('never touched')[Symbol.iterator]();
}
orphans.push(lateOrphan());
for (var j = 0; j < 300; j++) new Intl.Segmenter('en', {granularity: 'word'}).segment('x y').containing(0);
gc();
for (var o = 0; o < orphans.length; o++) print(rest(orphans[o]));
// CHECK-NEXT:  @3|two@4*| @7|three@8*
// CHECK-NEXT: b@1|c@2
// CHECK-NEXT: B. @3|C.@6
// CHECK-NEXT: never@0*| @5|touched@6*

// A few of the results, so a wrong boundary everywhere is caught too.
print(esc(expectedOf['en word']));
// CHECK-NEXT: Hello@0*|,@5| @6|world@7*|!@12| @13|\u65e5\u672c\u8a9e@14*|\u3067\u3059@17*|.@19| @20|\u0e17\u0e38\u0e01@21*|\u0e2a\u0e2d\u0e07@24*|.@27| @28|Ok@29*|?@31
print(esc(expectedOf['en sentence']));
// CHECK-NEXT: Hello, world! @0|\u65e5\u672c\u8a9e\u3067\u3059. @14|\u0e17\u0e38\u0e01\u0e2a\u0e2d\u0e07. @21|Ok?@29
