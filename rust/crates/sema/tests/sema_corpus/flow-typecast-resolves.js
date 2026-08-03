// FLAGS: -parse-flow
// `visit(TypeCastExpressionNode *)` (SemanticResolver.cpp:1591-1594, `#if
// HERMES_PARSE_FLOW`, `resolver/expressions.rs`'s
// `visit_type_cast_expression`, added by S4a T4's fix review). Reachable
// under plain untyped `-parse-flow` — no `-typed` needed, the C++ site is
// unconditional on `typed_`. `(x: number)` alone (with no prior `x`
// declaration) is exactly `flow-typecast-cover.js`'s NON-optional probe
// shape, which this port must NOT treat as a `CoverTypedIdentifier`: the
// parser already rewrote it to a `TypeCastExpressionNode` before sema ever
// runs (JSParserImpl.cpp:2633-2640), so this file is the direct positive
// counterpart to that one. "Visit the expression, but not the type
// annotation" means the inner `x` resolves normally while the type
// annotation is never walked (nothing inside `number` to resolve anyway,
// but the principle is the same one `ObjectPattern`/`ArrayPattern`'s
// `_typeAnnotation` skip documents). `x` is declared first so the dump
// shows a real resolved `[D:E:...]` decl reference rather than an
// on-the-fly `UndeclaredGlobalProperty`.
var x: number;
(x: number);
