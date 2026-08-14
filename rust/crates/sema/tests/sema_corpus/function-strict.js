"use strict";
// Strictness is inherited from the enclosing function...
function outer(a) {
  function inner(b) {
    return b;
  }
  return a;
}
