// A loose, simple parameter list may repeat a name: the later declaration
// wins and the earlier binding is updated to point at it.
function f(a, a) {
  return a;
}
