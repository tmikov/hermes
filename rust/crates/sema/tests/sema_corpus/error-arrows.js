// Arrow-specific error shapes.
//
// `uniqueParams` is unconditionally true for an arrow (cpp:1755-1756's third
// disjunct, `isa<ArrowFunctionExpressionNode>(node)`), so duplicate arrow
// parameters are an error even in loose mode — unlike a plain function.
var dup = (x, x) => x;
// A parameter and a `let` of the same name in the body still collide.
var shadow = (x) => { let x = 1; };
// 'use strict' is not allowed inside a function with a non-simple parameter
// list, and an arrow is a function like any other here.
var nonsimple = ({ a }) => { "use strict"; };
var deflt = (a = 1) => { "use strict"; };
