// FLAGS: -parse-flow
// Capstone fix (finding F1) — the Flow STATEMENT kinds that have no
// `visit()` override at all, half two: `OpaqueType` and the `Declare*`
// family (`DeclareVariable`, `DeclareFunction`).
//
// Same argument as `flow-interface-enum.js`: none of the three is in the
// header's `visit` inventory (SemanticResolver.h:191-305), so C++ walks
// their children through the default `visit(ESTree::Node *)` at :191-193,
// and their `_id`s resolve as ordinary `UndeclaredGlobalProperty`
// identifiers in the dump.
//
// `opaque type B = string;` is deliberately next to `type A = string;`:
// they are near-identical syntax with OPPOSITE dispatch — `TypeAlias` has a
// do-nothing visit (cpp:1579-1581) so `A` gets no `[D:E:...]` annotation,
// while `OpaqueType` has none so `B` does. A port that lumped the two
// together would show up right here.
//
// The `declare` statements pin the family's two commonest members; the rest
// of the `Declare*` universe (`DeclareClass`, `DeclareInterface`,
// `DeclareModule`, `DeclareExportDeclaration`, …) is served by the same
// single arm in `mod.rs`, which is written as the AST's `Flow` NodeKind
// range for exactly that reason.
//
// Before the fix all three panicked at `visit_node`'s catch-all while
// hermesc exited 0 with a full dump.
type A = string;
opaque type B = string;

declare var dv: number;
declare function df(): void;

var v = 1;
