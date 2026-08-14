// visit(YieldExpressionNode *) (cpp:1490-1506) inside a real generator: no
// error, and the argument (and the delegate form) resolve normally.
function* g() {
  var x = yield;
  var y = yield 1 + 2;
  yield* g;
  return x + y;
}
var h = function* () {
  yield 1;
};
function* outer() {
  var inner = function* () {
    yield 2;
  };
  yield inner;
}
