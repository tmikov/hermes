# `tests/sema_corpus_parser` corpus (S4a Tasks 2-3 + final review; C++ defect-fix propagation Task 5)

Companion corpus to `tests/sema_corpus/MANIFEST.md`, but for a DIFFERENT
oracle pair: the C++ `tools/sema-parser-dump/sema-parser-dump.cpp` tool vs
Rust `sema-dump --parser-entry`, exercised by `sema_parser_differential` in
`sema_differential.rs`. Both sides resolve via `resolveASTForParser`
(`SemResolve.cpp:299-310` / `resolve::resolve_ast_for_parser`) — the
`compile = false` entry point `hermes-parser-wasm.cpp:104` uses — instead of
`resolveAST`/`resolve_ast` (`compile = true`), and the C++ tool dumps
UNCONDITIONALLY (even when diagnostics were emitted), which `hermesc
-dump-sema` never does (`CompilerDriver.cpp:960-974` skips the dump on a
`resolveAST` failure). Every file here was run as `sema-parser-dump <file>`
vs `sema-dump --parser-entry <file>` before being imported, per the global
constraint that every corpus file is verified against the C++ side FIRST
with the raw stdout+stderr+exit triple — and with exactly the flags its own
first-line `// FLAGS:` carries, which `sema_parser_differential` appends
verbatim to BOTH binaries' argv (`flow-annotations.js` is the only file here
that carries one; the rest are flagless).

`read_dir` in `run_differential` is non-recursive, so a `pending/`
subdirectory is automatically excluded from the walk — no extra filtering
code needed. The Pending table has been empty since S4a Task 3, and the
directory itself was removed by the final review (it had lingered as an
empty, untracked leftover — git does not track empty directories). The
mechanism is documented here for whoever needs it next.

**Citation note (added 2026-08-10):** the 2026-08-10 C++ defect-fix propagation
cherry-picked 11 upstream commits that shifted line numbers in several C++
files (`ScopedFunctionPromoter.{h,cpp}`, `SemContext.cpp`, `SemResolve.cpp`,
`SemanticResolver.cpp`, `SourceErrorManager.cpp`, `JSONParser.{h,cpp}`,
`JSParserImpl-flow.cpp`). Historical/dated sections below are kept as
originally written per this file's history convention, so a citation inside a
section dated before 2026-08-10 references the PRE-cherry-pick tree. Only the
live Imported table's current descriptions are kept synced to the current tree.

## Imported (live differential gate)

