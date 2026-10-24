/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -O0 %s | %FileCheck --match-full-lines %s
// RUN: %hermes -O %s | %FileCheck --match-full-lines %s
// RUN: %hermes -lazy %s | %FileCheck --match-full-lines %s
// RUN: %hermes -O -emit-binary -out %t.hbc %s && %hermes %t.hbc | %FileCheck --match-full-lines %s
// RUN: %shermes -exec %s | %FileCheck --match-full-lines %s

// Getters and setters in object literals are methods (ECMA-262
// MethodDefinition): they are not constructors and have no "prototype".

function isConstructor(f) {
  try {
    Reflect.construct(String, [], f);
    return true;
  } catch (e) {
    return false;
  }
}

function describe(label, f) {
  var threw;
  try {
    new f();
    threw = 'none';
  } catch (e) {
    threw = e.constructor.name;
  }
  print(label, f.name, isConstructor(f), f.hasOwnProperty('prototype'), threw);
}

var key = 'c';
var obj = {
  get a() {
    return 1;
  },
  set a(v) {},
  get [key]() {
    return 2;
  },
  set [key](v) {},
};
var da = Object.getOwnPropertyDescriptor(obj, 'a');
var dc = Object.getOwnPropertyDescriptor(obj, 'c');
describe('getter', da.get);
// CHECK: getter get a false false TypeError
describe('setter', da.set);
// CHECK-NEXT: setter set a false false TypeError
describe('computed getter', dc.get);
// CHECK-NEXT: computed getter get c false false TypeError
describe('computed setter', dc.set);
// CHECK-NEXT: computed setter set c false false TypeError
print(obj.a, obj.c);
// CHECK-NEXT: 1 2
