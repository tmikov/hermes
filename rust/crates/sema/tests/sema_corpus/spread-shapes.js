// visit(SpreadElementNode *, Node *) (cpp:1455-1467): the parent-kind
// whitelist. ArrayExpression, ObjectExpression and NewExpression are the
// three accepted parents the corpus can reach; CallExpression and
// OptionalCallExpression are whitelisted too but have their own visit
// override (cpp:1117, the eval/$SHBuiltin specials), so they are not
// resolvable yet. No parse this port can produce puts a SpreadElement under
// any OTHER parent, so the "spread operator is not supported" error is
// unreachable — see the visit's doc comment.
var a = [1, ...b, 2];
var c = { p: 1, ...d };
var e = new f(...a);
var g = [...[...a]];
function h(x) {
  return [...x];
}
