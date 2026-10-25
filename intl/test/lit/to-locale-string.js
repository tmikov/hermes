/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xhermes-internal-test-methods -Xintl-default-locale=en-US %s | %FileCheck --match-full-lines %s

// The ECMA-402 versions of Number.prototype.toLocaleString,
// BigInt.prototype.toLocaleString, Array.prototype.toLocaleString and
// %TypedArray%.prototype.toLocaleString: they format with a NumberFormat
// for locales and options, and the list methods pass both to each
// element's toLocaleString. The expected strings are ICU4C's (Node 24,
// ICU 78.2).

/// The name of the error f throws, or "none".
function thrown(f) {
  try {
    f();
    return 'none';
  } catch (e) {
    return e.name;
  }
}

print('Number');
// CHECK-LABEL: Number
print((1234.5).toLocaleString('de'), (1234.5).toLocaleString());
// CHECK-NEXT: 1.234,5 1,234.5
// The default NumberFormat again, and with both arguments undefined.
print((-1234.5).toLocaleString(), (0.5).toLocaleString(undefined, undefined));
// CHECK-NEXT: -1,234.5 0.5
print((5).toLocaleString('en', {style: 'percent'}));
// CHECK-NEXT: 500%
print((1234.5).toLocaleString('en', {style: 'currency', currency: 'EUR'}));
// CHECK-NEXT: €1,234.50
// ThisNumberValue accepts a Number object; ToIntlMathematicalValue keeps -0.
print(new Number(-0).toLocaleString('de'), NaN.toLocaleString('de'));
// CHECK-NEXT: -0 NaN
// The default locale is en-US; Hermes's own method gave "Infinity".
print(Infinity.toLocaleString(), (-Infinity).toLocaleString());
// CHECK-NEXT: ∞ -∞
print(Number.prototype.toLocaleString.length);
// CHECK-NEXT: 0
print(thrown(() => Number.prototype.toLocaleString.call('1')));
// CHECK-NEXT: TypeError
print(thrown(() => Number.prototype.toLocaleString.call({valueOf: () => 1})));
// CHECK-NEXT: TypeError
print(thrown(() => Number.prototype.toLocaleString.call(1n)));
// CHECK-NEXT: TypeError
// ThisNumberValue comes before the NumberFormat reads any option.
var reads = 0;
var countingOptions = {
  get style() {
    reads++;
    return 'percent';
  },
};
print(thrown(() => Number.prototype.toLocaleString.call('1', 'en', countingOptions)), reads);
// CHECK-NEXT: TypeError 0
print((0.25).toLocaleString('en', countingOptions), reads);
// CHECK-NEXT: 25% 1
// The NumberFormat's errors.
print(thrown(() => (1).toLocaleString('x-invalid-')), thrown(() => (1).toLocaleString('en', {style: 'x'})));
// CHECK-NEXT: RangeError RangeError

print('BigInt');
// CHECK-LABEL: BigInt
print(12345678901234567890n.toLocaleString('en'), (-12345n).toLocaleString('de'));
// CHECK-NEXT: 12,345,678,901,234,567,890 -12.345
print((2n ** 70n).toLocaleString(), Object(5n).toLocaleString());
// CHECK-NEXT: 1,180,591,620,717,411,303,424 5
print(BigInt.prototype.toLocaleString.length);
// CHECK-NEXT: 0
print(thrown(() => BigInt.prototype.toLocaleString.call(1)));
// CHECK-NEXT: TypeError
print(thrown(() => BigInt.prototype.toLocaleString.call('1')));
// CHECK-NEXT: TypeError
print(thrown(() => BigInt.prototype.toLocaleString.call(Object(1))));
// CHECK-NEXT: TypeError
print(thrown(() => (1n).toLocaleString('en', {style: 'currency'})));
// CHECK-NEXT: TypeError

