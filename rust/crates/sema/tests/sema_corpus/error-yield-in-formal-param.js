// "'yield' not allowed in a formal parameter" (cpp:1497-1503):
// isFormalParams is set by visitParams while the parameter initializers are
// visited.
function* g(x = yield 1) {}
function* h(a, b = yield) {}
