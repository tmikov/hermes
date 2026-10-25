/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US %s | %FileCheck --match-full-lines %s

// Objects that look like Intl.PluralRules objects (plan Review Focus 4,
// the PluralRules half). The brand check is the private field, so an
// object made from the prototype, a proxy, an Intl.Collator and an
// Intl.NumberFormat are not PluralRules, while a subclass instance is.
// PluralRules has no legacy constructor behavior: called without new it
// throws, also on an instance, and leaves the object alone. It has no
// bound function either; a detached method has no this. No case reaches
// native code with an object it rejects, which would surface as
// "Exception in HostFunction:".

var proto = Intl.PluralRules.prototype;
var resolvedOptions = proto.resolvedOptions;
var select = proto.select;
var selectRange = proto.selectRange;

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

/// The three brand-checked methods on o: resolvedOptions, select and
/// selectRange.
function brand(o) {
  print(
    thrown(() => resolvedOptions.call(o)),
    thrown(() => select.call(o, 1)),
    thrown(() => selectRange.call(o, 1, 2)),
  );
}

// An object made from the prototype has no slots.
brand(Object.create(proto));
// CHECK: TypeError TypeError TypeError
// A proxy of a PluralRules does not have its private fields.
brand(new Proxy(new Intl.PluralRules(), {}));
// CHECK-NEXT: TypeError TypeError TypeError
// An Intl.Collator (with native state) and an Intl.NumberFormat (with
// private fields only) have private fields, but not a PluralRules's.
brand(new Intl.Collator());
// CHECK-NEXT: TypeError TypeError TypeError
brand(new Intl.NumberFormat('en', {style: 'unit', unit: 'meter', unitDisplay: 'long'}));
// CHECK-NEXT: TypeError TypeError TypeError
brand(undefined);
// CHECK-NEXT: TypeError TypeError TypeError
brand(5);
// CHECK-NEXT: TypeError TypeError TypeError
brand(proto);
// CHECK-NEXT: TypeError TypeError TypeError
// The NumberFormat methods reject a PluralRules.
var pr = new Intl.PluralRules('en');
print(
  thrown(() => Intl.NumberFormat.prototype.resolvedOptions.call(pr)),
  thrown(() => Intl.NumberFormat.prototype.formatToParts.call(pr, 1)),
  thrown(() => Intl.Collator.prototype.resolvedOptions.call(pr)),
);
// CHECK-NEXT: TypeError TypeError TypeError

// A subclass instance is a PluralRules, made from the subclass's
// prototype, with native plural rules of its own.
class P extends Intl.PluralRules {}
var sub = new P('ar', {type: 'ordinal'});
print(
  Object.getPrototypeOf(sub) === P.prototype,
  sub.resolvedOptions().locale,
  sub.resolvedOptions().type,
  new P('ar').select(2),
  sub.select(2),
);
// CHECK-NEXT: true ar ordinal two other

// Without new, Intl.PluralRules throws, whatever this is, and does not
// touch an instance it is called on.
var wrapper = Object.create(proto);
print(
  thrown(() => Intl.PluralRules.call(wrapper, 'de')),
  thrown(() => Intl.PluralRules.call(pr)),
  Reflect.ownKeys(wrapper).length,
  Reflect.ownKeys(pr).length,
);
// CHECK-NEXT: TypeError TypeError 0 0
// The wrapper is still rejected, and pr still works.
brand(wrapper);
// CHECK-NEXT: TypeError TypeError TypeError
print(pr.select(1), pr.resolvedOptions().locale);
// CHECK-NEXT: one en

// A detached method has no this.
var detached = pr.select;
print(thrown(() => detached(1)), detached.call(pr, 2), thrown(() => new select(1)));
// CHECK-NEXT: TypeError other TypeError

// Reflect.construct with a newTarget makes the object from its prototype.
function F() {}
F.prototype = Object.create(proto);
var fromF = Reflect.construct(Intl.PluralRules, ['fr'], F);
print(Object.getPrototypeOf(fromF) === F.prototype, fromF.resolvedOptions().locale, fromF.select(1.5));
// CHECK-NEXT: true fr one

// The design spec's documented deviation ("Constructors", "Documented
// deviation"): when newTarget.prototype is not an object, Hermes creates
// this from Object.prototype, where ECMA-402 requires
// %Intl.PluralRules.prototype%. The object is still a PluralRules, but
// select is not reachable through its prototype.
function G() {}
G.prototype = 5;
var deviant = Reflect.construct(Intl.PluralRules, ['ru'], G);
print(
  Object.getPrototypeOf(deviant) === Object.prototype,
  typeof deviant.select,
  resolvedOptions.call(deviant).locale,
  select.call(deviant, 5),
);
// CHECK-NEXT: true undefined ru many

// An instance has no own properties: its slots are private, and its
// native state is not a property.
print(Reflect.ownKeys(new Intl.PluralRules('en', {notation: 'compact'})).length);
// CHECK-NEXT: 0