print('Array');
// CHECK-LABEL: Array
// Each element's toLocaleString gets locales and options; null and
// undefined give the empty string.
print(
  [
    1234.5,
    null,
    undefined,
    7n,
    {
      toLocaleString(l, o) {
        return l + '/' + o.style;
      },
    },
  ].toLocaleString('de', {style: 'decimal'}),
);
// CHECK-NEXT: 1.234,5,,,7,de/decimal
// Always two arguments, even when none were given.
print([{toLocaleString() { return arguments.length; }}].toLocaleString());
// CHECK-NEXT: 2
// The method's result goes through ToString.
print([{toLocaleString: () => ({toString: () => 'S'})}].toLocaleString());
// CHECK-NEXT: S
print([true, 'x', 1234].toLocaleString('de'), '[' + [].toLocaleString() + ']');
// CHECK-NEXT: true,x,1.234 []
// The method is generic: an array-like object, and a string.
print(Array.prototype.toLocaleString.call({length: 3, 0: 1234.5, 2: 'x'}, 'de'));
// CHECK-NEXT: 1.234,5,,x
print(Array.prototype.toLocaleString.call('ab'), '[' + Array.prototype.toLocaleString.call(5) + ']');
// CHECK-NEXT: a,b []
// The Gets happen in order: length, then each index.
var log = [];
var proxy = new Proxy(
  {length: 2, 0: 1, 1: 2},
  {
    get(t, k) {
      log.push(String(k));
      return t[k];
    },
  },
);
print(Array.prototype.toLocaleString.call(proxy), log.join());
// CHECK-NEXT: 1,2 length,0,1
print(Array.prototype.toLocaleString.length);
// CHECK-NEXT: 0
print(thrown(() => [{toLocaleString: 1}].toLocaleString()));
// CHECK-NEXT: TypeError
print(thrown(() => [{toLocaleString: () => Symbol()}].toLocaleString()));
// CHECK-NEXT: TypeError
print(thrown(() => Array.prototype.toLocaleString.call(null)));
// CHECK-NEXT: TypeError
print(thrown(() => [1].toLocaleString('x-invalid-')));
// CHECK-NEXT: RangeError
// Two behaviors of Hermes's own method that the spec does not have. An
// object this method is already joining gives the empty string, so a
// cycle ends instead of recursing until the stack overflows.
var cyclic = [1, 2];
cyclic.push(cyclic);
print(cyclic.toLocaleString('de'));
// CHECK-NEXT: 1,2,
var outer = [1, 2];
outer.push([3, 4, outer]);
print(outer.toLocaleString());
// CHECK-NEXT: 1,2,3,4,
var arrayLike = {length: 2, 0: 1234.5};
arrayLike[1] = {toLocaleString: () => Array.prototype.toLocaleString.call(arrayLike, 'de')};
print(Array.prototype.toLocaleString.call(arrayLike, 'de'));
// CHECK-NEXT: 1.234,5,
// The check covers only the objects this method is joining, not those
// join is joining (Hermes's built-in shares one set between the two), so a
// cycle through both is cut one level later than in Hermes and V8 ("1,").
var throughJoin = [1];
throughJoin.push({toString: () => throughJoin.toLocaleString()});
print(throughJoin.join());
// CHECK-NEXT: 1,1,
// An object that appears twice without nesting is joined both times.
var inner = [1234.5];
print([inner, inner].toLocaleString('de'));
// CHECK-NEXT: 1.234,5,1.234,5
// A throw ends the join of the array, which can then be joined again.
var throwing = [1, {toLocaleString() { throw new RangeError('x'); }}];
print(thrown(() => throwing.toLocaleString()));
// CHECK-NEXT: RangeError
throwing[1] = 2;
print(throwing.toLocaleString());
// CHECK-NEXT: 1,2
// A length above 2^32 - 1 is read like any other: JoinIndexedProperties
// reads the elements from index 0, so the first read's throw comes out.
// The object is unmarked after the throw, so a second call reads it again
// rather than giving the empty string of an object being joined.
/// The value f throws, or 'none'.
function thrownValue(f) {
  try {
    f();
    return 'none';
  } catch (e) {
    return e;
  }
}
var elementReads = 0;
var huge = {
  get 0() {
    elementReads++;
    throw 'visited';
  },
};
huge.length = 2 ** 32;
print(thrownValue(() => Array.prototype.toLocaleString.call(huge)), elementReads);
// CHECK-NEXT: visited 1
print(thrownValue(() => Array.prototype.toLocaleString.call(huge)), elementReads);
// CHECK-NEXT: visited 2
huge.length = 2 ** 53;
print(thrownValue(() => Array.prototype.toLocaleString.call(huge)), elementReads);
// CHECK-NEXT: visited 3
// The reads go in order: index 0, then index 1.
var order = [];
var huge2 = {
  length: 2 ** 32 + 5,
  get 0() {
    order.push('0');
    return 1234.5;
  },
  get 1() {
    order.push('1');
    throw 'second';
  },
};
print(thrownValue(() => Array.prototype.toLocaleString.call(huge2, 'de')), order.join());
// CHECK-NEXT: second 0,1

print('TypedArray');
// CHECK-LABEL: TypedArray
var TypedArrayPrototype = Object.getPrototypeOf(Int8Array.prototype);
print(new Float64Array([1234.5, 2]).toLocaleString('de'));
// CHECK-NEXT: 1.234,5,2
print(new BigInt64Array([1234n, -5n]).toLocaleString('en', {style: 'currency', currency: 'USD'}));
// CHECK-NEXT: $1,234.00,-$5.00
print('[' + new Int8Array(0).toLocaleString() + ']', TypedArrayPrototype.toLocaleString.length);
// CHECK-NEXT: [] 0
// The length is the typed array's, not a "length" property.
var ta = new Uint8Array([1, 2, 3]);
Object.defineProperty(ta, 'length', {value: 0});
print(ta.toLocaleString());
// CHECK-NEXT: 1,2,3
// ValidateTypedArray: a detached typed array, empty or not, and objects
// that are not typed arrays.
var detached = new Float64Array([1, 2]);
HermesInternal.detachArrayBuffer(detached.buffer);
print(thrown(() => detached.toLocaleString()));
// CHECK-NEXT: TypeError
var detachedEmpty = new Float64Array(0);
HermesInternal.detachArrayBuffer(detachedEmpty.buffer);
print(thrown(() => detachedEmpty.toLocaleString()));
// CHECK-NEXT: TypeError
print(thrown(() => TypedArrayPrototype.toLocaleString.call([1])), thrown(() => TypedArrayPrototype.toLocaleString.call(undefined)));
// CHECK-NEXT: TypeError TypeError
print(thrown(() => new Int8Array(1).toLocaleString('x-invalid-')));
// CHECK-NEXT: RangeError
