# `tests/sema_corpus_parser` corpus (S4a Task 2)

Companion corpus to `tests/sema_corpus/MANIFEST.md`, but for a DIFFERENT
oracle pair: the C++ `tools/sema-parser-dump/sema-parser-dump.cpp` tool vs
Rust `sema-dump --parser-entry`, exercised by `sema_parser_differential` in
`sema_differential.rs`. Both sides resolve via `resolveASTForParser`
(`SemResolve.cpp:295-306` / `resolve::resolve_ast_for_parser`) — the
`compile = false` entry point `hermes-parser-wasm.cpp:104` uses — instead of
`resolveAST`/`resolve_ast` (`compile = true`), and the C++ tool dumps
UNCONDITIONALLY (even when diagnostics were emitted), which `hermesc
-dump-sema` never does (`CompilerDriver.cpp:960-974` skips the dump on a
`resolveAST` failure). Every file here was run as `sema-parser-dump <file>`
vs `sema-dump --parser-entry <file>` (no extra flags — matching what
`sema_parser_differential` actually invokes) before being imported, per the
global constraint that every corpus file is verified against the C++ side
FIRST with the raw stdout+stderr+exit triple.

`read_dir` in `run_differential` is non-recursive, so the `pending/`
subdirectory below is automatically excluded from the walk — no extra
filtering code needed.

## Imported (live differential gate)

