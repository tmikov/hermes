/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US %s | %FileCheck --match-full-lines %s

// Objects that look like Intl.NumberFormat objects (plan Review Focus 4,
// the NumberFormat half). The brand check is the private field, so an
// object made from the prototype, a proxy and an Intl.Collator are not
// NumberFormats, while a subclass instance is. The legacy wrapper that
// ChainNumberFormat returns is accepted by the format getter and
// resolvedOptions, which unwrap it, and rejected by every other method. No
// case reaches native code with an object it rejects, which would surface
// as "Exception in HostFunction:".

var proto = Intl.NumberFormat.prototype;
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

// An object made from the prototype has no slots, and no fallback symbol
// to unwrap.
brand(Object.create(proto));
// CHECK: TypeError TypeError TypeError TypeError TypeError
// A proxy of a NumberFormat does not have its private fields. It is an
// instance (its prototype is the prototype), so the unwrap reads the
// fallback symbol through the proxy and finds nothing.
brand(new Proxy(new Intl.NumberFormat(), {}));
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError
// An Intl.Collator has private fields, but not a NumberFormat's.
brand(new Intl.Collator());
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError
brand(undefined);
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError
brand(5);
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError
brand(proto);
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError

// A subclass instance is a NumberFormat, made from the subclass's
// prototype.
class N extends Intl.NumberFormat {}
var sub = new N('de', {style: 'percent'});
print(
  Object.getPrototypeOf(sub) === N.prototype,
  sub.resolvedOptions().locale,
  sub.resolvedOptions().style,
  JSON.stringify(sub.formatToParts(12.5).map(p => p.type)),
);
// CHECK-NEXT: true de percent ["integer","group","integer","literal","percentSign"]

// The legacy wrapper (ChainNumberFormat): Intl.NumberFormat called without
// new on an instance of it returns that object, with the new NumberFormat
// in a non-writable, non-enumerable, non-configurable symbol property.
var wrapper = Object.create(proto);
var returned = Intl.NumberFormat.call(wrapper, 'de');
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
print(
  Object.getPrototypeOf(d.value) === proto,
  d.value.resolvedOptions().locale,
  d.value.format(1234.5),
);
// CHECK-NEXT: true de 1.234,5
// The format getter and resolvedOptions unwrap it; the format getter
// returns the inner NumberFormat's bound format.
print(
  wrapper.format(1234.5),
  wrapper.format === d.value.format,
  wrapper.resolvedOptions().locale,
);
// CHECK-NEXT: 1.234,5 true de
// The other methods reject it.
print(
  thrown(() => wrapper.formatToParts(1)),
  thrown(() => wrapper.formatRange(1, 2)),
  thrown(() => wrapper.formatRangeToParts(1, 2)),
);
// CHECK-NEXT: TypeError TypeError TypeError
// The symbol is the same one for every wrapper.
var wrapper2 = Object.create(proto);
Intl.NumberFormat.call(wrapper2);
print(Object.getOwnPropertySymbols(wrapper2)[0] === symbols[0]);
// CHECK-NEXT: true
// A non-extensible instance cannot take the property: TypeError.
print(thrown(() => Intl.NumberFormat.call(Object.preventExtensions(Object.create(proto)))));
// CHECK-NEXT: TypeError
// Chaining twice redefines a non-configurable property: TypeError.
print(thrown(() => Intl.NumberFormat.call(wrapper)));
// CHECK-NEXT: TypeError
// The unwrap reads the symbol property with Get, so a wrapper's getter on
// that symbol is called, and a NumberFormat it returns is used.
var getterWrapper = Object.create(proto);
var getterCalls = 0;
Object.defineProperty(getterWrapper, symbols[0], {
  get() {
    getterCalls++;
    return new Intl.NumberFormat('en-IN');
  },
});
print(getterWrapper.resolvedOptions().locale, getterCalls);
// CHECK-NEXT: en-IN 1
// A NumberFormat this is an instance too, so it is chained: it keeps its
// own slots, which the unwrap prefers to the symbol property.
var nf = new Intl.NumberFormat('en-US');
var chained = Intl.NumberFormat.call(nf, 'de');
print(
  chained === nf,
  Object.getOwnPropertySymbols(nf).length,
  nf.resolvedOptions().locale,
  nf.format(1234.5),
  nf[Object.getOwnPropertySymbols(nf)[0]].resolvedOptions().locale,
);
// CHECK-NEXT: true 1 en-US 1,234.5 de
// A this that is not an instance is left alone: a new NumberFormat is
// returned.
var plain = {};
var made = Intl.NumberFormat.apply(plain, ['sv']);
print(
  made.resolvedOptions().locale,
  Object.getOwnPropertySymbols(plain).length,
  thrown(() => resolvedOptions.call(plain)),
);
// CHECK-NEXT: sv 0 TypeError
// With new, the this value is never chained.
var constructed = new Intl.NumberFormat('de');
print(Object.getOwnPropertySymbols(constructed).length, Reflect.ownKeys(constructed).length);
// CHECK-NEXT: 0 0

// The bound format, detached, formats with its own NumberFormat whatever
// this it gets.
var format = new Intl.NumberFormat('de').format;
print(
  format(1234.5),
  format.call(undefined, 1234.5),
  format.call(new Intl.NumberFormat('en'), 1234.5),
  format.call({}, 1234.5),
);
// CHECK-NEXT: 1.234,5 1.234,5 1.234,5 1.234,5
// Missing arguments are undefined, which is NaN.
print(format(), thrown(() => new format(1)), thrown(() => format(Symbol('a'))));
// CHECK-NEXT: NaN TypeError TypeError
print(format(1n), format('1.5'), format({valueOf: () => 2}));
// CHECK-NEXT: 1 1,5 2

// Reflect.construct with a newTarget makes the object from its prototype.
function F() {}
F.prototype = Object.create(proto);
var fromF = Reflect.construct(Intl.NumberFormat, ['de'], F);
print(Object.getPrototypeOf(fromF) === F.prototype, fromF.resolvedOptions().locale);
// CHECK-NEXT: true de

// The design spec's documented deviation ("Constructors", "Documented
// deviation"): when newTarget.prototype is not an object, Hermes creates
// this from Object.prototype, where ECMA-402 requires
// %Intl.NumberFormat.prototype%. The object is still a NumberFormat, but
// format is not reachable through its prototype. A Hermes fix would print
// "false function" here.
function G() {}
G.prototype = 5;
var deviant = Reflect.construct(Intl.NumberFormat, ['de'], G);
print(
  Object.getPrototypeOf(deviant) === Object.prototype,
  typeof deviant.format,
  resolvedOptions.call(deviant).locale,
  getFormat.call(deviant)(1234.5),
);
// CHECK-NEXT: true undefined de 1.234,5

// formatRange and formatRangeToParts check their arguments before the
// range formatting, which a later task adds.
var en = new Intl.NumberFormat('en');
print(
  thrown(() => en.formatRange(undefined, 1)),
  thrown(() => en.formatRange(1)),
  thrown(() => en.formatRangeToParts(1, undefined)),
  thrown(() => en.formatRange(1n, Symbol())),
);
// CHECK-NEXT: TypeError TypeError TypeError TypeError
