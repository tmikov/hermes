/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The iterator protocol of ECMA-262 7.4, "Operations on Iterator Objects",
// for the operations of the ECMA-402 draft https://tc39.es/ecma402/ dated
// October 24, 2026 that read iterables (StringListFromIterable). Section
// numbers are from the https://tc39.es/ecma262/ draft of the same date. The
// protocol is written out here because for-of consults user-modifiable
// state (README rule 2); every Get and Call on the iterable, the iterator
// and its results is one the spec performs.

/// The value IteratorStepValue returns for ~done~: a plugin-private object,
/// so no iterator can yield it.
var ITERATOR_DONE = {__proto__: null};

// ECMA-262 7.4.4 GetIterator ( obj, kind ), for kind sync. Returns the
// Iterator Record (7.4.1) as the null-prototype record { iterator,
// nextMethod }. [[Done]] is not kept: the plugin closes an iterator only
// through IteratorCloseOnThrow, which does not read it.
function GetIterator(obj) {
  // 1. If kind is async, then ... (not used)
  // 2. Else,
  //    a. Let method be ? GetMethod(obj, %Symbol.iterator%).
  var method = GetMethod(obj, SymbolIterator);
  // 3. If method is undefined, throw a TypeError exception.
  if (method === undefined) throw new _TypeError('The value is not iterable');
  // 4. Return ? GetIteratorFromMethod(obj, method).
  //    7.4.3 GetIteratorFromMethod ( obj, method ):
  //    1. Let iterator be ? Call(method, obj).
  var iterator = ReflectApply(method, obj, []);
  //    2. If iterator is not an Object, throw a TypeError exception.
  if (!isObject(iterator))
    throw new _TypeError('The iterator is not an object');
  //    3. Return ? GetIteratorDirect(iterator).
  //       7.4.2 GetIteratorDirect ( obj ):
  //       1. Let nextMethod be ? Get(obj, "next").
  //       2. Let iteratorRecord be the Iterator Record { [[Iterator]]: obj,
  //          [[NextMethod]]: nextMethod, [[Done]]: false }.
  //       3. Return iteratorRecord.
  return {__proto__: null, iterator: iterator, nextMethod: iterator.next};
}

// ECMA-262 7.4.10 IteratorStepValue ( iteratorRecord ). Returns the value,
// or ITERATOR_DONE for ~done~. None of its errors closes the iterator.
function IteratorStepValue(iteratorRecord) {
  // 1. Let result be ? IteratorStep(iteratorRecord).
  //    7.4.9 IteratorStep ( iteratorRecord ):
  //    1. Let result be ? IteratorNext(iteratorRecord).
  //       7.4.6 IteratorNext ( iteratorRecord [ , value ] ):
  //       1. If value is not present, then
  //          a. Let result be Completion(Call(iteratorRecord.[[NextMethod]],
  //             iteratorRecord.[[Iterator]])).
  //       2. Else, ... (not used)
  //       3. If result is a throw completion, then set
  //          iteratorRecord.[[Done]] to true and return ? result.
  //       4. Set result to ! result.
  var result = ReflectApply(
    iteratorRecord.nextMethod,
    iteratorRecord.iterator,
    [],
  );
  //       5. If result is not an Object, then set iteratorRecord.[[Done]]
  //          to true and throw a TypeError exception.
  if (!isObject(result))
    throw new _TypeError('The iterator result is not an object');
  //       6. Return result.
  //    2. Let done be Completion(IteratorComplete(result)).
  //       7.4.7 IteratorComplete ( iteratorResult ):
  //       1. Return ToBoolean(? Get(iteratorResult, "done")).
  //    3. If done is a throw completion, then set iteratorRecord.[[Done]]
  //       to true and return ? done.
  //    4. Set done to ! done.
  var done = !!result.done;
  //    5. If done is true, then
  //       a. Set iteratorRecord.[[Done]] to true.
  //       b. Return ~done~.
  if (done) return ITERATOR_DONE;
  //    6. Return result.
  // 2. If result is ~done~, then
  //    a. Return ~done~.
  // 3. Let value be Completion(IteratorValue(result)).
  //    7.4.8 IteratorValue ( iteratorResult ):
  //    1. Return ? Get(iteratorResult, "value").
  // 4. If value is a throw completion, then
  //    a. Set iteratorRecord.[[Done]] to true.
  // 5. Return ? value.
  return result.value;
}

// ECMA-262 7.4.11 IteratorClose ( iteratorRecord, completion ), for a
// completion that is the throw completion of error: any error from reading
// or calling return, and its result, are ignored, and error is thrown.
function IteratorCloseOnThrow(iteratorRecord, error) {
  // 1. Assert: iteratorRecord.[[Iterator]] is an Object.
  // 2. Let iterator be iteratorRecord.[[Iterator]].
  var iterator = iteratorRecord.iterator;
  try {
    // 3. Let innerResult be Completion(GetMethod(iterator, "return")).
    var returnMethod = GetMethod(iterator, 'return');
    // 4. If innerResult is a normal completion, then
    //    a. Let return be innerResult.[[Value]].
    //    b. If return is undefined, return ? completion.
    //    c. Set innerResult to Completion(Call(return, iterator)).
    if (returnMethod !== undefined) ReflectApply(returnMethod, iterator, []);
  } catch (e) {
    // innerResult is a throw completion, which step 5 drops.
  }
  // 5. If completion is a throw completion, return ? completion.
  throw error;
}
