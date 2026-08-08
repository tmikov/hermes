// Pins `reparse_assignment_pattern`'s `in_decl` parameter, which was never
// threaded into `reparse_array_assignment_pattern`/
// `reparse_object_assignment_pattern` (both hardcoded `false` at their own
// recursive calls). That silently disabled C++'s `inDecl`-gated
// rest-property identifier check (JSParserImpl.cpp:6080-6087, the live
// `#else` arm — the `#if 0` arm at cpp:6074 is dead code) for every array-
// or object-pattern reached with `in_decl=true` (arrow-function parameters).
// Before the fix: the object case fell through to "invalid destructuring
// target"; the array case silently accepted the malformed rest argument.
({ ...a.b }) => 1;
([...a.b]) => 1;
