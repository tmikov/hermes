/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US %s | %FileCheck --match-full-lines %s

// Intl.ListFormat: the constructor, its options, format and formatToParts
// over arrays and other iterables, the conditional patterns of es and he,
// literals after the last element, text that is not plain BMP text (plan
// Review Focus 1), resolvedOptions and supportedLocalesOf. The expected
// strings are Node's (ICU 78.2) except where the plugin keeps ICU4X's data
// (plan decision 2): he's " ו‑" has U+2011 where Node has U+002D.

var J = JSON.stringify;
/// format and formatToParts of list with a new ListFormat(locales, options).
function both(locales, options, list) {
  var lf = new Intl.ListFormat(locales, options);
  print(J(lf.format(list)));
  print(J(lf.formatToParts(list)));
}
/// The class of the exception f throws, or 'none'.
function thrown(f) {
  try {
    f();
    return 'none';
  } catch (e) {
    return e.constructor.name;
  }
}

both(undefined, undefined, ['a', 'b', 'c']);
// CHECK: "a, b, and c"
// CHECK-NEXT: [{"type":"element","value":"a"},{"type":"literal","value":", "},{"type":"element","value":"b"},{"type":"literal","value":", and "},{"type":"element","value":"c"}]
both('en-GB', undefined, ['a', 'b', 'c']);
// CHECK-NEXT: "a, b and c"
// CHECK-NEXT: [{"type":"element","value":"a"},{"type":"literal","value":", "},{"type":"element","value":"b"},{"type":"literal","value":" and "},{"type":"element","value":"c"}]
both('en', {type: 'disjunction'}, ['a', 'b', 'c']);
// CHECK-NEXT: "a, b, or c"
// CHECK-NEXT: [{"type":"element","value":"a"},{"type":"literal","value":", "},{"type":"element","value":"b"},{"type":"literal","value":", or "},{"type":"element","value":"c"}]
var en = new Intl.ListFormat('en');
var enShort = new Intl.ListFormat('en', {style: 'short'});
var enNarrow = new Intl.ListFormat('en', {style: 'narrow'});
print(J(enShort.format(['a', 'b', 'c'])), J(enNarrow.format(['a', 'b', 'c'])));
// CHECK-NEXT: "a, b, & c" "a, b, c"
both('en', {type: 'unit', style: 'narrow'}, ['5 lb', '12 oz']);
// CHECK-NEXT: "5 lb 12 oz"
// CHECK-NEXT: [{"type":"element","value":"5 lb"},{"type":"literal","value":" "},{"type":"element","value":"12 oz"}]
// Inner elements repeat the middle pattern.
print(J(en.format(['a', 'b', 'c', 'd', 'e'])));
// CHECK-NEXT: "a, b, c, d, and e"

// The conditional patterns, which test the last element.
var es = new Intl.ListFormat('es');
both('es', undefined, ['fresa', 'Ibiza']);
// CHECK-NEXT: "fresa e Ibiza"
// CHECK-NEXT: [{"type":"element","value":"fresa"},{"type":"literal","value":" e "},{"type":"element","value":"Ibiza"}]
print(J(es.format(['agua', 'hielo'])), J(es.format(['uno', 'dos', 'hijo'])));
// CHECK-NEXT: "agua y hielo" "uno, dos e hijo"
print(J(new Intl.ListFormat('es', {type: 'disjunction'}).format(['siete', 'ocho'])));
// CHECK-NEXT: "siete u ocho"
var he = new Intl.ListFormat('he');
both('he', undefined, ['תפוח', 'Ibiza']);
// CHECK-NEXT: "תפוח ו‑Ibiza"
// CHECK-NEXT: [{"type":"element","value":"תפוח"},{"type":"literal","value":" ו‑"},{"type":"element","value":"Ibiza"}]
print(J(he.format(['תפוח', 'Ibiza']).charCodeAt(6).toString(16)));
// CHECK-NEXT: "2011"
both('he', undefined, ['תפוח', 'בננה']);
// CHECK-NEXT: "תפוח ובננה"
// CHECK-NEXT: [{"type":"element","value":"תפוח"},{"type":"literal","value":" ו"},{"type":"element","value":"בננה"}]

