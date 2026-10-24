/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: cat %S/Inputs/strict.js %intl_js/01-intrinsics.js %intl_js/02-shapes.js %intl_js/03-locales.js %intl_js/04-options.js %s > %t.js
// RUN: %hermes_default -Xintl-plugin=false %t.js | %FileCheck --match-full-lines %s
// RUN: %hermesc -O -emit-binary -out=%t.hbc %t.js && %hermes_default -Xintl-plugin=false -b %t.hbc | %FileCheck --match-full-lines %s

// The function-shape helpers and the ReturnOverride private-field pattern
// (design spec "Constructors" and "Function shapes") in isolation.

// Stubs for what the plugin's C++ passes to setup.
var available = ['en', 'en-US', 'de', 'fr'];
var native = {
  canonicalizeLocale: function (tag) {
    if (!/^[a-z]{2,3}(-[A-Za-z0-9]{1,8})*$/.test(tag)) return undefined;
    // One UTS 35 value alias, so that callers which read a value back from
    // the canonical tag see it change.
    return tag.replace(/-u-ca-islamicc(?=-|$)/, '-u-ca-islamic-civil');
  },
  isAvailableLocale: function (tag) {
    return available.indexOf(tag) !== -1;
  },
};
var hostDefaults = {locales: ['fr'], timeZone: 'UTC'};
// 20-Locale.js is not included, so no object is an Intl.Locale.
function isLocaleObject(o) {
  return false;
}
function localeObjectTag(o) {
  throw new Error('not a Locale');
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

/// Whether f can be called with new, without calling it.
function isConstructor(f) {
  try {
    Reflect.construct(String, [], f);
    return true;
  } catch (e) {
    return false;
  }
}

var target = {};
defineMethods(target, {foo(a, b) { return a + b; }});
var d = Object.getOwnPropertyDescriptor(target, 'foo');
print(d.writable, d.enumerable, d.configurable, d.value.name, d.value.length, 'prototype' in d.value, isConstructor(d.value));
// CHECK: true false true foo 2 false false

defineGetters(target, {get bar() { return 1; }});
var g = Object.getOwnPropertyDescriptor(target, 'bar');
print(typeof g.get, g.set, g.enumerable, g.configurable, g.get.name);
// CHECK-NEXT: function undefined false true get bar

function Ctor(a) {}
var proto = {};
defineConstructor(Ctor, 0, proto);
var pd = Object.getOwnPropertyDescriptor(Ctor, 'prototype');
print(pd.value === proto, pd.writable, pd.enumerable, pd.configurable);
// CHECK-NEXT: true false false false
var ld = Object.getOwnPropertyDescriptor(Ctor, 'length');
print(ld.value, ld.writable, ld.enumerable, ld.configurable);
// CHECK-NEXT: 0 false false true
var cd = Object.getOwnPropertyDescriptor(proto, 'constructor');
print(cd.value === Ctor, cd.writable, cd.enumerable, cd.configurable);
// CHECK-NEXT: true true false true

// The functions the format/compare getters return: arrows from a factory.
function makeBound() {
  return (x) => x;
}
var bound = makeBound();
print(JSON.stringify(bound.name), bound.length, 'prototype' in bound, isConstructor(bound));
// CHECK-NEXT: "" 1 false false

setToStringTag(target, 'T');
var td = Object.getOwnPropertyDescriptor(target, Symbol.toStringTag);
print(td.value, td.writable, td.enumerable, td.configurable);
// CHECK-NEXT: T false false true

// The private-field pattern (spec "Constructors"): a subclass of
// ReturnOverride puts its private fields on the object passed to it.
class FooSlots extends ReturnOverride {
  #v;
  constructor(o, v) {
    super(o);
    this.#v = v;
  }
  static has(o) {
    return #v in o;
  }
  static get(o) {
    return o.#v;
  }
}
function Foo(v) {
  var o = Object.create(Foo.prototype);
  new FooSlots(o, v);
  return o;
}
var f = Foo(7);
print(FooSlots.has(f), FooSlots.get(f), FooSlots.has({}), Reflect.ownKeys(f).length, Object.getPrototypeOf(f) === Foo.prototype);
// CHECK-NEXT: true 7 false 0 true
print(thrown(() => new FooSlots(f, 8)));
// CHECK-NEXT: TypeError
var px = new Proxy({}, {});
new FooSlots(px, 9);
print(FooSlots.has(px), FooSlots.get(px));
// CHECK-NEXT: true 9
