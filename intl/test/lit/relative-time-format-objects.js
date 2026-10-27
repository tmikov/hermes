/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US %s | %FileCheck --match-full-lines %s

// Objects that look like Intl.RelativeTimeFormat objects (plan Review
// Focus 3, the RelativeTimeFormat part). The brand check is the private
// field, so an object made from the prototype, a proxy, and instances of
// the other Intl constructors (with native state or private fields of
// their own) are not RelativeTimeFormats, while a subclass instance is.
// RelativeTimeFormat has no legacy constructor behavior: called without
// new it throws, also on an instance, and leaves the object alone. Its
// methods are plain methods; a detached method has no this. No case
// reaches native code with an object it rejects, which would surface as
// "Exception in HostFunction:".

var proto = Intl.RelativeTimeFormat.prototype;
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
    thrown(() => format.call(o, 1, 'day')),
    thrown(() => formatToParts.call(o, 1, 'day')),
  );
}

// An object made from the prototype has no slots.
brand(Object.create(proto));
// CHECK: TypeError TypeError TypeError
// A proxy of a RelativeTimeFormat does not have its private fields.
brand(new Proxy(new Intl.RelativeTimeFormat(), {}));
// CHECK-NEXT: TypeError TypeError TypeError
// Instances of the other constructors: with native state (Collator,
// PluralRules, DateTimeFormat, ListFormat) and with private fields only
// (NumberFormat, Locale).
brand(new Intl.Collator());
// CHECK-NEXT: TypeError TypeError TypeError
brand(new Intl.PluralRules());
// CHECK-NEXT: TypeError TypeError TypeError
brand(new Intl.DateTimeFormat());
// CHECK-NEXT: TypeError TypeError TypeError
brand(new Intl.ListFormat());
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
// The brand check comes before the value and the unit are converted.
var touched = [];
var value = {
  valueOf() {
    touched.push('value');
    return 1;
  },
};
var unit = {
  toString() {
    touched.push('unit');
    return 'day';
  },
};
print(thrown(() => format.call({}, value, unit)), thrown(() => formatToParts.call({}, value, unit)), touched.length);
// CHECK-NEXT: TypeError TypeError 0
// The other constructors' methods reject a RelativeTimeFormat.
var rtf = new Intl.RelativeTimeFormat('en');
print(
  thrown(() => Intl.Collator.prototype.resolvedOptions.call(rtf)),
  thrown(() => Intl.PluralRules.prototype.select.call(rtf, 1)),
  thrown(() => Intl.NumberFormat.prototype.formatToParts.call(rtf, 1)),
  thrown(() => Intl.DateTimeFormat.prototype.formatToParts.call(rtf, 0)),
  thrown(() => Intl.ListFormat.prototype.format.call(rtf, ['a'])),
);
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError

// A subclass instance is a RelativeTimeFormat, made from the subclass's
// prototype; its own and the borrowed methods work.
class X extends Intl.RelativeTimeFormat {}
var sub = new X('de', {numeric: 'auto'});
print(
  Object.getPrototypeOf(sub) === X.prototype,
  sub instanceof Intl.RelativeTimeFormat,
  JSON.stringify(sub.resolvedOptions()),
  sub.format(-1, 'day'),
  format.call(sub, 3, 'hours'),
  JSON.stringify(formatToParts.call(sub, 0, 'day')),
);
// CHECK-NEXT: true true {"locale":"de","style":"long","numeric":"auto","numberingSystem":"latn"} gestern in 3 Stunden [{"type":"literal","value":"heute"}]

// Without new, Intl.RelativeTimeFormat throws, whatever this is, and does
// not touch an instance it is called on.
var wrapper = Object.create(proto);
print(
  thrown(() => Intl.RelativeTimeFormat.call(wrapper, 'de')),
  thrown(() => Intl.RelativeTimeFormat.call(rtf, 'de')),
  Reflect.ownKeys(wrapper).length,
  Reflect.ownKeys(rtf).length,
);
// CHECK-NEXT: TypeError TypeError 0 0
brand(wrapper);
// CHECK-NEXT: TypeError TypeError TypeError
print(rtf.format(1, 'day'), rtf.resolvedOptions().locale);
// CHECK-NEXT: in 1 day en

// A detached method has no this; methods are not constructors.
var detached = rtf.format;
print(thrown(() => detached(1, 'day')), detached.call(rtf, -2, 'weeks'), thrown(() => new format(1, 'day')));
// CHECK-NEXT: TypeError 2 weeks ago TypeError

// Reflect.construct with a newTarget makes the object from its prototype.
function F() {}
F.prototype = Object.create(proto);
var fromF = Reflect.construct(Intl.RelativeTimeFormat, ['fr'], F);
print(Object.getPrototypeOf(fromF) === F.prototype, fromF.resolvedOptions().locale, fromF.format(2, 'day'));
// CHECK-NEXT: true fr dans 2 jours

// The design spec's documented deviation ("Constructors", "Documented
// deviation"): when newTarget.prototype is not an object, Hermes creates
// this from Object.prototype, where ECMA-402 requires
// %Intl.RelativeTimeFormat.prototype%. The object is still a
// RelativeTimeFormat, but format is not reachable through its prototype.
function G() {}
G.prototype = 5;
var deviant = Reflect.construct(Intl.RelativeTimeFormat, ['de'], G);
print(
  Object.getPrototypeOf(deviant) === Object.prototype,
  typeof deviant.format,
  resolvedOptions.call(deviant).locale,
  format.call(deviant, 1, 'year'),
);
// CHECK-NEXT: true undefined de in 1 Jahr

// An instance has no own properties: its slots are private.
print(Reflect.ownKeys(new Intl.RelativeTimeFormat('en', {style: 'narrow'})).length);
// CHECK-NEXT: 0
