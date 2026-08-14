"use strict";
// In STRICT mode a block-nested function declaration is a ScopedFunction and
// no promotion happens (processPromotedFuncDecls is skipped), so this is
// reachable without S3's ScopedFunctionPromoter. The loose-mode counterpart
// deliberately stays out of the S1 corpus.
function f() {
  {
    function g() {
      return 1;
    }
    return g;
  }
}
{
  function top() {
    return 2;
  }
}
