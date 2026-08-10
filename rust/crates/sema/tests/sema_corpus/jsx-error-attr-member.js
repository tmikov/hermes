// FLAGS: -parse-jsx
// Import of `test/Parser/jsx-error-attr-member.js`, the regression test added
// by upstream `37520ccef` ("Fix rejection of member expressions as JSX
// attribute names"). A member expression is not a valid JSX attribute name,
// but it used to be accepted because `parseJSXElementName`'s check tested
// `MemberExpressionNode` instead of `JSXMemberExpressionNode` — and
// `JSXMemberExpression` derives from the JSX base node, so the check could
// never fire. The Rust port mirrored that dead check and the fix alike
// (`parser/src/js/jsx.rs`).
//
// This is a parse-error file: the parse fails before sema runs, so all three
// channels pin the PARSER diagnostic (empty stdout, the rendered error plus
// the driver epilogue on stderr, exit 2). The unit-level twin is
// `parser/tests/upstream_defect_fixes.rs`.
<foo a.b="1"></foo>
