// FLAGS: -parse-flow
// Capstone fix (finding F1) — `visit(TypeParameterInstantiationNode *)`
// (SemanticResolver.cpp:1587-1589, `#if HERMES_PARSE_FLOW`), a do-nothing
// visit like `visit(TypeAliasNode *)` next to it.
//
// The node is a call's/`new`'s type-ARGUMENT list, and it is reachable under
// plain untyped `-parse-flow` (no `-typed`) through three parents, all three
// of which reach it via a children walk rather than a hand-driven visit:
// `CallExpression` (`calls.rs`), `NewExpression` and
// `OptionalCallExpression` (both on `mod.rs`'s override-free generic arm).
// Before the fix all three panicked at `visit_node`'s catch-all while
// hermesc exited 0 with a full dump.
//
// Because the visit is a TRUE no-op (it does not call
// `visitESTreeChildren`), the identifiers inside the type arguments are
// never resolved: `number` here is a `GenericTypeAnnotation`'s `_id` and
// gets NO `[D:E:...]` annotation in the dump, exactly like
// `type-alias-children.js`'s `_right`. The callees `f`/`C` around them DO
// resolve, which is what makes this a real test rather than a vacuous one.
//
// The sibling `visit(TypeParameterDeclarationNode *)` (cpp:1583-1585) is not
// pinned here and cannot be: the function and class visits hand-drive their
// children and never dispatch a type-parameter DECLARATION list — see
// `mod.rs`'s arm, which carries it anyway because C++ does.
function f(x) { return x; }
function C() {}

f<number>(1);
new C<number>();
f?.<number>(1);
