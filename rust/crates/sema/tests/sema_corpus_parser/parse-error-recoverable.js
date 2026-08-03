// A RECOVERABLE parse error: the lexer reports the strict-mode octal and
// keeps going, so parseProgram() itself returns a Program node. C++
// JSParserImpl::parse (JSParserImpl.cpp:164-172) then throws that tree away
// -- `if (lexer_.getSourceMgr().getErrorCount() != 0) return None;` -- so the
// tool's `if (!parsedJs)` (sema-parser-dump.cpp:115-119) fires, nothing is
// dumped, and it exits 2. The Rust parse() has no such gate (it returns Some
// here), so sema-dump's own call site must apply the error-count check; this
// file is the pin for that. Without it, the unresolved tree reaches sem_dump
// and indexes an empty SemContext.
"use strict";
var x = 010;
