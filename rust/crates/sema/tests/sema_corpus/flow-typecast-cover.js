// FLAGS: -parse-flow
// `visit(CoverTypedIdentifierNode *)` (SemanticResolver.cpp:1575-1577,
// `resolver/expressions.rs:966`, ported unconditionally per the
// single-node-set precedent even though the C++ site is
// `#if HERMES_PARSE_FLOW`). A `CoverTypedIdentifierNode` is the parser's
// cover-grammar node for "this might be an arrow parameter with a type
// annotation" (`tryParseCoverTypedIdentifierNode`, JSParserImpl.cpp:4618);
// it only ever reaches sema when the surrounding `(...)` turns out NOT to be
// arrow parameters.
//
// `(x: number)` alone is NOT this shape: JSParserImpl.cpp:2633-2640
// converts a non-optional cover node with a type annotation into a
// `TypeCastExpressionNode` right there in the parenthesized-expression
// parser, before sema ever sees a cover node. The conversion is gated on
// `cover->_right && !cover->_optional`, so an OPTIONAL cover node
// (`x?: number`, i.e. the `?` is consumed before the `:`) skips that
// rewrite and survives into the resolver as a real `CoverTypedIdentifierNode`
// — verified against hermesc directly (`(x?: number);` errors here; the
// non-optional `(x: number);` instead reaches a *later* stage as a plain
// type cast and does not exercise this visit).
(x?: number);
