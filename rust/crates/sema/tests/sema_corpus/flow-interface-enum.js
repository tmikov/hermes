// FLAGS: -parse-flow
// Capstone fix (finding F1) — the Flow STATEMENT kinds that have no
// `visit()` override at all, half one: `InterfaceDeclaration` and
// `EnumDeclaration`.
//
// Neither appears anywhere in the header's `visit` inventory
// (SemanticResolver.h:191-307, whose `#if HERMES_PARSE_FLOW` block names
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
// no-descend `visit(InterfaceDeclarationNode *)` (DeclCollector.h:97-99),
// and `EnumDeclaration` is a plain children walk there too — neither
// creates a scope or a declaration, so the `E`/`I` bindings the resolver
// sees are the global-property ones the dump shows.
//
// Before the fix both panicked at `visit_node`'s catch-all while hermesc
// exited 0 with a full dump.
type A = number;

interface I { x: number }

enum E { A, B }

// `TypeParameterDeclaration` (`visit(TypeParameterDeclarationNode *)`,
// cpp:1597-1599) — a true no-op like `TypeAlias`'s above, but reached from a
// DIFFERENT parent than `flow-type-args.js`'s `TypeParameterInstantiation`
// pin: `InterfaceDeclaration`'s own `typeParameters` field, walked by the
// override-free Flow range arm this file's `I`/`E` already exercise. `T`'s
// bound and `U`'s default both hold `typeof host`; because the visit never
// calls `visitESTreeChildren`, neither `host` resolves, while the body's
// `host` (an ordinary child of the range-walked `ObjectTypeAnnotation`)
// does — the same walked/unwalked contrast as `A` vs `I`/`E` above, one
// level deeper. (`T`'s `bound` is wrapped in a `TypeAnnotation` node that
// the dump printer itself never descends into — `ASTPrinter::shouldVisit
// (TypeAnnotationNode *)`, SemResolve.cpp:52-54 — so only `U`'s unwrapped
// `default` is visible here to prove the point; the `TypeAnnotation`
// wrapper is a dump-only omission, unrelated to resolution.)
interface J<T: typeof host, U = typeof host> { b: typeof host }

var v = 1;
