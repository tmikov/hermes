// A HARD parse error: parseProgram() cannot produce a tree at all, so both
// parsers return None/nullptr through the `if (!res) return None;` arm
// (JSParserImpl.cpp:168-169) rather than the error-count arm exercised by
// parse-error-recoverable.js. Pins the other half of the tool pair's no-AST
// path: no dump, exit 2, and both diagnostics (the lexer's and the
// declaration parser's) on stderr in the same order.
var 1x;
