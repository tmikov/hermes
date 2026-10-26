/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US -Xintl-default-time-zone=UTC %s | %FileCheck --match-full-lines %s

// Objects that look like Intl.DateTimeFormat objects (plan Review Focus 4,
// the DateTimeFormat half). The brand check is the private field, so an
// object made from the prototype, a proxy and an Intl.NumberFormat are not
// DateTimeFormats, while a subclass instance is. The legacy wrapper that
// ChainDateTimeFormat returns is accepted by the format getter and
// resolvedOptions, which unwrap it, and rejected by every other method. No
// case reaches native code with an object it rejects, which would surface
// as "Exception in HostFunction:".

var proto = Intl.DateTimeFormat.prototype;
var getFormat = Object.getOwnPropertyDescriptor(proto, 'format').get;
var resolvedOptions = proto.resolvedOptions;
var formatToParts = proto.formatToParts;
var formatRange = proto.formatRange;
var formatRangeToParts = proto.formatRangeToParts;

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

/// The five brand-checked functions on o: the format getter,
/// resolvedOptions, formatToParts, formatRange and formatRangeToParts.
function brand(o) {
  print(
    thrown(() => getFormat.call(o)),
    thrown(() => resolvedOptions.call(o)),
    thrown(() => formatToParts.call(o, 1)),
    thrown(() => formatRange.call(o, 1, 2)),
    thrown(() => formatRangeToParts.call(o, 1, 2)),
  );
}

// A DateTimeFormat passes every brand check.
brand(new Intl.DateTimeFormat());
// CHECK: none none none none none
// An object made from the prototype has no slots, and no fallback symbol
// to unwrap.
brand(Object.create(proto));
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError
// A proxy of a DateTimeFormat does not have its private fields. It is an
// instance (its prototype is the prototype), so the unwrap reads the
// fallback symbol through the proxy and finds nothing.
brand(new Proxy(new Intl.DateTimeFormat(), {}));
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError
// An Intl.NumberFormat has private fields, but not a DateTimeFormat's.
brand(new Intl.NumberFormat());
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError
brand(undefined);
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError
brand(5);
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError
brand(proto);
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError
brand(new Date(0));
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError

// A subclass instance is a DateTimeFormat, made from the subclass's
// prototype.
class D extends Intl.DateTimeFormat {}
var sub = new D('de', {dateStyle: 'long'});
print(
  Object.getPrototypeOf(sub) === D.prototype,
  sub.resolvedOptions().locale,
  sub.resolvedOptions().dateStyle,
);
// CHECK-NEXT: true de long
brand(sub);
// CHECK-NEXT: none none none none none

// The legacy wrapper (ChainDateTimeFormat): Intl.DateTimeFormat called
// without new on an instance of it returns that object, with the new
// DateTimeFormat in a non-writable, non-enumerable, non-configurable symbol
// property.
var wrapper = Object.create(proto);
var returned = Intl.DateTimeFormat.call(wrapper, 'de');
var symbols = Object.getOwnPropertySymbols(wrapper);
var d = Object.getOwnPropertyDescriptor(wrapper, symbols[0]);
print(
  returned === wrapper,
  symbols.length,
  symbols[0].description,
  Object.getOwnPropertyNames(wrapper).length,
  d.writable,
  d.enumerable,
  d.configurable,
);
// CHECK-NEXT: true 1 IntlLegacyConstructedSymbol 0 false false false
print(Object.getPrototypeOf(d.value) === proto, d.value.resolvedOptions().locale);
// CHECK-NEXT: true de
// The symbol is the one Intl.NumberFormat uses.
var nfWrapper = Object.create(Intl.NumberFormat.prototype);
Intl.NumberFormat.call(nfWrapper);
print(Object.getOwnPropertySymbols(nfWrapper)[0] === symbols[0]);
// CHECK-NEXT: true
// The format getter and resolvedOptions unwrap it; the format getter
// returns the inner DateTimeFormat's bound format.
print(wrapper.format === d.value.format, wrapper.resolvedOptions().locale);
// CHECK-NEXT: true de
print(thrown(() => wrapper.format(0)), wrapper.format(0));
// CHECK-NEXT: none 1.1.1970
// The other methods reject it.
print(
  thrown(() => wrapper.formatToParts(1)),
  thrown(() => wrapper.formatRange(1, 2)),
  thrown(() => wrapper.formatRangeToParts(1, 2)),
);
// CHECK-NEXT: TypeError TypeError TypeError
// A non-extensible instance cannot take the property: TypeError.
print(thrown(() => Intl.DateTimeFormat.call(Object.preventExtensions(Object.create(proto)))));
// CHECK-NEXT: TypeError
// Chaining twice redefines a non-configurable property: TypeError.
print(thrown(() => Intl.DateTimeFormat.call(wrapper)));
// CHECK-NEXT: TypeError
// The unwrap reads the symbol property with Get, so a wrapper's getter on
// that symbol is called, and a DateTimeFormat it returns is used.
var getterWrapper = Object.create(proto);
var getterCalls = 0;
Object.defineProperty(getterWrapper, symbols[0], {
  get() {
    getterCalls++;
    return new Intl.DateTimeFormat('en-IN');
  },
});
print(getterWrapper.resolvedOptions().locale, getterCalls);
// CHECK-NEXT: en-IN 1
// A getter that returns something else: the brand check after the unwrap
// throws.
var badWrapper = Object.create(proto);
Object.defineProperty(badWrapper, symbols[0], {value: {}});
print(thrown(() => badWrapper.resolvedOptions()), thrown(() => badWrapper.format));
// CHECK-NEXT: TypeError TypeError
// A DateTimeFormat this is an instance too, so it is chained: it keeps its
// own slots, which the unwrap prefers to the symbol property.
var dtf = new Intl.DateTimeFormat('en-US');
var chained = Intl.DateTimeFormat.call(dtf, 'de');
print(
  chained === dtf,
  Object.getOwnPropertySymbols(dtf).length,
  dtf.resolvedOptions().locale,
  dtf[Object.getOwnPropertySymbols(dtf)[0]].resolvedOptions().locale,
);
// CHECK-NEXT: true 1 en-US de
// A this that is not an instance is left alone: a new DateTimeFormat is
// returned.
var plain = {};
var made = Intl.DateTimeFormat.apply(plain, ['sv']);
print(
  made.resolvedOptions().locale,
  Object.getOwnPropertySymbols(plain).length,
  thrown(() => resolvedOptions.call(plain)),
);
// CHECK-NEXT: sv 0 TypeError
// With new, the this value is never chained, and an instance has no own
// properties: its slots are private.
var constructed = new Intl.DateTimeFormat('de');
print(Object.getOwnPropertySymbols(constructed).length, Reflect.ownKeys(constructed).length);
// CHECK-NEXT: 0 0

