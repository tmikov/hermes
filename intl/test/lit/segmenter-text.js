/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US %s | %FileCheck --match-full-lines %s

// Intl.Segmenter on text that is not plain BMP text (plan Review Focus 1)
// and on extreme inputs (Review Focus 4). Indices are UTF-16 code-unit
// indices. For every input but the strings of one million code units, check
// verifies that the segments tile the input, that no word boundary falls
// before a character UAX #29's rule WB4 ignores except where WB3a keeps
// one, and that containing agrees with the iteration at every index. Unless a comment
// says otherwise, every result is the one Node 24 gives.

/// s with every code unit outside printable ASCII as \uXXXX.
function esc(s) {
  var out = '';
  for (var i = 0; i < s.length; i++) {
    var c = s.charCodeAt(i);
    out += c < 0x20 || c > 0x7e ? '\\u' + (c + 0x10000).toString(16).substring(1) : s[i];
  }
  return out;
}

/// The segments of segments, through its iterator's next, as "segment",
/// with * for a word-like segment, separated by "|".
function render(segments) {
  var it = segments[Symbol.iterator]();
  var out = [];
  for (var r = it.next(); !r.done; r = it.next())
    out.push(esc(r.value.segment) + (r.value.isWordLike ? '*' : ''));
  return out.join('|');
}

/// The code point of string that starts at index i, or '' at the end.
function codePointAt(string, i) {
  var c = string.codePointAt(i);
  return c === undefined ? '' : String.fromCodePoint(c);
}

/// The code point of string that ends at index i, or '' at the start.
function codePointBefore(string, i) {
  if (i === 0) return '';
  var low = string.charCodeAt(i - 1);
  var high = i >= 2 ? string.charCodeAt(i - 2) : 0;
  var pair = low >= 0xdc00 && low <= 0xdfff && high >= 0xd800 && high <= 0xdbff;
  return string.slice(pair ? i - 2 : i - 1, i);
}

/// Whether c has Word_Break Extend, Format or ZWJ, by UAX #29's
/// definitions: Grapheme_Extend, General_Category Mc, Emoji_Modifier, ZWJ,
/// or General_Category Cf other than U+200B and the Prepend characters
/// (U+200C is Grapheme_Extend).
function isIgnored(c) {
  return /^(?:[\p{Grapheme_Extend}\p{Mc}\p{Emoji_Modifier}\u200d]|(?![\u200b\u0600-\u0605\u06dd\u070f\u0890\u0891\u08e2\u{110bd}\u{110cd}])\p{Cf})$/u.test(c);
}

/// Whether c is CR, LF or has Word_Break Newline.
function isNewline(c) {
  return /^[\r\n\u000b\u000c\u0085\u2028\u2029]$/.test(c);
}

/// "ok" if the segments of segments over string tile it: each segment
/// starts where the last one ended, is string.slice(index, end), and has
/// string as its input; no word segment (one with isWordLike) starts before
/// a character that WB4 ignores unless it starts the string or follows CR,
/// LF or a Newline character (WB3a); and containing(i) gives the segment
/// that the iteration gives for every index i. Otherwise what is wrong.
function check(segments, string) {
  var bySegment = [];
  var end = 0;
  var it = segments[Symbol.iterator]();
  for (var r = it.next(); !r.done; r = it.next()) {
    var v = r.value;
    if (v.index !== end) return 'gap at ' + end;
    if ('isWordLike' in v && isIgnored(codePointAt(string, v.index)) && v.index > 0 &&
        !isNewline(codePointBefore(string, v.index)))
      return 'word boundary before an ignored character: ' + v.index;
    if (v.segment.length === 0) return 'empty segment at ' + end;
    if (v.segment !== string.slice(v.index, v.index + v.segment.length)) return 'slice at ' + end;
    if (v.input !== string) return 'input at ' + end;
    end += v.segment.length;
    for (var k = v.index; k < end; k++) bySegment.push(v);
  }
  if (end !== string.length) return 'ends at ' + end;
  for (var i = 0; i < string.length; i++) {
    var c = segments.containing(i);
    var e = bySegment[i];
    if (c.index !== e.index || c.segment !== e.segment || c.isWordLike !== e.isWordLike)
      return 'containing(' + i + ')';
  }
  return 'ok';
}