// Literals before the first element are never empty parts; one after the
// last element is.
both('ti', undefined, ['a', 'b']);
// CHECK-NEXT: "aን bን"
// CHECK-NEXT: [{"type":"element","value":"a"},{"type":"literal","value":"ን "},{"type":"element","value":"b"},{"type":"literal","value":"ን"}]
both('mn', {type: 'disjunction'}, ['a', 'b', 'c']);
// CHECK-NEXT: "a, b, c зэргийн аль нэг"
// CHECK-NEXT: [{"type":"element","value":"a"},{"type":"literal","value":", "},{"type":"element","value":"b"},{"type":"literal","value":", "},{"type":"element","value":"c"},{"type":"literal","value":" зэргийн аль нэг"}]
print(J(new Intl.ListFormat('ja').format(['a', 'b', 'c'])));
// CHECK-NEXT: "a、b、c"
print(J(new Intl.ListFormat('zh', {type: 'disjunction'}).format(['a', 'b', 'c'])));
// CHECK-NEXT: "a、b或c"
print(J(new Intl.ListFormat('ar').format(['a', 'b', 'c'])));
// CHECK-NEXT: "a وb وc"

// Plan Review Focus 1: elements come back unchanged, unpaired surrogates
// included. An unpaired surrogate reads as U+FFFD, which is not Hebrew, so
// he takes " ו‑".
both('en', undefined, ['\ud800', 'x\udfff']);
// CHECK-NEXT: "\ud800 and x\udfff"
// CHECK-NEXT: [{"type":"element","value":"\ud800"},{"type":"literal","value":" and "},{"type":"element","value":"x\udfff"}]
print(J(he.formatToParts(['x', '\ud800'])));
// CHECK-NEXT: [{"type":"element","value":"x"},{"type":"literal","value":" ו‑"},{"type":"element","value":"\ud800"}]
print(J(he.format(['x', '\udfff\ud800', '𐀀'])));
// CHECK-NEXT: "x, \udfff\ud800 ו‑𐀀"
var odd = ['👨‍👩‍👧', '🇯🇵', '𠀀𠀁𠀂', '日本語𠀀テスト'];
print(J(en.format(odd)));
// CHECK-NEXT: "👨‍👩‍👧, 🇯🇵, 𠀀𠀁𠀂, and 日本語𠀀テスト"
var oddParts = en.formatToParts(odd);
print(oddParts.length, oddParts[0].value === odd[0], oddParts[2].value === odd[1], oddParts[4].value === odd[2], oddParts[6].value === odd[3]);
// CHECK-NEXT: 7 true true true true

// Short lists, and the iterables.
print(J(en.format([])), J(en.formatToParts([])));
// CHECK-NEXT: "" []
print(J(en.format(['only'])), J(en.formatToParts(['only'])));
// CHECK-NEXT: "only" [{"type":"element","value":"only"}]
// An empty element is still an element part (DeconstructPattern appends
// every placeable); only empty literals are dropped (en has nothing before
// the first element or after the last). Node drops empty elements and
// merges the literals around them, which the draft does not allow.
print(J(en.format(['', 'b'])), J(en.formatToParts(['', 'b'])));
// CHECK-NEXT: " and b" [{"type":"element","value":""},{"type":"literal","value":" and "},{"type":"element","value":"b"}]
print(J(en.format(['a', ''])), J(en.formatToParts(['a', ''])));
// CHECK-NEXT: "a and " [{"type":"element","value":"a"},{"type":"literal","value":" and "},{"type":"element","value":""}]
print(J(en.formatToParts(['a', '', 'c'])), J(en.formatToParts([''])));
// CHECK-NEXT: [{"type":"element","value":"a"},{"type":"literal","value":", "},{"type":"element","value":""},{"type":"literal","value":", and "},{"type":"element","value":"c"}] [{"type":"element","value":""}]
print(J(en.format()), J(en.formatToParts()), J(en.format(undefined)));
// CHECK-NEXT: "" [] ""
// A string iterates by code point.
print(J(en.format('abc')), J(en.format('a😀')));
// CHECK-NEXT: "a, b, and c" "a and 😀"
print(J(en.format(new Set(['x', 'y']))), J(en.format(new Map([['k', 'v']]).keys())));
// CHECK-NEXT: "x and y" "k"
print(J(en.format((function* () { yield 'g'; yield 'h'; })())));
// CHECK-NEXT: "g and h"
// A value that is not a String closes the iterator: one return call, after
// two next calls, and a TypeError. A plain array iterator has no return.
var nextCalls = 0;
var returnCalls = 0;
var custom = {};
custom[Symbol.iterator] = function () {
  var values = ['a', 1, 'b'];
  return {
    next() {
      return {done: false, value: values[nextCalls++]};
    },
    return() {
      returnCalls++;
      return {};
    },
  };
};
print(thrown(() => en.format(custom)), nextCalls, returnCalls);
// CHECK-NEXT: TypeError 2 1
nextCalls = 0;
returnCalls = 0;
print(thrown(() => en.formatToParts(custom)), nextCalls, returnCalls);
// CHECK-NEXT: TypeError 2 1
print(typeof [][Symbol.iterator]().return);
// CHECK-NEXT: undefined
print(thrown(() => en.format(['a', 1])), thrown(() => en.format(['a', null])), thrown(() => en.format([Symbol()])));
// CHECK-NEXT: TypeError TypeError TypeError
print(thrown(() => en.format(null)), thrown(() => en.format(5)), thrown(() => en.format({})));
// CHECK-NEXT: TypeError TypeError TypeError
print(thrown(() => en.format([new String('x')])), thrown(() => en.formatToParts(['a', new String('x')])));
// CHECK-NEXT: TypeError TypeError

