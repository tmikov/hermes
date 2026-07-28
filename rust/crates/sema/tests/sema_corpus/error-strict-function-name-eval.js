"use strict";
// Two errors from one identifier: validateAndDeclareIdentifier rejects the
// hoisted declaration, and visitFunctionLikeInFunctionContext's
// validateDeclarationName(FunctionExprName, id) rejects it again (cpp:1721).
function eval() {
  return 1;
}
