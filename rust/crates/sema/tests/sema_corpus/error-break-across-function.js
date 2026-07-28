/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// `labelMap`, `currentLoop` and `currentLoopOrSwitch` all live on the
// FunctionContext, so a nested function sees none of the enclosing loops or
// labels. (`test/Sema/break-in-nested-func.js` tests the same thing with a
// block-scoped function *declaration*, which needs the S3 promoter; a
// function expression reaches the same code path today.)

lab: while (cond) {
  var f = function () {
    break;
  };
  var g = function () {
    continue;
  };
  var h = function () {
    break lab;
  };
  var i = function () {
    continue lab;
  };
}
