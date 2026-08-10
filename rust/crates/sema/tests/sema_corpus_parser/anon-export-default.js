// An anonymous `export default function` is rewritten to a
// FunctionExpression only when compiling (rewrite #4,
// SemanticResolver.cpp:1526-1544, gated on compile_), so under this pair's
// compile = false entry point the FunctionDeclaration survives with a null
// _id -- and visit(FunctionDeclarationNode*) hoists it unconditionally, so a
// nameless function reaches the hoistedFunction printer. Upstream 918158cb0
// made SemContextDumper::printScope print `*default*` instead of casting
// _id unconditionally; dump_context.rs's print_scope mirrors it. Pins the
// `hoistedFunction *default*` line, unreachable from the driver corpus.
export default function () {}
