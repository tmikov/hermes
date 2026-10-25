/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US %s | %FileCheck --match-full-lines %s

// Objects that look like Intl.Collator objects but are not. The brand check
// is the private field, so an object made from the prototype, a proxy and an
// Intl.Locale are not Collators, while a subclass instance is. No case
// reaches native code with an object it rejects, which would surface as
// "Exception in HostFunction:".

var getCompare = Object.getOwnPropertyDescriptor(
  Intl.Collator.prototype,
  'compare',
).get;
var resolvedOptions = Intl.Collator.prototype.resolvedOptions;

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

/// The two brand-checked functions on o: the compare getter and
/// resolvedOptions.
function brand(o) {
  print(
    thrown(() => getCompare.call(o)),
    thrown(() => resolvedOptions.call(o)),
  );
}

// An object made from the prototype has no slots.
brand(Object.create(Intl.Collator.prototype));
// CHECK: TypeError TypeError
// A proxy of a Collator does not have the Collator's private fields.
brand(new Proxy(new Intl.Collator(), {}));
// CHECK-NEXT: TypeError TypeError
// An Intl.Locale has private fields, but not the Collator's.
brand(new Intl.Locale('en'));
// CHECK-NEXT: TypeError TypeError
// Neither do primitives, nor the prototype itself.
brand(undefined);
// CHECK-NEXT: TypeError TypeError
brand('en');
// CHECK-NEXT: TypeError TypeError
brand(Intl.Collator.prototype);
// CHECK-NEXT: TypeError TypeError

// A subclass instance is a Collator, made from the subclass's prototype.
class C extends Intl.Collator {}
var sub = new C('de', {sensitivity: 'base'});
print(
  Object.getPrototypeOf(sub) === C.prototype,
  sub.resolvedOptions().locale,
  sub.resolvedOptions().sensitivity,
  sub.compare('a', 'A'),
  sub.compare('a', 'b'),
);
// CHECK-NEXT: true de base 0 -1

// The bound compare, detached, compares with its own collator whatever this
// it gets.
var compare = new Intl.Collator('en', {sensitivity: 'base'}).compare;
var variant = new Intl.Collator('en');
print(
  compare('a', 'A'),
  compare.call(undefined, 'a', 'A'),
  compare.call(variant, 'a', 'A'),
  compare.call({}, 'a', 'A'),
  compare.call(new Intl.Locale('en'), 'b', 'a'),
  variant.compare('a', 'A'),
);
// CHECK-NEXT: 0 0 0 0 1 -1
// It converts its arguments with ToString, and missing ones are
// "undefined".
print(compare(1, '1'), compare('undefined'), compare(), compare(null, 'NULL'));
// CHECK-NEXT: 0 0 0 0
// It is not a constructor.
print(thrown(() => new compare('a', 'b')));
// CHECK-NEXT: TypeError
// A symbol cannot be converted.
print(thrown(() => compare(Symbol('a'), 'a')));
// CHECK-NEXT: TypeError

// Intl.Collator called on a Collator makes a new one and leaves this alone.
var collator = new Intl.Collator('en-US');
var other = Intl.Collator.call(collator, 'de');
print(
  other !== collator,
  other instanceof Intl.Collator,
  other.resolvedOptions().locale,
  collator.resolvedOptions().locale,
);
// CHECK-NEXT: true true de en-US
// The same through apply, and with a plain object as this: no slots are
// added to it.
var plain = {};
var made = Intl.Collator.apply(plain, ['sv']);
print(made.resolvedOptions().locale, thrown(() => resolvedOptions.call(plain)));
// CHECK-NEXT: sv TypeError

// Reflect.construct with a newTarget makes the object from its prototype.
function F() {}
F.prototype = Object.create(Intl.Collator.prototype);
var constructed = Reflect.construct(Intl.Collator, ['de'], F);
print(
  Object.getPrototypeOf(constructed) === F.prototype,
  constructed.resolvedOptions().locale,
);
// CHECK-NEXT: true de

// The design spec's documented deviation ("Constructors", "Documented
// deviation"): when newTarget.prototype is not an object, Hermes creates
// this from Object.prototype, where ECMA-402 requires
// %Intl.Collator.prototype%. The object is still a Collator, but compare is
// not reachable through its prototype. A Hermes fix would print "false
// function" here.
function G() {}
G.prototype = 1;
var deviant = Reflect.construct(Intl.Collator, [], G);
print(Object.getPrototypeOf(deviant) === Object.prototype, typeof deviant.compare);
// CHECK-NEXT: true undefined
