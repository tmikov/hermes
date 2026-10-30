/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US %s | %FileCheck --match-full-lines %s

// Objects that look like Intl.DisplayNames objects (plan Review Focus 3,
// the DisplayNames part). The brand check is the private field, so an
// object made from the prototype, a proxy, and instances of the other Intl
// constructors (with native state or private fields of their own) are not
// DisplayNames, while a subclass instance is. DisplayNames has no legacy
// constructor behavior: called without new it throws, also on an instance,
// and leaves the object alone. Its methods are plain methods; a detached
// method has no this. No case reaches native code with an object it
// rejects, which would surface as "Exception in HostFunction:".

var proto = Intl.DisplayNames.prototype;
var resolvedOptions = proto.resolvedOptions;
var of = proto.of;

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

/// The two brand-checked methods on o: resolvedOptions and of.
function brand(o) {
  print(thrown(() => resolvedOptions.call(o)), thrown(() => of.call(o, 'US')));
}

// An object made from the prototype has no slots.
brand(Object.create(proto));
// CHECK: TypeError TypeError
// A proxy of a DisplayNames does not have its private fields.
brand(new Proxy(new Intl.DisplayNames('en', {type: 'region'}), {}));
// CHECK-NEXT: TypeError TypeError
// Instances of the other constructors: with native state (Collator,
// PluralRules, DateTimeFormat, ListFormat) and with private fields only
// (NumberFormat, Locale, RelativeTimeFormat).
brand(new Intl.Collator());
// CHECK-NEXT: TypeError TypeError
brand(new Intl.PluralRules());
// CHECK-NEXT: TypeError TypeError
brand(new Intl.DateTimeFormat());
// CHECK-NEXT: TypeError TypeError
brand(new Intl.ListFormat());
// CHECK-NEXT: TypeError TypeError
brand(new Intl.NumberFormat());
// CHECK-NEXT: TypeError TypeError
brand(new Intl.Locale('en'));
// CHECK-NEXT: TypeError TypeError
brand(new Intl.RelativeTimeFormat());
// CHECK-NEXT: TypeError TypeError
brand(undefined);
// CHECK-NEXT: TypeError TypeError
brand('en');
// CHECK-NEXT: TypeError TypeError
brand(proto);
// CHECK-NEXT: TypeError TypeError
// The brand check comes before the code is converted.
var touched = 0;
var code = {
  toString() {
    touched++;
    return 'US';
  },
};
print(thrown(() => of.call({}, code)), touched);
// CHECK-NEXT: TypeError 0
// The other constructors' methods reject a DisplayNames.
var dn = new Intl.DisplayNames('en', {type: 'region'});
print(
  thrown(() => Intl.Collator.prototype.resolvedOptions.call(dn)),
  thrown(() => Intl.PluralRules.prototype.select.call(dn, 1)),
  thrown(() => Intl.NumberFormat.prototype.formatToParts.call(dn, 1)),
  thrown(() => Intl.DateTimeFormat.prototype.formatToParts.call(dn, 0)),
  thrown(() => Intl.ListFormat.prototype.format.call(dn, ['a'])),
  thrown(() => Intl.RelativeTimeFormat.prototype.format.call(dn, 1, 'day')),
);
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError TypeError

// A subclass instance is a DisplayNames, made from the subclass's
// prototype; its own and the borrowed methods work.
class X extends Intl.DisplayNames {}
var sub = new X('de', {type: 'language', style: 'short'});
print(
  Object.getPrototypeOf(sub) === X.prototype,
  sub instanceof Intl.DisplayNames,
  JSON.stringify(sub.resolvedOptions()),
  sub.of('en-GB'),
  of.call(sub, 'fr'),
);
// CHECK-NEXT: true true {"locale":"de","style":"short","type":"language","fallback":"code","languageDisplay":"dialect"} Englisch (UK) Französisch

// Without new, Intl.DisplayNames throws, whatever this is, and does not
// touch an instance it is called on.
var wrapper = Object.create(proto);
print(
  thrown(() => Intl.DisplayNames.call(wrapper, 'de', {type: 'region'})),
  thrown(() => Intl.DisplayNames.call(dn, 'de', {type: 'region'})),
  Reflect.ownKeys(wrapper).length,
  Reflect.ownKeys(dn).length,
);
// CHECK-NEXT: TypeError TypeError 0 0
brand(wrapper);
// CHECK-NEXT: TypeError TypeError
print(dn.of('US'), dn.resolvedOptions().locale);
// CHECK-NEXT: United States en

// A detached method has no this; methods are not constructors.
var detached = dn.of;
print(thrown(() => detached('US')), detached.call(dn, 'DE'), thrown(() => new of('US')));
// CHECK-NEXT: TypeError Germany TypeError

// Reflect.construct with a newTarget makes the object from its prototype.
function F() {}
F.prototype = Object.create(proto);
var fromF = Reflect.construct(Intl.DisplayNames, ['fr', {type: 'region'}], F);
print(Object.getPrototypeOf(fromF) === F.prototype, fromF.resolvedOptions().locale, fromF.of('DE'));
// CHECK-NEXT: true fr Allemagne

// The design spec's documented deviation ("Constructors", "Documented
// deviation"): when newTarget.prototype is not an object, Hermes creates
// this from Object.prototype, where ECMA-402 requires
// %Intl.DisplayNames.prototype%. The object is still a DisplayNames, but
// of is not reachable through its prototype.
function G() {}
G.prototype = 5;
var deviant = Reflect.construct(Intl.DisplayNames, ['de', {type: 'region'}], G);
print(
  Object.getPrototypeOf(deviant) === Object.prototype,
  typeof deviant.of,
  resolvedOptions.call(deviant).locale,
  of.call(deviant, 'US'),
);
// CHECK-NEXT: true undefined de Vereinigte Staaten

// An instance has no own properties: its slots are private.
print(Reflect.ownKeys(new Intl.DisplayNames('en', {type: 'script', style: 'narrow'})).length);
// CHECK-NEXT: 0
