/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermesc -transform-jsx -dump-transformed-ast -pretty-json %s | %FileCheck %s

// Spread children
<div>{...items}</div>;
// CHECK: "type": "CallExpression"
// CHECK: "name": "jsx"
// CHECK: "type": "SpreadElement"

// Key with spread attributes - key should be extracted as third argument
<Item {...props} key={id} value={1} />;
// CHECK: "type": "CallExpression"
// CHECK: "name": "jsx"
// The arguments should be: [Item, {props object}, id]
// CHECK: "arguments": [
// CHECK: "type": "Identifier"
// CHECK: "name": "Item"
// CHECK: "type": "ObjectExpression"
// Key is third argument (extracted from props):
// CHECK: "type": "Identifier"
// CHECK: "name": "id"

// Namespaced attribute names (e.g., aria:label, xml:lang)
<div aria:label="test" />;
// CHECK: "type": "CallExpression"
// CHECK: "name": "jsx"

// JSX in function call arguments
fn(<div>inner</div>);
// CHECK: "type": "CallExpression"
// CHECK: "type": "CallExpression"
// CHECK: "name": "jsx"

// Self-closing vs explicit closing should produce same output
<div />;
// CHECK: "type": "CallExpression"
// CHECK: "name": "jsx"
// CHECK: "type": "StringLiteral"
// CHECK: "value": "div"

<div></div>;
// CHECK: "type": "CallExpression"
// CHECK: "name": "jsx"
// CHECK: "type": "StringLiteral"
// CHECK: "value": "div"

// Uppercase component (should be identifier, not string)
<MyComponent />;
// CHECK: "type": "CallExpression"
// CHECK: "name": "jsx"
// CHECK: "type": "Identifier"
// CHECK: "name": "MyComponent"

// Member expression component
<Foo.Bar.Baz />;
// CHECK: "type": "CallExpression"
// CHECK: "name": "jsx"
// CHECK: "type": "MemberExpression"

// Fragment with no children
<></>;
// CHECK: "type": "CallExpression"
// CHECK: "name": "jsx"
// CHECK: "name": "Fragment"

// Attribute with no value (should be true)
<input disabled />;
// CHECK: "type": "CallExpression"
// CHECK: "name": "jsx"
// CHECK: "type": "BooleanLiteral"
// CHECK: "value": true

// Expression container with complex expression
<div onClick={() => foo()} />;
// CHECK: "type": "CallExpression"
// CHECK: "name": "jsx"
// CHECK: "type": "ArrowFunctionExpression"

// Spread attributes
<div {...props} className="extra" />;
// CHECK: "type": "CallExpression"
// CHECK: "name": "jsx"
// CHECK: "type": "SpreadElement"

// Empty expression container (should be skipped)
<div>{}</div>;
// CHECK: "type": "CallExpression"
// CHECK: "name": "jsx"

// Namespaced element name (becomes string)
<ns:tag />;
// CHECK: "type": "CallExpression"
// CHECK: "name": "jsx"
// CHECK: "type": "StringLiteral"
// CHECK: "value": "ns:tag"
