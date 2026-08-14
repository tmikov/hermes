// Recursion-depth boundary pin, clean side: N*-1 nested parentheses, where
// N* is the first depth at which BOTH hermesc and ast-dump report "Too many
// nested expressions/statements/declarations". With the ASan/debug limit
// (JSParserImpl::MAX_RECURSION_DEPTH = 128 under HERMES_LIMIT_STACK_DEPTH)
// N* is 126 for this shape, so 125 must still parse on both sides. If the
// Rust counter ever trips one production early, this file breaks the gate.
// The error side of the same boundary is pinned by the sema corpus
// (nested-expressions.js) and by parser/tests/recursion_depth_limit.rs.
((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((
(((((((((((((((((((((((((((((((((((((((((((((((
1
))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))
)))))))))))))))))))))))))))))))))))))))))))))));
