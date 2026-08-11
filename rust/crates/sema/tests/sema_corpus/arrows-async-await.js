// Async arrows: `await` is allowed in the body (forbidAwaitExpression_ is
// false inside an async function) and `await` is forbidden as an identifier
// anywhere in an async arrow's parameters (the `visitParams` lambda,
// cpp:1818-1833).
var a = async x => await x;
var b = async (x, y) => { return await x + await y; };
async function outer() {
  var c = async () => await 1;
  var d = () => 1;
  return await d;
}
