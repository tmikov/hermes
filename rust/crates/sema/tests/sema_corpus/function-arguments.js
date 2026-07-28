// The implicit 'arguments' object: declared for every non-arrow function
// that has no parameter and no variable of that name, whether or not it is
// referenced.
function uses() {
  return arguments;
}
function unused(a) {
  return a;
}
function shadowedByParam(arguments) {
  return arguments;
}
function shadowedByVar() {
  var arguments;
  return arguments;
}
