// FLAGS: -parse-flow
// Negative control for the two error-shape files in this batch
// (`flow-typecast-cover.js`, `flow-this-param.js`): ordinary parameter,
// return and variable type annotations, parsed under `-parse-flow`, resolve
// completely cleanly. The annotation nodes themselves are never visited as
// expressions (`TypeAnnotation`/`GenericTypeAnnotation` are not on
// `visit_node`'s dispatch and the parser keeps them off the identifier's
// resolved-expression path), so they neither perturb declarations nor
// scopes — `f`'s parameter `x` and the top-level `y` resolve exactly as
// they would without the annotations. Exit 0, full dump comparison.
function f(x: number): number { return x; }
var y: string;
