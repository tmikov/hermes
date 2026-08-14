// FLAGS: -parse-flow
// Negative control for the two error-shape files in this batch
// (`flow-typecast-cover.js`, `flow-this-param.js`): ordinary parameter,
// return and variable type annotations, parsed under `-parse-flow`, resolve
// completely cleanly. The annotation nodes themselves are never visited as
// expressions: `visit(IdentifierNode *, Node *)` does not walk the
// identifier's children, so its `typeAnnotation` is never dispatched at all
// (the capstone fix's `Flow`-range arm in `visit_node` — which DOES walk
// `TypeAnnotation`/`GenericTypeAnnotation` children when something else
// reaches them, e.g. an `interface` body — is not on this path), so they
// neither perturb declarations nor
// scopes — `f`'s parameter `x` and the top-level `y` resolve exactly as
// they would without the annotations. Exit 0, full dump comparison.
function f(x: number): number { return x; }
var y: string;
