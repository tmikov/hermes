// The four Cover* visits (cpp:1572-1586). The parser builds a Cover node when
// something is only legal as part of arrow parameters (or a destructuring
// target) and it turned out not to be, leaving the rejection to sema.
var a = ();
var b = (1, );
var c = ({ p = 1 });
var d = (...e);