| File | Covers |
|---|---|
| `plain.js` | `var x = 1 + 2; print(x);`. Pins two `compile = false` behaviors at once: (1) NO ambient globals — `resolveASTForParser` passes `ambientDecls = nullptr`, so the dump's `Scope %s.1` contains only `x` (declared) and `print` (resolves `UndeclaredGlobalProperty`, not one of `libhermes`'s 63 ambient decls); (2) NO constant folding — `+`/`-` folding is gated on `compile_` (cpp:405-436), so the dump shows the unfolded `BinaryExpression`/`BinOp +` tree via `ASTPrinter`, not a folded `NumericLiteral 3`. Verified: `sema-parser-dump plain.js` exit 0, no stderr; `sema-dump --parser-entry plain.js` byte-identical stdout. This was the corpus's first hermesc-analogue SUCCESS (exit 0) file — the non-degeneracy guard in `run_differential` needs at least one; `compile-false-basics.js` and `flow-annotations.js` are the other two. |
| `error-break-outside-loop.js` | `break;`. Proves "dump despite errors": resolution reports `'break' not within a loop or a switch` (a genuine post-walk `sm_.getErrorCount() != 0` from `SemanticResolver::run`/`run_always`'s SECOND gate, `SemanticResolver.cpp:69` / `resolver/mod.rs`), yet BOTH tools still print the full dump (`Func loose`/`Scope %s.1`/`BreakStatement`) and exit 2. Verified byte-identical: 77 bytes stdout both sides, matching stderr, exit 2 both sides. |
| `error-arrow-rewrite-then-error.js` | `var f = (a) => a + 1;\nbreak;`. Same post-walk gate as above, but AFTER a real rewrite (arrow-function processing, S2 rewrite #1) has already mutated the tree — proves `run_always`'s rebuilt-tree-on-error path carries the rewrite through, not just the original unmodified nodes. Verified byte-identical: 481 bytes stdout both sides, matching stderr, exit 2 both sides. |
| `error-continue-outside-loop.js` | `function f(){ continue; }`. Same post-walk gate, nested one function deep (`continue` outside a loop inside a function body, not at Program scope) — proves the dump-despite-error path also works when the error site is below the top-level function context. Verified byte-identical: 310 bytes stdout both sides, matching stderr, exit 2 both sides. |
| `compile-false-basics.js` | `export default function f(){}`. **S4a Task 3** — moved in from `pending/`. Pins TWO `compile_`-gated behaviors of `visit(ExportDefaultDeclarationNode *)` at once (cpp:1533-1561): no `'export' statement requires module mode` error is emitted (cpp:1534 is `compile_ &&`), and **rewrite #4 does not fire** (cpp:1541 likewise) — the dump shows `ExportDefaultDeclaration` → `FunctionDeclaration`, not the `FunctionExpression` the rewrite would have produced. Verified byte-identical: exit 0 both sides, full dump, empty stderr. This is the corpus's SECOND hermesc-analogue success file |
| `module-imports.js` | `import d, {a as b} from 'm'; import * as ns from 'n';`. **S4a Task 3** — the other side of the module-mode asymmetry: the import error is NOT `compile_`-gated (cpp:876-879), so both declarations error even here, and the tool dumps anyway. That dump is what the DRIVER corpus can never show (hermesc skips the dump on a `resolveAST` failure), so this file is the only pin for `extractIdentsFromDecl`'s `ImportDeclaration` arm (cpp:2334-2347): `Decl %d.N Import` for `d` (`ImportDefaultSpecifier`), `b` (`ImportSpecifier` `_local`) and `ns` (`ImportNamespaceSpecifier`), plus — proving the specifier children walk really runs — `a` (the `ImportSpecifier`'s `_imported`) resolving as an ordinary `UndeclaredGlobalProperty`. Verified byte-identical: exit 2 both sides, matching stdout and stderr |
| `error-invalid-assignment-lvalue.js` | `1 = 2;`. Also the post-walk gate (`ResolverTest.cpp`'s `TestBadAssignmentLValue` confirms "invalid assignment left-hand side" is a `sema::resolveAST`-time check on an already-cleanly-parsed tree, not a parser diagnostic) — see "Gate classification" below for why this file does NOT exercise the entry gate, correcting an initial (hedged, "verify yourself") classification from code review. Verified byte-identical: 165 bytes stdout both sides, matching stderr, exit 2 both sides. |
| `parse-error-recoverable.js` | `"use strict"; var x = 010;`. **S4a final review.** A RECOVERABLE parse error: the lexer reports the strict-mode octal and `parseProgram()` still returns a tree, which `JSParserImpl::parse` then discards via its trailing `if (lexer_.getSourceMgr().getErrorCount() != 0) return None;` (`JSParserImpl.cpp:170-171`) — so the tool's `if (!parsedJs)` (`sema-parser-dump.cpp:115-119`) fires: nothing dumped, exit 2. At the time, the Rust `parse()` had no such gate and returned `Some` here, so `sema-dump` had to apply the error-count check at its own call site; before it did, `--parser-entry` handed the unresolved tree to `sem_dump` and panicked indexing an empty `SemContext` (`sem_context.rs:845`, exit 101). This file is the pin for that fix — and, since parser-phase follow-up (c), for `parse()`'s own `cpp:168-172` gate too, which now makes the same `Some`/nonzero-error-count case unreachable at the source. Verified byte-identical: 0 bytes stdout both sides, 151 bytes stderr both sides, exit 2 both sides. |
| `parse-error-no-ast.js` | `var 1x;`. **S4a final review.** The OTHER no-AST path: a HARD parse error, where `parseProgram()` cannot build a tree at all and `parse()` returns through `if (!res) return None;` (`JSParserImpl.cpp:168-169`) rather than the error-count arm above. Pins that both tools stay silent on stdout, print both diagnostics (the lexer's `invalid numeric literal` and the declaration parser's `'identifier' expected in declaration`) in the same order, and exit 2 — with no `Emitted N errors. exiting.` epilogue on either side (that is the DRIVER pair's contract, not this one's). Verified byte-identical: 0 bytes stdout both sides, 242 bytes stderr both sides, exit 2 both sides. |
| `import-assertions-compile-false.js` | `import 'b.js' with {type:'json'};`. **S4a final review.** (Named apart from the driver corpus's own upstream `import-assertions.js`, which pins the TRUE side of the same gate.) The FALSE side of the `compile_` gate on the import-assertions error (cpp:882-885, `if (compile_ && !importDecl->_attributes.empty())`): the attribute list here is non-empty, yet under `compile = false` the "import assertions are not supported" error is NOT emitted — the only diagnostic is the ungated module-mode one from cpp:876-880. `module-imports.js` cannot see this (no attributes there), so a port that dropped the `compile_ &&` half would pass the whole corpus without this file. The dump also shows the `ImportAttribute` subtree being walked: its key `type` resolves as an ordinary `UndeclaredGlobalProperty`. Verified byte-identical: 242 bytes stdout both sides, 183 bytes stderr both sides, exit 2 both sides. |
| `with-statement.js` | `with (o) { x; }`. **Task 5 (defect-fix propagation).** Was the corpus's first landmine (see below): `Unresolver::visit` (`SemanticResolver.cpp:3206-3224`) marks the body's `x` unresolvable, and a DEBUG `sema-parser-dump` aborted on `getExpressionDecl`'s `assert(!node->isUnresolvable())` (`SemContext.h:559-561`) while this port printed ` UNR` — release C++'s behavior. Upstream `918158cb0` made the C++ dumper guard the call (`SemResolve.cpp:99-110`), so debug now matches release and both match the port. `with` is a `compile_`-gated error, so the DRIVER corpus can never dump a `with` body: this is the only pin for the ` UNR` flag reaching a dump at all, and for the `with` object staying resolved (`Id 'o' [D:E:…]`) because it lies outside the `Unresolver`'s root. Verified byte-identical: 312 bytes stdout both sides, empty stderr both sides, exit 0 both sides — an oracle-success file |
| `anon-export-default.js` | `export default function () {}`. **Task 5 (defect-fix propagation).** The corpus's second landmine, and the other half of `918158cb0`. Rewrite #4 (`SemanticResolver.cpp:1539-1558`) is `compile_`-gated, so under this pair the anonymous `FunctionDeclaration` is never rewritten to a `FunctionExpression`; `visit(FunctionDeclarationNode*)` hoists it unconditionally (the hoist does not check for a name), so a null-`_id` function reaches the `hoistedFunction` printer. Both dumpers used to crash there — C++ on `llvh::cast`'s null check, this port on `print_scope`'s `.expect` — and both now print `hoistedFunction *default*` (`SemContext.cpp:493-501` / `dump_context.rs`'s `print_scope`). Verified byte-identical: 258 bytes stdout both sides, empty stderr both sides, exit 0 both sides — also an oracle-success file. `compile-false-basics.js` is its NAMED counterpart, which keeps printing `hoistedFunction f`, so `*default*` cannot be a blanket replacement |
| `flow-annotations.js` | `// FLAGS: -parse-flow` + `function f(x: number): number { return x; } var y = f(1);`. **S4a final review.** The corpus's only FLAGS-bearing file and its only Flow file: the sole exercise of the C++ tool's `if (parseFlow) ctx.setParseFlow(ParseFlowSetting::ALL)` branch, which was dead before it (spec §5 called for a flow seed here; it never shipped). The type annotations parse into type nodes the resolver walks past without declaring anything, so the dump is the same shape the untyped version would give (`f`/`y` `GlobalProperty`, `x` `Parameter`). The same review taught the C++ tool the `-parse-flow` spelling alongside `--parse-flow` — the FLAGS line is appended verbatim to BOTH binaries' argv, and hermesc's own spelling is the single dash. Resolves clean, so this is also an oracle-success file. Verified byte-identical: 630 bytes stdout both sides, empty stderr both sides, exit 0 both sides. |

## Pending (excluded from the walk — `pending/` subdirectory)

Empty as of S4a Task 3. The one row that lived here,
`compile-false-basics.js`, was blocked on `resolver/mod.rs`'s catch-all
panicking for `Node::ExportDefaultDeclaration`; Task 3 landed the four
module-visit arms (`resolver/modules.rs`) and the file moved into the live
table above.

## Gate

`sema differential (tests/sema_corpus_parser): 13 corpus files matched (5
succeeded on the oracle)`.

History: 7 → **11** files (+4, all from S4a's final review:
`parse-error-recoverable.js`, `parse-error-no-ast.js`,
`import-assertions-compile-false.js`, `flow-annotations.js`),
oracle-succeeded 2 → **3** (+1: `flow-annotations.js` is an exit-0 file; the
other three are error-path pins). Then 11 → **13** files (+2, Task 5 of the
C++ defect-fix propagation plan: `with-statement.js`,
`anon-export-default.js` — the two shapes upstream `918158cb0` unblocked),
oracle-succeeded 3 → **5** (+2: BOTH new files resolve clean and exit 0 on
both sides). Arithmetic: 11 + 2 = 13; 3 + 2 = 5.

The non-degeneracy guard in `run_differential` (at least one oracle success)
is satisfied five times over; the remaining eight are all legitimate
error-path pins (oracle exit 2), same convention as
`tests/sema_corpus/parse-error.js`.

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

Every error file above that produces a dump shows the header, so all of
them hit the POST-WALK gate — none exercises the entry gate. The
mechanism, on the C++ side: `JSParserImpl::parse()`
(`JSParserImpl.cpp:164-172`) ends with
`if (lexer_.getSourceMgr().getErrorCount() != 0) return None;` — so it is
IMPOSSIBLE for `JSParser::parse()` to return `Some(ProgramNode*)` while
`sm.getErrorCount() != 0` from parsing. Since `sema-parser-dump` calls
`resolveASTForParser` immediately after a fresh `parse()` with nothing in
between that could add errors (this pair never loads ambient decls, so
there is no `libhermes`-parse step to fail either), `sm.getErrorCount()` is
0 at that call site whenever it is reached at all, and C++'s entry gate
cannot fire through this tool's call path.

**The same argument now holds on the Rust side too.** Before S4a's final
review, the Rust `parse()` (`parser/src/js/mod.rs`) did NOT port the
`cpp:170-171` error-count check: on a recoverable parse error it returned
`Some` with a nonzero `sm.error_count()`, which walked straight into
`resolve_ast_for_parser`, fired `run_always`'s ENTRY gate (returning the
original, unresolved root) and then panicked in `sem_dump` indexing an
empty `SemContext`, while the C++ tool printed the diagnostic and exited 2
with no dump. `parse-error-recoverable.js` is the pin for that first fix:
`sema-dump` was given the error-count check at its own call site, on BOTH
entry points. Parser-phase follow-up (c) in
`doc/superpowers/RustPortRoadmap.md` has since closed the gap at the
source: `parse()` itself now ports the `cpp:168-172` tail gate, so it is
IMPOSSIBLE — by `parse()`'s own contract, exactly like the C++ paragraph
above — for `parse()` to return `Some(&Node)` while `sm.error_count() != 0`.
The Rust entry gate is therefore unreachable through this tool pair for the
same reason the C++ one is. `sema-dump`'s two call-site checks stay in
place as redundant defense in depth (their comments say so), and
`run_always`'s first branch stays correctly ported for faithfulness and for
any future caller that feeds it a `SourceErrorManager` with preexisting
errors.

(One review round initially guessed `1 = 2;`/"invalid assignment left-hand
side" fires the entry gate, reasoning it might be a parser-level check; it
is not — see `ResolverTest.cpp`'s `TestBadAssignmentLValue`, which calls
`sema::resolveAST` on an already-successfully-parsed `"a + 1 = 10;"` and
expects `false` — confirming the check lives in the resolver, not the
parser, hence the post-walk gate there too.)

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

## For S4b: what the `$SHBuiltin` branches do under `compile = false`

Recorded by the whole-Sema capstone review (2026-08-04, finding F2), because
this pair is the ONLY way to observe it and the answers are not obvious from
`SemanticResolver.cpp:1168-1189`. The three `$SHBuiltin` property branches
of rewrite #3 keep their loud S4b panics in `resolver/calls.rs` through S4a
(spec-sanctioned), so none of these shapes can be a corpus file yet — but
whoever lands S4b should make them into three, and should know the answers
first:

| Input (single statement) | `sema-parser-dump` | What the C++ actually does |
|---|---|---|
| `$SHBuiltin.moduleFactory(0, function(exports){ var inner = 1; });` | exit 0, full dump | `if (compile_) visitModuleFactory(node); return;` — the `return` at cpp:1176 is **outside** the `if`, so under `compile = false` the call is skipped **and the children walk is still skipped**. Observable: the inner `Id 'exports'`/`Id 'inner'` carry NO `[D:E:...]` annotation |
| `var v = 1; $SHBuiltin.export("n", v);` | exit 0, full dump | `visitESTreeChildren` + `visitModuleExport` both run **ungated** by `compile_` (cpp:1182-1187). Observable: the argument `Id 'v'` DOES resolve |
| `$SHBuiltin.import("m");` | exit 2 + dump | `visitModuleImport` runs **ungated** (cpp:1188-1189) and there is no `return`, so the branch falls through to the children walk; the arity error comes from `visitModuleImport` itself |

The trap this closes: an earlier `calls.rs` comment justified the
unconditional `moduleFactory` panic by asserting `compile` is `true` on every
entry into this port's resolver. S4a T2 made that false
(`resolve_ast_for_parser`, `resolve.rs:97`), and an implementer who believed
it could reasonably drop the `if (compile_)` gate, the children-skipping
`return`, or both. The comment is corrected at the site
(`resolver/calls.rs`), which is where an S4b implementer will be reading.

## CLOSED landmine: `with (o) { x; }` — a DEBUG `sema-parser-dump` used to abort

**Closed by upstream `918158cb0`** ("Fix semDump crashes on ASTs resolved for
a parser"), mirrored by Task 5 of the C++ defect-fix propagation plan. The
shape is a live corpus file now — `with-statement.js` in the table above,
oracle exit 0, byte-identical. The section is kept for the history; what
follows describes the state BEFORE that fix.

Also from the capstone review; roadmap landmine (v). `with` is a
`compile_`-gated error, so the DRIVER pair never dumps a `with` body and
never sees this — only this pair does:

```
$ cmake-build-asan/bin/sema-parser-dump with.js          # `with (o) { x; }`
sema-parser-dump: .../include/hermes/Sema/SemContext.h:559:
  ... Assertion `!node->isUnresolvable() && "Attempt to read decl for
  unresolvable identifier"' failed.
Aborted (core dumped)                                          # exit 134

$ sema-dump --parser-entry with.js
... Id 'x' UNR                                                 # exit 0
```

The dumper's `enter(IdentifierNode *)` (`SemResolve.cpp:96-102`) calls
`getExpressionDecl` unconditionally, right after `getDeclarationDecl`, and
`Unresolver::visit` (`SemanticResolver.cpp:3192-3206`) has marked `x`
unresolvable. Unlike the landmine below (and the `computed-fn-name.js` one
in the driver corpus), this port did **not** mirror the abort: the assert is
compiled out under `NDEBUG`, and in that build the call provably returns
`nullptr` (the `Unresolver` always clears the have-expression-decl bit first
via `setExpressionDecl(node, nullptr)`), so reproducing the *value* is
reproducing real Release hermesc. `dump.rs`'s "getExpressionDecl on an
unresolvable identifier" section argues it in full (and now records that the
divergence is retired). The shape stayed out of this corpus only because
there was no C++ output to compare against in a debug build; `918158cb0`
gave it one, and the C++ dumper now guards the call exactly the way this
port always did.

## CLOSED landmine: anonymous `export default function` under `compile = false` — BOTH dumpers used to crash

**Closed by upstream `918158cb0`**, mirrored by Task 5 of the C++ defect-fix
propagation plan: the C++ `printScope` and this port's `print_scope` both
print `hoistedFunction *default*` for a null-id hoisted function instead of
casting/unwrapping unconditionally. The shape is a live corpus file now —
`anon-export-default.js` in the table above, oracle exit 0, byte-identical.
The section is kept for the history; what follows describes the state BEFORE
that fix.

`export default function () {}` could not be added to this corpus, by
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
itself changing. The last clause is what changed: hermesc DID change
(`918158cb0`), so the shape is a corpus file now rather than an excluded
landmine. (`computed-fn-name.js`'s own defect was likewise closed upstream,
by `b351e1184`; see `tests/sema_corpus/MANIFEST.md`, which records it as
re-matching but keeps the minimal `class-field-class-expr.js` as the pin
instead of importing the 18 KB original.)

## Upstream sync task 2: the dump gained `mayReachImplicitReturn`

Upstream `04f1f53a8` (cherry-picked as `1e3806f47`, mirrored by `de917f249`)
appends ` mayReachImplicitReturn` / ` noImplicitReturn` to every
`Func`/`StaticBlock` line. This corpus compares against a live oracle, so
nothing was regenerated; all 13 files still match on all three channels
(**13 matched, 5 succeeded on the oracle**).

This corpus is the ONLY place the `compile = false` answer is pinned, and
upstream's commit message is explicit that it differs from the compile one
because `SemanticResolver` skips its AST rewrites under `compile = false`.
Its coverage of the new token is thin but real: the 13 files' dumps carry
**14 `mayReachImplicitReturn` and 2 `noImplicitReturn`** lines, and the
mutation `ReturnStatement => make_next_statement()` in
`check_implicit_return.rs` is caught here by `flow-annotations.js`
(the driver corpus catches it in 55 of 219). Widening this corpus for
implicit-return shapes specifically is worth doing when
`5ae5260c8` ("Handle try-catch-finally in CheckImplicitReturn",
`CppDefectsFound.md` item 12) lands — that fix is precisely about a
parser-mode implicit-return shape that aborts today, so its arrival is the
natural moment to add try/catch/finally files here.

### `-Xcompile=false` does NOT drop in for `sema-parser-dump`

`04f1f53a8` also adds `hermesc -Xcompile=false -dump-sema`, which was the
hoped-for replacement for this corpus's local-only oracle (see
`doc/superpowers/UpstreamSyncState.md`'s deferred item). Measured with the
cherry-pick in place: **6 of these 13 files differ** — same exit status (2),
but the driver emits an EMPTY stdout where `sema-parser-dump` emits the dump
(`error-arrow-rewrite-then-error.js` 0 vs 521 B,
`error-break-outside-loop.js` 0/100, `error-continue-outside-loop.js` 0/356,
`error-invalid-assignment-lvalue.js` 0/188,
`import-assertions-compile-false.js` 0/265, `module-imports.js` 0/530).
That is the driver's dump-suppression-on-error, and it is exactly what most
of this corpus exists to pin. Recorded here so the swap is not attempted as
a drop-in.
