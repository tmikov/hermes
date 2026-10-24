/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes %s | %FileCheck --match-full-lines %s

// Objects that look like Intl.Locale objects but are not. The brand check
// is the private field, so an object made from the prototype and a proxy
// are not Locales, while a subclass instance is. CanonicalizeLocaleList
// reads the tag of a real Locale from its slot, and converts anything else
// with ToString.

/// The class of the exception f throws, or 'none'.
function thrown(f) {
  try {
    f();
    return 'none';
  } catch (e) {
    return e.constructor.name;
  }
}

// An object made from the prototype has no slots: the getters throw, and
// ToString reaches the inherited toString, which throws too.
var fake = Object.create(Intl.Locale.prototype);
print(thrown(() => fake.baseName));
// CHECK: TypeError
print(thrown(() => Intl.getCanonicalLocales([fake])));
// CHECK-NEXT: TypeError

// With its own toString it is a plain object, converted with ToString.
var fakeDe = Object.create(Intl.Locale.prototype);
fakeDe.toString = function () {
  return 'de';
};
print(JSON.stringify(Intl.getCanonicalLocales([fakeDe])));
// CHECK-NEXT: ["de"]

// A proxy of a Locale does not have the Locale's private fields.
var proxy = new Proxy(new Intl.Locale('en'), {});
print(thrown(() => proxy.language));
// CHECK-NEXT: TypeError
print(thrown(() => Intl.getCanonicalLocales([proxy])));
// CHECK-NEXT: TypeError

// A subclass instance is a Locale.
class L extends Intl.Locale {}
var sub = new L('en-us');
print(sub.baseName, Object.getPrototypeOf(sub) === L.prototype);
// CHECK-NEXT: en-US true
print(JSON.stringify(Intl.getCanonicalLocales([sub])));
// CHECK-NEXT: ["en-US"]

// A Locale's own toString is never called: its tag comes from the slot.
var loc = new Intl.Locale('en-US');
var calls = 0;
loc.toString = function () {
  calls++;
  throw new Error('toString was called');
};
print(JSON.stringify(Intl.getCanonicalLocales([loc, 'en-US'])), calls);
// CHECK-NEXT: ["en-US"] 0
// Nor by the constructor, which also takes the tag from the slot.
print(Intl.Locale.prototype.toString.call(new Intl.Locale(loc)), calls);
// CHECK-NEXT: en-US 0

// A Locale and a string with the same canonical tag are one entry.
print(JSON.stringify(Intl.getCanonicalLocales([new Intl.Locale('en-us'), 'EN-US'])));
// CHECK-NEXT: ["en-US"]

// Intl.Locale requires new.
print(thrown(() => Intl.Locale('en')));
// CHECK-NEXT: TypeError
