function f<T>(x: T, y: number): T {
  return x;
}
function g(this: Object, a: number): void {}
var h = function <T>(): T {};
function p(x: mixed): boolean %checks {
  return !!x;
}
function q(x: mixed): %checks (typeof x === 'string') {}