// The constructor and its options.
print(J(new Intl.ListFormat().resolvedOptions()));
// CHECK-NEXT: {"locale":"en-US","type":"conjunction","style":"long"}
print(J(new Intl.ListFormat('es-MX', {type: 'unit', style: 'narrow'}).resolvedOptions()));
// CHECK-NEXT: {"locale":"es-MX","type":"unit","style":"narrow"}
// ListFormat has no relevant extension keys.
print(new Intl.ListFormat('de-u-co-phonebk').resolvedOptions().locale);
// CHECK-NEXT: de
print(thrown(() => new Intl.ListFormat('en', {type: 'foo'})), thrown(() => new Intl.ListFormat('en', {style: 'long '})));
// CHECK-NEXT: RangeError RangeError
print(thrown(() => Intl.ListFormat()), thrown(() => Intl.ListFormat.call(new Intl.ListFormat())));
// CHECK-NEXT: TypeError TypeError
// GetOptionsObject: only undefined or an Object.
print(thrown(() => new Intl.ListFormat('en', null)), thrown(() => new Intl.ListFormat('en', 'x')), thrown(() => new Intl.ListFormat('en', 5)));
// CHECK-NEXT: TypeError TypeError TypeError
print(thrown(() => new Intl.ListFormat('en-')), thrown(() => new Intl.ListFormat('en', {localeMatcher: 'x'})));
// CHECK-NEXT: RangeError RangeError
// The options are read in the order localeMatcher, type, style, each
// converted with ToString when read.
var reads = [];
var options = {};
['localeMatcher', 'type', 'style'].forEach(function (name) {
  Object.defineProperty(options, name, {
    get() {
      reads.push(name);
      return {toString() { reads.push(name + ' toString'); return undefined; }};
    },
  });
});
print(thrown(() => new Intl.ListFormat('en', options)), reads.join());
// CHECK-NEXT: RangeError localeMatcher,localeMatcher toString
var ordered = {
  get localeMatcher() { reads.push('localeMatcher'); return 'lookup'; },
  get type() { reads.push('type'); return 'unit'; },
  get style() { reads.push('style'); return 'short'; },
};
reads = [];
print(J(new Intl.ListFormat('en', ordered).resolvedOptions()), reads.join());
// CHECK-NEXT: {"locale":"en","type":"unit","style":"short"} localeMatcher,type,style
// resolvedOptions returns a new ordinary object each time.
var ro = en.resolvedOptions();
print(ro !== en.resolvedOptions(), Object.getPrototypeOf(ro) === Object.prototype, Object.keys(ro).join());
// CHECK-NEXT: true true locale,type,style

// supportedLocalesOf and @@toStringTag.
print(J(Intl.ListFormat.supportedLocalesOf(['de', 'xx'])), J(Intl.ListFormat.supportedLocalesOf('en-US', {localeMatcher: 'lookup'})));
// CHECK-NEXT: ["de"] ["en-US"]
print(Object.prototype.toString.call(new Intl.ListFormat()));
// CHECK-NEXT: [object Intl.ListFormat]
// A part is an ordinary object with type and value, in that order.
var part = en.formatToParts(['a', 'b'])[1];
print(Object.getPrototypeOf(part) === Object.prototype, Object.keys(part).join());
// CHECK-NEXT: true type,value
