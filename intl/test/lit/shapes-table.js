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
  // [holder, property, 'constructor', name, length]
  // [holder, property, 'method', name, length]
  // [holder, property, 'getter', name], whose length is 0
  // [holder, '@@toStringTag', 'toStringTag', value]
  ['Intl', 'getCanonicalLocales', 'method', 'getCanonicalLocales', 1],
  ['Intl', 'supportedValuesOf', 'method', 'supportedValuesOf', 1],
  ['Intl', 'Locale', 'constructor', 'Locale', 1],
  ['Intl', '@@toStringTag', 'toStringTag', 'Intl'],
  ['Intl.Locale.prototype', 'baseName', 'getter', 'get baseName'],
  ['Intl.Locale.prototype', 'calendar', 'getter', 'get calendar'],
  ['Intl.Locale.prototype', 'caseFirst', 'getter', 'get caseFirst'],
  ['Intl.Locale.prototype', 'collation', 'getter', 'get collation'],
  ['Intl.Locale.prototype', 'firstDayOfWeek', 'getter', 'get firstDayOfWeek'],
  ['Intl.Locale.prototype', 'hourCycle', 'getter', 'get hourCycle'],
  ['Intl.Locale.prototype', 'language', 'getter', 'get language'],
  ['Intl.Locale.prototype', 'numberingSystem', 'getter', 'get numberingSystem'],
  ['Intl.Locale.prototype', 'numeric', 'getter', 'get numeric'],
  ['Intl.Locale.prototype', 'region', 'getter', 'get region'],
  ['Intl.Locale.prototype', 'script', 'getter', 'get script'],
  ['Intl.Locale.prototype', 'variants', 'getter', 'get variants'],
  ['Intl.Locale.prototype', 'maximize', 'method', 'maximize', 0],
  ['Intl.Locale.prototype', 'minimize', 'method', 'minimize', 0],
  ['Intl.Locale.prototype', 'toString', 'method', 'toString', 0],
  ['Intl.Locale.prototype', '@@toStringTag', 'toStringTag', 'Intl.Locale'],
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
  } else if (row[2] === 'constructor') {
    if (!d.writable || d.enumerable || !d.configurable) problems.push('holder attributes');
    f = d.value;
    if (typeof f !== 'function') return label + 'not a function';
    if (!isConstructor(f)) problems.push('not constructible');
    // The prototype is non-writable, non-enumerable and non-configurable,
    // and its constructor is the function.
    var pd = Object.getOwnPropertyDescriptor(f, 'prototype');
    if (!pd || pd.writable || pd.enumerable || pd.configurable) problems.push('prototype attributes');
    var cd = pd && Object.getOwnPropertyDescriptor(pd.value, 'constructor');
    if (!cd || cd.value !== f) problems.push('prototype.constructor');
  } else if (row[2] === 'getter') {
    // An accessor with no setter: non-enumerable, configurable.
    if (!('get' in d)) return label + 'not an accessor';
    if (d.set !== undefined || d.enumerable || !d.configurable) problems.push('holder attributes');
    f = d.get;
    if (typeof f !== 'function') return label + 'no getter';
    if (isConstructor(f)) problems.push('constructible');
  } else {
    return label + 'unknown kind ' + row[2];
  }
  checkFunctionProperty(f, 'name', row[3], problems);
  // A getter's length is 0.
  checkFunctionProperty(f, 'length', row[2] === 'getter' ? 0 : row[4], problems);
  if (Function.prototype.toString.call(f).indexOf('[native code]') === -1)
    problems.push('toString');
  return label + (problems.length ? problems.join(', ') : 'ok');
}

for (var i = 0; i < TABLE.length; i++) print(check(TABLE[i]));
// CHECK: Intl.getCanonicalLocales: ok
// CHECK-NEXT: Intl.supportedValuesOf: ok
// CHECK-NEXT: Intl.Locale: ok
// CHECK-NEXT: Intl.@@toStringTag: ok
// CHECK-NEXT: Intl.Locale.prototype.baseName: ok
// CHECK-NEXT: Intl.Locale.prototype.calendar: ok
// CHECK-NEXT: Intl.Locale.prototype.caseFirst: ok
// CHECK-NEXT: Intl.Locale.prototype.collation: ok
// CHECK-NEXT: Intl.Locale.prototype.firstDayOfWeek: ok
// CHECK-NEXT: Intl.Locale.prototype.hourCycle: ok
// CHECK-NEXT: Intl.Locale.prototype.language: ok
// CHECK-NEXT: Intl.Locale.prototype.numberingSystem: ok
// CHECK-NEXT: Intl.Locale.prototype.numeric: ok
// CHECK-NEXT: Intl.Locale.prototype.region: ok
// CHECK-NEXT: Intl.Locale.prototype.script: ok
// CHECK-NEXT: Intl.Locale.prototype.variants: ok
// CHECK-NEXT: Intl.Locale.prototype.maximize: ok
// CHECK-NEXT: Intl.Locale.prototype.minimize: ok
// CHECK-NEXT: Intl.Locale.prototype.toString: ok
// CHECK-NEXT: Intl.Locale.prototype.@@toStringTag: ok

// Intl and each prototype have exactly the table's properties, and a
// prototype also its constructor.
print(Reflect.ownKeys(Intl).map(String).join(','));
// CHECK-NEXT: getCanonicalLocales,supportedValuesOf,Locale,Symbol(Symbol.toStringTag)
print(Reflect.ownKeys(Intl.Locale.prototype).map(String).join(','));
// CHECK-NEXT: constructor,baseName,calendar,caseFirst,collation,firstDayOfWeek,hourCycle,language,numberingSystem,numeric,region,script,variants,maximize,minimize,toString,Symbol(Symbol.toStringTag)
