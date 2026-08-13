// The three export visits in one file: visit(ExportNamedDeclarationNode *)
// (cpp:1524-1531), visit(ExportDefaultDeclarationNode *) (cpp:1533-1561)
// and visit(ExportAllDeclarationNode *) (cpp:1563-1568). All three share
// the same `compile_ && !getUseCJSModules()` gate and, since f90a83146,
// the same message: `'export' statement requires module mode` (ExportAll
// used to say "CommonJS module mode"). This file pins all three at once.
// The anonymous `export default function` also drives rewrite #4
// (cpp:1539-1558) through the walk under compile_ = true; the rewrite itself
// is dump-invisible here (hermesc skips the dump on a resolveAST failure,
// CompilerDriver.cpp:978-992), so what this pins is that the rewritten
// subtree still resolves without incident.
export {a};
export default function () {}
export * from 'm';
var a = 1;
