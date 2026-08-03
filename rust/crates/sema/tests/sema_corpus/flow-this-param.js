// FLAGS: -parse-flow
// The `declareParams` lambda's `this`-parameter check
// (SemanticResolver.cpp:1767-1771, `resolver/functions.rs:897`), gated on
// `compile_ && !typed_`: under `-parse-flow` the parser accepts a `this`
// parameter (Flow/TS syntax for typing the receiver), but this dialect is
// untyped (`typed_` is always false here), so the resolver rejects it. In
// the plain untyped dialect (no `-parse-flow`) the parser itself rejects
// `this` in a binding position first (`identifier, '{' or '[' expected in
// binding pattern`), which is why this diagnostic needs the flag to be
// reachable at all (see MANIFEST.md's S2 Task 8 note).
function f(this: number) {}
