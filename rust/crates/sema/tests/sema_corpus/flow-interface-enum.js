// FLAGS: -parse-flow
// Capstone fix (finding F1) — the Flow STATEMENT kinds that have no
// `visit()` override at all, half one: `InterfaceDeclaration` and
// `EnumDeclaration`.
//
// Neither appears anywhere in the header's `visit` inventory
// (SemanticResolver.h:191-305, whose `#if HERMES_PARSE_FLOW` block names
// only `CoverTypedIdentifier`, `TypeAlias`, `TypeParameterDeclaration`,
// `TypeParameterInstantiation`, `TypeCastExpression`, `AsExpression`,
// `ComponentDeclaration` and `HookDeclaration`), so C++ reaches them
// through the default `visit(ESTree::Node *node) { visitESTreeChildren(
// *this, node); }` at :191-193. Under untyped `-parse-flow` that is
// observable: the interface's own `I` and the enum's own `E` are visited as
// ordinary `Identifier`s and resolve to `UndeclaredGlobalProperty` decls —
// which is precisely what distinguishes a children walk from `TypeAlias`'s
// do-nothing visit, where `A` gets no annotation at all. Both are in this
// file side by side so the difference is visible in one dump.
//
// `DeclCollector` needs nothing extra for either: it has its own
// no-descend `visit(InterfaceDeclarationNode *)` (DeclCollector.h:96-98),
// and `EnumDeclaration` is a plain children walk there too — neither
// creates a scope or a declaration, so the `E`/`I` bindings the resolver
// sees are the global-property ones the dump shows.
//
// Before the fix both panicked at `visit_node`'s catch-all while hermesc
// exited 0 with a full dump.
type A = number;

interface I { x: number }

enum E { A, B }

var v = 1;
