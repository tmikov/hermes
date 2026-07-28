// Named and anonymous function declarations and expressions, plus a nested
// function declaration at the top level of a function body (NOT inside a
// block: block-nested declarations need ScopedFunctionPromoter, S3).
function outer(a) {
  function inner(b) {
    return b;
  }
  var anon = function (c) {
    return c;
  };
  var named = function self(d) {
    return self;
  };
  return a;
}
var top = function () {
  return 1;
};
