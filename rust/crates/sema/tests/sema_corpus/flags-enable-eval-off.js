// FLAGS: -enable-eval=false
// Pins the `EvalDisabled` branch of `visit(CallExpressionNode *)`
// (SemanticResolver.cpp:1157, `resolver/calls.rs:232`'s `else if is_eval`
// arm): with eval support turned off, a direct call to the global `eval`
// still resolves the identifier but warns "eval() is disabled at runtime"
// instead of registering a local eval / warning about lexical scope. The
// ENABLED branch is already pinned by `disabled-eval.js` (S2 T6); this file
// is what its Deferred-turned-imported note calls out as unit-tested only
// until the harness grew per-file flags — see MANIFEST.md.
eval("1");
