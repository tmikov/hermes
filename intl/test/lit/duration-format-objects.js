/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US %s | %FileCheck --match-full-lines %s

// Objects that look like Intl.DurationFormat objects (plan Review Focus 3,
// the DurationFormat part). The brand check is the private field, so an
// object made from the prototype, a proxy, and instances of the other Intl
// constructors (with native state or private fields of their own) are not
// DurationFormats, while a subclass instance is. DurationFormat has no
// legacy constructor behavior: called without new it throws, also on an
// instance, and leaves the object alone. Its methods are plain methods; a
// detached method has no this. No case reaches native code with an object
// it rejects, which would surface as "Exception in HostFunction:".

var proto = Intl.DurationFormat.prototype;
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
    thrown(() => format.call(o, {hours: 1})),
    thrown(() => formatToParts.call(o, {hours: 1})),
  );
}

// An object made from the prototype has no slots.
brand(Object.create(proto));
// CHECK: TypeError TypeError TypeError
// A proxy of a DurationFormat does not have its private fields.
brand(new Proxy(new Intl.DurationFormat(), {}));
// CHECK-NEXT: TypeError TypeError TypeError
// Instances of the other constructors: with native state (Collator,
// PluralRules, DateTimeFormat, ListFormat) and with private fields only
// (NumberFormat, Locale, RelativeTimeFormat, DisplayNames).
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
brand(new Intl.RelativeTimeFormat());
// CHECK-NEXT: TypeError TypeError TypeError
brand(new Intl.DisplayNames('en', {type: 'region'}));
// CHECK-NEXT: TypeError TypeError TypeError
brand(undefined);
// CHECK-NEXT: TypeError TypeError TypeError
brand('en');
// CHECK-NEXT: TypeError TypeError TypeError
brand(proto);
// CHECK-NEXT: TypeError TypeError TypeError
// The brand check comes before the duration is read.
var touched = [];
var duration = {
  get hours() {
    touched.push('hours');
    return 1;
  },
};
print(thrown(() => format.call({}, duration)), thrown(() => formatToParts.call({}, duration)), touched.length);
// CHECK-NEXT: TypeError TypeError 0
// The other constructors' methods reject a DurationFormat.
var df = new Intl.DurationFormat('en');
print(
  thrown(() => Intl.Collator.prototype.resolvedOptions.call(df)),
  thrown(() => Intl.PluralRules.prototype.select.call(df, 1)),
  thrown(() => Intl.NumberFormat.prototype.formatToParts.call(df, 1)),
  thrown(() => Intl.DateTimeFormat.prototype.formatToParts.call(df, 0)),
  thrown(() => Intl.ListFormat.prototype.format.call(df, ['a'])),
  thrown(() => Intl.RelativeTimeFormat.prototype.format.call(df, 1, 'day')),
  thrown(() => Intl.DisplayNames.prototype.of.call(df, 'US')),
);
// CHECK-NEXT: TypeError TypeError TypeError TypeError TypeError TypeError TypeError

// A subclass instance is a DurationFormat, made from the subclass's
// prototype; its own and the borrowed methods work.
class X extends Intl.DurationFormat {}
var sub = new X('de', {style: 'long'});
print(
  Object.getPrototypeOf(sub) === X.prototype,
  sub instanceof Intl.DurationFormat,
  sub.resolvedOptions().style,
  sub.format({hours: 1, minutes: 30}),
  format.call(sub, {days: 2}),
  JSON.stringify(formatToParts.call(sub, {seconds: 1})),
);
// CHECK-NEXT: true true long 1 Stunde, 30 Minuten 2 Tage [{"type":"integer","value":"1","unit":"second"},{"type":"literal","value":" ","unit":"second"},{"type":"unit","value":"Sekunde","unit":"second"}]

// Without new, Intl.DurationFormat throws, whatever this is, and does not
// touch an instance it is called on.
var wrapper = Object.create(proto);
print(
  thrown(() => Intl.DurationFormat.call(wrapper, 'de')),
  thrown(() => Intl.DurationFormat.call(df, 'de')),
  Reflect.ownKeys(wrapper).length,
  Reflect.ownKeys(df).length,
);
// CHECK-NEXT: TypeError TypeError 0 0
brand(wrapper);
// CHECK-NEXT: TypeError TypeError TypeError
print(df.format({hours: 1}), df.resolvedOptions().locale);
// CHECK-NEXT: 1 hr en

// A detached method has no this; methods are not constructors.
var detached = df.format;
print(thrown(() => detached({hours: 1})), detached.call(df, {minutes: 2}), thrown(() => new format({hours: 1})));
// CHECK-NEXT: TypeError 2 min TypeError

// Reflect.construct with a newTarget makes the object from its prototype.
function F() {}
F.prototype = Object.create(proto);
var fromF = Reflect.construct(Intl.DurationFormat, ['fr', {style: 'digital'}], F);
print(Object.getPrototypeOf(fromF) === F.prototype, fromF.resolvedOptions().locale, fromF.format({hours: 1, seconds: 2, milliseconds: 5}));
// CHECK-NEXT: true fr 1:00:02,005

// The design spec's documented deviation ("Constructors", "Documented
// deviation"): when newTarget.prototype is not an object, Hermes creates
// this from Object.prototype, where ECMA-402 requires
// %Intl.DurationFormat.prototype%. The object is still a DurationFormat,
// but format is not reachable through its prototype.
function G() {}
G.prototype = 5;
var deviant = Reflect.construct(Intl.DurationFormat, ['de'], G);
print(
  Object.getPrototypeOf(deviant) === Object.prototype,
  typeof deviant.format,
  resolvedOptions.call(deviant).locale,
  format.call(deviant, {years: 1}),
);
// CHECK-NEXT: true undefined de 1 J

// An instance has no own properties: its slots are private, and so are the
// NumberFormats and the ListFormat it makes for format.
var used = new Intl.DurationFormat('en', {style: 'narrow'});
used.format({hours: 1, minutes: 2});
print(Reflect.ownKeys(used).length);
// CHECK-NEXT: 0
