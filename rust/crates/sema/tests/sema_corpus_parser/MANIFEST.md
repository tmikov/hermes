# `tests/sema_corpus_parser` corpus (S4a Tasks 2-3)

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

`read_dir` in `run_differential` is non-recursive, so a `pending/`
subdirectory is automatically excluded from the walk — no extra filtering
code needed. As of S4a Task 3 the Pending table is empty and the directory is
gone (git does not track empty directories); the mechanism is documented here
for whoever needs it next.

## Imported (live differential gate)

| File | Covers |
|---|---|
| `plain.js` | `var x = 1 + 2; print(x);`. Pins two `compile = false` behaviors at once: (1) NO ambient globals — `resolveASTForParser` passes `ambientDecls = nullptr`, so the dump's `Scope %s.1` contains only `x` (declared) and `print` (resolves `UndeclaredGlobalProperty`, not one of `libhermes`'s 63 ambient decls); (2) NO constant folding — `+`/`-` folding is gated on `compile_` (cpp:405-436), so the dump shows the unfolded `BinaryExpression`/`BinOp +` tree via `ASTPrinter`, not a folded `NumericLiteral 3`. Verified: `sema-parser-dump plain.js` exit 0, no stderr; `sema-dump --parser-entry plain.js` byte-identical stdout. This is the corpus's only hermesc-analogue SUCCESS (exit 0) file — the non-degeneracy guard in `run_differential` needs at least one. |
| `error-break-outside-loop.js` | `break;`. Proves "dump despite errors": resolution reports `'break' not within a loop or a switch` (a genuine post-walk `sm_.getErrorCount() != 0` from `SemanticResolver::run`/`run_always`'s SECOND gate, `SemanticResolver.cpp:69` / `resolver/mod.rs`), yet BOTH tools still print the full dump (`Func loose`/`Scope %s.1`/`BreakStatement`) and exit 2. Verified byte-identical: 77 bytes stdout both sides, matching stderr, exit 2 both sides. |
| `error-arrow-rewrite-then-error.js` | `var f = (a) => a + 1;\nbreak;`. Same post-walk gate as above, but AFTER a real rewrite (arrow-function processing, S2 rewrite #1) has already mutated the tree — proves `run_always`'s rebuilt-tree-on-error path carries the rewrite through, not just the original unmodified nodes. Verified byte-identical: 481 bytes stdout both sides, matching stderr, exit 2 both sides. |
| `error-continue-outside-loop.js` | `function f(){ continue; }`. Same post-walk gate, nested one function deep (`continue` outside a loop inside a function body, not at Program scope) — proves the dump-despite-error path also works when the error site is below the top-level function context. Verified byte-identical: 310 bytes stdout both sides, matching stderr, exit 2 both sides. |
| `compile-false-basics.js` | `export default function f(){}`. **S4a Task 3** — moved in from `pending/`. Pins TWO `compile_`-gated behaviors of `visit(ExportDefaultDeclarationNode *)` at once (cpp:1519-1547): no `'export' statement requires module mode` error is emitted (cpp:1520 is `compile_ &&`), and **rewrite #4 does not fire** (cpp:1526 likewise) — the dump shows `ExportDefaultDeclaration` → `FunctionDeclaration`, not the `FunctionExpression` the rewrite would have produced. Verified byte-identical: exit 0 both sides, full dump, empty stderr. This is the corpus's SECOND hermesc-analogue success file |
| `module-imports.js` | `import d, {a as b} from 'm'; import * as ns from 'n';`. **S4a Task 3** — the other side of the module-mode asymmetry: the import error is NOT `compile_`-gated (cpp:876-879), so both declarations error even here, and the tool dumps anyway. That dump is what the DRIVER corpus can never show (hermesc skips the dump on a `resolveAST` failure), so this file is the only pin for `extractIdentsFromDecl`'s `ImportDeclaration` arm (cpp:2334-2347): `Decl %d.N Import` for `d` (`ImportDefaultSpecifier`), `b` (`ImportSpecifier` `_local`) and `ns` (`ImportNamespaceSpecifier`), plus — proving the specifier children walk really runs — `a` (the `ImportSpecifier`'s `_imported`) resolving as an ordinary `UndeclaredGlobalProperty`. Verified byte-identical: exit 2 both sides, matching stdout and stderr |
| `error-invalid-assignment-lvalue.js` | `1 = 2;`. Also the post-walk gate (`ResolverTest.cpp`'s `TestBadAssignmentLValue` confirms "invalid assignment left-hand side" is a `sema::resolveAST`-time check on an already-cleanly-parsed tree, not a parser diagnostic) — see "Gate classification" below for why this file does NOT exercise the entry gate, correcting an initial (hedged, "verify yourself") classification from code review. Verified byte-identical: 165 bytes stdout both sides, matching stderr, exit 2 both sides. |

## Pending (excluded from the walk — `pending/` subdirectory)

Empty as of S4a Task 3. The one row that lived here,
`compile-false-basics.js`, was blocked on `resolver/mod.rs`'s catch-all
panicking for `Node::ExportDefaultDeclaration`; Task 3 landed the four
module-visit arms (`resolver/modules.rs`) and the file moved into the live
table above.

## Gate

`sema differential (tests/sema_corpus_parser): 7 corpus files matched (2
succeeded on the oracle)` — 5 → **7** files (+2: `module-imports.js`,
authored by S4a Task 3, and `compile-false-basics.js`, moved in from
`pending/`), oracle-succeeded 1 → **2** (+1: `compile-false-basics.js` is an
exit-0 file; `module-imports.js` is an error-path pin). The non-degeneracy
guard in `run_differential` (at least one oracle success) is satisfied twice
over; the other five are all legitimate error-path pins (oracle exit 2), same
convention as `tests/sema_corpus/parse-error.js`.

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

## Landmine: anonymous `export default function` under `compile = false` — BOTH dumpers crash

`export default function () {}` can never be added to this corpus, by
construction, on either side (deferred from S4a T3's review; verified here
2026-08-03). Under `compile = false` — this pair's whole reason to exist —
rewrite #4 (`visit_export_default_declaration`, cpp:1526-1544) is
`compile_`-gated and does not fire, so the anonymous `FunctionDeclaration`
survives unrewritten. `visit(FunctionDeclarationNode*)`
(`SemanticResolver.cpp:232-236`, `resolver/functions.rs`'s port) pushes it
onto the enclosing scope's `hoistedFunctions` UNCONDITIONALLY — the hoist
does not check for a name — so a null-`_id` function ends up in that list.
Both dumpers then crash printing it, at the SAME underlying defect: a
null-id function reaching the `hoistedFunction` printer, which
unconditionally casts `_id` to an identifier:

- C++ `SemContextDumper::printScope` (`SemContext.cpp:493-494`,
  `llvh::cast<ESTree::IdentifierNode>(fd->_id)`) hits `isa<> used on a null
  pointer` (`Casting.h:106`), SIGABRT.
- Rust `dump_context.rs`'s `print_scope`
  (`.expect("a hoisted FunctionDeclaration always has an id")`) panics.

Verified directly, both sides, on `export default function () {}\n`:

```
$ cmake-build-asan/bin/sema-parser-dump anon-default.js
sema-parser-dump: .../Casting.h:106: ... Assertion `Val && "isa<> used on a
null pointer"' failed.
Aborted (core dumped)                                          # exit 134

$ sema-dump --parser-entry anon-default.js
thread 'main' panicked at crates/sema/src/dump_context.rs:304:18:
a hoisted FunctionDeclaration always has an id                 # exit 101
```

Same category as `test/hermes/computed-fn-name.js`
(`SemContext.cpp:478`'s scope-walk assertion, one of the roadmap's Sema-row
documented hermesc self-aborts): a pre-existing C++ **dumper** defect,
faithfully mirrored — not a port gap, and not fixable without hermesc
itself changing. This exact shape (anonymous default export, dumped under
`compile = false`) is excluded from this corpus for that reason, same as
`computed-fn-name.js` is excluded from `tests/sema_corpus`.