/// Each granularity on string in locale: the rendered segments and the
/// tiling check.
function all(locale, string) {
  var out = [];
  var granularities = ['grapheme', 'word', 'sentence'];
  for (var g = 0; g < granularities.length; g++) {
    var segments = new Intl.Segmenter(locale, {granularity: granularities[g]}).segment(string);
    out.push(granularities[g] + ' ' + check(segments, string) + ' ' + render(segments));
  }
  print(out.join('\n'));
}

/// check of the word and of the sentence segments of string in locale:
/// "ok", or the first problem with its granularity.
function checkWordAndSentence(locale, string) {
  var granularities = ['word', 'sentence'];
  for (var g = 0; g < granularities.length; g++) {
    var result = check(new Intl.Segmenter(locale, {granularity: granularities[g]}).segment(string), string);
    if (result !== 'ok') return granularities[g] + ': ' + result;
  }
  return 'ok';
}

/// The checks of checkWordAndSentence and the rendered word segments of
/// string in locale.
function word(locale, string) {
  return checkWordAndSentence(locale, string) + ' ' + render(new Intl.Segmenter(locale, {granularity: 'word'}).segment(string));
}

/// The checks of checkWordAndSentence and the rendered sentence segments of
/// string in locale.
function sentence(locale, string) {
  return checkWordAndSentence(locale, string) + ' ' + render(new Intl.Segmenter(locale, {granularity: 'sentence'}).segment(string));
}

// Unpaired surrogates: leading, trailing, a reversed pair, each next to
// other text, and alone. Each is a segment of its own in every granularity
// but sentence, and never word-like.
all('en', 'a\ud800b \udc00');
// CHECK: grapheme ok a|\ud800|b| |\udc00
// CHECK-NEXT: word ok a*|\ud800|b*| |\udc00
// CHECK-NEXT: sentence ok a\ud800b \udc00
all('en', '\udc00\ud800');
// CHECK-NEXT: grapheme ok \udc00|\ud800
// CHECK-NEXT: word ok \udc00|\ud800
// CHECK-NEXT: sentence ok \udc00\ud800
all('en', '\udc00x. Y\ud83d');
// CHECK-NEXT: grapheme ok \udc00|x|.| |Y|\ud83d
// CHECK-NEXT: word ok \udc00|x*|.| |Y*|\ud83d
// CHECK-NEXT: sentence ok \udc00x. |Y\ud83d
all('en', '\ud800');
// CHECK-NEXT: grapheme ok \ud800
// CHECK-NEXT: word ok \ud800
// CHECK-NEXT: sentence ok \ud800

// Supplementary characters: an emoji ZWJ sequence with a skin tone, two
// flags, Ext-B Han, and a supplementary character in a Japanese run.
all('en', '\ud83e\uddd1\ud83c\udffd\u200d\ud83e\uddaf \ud83c\uddfa\ud83c\uddf8\ud83c\udde9\ud83c\uddea!');
// CHECK-NEXT: grapheme ok \ud83e\uddd1\ud83c\udffd\u200d\ud83e\uddaf| |\ud83c\uddfa\ud83c\uddf8|\ud83c\udde9\ud83c\uddea|!
// CHECK-NEXT: word ok \ud83e\uddd1\ud83c\udffd\u200d\ud83e\uddaf| |\ud83c\uddfa\ud83c\uddf8|\ud83c\udde9\ud83c\uddea|!
// CHECK-NEXT: sentence ok \ud83e\uddd1\ud83c\udffd\u200d\ud83e\uddaf \ud83c\uddfa\ud83c\uddf8\ud83c\udde9\ud83c\uddea!
all('en', '\ud840\udc00\ud840\udc01\ud840\udc02');
// CHECK-NEXT: grapheme ok \ud840\udc00|\ud840\udc01|\ud840\udc02
// CHECK-NEXT: word ok \ud840\udc00*|\ud840\udc01*|\ud840\udc02*
// CHECK-NEXT: sentence ok \ud840\udc00\ud840\udc01\ud840\udc02
all('ja', '\u65e5\u672c\u8a9e\ud840\udc00\u30c6\u30b9\u30c8');
// CHECK-NEXT: grapheme ok \u65e5|\u672c|\u8a9e|\ud840\udc00|\u30c6|\u30b9|\u30c8
// CHECK-NEXT: word ok \u65e5\u672c\u8a9e*|\ud840\udc00*|\u30c6\u30b9\u30c8*
// CHECK-NEXT: sentence ok \u65e5\u672c\u8a9e\ud840\udc00\u30c6\u30b9\u30c8
// A lone surrogate inside a Han run splits it and is not word-like.
all('zh', '\u5317\u4eac\ud800\u5929\u5b89\u95e8');
// CHECK-NEXT: grapheme ok \u5317|\u4eac|\ud800|\u5929|\u5b89|\u95e8
// CHECK-NEXT: word ok \u5317\u4eac*|\ud800|\u5929\u5b89\u95e8*
// CHECK-NEXT: sentence ok \u5317\u4eac\ud800\u5929\u5b89\u95e8

