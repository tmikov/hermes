/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -transform-jsx %s | %FileCheck %s
// RUN: %hermes -transform-jsx -jsx-runtime=createElement -jsx-global=React %s | %FileCheck %s --check-prefix=CHECK-CE

// Mock JSX runtime for testing
var JSX = {
  Fragment: Symbol('Fragment'),
  jsx: function(type, props, key) {
    return {
      $$typeof: 'element',
      type: type,
      props: props,
      key: key === undefined ? null : key
    };
  },
  jsxs: function(type, props, key) {
    return {
      $$typeof: 'element',
      type: type,
      props: props,
      key: key === undefined ? null : key,
      static: true
    };
  }
};

// Mock React runtime for createElement mode
var React = {
  Fragment: Symbol('React.Fragment'),
  createElement: function(type, props) {
    var children = [];
    for (var i = 2; i < arguments.length; i++) {
      children.push(arguments[i]);
    }
    return {
      $$typeof: 'element',
      type: type,
      props: props || {},
      children: children.length === 0 ? null :
                children.length === 1 ? children[0] : children
    };
  }
};

// Test simple element
var simple = <div className="test">Hello</div>;
print("simple type:", simple.type);
print("simple className:", simple.props.className);
print("simple children:", simple.props.children || simple.children);
// CHECK: simple type: div
// CHECK: simple className: test
// CHECK: simple children: Hello
// CHECK-CE: simple type: div
// CHECK-CE: simple className: test
// CHECK-CE: simple children: Hello

// Test component (uppercase)
function MyComponent(props) { return props; }
var comp = <MyComponent value={42} />;
print("comp type is function:", typeof comp.type === 'function');
print("comp value:", comp.props.value);
// CHECK: comp type is function: true
// CHECK: comp value: 42
// CHECK-CE: comp type is function: true
// CHECK-CE: comp value: 42

// Test multiple children (uses jsxs in jsx mode)
var multi = <div><span>A</span><span>B</span></div>;
print("multi type:", multi.type);
print("multi has children array:", Array.isArray(multi.props.children) || Array.isArray(multi.children));
// CHECK: multi type: div
// CHECK: multi has children array: true
// CHECK-CE: multi type: div
// CHECK-CE: multi has children array: true

// Test key extraction (jsx mode extracts key as third arg, createElement keeps in props)
var keyed = <div key="mykey">content</div>;
print("keyed key:", keyed.key);
print("keyed props.key:", keyed.props.key);
// CHECK: keyed key: mykey
// CHECK: keyed props.key: undefined
// CHECK-CE: keyed key: undefined
// CHECK-CE: keyed props.key: mykey

// Test fragment
var frag = <><span>A</span><span>B</span></>;
print("fragment type is symbol:", typeof frag.type === 'symbol');
// CHECK: fragment type is symbol: true
// CHECK-CE: fragment type is symbol: true

// Test boolean attribute (no value = true)
var boolAttr = <input disabled />;
print("disabled:", boolAttr.props.disabled);
// CHECK: disabled: true
// CHECK-CE: disabled: true

// Test spread attributes
var spreadProps = { a: 1, b: 2 };
var spread = <div {...spreadProps} c={3} />;
print("spread a:", spread.props.a);
print("spread b:", spread.props.b);
print("spread c:", spread.props.c);
// CHECK: spread a: 1
// CHECK: spread b: 2
// CHECK: spread c: 3
// CHECK-CE: spread a: 1
// CHECK-CE: spread b: 2
// CHECK-CE: spread c: 3

// Test key with spread - key should be extracted in jsx mode
var keySpreadProps = { a: 1 };
var keySpread = <div {...keySpreadProps} key="extracted" b={2} />;
print("keySpread key:", keySpread.key);
print("keySpread props.key:", keySpread.props.key);
print("keySpread a:", keySpread.props.a);
print("keySpread b:", keySpread.props.b);
// CHECK: keySpread key: extracted
// CHECK: keySpread props.key: undefined
// CHECK: keySpread a: 1
// CHECK: keySpread b: 2
// CHECK-CE: keySpread key: undefined
// CHECK-CE: keySpread props.key: extracted
// CHECK-CE: keySpread a: 1
// CHECK-CE: keySpread b: 2

// ============================================
// Whitespace normalization tests
// ============================================

// Test: leading/trailing whitespace should be trimmed
var trimmed = <div>  Hello  </div>;
var trimmedText = trimmed.props.children || trimmed.children;
print("trimmed:", "[" + trimmedText + "]");
// CHECK: trimmed: [Hello]
// CHECK-CE: trimmed: [Hello]

// Test: multiline text with indentation should collapse to single spaces
var multiline = <div>
  Hello
  World
</div>;
var multilineText = multiline.props.children || multiline.children;
print("multiline:", "[" + multilineText + "]");
// CHECK: multiline: [Hello World]
// CHECK-CE: multiline: [Hello World]

// Test: whitespace-only text should produce no children
var whitespaceOnly = <div>   </div>;
var whitespaceChildren = whitespaceOnly.props.children || whitespaceOnly.children;
print("whitespaceOnly children:", whitespaceChildren);
// CHECK: whitespaceOnly children: undefined
// CHECK-CE: whitespaceOnly children: null

// Test: mixed content preserves text between elements
var mixed = <div>A<span>B</span>C</div>;
var mixedChildren = mixed.props.children || mixed.children;
print("mixed is array:", Array.isArray(mixedChildren));
print("mixed[0]:", mixedChildren[0]);
print("mixed[2]:", mixedChildren[2]);
// CHECK: mixed is array: true
// CHECK: mixed[0]: A
// CHECK: mixed[2]: C
// CHECK-CE: mixed is array: true
// CHECK-CE: mixed[0]: A
// CHECK-CE: mixed[2]: C

print("All tests passed!");
// CHECK: All tests passed!
// CHECK-CE: All tests passed!
