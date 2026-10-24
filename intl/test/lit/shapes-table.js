/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes %s | %FileCheck --match-full-lines %s

// Every function and @@toStringTag the plugin installs, checked against the
// spec's function-shape table. Each phase adds its rows here, and a new kind
// of row gets its check below.

var TABLE = [
  // [holder, property, 'method', name, length]
  // [holder, '@@toStringTag', 'toStringTag', value]
  ['Intl', 'getCanonicalLocales', 'method', 'getCanonicalLocales', 1],
  ['Intl', 'supportedValuesOf', 'method', 'supportedValuesOf', 1],
  ['Intl', '@@toStringTag', 'toStringTag', 'Intl'],
];

function isConstructor(f) {
  try {
    Reflect.construct(String, [], f);
    return true;
  } catch (e) {
    return false;
  }
}

/// Add key to problems unless f has an own property key with the expected
/// value and the attributes ECMA-262 18 gives the "name" and "length" of
/// built-in functions (non-writable, non-enumerable, configurable).
function checkFunctionProperty(f, key, expected, problems) {
  var d = Object.getOwnPropertyDescriptor(f, key);
  if (!d || d.value !== expected || d.writable || d.enumerable || !d.configurable)
    problems.push(key);
}

/// "<holder>.<property>: ok", or the list of what is wrong with that row.
function check(row) {
  var label = row[0] + '.' + row[1] + ': ';
  var holder = (0, eval)(row[0]);
  var key = row[1] === '@@toStringTag' ? Symbol.toStringTag : row[1];
  var d = Object.getOwnPropertyDescriptor(holder, key);
  if (!d) return label + 'missing';
  var problems = [];
  var f;
  if (row[2] === 'toStringTag') {
    // A data property: non-writable, non-enumerable, configurable (ECMA-402
    // 8.1.1 for Intl, and the same for every Intl prototype).
    if (d.value !== row[3]) problems.push('value');
    if (d.writable || d.enumerable || !d.configurable) problems.push('attributes');
    return label + (problems.length ? problems.join(', ') : 'ok');
  } else if (row[2] === 'method') {
    if (!d.writable || d.enumerable || !d.configurable) problems.push('holder attributes');
    f = d.value;
    if (typeof f !== 'function') return label + 'not a function';
    if (isConstructor(f)) problems.push('constructible');
    if (Object.prototype.hasOwnProperty.call(f, 'prototype')) problems.push('prototype');
  } else {
    return label + 'unknown kind ' + row[2];
  }
  checkFunctionProperty(f, 'name', row[3], problems);
  checkFunctionProperty(f, 'length', row[4], problems);
  if (Function.prototype.toString.call(f).indexOf('[native code]') === -1)
    problems.push('toString');
  return label + (problems.length ? problems.join(', ') : 'ok');
}

for (var i = 0; i < TABLE.length; i++) print(check(TABLE[i]));
// CHECK: Intl.getCanonicalLocales: ok
// CHECK-NEXT: Intl.supportedValuesOf: ok
// CHECK-NEXT: Intl.@@toStringTag: ok

// Intl has exactly the table's properties.
print(Reflect.ownKeys(Intl).map(String).join(','));
// CHECK-NEXT: getCanonicalLocales,supportedValuesOf,Symbol(Symbol.toStringTag)
