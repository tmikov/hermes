// FLAGS: -parse-flow
// Capstone fix (finding F1) — the two pattern overrides,
// `visit(ObjectPatternNode *, Node *)` and `visit(ArrayPatternNode *,
// Node *)` (SemanticResolver.h:209-214). Both are inline header one-liners:
//
//     visitESTreeNodeList(*this, node->_properties, node);
//     visitESTreeNodeList(*this, node->_elements, node);
//
// `ObjectPattern`/`ArrayPattern` each have TWO children in the AST —
// `properties`/`elements` and `typeAnnotation` (ESTree.def:646-656) — and
// the overrides visit only the first. So an ANNOTATED destructuring pattern
// resolves its bindings normally while sema never descends into the Flow
// type annotation.
//
// This is the shape that made the overrides load-bearing rather than
// equivalent to the generic children walk: before the fix, `ObjectPattern`/
// `ArrayPattern` sat on `mod.rs`'s override-free generic arm, which DOES
// visit `type_annotation`, so all three shapes below panicked at the
// catch-all while hermesc exited 0 with a full dump.
//
// Covered: a `var` object pattern, a `var` array pattern, an annotated
// object pattern as a function PARAMETER (the pattern is reached from
// `declareParams`' walk rather than from a variable declaration), a nested
// pattern whose inner pattern is annotated too, and an annotated pattern
// with a default — each binding still resolves, and `Obj`/`Arr` (which are
// nowhere declared) get no resolution annotation because the annotation is
// never visited.
type Obj = { a: number };
type Arr = number[];

var {a}: Obj = {a: 1};
var [b]: Arr = [1];
print(a, b);

function g({a}: Obj) { return a; }

var {p: [q]}: Obj = {p: [1]};
var [{r}]: Arr = [{r: 1}];
var {s = 1}: Obj = {};
print(q, r, s);
