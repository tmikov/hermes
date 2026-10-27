/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: cat %S/Inputs/strict.js %intl_js/01-intrinsics.js %intl_js/02-shapes.js %intl_js/06-iterators.js %s > %t.js
// RUN: %hermes_default -Xintl-plugin=false %t.js | %FileCheck --match-full-lines %s
// RUN: %hermesc -O -emit-binary -out=%t.hbc %t.js && %hermes_default -Xintl-plugin=false -b %t.hbc | %FileCheck --match-full-lines %s

// The iterator protocol (06-iterators.js, plan decision 3) in isolation:
// GetIterator, IteratorStepValue and IteratorCloseOnThrow, and the
// intrinsics 01-intrinsics.js captures for them.

/// The class of the exception f throws and its message, or 'none'.
function thrown(f) {
  try {
    f();
    return 'none';
  } catch (e) {
    return e.constructor.name + ': ' + e.message;
  }
}
/// The class of the exception f throws, or 'none'.
function thrownClass(f) {
  try {
    f();
    return 'none';
  } catch (e) {
    return e.constructor.name;
  }
}
/// Every value the iterable gives, in order.
function drain(iterable) {
  var record = GetIterator(iterable);
  var out = [];
  for (;;) {
    var v = IteratorStepValue(record);
    if (v === ITERATOR_DONE) return out;
    out.push(v);
  }
}
/// An iterable whose iterator logs every step to log and gives the
/// results results, one per call of next, with a return method that logs
/// its call (whether this is the iterator, and the argument count) and
/// returns returnResult.
function logged(log, results, returnResult) {
  var iterable = {};
  iterable[Symbol.iterator] = function () {
    log.push('@@iterator');
    var i = 0;
    var iterator = {
      get next() {
        log.push('get next');
        return function () {
          log.push('next');
          return results[i++];
        };
      },
      return() {
        log.push('return ' + (this === iterator) + ' ' + arguments.length);
        return returnResult;
      },
    };
    return iterator;
  };
  return iterable;
}

// The captured intrinsics.
print(SymbolIterator === Symbol.iterator);
// CHECK: true
print(IteratorPrototype === Object.getPrototypeOf(Object.getPrototypeOf([].values())));
// CHECK-NEXT: true
print(typeof IteratorPrototype[Symbol.iterator]);
// CHECK-NEXT: function

// Arrays, strings (by code point), Sets and generators.
print(JSON.stringify(drain(['a', 1, undefined])));
// CHECK-NEXT: ["a",1,null]
print(JSON.stringify(drain('a😀b')));
// CHECK-NEXT: ["a","😀","b"]
print(JSON.stringify(drain(new Set([3, 4]))));
// CHECK-NEXT: [3,4]
print(JSON.stringify(drain((function* () { yield 'g'; })())));
// CHECK-NEXT: ["g"]

// GetIterator: GetMethod's GetV throws for null and undefined; a method
// that is undefined or null, not callable, or that returns a primitive
// throws a TypeError.
print(thrownClass(() => GetIterator(null)), thrownClass(() => GetIterator(undefined)));
// CHECK-NEXT: TypeError TypeError
print(thrownClass(() => GetIterator(5)), thrownClass(() => GetIterator({})));
// CHECK-NEXT: TypeError TypeError
var iterableNull = {};
iterableNull[Symbol.iterator] = null;
var iterableObject = {};
iterableObject[Symbol.iterator] = {};
var iterableString = {};
iterableString[Symbol.iterator] = 'not callable';
print(
  thrownClass(() => GetIterator(iterableNull)),
  thrownClass(() => GetIterator(iterableObject)),
  thrownClass(() => GetIterator(iterableString)),
);
// CHECK-NEXT: TypeError TypeError TypeError
var primitiveIterator = {};
primitiveIterator[Symbol.iterator] = function () {
  return 'abc';
};
var undefinedIterator = {};
undefinedIterator[Symbol.iterator] = function () {};
print(thrownClass(() => GetIterator(primitiveIterator)), thrownClass(() => GetIterator(undefinedIterator)));
// CHECK-NEXT: TypeError TypeError
// The method is called with the iterable as this and no arguments.
var self = {};
self[Symbol.iterator] = function () {
  print(this === self, arguments.length);
  return {next: 0};
};
var record = GetIterator(self);
// CHECK-NEXT: true 0
print(Object.getPrototypeOf(record), record.nextMethod);
// CHECK-NEXT: null 0

