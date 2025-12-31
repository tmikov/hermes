/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermesc -transform-jsx -dump-transformed-ast -pretty-json %s | %FileCheck %s

// Simple element with text child
<div>Hello</div>;
// CHECK: "type": "CallExpression",
// CHECK: "type": "MemberExpression",
// CHECK: "name": "JSX"
// CHECK: "name": "jsx"

// Element with attributes
<div className="foo" id="bar">Content</div>;
// CHECK: "type": "CallExpression",
// CHECK: "type": "MemberExpression",
// CHECK: "name": "JSX"
// CHECK: "name": "jsx"

// Multiple children uses jsxs
<div><span>A</span><span>B</span></div>;
// CHECK: "type": "CallExpression",
// CHECK: "type": "MemberExpression",
// CHECK: "name": "JSX"
// CHECK: "name": "jsxs"
