// FLAGS: -parse-flow
// The corpus's only -parse-flow file, and the only exercise of the C++
// tool's ParseFlowSetting::ALL branch (sema-parser-dump.cpp's `if (parseFlow)
// ctx.setParseFlow(...)`) -- without it that branch is dead and a corpus file
// could never reach the Flow grammar on either side. The annotations parse
// into type nodes that the resolver walks past without declaring anything, so
// the dump is the same shape an untyped version would give: `f`/`y` global
// properties, `x` a parameter. Resolves clean (exit 0), so it is also an
// oracle-success file for the non-degeneracy guard.
function f(x: number): number {
  return x;
}
var y = f(1);
