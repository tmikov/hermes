// The parser allows 'let' as a binding identifier here (the parameters are
// parsed before the directive is seen), so the error comes from sema's
// validateDeclarationName.
function f(let) {
  "use strict";
  return 1;
}
