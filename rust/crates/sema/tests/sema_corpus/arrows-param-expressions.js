// An arrow with parameter expressions gets the dual parameter/body scope
// layout (visitFunctionLikeInFunctionContext, cpp:1846-1881) and, unlike a
// normal function, never declares `arguments` in either of them
// (cpp:1854-1864 for the parameter scope, cpp:1919-1924 for the body). The
// folds in the defaults rebuild the arrow, which is the shape the S1 capstone
// asked to pin.
var a = (x = 1 + 2) => x;
var b = (x, y = x + 1) => y;
function outer(p = () => 1 + 2) {
  var c = (q = p) => q;
  return c;
}
