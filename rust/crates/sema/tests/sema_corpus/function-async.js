// Async functions are ordinary functions here: forbidAwaitExpression_ is
// cleared for the body and forbidAwaitAsIdentifier_ is set. (Async
// GENERATORS would be rejected — getEnableAsyncGenerators() is false — but
// a generator body needs the YieldExpression visit, which is S2.)
async function f(a) {
  return a;
}
var g = async function (b) {
  return b;
};