// The bound format, detached, keeps its own DateTimeFormat whatever this
// it gets: it is not a constructor, and it converts its argument with
// ToNumber (calling valueOf) before formatting.
var format = new Intl.DateTimeFormat('de').format;
var valueOfCalls = 0;
print(
  thrown(() => format(0)),
  thrown(() => format.call(undefined, 0)),
  thrown(() => format.call({}, 0)),
  thrown(() => format()),
);
// CHECK-NEXT: none none none none
print(format(0), format.call({}, 86400000), format.call(new Intl.DateTimeFormat('en'), 86400000));
// CHECK-NEXT: 1.1.1970 2.1.1970 2.1.1970
print(
  thrown(() => new format(1)),
  thrown(() => format(Symbol('a'))),
  thrown(() => format(1n)),
  thrown(() => format({valueOf: () => valueOfCalls++})),
  valueOfCalls,
);
// CHECK-NEXT: TypeError TypeError TypeError none 1
// formatToParts converts its argument too; undefined is Date.now().
print(thrown(() => dtf.formatToParts()), thrown(() => dtf.formatToParts(Symbol())));
// CHECK-NEXT: none TypeError

// formatRange and formatRangeToParts check their arguments before the
// range formatting, start then end.
var order = [];
var start = {valueOf: () => (order.push('start'), 0)};
var end = {valueOf: () => (order.push('end'), 1)};
print(
  thrown(() => dtf.formatRange(undefined, 1)),
  thrown(() => dtf.formatRange(1)),
  thrown(() => dtf.formatRangeToParts(1, undefined)),
  thrown(() => dtf.formatRange(1n, Symbol())),
  thrown(() => dtf.formatRange(start, end)),
  order.join(),
);
// CHECK-NEXT: TypeError TypeError TypeError TypeError none start,end
// The receiver check comes before the argument checks.
print(thrown(() => formatRange.call({}, undefined, undefined)));
// CHECK-NEXT: TypeError

// Reflect.construct with a newTarget makes the object from its prototype,
// which it reads once, before the options.
var reads = [];
var target = new Proxy(function () {}, {
  get(t, key) {
    reads.push(String(key));
    return key === 'prototype' ? proto : t[key];
  },
});
var options = {
  get timeZone() {
    reads.push('timeZone');
    return 'UTC';
  },
};
var fromProxy = Reflect.construct(Intl.DateTimeFormat, ['de', options], target);
print(Object.getPrototypeOf(fromProxy) === proto, fromProxy.resolvedOptions().locale, reads.join());
// CHECK-NEXT: true de prototype,timeZone
function F() {}
F.prototype = Object.create(proto);
var fromF = Reflect.construct(Intl.DateTimeFormat, ['de'], F);
print(Object.getPrototypeOf(fromF) === F.prototype, fromF.resolvedOptions().locale);
// CHECK-NEXT: true de

// The design spec's documented deviation ("Constructors", "Documented
// deviation"): when newTarget.prototype is not an object, Hermes creates
// this from Object.prototype, where ECMA-402 requires
// %Intl.DateTimeFormat.prototype%. The object is still a DateTimeFormat,
// but format is not reachable through its prototype. A Hermes fix would
// print "false function" here.
function G() {}
G.prototype = 5;
var deviant = Reflect.construct(Intl.DateTimeFormat, ['de'], G);
print(
  Object.getPrototypeOf(deviant) === Object.prototype,
  typeof deviant.format,
  resolvedOptions.call(deviant).locale,
  typeof getFormat.call(deviant),
);
// CHECK-NEXT: true undefined de function

// A failed construction leaves no half-made DateTimeFormat behind: an
// error in the options is thrown before the slots are attached.
var holder = Object.create(proto);
print(thrown(() => Intl.DateTimeFormat.call(holder, 'en', {timeZone: 'Nowhere'})), Reflect.ownKeys(holder).length);
// CHECK-NEXT: RangeError 0
print(thrown(() => resolvedOptions.call(holder)));
// CHECK-NEXT: TypeError
