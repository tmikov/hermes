/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US %s | %FileCheck --match-full-lines %s
// RUN: %hermes -Xintl-default-locale=tr %s | %FileCheck --match-full-lines --check-prefix=TR %s

// The ECMA-402 versions of String.prototype.localeCompare, toLocaleLowerCase
// and toLocaleUpperCase: the locale and options reach the collator and the
// case mapping, errors come from the right steps, and the methods are
// generic.

// The default locale, en-US, sorts a before b.
print('a'.localeCompare('b'));
// CHECK: -1
// Swedish sorts a with umlaut after z; German sorts it with a.
print('ä'.localeCompare('z', 'sv'), 'ä'.localeCompare('z', 'de'));
// CHECK-NEXT: 1 -1
// The options reach the collator.
print('a'.localeCompare('A', undefined, {sensitivity: 'base'}));
// CHECK-NEXT: 0
print('a'.localeCompare('A'), 'A'.localeCompare('a'), 'a'.localeCompare('a'));
// CHECK-NEXT: -1 1 0
print('2'.localeCompare('10', 'en-u-kn'), '2'.localeCompare('10', 'en'));
// CHECK-NEXT: -1 1

// Turkish uppercases i to dotted capital I, by prefix too; English does not.
print('i'.toLocaleUpperCase('tr'), 'i'.toLocaleUpperCase('tr-TR'), 'i'.toLocaleUpperCase('en'));
// CHECK-NEXT: İ İ I
// Only the first requested locale counts: Azerbaijani lowercases I to
// dotless i.
print('I'.toLocaleLowerCase(['az', 'en']));
// CHECK-NEXT: ı
print('I'.toLocaleLowerCase(['en', 'az']));
// CHECK-NEXT: i
// Greek drops the accents when uppercasing.
print('άλφα'.toLocaleUpperCase('el'));
// CHECK-NEXT: ΑΛΦΑ
// Lithuanian keeps the dot of i under an accent when lowercasing.
print(escape('Ì'.toLocaleLowerCase('lt')), escape('Ì'.toLocaleLowerCase('en')));
// CHECK-NEXT: i%u0307%u0300 %EC
// Lithuanian drops the dot above after i when uppercasing; the default
// mapping keeps it.
print(escape('i\u0307'.toLocaleUpperCase('lt')), escape('i\u0307'.toLocaleUpperCase('en')));
// CHECK-NEXT: I I%u0307
// Armenian uppercases the ligature ech-yiwn to ech-vew; the default mapping
// gives ech-yiwn.
print('և'.toLocaleUpperCase('hy'), 'և'.toLocaleUpperCase('en'));
// CHECK-NEXT: ԵՎ ԵՒ
// A locale with no tailoring, and one with a Unicode extension, give the
// default mapping or the tailoring of their language.
print('i'.toLocaleUpperCase('de'), 'i'.toLocaleUpperCase('tr-u-co-trad'));
// CHECK-NEXT: I İ

/// The class of the exception f throws, or 'none'.
function thrown(f) {
  try {
    f();
    return 'none';
  } catch (e) {
    return e.constructor.name;
  }
}
print(thrown(() => 'x'.toLocaleUpperCase('xx-invalid-')));
// CHECK-NEXT: RangeError
print(thrown(() => 'x'.toLocaleLowerCase([5])));
// CHECK-NEXT: TypeError
print(thrown(() => 'x'.localeCompare('y', 'xx-invalid-')));
// CHECK-NEXT: RangeError
print(thrown(() => 'x'.localeCompare('y', 'en', {sensitivity: 'x'})));
// CHECK-NEXT: RangeError
print(thrown(() => String.prototype.localeCompare.call(null, 'a')));
// CHECK-NEXT: TypeError
print(thrown(() => String.prototype.localeCompare.call(undefined, 'a')));
// CHECK-NEXT: TypeError
print(thrown(() => String.prototype.toLocaleLowerCase.call(null)));
// CHECK-NEXT: TypeError
print(thrown(() => String.prototype.toLocaleUpperCase.call(undefined)));
// CHECK-NEXT: TypeError
// The message names the method.
for (var name of ['localeCompare', 'toLocaleLowerCase', 'toLocaleUpperCase']) {
  try {
    String.prototype[name].call(null, 'a');
  } catch (e) {
    print(e.message);
  }
}
// CHECK-NEXT: String.prototype.localeCompare called on null or undefined
// CHECK-NEXT: String.prototype.toLocaleLowerCase called on null or undefined
// CHECK-NEXT: String.prototype.toLocaleUpperCase called on null or undefined

// The steps run in order: this and that are converted before the locales
// are read, and the locales before the options.
var log = [];
var probe = name => ({
  toString() {
    log.push(name);
    return name;
  },
});
thrown(() =>
  String.prototype.localeCompare.call(probe('this'), probe('that'), [probe('xx-invalid-')], {
    get usage() {
      log.push('usage');
    },
  }),
);
print(log.join(' '));
// CHECK-NEXT: this that xx-invalid-
log = [];
'a'.localeCompare('b', undefined, {
  get usage() {
    log.push('usage');
  },
});
print(log.join(' '));
// CHECK-NEXT: usage

// The methods are generic: this can be any value but undefined and null.
print(String.prototype.toLocaleUpperCase.call(true));
// CHECK-NEXT: TRUE
print(String.prototype.toLocaleLowerCase.call(123), String.prototype.toLocaleLowerCase.call({}));
// CHECK-NEXT: 123 [object object]
print(String.prototype.localeCompare.call(10, '9'), String.prototype.localeCompare.call(10, '9', 'en-u-kn'));
// CHECK-NEXT: -1 1
print(String.prototype.toLocaleUpperCase.call(new String('i'), 'tr'));
// CHECK-NEXT: İ

// With no locales, the case mapping uses the default locale.
print('default:', 'i'.toLocaleUpperCase(), 'I'.toLocaleLowerCase(), 'i'.toLocaleUpperCase([]));
// CHECK-NEXT: default: I i I
// TR: default: İ ı İ
