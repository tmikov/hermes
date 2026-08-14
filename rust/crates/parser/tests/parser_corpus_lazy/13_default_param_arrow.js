function f(x = () => arguments) { return x; }
function g(y = () => 1) { return y; }
var h = function (z = () => arguments) { return z; };
function outer(a = () => 2) { function inner() { return 1; } return inner; }
