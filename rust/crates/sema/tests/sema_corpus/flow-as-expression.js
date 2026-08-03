// FLAGS: -parse-flow
// `visit(AsExpressionNode *)` (SemanticResolver.cpp:1596-1599, `#if
// HERMES_PARSE_FLOW`, `resolver/expressions.rs`'s `visit_as_expression`,
// added by S4a T4's fix review) — the same "visit the expression, but not
// the type annotation" shape as `flow-typecast-resolves.js`'s
// `TypeCastExpressionNode`, for Flow's `as` operator. The parser builds an
// `AsExpressionNode` whenever `getParseFlow()` is set
// (JSParserImpl.cpp:4329-4350), independent of `typed_`, so `x as number`
// reaches this visit under plain untyped `-parse-flow` too — no `-typed`.
// `x` is declared first so the dump shows it resolving normally.
// (`x as const`, the sibling `AsConstExpressionNode` shape, has no C++
// `visit()` override at all and is left to whichever later corpus file
// needs it — see `visit_as_expression`'s doc comment.)
var x = 1;
x as number;
