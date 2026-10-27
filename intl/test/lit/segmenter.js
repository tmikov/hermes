/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US %s | %FileCheck --match-full-lines %s

// Intl.Segmenter: the constructor, resolvedOptions, supportedLocalesOf,
// segment, the Segments objects' containing and @@iterator, and the
// Segment Iterators' next, with the values Node 24 gives.

/// The class of the exception f throws, or 'none'.
function thrown(f) {
  try {
    f();
    return 'none';
  } catch (e) {
    return e.constructor.name;
  }
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

/// The segments of segments, through its iterator's next, as
/// "segment"@index, with * for a word-like segment and - for one that is
/// not; a segment data object without an own isWordLike has neither.
function render(segments) {
  var it = segments[Symbol.iterator]();
  var out = [];
  for (;;) {
    var r = it.next();
    if (r.done) break;
    var v = r.value;
    var mark = Object.hasOwn(v, 'isWordLike') ? (v.isWordLike ? '*' : '-') : '';
    out.push('"' + esc(v.segment) + '"@' + v.index + mark);
  }
  return out.join(' ');
}

// containing, by ToIntegerOrInfinity of the index.
var word = new Intl.Segmenter('en', {granularity: 'word'});
var hw = word.segment('Hello world');
print(JSON.stringify(hw.containing(7)));
// CHECK: {"segment":"world","index":6,"input":"Hello world","isWordLike":true}
print(hw.containing(-1), hw.containing(11), hw.containing(Infinity), hw.containing(-Infinity));
// CHECK-NEXT: undefined undefined undefined undefined
print(JSON.stringify(hw.containing('5')));
// CHECK-NEXT: {"segment":" ","index":5,"input":"Hello world","isWordLike":false}
print(JSON.stringify(hw.containing(NaN)));
// CHECK-NEXT: {"segment":"Hello","index":0,"input":"Hello world","isWordLike":true}
print(JSON.stringify(hw.containing(-0.5)));
// CHECK-NEXT: {"segment":"Hello","index":0,"input":"Hello world","isWordLike":true}
print(hw.containing().segment, hw.containing(10.9).segment, hw.containing({valueOf: () => 4}).segment);
// CHECK-NEXT: Hello world Hello
print(thrown(() => hw.containing(1n)), thrown(() => hw.containing(Symbol())));
// CHECK-NEXT: TypeError TypeError
print(word.segment('').containing(0), word.segment('').containing(-0));
// CHECK-NEXT: undefined undefined
print(Object.getPrototypeOf(hw.containing(0)) === Object.prototype);
// CHECK-NEXT: true

// Iteration, calling next directly. Grapheme clusters, in UTF-16 indices.
var grapheme = new Intl.Segmenter('en');
print(render(grapheme.segment('e\u0301\ud83d\udc69\u200d\ud83d\udc69\u200d\ud83d\udc67\u200d\ud83d\udc66\ud83c\uddfa\ud83c\uddf8\r\n\ud800x')));
// CHECK-NEXT: "e\u0301"@0 "\ud83d\udc69\u200d\ud83d\udc69\u200d\ud83d\udc67\u200d\ud83d\udc66"@2 "\ud83c\uddfa\ud83c\uddf8"@13 "\u000d\u000a"@17 "\ud800"@19 "x"@20
print(render(word.segment("Hello, world! It's 3.14.")));
// CHECK-NEXT: "Hello"@0* ","@5- " "@6- "world"@7* "!"@12- " "@13- "It's"@14* " "@18- "3.14"@19* "."@23-
var sentence = new Intl.Segmenter('en', {granularity: 'sentence'});
print(render(sentence.segment('Mr. Smith left. Did he? Yes!\nOK')));
// CHECK-NEXT: "Mr. "@0 "Smith left. "@4 "Did he? "@16 "Yes!\u000a"@24 "OK"@29

// The segment data objects' own properties, in order.
print(JSON.stringify(Object.getOwnPropertyNames(hw.containing(0))));
// CHECK-NEXT: ["segment","index","input","isWordLike"]
print(JSON.stringify(Object.getOwnPropertyNames(grapheme.segment('ab').containing(1))));
// CHECK-NEXT: ["segment","index","input"]
print(JSON.stringify(Object.getOwnPropertyNames(sentence.segment('A. B.').containing(3))));
// CHECK-NEXT: ["segment","index","input"]
var d = Object.getOwnPropertyDescriptor(hw.containing(0), 'isWordLike');
print(d.writable, d.enumerable, d.configurable);
// CHECK-NEXT: true true true

// The iterator results: { value, done }, and done stays done.
var it = grapheme.segment('ab')[Symbol.iterator]();
print(JSON.stringify(it.next()));
// CHECK-NEXT: {"value":{"segment":"a","index":0,"input":"ab"},"done":false}
it.next();
var done = it.next();
print(JSON.stringify(Object.getOwnPropertyNames(done)), done.value, done.done, JSON.stringify(it.next()));
// CHECK-NEXT: ["value","done"] undefined true {"done":true}
print(JSON.stringify(word.segment('')[Symbol.iterator]().next()));
// CHECK-NEXT: {"done":true}

// Two iterators of one Segments object are independent, and containing
// between calls of next changes nothing.
var hs = word.segment('Hello, world');
var it1 = hs[Symbol.iterator]();
var it2 = hs[Symbol.iterator]();
print(it1 !== it2, it1.next().value.segment, it1.next().value.segment, it2.next().value.segment);
// CHECK-NEXT: true Hello , Hello
print(hs.containing(8).segment, it1.next().value.index, hs.containing(0).segment, it1.next().value.segment, it2.next().value.segment);
// CHECK-NEXT: world 6 Hello world ,
// for-of and spread go through @@iterator.
var spread = [...grapheme.segment('a\ud83d\ude00b')].map(s => s.segment);
print(spread.length, esc(spread.join('|')));
// CHECK-NEXT: 3 a|\ud83d\ude00|b
var count = 0;
for (var s of word.segment('one two')) count++;
print(count);
// CHECK-NEXT: 3

// The shapes Node shows. A Segments object has no @@toStringTag.
var segs = grapheme.segment('a');
var segIt = segs[Symbol.iterator]();
var SegmentsPrototype = Object.getPrototypeOf(segs);
var SegmentIteratorPrototype = Object.getPrototypeOf(segIt);
var IteratorPrototype = Object.getPrototypeOf(Object.getPrototypeOf([][Symbol.iterator]()));
print(Object.prototype.toString.call(segs), Object.prototype.toString.call(segIt));
// CHECK-NEXT: [object Object] [object Segmenter String Iterator]
print(Object.getPrototypeOf(SegmentsPrototype) === Object.prototype, Object.getPrototypeOf(SegmentIteratorPrototype) === IteratorPrototype);
// CHECK-NEXT: true true
print(Reflect.ownKeys(SegmentsPrototype).map(String).join(','));
// CHECK-NEXT: containing,Symbol(Symbol.iterator)
print(Reflect.ownKeys(SegmentIteratorPrototype).map(String).join(','));
// CHECK-NEXT: next,Symbol(Symbol.toStringTag)
print(JSON.stringify(Object.getOwnPropertyDescriptor(SegmentIteratorPrototype, Symbol.toStringTag)));
// CHECK-NEXT: {"value":"Segmenter String Iterator","writable":false,"enumerable":false,"configurable":true}
print(JSON.stringify(Object.getOwnPropertyDescriptor(SegmentIteratorPrototype, 'next')));
// CHECK-NEXT: {"writable":true,"enumerable":false,"configurable":true}
print(JSON.stringify(Object.getOwnPropertyDescriptor(SegmentsPrototype, Symbol.iterator)));
// CHECK-NEXT: {"writable":true,"enumerable":false,"configurable":true}
print(SegmentIteratorPrototype.next.length, SegmentsPrototype.containing.length, SegmentsPrototype[Symbol.iterator].length);
// CHECK-NEXT: 0 1 0
print(SegmentsPrototype[Symbol.iterator].name, SegmentsPrototype.containing.name, SegmentIteratorPrototype.next.name);
// CHECK-NEXT: [Symbol.iterator] containing next
// The iterator's @@iterator, from %Iterator.prototype%, returns it.
print(segIt[Symbol.iterator]() === segIt, Object.hasOwn(SegmentIteratorPrototype, Symbol.iterator));
// CHECK-NEXT: true false
// Every Segments object and iterator shares the prototypes and has no own
// properties: their slots are private.
print(Object.getPrototypeOf(word.segment('x')) === SegmentsPrototype, Object.getPrototypeOf(hs[Symbol.iterator]()) === SegmentIteratorPrototype);
// CHECK-NEXT: true true
print(Reflect.ownKeys(segs).length, Reflect.ownKeys(segIt).length, Reflect.ownKeys(word).length);
// CHECK-NEXT: 0 0 0
print(segs !== grapheme.segment('a'), segIt !== segs[Symbol.iterator]());
// CHECK-NEXT: true true

// segment's argument goes through ToString.
print(JSON.stringify(word.segment().containing(0)));
// CHECK-NEXT: {"segment":"undefined","index":0,"input":"undefined","isWordLike":true}
print(word.segment(12.5).containing(0).input, word.segment({toString: () => 'ab cd'}).containing(3).segment);
// CHECK-NEXT: 12.5 cd
print(thrown(() => word.segment(Symbol())));
// CHECK-NEXT: TypeError

// resolvedOptions: no relevant extension keys, so every keyword goes.
print(JSON.stringify(new Intl.Segmenter('en-u-co-phonebk-x-foo').resolvedOptions()));
// CHECK-NEXT: {"locale":"en","granularity":"grapheme"}
print(JSON.stringify(new Intl.Segmenter(['zh-Hant-TW', 'de'], {granularity: 'sentence'}).resolvedOptions()));
// CHECK-NEXT: {"locale":"zh-Hant-TW","granularity":"sentence"}
print(new Intl.Segmenter('en-us').resolvedOptions().locale, new Intl.Segmenter().resolvedOptions().locale);
// CHECK-NEXT: en-US en-US
print(JSON.stringify(word.resolvedOptions()), word.resolvedOptions() !== word.resolvedOptions());
// CHECK-NEXT: {"locale":"en","granularity":"word"} true
var ro = Object.getOwnPropertyDescriptor(word.resolvedOptions(), 'granularity');
print(ro.writable, ro.enumerable, ro.configurable);
// CHECK-NEXT: true true true
print(thrown(() => new Intl.Segmenter('en', {granularity: 'line'})), thrown(() => new Intl.Segmenter('en', 'word')), thrown(() => new Intl.Segmenter('en', null)));
// CHECK-NEXT: RangeError TypeError TypeError
print(thrown(() => Intl.Segmenter()), thrown(() => Intl.Segmenter('en')), thrown(() => new Intl.Segmenter('x-')));
// CHECK-NEXT: TypeError TypeError RangeError
// The options read, in order; lineBreakStyle is never read.
var reads = [];
new Intl.Segmenter('en', new Proxy({lineBreakStyle: 'strict'}, {
  get(target, key) {
    reads.push(String(key));
    return target[key];
  },
}));
print(reads.join(','));
// CHECK-NEXT: localeMatcher,granularity
var r = new Intl.Segmenter('en', {granularity: 'word', lineBreakStyle: 'loose'}).resolvedOptions();
print(Object.keys(r).join(','));
// CHECK-NEXT: locale,granularity

// supportedLocalesOf.
print(JSON.stringify(Intl.Segmenter.supportedLocalesOf(['th', 'xx', 'zh-Hant-TW', 'en-US-u-ca-buddhist'])));
// CHECK-NEXT: ["th","zh-Hant-TW","en-US-u-ca-buddhist"]
print(JSON.stringify(Intl.Segmenter.supportedLocalesOf()), thrown(() => Intl.Segmenter.supportedLocalesOf('en', null)));
// CHECK-NEXT: [] TypeError