// ICU4X 2.4 word bugs that native code works around (plan decision 9).
// Consecutive supplementary Han, which ICU4X's UTF-16 path keeps as one
// segment.
print(word('en', '\ud840\udc00\ud840\udc01\ud840\udc02 \ud869\udea5\ud869\udea5'));
// CHECK-NEXT: ok \ud840\udc00*|\ud840\udc01*|\ud840\udc02*| |\ud869\udea5*|\ud869\udea5*
// A word that ends in a code point with Word_Break Extend, Format or ZWJ,
// followed by more text, is word-like: decomposed "cafe\u0301", Hindi,
// Arabic with harakat, and a word ending in U+00AD (Format). So is a word
// that ends in ZWJ at the end of the text, and one that ends in a mark and
// then ZWJ. The last line has marks only inside words, and precomposed
// letters.
print(word('fr', 'cafe\u0301 x'));
// CHECK-NEXT: ok cafe\u0301*| |x*
print(word('hi', '\u0915\u0940 \u090f\u0938'));
// CHECK-NEXT: ok \u0915\u0940*| |\u090f\u0938*
print(word('ar', '\u0643\u064e\u062a\u064e\u0628\u064e \u0648\u0644\u062f'));
// CHECK-NEXT: ok \u0643\u064e\u062a\u064e\u0628\u064e*| |\u0648\u0644\u062f*
print(word('en', 'ab cd\u00ad ef'));
// CHECK-NEXT: ok ab*| |cd\u00ad*| |ef*
print(word('en', 'ab\u200d x'));
// CHECK-NEXT: ok ab\u200d*| |x*
print(word('en', 'ab\u200d'));
// CHECK-NEXT: ok ab\u200d*
print(word('en', 'ab\u0301\u200d x'));
// CHECK-NEXT: ok ab\u0301\u200d*| |x*
print(word('en', 'caf\u00e9 na\u00efve \u0915\u0940\u092e\u0924'));
// CHECK-NEXT: ok caf\u00e9*| |na\u00efve*| |\u0915\u0940\u092e\u0924*

// A Prepend character (U+0600, U+06DD) joins the next character's
// grapheme cluster, but UAX #29's word rules break after it, and WB4 does
// not ignore what follows it.
print(word('ar', '\u0600 x'));
// CHECK-NEXT: ok \u0600*| |x*
print(word('en', 'x \u06dd y'));
// CHECK-NEXT: ok x*| |\u06dd*| |y*
print(sentence('en', 'Hi. \u0600Bye'));
// CHECK-NEXT: ok Hi. |\u0600Bye

// ICU4X tailors el sentences: ";" is a question mark there.
var greek = '\u03a4\u03b9 \u03ba\u03ac\u03bd\u03b5\u03b9\u03c2; \u039a\u03b1\u03bb\u03ac. \u039d\u03b1\u03b9;\u038c\u03c7\u03b9';
print(sentence('el', greek));
// CHECK-NEXT: ok \u03a4\u03b9 \u03ba\u03ac\u03bd\u03b5\u03b9\u03c2; |\u039a\u03b1\u03bb\u03ac. |\u039d\u03b1\u03b9;|\u038c\u03c7\u03b9
print(sentence('en', greek));
// CHECK-NEXT: ok \u03a4\u03b9 \u03ba\u03ac\u03bd\u03b5\u03b9\u03c2; \u039a\u03b1\u03bb\u03ac. |\u039d\u03b1\u03b9;\u038c\u03c7\u03b9

