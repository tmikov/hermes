// FLAGS: -parse-flow
// Capstone fix (finding F1) — `visit(TypeParameterInstantiationNode *)`
// (SemanticResolver.cpp:1601-1603, `#if HERMES_PARSE_FLOW`), a do-nothing
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
// never resolved. `Foo`/`Bar`/`Baz` are `GenericTypeAnnotation`'s `_id`s and
// get NO `[D:E:...]` annotation in the dump, exactly like
// `type-alias-children.js`'s `_right`. The callees `f`/`C` around them DO
// resolve, which is what makes this a real test rather than a vacuous one.
//
// The shape is `GenericTypeAnnotation`, not `number`/`NumberTypeAnnotation`:
// a `NumberTypeAnnotation` is childless either way, so `f<number>(1)` cannot
// tell a walked argument from an unwalked one — `Foo` et al. can, because an
// unwalked `Id 'Foo'` prints with no `[D:E:...]` while a walked one (as an
// `UndeclaredGlobalProperty`) would carry one.
//
// The sibling `visit(TypeParameterDeclarationNode *)` (cpp:1597-1599) is a
// different node (the type-parameter DECLARATION list, not this
// instantiation list) and is pinned separately, in
// `flow-interface-enum.js`.
function f(x) { return x; }
function C() {}

f<Foo>(1);
new C<Bar>();
f?.<Baz>(1);
