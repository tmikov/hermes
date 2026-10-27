/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US %s | %FileCheck --match-full-lines %s

// Objects that look like Intl.ListFormat objects (plan Review Focus 3, the
// ListFormat part). The brand check is the private field, so an object
// made from the prototype, a proxy, and instances of the other Intl
// constructors (with native state or private fields of their own) are not
// ListFormats, while a subclass instance is. ListFormat has no legacy
// constructor behavior: called without new it throws, also on an
// instance, and leaves the object alone. Its methods are plain methods; a
// detached method has no this. No case reaches native code with an object
// it rejects, which would surface as "Exception in HostFunction:".

var proto = Intl.ListFormat.prototype;
var resolvedOptions = proto.resolvedOptions;
var format = proto.format;
var formatToParts = proto.formatToParts;

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

/// The three brand-checked methods on o: resolvedOptions, format and
/// formatToParts.
function brand(o) {
  print(
    thrown(() => resolvedOptions.call(o)),
    thrown(() => format.call(o, ['a', 'b'])),
    thrown(() => formatToParts.call(o, ['a', 'b'])),
  );
}

// An object made from the prototype has no slots.
brand(Object.create(proto));
// CHECK: TypeError TypeError TypeError
// A proxy of a ListFormat does not have its private fields.
brand(new Proxy(new Intl.ListFormat(), {}));
// CHECK-NEXT: TypeError TypeError TypeError
// Instances of the other constructors: with native state (Collator,
// PluralRules, DateTimeFormat) and with private fields only (NumberFormat,
// Locale).
brand(new Intl.Collator());
// CHECK-NEXT: TypeError TypeError TypeError
brand(new Intl.PluralRules());
// CHECK-NEXT: TypeError TypeError TypeError
brand(new Intl.DateTimeFormat());
// CHECK-NEXT: TypeError TypeError TypeError
brand(new Intl.NumberFormat());
// CHECK-NEXT: TypeError TypeError TypeError
brand(new Intl.Locale('en'));
// CHECK-NEXT: TypeError TypeError TypeError
brand(undefined);
// CHECK-NEXT: TypeError TypeError TypeError
brand('en');
// CHECK-NEXT: TypeError TypeError TypeError
brand(proto);
// CHECK-NEXT: TypeError TypeError TypeError
// The brand check comes before the list is read.
var touched = false;
var list = {};
list[Symbol.iterator] = function () {
  touched = true;
  return [][Symbol.iterator]();
};
print(thrown(() => format.call({}, list)), thrown(() => formatToParts.call({}, list)), touched);
// CHECK-NEXT: TypeError TypeError false
// The other constructors' methods reject a ListFormat.
var lf = new Intl.ListFormat('en');
print(
  thrown(() => Intl.Collator.prototype.resolvedOptions.call(lf)),
  thrown(() => Intl.PluralRules.prototype.select.call(lf, 1)),
  thrown(() => Intl.NumberFormat.prototype.formatToParts.call(lf, 1)),
  thrown(() => Intl.DateTimeFormat.prototype.formatToParts.call(lf, 0)),
);
// CHECK-NEXT: TypeError TypeError TypeError TypeError

// A subclass instance is a ListFormat, made from the subclass's prototype,
// with a native list formatter of its own; its own and the borrowed
// methods work.
class X extends Intl.ListFormat {}
var sub = new X('es', {type: 'disjunction'});
print(
  Object.getPrototypeOf(sub) === X.prototype,
  sub instanceof Intl.ListFormat,
  JSON.stringify(sub.resolvedOptions()),
  sub.format(['siete', 'ocho']),
  format.call(sub, ['a', 'b']),
  JSON.stringify(formatToParts.call(sub, ['x'])),
);
// CHECK-NEXT: true true {"locale":"es","type":"disjunction","style":"long"} siete u ocho a o b [{"type":"element","value":"x"}]

// Without new, Intl.ListFormat throws, whatever this is, and does not
// touch an instance it is called on.
var wrapper = Object.create(proto);
print(
  thrown(() => Intl.ListFormat.call(wrapper, 'de')),
  thrown(() => Intl.ListFormat.call(lf, 'de')),
  Reflect.ownKeys(wrapper).length,
  Reflect.ownKeys(lf).length,
);
// CHECK-NEXT: TypeError TypeError 0 0
brand(wrapper);
// CHECK-NEXT: TypeError TypeError TypeError
print(lf.format(['a', 'b']), lf.resolvedOptions().locale);
// CHECK-NEXT: a and b en

// A detached method has no this; methods are not constructors.
var detached = lf.format;
print(thrown(() => detached(['a'])), detached.call(lf, ['a', 'b', 'c']), thrown(() => new format(['a'])));
// CHECK-NEXT: TypeError a, b, and c TypeError

// Reflect.construct with a newTarget makes the object from its prototype.
function F() {}
F.prototype = Object.create(proto);
var fromF = Reflect.construct(Intl.ListFormat, ['fr'], F);
print(Object.getPrototypeOf(fromF) === F.prototype, fromF.resolvedOptions().locale, fromF.format(['a', 'b']));
// CHECK-NEXT: true fr a et b

// The design spec's documented deviation ("Constructors", "Documented
// deviation"): when newTarget.prototype is not an object, Hermes creates
// this from Object.prototype, where ECMA-402 requires
// %Intl.ListFormat.prototype%. The object is still a ListFormat, but
// format is not reachable through its prototype.
function G() {}
G.prototype = 5;
var deviant = Reflect.construct(Intl.ListFormat, ['de'], G);
print(
  Object.getPrototypeOf(deviant) === Object.prototype,
  typeof deviant.format,
  resolvedOptions.call(deviant).locale,
  format.call(deviant, ['a', 'b']),
);
// CHECK-NEXT: true undefined de a und b

// An instance has no own properties: its slots are private, and its
// native state is not a property.
print(Reflect.ownKeys(new Intl.ListFormat('en', {type: 'unit'})).length);
// CHECK-NEXT: 0
