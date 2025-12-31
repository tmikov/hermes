/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermesc -transform-jsx -jsx-dev -dump-transformed-ast -pretty-json %s | %FileCheck %s --check-prefix=CHECK-JSX
// RUN: %hermesc -transform-jsx -jsx-dev -jsx-runtime=createElement -dump-transformed-ast -pretty-json %s | %FileCheck %s --check-prefix=CHECK-CE

// JSX dev mode uses jsxDEV with source info as arguments
<div>Hello</div>;
// CHECK-JSX: "name": "jsxDEV"
// CHECK-JSX: "fileName"
// CHECK-JSX: "lineNumber"
// CHECK-JSX: "columnNumber"

// createElement dev mode adds __source and __self as props
// CHECK-CE: "name": "createElement"
// CHECK-CE: "__source"
// CHECK-CE: "fileName"
// CHECK-CE: "__self"
