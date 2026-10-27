/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US %s | %FileCheck --match-full-lines %s

// Objects that look like Intl.Segmenter objects, Segments objects or
// Segment Iterators (plan Review Focus 3, the Segmenter part). Each brand
// check is a private field, so an object made from a prototype, a proxy,
// an object of one of the other two kinds, and instances of the other Intl
// constructors (with native state or private fields of their own) are
// rejected, while a subclass instance is a Segmenter. Segmenter has no
// legacy constructor behavior. Its methods are plain methods; a detached
// method has no this. No case reaches native code with an object it
// rejects, which would surface as "Exception in HostFunction:".

var proto = Intl.Segmenter.prototype;
var resolvedOptions = proto.resolvedOptions;
var segment = proto.segment;
var segmenter = new Intl.Segmenter('en', {granularity: 'word'});
var segments = segmenter.segment('Hello world');
var iterator = segments[Symbol.iterator]();
var SegmentsPrototype = Object.getPrototypeOf(segments);
var SegmentIteratorPrototype = Object.getPrototypeOf(iterator);
var containing = SegmentsPrototype.containing;
var iterate = SegmentsPrototype[Symbol.iterator];
var next = SegmentIteratorPrototype.next;

/// The class of the exception f throws, 'none', or the message of an
/// internal error from native code.
function thrown(f) {
  try {
    f();
    return 'none';
  } catch (e) {
    var message = String(e.message);
    if (message.indexOf('HostFunction') !== -1) return 'native: ' + message;
    return e.constructor.name;
  }
}

/// The five brand-checked methods on o: resolvedOptions and segment of
/// Intl.Segmenter.prototype, containing and @@iterator of
/// %IntlSegmentsPrototype%, and next of %IntlSegmentIteratorPrototype%.
function brand(o) {
  print(
    thrown(() => resolvedOptions.call(o)),
    thrown(() => segment.call(o, 'a b')),
    thrown(() => containing.call(o, 0)),
    thrown(() => iterate.call(o)),
    thrown(() => next.call(o)),
  );
}

// The real objects pass their own checks and fail the others: a Segments
// object passed to next, an iterator passed to containing, a Segmenter
// passed to either.
brand(segmenter);
// CHECK: none none TypeError TypeError TypeError
brand(segments);
// CHECK-NEXT: TypeError TypeError none none TypeError
brand(iterator);
// CHECK-NEXT: TypeError TypeError TypeError TypeError none
// Objects made from the prototypes have no slots.
brand(Object.create(proto));
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError
brand(Object.create(SegmentsPrototype));
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError
brand(Object.create(SegmentIteratorPrototype));
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError
brand(SegmentsPrototype);
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError
brand(SegmentIteratorPrototype);
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError
brand(proto);
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError
// Proxies do not have the private fields.
brand(new Proxy(segmenter, {}));
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError
brand(new Proxy(segments, {}));
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError
brand(new Proxy(iterator, {}));
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError
// Instances of the other constructors: with native state (Collator,
// PluralRules, DateTimeFormat, ListFormat) and with private fields only
// (NumberFormat, Locale, RelativeTimeFormat, DisplayNames,
// DurationFormat); and an array iterator.
brand(new Intl.Collator());
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError
brand(new Intl.PluralRules());
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError
brand(new Intl.DateTimeFormat());
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError
brand(new Intl.ListFormat());
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError
brand(new Intl.NumberFormat());
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError
brand(new Intl.Locale('en'));
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError
brand(new Intl.RelativeTimeFormat());
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError
brand(new Intl.DisplayNames('en', {type: 'region'}));
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError
brand(new Intl.DurationFormat());
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError
brand([][Symbol.iterator]());
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError
brand(undefined);
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError
brand('a b');
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError

// The brand checks come before the argument is converted.
var touched = false;
var arg = {
  toString() {
    touched = true;
    return 'x';
  },
  valueOf() {
    touched = true;
    return 0;
  },
};
print(thrown(() => segment.call({}, arg)), thrown(() => containing.call(iterator, arg)), touched);
// CHECK-NEXT: TypeError TypeError false
// The other constructors' methods reject a Segmenter.
print(
  thrown(() => Intl.Collator.prototype.resolvedOptions.call(segmenter)),
  thrown(() => Intl.ListFormat.prototype.format.call(segmenter, [])),
  thrown(() => Intl.NumberFormat.prototype.formatToParts.call(segmenter, 1)),
  thrown(() => Intl.DurationFormat.prototype.format.call(segmenter, {hours: 1})),
);
// CHECK-NEXT: TypeError TypeError TypeError TypeError
// The rejected calls left the real objects as they were: the one call of
// next that passed, in brand(iterator), took "Hello".
print(JSON.stringify(iterator.next().value), segments.containing(7).segment);
// CHECK-NEXT: {"segment":" ","index":5,"input":"Hello world","isWordLike":false} world

// A subclass instance is a Segmenter, made from the subclass's prototype,
// with a native segmenter of its own; its own and the borrowed methods
// work, and its Segments objects are ordinary ones.
class X extends Intl.Segmenter {}
var sub = new X('en', {granularity: 'sentence'});
var subSegments = sub.segment('One. Two.');
print(
  Object.getPrototypeOf(sub) === X.prototype,
  sub instanceof Intl.Segmenter,
  JSON.stringify(sub.resolvedOptions()),
  JSON.stringify([...subSegments].map(s => s.segment)),
  segment.call(sub, 'A. B.').containing(3).segment,
  Object.getPrototypeOf(subSegments) === SegmentsPrototype,
);
// CHECK-NEXT: true true {"locale":"en","granularity":"sentence"} ["One. ","Two."] B. true

// Without new, Intl.Segmenter throws, whatever this is, and does not touch
// an instance it is called on.
var wrapper = Object.create(proto);
print(
  thrown(() => Intl.Segmenter.call(wrapper, 'de')),
  thrown(() => Intl.Segmenter.call(segmenter, 'de')),
  Reflect.ownKeys(wrapper).length,
  segmenter.resolvedOptions().locale,
);
// CHECK-NEXT: TypeError TypeError 0 en
brand(wrapper);
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError

// A detached method has no this; methods are not constructors.
var detached = segmenter.segment;
print(
  thrown(() => detached('a')),
  thrown(() => new segment('a')),
  thrown(() => new containing(0)),
  thrown(() => new iterate()),
  thrown(() => new next()),
);
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError

// Reflect.construct with a newTarget makes the object from its prototype.
function F() {}
F.prototype = Object.create(proto);
var fromF = Reflect.construct(Intl.Segmenter, ['fr', {granularity: 'word'}], F);
print(Object.getPrototypeOf(fromF) === F.prototype, fromF.resolvedOptions().locale, fromF.segment('a b').containing(2).isWordLike);
// CHECK-NEXT: true fr true

// The design spec's documented deviation ("Constructors", "Documented
// deviation"): when newTarget.prototype is not an object, Hermes creates
// this from Object.prototype, where ECMA-402 requires
// %Intl.Segmenter.prototype%. The object is still a Segmenter, but segment
// is not reachable through its prototype.
function G() {}
G.prototype = 5;
var deviant = Reflect.construct(Intl.Segmenter, ['de'], G);
print(
  Object.getPrototypeOf(deviant) === Object.prototype,
  typeof deviant.segment,
  resolvedOptions.call(deviant).locale,
  segment.call(deviant, 'ab').containing(1).segment,
);
// CHECK-NEXT: true undefined de b
