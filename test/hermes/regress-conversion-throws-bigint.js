/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -O0 %s | %FileCheck --match-full-lines %s
// RUN: %hermes -O %s | %FileCheck --match-full-lines %s

// The type conversion instructions are side-effect free only when the operand
// cannot be a Symbol or (for the ToNumber family) a BigInt. Marking them pure
// unconditionally for primitive operands let DCE delete a conversion whose
// result was unused, silently dropping a mandatory TypeError. Both RUN lines
// must produce identical output: optimization must not change semantics.

function tryIt(name, f) {
  try {
    f();
    print(name + ": no throw");
  } catch (e) {
    print(name + ": " + e.name);
  }
}

// --- ToNumber family: must throw for a BigInt, even when the result is dead.

// Exact :bigint operand, result discarded -> eligible for DCE.
tryIt("ToNumber exact", function () {
  var b = true ? 1n : 2n;
  +b;
});
// CHECK: ToNumber exact: TypeError

tryIt("ToInt32 exact", function () {
  var b = true ? 1n : 2n;
  b | 0;
});
// CHECK-NEXT: ToInt32 exact: TypeError

tryIt("ToUint32 exact", function () {
  var b = true ? 1n : 2n;
  b >>> 0;
});
// CHECK-NEXT: ToUint32 exact: TypeError

// Union number|bigint is also "primitive"; it must still throw on the BigInt
// value while remaining correct for the Number one.
tryIt("ToNumber union number", function () {
  var b = false ? 1n : 2;
  +b;
});
// CHECK-NEXT: ToNumber union number: no throw

tryIt("ToNumber union bigint", function () {
  var b = true ? 1n : 2;
  +b;
});
// CHECK-NEXT: ToNumber union bigint: TypeError

// --- Conversions that are legal on a BigInt must NOT be pessimized.

// ToString(BigInt) is fine, so AddEmptyStringInst stays side-effect free.
tryIt("ToString bigint", function () {
  var b = true ? 1n : 2n;
  b + "";
});
// CHECK-NEXT: ToString bigint: no throw

// ToNumeric(BigInt) is a BigInt, so AsNumericInst stays side-effect free.
tryIt("ToNumeric bigint", function () {
  var b = true ? 1n : 2n;
  b++;
});
// CHECK-NEXT: ToNumeric bigint: no throw

// --- A conversion whose result is used was never deleted; guard against
// regressing the ordinary path.
tryIt("ToNumber used", function () {
  var b = true ? 1n : 2n;
  return +b;
});
// CHECK-NEXT: ToNumber used: TypeError

// Safe primitive operands must keep converting without throwing.
tryIt("ToNumber number", function () {
  var n = true ? 1 : 2;
  +n;
});
// CHECK-NEXT: ToNumber number: no throw

tryIt("ToNumber string", function () {
  var s = true ? "1" : "2";
  +s;
});
// CHECK-NEXT: ToNumber string: no throw

print("done");
// CHECK-NEXT: done
