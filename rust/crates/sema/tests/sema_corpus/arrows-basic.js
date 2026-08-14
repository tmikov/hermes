// Arrow functions: the expression-body rewrite (SemanticResolver.cpp:253-266)
// turns `=> expr` into `=> { return expr; }` before the body is visited, so
// the dump below shows a BlockStatement + ReturnStatement for every
// expression-bodied arrow and leaves the block-bodied ones alone.
var a = x => x;
var b = (x, y) => x + y;
var c = () => 1;
var d = (x) => { return x; };
var e = x => y => x + y;
var f = (x = 1) => x;
var g = ({ p, q }) => p;
var h = (...rest) => rest;
