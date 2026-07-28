// A function expression inside a parameter initializer is created in the
// PARAMETER scope, not the body scope — that is what the split layout is
// for. The initializer also references the temporarily-declared
// 'arguments', which resolves to the enclosing function's Arguments decl.
function f(a, b = function () {
  return a;
}, c = arguments) {
  var d = b;
  return d;
}