// next is read once, by GetIterator, even if the iterator replaces it. The
// next method is called with the iterator as this and no arguments.
var log = [];
var replacing = {};
replacing[Symbol.iterator] = function () {
  var n = 0;
  var iterator = {
    next() {
      log.push(this === iterator ? 'next' : 'wrong this', arguments.length);
      iterator.next = function () {
        log.push('replacement');
        return {done: true};
      };
      return {done: ++n > 3, value: n};
    },
  };
  return iterator;
};
print(JSON.stringify(drain(replacing)), log.join());
// CHECK-NEXT: [1,2,3] next,0,next,0,next,0,next,0

// IteratorStepValue reads done, then value, and value only when done is
// false. done goes through ToBoolean.
log = [];
var stepper = {};
stepper[Symbol.iterator] = function () {
  var n = 0;
  return {
    next() {
      n++;
      return {
        get done() {
          log.push('done' + n);
          return n === 3 ? 'yes' : 0;
        },
        get value() {
          log.push('value' + n);
          return n;
        },
      };
    },
  };
};
print(JSON.stringify(drain(stepper)), log.join());
// CHECK-NEXT: [1,2] done1,value1,done2,value2,done3

// A next that is not callable, a result that is not an Object, and a
// throwing next, done getter or value getter all propagate from
// IteratorStepValue without calling return.
log = [];
print(thrownClass(() => IteratorStepValue(GetIterator(logged(log, [5], undefined)))), log.join());
// CHECK-NEXT: TypeError @@iterator,get next,next
var nextless = {};
nextless[Symbol.iterator] = function () {
  return {next: 1, return() { log.push('return'); }};
};
log = [];
print(thrownClass(() => IteratorStepValue(GetIterator(nextless))), log.join());
// CHECK-NEXT: TypeError
var boom = {};
boom[Symbol.iterator] = function () {
  return {
    next() {
      throw new RangeError('from next');
    },
    return() {
      log.push('return');
    },
  };
};
log = [];
print(thrown(() => IteratorStepValue(GetIterator(boom))), log.join());
// CHECK-NEXT: RangeError: from next
var doneThrows = [{get done() { throw new RangeError('from done'); }}];
log = [];
print(thrown(() => IteratorStepValue(GetIterator(logged(log, doneThrows, undefined)))), log.join());
// CHECK-NEXT: RangeError: from done @@iterator,get next,next
var valueThrows = [{done: false, get value() { throw new RangeError('from value'); }}];
log = [];
print(thrown(() => IteratorStepValue(GetIterator(logged(log, valueThrows, undefined)))), log.join());
// CHECK-NEXT: RangeError: from value @@iterator,get next,next

// IteratorCloseOnThrow calls return once, with the iterator as this and no
// arguments, ignores its result, and throws the given error.
var error = new SyntaxError('the given error');
log = [];
record = GetIterator(logged(log, [], 'not an object'));
print(thrown(() => IteratorCloseOnThrow(record, error)), log.join());
// CHECK-NEXT: SyntaxError: the given error @@iterator,get next,return true 0
log = [];
record = GetIterator(logged(log, [], {}));
try {
  IteratorCloseOnThrow(record, error);
} catch (e) {
  print(e === error, log.join());
}
// CHECK-NEXT: true @@iterator,get next,return true 0
// A throwing return, a throwing getter for return, and a return that is
// not callable are ignored; undefined and null are not called.
function withReturn(descriptor) {
  var iterator = {next() {}};
  Object.defineProperty(iterator, 'return', descriptor);
  var iterable = {};
  iterable[Symbol.iterator] = function () {
    return iterator;
  };
  return GetIterator(iterable);
}
print(
  thrown(() => IteratorCloseOnThrow(withReturn({value() { throw new RangeError('x'); }}), error)),
  thrown(() => IteratorCloseOnThrow(withReturn({get() { throw new RangeError('x'); }}), error)),
  thrown(() => IteratorCloseOnThrow(withReturn({value: 5}), error)),
  thrown(() => IteratorCloseOnThrow(withReturn({value: undefined}), error)),
  thrown(() => IteratorCloseOnThrow(withReturn({value: null}), error)),
);
// CHECK-NEXT: SyntaxError: the given error SyntaxError: the given error SyntaxError: the given error SyntaxError: the given error SyntaxError: the given error
// The given error may be any value.
print(thrown(() => IteratorCloseOnThrow(withReturn({value: undefined}), new TypeError('t'))));
// CHECK-NEXT: TypeError: t
