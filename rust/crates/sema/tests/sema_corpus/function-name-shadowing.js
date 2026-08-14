// A function's own name is a binding in the ENCLOSING scope, so a var or a
// parameter of the same name inside the function shadows it rather than
// conflicting with it.
function f() {
  var f;
  return f;
}
function g(g) {
  return g;
}
