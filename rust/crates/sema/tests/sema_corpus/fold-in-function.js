// A fold inside a hoisted function declaration rebuilds the BlockStatement
// and therefore the FunctionDeclaration itself, which is what the
// LexicalScope::hoistedFunctions backref fixup exists for.
function f(a) {
  var x = 1 + 2;
  return a;
}
