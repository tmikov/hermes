/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// Lists, and the function-shape rules (design spec "Function shapes").
// Everything the plugin installs goes through these helpers. ECMA-402 draft
// of October 24, 2026; ECMA-262 section numbers are from the
// https://tc39.es/ecma262/ draft of the same date.

// ECMA-262 7.3.6 CreateDataPropertyOrThrow ( obj, propertyKey, value ). The
// property is defined, not assigned, so setters on the prototype chain are
// not consulted.
function CreateDataPropertyOrThrow(obj, propertyKey, value) {
  // 1. Let success be ? CreateDataProperty(obj, propertyKey, value), which
  //    is ? obj.[[DefineOwnProperty]](propertyKey, PropertyDescriptor {
  //    [[Value]]: value, [[Writable]]: true, [[Enumerable]]: true,
  //    [[Configurable]]: true }) (7.3.5).
  // 2. If success is false, throw a TypeError exception.
  //    The captured Object.defineProperty performs both steps.
  ObjectDefineProperty(obj, propertyKey, {
    __proto__: null,
    value: value,
    writable: true,
    enumerable: true,
    configurable: true,
  });
  // 3. Return unused.
}

/// Append v to list without consulting Array.prototype: a[i] = v would run
/// a setter for i on Array.prototype.
function listPush(list, v) {
  // 1. Perform ! CreateDataPropertyOrThrow(list, list.length, v).
  CreateDataPropertyOrThrow(list, list.length, v);
}

/// Replace the element at index i of list with v without consulting
/// Array.prototype (rule 3 of intl/doc/Development.md). For an index
/// listPush created, a[i] = v would find the own data property and run no
/// setter, but defining the property keeps the rule by letter and does not
/// depend on that.
function listSet(list, i, v) {
  // 1. Perform ! CreateDataPropertyOrThrow(list, i, v).
  CreateDataPropertyOrThrow(list, i, v);
}

/// Whether list contains v. Lists hold strings, integers or objects, so ===
/// is the same as SameValue here; it compares objects by identity, which is
/// what the cycle check of Array.prototype.toLocaleString needs.
function listContains(list, v) {
  // 1. For each element e of list, if e is v, return true.
  for (var i = 0; i < list.length; i++) {
    if (list[i] === v) return true;
  }
  // 2. Return false.
  return false;
}

// ECMA-262 7.3.17 CreateArrayFromList ( elements )
function CreateArrayFromList(list) {
  // 1. Let array be ! ArrayCreate(0).
  var array = [];
  // 2-3. For each element of elements, perform
  //      ! CreateDataPropertyOrThrow(array, ! ToString(𝔽(n)), element).
  for (var i = 0; i < list.length; i++) listPush(array, list[i]);
  // 4. Return array.
  return array;
}

/// Install the concise methods of source on target: writable,
/// non-enumerable, configurable. Concise methods are not constructors and
/// have no prototype property; their name and length come from the source.
function defineMethods(target, source) {
  // 1. For each own property key of source, do
  var keys = ReflectOwnKeys(source);
  for (var i = 0; i < keys.length; i++) {
    // a. Let d be ! source.[[GetOwnProperty]](key).
    var d = ObjectGetOwnPropertyDescriptor(source, keys[i]);
    // b. Perform ! DefinePropertyOrThrow(target, key, PropertyDescriptor {
    //    [[Value]]: d.[[Value]], [[Writable]]: true, [[Enumerable]]: false,
    //    [[Configurable]]: true }).
    ObjectDefineProperty(target, keys[i], {
      __proto__: null,
      value: d.value,
      writable: true,
      enumerable: false,
      configurable: true,
    });
  }
}

/// Install the getters of source on target: non-enumerable, configurable,
/// with no setter.
function defineGetters(target, source) {
  // 1. For each own property key of source, do
  var keys = ReflectOwnKeys(source);
  for (var i = 0; i < keys.length; i++) {
    // a. Let d be ! source.[[GetOwnProperty]](key).
    var d = ObjectGetOwnPropertyDescriptor(source, keys[i]);
    // b. Perform ! DefinePropertyOrThrow(target, key, PropertyDescriptor {
    //    [[Get]]: d.[[Get]], [[Enumerable]]: false, [[Configurable]]: true }).
    ObjectDefineProperty(target, keys[i], {
      __proto__: null,
      get: d.get,
      enumerable: false,
      configurable: true,
    });
  }
}

/// Shape the plain function fn as a constructor: its length, its prototype
/// (non-writable, non-enumerable, non-configurable), and
/// proto.constructor.
function defineConstructor(fn, length, proto) {
  // 1. Perform ! DefinePropertyOrThrow(fn, "length", PropertyDescriptor {
  //    [[Value]]: length, [[Writable]]: false, [[Enumerable]]: false,
  //    [[Configurable]]: true }), the attributes ECMA-262 18 gives the
  //    "length" of every built-in function.
  ObjectDefineProperty(fn, 'length', {
    __proto__: null,
    value: length,
    writable: false,
    enumerable: false,
    configurable: true,
  });
  // 2. Perform ! DefinePropertyOrThrow(fn, "prototype", PropertyDescriptor {
  //    [[Value]]: proto, [[Writable]]: false, [[Enumerable]]: false,
  //    [[Configurable]]: false }).
  ObjectDefineProperty(fn, 'prototype', {
    __proto__: null,
    value: proto,
    writable: false,
    enumerable: false,
    configurable: false,
  });
  // 3. Perform ! DefinePropertyOrThrow(proto, "constructor",
  //    PropertyDescriptor { [[Value]]: fn, [[Writable]]: true,
  //    [[Enumerable]]: false, [[Configurable]]: true }).
  ObjectDefineProperty(proto, 'constructor', {
    __proto__: null,
    value: fn,
    writable: true,
    enumerable: false,
    configurable: true,
  });
}

/// Define obj[@@toStringTag] = tag: non-writable, non-enumerable,
/// configurable, as ECMA-402 gives Intl (8.1.1) and every Intl prototype.
function setToStringTag(obj, tag) {
  // 1. DefinePropertyOrThrow(obj, @@toStringTag, PropertyDescriptor {
  //    [[Value]]: tag, [[Writable]]: false, [[Enumerable]]: false,
  //    [[Configurable]]: true }).
  ObjectDefineProperty(obj, SymbolToStringTag, {
    __proto__: null,
    value: tag,
    writable: false,
    enumerable: false,
    configurable: true,
  });
}

/// The base of every internal-slots class (design spec "Constructors"): its
/// constructor returns the object passed to it, so a subclass's private
/// fields are added to that object.
class ReturnOverride {
  constructor(o) {
    // 1. Return o, which becomes the subclass constructor's this.
    return o;
  }
}
