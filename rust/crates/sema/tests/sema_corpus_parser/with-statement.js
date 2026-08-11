// `with` is a compile_-gated error (SemanticResolver.cpp), so the DRIVER
// pair can never dump a `with` body -- only this compile = false pair can.
// Unresolver::visit (SemanticResolver.cpp:3206-3224) marks `x` unresolvable,
// and the dumper's enter(IdentifierNode*) must NOT call getExpressionDecl()
// on it (its precondition, SemContext.h:559-561). Upstream 918158cb0 made
// the C++ dumper check isUnresolvable() first, so a DEBUG sema-parser-dump
// no longer aborts here and prints ` UNR` like release always did -- which
// is what this port has always printed (dump.rs). Pins that agreement.
with (o) { x; }