// ICU4X 2.4's Thai dictionary keeps the tone mark U+0E48 of "\u0e0a\u0e48"
// in the second Thai line on its base, so "\u0e07\u0e0a\u0e48" is one piece;
// the line stays as a regression case (ICU4X 2.3 split the mark off).
//
// What remains are implementation-defined results that differ from ICU4C
// and that plan decision 9 accepts, pinned to the plugin's output. Common
// Thai words split as in ICU4C (the first line). The second line has a word
// ICU4X's Thai dictionary lacks, and where ICU4C gives five words from the
// third one on, the plugin gives six pieces. Korean follows UAX #29 next to
// Latin letters and U+00B7, which ICU4C splits ("MB|" and "|\u00b7|").
print(word('th', '\u0e17\u0e38\u0e01\u0e2a\u0e2d\u0e07\u0e2a\u0e31\u0e1b\u0e14\u0e32\u0e2b\u0e4c'));
// CHECK-NEXT: ok \u0e17\u0e38\u0e01*|\u0e2a\u0e2d\u0e07*|\u0e2a\u0e31\u0e1b\u0e14\u0e32\u0e2b\u0e4c*
print(word('th', '\u0e27\u0e31\u0e14\u0e44\u0e17\u0e23\u0e15\u0e35\u0e23\u0e30\u0e06\u0e31\u0e07\u0e40\u0e1a\u0e32\u0e25\u0e07\u0e0a\u0e48\u0e27\u0e07\u0e40\u0e02\u0e49\u0e32\u0e1e\u0e23\u0e23\u0e29\u0e32'));
// CHECK-NEXT: ok \u0e27\u0e31\u0e14*|\u0e44\u0e17\u0e23*|\u0e15\u0e35\u0e23\u0e30*|\u0e06\u0e31*|\u0e07\u0e40*|\u0e1a\u0e32\u0e25*|\u0e07\u0e0a\u0e48*|\u0e27\u0e07*|\u0e40\u0e02\u0e49\u0e32*|\u0e1e\u0e23\u0e23\u0e29\u0e32*
print(word('ko', '\ud55c\uad6d\uc5b4 MB\uac00 \ud14c\uc2a4\ud2b8'));
// CHECK-NEXT: ok \ud55c\uad6d\uc5b4*| |MB\uac00*| |\ud14c\uc2a4\ud2b8*
print(word('ko', '\ucc98\ubd84\uad8c\u00b7\uc218\uc775\uad8c'));
// CHECK-NEXT: ok \ucc98\ubd84\uad8c\u00b7\uc218\uc775\uad8c*

// Another ICU4X 2.4 word bug: it puts word boundaries before characters
// that WB4 ignores (Word_Break Extend, Format or ZWJ), as before a soft
// hyphen after Thai. Native code repairs such a boundary by dropping it
// unless it follows the start of the text, CR, LF or a Newline character,
// as in Node: the soft hyphen stays on the word. U+0E33 is
// Grapheme_Cluster_Break SpacingMark but Word_Break Other, so nothing is
// merged before it: " \u0e33".containing(1) is "\u0e33" at 1. Node gives
// "\u0e48|\u0e33*" for "\u0e48\u0e33"; ICU4X 2.4's Thai dictionary, on
// Unicode 18's data, keeps a leading tone mark and U+0E33 as one word, and
// ECMA-402 leaves segmentation to the implementation. After CR and U+2028
// (Newline) the boundary before the marks stays.
print(word('th', '\u0e01\u00ad x'));
// CHECK-NEXT: ok \u0e01\u00ad*| |x*
print(word('th', ' \u0e33'), esc(JSON.stringify(new Intl.Segmenter('th', {granularity: 'word'}).segment(' \u0e33').containing(1))));
// CHECK-NEXT: ok  |\u0e33* {"segment":"\u0e33","index":1,"input":" \u0e33","isWordLike":true}
print(word('th', '\u0e48\u0e33'));
// CHECK-NEXT: ok \u0e48\u0e33*
print(word('en', '\r\u0301\u0301x'));
// CHECK-NEXT: ok \u000d|\u0301\u0301|x*
print(word('en', '\u2028\u0301\u0301 x'));
// CHECK-NEXT: ok \u2028|\u0301\u0301| |x*
// Word types ICU4X 2.4 gets wrong, as in Node: a Hebrew word that ends in
// an apostrophe (WB7a) is word-like; a segment made only of ignored
// characters is not, also merged with a space, unless one of them is
// Ideographic (U+16FE4 KHITAN SMALL SCRIPT FILLER).
print(word('he', '\u05de\u05e1\' x'));
// CHECK-NEXT: ok \u05de\u05e1'*| |x*
print(word('th', ' \u0e48'));
// CHECK-NEXT: ok  \u0e48
print(word('en', '\ud81b\udfe4'));
// CHECK-NEXT: ok \ud81b\udfe4*

