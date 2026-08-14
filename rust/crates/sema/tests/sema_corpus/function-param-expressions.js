// hasParameterExpressions splits the parameters into their own scope: the
// parameter scope, then the (empty) temporary 'arguments' scope, then the
// function body scope.
function withExpressions(a, b = a, [c, d], {e}, ...rest) {
  var g = a;
  return g;
}
// Simple parameter list: parameters and body share one scope.
function simpleParams(x, y) {
  return x;
}
// Non-simple but expression-free parameters still take the split layout,
// because extractDeclaredIdentsFromID reports the AssignmentPattern.
function onlyPatterns([p], {q}) {
  return p;
}
