// ES10.0 14.1.2: a top-level `let` may not repeat a parameter name.
function f(a) {
  let a;
  return a;
}
