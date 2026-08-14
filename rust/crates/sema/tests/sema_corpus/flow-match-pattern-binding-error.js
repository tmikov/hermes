// FLAGS: -parse-flow --Xparse-flow-match
// Import of `test/Parser/flow/match/pattern-binding-error.js`, the regression
// test added by upstream `550aafe33` ("Fix crash after reporting a bad match
// binding pattern"). After reporting "'identifier' expected in match binding
// pattern", the parser used to continue into
// `parseMatchBindingIdentifierFlow`, which reads the identifier off the
// current — here non-identifier — token and asserts. That was defect 11 in
// `doc/superpowers/CppDefectsFound.md`; the Rust port panicked identically at
// `Token::get_res_word_or_identifier` (bug-for-bug parity), and the pin is
// flipped along with the fix in `parser/src/js/flow/match_.rs`.
//
// The flag is spelled `--Xparse-flow-match` (double dash) because the whole
// `// FLAGS:` line goes verbatim to BOTH binaries: LLVM's `cl` accepts either
// dash count for hermesc's hidden `-Xparse-flow-match`, and `sema-dump`
// carries a hidden alias under that same long name (its `command_line`
// single-dash path would read `-X` as a short option).
//
// This is a parse-error file: the parse fails before sema runs, so all three
// channels pin the PARSER diagnostic (empty stdout, the rendered error plus
// the driver epilogue on stderr, exit 2). The unit-level twin is
// `parser/tests/upstream_defect_fixes.rs`.
const e = match (x) { const [y]: 2 };
