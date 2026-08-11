// A NAMED function expression whose body folds: `visitFunctionExpression`
// (cpp:1961-1979) opens a scope decorated on the FunctionExpression node
// itself before visiting the body, and the fold rebuilds that node — so the
// scope decoration has to survive the rebuild (decorate before recursing).
// The S1 capstone asked for this shape as a corpus pin.
var f = function named() {
  return 1 + 2;
};
var g = function inner(x) {
  var h = function () {
    return x + 3 + 4;
  };
  return h;
};
