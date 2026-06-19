// JSX as a sub-expression and the JS<->JSX token-stream round-trip. Exercises
// the lexer-mode resumption after JSX (post-self-close `advance()` returns to
// standard JS mode), plus deep nesting that drives the jsxDepth_ mode-switch.
var a = <div><span><b>deep</b></span></div>;
var b = cond ? <a/> : <b/>;
var c = [<x/>, <y/>];
var d = <A.B.C />;
var e = <while>kw</while>;
var f = <div>{<span>nested</span>}</div>;
var g = <a/> + 1;
var h = <input type="text" value={v} required {...rest} />;
var i = <a><b><c>deep</c></b></a>;
var j = <ns:while attr={x}>txt</ns:while>;
var k = <A.B.C></A.B.C>;
var l = <><><x/></></>;
