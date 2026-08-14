// Nested arrows: the chain of rewrites (each level's expression body becomes a
// block), an arrow reaching `arguments` past another arrow and past a function
// expression that has its own, and `for await (... of ...)` inside an async
// arrow.
var a = () => () => () => 1 + 2 + 3;
var b = (x) => (y) => x + y;
function outer() {
  var c = () => { var d = () => arguments; return d; };
  var e = function () { return arguments; };
  return c;
}
var f = async () => { for await (var x of [1]) { } };
