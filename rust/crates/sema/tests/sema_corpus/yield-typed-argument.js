// FLAGS: -parse-flow
// Pins `parse_yield_expression`'s argument parse (JSParserImpl.cpp:4674-4678):
// C++ passes `CoverTypedParameters::No`, overriding the ambient cover
// grammar for the yield argument. The port previously passed `::Yes`, so
// `(yield a: number)` parsed the argument as a `CoverTypedIdentifier` nested
// inside the `YieldExpression` instead of letting the parenthesized-
// expression parser rewrite the whole thing into a `TypeCastExpression`
// wrapping a plain `YieldExpression` (JSParserImpl.cpp:2633-2640) — the
// shape hermesc actually produces.
function* f() {
  (yield a: number);
}