| File | Covers |
|---|---|
| `plain.js` | `var x = 1 + 2; print(x);`. Pins two `compile = false` behaviors at once: (1) NO ambient globals — `resolveASTForParser` passes `ambientDecls = nullptr`, so the dump's `Scope %s.1` contains only `x` (declared) and `print` (resolves `UndeclaredGlobalProperty`, not one of `libhermes`'s 63 ambient decls); (2) NO constant folding — `+`/`-` folding is gated on `compile_` (cpp:405-436), so the dump shows the unfolded `BinaryExpression`/`BinOp +` tree via `ASTPrinter`, not a folded `NumericLiteral 3`. Verified: `sema-parser-dump plain.js` exit 0, no stderr; `sema-dump --parser-entry plain.js` byte-identical stdout. This is the corpus's only hermesc-analogue SUCCESS (exit 0) file — the non-degeneracy guard in `run_differential` needs at least one. |
| `error-break-outside-loop.js` | `break;`. Proves "dump despite errors": resolution reports `'break' not within a loop or a switch` (a genuine post-walk `sm_.getErrorCount() != 0` from `SemanticResolver::run`/`run_always`'s SECOND gate, `SemanticResolver.cpp:69` / `resolver/mod.rs`), yet BOTH tools still print the full dump (`Func loose`/`Scope %s.1`/`BreakStatement`) and exit 2. Verified byte-identical: 77 bytes stdout both sides, matching stderr, exit 2 both sides. |
| `error-arrow-rewrite-then-error.js` | `var f = (a) => a + 1;\nbreak;`. Same post-walk gate as above, but AFTER a real rewrite (arrow-function processing, S2 rewrite #1) has already mutated the tree — proves `run_always`'s rebuilt-tree-on-error path carries the rewrite through, not just the original unmodified nodes. Verified byte-identical: 481 bytes stdout both sides, matching stderr, exit 2 both sides. |
| `error-continue-outside-loop.js` | `function f(){ continue; }`. Same post-walk gate, nested one function deep (`continue` outside a loop inside a function body, not at Program scope) — proves the dump-despite-error path also works when the error site is below the top-level function context. Verified byte-identical: 310 bytes stdout both sides, matching stderr, exit 2 both sides. |
| `error-invalid-assignment-lvalue.js` | `1 = 2;`. Also the post-walk gate (`ResolverTest.cpp`'s `TestBadAssignmentLValue` confirms "invalid assignment left-hand side" is a `sema::resolveAST`-time check on an already-cleanly-parsed tree, not a parser diagnostic) — see "Gate classification" below for why this file does NOT exercise the entry gate, correcting an initial (hedged, "verify yourself") classification from code review. Verified byte-identical: 165 bytes stdout both sides, matching stderr, exit 2 both sides. |

## Pending (excluded from the walk — `pending/` subdirectory)

| File | Blocked on | Target phase |
|---|---|---|
| `compile-false-basics.js` | `export default function f(){}`. On the C++ side this pins that NO module-mode error is emitted under `compile = false` (the export gate is `compile_ &&`, cpp:1511) — verified: `sema-parser-dump compile-false-basics.js` exits 0 with a full dump (`ExportDefaultDeclaration` → `FunctionDeclaration`). On the Rust side `sema-dump --parser-entry` PANICS: `resolver/mod.rs`'s catch-all hits `Node::ExportDefaultDeclaration`, which is one of the four module-visit arms explicitly reserved for S4a Task 3 (per the plan's global constraints: "ONLY the four module-visit arms replace catch-all panics in this phase"). Move to the parent directory (out of `pending/`) once Task 3 lands `Import`/`Export*` visits. | S4a Task 3 |

## Gate

`sema differential (tests/sema_corpus_parser): 5 corpus files matched (1
succeeded on the oracle)` — as of this task's fix round. The non-degeneracy
guard in `run_differential` (at least one oracle success) is satisfied by
`plain.js`; the other four are all legitimate error-path pins (oracle exit 2),
same convention as `tests/sema_corpus/parse-error.js`.

## `SemanticResolver::run`'s two gates, and which files hit which

`SemanticResolver::run` (`SemanticResolver.cpp:65-70`) / its Rust ports
`run`/`run_always` (`resolver/mod.rs`) have TWO `sm_.getErrorCount() != 0`
checks: an ENTRY gate before visiting starts, and a POST-WALK gate after.
Both C++ and Rust dumps carry a tell: the `Func loose` / `Scope %s.1` header
is only printed once `SemanticResolver::visit(ProgramNode*)` has actually
run (it creates the global `FunctionInfo`/scope as the very first thing),
so its presence in a dump proves the walk happened — i.e. distinguishes
"entry gate fired" (no header, nothing rebuilt) from "post-walk gate fired"
(header present, tree rebuilt, error(s) reported somewhere in it).

All four error files above show the header, so **all four hit the POST-WALK
gate** — none of them exercises the entry gate. This was verified
empirically (`sema-parser-dump <file>` inspected by hand) after finding the
actual mechanism: `JSParserImpl::parse()` (`JSParserImpl.cpp:164-172`) ends
with `if (lexer_.getSourceMgr().getErrorCount() != 0) return None;` — i.e.
it is IMPOSSIBLE for `JSParser::parse()` to return `Some(ProgramNode*)`
while `sm.getErrorCount() != 0` from parsing. Since both `sema-parser-dump`
and `sema-dump --parser-entry` call `resolveASTForParser`/
`resolve_ast_for_parser` immediately after a fresh `parse()` with nothing
in between that could add errors (`--parser-entry` skips ambient-decl
loading entirely, so there is no `libhermes`-parse step to fail either),
`sm.error_count()` is always 0 at that call site — the entry gate can never
fire through this tool pair's call path. It remains correctly ported
(`run_always`'s first branch) for faithfulness and for any future caller
that might feed it a `SourceErrorManager` with preexisting errors, but is
provably dead code for this corpus's shape of test. (One review round
initially guessed `1 = 2;`/"invalid assignment left-hand side" fires the
entry gate, reasoning it might be a parser-level check; it is not — see
`ResolverTest.cpp`'s `TestBadAssignmentLValue`, which calls
`sema::resolveAST` on an already-successfully-parsed `"a + 1 = 10;"` and
expects `false` — confirming the check lives in the resolver, not the
parser, hence the post-walk gate here too.)

## Fixed gap: dump-despite-error now works

An earlier revision of this corpus (before the four error files above were
added) documented a "known gap": `resolve_ast_for_parser` delegated to
`SemanticResolver::run`, whose `Option`-returning contract discards the
fully-rewritten tree on any post-walk error, unlike C++'s in-place mutation
(where the caller's `root` pointer survives a `false` return regardless).
This is now fixed: `SemanticResolver` grew a SEPARATE `run_always` method
(`resolver/mod.rs`) that returns the rebuilt tree unconditionally —
`resolve_ast_for_parser` uses it instead of `run`, and its own return type
changed from `Option<&Node>` to `&Node` (it can no longer fail to produce a
tree) to make that contract explicit at the type level.
`resolve_ast`/`resolve_ast`'s driver-path behavior (`compile = true`) is
untouched — `run` (used only by `resolve_ast`) still returns `None` on
error, which is correct there (`hermesc` never dumps after a `resolveAST`
failure either).
