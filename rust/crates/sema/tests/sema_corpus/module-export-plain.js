// The three export visits in one file: visit(ExportNamedDeclarationNode *)
// (cpp:1510-1517), visit(ExportDefaultDeclarationNode *) (cpp:1519-1547)
// and visit(ExportAllDeclarationNode *) (cpp:1549-1554). All three share the
// same `compile_ && !getUseCJSModules()` gate, but NOT the same message:
// ExportAll says "CommonJS module mode" where the other two say plain
// "module mode" (cpp:1552-1553) — the wording quirk this file pins.
// The anonymous `export default function` also drives rewrite #4
// (cpp:1525-1544) through the walk under compile_ = true; the rewrite itself
// is dump-invisible here (hermesc skips the dump on a resolveAST failure,
// CompilerDriver.cpp:960-974), so what this pins is that the rewritten
// subtree still resolves without incident.
export {a};
export default function () {}
export * from 'm';
var a = 1;