// A string of one million code units: iterated to the end, and containing
// at the last index, which for grapheme is the trailing half of a pair.
var big = 'word '.repeat(200000);
var words = 0;
var segmentsSeen = 0;
var last;
var bigIt = new Intl.Segmenter('en', {granularity: 'word'}).segment(big)[Symbol.iterator]();
for (var r = bigIt.next(); !r.done; r = bigIt.next()) {
  segmentsSeen++;
  if (r.value.isWordLike) words++;
  last = r.value;
}
print(big.length, segmentsSeen, words, last.index, JSON.stringify(last.segment));
// CHECK-NEXT: 1000000 400000 200000 999999 " "
print(JSON.stringify(new Intl.Segmenter('en', {granularity: 'word'}).segment(big).containing(big.length - 1).index));
// CHECK-NEXT: 999999
var bigPair = 'a'.repeat(999998) + '\ud83d\ude00';
var g = new Intl.Segmenter('en').segment(bigPair);
print(bigPair.length, g.containing(999999).index, esc(g.containing(999999).segment), g.containing(999998).index, g.containing(500000).segment);
// CHECK-NEXT: 1000000 999998 \ud83d\ude00 999998 a
var s = new Intl.Segmenter('en', {granularity: 'sentence'}).segment(big);
print(s.containing(big.length - 1).index, s.containing(big.length - 1).segment.length, s.containing(1e6), s.containing(NaN).index);
// CHECK-NEXT: 0 1000000 undefined 0

// Long unbroken Han and Thai runs (plan Review Focus 5), which ICU4X 2.3
// segmented in time quadratic in their words: 30,000 copies of
// \u6211|\u7231|\u5317\u4eac|\u5929\u5b89\u95e8 (210,000 code units),
// and \u0e01 and 199,999 tone marks U+0E48 (200,000 code units), one
// segment. Segmenting them, which the first containing does, takes well
// under a second (about 40 ms in the Debug ASan build).
var han = '\u6211\u7231\u5317\u4eac\u5929\u5b89\u95e8';

/// The number of segments of segments and the milliseconds that iterating
/// them takes.
function iterate(segments) {
  var start = Date.now();
  var count = 0;
  var it = segments[Symbol.iterator]();
  for (var r = it.next(); !r.done; r = it.next()) count++;
  return {count: count, ms: Date.now() - start};
}

/// Segment string in locale, then iterate it: the length, the segment
/// containing the last index, the segment count, and whether segmenting
/// was fast. Returns the segments.
function longRun(locale, string) {
  var start = Date.now();
  var segments = new Intl.Segmenter(locale, {granularity: 'word'}).segment(string);
  var c = segments.containing(string.length - 1);
  var ms = Date.now() - start;
  var r = iterate(segments);
  print(string.length, c.index, c.segment.length, c.isWordLike, r.count, ms < 1000 ? 'fast' : 'slow: ' + ms + ' ms');
  return segments;
}
var whole = longRun('zh', han.repeat(30000));
// CHECK-NEXT: 210000 209997 3 true 120000 fast
longRun('th', '\u0e01' + '\u0e48'.repeat(199999));
// CHECK-NEXT: 200000 0 200000 true 1 fast

// A not-quadratic guard on the iteration: its time per segment over the
// whole Han run is under twice that over a run of an eighth of it (15,000
// segments, iterated four times). It is about 1 when the iteration is
// linear (about 12 microseconds per segment in the Debug ASan build), and
// about 4 with a linear rescan of the boundaries at each step. It is a
// ratio because a parallel test load makes the iteration about three
// times slower, so an absolute bound that never fails on a loaded machine
// would also pass a quadratic iteration. The two are measured in three
// interleaved rounds, alternating which goes first, and each takes its
// fastest round, so that neither a pause nor a change of load between
// the measurements decides the ratio.
var eighth = new Intl.Segmenter('zh', {granularity: 'word'}).segment(han.repeat(3750));
eighth.containing(0);
/// The milliseconds that iterating eighth four times takes.
function eighthPass() {
  var ms = 0;
  for (var k = 0; k < 4; k++) ms += iterate(eighth).ms;
  return ms;
}
var wholeMs = Infinity;
var eighthMs = Infinity;
for (var round = 0; round < 3; round++) {
  if (round % 2 === 0) {
    wholeMs = Math.min(wholeMs, iterate(whole).ms);
    eighthMs = Math.min(eighthMs, eighthPass());
  } else {
    eighthMs = Math.min(eighthMs, eighthPass());
    wholeMs = Math.min(wholeMs, iterate(whole).ms);
  }
}
var ratio = (wholeMs / 120000) / (eighthMs / 60000);
print(ratio < 2 ? 'linear' : 'not linear: ' + ratio);
// CHECK-NEXT: linear
