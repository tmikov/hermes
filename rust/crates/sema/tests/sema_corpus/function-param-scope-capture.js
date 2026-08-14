// A nested function declared in the body may default a parameter from the
// enclosing function's parameter scope, which only resolves because the
// parameter scope is still on the binding stack.
function f(a, b = 1) {
  function g(c = a) {
    return c;
  }
  return g;
}
