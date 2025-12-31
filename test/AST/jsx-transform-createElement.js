/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermesc -transform-jsx -jsx-runtime=createElement -dump-transformed-ast -pretty-json %s | %FileCheck %s

// Simple element with text child
<div>Hello</div>;
// CHECK: "type": "CallExpression",
// CHECK: "type": "MemberExpression",
// CHECK: "name": "React"
// CHECK: "name": "createElement"

// Element with attributes
<div className="foo" id="bar">Content</div>;
// CHECK: "type": "CallExpression",
// CHECK: "type": "MemberExpression",
// CHECK: "name": "React"
// CHECK: "name": "createElement"

// Fragment - verify createElement is called with React.Fragment as first arg
<><div>A</div><div>B</div></>;
// CHECK: "type": "CallExpression",
// CHECK: "name": "React"
// CHECK: "name": "Fragment"
