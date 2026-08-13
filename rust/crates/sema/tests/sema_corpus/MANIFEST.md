# `test/Sema` corpus sweep (S1 Task 8)

Every file below was run as `hermesc -dump-sema <file>` vs `sema-dump <file>`
(no extra flags on either side — matching what `sema_differential.rs` actually
invokes) and classified by byte-for-byte comparison of stdout, stderr and
exit status. "Imported" means the file was copied verbatim into this
directory and is part of the live differential gate. "Deferred" means it
currently panics (unhandled node kind) or genuinely mismatches on
S1-out-of-scope behavior; the reason and target phase are recorded so a later
task can re-run this sweep and pick it up.

The C++ lit file's own `RUN:` line (which may request `-dump-ir`,
`-dump-transformed-ast`, dialect flags, etc.) is irrelevant to this
classification — we always probe with plain `-dump-sema`, because that's the
only thing `sema_differential.rs` tests and the harness has no per-file-flag
support (out of scope to add here, per the task brief).

**Citation note (added 2026-08-10):** the 2026-08-10 C++ defect-fix propagation
cherry-picked 11 upstream commits that shifted line numbers in several C++ files
(`ScopedFunctionPromoter.{h,cpp}`, `SemContext.cpp`, `SemResolve.cpp`,
`SemanticResolver.cpp`, `SourceErrorManager.cpp`, `JSONParser.{h,cpp}`,
`JSParserImpl-flow.cpp`). Historical entries below (sweep-history sections,
dated task write-ups) are kept as originally written per this file's own
"kept for the history" convention, so any `cpp:NNNN`/`SemanticResolver.cpp:NNNN`
citation inside a section dated before 2026-08-10 references the PRE-cherry-pick
tree, not the current one — do not "fix" those in place. Only the live
Imported/Deferred tables' current reasons are kept synced to the current tree.

**Citation note (added 2026-08-12):** the publication track renamed the
publishable Cargo packages to the `hermes-*` family, so the *current* spelling
of a parser gate is `-p hermes-parser` (likewise `-p hermes-ast`,
`-p hermes-support`); `command_line` was renamed the same way on 2026-08-12,
so it is now `-p hermes-command-line`, and `sema` followed on the same day —
it is now `-p hermes-sema`. `-p tools` is unchanged: that crate keeps its bare
name and `publish = false`. Per the same "kept for the history" convention,
historical sections below still show `-p parser`/`-p ast`/`-p sema`; those
blocks also carry the corpus counts of their own date (e.g. "208 / 107" where
the live gate now reports "219 / 109"), so they are records of a past run, not
commands to re-run. The live gate command is the one under "## Gate" above.

**Invocation note (added 2026-08-12):** the same publication change moved the
`sema-dump` driver out of the `sema` crate into the unpublished `tools` crate,
which retired the `dump-bin` feature that used to gate it. The live gate has
no `--features` flag at all any more; every `--features dump-bin` below is
part of a past run's command line and must not be re-run as written.

Total top-level files: 54. Imported **as of the S1 Task 8 sweep**: 15 (14 from
`test/Sema` + 1 new gap-filler, `expr-visit-generic.js`, added in Step 2
below); deferred: 40 (14 + 40 = 54; counting `deep-ast-err.js`, which is listed
but is a vacuous non-gap — see its row's note below). Later tasks move rows out
of Deferred as they unblock them, so the *live* tables below are the source of
truth: after S2 Task 2 the Imported table has 20 rows (19 from `test/Sema` + the
gap-filler) and the Deferred table 35, i.e. 19 + 35 = 54 top-level files still
fully accounted for.

**S2 Task 1** (loops, labels, `break`/`continue`, `switch`) re-ran the sweep
for the files it unblocked and imported three of them
(`label-errors.js`, `for-using-not-supported.js`,
`regress-ast-const-folding.js`); see "S2 Task 1 additions" at the end.

**S2 Task 2** (arrows + rewrite #1, yield/await/spread/meta, the Cover errors)
re-probed the three `ArrowFunctionExpression`-deferred rows and imported two of
them (`await-arrow.js`, `await-arrow-error.js`); see "S2 Task 2 additions" at
the end.

**S2 Task 3** (try/catch + rewrite #2, `with` + the `Unresolver`, the regexp
visit) re-probed the six rows blocked on `TryStatement`/`WithStatement` and
imported three of them (`catch-block.js`, `catch-block-destr.js`,
`catch-block-error.js`); the other three turned out to be blocked on something
else and their rows were corrected. After it the Imported table has 23 rows
(22 from `test/Sema` + the gap-filler) and the Deferred table 32, i.e.
22 + 32 = 54. See "S2 Task 3 additions" at the end.

**S2 Task 4** (classes core: `ClassContext`, `visitClassAsExpr`, class
properties, method definitions, `super`) re-probed the sixteen class-family
rows and imported three of them (`class-children.js`, `field-init-bindings.js`,
`reject-super-references.js`); the other thirteen are blocked on S2 T5
(private names, static blocks) or S2 T6 (`CallExpression`, `super()`) and
their rows were re-classified accordingly. After it the Imported table has 26
rows (25 from `test/Sema` + the gap-filler) and the Deferred table 29, i.e.
25 + 29 = 54. See "S2 Task 4 additions" at the end.

**S2 Task 5** (private names + static blocks) re-probed the nine rows blocked on
`collectDeclaredPrivateIdentifiers` / `StaticBlock` and imported five of them
(`private-names.js`, `private-declaration-dup-error.js`,
`private-name-in-extends-error.js`, `field-value-arguments-error.js`,
`static-initialization-block-error.js`); the other four turned out to be blocked
on `CallExpression` (S2 T6) and their rows were re-classified. After it the
Imported table has 31 rows (30 from `test/Sema` + the gap-filler) and the
Deferred table 24, i.e. 30 + 24 = 54. See "S2 Task 5 additions" at the end.

**S2 Task 6** (`visit(CallExpressionNode *)`: the direct-`eval` detection,
rewrite #3 `$SHBuiltin.prop(...)` → `SHBuiltin`, the `super()` check) is the
single biggest unlock of the sweep: it re-probed the **sixteen** rows blocked on
`CallExpression` and imported **all sixteen** (`arguments-arg-let.js`,
`const-reassignment.js`, `diagnode_errors.js`, `disabled-eval.js`,
`eval-warn.js`, `let-arguments-in-arrow.js`, `private-load-store-error.js`,
`reject-with.js`, `static-initialization-block.js`,
`static-initialization-block-lazy-error.js`, `super-in-arrow.js`,
`super-in-subclass.js`, `super-in-subclass-error.js`,
`undeclared-private-name-error.js`, `valid-super-references.js`,
`var-scope-redeclaration-error.js`). The seventeenth `CallExpression` row,
`xmod-errors.js`, turned out to be blocked on something strictly deeper — the
`$SHBuiltin` CommonJS-module protocol — and was re-classified to S4. After it
the Imported table has 47 rows (46 from `test/Sema` + the gap-filler) and the
Deferred table 8, i.e. 46 + 8 = 54. See "S2 Task 6 additions" at the end.

**S2 Task 8** (the round-2 sweep) re-probed all eight remaining Deferred rows —
**none** unblocked, every stated reason confirmed — and then went at coverage
from the other end: six exhaustive inventories of the dump's own vocabulary,
plus a differential run of both binaries over the 1416 `.js` files in the REST
of `test/` (`Parser`, `IRGen`, `BCGen`, `Optimizer`, `hermes`, `AST`, `Driver`,
`RA`). That found three node kinds the resolver panicked on, one node kind with
no coverage, two missing decl-kind/special pairs, an unapplied `-ferror-limit`,
a wrong `<unknown>:0` render for location-less messages, and a 180-file
PARSER-side diagnostic-geometry gap (left to the parser track, with numbers).
See "S2 Task 8: corpus sweep round 2" at the end; the Imported table is
unchanged at 47 rows and the Deferred table stays at 8, i.e. 46 + 8 = 54.

## Imported (byte-identical vs hermesc)

| File | Note |
|---|---|
| `arguments-var.js` | loose-mode `var arguments` shadowing (IRGen deviation from spec, both sides identical) |
| `assign-arguments.js` | assigning/updating `arguments` — invalid-lvalue errors |
| `assign-eval-loose.js` | assigning `eval` allowed in loose mode |
| `assign-eval-strict.js` | assigning `eval` forbidden in strict mode |
| `directives-3.js` | directive-prologue scanning across `//`, `/* */`, ASI forms, before non-directive statements |
| `function-inline-directive-error.js` | conflicting `'inline'`/`'noinline'` directive warning |
| `function-name-arguments.js` | `function arguments(){'use strict'}` — strict declare-arguments error |
| `function-redeclaration.js` | function declaration redeclaration/merge shapes |
| `optional-chaining.js` | `?.` optional member expression (no optional call) |
| `param-redeclaration-error.js` | parameter-then-`let` redeclaration error |
| `restricted-global-error.js` | `let undefined;` at global scope — restricted-global-shadow error |
| `restricted-global-nested.js` | restricted global name shadowed inside a nested (non-global) scope — allowed |
| `restricted-global-var.js` | `var` (not `let`) shadowing a restricted global — allowed |
| `var-scope-redeclaration.js` | `var` redeclared across nested function/block scopes — allowed shapes |
| `expr-visit-generic.js` | **new file, not from `test/Sema`** — Step 2 gap-filler, see below |
| `for-using-not-supported.js` | **S2 T1** — `using` declarations in `for-in`/`for-of` heads (the explicit rejection in `extractIdentsFromDecl`) |
| `label-errors.js` | **S2 T1** — every `break`/`continue`/label error shape, including the two error+note pairs |
| `regress-ast-const-folding.js` | **S2 T1** — `for (w of (1 + 1))`: a fold in a `for-of` right-hand side |
| `await-arrow.js` | **S2 T2** — `let await` referenced from a nested arrow inside an async arrow's parameter default |
| `await-arrow-error.js` | **S2 T2** — the three `await is not a valid identifier name in an async function` shapes (`forbidAwaitAsIdentifier_` through nested arrow params) |
| `catch-block.js` | **S2 T3** — `try {} catch (e) { let x; }`: the ES5Catch decl and the clause's two nested scopes |
| `catch-block-destr.js` | **S2 T3** — a destructured catch parameter (plain `Catch` decls) |
| `catch-block-error.js` | **S2 T3** — the `Catch`-vs-`let` redeclaration error+note pair in the clause's own scope |
| `class-children.js` | **S2 T4** — the untyped class path: the class scope's `ClassExprName`, the two decls on one `Identifier`, and the three synthetic `FunctionInfo`s a field + a method + no constructor produce |
| `field-init-bindings.js` | **S2 T4** — field initializers resolving against an enclosing *function*'s scope, so the synthetic initializer functions are that function's children |
| `reject-super-references.js` | **S2 T4** — every `super not allowed here` shape (`visit(SuperNode *, Node *)`), including the computed-key `canReferenceSuper_ = false` cases and `delete super.x` |
| `private-names.js` | **S2 T5** — all five private `Decl` kinds, including the `PrivateGetterSetter` a legal accessor pair collapses to |
| `private-declaration-dup-error.js` | **S2 T5** — five rows of the ES2024 15.7.1 duplicate-private-name matrix plus the static/non-static accessor mismatch |
| `private-name-in-extends-error.js` | **S2 T5** — `the private name "#foo" was not declared in any enclosing class`: the superclass expression is visited BEFORE the private names are declared (cpp:936-939) |
| `field-value-arguments-error.js` | **S2 T5** — `invalid use of 'arguments'` in a PRIVATE field initializer (the `ClassPrivateProperty` half of the flag save/restores), which is what kept this file deferred after S2 T4 |
| `static-initialization-block-error.js` | **S2 T5** — a `let`/`let` redeclaration inside a static block, i.e. the block's own body scope |
| `arguments-arg-let.js` | **S2 T6** — `let arguments` in a function that also has a parameter named `arguments` |
| `const-reassignment.js` | **S2 T6** — every `const` reassignment shape, reported through calls |
| `diagnode_errors.js` | **S2 T6** — the `$SHBuiltin`-free half of the diag-node tests |
| `disabled-eval.js` | **S2 T6** — a bare `eval("print(1)")` at global scope. Its `RUN:` lines want `-enable-eval=false`, which the harness has no flag support for, so what this pins is the **enabled** (`DirectEval`) branch; the `EvalDisabled` branch is unit-tested instead — see the note below |
| `eval-warn.js` | **S2 T6** — two direct `eval()` calls inside functions, one with extra arguments, i.e. the `DirectEval` warning over the callee's range |
| `let-arguments-in-arrow.js` | **S2 T6** — `let arguments` referenced from inside an arrow |
| `private-load-store-error.js` | **S2 T6** — the private load/store restrictions as `test/Sema` writes them (with `sink(...)` calls); the call-free `error-private-load-store.js` from S2 T5 remains the exhaustive version |
| `reject-with.js` | **S2 T6** — `with` plus a call in its body |
| `static-initialization-block.js` | **S2 T6** — a static block whose body calls `sink(y)`; also the first corpus file to need `IfStatement` (added to `visit_node`'s override-free generic arm by this task) |
| `static-initialization-block-lazy-error.js` | **S2 T6** — `forbidArgumentsAsIdentifier_` reached through three nested arrows in a static block. Its `RUN:` line also wants `-lazy`, which the harness ignores; the diagnostics are identical without it |
| `super-in-arrow.js` | **S2 T6** — `super()` through one and two levels of arrow inside a derived constructor (`nearestNonArrow`) |
| `super-in-subclass.js` | **S2 T6** — `super()` two arrows deep in a derived constructor |
| `super-in-subclass-error.js` | **S2 T6** — four `super() call only allowed in derived class constructor` shapes: a base-class constructor, an arrow inside one, a plain function inside a *derived* constructor, and an object-literal method |
| `undeclared-private-name-error.js` | **S2 T6** — `the private name "#x" was not declared in any enclosing class`, as `test/Sema` writes it |
| `valid-super-references.js` | **S2 T6** — every legal `super.x` shape, including the two field initializers that call an IIFE (which is what needed `CallExpression`) |
| `var-scope-redeclaration-error.js` | **S2 T6** — `var` redeclaration errors across `try`/`catch` with a call in the body |
| `break-in-nested-func.js` | **S3 T2** — `break;` inside a loose-mode block-nested `FunctionDeclaration`: the promoter still runs (and would promote `foo`), but `break`'s own loop/switch-nesting check fires first, so this pins that ordering rather than promotion itself |
| `function-redeclaration-error.js` | **S3 T2** — sixteen redeclaration shapes (`var`/`let`/`Catch`-vs-`function`, strict AND loose, block-nested AND top-level) crossed with the Annex B.3.3 loose-mode exception; several of the loose, block-nested pairs (e.g. `var b2; function b2(){}`) are exactly the `visit(VariableDeclarationNode *)` "already declared" shape `promotion-var-shadows-promoted.js` isolates on its own |
| `regress-function-promotion-decl.js` | **S3 T2** — the canonical positive case: one block-nested `function inner(){}` promoted to `Var`, alongside a `let foo` sibling that is untouched |
| `type-alias-children.js` | **S4a T1** — **upstream + `// FLAGS: -parse-flow` line prepended, so NOT byte-identical to the upstream source** (only the dump output is byte-for-byte vs hermesc, per this table's usual methodology); with `-parse-flow` actually enabling the Flow grammar, pins `visit(TypeAliasNode *)`'s true no-op (SemanticResolver.cpp:1579-1581, newly ported to `resolver/mod.rs`'s `visit_node`, cited there) — the alias's `_id`/`_right` children are never visited, so `Id 'A'`/`Id 'B'`/`GenericTypeAnnotation` appear in the dump with no `[D:E:...]` resolution annotations, which is the file's whole point ("children of type alias AST node are not resolved as variables") |
| `flow-typecast-cover.js` | **S4a T4** — `visit(CoverTypedIdentifierNode *)` (SemanticResolver.cpp:1575-1577, `resolver/expressions.rs:966`). `(x: number)` alone does NOT reach this visit — JSParserImpl.cpp:2633-2640 rewrites a non-optional cover node with a type annotation into a `TypeCastExpressionNode` inside the parenthesized-expression parser itself; the OPTIONAL form (`x?: number`, `_optional = true`) skips that rewrite and survives as a real `CoverTypedIdentifierNode` when it is not consumed as arrow parameters, which is what `(x?: number);` pins. hermesc: exit 2, 1 error |
| `flow-this-param.js` | **S4a T4** — `declareParams`'s `this`-parameter check (SemanticResolver.cpp:1767-1771, `resolver/functions.rs:897`), gated `compile_ && !typed_`: `function f(this: number) {}` under `-parse-flow` parses (Flow accepts a `this` parameter) but the untyped dialect rejects it in sema. hermesc: exit 2, 1 error |
| `flow-annotations-benign.js` | **S4a T4** — negative control: parameter, return and variable type annotations under `-parse-flow` resolving completely cleanly — the annotation nodes are never visited as expressions, so they neither perturb declarations nor scopes. hermesc: exit 0, full dump match |
| `flow-typecast-resolves.js` | **S4a T4 fix review** — `visit(TypeCastExpressionNode *)` (SemanticResolver.cpp:1591-1594, `#if HERMES_PARSE_FLOW`, `resolver/expressions.rs`'s `visit_type_cast_expression`). A **review-found gap**: `(x: number);`, the task brief's original (unverified) sketch for `flow-typecast-cover.js`, does not hit the Cover-node error at all — it is the parser's rewritten `TypeCastExpressionNode` (JSParserImpl.cpp:2633-2640) and resolves cleanly, but that visit had no port and the resolver panicked at the catch-all. Ported and pinned here: `x` is declared first, so the dump shows it resolving through the cast normally ("visit the expression, but not the type annotation"). hermesc: exit 0 |
| `flow-as-expression.js` | **S4a T4 fix review** — `visit(AsExpressionNode *)` (SemanticResolver.cpp:1596-1599, `#if HERMES_PARSE_FLOW`, `resolver/expressions.rs`'s `visit_as_expression`), the same shape as `flow-typecast-resolves.js` for Flow's `as` operator (`x as number`, JSParserImpl.cpp:4329-4350) — also unconditional on `typed_`, also found panicking during the fix review. hermesc: exit 0 |
| `invalid-args-eval.js` | **Task 5 (defect-fix propagation)** — the S1 `arguments`/`eval` declaration rules, upstream verbatim. Deferred since S1 on a same-location diagnostic-order tie at `89:9` (the strict-mode `cannot declare 'arguments'` error and the `was not declared in function "global"` warning), which C++'s `std::sort` over the buffered-message array left unspecified. Upstream `5f313a13a` made that a `std::stable_sort` (`SourceErrorManager.cpp:60-74`), matching this port's stable `sort_by_key` in `disable_buffering` (`support/src/manager.rs`), so both sides now break the tie in emission order and the match is by construction. hermesc: exit 2 (error-path file, not an oracle success) |

`deep-ast-err.js` is listed in the Deferred table below but is NOT a real S1
gap: the entire `.js` file is comment lines (its `RUN:` lines generate the
actual `1 + 1 + ...` stress input via a shell `echo` pipe, never written into
the file itself), so running `-dump-sema` on the file as-is degenerates to an
empty program that trivially matches. Importing it would inflate the
"matched" count without testing anything, so it stays out of `sema_corpus/`;
it's kept in the table purely for accounting completeness (54 top-level
files in, 54 accounted for below) rather than silently dropped. It is also
`UNSUPPORTED: linux && asan` upstream.

## Deferred

| File | Blocking construct | Target phase |
|---|---|---|
| `deep-ast-err.js` | vacuous — see note above (not a real S1 gap) | n/a |
| `xmod-errors.js` | the `$SHBuiltin` CommonJS-module protocol: `visitModuleFactory`/`visitModuleExport`/`visitModuleImport` (cpp:1334-1467), reached from the three property-name branches of rewrite #3 (cpp:1183-1204). `CallExpression` itself landed in S2 T6, which ports those three branches as loud phase-tagged panics — its row was re-classified from "`CallExpression` / S2" accordingly. Every diagnostic in the file (`$SHBuiltin.moduleFactory requires exactly two arguments.` and 17 more) comes from those three functions | S4 modules |

## Subdirectories (`test/Sema/flow/`, `test/Sema/flow/ffi/`, `test/Sema/lowering/`)

`test/Sema/flow/` (178 files across `flow/` and `flow/ffi/`: 173 + 5) and
`test/Sema/lowering/fastarray-push.js` all require `-typed`/`-parse-flow` (or
`-parse-ts`) just to parse their Flow/TS type syntax (e.g. `var x:
number[];`). `sema_differential.rs` has no per-file-flag mechanism (adding one
is explicitly out of scope for this task — see the task brief), and these
aren't S1-scope constructs regardless (typed dialects are their own future
track, not an S1/S2/S3 phase). Deferred in bulk rather than itemized
per-file:

- **Blocking construct:** Flow/TS typed-dialect syntax (`-typed`, `-parse-flow`, `-parse-ts`)
- **Target:** typed-dialect sema track (separate from the untyped S1-S5 roadmap); revisit once the differential harness supports per-file dialect flags

## Step 2: corpus coverage of S1 features (Tasks 4-7)

Grepped the corpus (pre-sweep, 54 files) for every node kind Tasks 4-7 claim
to support (the override-free generic-dispatch whitelist in
`resolver/mod.rs`'s `visit_node`, cpp:200-304 inventory) and found four kinds
with zero real exercise anywhere in the corpus: `ConditionalExpression`,
`LogicalExpression`, `SequenceExpression` and `TemplateLiteral`/
`TemplateElement` (a few files had backticks, but only inside `//` comments —
no actual template-literal node ever appeared). All four rely on the generic
`visit_children_mut` rebuild rather than a dedicated visit method, so a bug in
that shared rebuild path could exist without any file catching it.

Added `expr-visit-generic.js` exercising all four in one file (`a ? b : c`,
`a && b || c`, `(a, b, c)`, `` `x=${a} y=${b}` ``), verified byte-identical
against `hermesc -dump-sema` before being added to the corpus. Every other S1
feature (var/let/const, destructuring incl. defaults/rest/nested, blocks,
binary/unary + folding, assignment/update, function decls/exprs, parameter
scopes incl. the dual-scope layout, `arguments`, `return`, directives,
restricted globals, redeclaration-error shapes) already had corpus coverage
before this sweep (now reinforced by the 14 files imported above).

## Gate

```
REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml \
    -p hermes-sema --test sema_differential -- --nocapture
```

Final count after S1 Task 8: **69 corpus files matched** (54 pre-existing + 14
imported from `test/Sema` + 1 new gap-filler; 42 succeed on hermesc, 27 are
hermesc-failure files compared byte-for-byte on the error path).

Final count after S2 Task 1: **80 corpus files matched** (48 succeed on
hermesc, 32 are hermesc-failure files).

Final count after S2 Task 2: **99 corpus files matched** (58 succeed on
hermesc, 41 are hermesc-failure files).

Final count after S2 Task 3: **107 corpus files matched** (63 succeed on
hermesc, 44 are hermesc-failure files).

Final count after S2 Task 4: **120 corpus files matched** (69 succeed on
hermesc, 51 are hermesc-failure files).

Final count after S2 Task 5: **134 corpus files matched** (72 succeed on
hermesc, 62 are hermesc-failure files).

Final count after S2 Task 6: **157 corpus files matched** (86 succeed on
hermesc, 71 are hermesc-failure files). S2 Task 7 (`CheckImplicitReturn`) added
no corpus file — the flag it computes is not printed by `-dump-sema`, see the
S2 Task 8 section.

Final count after S2 Task 8: **160 corpus files matched** (88 succeed on
hermesc, 72 are hermesc-failure files).

Final count after S3 Task 1: **162 corpus files matched** (90 succeed on
hermesc, 72 are hermesc-failure files).

Final count after S3 Task 2: **172 corpus files matched** (96 succeed on
hermesc, 76 are hermesc-failure files) — see "S3 Task 2 additions" below for
the two-step arithmetic (171 then 172) that reaches this number; this line
was missing from the table and is filled in retroactively by S3 Task 3.

Final count after S3 Task 3: **172 corpus files matched** (96 succeed on
hermesc, 76 are hermesc-failure files) — unchanged; the upstream re-probe
(see "S3 Task 3 additions" below) imported no new `test/Sema` files.

Final count after the S3 final-review follow-up: **173 corpus files
matched** (97 succeed on hermesc, 76 are hermesc-failure files) — see "S3
final-review follow-up" below.

Final count after S4a Task 1: **176 corpus files matched** (100 succeed on
hermesc, 76 are hermesc-failure files) — see "S4a Task 1: the `// FLAGS:`
per-file harness" below.

Final count after S4a Task 3: **187 corpus files matched** (100 succeed on
hermesc, 87 are hermesc-failure files) — see "S4a Task 3: the module visits"
below; this line was missing from the table and is filled in retroactively by
S4a Task 4, same convention as the S3 Task 2/3 pair above.

Final count after S4a Task 4: **190 corpus files matched** (101 succeed on
hermesc, 89 are hermesc-failure files) — see "S4a Task 4: the untyped
`-parse-flow` corpus battery" below.

Final count after S4a Task 4's fix review: **192 corpus files matched** (103
succeed on hermesc, 89 are hermesc-failure files) — see "S4a Task 4 fix
review" below.

Final count after S4a Task 5: **unchanged at 192 corpus files matched** (103
succeed on hermesc, 89 are hermesc-failure files) — the upstream re-probe
imported no new `test/Sema` row (all four remaining Deferred rows stayed
blocked on their existing reasons) and fixed no code (zero S4a-attributable
panics were found). See "S4a Task 5: upstream re-probe" below.

## S2 Task 1 additions

Eight new files, each verified byte-for-byte (stdout, stderr and exit status)
against `hermesc -dump-sema` before being added:

| File | Covers |
|---|---|
| `loop-shapes.js` | all five `LoopStatement` kinds, the scope each creates (`while`/`do-while` create none) and the declarations hoisted into it, at global scope and inside a function |
| `labeled-loops.js` | labeled loops, label reuse after the labeled statement is left (the `make_scope_exit` erase), a label-enclosing-a-label-enclosing-a-loop, a non-loop label targeted by `break`, and a per-function `labelMap` |
| `switch-shapes.js` | a `switch` with no declarations (scope created, not populated) vs. one with `let`/`const` cases, a discriminant resolved *outside* the switch scope, a folding discriminant (`switch (1 + 2)` — rebuilds the node), and a `var` hoisted out of a case |
| `for-in-of-init.js` | the allowed `for (var x = 1 in o)` loose-mode row, plus the four `validateAssignmentTarget` left-hand-side shapes (identifier, member, array and object pattern) |
| `error-for-in-of-init.js` | every error row of the initializer matrix: destructuring + init, `let`/`const` + init, `for-of` + init, and the strict-mode loss of the loose `var` exception |
| `error-for-decl-strict.js` | a `var` in a `for` init still hoisting to function scope for `validateDeclarationName` (extracted from `invalid-args-eval.js` — see its Deferred row) |
| `error-break-across-function.js` | `labelMap`/`currentLoop`/`currentLoopOrSwitch` not crossing a function boundary — all four error shapes |
| `loop-await-of.js` | `for await (... of ...)` inside an async function |

`break-in-nested-func.js` was re-probed and stays deferred: it needs the
loose-mode `ScopedFunctionPromoter` (S3), which is why
`error-break-across-function.js` uses function *expressions* to reach the same
resolver paths today.

## S2 Task 2 additions

Seventeen new files plus the two re-probed `test/Sema` imports
(`await-arrow.js`, `await-arrow-error.js`), each verified byte-for-byte (stdout,
stderr and exit status) against `hermesc -dump-sema` before being added:

| File | Covers |
|---|---|
| `arrows-basic.js` | **rewrite #1** — the dump shows the synthesized `BlockStatement`/`ReturnStatement` for every expression-bodied arrow, byte-compared; single-token params, block bodies (untouched), curried arrows, defaults, destructuring params, rest params |
| `arrows-arguments.js` | an arrow's `arguments` resolving through `nearestNonArrow` to the enclosing function's declaration (and to a global property at top level) |
| `arrows-async-await.js` | `async` arrows with `await` in the body, an async arrow nested in an async function, and a plain arrow beside it |
| `arrows-param-expressions.js` | an arrow with parameter expressions — the dual parameter/body scope layout, `arguments` NOT declared in either, and folds inside the defaults (the "fold inside an arrow" shape the S1 capstone asked for) |
| `generators-yield.js` | `visit(YieldExpressionNode *)` in real generators: `yield` with and without an argument, `yield*`, a folding argument, generator function expressions and a nested generator |
| `error-yield-in-formal-param.js` | `'yield' not allowed in a formal parameter` (`isFormalParams`) |
| `error-await-in-formal-param.js` | `'await' not allowed in a formal parameter` (ES14.0 15.8.1) |
| `error-async-generator.js` | **S1-capstone pin** — `async generators are unsupported`, i.e. the `ENABLE_ASYNC_GENERATORS` constant's branch, for both a declaration and an expression |
| `new-target.js` | `new.target` accepted: in a function, in a function expression, and in arrows nested one and two deep inside a function (`nearestNonArrow`) |
| `error-new-target-global.js` | both `new.target` error shapes — at global scope BOTH fire (`isGlobalScope()` *and* `nearestNonArrow(global) == globalFunction`), inside a global arrow only the second |
| `error-import-meta.js` | `'import.meta' is currently unsupported` (the `compile_`-guarded branch) |
| `spread-shapes.js` | the reachable arms of `visit(SpreadElementNode *)`'s parent whitelist: `ArrayExpression`, `ObjectExpression`, `NewExpression`, and a nested spread |
| `error-cover-nodes.js` | all four non-Flow `Cover*` error stubs: `( )`, `(1, )`, `({ p = 1 })`, `(...e)` — including the one that reports at `getStartLoc` rather than over a range |
| `arrows-nested.js` | a chain of nested arrows (each level rewritten), `arguments` reached past another arrow and past a function expression that has its own, and `for await (... of ...)` inside an async arrow |
| `error-arrows.js` | duplicate arrow parameters (an error even in loose mode — `uniqueParams` is unconditionally true for arrows, cpp:1755-1756), a parameter/`let` collision in the body, and `'use strict'` inside an arrow with a non-simple parameter list |
| `error-arrows-strict.js` | `arguments`/`eval` as arrow parameter names in strict mode |
| `function-expr-name-fold.js` | **S1-capstone pin** — a NAMED `FunctionExpression` whose body folds, so the node carrying the function-expression-name scope decoration is rebuilt |

Not corpus-reachable, and documented at their sites rather than curated away:

- **`spread operator is not supported`** (cpp:1465). `JSParserImpl` builds a
  `SpreadElementNode` in exactly three places, and all of their parents are on
  the whitelist; `...` anywhere else is an `invalid expression` parse error or
  gets reinterpreted into a `RestElement`/`CoverRestElement`. Probed with
  `import(...a)`, `switch (...a)`, `` tag`${...a}` `` and the destructuring
  reinterpret paths. `CallExpression`/`OptionalCallExpression` are whitelisted
  parents that were still deferred when this note was written; **S2 T6
  RESOLVED that** — `calls-shapes.js` now exercises `f(...a)`, `f(1, ...a, 2)`,
  `f?.(...a)` and `new f(...a)`. So **all five** whitelisted parents are live:
  `ArrayExpression`, `ObjectExpression` and `NewExpression` since S2 T2
  (`spread-shapes.js`'s `[1, ...b, 2]`, `{ p: 1, ...d }` and `new f(...a)`),
  `CallExpression` and `OptionalCallExpression` since S2 T6. (This sentence
  used to claim "four of the five ... `ArrayExpression` is the fifth", which
  double-counted the two call kinds as one and left `ObjectExpression`
  unnamed; S2 T8 re-derived the coverage from the AST dumps — the
  `SpreadElement` under `spread-shapes.js`'s `ObjectExpression` is right
  there in hermesc's output — and corrected the arithmetic.)
- **`invalid meta property X.Y`** (cpp:868-871). The parser only builds a
  `MetaProperty` after matching `new` `.` `target` / `import` `.` `meta`
  exactly, and reports `'target'/'meta' expected in member expression`
  otherwise (probed: `new.foo`, `import.foo`).
- **`'yield' not in a generator function` / `'await' not in an async
  function'`** (cpp:1480, 1496). Both need a `YieldExpression`/
  `AwaitExpression` whose *enclosing* function context is not a
  generator/async one, which the parser only produces inside class field
  initializers and static blocks (`test/Parser/await-field-error.js`,
  `test/Parser/class-static-block-await-error.js`) — i.e. once S2 T4/T5 land
  the class visits. **S2 T4 RESOLVED the `await` half** by importing
  `test/Parser/await-field-error.js`, and established that the `yield` half is
  **NOT reachable at all**: the parser rejects `yield` in a class field
  initializer as `invalid expression` before sema ever sees a
  `YieldExpression` there (`test/Parser/yield-field-error.js`, also imported,
  pins that). Whether a static block can reach it is S2 T5's to re-check
  (`test/Parser/class-static-block-yield-error.js` suggests the parser rejects
  that too). `CoverTypedIdentifier` likewise needs `-parse-flow`
  (dialect-corpus phase).

## S2 Task 3 additions

Five new files plus the three re-probed `test/Sema` imports
(`catch-block.js`, `catch-block-destr.js`, `catch-block-error.js`), each
verified byte-for-byte (stdout, stderr and exit status) against
`hermesc -dump-sema` before being added:

| File | Covers |
|---|---|
| `try-catch-finally.js` | **rewrite #2** — `try`/`catch`, `try`/`finally` and `try`/`catch`/`finally`, the last of which the dump shows as two nested `TryStatement`s wrapped in a synthesized `BlockStatement` with its own (empty) scope; nested rewrites, a `var` hoisted out of a `try` body, `throw` in both a `try` body and a handler (`ThrowStatement`, added to `visit_node`'s override-free generic arm by this task), and an outer `try`/`finally` whose *inner* statement is rewritten while a fold in the innermost body rebuilds the whole spine |
| `catch-shapes.js` | every catch-parameter shape: a simple binding (`ES5Catch`), array and object patterns incl. defaults/rest/nesting (plain `Catch`), the optional (absent) binding, the ES10 B.3.5 `var`-merges-with-a-simple-catch-binding case (which also exercises the `[D:%d.N E:%d.M]` side-table dump), a `let` shadowing the param in the body block, and a fold in a catch body (which rebuilds the `CatchClause` and must not lose its `scope`) |
| `error-catch-redecl.js` | the `Catch`/`ES5Catch` rows of the redeclaration decision table, now reachable: `let`/`const` in the clause's own scope, a destructured binding vs `var` in the body (no B.3.5 exception), two names bound by one catch parameter, and a nested-block `let`/`let` |
| `error-with.js` | `with statement is not supported`, at `getStartLoc` (a caret, not a range), twice — hermesc exits 2 without printing any dump, which is why the `Unresolver` is unit-tested rather than differentially tested |
| `regexp-literals.js` | valid `RegExpLiteral`s of every flavor (classes, quantifiers, all flags, named/non-capturing groups, escapes, lookaround, one inside a function) — see the deferral note below |

Not corpus-reachable, and documented at their sites rather than curated away:

- **`Invalid regular expression: <engine error>`** (cpp:829-832) needs Hermes's
  regex engine (`lib/Regex/` + `CompiledRegExp::tryCompile`), which is a
  separate unported component. `resolver/expressions.rs`'s stub validator
  accepts everything, so an invalid-regex file (`var re = /a(/;` → hermesc:
  `Invalid regular expression: Parenthesized expression not closed`, exit 2)
  cannot be matched and is **deferred to the regex component**. Valid regexes
  are unaffected, which is what makes `regexp-literals.js` a real test — see
  that file's header and the module doc's "REGEX-ENGINE DEFERRED" block.
- **The `Unresolver`'s local-`eval` call site** (cpp:1931-1937) is dead in C++
  too (`if (false && lexScope->localEval && ...)`), so only the `with` call
  site exercises the pass, and that one is dump-invisible (above).

## S2 Task 4 additions

Seven new files, the three re-probed `test/Sema` imports (`class-children.js`,
`field-init-bindings.js`, `reject-super-references.js`) and three imports from
`test/Parser` (the first files this corpus takes from there — the MANIFEST's
own S2 T2 note named `await-field-error.js` as the pin candidate for a
diagnostic no `test/Sema` file can reach). Each was verified byte-for-byte
(stdout, stderr and exit status) against `hermesc -dump-sema` before being
added:

| File | Covers |
|---|---|
| `classes-shapes.js` | every shape `visitClassAsExpr` handles: declarations and expressions, named and anonymous, self-reference through the inner `ClassExprName` decl, every `MethodDefinition` kind (plain, computed, getter, setter, static, static computed, generator, `async`), classes nested in methods and in functions, a class whose only constructor is the SYNTHETIC implicit one vs. one with an explicit constructor (which suppresses it), and a method body that folds (rebuilding the class node) |
| `class-properties.js` | `visit(ClassPropertyNode *)`: instance-only, static-only, both (in either order, so both creation orders of the two synthetic initializer functions are pinned) and neither; a field with no initializer (which still creates the initializer function in untyped mode but runs no `declareArguments`, so its scope has no `arguments` decl) vs. one with; computed keys resolved in the ENCLOSING context; `this`, an arrow and a fold inside an initializer; fields in a class inside a function; and `arguments` in a computed key, which is legal precisely because no `FunctionContext` is pushed for it |
| `classes-derived.js` | `extends` of an identifier, a member expression, a folding sequence expression and a class expression; `super.x` in a method, a static method, a getter, a setter, an arrow inside a method, a doubly-nested arrow and a field initializer (`canReferenceSuper_` inheritance, cpp:1027/1675); anonymous and function-local derived classes. `super()` CALLS are deliberately absent — that check is S2 T6 |
| `error-class-name.js` | class-name errors: duplicate class, `let`-then-class, `class arguments`/`class eval` (reachable at loose global scope only because a class forces strict mode on the enclosing function, cpp:919), the same for a class *expression* name, and assignment/`+=`/`++` to the class name from inside the body — the inner `ClassExprName` decl's const rules |
| `error-class-decorators.js` | all three `decorators are not supported` sites (cpp:914-916 on the class, cpp:1009-1011 on a `ClassProperty`, cpp:1097-1099 on a `MethodDefinition`), for declarations and expressions, instance and static members, and a class with two decorators (which reports once) |
| `error-class-field.js` | the two errors a field initializer's flag save/restores produce: `invalid use of 'arguments'` (`forbidSpecialArgumentsReference_`, including through one and two levels of arrow, at global scope and inside a function) and `'await' not in an async function` (`forbidAwaitExpression_`, even inside an `async` function, for instance and static fields) — plus the contrast that `await` in a COMPUTED KEY inside an `async` function is legal, because the key is resolved in the async function's own context |
| `super-member-shapes.js` | `visit(SuperNode *, Node *)`'s `isa<MemberExpressionLikeNode>` test — the reachable (`MemberExpression`) half of the range, in several nesting shapes including `super.a?.b`; the `OptionalMemberExpression` half is **unreachable in Hermes's grammar** (see the note below) — and `canReferenceSuper_` coming from `isMethodDefinition` on OBJECT-literal method shorthand (plain, getter, setter, computed, generator, `async`, and an arrow inside one) — all legal, so it is the non-error counterpart to `reject-super-references.js` |
| `await-field-error.js` | **S2 T2 pin, from `test/Parser`** — `'await' not in an async function` (cpp:1496), unreachable before the class visits existed |
| `arguments-field-error.js` | from `test/Parser` — `invalid use of 'arguments'` reaching a field initializer through an arrow, inside a generator |
| `yield-field-error.js` | from `test/Parser` — evidence that the `'yield' not in a generator function` pin is unreachable (the parser rejects `yield` in a field initializer first) AND the regression pin for this task's parser fix: C++ reports that `invalid expression` through `error(SMLoc, Twine)` (a bare caret), which this port was rendering as an underlined 5-character range |

Not corpus-reachable, and documented rather than curated away:

- **The `OptionalMemberExpression` half of `visit(SuperNode *, Node *)`'s
  `isa<MemberExpressionLikeNode>(parent)` range** (cpp:1089). A `Super`'s
  parent can never be an `OptionalMemberExpression`: the parser requires `(`,
  `[` or `.` immediately after `super` (`super?.a` is `'(', '[' or '.'
  expected after 'super' keyword`), and in `super.a?.b` the
  `OptionalMemberExpression` wraps a plain `MemberExpression` whose `_object`
  is the `Super` (verified with `hermesc -dump-ast`). The dead sub-case exists
  in the C++ `isa<>` range test too, so the condition is ported verbatim
  rather than narrowed; `classes.rs`'s `visit_super` says so at the site.

- **A class expression inside a class field initializer** — *SUPERSEDED:
  fixed upstream in `b351e1184` and mirrored here; see "C++ defect-fix
  propagation, Task 4" at the end of this file. The shape below is now the
  corpus file `class-field-class-expr.js`.* (`class C { x =
  class {}; }`) makes **hermesc itself abort** on an assertion in the C++
  dumper: `SemContext.cpp:478: printFunction: Assertion 'processedCount ==
  f.getScopes().size() && "not all scopes were visited"' failed`. The inner
  class's `LexicalScope` is created with `parentFunction = curFunctionInfo()`
  (the synthetic elements-initializer function) but `parentScope = curScope_`
  (the OUTER class's scope), so `SemContextDumper`'s recursive scope walk
  never reaches it from the initializer function's body scope. This port
  reproduces the bug faithfully — `dump_context.rs`'s matching
  `assert_eq!(processed_count, ...)` fires with the same message — but the two
  abort messages can never be byte-identical (one names a C++ source path), so
  the shape stays out of the corpus. Both `class-properties.js` and
  `classes-derived.js` were trimmed to avoid it. It is a pre-existing C++
  defect, NOT a port gap; a release hermesc (no assertions) dumps the
  incomplete scope tree instead.

## S2 Task 5 additions

Six new files, the five re-probed `test/Sema` imports (`private-names.js`,
`private-declaration-dup-error.js`, `private-name-in-extends-error.js`,
`field-value-arguments-error.js`, `static-initialization-block-error.js`) and
three more imports from `test/Parser` (following the precedent S2 T4 set — the
`class-static-block-*` files are where the static-block diagnostics live, since
`test/Sema`'s own static-block files all need `CallExpression`). Each was
verified byte-for-byte (stdout, stderr and exit status) against
`hermesc -dump-sema` before being added:

| File | Covers |
|---|---|
| `private-members.js` | every legal private-name shape: fields with and without an initializer, instance and static; methods incl. generator and `async`; a getter+setter pair in BOTH declaration orders plus a static pair (so both `PrivateGetterSetter` upgrade orders are pinned); getter-only and setter-only; private access as `this.#x`, `o.#x`, `o?.#x` and the ES2022 `#x in o` check (whose `PrivateName` reaches `visit(PrivateNameNode *)` without a member expression); a member referencing a private name declared LATER in the class (which is the whole reason `collectDeclaredPrivateIdentifiers` runs before the body walk); private fields in nested classes with the same spelling; and the derived-class and class-expression forms; plus two private field initializers that FOLD, rebuilding the `ClassPrivateProperty` and hence the whole class node |
| `error-private-dups.js` | the rows of the cpp:2143-2260 early-error matrix that `private-declaration-dup-error.js` does not reach: method+method, setter+setter, accessor-then-field, method-then-field, accessor-then-method, a complete pair plus a third accessor, the static-mismatch rule in the opposite order, and (as the negative control) a legal static getter+static setter pair |
| `error-private-load-store.js` | every cpp:1207-1295 restriction, written without a `CallExpression` so it is reachable today: load from a setter-only name, store to a getter-only name, store to a method, `delete` on both member kinds — **pinning that the two overloads deliberately report `delete` at DIFFERENT ranges** (`node` vs `parent`) — `super.#y`, an undeclared private name both inside and outside a class, and the four assignment-target shapes where this port's `path.field == left` test could diverge from C++'s `assign->_left == node` pointer comparison (compound assignment, a parenthesized LHS, a linearized `=` chain with a different name per link, and an `UpdateExpression` parent, which is a LOAD) |
| `static-blocks.js` | `visit(StaticBlockNode *)`: an empty static block, three blocks each hoisting their own `var x` into their own body scope (they would collide if they hoisted to the enclosing function), a `var` hoisted out of a nested block, a block alongside static and instance fields (the shared static-elements-init function), a block as a class's ONLY static element (which still creates that function, cpp:1057), `this`/`super.x`/arrows inside a block, a block inside a class inside a function, a class nested inside a block, private names visible from a block, and a block whose body folds (rebuilding the `StaticBlock` node) |
| `error-static-block.js` | the diagnostics the four flag save/restores make reachable: `'await' not in an async function` for `await` in a block inside an `async` function, `invalid use of 'arguments' as an identifier` directly and through an arrow, and a `let`/`var` redeclaration in the block's own scope |
| `error-static-block-typeof-arguments.js` | **PIN for a bug-for-bug quirk.** `class C { static { typeof arguments; } }` reports `invalid use of 'arguments' as an identifier` TWICE at the same location, because `visit(IdentifierNode *, Node *)` has no early return after its `typeof` arm (cpp:304-308 falls through to cpp:322) while `resolveIdentifier`'s two forbid-flag checks run before its decl-cache early return. `forbidArgumentsAsIdentifier_` is only ever set by `visit(StaticBlockNode *)`, so this shape is the only way to reach the double fire — it is the corpus pin S2 T2's report asked S2 T5 to add |
| `class-static-block-await-error.js` | from `test/Parser` — `'await' not in an async function` inside a static block, contrasted with a legal `await` in the class's `extends` clause of the same async function |
| `class-static-block-return-error.js` | from `test/Parser` — `'return' not in a function` for a `return` in a static block, at global scope AND inside a function; the diagnostic is the PARSER's (JSParserImpl.cpp:700), which is why sema's `visit(ReturnStatementNode *)` never sees it |
| `class-static-block-yield-error.js` | from `test/Parser` — the evidence closing S2 T2's open question: `yield` in a static block is `invalid expression` from the parser, so `'yield' not in a generator function` (cpp:1480) is unreachable from there too |

Not corpus-reachable, and documented at their sites rather than curated away:

- **The `@Hermes.overload` duplicate-private-method exemption** (cpp:2197-2200)
  is `typed_`-only; `TYPED` is a constant `false` in this port, so the `&&`
  short-circuits and `hermes::findDecorator` is never called. Ported as a panic
  inside the `if TYPED` that guards it, like the rest of the typed-dialect
  branches.
- **The `test262` code-generation setting** (cpp:1221/1265) gates the whole
  load/store validation block in both member overloads. It is a compiler-driver
  knob (`hermesc -test262`) this port has no flag for, so it reads the
  documented `CODE_GENERATION_SETTINGS_TEST262 = false` constant — which is
  also hermesc's default, i.e. what the corpus compares against.
- **`'yield' not in a generator function`** (cpp:1480) is now known to be
  unreachable from a static block too: the parser rejects `yield` there as
  `invalid expression` (pinned by the imported
  `class-static-block-yield-error.js`), closing the question S2 T2's note left
  open. Combined with S2 T4's finding for field initializers, the diagnostic has
  no reachable source in this dialect at all.
- **`DebugInfoSetting::ALL`** (cpp:1065-1069) would store the static block's
  binding-table scope for `eval` of its children. Ported in shape behind
  `DEBUG_INFO_SETTING_ALL`, exactly like the other two uses of that constant.

## S2 Task 6 additions

Seven new files plus the sixteen re-probed `test/Sema` imports listed in the
S2 Task 6 paragraph at the top. Each was verified byte-for-byte (stdout, stderr
and exit status) against `hermesc -dump-sema` **before** being added:

| File | Covers |
|---|---|
| `calls-shapes.js` | every call shape that hits NONE of the three specials, i.e. the plain `visitESTreeChildren` tail at cpp:1204: callee shapes (identifier, member, computed member, IIFE, arrow IIFE, sequence, logical, a call of a call), `new` in five forms, `OptionalCallExpression` in seven forms (`f?.()`, `o.m?.()`, `o?.m()`, `o?.m?.(1)`, `f?.()()`, `f()?.()`), spread arguments in `CallExpression`/`OptionalCallExpression`/`NewExpression` (three of `visit(SpreadElementNode *)`'s five whitelisted parents, cpp:1460), calls in every statement position (`if`/`while`/`do`/`for`/`for-in`/`switch`/`try`/`throw`/labeled), calls in every function-like body (function, nested function expression, both arrow body shapes, generator, `async`), and calls in a computed class key, a field initializer, a method body and a static block. This is also the first corpus file to need `OptionalCallExpression` in `visit_node`'s override-free generic arm |
| `eval-direct.js` | the `DirectEval` half of the eval detection (cpp:1118-1151): a direct `eval()` at global scope, with extra arguments, with none, inside a function, inside both arrow body shapes, three block levels deep, in a method, a field initializer and a static block — plus the four shapes that are NOT direct calls and therefore warn about nothing (`o.eval("8")`, `eval?.("9")`, `new eval("10")`, and `eval` merely referenced) |
| `eval-shadowed.js` | the negative half (cpp:1121-1131): `isEval` is false when the binding is not a global-scope `UndeclaredGlobalProperty`/`GlobalProperty`, so a parameter, a `var`, a `let`, a block `let`, a nested `function` and a catch parameter all named `eval` suppress the warning — **and the quirk that a GLOBAL `var eval` does NOT**, because `GlobalProperty` in the global scope is one of the two kinds the check accepts. Loose mode throughout, since every one of those declarations is a strict-mode error |
| `shbuiltin-calls.js` | **rewrite #3** (cpp:1153-1165): `$SHBuiltin.foo(1)` and friends, whose dump shows the `Id '$SHBuiltin'` line replaced by a bare `SHBuiltin` line (with no `[D:E:...]`) — at global scope, in a function, in an arrow, in a method, a field initializer and a static block; a rewritten call used as a value, as a callee and as an argument to another rewritten call; and one whose argument FOLDS, so the rebuilt `CallExpression` is rebuilt a second time by its own children walk |
| `error-shbuiltin.js` | every shape rewrite #3 does NOT rewrite, all of which end in `invalid use of $SHBuiltin` from `visit(IdentifierNode *)` (cpp:310-314) because the identifier survives into the children walk: a bare reference, a member access that is not a call, a call whose callee is the identifier itself, a COMPUTED member call (both literal and dynamic key), an `OptionalCallExpression` and an `OptionalMemberExpression` callee, a `NewExpression`, and a shadowed `let $SHBuiltin` — **pinning that each surviving occurrence is reported exactly ONCE** even where `visit(CallExpressionNode *)` also called `resolveIdentifier` on it. Its one legal line, `a.$SHBuiltin(1)`, is the contrast: a non-computed member *property* returns early at cpp:287-293 |
| `super-calls.js` | the legal `super()` shapes (cpp:1195-1202): a derived constructor, with arguments, with a spread, in nested block/`if`/`for`/`try`/`catch`/`switch` positions, through one and two arrows and through an arrow's parameter default (all `nearestNonArrow`), a derived class expression, a derived class inside a function, and `extends` of a parenthesized expression |
| `error-super-call.js` | the eight `super() call only allowed in derived class constructor` shapes: a base-class constructor, an arrow inside one, a plain function inside a *derived* constructor (the function is itself the nearest non-arrow), an object-literal method, an instance and a static method of a derived class, a derived class's field initializer (which runs in the synthetic elements-initializer `FunctionInfo`), and `super(1, 2 + 3)` — the last one pinning that the range covers the ARGUMENTS, since the diagnostic uses `node->getSourceRange()` |

Not corpus-reachable, and documented at their sites rather than curated away:

- **The `EvalDisabled` warning and the `registerLocalEval`-is-skipped branch**
  (cpp:1143-1149) need `Context::setEnableEval(false)`, i.e. hermesc's
  `-enable-eval=false`. `sema_differential.rs` has no per-file flag mechanism,
  so the corpus can only ever compare hermesc's default (eval enabled) against
  ours — which is why `disabled-eval.js` was imported for its ENABLED-branch
  behavior and the disabled branch is pinned by the unit test
  `disabled_eval_warns_differently_and_marks_no_scope` in `tests/resolver.rs`
  instead. The flag itself IS ported (`ast::Context::enable_eval`, default
  `true`, matching Context.h:228).
- **`LexicalScope::localEval`** — what `registerLocalEval` (cpp:2835-2843)
  actually writes — is never printed by `-dump-sema`, so the differential is
  structurally blind to it. Pinned by two unit tests instead:
  `register_local_eval_marks_the_whole_ancestor_chain` (the helper directly,
  in `resolver/calls.rs`) and `a_direct_eval_marks_its_whole_scope_chain` (end
  to end through a real `eval()` call, in `tests/resolver.rs`).
- **The three `$SHBuiltin` module property names** (`moduleFactory`, `export`,
  `import`, cpp:1168-1189) are S4; see `xmod-errors.js`'s Deferred row. All
  three are loud phase-tagged panics, pinned by
  `shbuiltin_{module_factory,export,import}_is_not_modeled` in
  `tests/resolver.rs`.
- **`$SHBuiltin.#x(...)`** — *SUPERSEDED: fixed upstream in `07efab88d` and
  mirrored here; see "C++ defect-fix propagation, Task 4" at the end of this
  file. The shape below is now the corpus file `shbuiltin-private-name.js`.*
  Inside a class declaring `#x` it makes the C++ assert:
  cpp:1166-1167 uses `llvh::cast<IdentifierNode>(methodCallee->_property)`, but a
  non-computed member expression's property may also be a `PrivateName`. Same
  category as S2 T4's `class C { x = class {}; }` finding — a pre-existing C++
  defect, not a port gap — so the shape stays out of the corpus and
  `calls.rs`'s `sh_builtin_property_name` reproduces the failing `cast` as an
  explicit panic.

## S2 Task 8: corpus sweep round 2

### Step 1 — the eight remaining Deferred rows, re-probed

Every row in the Deferred table above was re-run through both binaries (raw
stdout + stderr + exit status). **None unblocked**; each row's stated reason was
confirmed, not assumed:

| File | Re-probe result |
|---|---|
| `break-in-nested-func.js` | panics `scoped function declarations are S3 scope` (`resolver/mod.rs`); hermesc reports `'break' not within a loop or a switch` — still S3 |
| `deep-ast-err.js` | still a vacuous match (comment-only file); still excluded on purpose |
| `function-redeclaration-error.js` | panics in `resolver/functions.rs` (same S3 promoter) |
| `invalid-args-eval.js` | still the SAME single same-location pair at `89:9` and nothing else; C++'s unstable `std::sort` orders the tie the other way. Unchanged, unfixable-by-construction |
| `regress-function-promotion-decl.js` | panics (S3 promoter) |
| `regress-nested-expressions-error.js` | still col 3052 vs 6124 — see the row's updated note, which S2 T8 upgraded with a real crash case |
| `type-alias-children.js` | still a vacuous `';' expected` match without `-parse-flow` |
| `xmod-errors.js` | panics `$SHBuiltin.moduleFactory needs visitModuleFactory` — S4, as re-classified by S2 T6 |

So the Deferred table is final for S2: 8 rows, all blocked on S3 promotion (3),
S4 modules (1), the C++ unstable-sort tie (1), the recursion-depth gap (1), a
dialect flag (1) and the vacuous file (1).

### Step 2 — feature coverage, by exhaustive inventory rather than by eye

The dump is the only thing the differential can see, so coverage was measured on
the dump's own vocabulary: every corpus file's `hermesc -dump-sema` output was
collected and inventoried, and each inventory compared against the full set the
port can produce. Six inventories, six answers (the sixth is stated after the
list, because it is about the scope decorations rather than the dump's
vocabulary):

1. **Node kinds** (the AST half of the dump — 72 distinct labels over the
   corpus, i.e. 71 node kinds plus the `BinOp` line the `+`/`-` linearizer
   prints)
   vs everything `visit_node` handles: exactly ONE handled kind had zero
   occurrences — `DebuggerStatement`, whitelisted by S2 T7 for
   `CheckImplicitReturn`'s sake. Closed by the new `debugger-statement.js`.
   (`WithStatement` and the five `Cover*` kinds are also absent from the AST
   inventory, but only because every file that reaches them exits 2 before a
   dump is printed — `error-with.js`/`error-cover-nodes.js` do cover the
   visits.)
2. **`Decl::Kind`** (SemContext.h:58-105, 18 kinds): all reachable ones appear.
   The two absent are `Import` (S4 modules) and `TypedBuiltin` (`typed_`-only,
   cpp:2630-2639).
3. **`Decl::Special`** (SemContext.h:110-116): `Arguments` (218 occurrences)
   and `PrivateStatic` appear; `Eval` appears nowhere — because **nothing in
   the whole C++ tree ever sets or reads it**. `Decl::Special::Eval` is a dead
   enumerator whose only mention outside the header is the `CASE(Eval)` line of
   `printDecl`'s macro (SemContext.cpp:535-545); across `lib/`, only
   `Special::Arguments` (9 sites) and `Special::PrivateStatic` (5) are ever
   used. The kind×special
   PAIRS were then inventoried, which found a real gap:
   `PrivateGetter PrivateStatic` and `PrivateSetter PrivateStatic` (a
   one-sided STATIC private accessor — neither upgraded to
   `PrivateGetterSetter` nor a `PrivateMethod`) had no corpus exercise. Closed
   by two lines in `private-members.js`.
4. **The `[D:…]` annotation printer's three branches** (SemResolve.cpp:100-118):
   `D:E:%d.N` (921 occurrences) and `D:%d.N E:%d.M` (65) are covered; the
   third — `declD` set with NO `exprD` — is unreachable in the corpus, and so
   is ` UNR`. Both come from the `Unresolver` (`setExpressionDecl(node,
   nullptr)` at cpp:3204, `setUnresolvable`), whose only live call site is
   `with` (cpp:1931-1937's local-`eval` site is `if (false && …)`), and `with`
   always exits 2 without a dump. That is the same structural blindness
   `error-with.js`'s row already records; the unit tests are the net.
5. **Diagnostics**: the 54 distinct `error`/`warning`/`note` messages the
   resolver can emit were harvested from `resolver/*.rs` and each one grepped
   for in the corpus's aggregate stderr, regenerated from a clean run. (Two
   more harvested strings — `Static class properties cannot be named
   'prototype'` and `constructor method must not be private` — turned out to be
   PARSER messages, `parser/src/js/classes.rs:1102`/`:1261`, and are not this
   corpus's business. They only showed up because the first run of this
   inventory read a scratch directory polluted with dumps of parser test files;
   that is exactly how a bogus "covered" verdict gets manufactured, so every
   number in this section comes from the clean regeneration.) Every reachable
   message is covered; the uncovered ones are exactly the already-documented
   set (`Invalid regular expression:` — regex engine;
   `eval() is disabled at runtime` — needs `-enable-eval=false`;
   `spread operator is not supported`, `invalid meta property X.Y`,
   `'yield' not in a generator function` — unreachable in this grammar;
   `typecast not allowed in this context` — `-parse-flow`;
   `Too many nested expressions…` — the recursion-depth row) **plus one this
   sweep found undocumented**: `'this' parameter requires typed mode`
   (cpp:1768-1772). It fires when Flow syntax is parsed but typing is off, i.e.
   under `-parse-flow` and NOT under `-typed`; in the untyped dialect the
   parser rejects a `this` parameter first (`identifier, '{' or '[' expected in
   binding pattern`, probed for functions, methods, arrows and object methods).
   Dialect-corpus phase, like the other `-parse-flow`-only rows.

Task 7's own feature is invisible here by construction:
`SemContextDumper::printFunction` (SemContext.cpp:449-480) prints only
`Func`/`StaticBlock` + strictness + scopes + decls + hoisted functions, and
`FunctionInfo::mayReachImplicitReturn` (SemContext.h:354) is read only by the
FlowChecker and IRGen. `tests/check_implicit_return.rs` is its regression net,
which is why S2 T7 added no corpus file and why `debugger-statement.js` pins
only the resolver-visible half.

A sixth inventory covered `set_node_scope`'s 15 scope-bearing kinds
(SemanticResolver.cpp:2931-2932): 11 of them appear with a printed `Scope
%s.N` in the corpus. The other four never get one from this resolver —
`FunctionDeclaration`/`ArrowFunctionExpression` because `visitFunctionLike`
opens the function scope with the node-less `ScopeRAII` (verified: even with a
non-simple parameter list neither the function node nor its body block carries
a scope in hermesc's dump), and `ComponentDeclaration`/`HookDeclaration`
because they are Flow-only.

### Step 3 — the differential run over the REST of `test/`

`test/Sema` was mined out by S1 T8 + S2 T1-T6, so the sweep was pointed at
every other `.js` file under `test/` that plain `-dump-sema` can consume:
`test/Parser` (366), `test/IRGen` + `test/BCGen` + `test/Optimizer` (395),
`test/hermes` + `test/AST` + `test/Driver` + `test/RA` (655) — 1416 files, both
binaries, three channels. Result after this task's fixes:

| Outcome | Count | What it is |
|---|---|---|
| byte-identical | 1203 | every one of the 190 mismatches is a file hermesc FAILS on: not one file that hermesc compiles successfully disagrees |
| mismatch | 190 | almost all differ in PARSER diagnostic geometry — see below |
| panic | 23 | S4 modules (`Import`/`Export*` kinds, `$SHBuiltin` module protocol) and the S3 promoter, i.e. the known deferrals |

Two of those are single-file findings worth naming:

- `test/AST/regexp.js` — hermesc exits 2 with `Invalid regular expression:
  Character class range out of order` (and two more); we exit 0. That is the
  documented regex-engine deferral (see S2 T3's note), now with an upstream
  file as its witness.
- `test/hermes/computed-fn-name.js:71` (`[k("strClass")] = class {};`) — makes
  **hermesc itself** abort on `SemContext.cpp:478`, exactly the pre-existing
  C++ defect S2 T4 documented; this port reproduces it with its own
  `assert_eq!`. Confirmation that the defect is real upstream code, not a
  contrived shape.

The 190 mismatches are one root cause, and it is **not** in sema: C++'s
`errorExpected` (JSParserImpl.cpp:175-226) merges the "what" location into the
error's RANGE when it is on the same source line (`combineIntoRange(whatLoc,
errorLoc)`), and emits a separate `note:` only when it is not. Many Rust
`need`/`eat` call sites drop C++'s `what`/`whatLoc` arguments (e.g.
`functions.rs:411-415` vs `eat(r_paren, …, "start of parameter list",
lparenLoc)` at JSParserImpl.cpp:657-662), so the range is lost; a few
hand-rolled sites (`classes.rs:1032`, `:289`, `:108`, `modules.rs:905`) emit
`error_cur` + an unconditional `note_at` and also drop the quotes C++ puts
around the token name (`identifier expected in decorator` vs `'identifier'
expected in decorator`). Classified mechanically: **111** differ in caret/range
geometry alone and **69** in geometry plus an extra `note:` we emit and C++
folds into the range (4 of those 69 also lose the quotes — a subset, not a
third bucket), i.e. **180** share the one root cause. The residual **10** are:
three genuinely different parser messages (e.g. `unexpected token after yield
expression` vs `';' expected`, `test/Parser/es6/yield-paren-error.js`), the
three files belonging to rows named above (`test/AST/regexp.js` for the regex
engine, plus BOTH stack-overflow files — `test/Parser/nested-expressions.js` and
`test/hermes/far-environment-access.js` — for the recursion row), and four
one-off geometry cases, including the REVERSE shape
(`test/Parser/escaped-this.js`: C++ prints a bare caret where we print a
range). 111 + 69 + 10 = 190. This
subsumes the roadmap's
existing parser follow-up item (a) — which so far only recorded the *dropped
different-line note* — with the much larger *missing same-line range* half.
Left to the parser track on purpose: it is a call-site audit across the parser
with its own differential harness, and no file the sema corpus wants is blocked
on it. **`doc/superpowers/RustPortRoadmap.md`'s "Parser-phase follow-up" bullet
now carries all of this** (both items rewritten as tracked tasks, with these
numbers and the representative call sites), so the owning track sees it without
reading this MANIFEST.

### What this task added or fixed

| File | Change |
|---|---|
| `debugger-statement.js` | **new** — the only handled node kind with zero corpus occurrences (inventory 1) |
| `expr-visit-generic-2.js` | **new** — `BigIntLiteral`, `TaggedTemplateExpression` and `ImportExpression`: three override-free kinds the resolver PANICKED on (`1n`, `` tag`x${a}` ``, `import("m")`) while hermesc dumps them happily. Found by the Step 3 sweep: **26** upstream files contain one of the three kinds and **25** of them were panicking pre-fix (the 26th, `test/Parser/es6/import-assertions.js`, panics earlier on `ImportDeclaration`, S4). Fixed by adding them to `visit_node`'s generic arm with the usual citation, and this file is the pin — including that BigInt operands are NOT folded |
| `error-limit.js` | **new** — hermesc's driver sets `-ferror-limit` = 20 (CompilerDriver.cpp:555-559, :1223) and `sema-dump` never did, so any input with >20 errors diverged (the corpus's noisiest other file, `error-private-load-store.js`, stops at 15). Pins the cut-off, the `<unknown>:0: error: too many errors emitted` sentinel, its forced-last position, the post-limit suppression of errors AND warnings, and that the surviving 20th is a DECLARATION-pass error from the file's last line (generation order, not location order) |
| `private-members.js` | +2 lines: the static one-sided private accessors (inventory 3) |
| `error-class-field.js` | +1 shape: `class C { a = typeof arguments; }` double-fires `invalid use of 'arguments'`, the `forbidSpecialArgumentsReference_` sibling of `error-static-block-typeof-arguments.js` |
| `shbuiltin-calls.js` | comment fix: the `$SHBuiltin` ambient decl is `%d.27`, not `%d.23`, and this corpus keeps no CHECK lines ("in the dump below" was false) |
| `MANIFEST.md` | this section; `flow/**` is 178 files (173 + 5), not 179; S2 T5 added six new files, not five; the `SpreadElement` parent-whitelist note now names all five parents correctly; the missing S2 T6/T7 count lines |

Out-of-corpus fixes the sweep forced (each TDD'd, smallest repro first):

- `sema/src/bin/sema_dump.rs` — apply the driver's error limit (above), as a
  real `--ferror-limit` option (`init(20)`, 0 = unlimited) rather than a
  hard-coded 20, so hermesc's escape hatch survives. Cross-checked against
  `hermesc -dump-sema -ferror-limit=N` at N = 0 (26 errors both sides) and
  N = 3 (byte-identical).
- `support/src/render.rs` + `support/src/manager.rs` — the location prefix in
  `printDiagnosticHelper` (SourceErrorManager.cpp:574-582) is conditional: an
  empty filename prints no prefix, `-` prints as `<stdin>`, and the column is
  omitted when C++'s `columnNo` is -1 (i.e. `col == 0` here). A location-less
  message also keeps `SourceMgr::GetMessage`'s `BufferID = "<unknown>"`
  default (SourceMgr.cpp:246). We were printing `:0:0: error: …` for the
  sentinel instead of `<unknown>:0: error: …`. Unit-tested in
  `render.rs::header_prefix_is_conditional`.
- `sema/src/resolver/mod.rs` — the three node kinds above.

## S3 Task 1 additions (`ScopedFunctionPromoter`)

S3 Task 1 ported `lib/Sema/ScopedFunctionPromoter.cpp` and wired both of its
in-scope call sites (`visit(ProgramNode *)`, SemanticResolver.cpp:224-227, and
`visitFunctionBodyAfterParamsVisited`, cpp:1904-1910), replacing the two
`assert!` seams that fired on any loose-mode function containing a block-nested
function declaration. It imported no upstream `test/Sema` file — the remaining
Deferred rows are all blocked on S4/S5 features (modules, per-file harness
flags, lazy/eval) — and added two new files:

| File | What it pins |
|---|---|
| `promotion-basic.js` | **new** — promotion HAPPENS. Both call sites (top level → `GlobalProperty`, inside a function → `Var`), the parameter rule (`processParameters`, cpp:147-158 / ES2022 B.3.2.1 29.a.ii: a formal parameter of the same name blocks), four more of the seven `visitScope` kinds (`Switch`/`For`/`ForIn`/`ForOf`), and that two same-named candidates in sibling blocks both promote onto ONE function-scope decl (`try_emplace`, cpp:2138) |
| `promotion-blocked-by-let.js` | **new** — promotion is REFUSED. `let`, `const` and `class` each block from the enclosing scope, at top level and inside a function; the `catch (e)` case is the counter-example that must NOT block (`ES5Catch` is skipped, cpp:212-216 / ES14.0 B.3.4), so the function nested inside the catch block IS promoted to `Var` |

Twenty further shapes were probed against hermesc without being imported (they
add no behavior the two files above don't already pin): restricted global names
(`{ function undefined() {} }`), `var`-before/after-block, a promoted name in a
class static block, a strict-mode function (no promotion), an arrow body,
doubly-nested blocks, a `let` in an ENCLOSING function (does not block — the
promoter is per-function), generators/`async`, destructured catch params and
parameters (`Catch` blocks, `ES5Catch` does not), `eval`/`arguments` names, and
a defaulted parameter. All twenty matched byte-for-byte.

One probe did NOT match, and is a **pre-existing parser landmine, not a sema
one**: `{ l: function f(){} }` is a parse error ("Function declaration not
allowed as body of labeled statement") whose caret C++ prints bare where this
port prints the full range. That is exactly the tracked "missing same-line
range" half of the parser-phase follow-up recorded in the S2 Task 8 section
above (and in `doc/superpowers/RustPortRoadmap.md`); it never reaches sema, so
the file was not imported.

### A new landmine found while porting the promoter

> **SUPERSEDED** — fixed upstream in `4ad67c992` and mirrored here; see "C++
> defect-fix propagation, Task 3" at the end of this file. The shape below is
> now the corpus file `using-scoped-fn-promotion.js`.

`hermesc` ITSELF aborts (Debug assertion, exit 134) on a `using` declaration
that shares a function with a promotable block-nested function declaration:

```js
using x = 1;
{ function f() {} }
```

```
hermesc: lib/Sema/ScopedFunctionPromoter.cpp:260: ... Assertion
`varDeclaration->_kind == resolver_.keywords().identVar' failed.
```

`ScopedFunctionPromoter::extractDeclaredIdents` (cpp:255-262) knows only
`let`/`const`/`var`, and the promoter runs BEFORE
`visit(VariableDeclarationNode *)` can report "using declarations are not
supported" (cpp:329-336). The port reproduces it as a `debug_assert!` at the
same place, so such a file cannot go in the corpus (the abort exit codes
differ, 134 vs 101 — same situation as the already-listed
`class C { x = class {}; }` and `$SHBuiltin.#x()` aborts).

## S3 Task 2 additions (promotion corpus unlock)

S3 Task 2 re-probed and imported the three remaining S3-blocked `test/Sema`
rows (all re-verified against hermesc, raw stdout/stderr/exit bytes, before
copying — see the "Imported" table above for their one-line notes) and added
a six-file feature battery covering the rest of `ScopedFunctionPromoter` and
its call sites that `promotion-basic.js`/`promotion-blocked-by-let.js` (S3
T1) didn't already pin. Two bullets from the battery list needed shapes
derived directly from the C++ (their sketches in the task brief were flagged
as possibly wrong) rather than the ones suggested; both are documented in
full in the files' own header comments:

| File | What it pins |
|---|---|
| `promotion-catch-destructuring-blocks.js` | **new** — `catch ({ e })` (a destructuring param) maps to `Decl::Kind::Catch`, not `ES5Catch` (cpp:287-294), so — unlike `promotion-blocked-by-let.js`'s `inCatch()` — it DOES block promotion (cpp:212-216) |
| `promotion-nested-scope-visibility.js` | **new** — a `let` blocks a candidate arbitrarily deep in its own descendant scopes, but stops applying the moment its own block closes: a candidate in a later sibling block promotes normally |
| `promotion-var-reuse.js` | **new** — the `Var, ScopedFunc` arm of the "when to create a new declaration" switch (cpp:2546-2562) in both source orders (`function` then `var`, `var` then `function`), plus the genuinely cross-scope sub-case (`reuseDeclForNewBinding`, cpp:2554-2561): a second, same-named candidate whose OWN identifier has never been declared before, reached with a `Var`-like decl as the nearest (unshadowed) binding. Derived from the C++, not the brief's sketch — see the file's header comment for the exact mechanics (why a `let` in the SAME block, positioned AFTER the function in source order, produces this instead of just blocking it, the way an enclosing `let` would) |
| `promotion-var-shadows-promoted.js` | **new** — `visit(VariableDeclarationNode *)`'s `prevIsLexicalBindingOfPromotedFunc` special case (cpp:365-374, feeding the error at cpp:391-401), at both top level and function scope. Derived from the C++, not the brief's `let`-based sketch (a `let` there doesn't even reach this code path — only a `var` does, since the check is gated on `kw_.identVar`). Its `prevKind` is `ScopedFunction`, which is independently let-like, so this shape alone does not prove the flag is load-bearing — see `promotion-es5catch-var-shadows.js` for that |
| `promotion-es5catch-var-shadows.js` | **new (review follow-up)** — isolates `prevIsLexicalBindingOfPromotedFunc` as the SOLE cause of the error: with `prevKind == ES5Catch`, the ordinary check at cpp:392 is explicitly excluded (`!= ES5Catch`, the B.3.5 exemption `catch-shapes.js` pins), so only the flag being `true` can fire it. A function promoted from a sibling block, then a nested `var` inside `catch (e) { ... }` with the SAME name |
| `promotion-es5catch-cross-scope-reuse.js` | **new** — the `ES5Catch` counterpart of `promotion-var-reuse.js`'s cross-scope case: the `ES5Catch, ScopedFunc` arm (cpp:2563-2578, specifically the `promotedFuncDecls` lookup at cpp:2569-2577) — the S1-T5 matrix row that stayed S3-blocked until this task. Needs an outer, already-popped `let` of the same name to make the blocked candidate's nearest binding land on the catch's `ES5Catch` decl rather than the `let` itself; see the file's header comment |
| `promotion-strict-mode-negative.js` | **new** — the strict-mode gate on both promotion call sites (cpp:224-227, cpp:1906-1910): Annex B.3.3 is loose-mode-only, so a block-nested function keeps its local `ScopedFunction` decl and a same-name reference resolves as an undeclared global instead of a promoted `Var` |

Two battery bullets did not get a new file:

- **param-name shadowing** (cpp:147-158) and the **`switch`-case-scope
  candidate** (cpp:47-49) are already pinned by `promotion-basic.js`
  (`withParam` and `scopes()` respectively) — extending them would have
  duplicated shapes the seed file already covers, which the task brief
  explicitly said not to do.
- **`const`/`class` blockers** (cpp:215's `isKindLetLike`) are already
  pinned by `promotion-blocked-by-let.js`.

**The `with`-scope arm** (`ScopedFunctionPromoter.cpp:62-64`) was probed and
confirmed **not corpus-reachable**: `visit(WithStatementNode *)`
unconditionally reports `with statement is not supported` whenever
`compile_` is set (SemanticResolver.cpp:757-759), which is true for every
invocation `sema_differential.rs` makes (there is no per-file flag support
to turn `compile_` off, per the harness's known limitation). hermesc
confirms: `with ({}) { function w() {} }` exits 2 with that error before
the promoter's `With` arm ever runs. This matches (and reconfirms)
`promotion-basic.js`'s own note about the seventh `visitScope` kind.

### MANIFEST arithmetic

- `test/Sema` top-level sweep: Imported 46 → **49** (+3: the three rows
  above), Deferred 8 → **5** (-3), 49 + 5 = 54 (unchanged, still fully
  accounted for).
- `sema_corpus/` directory / differential gate: 162 → **171** files
  (+3 imported + 6 new battery files = +9). hermesc-succeeded: 90 → **96**
  (+6): of the 9 new files, 6 succeed on hermesc
  (`regress-function-promotion-decl.js` and the five battery files other
  than `promotion-var-shadows-promoted.js`) and 3 fail on hermesc
  (`break-in-nested-func.js`, `function-redeclaration-error.js`,
  `promotion-var-shadows-promoted.js` — all genuine `error:`/exit-2 cases,
  not panics). Gate output:
  `sema differential (tests/sema_corpus): 171 corpus files matched (96 succeeded on hermesc)`.

### Review follow-up: `promotion-es5catch-var-shadows.js`

A task review found that the claim above ("the ordinary let-like check
already fires in this shape") was true only of
`promotion-var-shadows-promoted.js`, and left
`prevIsLexicalBindingOfPromotedFunc` (cpp:365-374) unpinned as an
independent cause: a Rust port that dropped the
`promotedFuncDecls`/depth computation entirely would still pass all 171
files. `promotion-es5catch-var-shadows.js` (added in response) closes that
gap — see its header comment and the table row above. Re-running the gate
after adding it:
`sema differential (tests/sema_corpus): 172 corpus files matched (96 succeeded on hermesc)`
— 171 → **172** files (+1), hermesc-succeeded stays at **96** (the new
file is an `error:`/exit-2 case on hermesc, same as
`promotion-var-shadows-promoted.js`, so it adds to the failing side only).
No Rust-side bug was found: `sema-dump` already matches hermesc
byte-for-byte on this input — the finding was a corpus gap, not a port
defect.

## S3 Task 3: upstream re-probe

S3 Task 3 re-ran the exact S2-T8 sweep — both binaries, raw stdout + stderr +
exit status, no extra flags on either side — over the same 1416 files in the
same 8 upstream dirs (`test/Parser` 366, `test/IRGen`+`test/BCGen`+
`test/Optimizer` 395, `test/hermes`+`test/AST`+`test/Driver`+`test/RA` 655),
now that both S3 `assert!` seams are replaced by the real
`ScopedFunctionPromoter`. File count re-verified: `find test/{Parser,IRGen,
BCGen,Optimizer,hermes,AST,Driver,RA} -iname '*.js' | wc -l` = 1416, unchanged
from S2-T8.

### Result

| Outcome | S2-T8 | S3-T3 | Delta |
|---|---|---|---|
| byte-identical | 1203 | **1209** | +6 |
| mismatch | 190 | **190** | 0 |
| panic | 23 | **17** | −6 |

1209 + 190 + 17 = 1416 (S2-T8's 1203 + 190 + 23 = 1416, same total, both
verified before writing this table).

### The +6 / −6 move, named

Re-running the PRE-S3 `sema-dump` (checked out at `5aab87d1d`, the commit
immediately before S3 Task 1, built in an isolated worktree so the current
binary was untouched) against the same 1416 files and the same `hermesc`
found exactly **six** files that hit one of the two old `assert!` seams
(`"sema S0: scoped function declarations are S3 scope"`,
`resolver/mod.rs:1387-1397` pre-S3) and nothing else changed bucket:

| File | Pre-S3 | Post-S3 |
|---|---|---|
| `test/IRGen/function-promotion.js` | panic (old assert) | identical |
| `test/IRGen/scoped-func-init.js` | panic (old assert) | identical |
| `test/Parser/if-function.js` | panic (old assert) | identical |
| `test/hermes/iterator-close-throw.js` | panic (old assert) | identical |
| `test/hermes/promoted-function-redeclaration.js` | panic (old assert) | identical |
| `test/hermes/stack-overflow-apply.js` | panic (old assert) | identical |

All six succeed on hermesc (`hc_code == 0`) and are now byte-identical
against it.

One reconciliation step was needed to get from a raw exit-status-shape
classification to the table above. A first automated pass — bucketing by
whether the Rust side's exit differs from a clean 0/2, i.e. treating any
`SIGABRT`/`SIGSEGV`/panic-string exit as "panic" — read **1203 / 188 / 25**
at the pre-S3 commit and **1209 / 188 / 19** post-S3, not matching the
documented S2-T8 baseline (1203 / 190 / 23) even at the identical pre-S3
commit. The 2-file gap in both pairs is the same two files each time:
`test/Parser/nested-expressions.js` and
`test/hermes/far-environment-access.js` are process aborts on our side
(`SIGABRT`, Rust's stack-overflow guard-page handler), which the raw pass
counts as "panic" — but the S2-T8 text's own "residual 10" paragraph
explicitly places **both** of them inside the 190-mismatch bucket ("both
hermesc and sema-dump both correctly error … at different columns" /
"STACK-OVERFLOWS and aborts … before its own tracker trips": a
recursion-depth-parity landmine where both sides fail, just differently, not
an unhandled-construct gap). Reclassifying those two into "mismatch" (the
established convention, applied identically at both commits since both
commits contain the same 2 files) turns 1203/188/25 into 1203/190/23 — an
exact match to the documented S2-T8 baseline, confirming the convention and
validating the tooling — and turns 1209/188/19 into the reported
**1209/190/17**. No other file's bucket moved for any reason besides the six
in the table above: the mismatch bucket's file-level membership (190 files
under the convention) was diffed between the pre-S3 and post-S3 runs and is
identical.

### Zero S3-attributable panics

The 17-file panic bucket is, exhaustively (each message read, not assumed):

- **16 S4 files** — `mod.rs:1324`'s catch-all (`sema: unhandled node kind
  X (S3+/typed phases)`) on 9 files, each message read individually: `X =
  ExportDefaultDeclaration` on `test/AST/es6/export-default-function.js` and
  `test/Parser/es6/{export-default-async,export-default-class,
  export-default}.js` plus `test/Parser/flow/component-syntax/
  component-identifier.js` (not a Flow-only kind — the panic fires on the
  plain `export default component ...` declaration, before any
  component-syntax node is reached); `X = ExportAllDeclaration` on
  `test/Parser/es6/export.js`; `X = ImportDeclaration` on
  `test/Parser/es6/{import-assertions,import-location,import}.js`. And
  `calls.rs:312`'s `$SHBuiltin.moduleFactory needs visitModuleFactory
  (cpp:1320-1366) — S4 modules` panic on 7 files
  (`test/BCGen/HBC/xmod-requires-opt.js`, `test/Optimizer/xmod-{builtins,
  require-cse,requires-opt-extension,requires-opt}.js`,
  `test/hermes/xmod-exec-require{-bad-func,}.js`). Same two call sites,
  same messages, as `xmod-errors.js`'s already-Deferred row.
- **1 pre-existing-C++-defect reproduction** — `test/hermes/
  computed-fn-name.js:71` (`[k("strClass")] = class {};`), the same
  `SemContext.cpp:478` scope-walk assertion S2 T4 already documented (hermesc
  itself aborts, exit -6/134; this port's `dump_context.rs` `assert_eq!`
  fires too, exit 101 — different abort mechanisms, so never byte-identical,
  but not a port gap).

Zero of the 17 mention `ScopedFunctionPromoter`, `promot`, or anything S3;
confirmed by `grep -n "scoped function declarations are S3 scope"
rust/crates/sema/src/` returning nothing (the seam itself is gone from the
source) and by reading every one of the 17 panic messages above individually
(no grep-and-trust — each one quoted verbatim).

### Step 2 — the five Deferred `test/Sema` rows, re-probed

Every row in the Deferred table was re-diffed against hermesc (raw
stdout+stderr+exit):

| File | Re-probe result |
|---|---|
| `deep-ast-err.js` | still the vacuous comment-only match; still excluded on purpose |
| `invalid-args-eval.js` | still the same single same-location tie at `89:9` (`warning: the variable "arguments" was not declared…` vs `error: cannot declare 'arguments' in strict mode`, swapped order) — C++'s unstable `std::sort`, unchanged |
| `regress-nested-expressions-error.js` | still col 3052 (hermesc) vs col 6124 (sema-dump) — the recursion-depth-parity gap, unchanged |
| `type-alias-children.js` | still the vacuous `';' expected` match without `-parse-flow` |
| `xmod-errors.js` | still panics `$SHBuiltin.moduleFactory needs visitModuleFactory` — S4, unchanged |

`test/Sema/lowering/fastarray-push.js` (the one `flow/**`-adjacent file
outside `flow/`) was also re-probed: still a vacuous `';' expected` match on
`var x: number[];` without `-parse-flow`, same category as
`type-alias-children.js`. None of the five Deferred rows, nor the bulk-deferred
`flow/**`/`lowering/` files, newly unblocked. **No files imported** by this
task; the `test/Sema` Imported/Deferred counts stay at 49/5 (49 + 5 = 54,
unchanged from S3 Task 2).

### Step 3 — no fixes needed

Because zero panics traced to the S3 promoter, no TDD repro/fix round was
required. The gate (`REQUIRE_DIFFERENTIAL=1 cargo test … --test
sema_differential`) is unchanged at **172 corpus files matched (96 succeeded
on hermesc)** — no corpus files were added or removed by this task.

## S3 final-review follow-up

The final whole-branch review of S3 found four issues, one of which
(the `For`/`ForIn`/`ForOf` `visitScope` arms) needed a new corpus file;
the rest were comment-only corrections with no code or dump-visible
behavior change.

### New file: `promotion-for-family-let-blocker.js`

The reviewer confirmed the port's `ForStatement`/`ForInStatement`/
`ForOfStatement` arms (`ScopedFunctionPromoter::visit`,
ScopedFunctionPromoter.cpp:53-61 — each a thin `visitScope(node)` forward,
same as `BlockStatementNode`'s at cpp:50-52) are CORRECT, but noted that no
existing corpus file actually exercises them: a `FunctionDeclaration` can
never be a bare loop body, so the three arms are only observable via a
`let`-like declaration in the loop HEAD blocking a promotion candidate
declared in the loop's BODY block — a port that dropped all three arms
(silently falling back to the default `visit(Node *)`, which still
recurses into children but never calls `processDeclarations` on the loop's
own scope) would still pass all 172 existing files.

One new file, with one function per arm, each `let <name>` in the loop head
and a same-named `function <name>() {}` as the loop body's sole statement.
Verified with hermesc FIRST (`hermesc -dump-sema`, raw stdout+stderr+exit
byte comparison against `sema-dump`): exit 0 on both sides, byte-identical;
all three inner functions stay block-scoped `ScopedFunction` decls (not
promoted to `Var`/`GlobalProperty`), confirming the blocker is honored.

| File | What it pins |
|---|---|
| `promotion-for-family-let-blocker.js` | **new** — the `ForStatement`/`ForInStatement`/`ForOfStatement` `visitScope` arms (cpp:53-61): a `let` in a loop's HEAD scope blocks a same-named `function` candidate declared in the loop's BODY block, one function per arm (`forHead`/`forOfHead`/`forInHead`) |

Gate: `sema differential (tests/sema_corpus): 173 corpus files matched (97
succeeded on hermesc)` — 172 → **173** files (+1), hermesc-succeeded 96 →
**97** (+1: the new file is a legal, blocked-promotion shape, not an error
case).

### Comment-only corrections (no corpus-count change)

- `promotion-es5catch-cross-scope-reuse.js`: the header comment
  misdescribed its own code, saying `let e;` sits "inside catch(e) in an
  extra nested block" — it actually sits in the enclosing SIBLING block,
  outside the `try`/`catch` entirely; the nested block that does exist
  wraps the candidate `function e(){}` (to dodge the `prevInPrevScope`
  error, SemanticResolver.cpp:2529-2530), not the `let`. The paragraph was
  replaced with the correct derivation (matching the task-2 report's own
  worked example).
- `rust/crates/sema/src/resolver/mod.rs` (`process_promoted_func_decls`):
  the expect-unreachability comment enumerated two of
  `validateAndDeclareIdentifier`'s three early-returns that could leave a
  declaration decl unset, omitting the "two declarations put" path
  (cpp:2619-2625). Added: that path's own guard requires
  `semCtx_.getDeclarationDecl(ident)` to already be non-null, which is
  impossible for a promoted function's own identifier node being declared
  for the first time.
- `promotion-nested-scope-visibility.js`: widened a citation from
  cpp:160-224 to cpp:160-244 — `processDeclarations`'s promotion decision
  (the `bindingTable_.lookup` check) is in the back half of the function,
  cpp:226-244, not just the decl-scanning loop the old range covered.
- `promotion-var-shadows-promoted.js`: corrected a citation from
  cpp:376-379 to cpp:376-383 — the `continue`-guard `if` block (the
  `prevIsLexicalBindingOfPromotedFunc` check plus its body) spans all five
  lines, not four.

## S4a Task 1: the `// FLAGS:` per-file harness

`sema_differential.rs` grew a per-file-flag mechanism (`per_file_flags`): if a
corpus file's FIRST LINE is exactly `// FLAGS: <args>`, the whitespace-split
args are appended, in order, to BOTH `hermesc`'s and `sema-dump`'s argv, after
the per-corpus extras and before the file path. The spellings are hermesc's
own (single-dash, e.g. `-enable-eval=false`, `-fno-std-globals`), not
`sema-dump`'s previous double-dash-only convention — the `command_line` crate
(`rust/crates/command_line/src/parser.rs`) grew a small, backward-compatible
fix alongside this (`parse_single_dash_arg`, covered by its own
`test_single_dash_long` unit test): a single leading dash now tries a
full-long-name lookup FIRST (identical to `--name` handling, including
`name=value` splitting), falling back to the pre-existing single-character
short-option grouping (`-i32`, `-m 10`) only when no long name matches —
mirroring real LLVM `cl`, which treats `-flag` and `--flag` as exact
synonyms for named options (verified against hermesc directly: `hermesc
--fno-std-globals -dump-sema` and `hermesc -fno-std-globals -dump-sema`
produce byte-identical output). Flagless files (everything predating this
harness) are unaffected either way.

`sema-dump` grew two new options to go with it, modeled on the existing
`parse_flow`/`ferror-limit` options (`rust/crates/sema/src/bin/sema_dump.rs`):

- `--enable-eval` (hermesc `-enable-eval`, `CompilerRuntimeFlags.h:19-22`): a
  plain optional-value bool defaulting to true, wired into
  `ast::Context::enable_eval` (the same field `resolver/calls.rs`'s
  `visit_call_expression` already read — S2 T6 wired the READ side; this task
  wires the WRITE side that lets a corpus file actually flip it).
- `--fstd-globals` / `--fno-std-globals` (hermesc's `CLFlag` pair,
  `CompilerDriver.cpp:273-278`, both defaulting to true): gates whether
  `libhermes` is parsed and loaded as the ambient `DeclarationFileListTy` at
  all (previously unconditional), mirroring `if (cl::StdGlobals) {
  loadGlobalDefinition(...) }` at `CompilerDriver.cpp:2000-2007`. Ported as
  two independent `Opt<bool>`s merged in `main()` (`fstd_globals &&
  !no_std_globals`), NOT as a single option sharing one `OptValue` via
  `OptDesc::opt_value`: that sharing mechanism exists in the crate but every
  registered `Opt` unconditionally calls `OptValue::finish()`
  (`command_line/src/opt.rs:384-385`), and `OptValue::finish()` asserts it is
  never called twice (`opt.rs:72-78`) — two options sharing one `OptValue`
  panics there (`finish() must not be called twice`, discovered empirically
  running the new corpus file). The merge is a deliberate simplification, not
  a full port of `CLFlag::getValue()`'s last-one-wins position tie-break for
  when BOTH spellings are given on the same command line — unreachable via
  this harness's per-file `// FLAGS:` line, which never spells out both for
  the same file. See the `no_std_globals` field doc for the full citation.
  **[2026-08-12: superseded.** The publication track fixed
  `OptValue::finish()` to be idempotent and moved `sema-dump` into the `tools`
  crate, so the sharing mechanism now works and the two options were collapsed
  onto one shared `OptValue`. The merge expression, the `no_std_globals` field
  and its doc are gone; last-one-wins is now inherited from the shared storage
  rather than approximated, which also closed the tie-break gap this paragraph
  describes — verified against hermesc in both orders. This paragraph is kept
  as the record of why the split existed.**]

### New files

| File | Covers |
|---|---|
| `flags-enable-eval-off.js` | `// FLAGS: -enable-eval=false`; pins the `EvalDisabled` branch of `visit(CallExpressionNode *)` (SemanticResolver.cpp:1147, `resolver/calls.rs:232`'s `else if is_eval` arm) — a direct call to `eval` still resolves the identifier but warns "eval() is disabled at runtime" instead of the enabled branch's `DirectEval` warning. The enabled branch is already pinned by `disabled-eval.js` (S2 T6); this file is what that row's note flagged as unit-tested-only until the harness grew per-file flags |
| `flags-no-std-globals.js` | `// FLAGS: -fno-std-globals`; pins two things at once: the ambient-decl load being skipped entirely (no 63 `UndeclaredGlobalProperty` decls in the dump) AND that `print` — normally one of those 63 — still resolves as an on-the-fly `UndeclaredGlobalProperty` when there is no ambient decl and no local declaration either |

`type-alias-children.js` (`test/Sema`, formerly a Deferred row) was
hermesc-verified with its needed `-parse-flow` flag (`hermesc -parse-flow
-dump-sema type-alias-children.js`: exit 0, full byte output including the
`TypeAlias`/`GenericTypeAnnotation` tree) then imported with a prepended
`// FLAGS: -parse-flow` line. With `-parse-flow` actually enabling the Flow
grammar, `sema-dump` no longer hits the old vacuous `';' expected`
parse-error match that kept this file Deferred — it now PARSES the file and
resolves it, which needed one small resolver addition: `resolver/mod.rs`'s
`visit_node` grew a `Node::TypeAlias(_) => TransformResult::Unchanged` arm,
porting `SemanticResolver::visit(TypeAliasNode *node) { // Do nothing. }`
(SemanticResolver.cpp:1579-1581) — a TRUE no-op that does NOT recurse into
`_id`/`_typeParameters`/`_right` (unlike this port's generic
`visit_children_mut` catch-all arms), which is exactly why the dump shows
`Id 'A'`/`Id 'B'`/`GenericTypeAnnotation` with no `[D:E:...]` resolution
annotations — the file's own stated purpose ("children of type alias AST
node are not resolved as variables"). Scope check against the plan's global
constraint ("ONLY the four module-visit arms replace catch-all panics in
this phase", reserved for Task 3's `Import`/`Export*` arms): resolved as a
plan-drafting inconsistency, since the user-approved spec's §3.4 explicitly
authorizes "whatever their surrounding visits need to exist" for the untyped
`-parse-flow` paths — the plan's own Global Constraints were amended
(commit 9d2fa2d92) to reflect this, and this one-line, single-citation,
this-file-only arm is squarely what that authorization covers. The
neighboring cpp:1583-1596 do-nothing arms (`TypeParameterDeclarationNode`,
`TypeParameterInstantiationNode`) are NOT ported here — `type-alias-
children.js` never reaches them (no type parameters in `type A = B;`) — and
are left for whichever later task's corpus needs them.

Gate as of this task: `sema differential (tests/sema_corpus): 176 corpus
files matched (100 succeeded on hermesc)` — 173 → **176** files (+3:
`flags-enable-eval-off.js`, `flags-no-std-globals.js`,
`type-alias-children.js`), hermesc-succeeded 97 → **100** (+3: all three new
files are hermesc successes, not error-path pins). Deferred table 5 → **4**
(`type-alias-children.js`'s row removed; see its new row in "Imported"
above).

## S4a Task 3: the module visits

`resolver/modules.rs` ports the four ES-module declaration visits —
`visit(ImportDeclarationNode *)` (SemanticResolver.cpp:874-890),
`visit(ExportNamedDeclarationNode *)` (cpp:1510-1517),
`visit(ExportDefaultDeclarationNode *)` (cpp:1519-1547, carrying **rewrite
#4**) and `visit(ExportAllDeclarationNode *)` (cpp:1549-1554) — replacing
`visit_node`'s catch-all panic for those four kinds, per the plan's global
constraint that ONLY these four arms may do so in this phase. The
`$SHBuiltin` CommonJS-module protocol (`calls.rs`'s three phase-tagged
panics) is untouched: it is S4b, together with `-commonjs` itself, which is
implemented nowhere in this port. Six module-SPECIFIER kinds
(`ImportSpecifier`, `ImportDefaultSpecifier`, `ImportNamespaceSpecifier`,
`ImportAttribute`, `ExportSpecifier`, `ExportNamespaceSpecifier`) joined
`visit_node`'s override-free generic arm at the same time — none of them
appears in the SemanticResolver.h:200-304 `visit` inventory or in
DeclCollector.h:81-99, so C++ reaches their children through
`visitESTreeChildren`, exactly like that arm; they are the children the four
new visits walk into.

Two C++ quirks are preserved bug-for-bug and flagged in `modules.rs`:

> **SUPERSEDED** — both quirks were fixed upstream (`f90a83146`, `6b59daf0d`)
> and mirrored here; see "C++ defect-fix propagation, Task 3" at the end of
> this file. The third asymmetry below (import's ungated error) still holds.

- **`ExportAllDeclaration`'s message wording** (cpp:1552-1553) is `'export'
  statement requires **CommonJS** module mode`, where the Named and Default
  visits — same gate, same condition — say plain `'export' statement
  requires module mode`. Pinned by `module-export-plain.js` (all three in
  one file) and by upstream `export.js`.
- **Rewrite #4 hard-codes `/* async */ false`** (cpp:1538) instead of
  `funcDecl->_async`, so an anonymous `export default async function () {}`
  loses its async flag on the rewritten `FunctionExpression`. Not
  dump-visible without `-commonjs`; pinned by the unit test
  `export_default_anonymous_function_is_rewritten_to_an_expression`.

A third asymmetry, also preserved: the IMPORT error (cpp:876-879) is NOT
`compile_`-gated while all three export errors are, so an `import` errors
even under `resolveASTForParser`. Both sides of that are pinned in the
parser-entry corpus (`sema_corpus_parser/MANIFEST.md`).

`FunctionInfo::imports` (cpp:887) is now populated, which discharges spec
§3.4 (a)'s second and last backref-fixup obligation (`hoisted_functions` was
the first, S1 T7). The list is dump-blind — `SemContextDumper.cpp` never
mentions it, and neither does this port's `dump_context.rs` — so the
differential cannot see it at all; the unit tests
`import_declarations_are_recorded_on_the_function_info` and
`import_backref_is_untouched_without_a_rebuild` in `tests/resolver.rs` are
its only pin, asserting list CONTENT (node identity against the returned
tree, in order) rather than just length.

### New files (Step 2 — authored)

| File | Covers |
|---|---|
| `module-import-plain.js` | `import {a} from 'm';`. The import visit's module-mode error, which is NOT `compile_`-gated (cpp:876-879) — the bug-for-bug asymmetry against the exports. hermesc: exit 2, 1 error, no dump (the post-walk gate). Verified against `hermesc -dump-sema` FIRST, raw stdout+stderr+exit |
| `module-export-plain.js` | All three export visits in one file, including the ExportAll **message-wording quirk** (`CommonJS module mode` vs `module mode`) side by side with the other two. Its `export default function () {}` also drives rewrite #4 through the walk under `compile_ = true` — dump-invisible here (hermesc skips the dump on a `resolveAST` failure, CompilerDriver.cpp:960-974), so what it pins is that the rewritten subtree still resolves cleanly. hermesc: exit 2, 3 errors. Verified FIRST |

### New files (Step 4 — the S3-T3 sweep's module panic bucket)

The S3-T3 re-probe left a 17-file panic bucket: 9 files on `mod.rs`'s
catch-all for a module kind, 7 on `calls.rs`'s `$SHBuiltin.moduleFactory`
panic, and `computed-fn-name.js`'s pre-existing-C++-defect reproduction. All
**nine** module files became byte-identical with this task and are imported
below, each hermesc-verified FIRST (raw stdout+stderr+exit, no extra flags on
either side — none needs a `// FLAGS:` line, so each is a byte-identical
upstream copy). The other eight are untouched and stay out, exactly as the
global constraints require.

| File | Upstream | Covers |
|---|---|---|
| `import.js` | `test/Parser/es6/import.js` | Eight `import` forms — bare, namespace, named, renamed, trailing comma, reserved-word-as-imported-name, default, default + namespace. 8 module-mode errors, exit 2 |
| `import-location.js` | `test/Parser/es6/import-location.js` | `import {foo, bar as baz} from 'other';` — one error, exit 2 |
| `import-assertions.js` | `test/Parser/es6/import-assertions.js` | The `import assertions are not supported` error and its `compile_ && !_attributes.empty()` gate (cpp:881-885): `import 'foo.js' with {}` reports ONE error (empty attributes), every `with {...}` form reports TWO at the same location, module-mode first. Also carries `import('foo', 1)` (`ImportExpression`, S2 T8). 13 errors, exit 2 |
| `export.js` | `test/Parser/es6/export.js` | The widest export file: `export *` (the **CommonJS-wording** message), `export * as bar` (`ExportNamespaceSpecifier`), `export default function myFun()` (NAMED — rewrite #4 does not fire), `export var/function/let/const`, `export {}`/`{x}`/`{y,}`/`{a as b, c, last}`. 11 errors (one per `export` line, none of them the assertions kind), exit 2 |
| `export-default.js` | `test/Parser/es6/export-default.js` | `export default 2 + 2;` — the non-function default export (rewrite #4's `dyn_cast` fails), 1 error, exit 2 |
| `export-default-class.js` | `test/Parser/es6/export-default-class.js` | `export default class {}` — anonymous CLASS default export: another kind rewrite #4 must leave alone, 1 error, exit 2 |
| `export-default-async.js` | `test/Parser/es6/export-default-async.js` | `export default async function foo() {}` — NAMED async, so the `/* async */ false` quirk does not fire (it needs the anonymous form); 1 error, exit 2 |
| `export-default-function.js` | `test/AST/es6/export-default-function.js` | `'use strict'; export default function() {}` — the ANONYMOUS form, i.e. the one input in the whole upstream tree that actually fires rewrite #4. Its own `RUN:` line uses `-dump-transformed-ast -commonjs`, which this port does not implement (S4b); imported at the flagless S4a shape, where hermesc reports the module-mode error and skips the dump, so what it pins here is that the rewritten subtree resolves without incident under a strict-mode program. 1 error, exit 2 |
| `component-identifier.js` | `test/Parser/flow/component-syntax/component-identifier.js` | `export default component + 1;` — `component` used purely as an IDENTIFIER, so the file parses identically with and without `-parse-flow` (checked both ways) and reaches the export visit either way; imported flagless. Not a vacuous parse-error match: hermesc reports exactly the one `'export'` error, at 52:3. 1 error, exit 2 |

### Sweep result

The full S2-T8/S3-T3 sweep was re-run with the same tooling — both binaries,
raw stdout + stderr + exit status, no extra flags on either side — over the
same 1416 files in the same 8 upstream dirs (count re-verified: `find
test/{Parser,IRGen,BCGen,Optimizer,hermes,AST,Driver,RA} -iname '*.js' | wc
-l` = 1416):

| Outcome | S2-T8 | S3-T3 | S4a-T3 | Delta vs S3-T3 |
|---|---|---|---|---|
| byte-identical | 1203 | 1209 | **1218** | +9 |
| mismatch | 190 | 190 | **190** | 0 |
| panic | 23 | 17 | **8** | −9 |

1218 + 190 + 8 = 1416. The raw exit-shape pass reads 1218 / 188 / 10; the two
extra "panics" are `test/Parser/nested-expressions.js` and
`test/hermes/far-environment-access.js`, the two stack-overflow `SIGABRT`
files the S3-T3 section already places in the mismatch bucket (the
recursion-depth-parity landmine, where both sides fail differently). Applying
that same established convention gives the 190 / 8 above — the identical
reconciliation S3-T3 documents, and the only one applied.

The residual 8 panics are, exhaustively (each message read, not assumed):
seven `calls.rs` `$SHBuiltin.moduleFactory needs visitModuleFactory
(cpp:1320-1366) — S4 modules` files (`test/BCGen/HBC/xmod-requires-opt.js`,
`test/Optimizer/xmod-{builtins,require-cse,requires-opt-extension,
requires-opt}.js`, `test/hermes/xmod-exec-require{-bad-func,}.js`) plus
`test/hermes/computed-fn-name.js`'s `not all scopes were visited` assertion
(the pre-existing C++ defect reproduction). **Zero** catch-all "unhandled
node kind" panics remain in the sweep. `test/Sema/xmod-errors.js` (the
Deferred row) still panics the same way, unchanged.

Gate as of this task: `sema differential (tests/sema_corpus): 187 corpus
files matched (100 succeeded on hermesc)` — 176 → **187** files (+11: 2
authored in Step 2, 9 imported in Step 4), hermesc-succeeded **100**,
UNCHANGED, because all eleven new files are error-path pins (hermesc exit 2
on every one: a module declaration without `-commonjs` is always an error).
Arithmetic: 176 + 2 + 9 = 187; 100 + 0 + 0 = 100.

## S4a Task 4: the untyped `-parse-flow` corpus battery

Three new files, each hermesc-verified FIRST (raw stdout+stderr+exit, `-parse-
flow` on both sides) before being added. Both diagnostics this battery targets
were already ported before this task — `visit(CoverTypedIdentifierNode *)`
(SemanticResolver.cpp:1575-1577, `resolver/expressions.rs:966`, ported
unconditionally even though the C++ site is `#if HERMES_PARSE_FLOW`, per the
single-node-set precedent) and the `this`-parameter check inside
`declareParams` (cpp:1767-1771, `resolver/functions.rs:897`, gated `compile_
&& !typed_`) — so this task is pure corpus work, no resolver changes.

| File | Covers |
|---|---|
| `flow-typecast-cover.js` | `visit(CoverTypedIdentifierNode *)`. The task brief's sketch shape, `(x: number);`, does **not** reach this visit: JSParserImpl.cpp:2633-2640 converts a non-optional cover node carrying a type annotation into a `TypeCastExpressionNode` right inside the parenthesized-expression parser (`cover->_right && !cover->_optional`), before sema ever runs. The OPTIONAL form does — `x?: number` parses `?` first (`tryParseCoverTypedIdentifierNode(test, /*optional=*/true)`, cpp:4517-4528), so `_optional = true` skips that rewrite and the cover node survives as the parenthesized expression's value when it is not consumed as arrow parameters. `(x?: number);` verified directly against hermesc: `error: typecast not allowed in this context`, caret+range over `x?: number` (10 columns, matching the node's `test`-start-to-prev-token-end range from `tryParseCoverTypedIdentifierNode`), exit 2. Non-optional `(x: number);` was probed too, confirming it takes the `TypeCastExpressionNode` path instead and does not exercise this visit — not used, since it would test the wrong node |
| `flow-this-param.js` | The `this`-parameter check. `function f(this: number) {}` — Flow's parser accepts a `this` parameter (typing the receiver), but `typed_` is always false in this untyped-dialect port, so `compile_ && !typed_` fires. Contrast: without `-parse-flow`, the parser itself rejects `this` in a binding position first (`identifier, '{' or '[' expected in binding pattern`, per MANIFEST's S2 Task 8 note), so `-parse-flow` is required to reach this diagnostic at all. hermesc: exit 2, 1 error at the parameter's range |
| `flow-annotations-benign.js` | Negative control, pinning that ordinary annotations don't perturb resolution: `function f(x: number): number { return x; } var y: string;` resolves with the exact same `SemContext`/AST dump shape (decls, scopes, `[D:E:...]` annotations on `f`, `x` and `y`) that the equivalent untyped file would produce — the `TypeAnnotation`/`GenericTypeAnnotation` subtrees are simply never visited as expressions. hermesc: exit 0, full dump byte-for-byte |

Gate as of this task: `sema differential (tests/sema_corpus): 190 corpus
files matched (101 succeeded on hermesc)` — 187 → **190** files (+3, all
authored), hermesc-succeeded 100 → **101** (+1: `flow-annotations-benign.js`
is the only exit-0 file of the three; the other two are error-path pins, like
every other diagnostic-shape file in this corpus). Arithmetic: 187 + 3 = 190;
100 + 1 = 101.

## S4a Task 4 fix review

A review of this task found an Important gap: `visit(TypeCastExpressionNode
*)` (SemanticResolver.cpp:1591-1594) was never ported, even though the
task's own derivation for `flow-typecast-cover.js` had already shown that
`(x: number);` — the task brief's original sketch — resolves to exactly this
node under hermesc (exit 0), not to a `CoverTypedIdentifierNode`. The
derivation reasoning was used to pick a DIFFERENT, correct corpus shape
(`(x?: number);`) but the positive shape it ruled out was never itself
probed against `sema-dump`, so the gap went unverified on the Rust side:
`sema-dump -parse-flow` on `(x: number);` panicked at `visit_node`'s
catch-all, `sema: unhandled node kind TypeCastExpression (S3+/typed
phases)`. The sibling `visit(AsExpressionNode *)` (cpp:1596-1599, Flow's `as`
operator, also unconditional on `typed_`) was probed on the same reasoning
and found to have the identical gap (`x as number;` panicked the same way).

Both are now ported in `resolver/expressions.rs`
(`visit_type_cast_expression`, `visit_as_expression`) and wired into
`resolver/mod.rs`'s `visit_node` dispatch, faithfully carrying the C++
comment both share ("Visit the expression, but not the type annotation"):
each visits only `_expression` through the generated builder (`self.call`
with `NodeField::expression`, the same one-field-at-a-time pattern
`visit_assignment_expression` uses, per the module doc's "work between two
children" section) and leaves `_type_annotation` untouched — mirroring the
`ObjectPattern`/`ArrayPattern` `_typeAnnotation` skip already documented
there. `AsConstExpressionNode` (`x as const`) was checked and found to have
**no** C++ `visit()` override at all (SemanticResolver.h's inventory), so it
correctly belongs to the generic catch-all/whitelist arm like
`NewExpression`; it is not ported here because no corpus file reaches it
(documented at `visit_as_expression`'s doc comment as the pointer for
whoever's corpus does).

The catch-all panic's tag was also corrected: `(S3+/typed phases)` →
`(S3+/dialect phases)`, in `resolver/mod.rs` (the module-doc mirror and the
panic string). The old tag implied every remaining unhandled kind needed
`-typed`, which was demonstrably false for these two (both fire under plain
`-parse-flow`) and would mislead the next person chasing a similar gap; "S3+"
(genuinely-future-phase kinds) and "dialect" (Flow/TS-only kinds, typed or
not) is the accurate split. Message-only change, no restructuring; the one
place that quotes the OLD string verbatim (`MANIFEST.md`'s S2 Task 8 section,
recording what the panic said when that historical sweep ran) is left
alone, since it is a dated quotation, not live documentation.

Two new corpus files, each hermesc-verified FIRST (raw stdout+stderr+exit,
`-parse-flow` on both sides) and then confirmed against `sema-dump` directly
before being added to the gate:

| File | Covers |
|---|---|
| `flow-typecast-resolves.js` | `visit(TypeCastExpressionNode *)`. `var x: number; (x: number);` — `x` is declared first so the dump shows a real `[D:E:...]` resolution on the identifier inside the cast, not just an on-the-fly `UndeclaredGlobalProperty`; the type annotation itself is never walked. hermesc: exit 0, full dump match |
| `flow-as-expression.js` | `visit(AsExpressionNode *)`. `var x = 1; x as number;` — same resolving-identifier shape as the file above, for the `as` operator. hermesc: exit 0, full dump match |

Gate as of the fix: `sema differential (tests/sema_corpus): 192 corpus files
matched (103 succeeded on hermesc)` — 190 → **192** files (+2, both
authored, both exit-0), hermesc-succeeded 101 → **103** (+2: both new files
succeed). Arithmetic: 190 + 2 = 192; 101 + 2 = 103.

## S4a Task 5: upstream re-probe

S4a Task 5 re-ran the exact S2-T8/S3-T3 sweep — both binaries, raw stdout +
stderr + exit status, no extra flags on either side — over the same 1416
files in the same 8 upstream dirs (`test/Parser` 366,
`test/IRGen`+`test/BCGen`+`test/Optimizer` 395,
`test/hermes`+`test/AST`+`test/Driver`+`test/RA` 655), now that S4a Tasks 1-4
have landed the module visits, the `// FLAGS:` harness and the
`TypeCastExpression`/`AsExpression` visits. File count re-verified: `find
test/{Parser,IRGen,BCGen,Optimizer,hermes,AST,Driver,RA} -iname '*.js' | wc
-l` = 1416, unchanged. Both binaries were plain debug builds (`cargo build
--manifest-path rust/Cargo.toml -p sema --features dump-bin`, no
`--release`; `hermesc` is the prebuilt ASan+Debug binary) — load-bearing, see
the note under "Zero S4a-attributable panics" below.

### Result

| Outcome | S2-T8 | S3-T3 | S4a-T5 | Delta vs S3-T3 |
|---|---|---|---|---|
| byte-identical | 1203 | 1209 | **1218** | +9 |
| mismatch | 190 | 190 | **190** | 0 |
| panic | 23 | 17 | **8** | −9 |

1218 + 190 + 8 = 1416 (S3-T3's 1209 + 190 + 17 = 1416, same total). This
matches the PREVIEW S4a-T3's own report recorded (1218/190/8) exactly,
**confirming** — by the formal run, not by assumption — the brief's
expectation that T4's `TypeCastExpression`/`AsExpression` visits do not move
any flagless-sweep bucket: the sweep passes no flags to either binary, and
`TypeCastExpressionNode`/`AsExpressionNode` can only be produced by the
parser's `l_paren`/`as`-operator rewrites when Flow parsing is enabled
(`JSParserImpl.cpp:2633-2640`, `:4329-4350`, both `#if HERMES_PARSE_FLOW`,
re-derived by S4a T4's own report) — structurally unreachable from any
upstream file parsed without `-parse-flow`.

### The +9 / −9 move, named

The moved set is exactly the **nine** files S4a Task 3's own "Step 4" table
already names and imports into this corpus — no other file's bucket changed.
Each was independently re-checked against `identical.txt` from this task's
sweep:

| File | S3-T3 | S4a-T5 |
|---|---|---|
| `test/Parser/es6/import.js` | panic (`mod.rs` catch-all, `ImportDeclaration`) | identical |
| `test/Parser/es6/import-location.js` | panic (`mod.rs` catch-all, `ImportDeclaration`) | identical |
| `test/Parser/es6/import-assertions.js` | panic (`mod.rs` catch-all, `ImportDeclaration`) | identical |
| `test/Parser/es6/export.js` | panic (`mod.rs` catch-all, `ExportAllDeclaration`) | identical |
| `test/Parser/es6/export-default.js` | panic (`mod.rs` catch-all, `ExportDefaultDeclaration`) | identical |
| `test/Parser/es6/export-default-class.js` | panic (`mod.rs` catch-all, `ExportDefaultDeclaration`) | identical |
| `test/Parser/es6/export-default-async.js` | panic (`mod.rs` catch-all, `ExportDefaultDeclaration`) | identical |
| `test/AST/es6/export-default-function.js` | panic (`mod.rs` catch-all, `ExportDefaultDeclaration`) | identical |
| `test/Parser/flow/component-syntax/component-identifier.js` | panic (`mod.rs` catch-all, `ExportDefaultDeclaration`) | identical |

These are precisely the "16 S4 files" sub-bucket of nine `mod.rs`-catch-all
panics S3-T3's own "Zero S3-attributable panics" section enumerated (the
other seven of that "16" were already the `calls.rs` `$SHBuiltin` panics,
which stay put — see below). **No identical-but-not-imported module files
exist**: 1218 − 1209 = 9, matching the nine above one-for-one, and the
mismatch bucket's total is unchanged (190 = 190), so no file entered or left
it either. (Full file-level mismatch-set diffing the way S3-T3 did against
an adjacent pre/post commit pair was not repeated here — S4a T1-T4 touched no
diagnostic-geometry code, only added dispatch arms for previously-panicking
kinds, so there is no mechanism by which a mismatch-bucket file could change
shape; the unchanged total count is the expected, and sufficient, evidence.)

### Zero S4a-attributable panics

The residual 8-file panic bucket is, exhaustively (each message read, not
assumed, from this task's own `panic.txt`):

- **7 `calls.rs:312` `$SHBuiltin.moduleFactory needs visitModuleFactory
  (cpp:1320-1366) — S4 modules` panics** — `test/BCGen/HBC/
  xmod-requires-opt.js`, `test/Optimizer/xmod-{builtins,require-cse,
  requires-opt-extension,requires-opt}.js`,
  `test/hermes/xmod-exec-require{-bad-func,}.js`. Untouched by design: the
  global constraints reserve the `$SHBuiltin` module branches for S4b.
- **1 pre-existing-C++-defect reproduction** — `test/hermes/
  computed-fn-name.js`, the same `SemContext.cpp:478` scope-walk assertion
  S2 T4/S3 T3 already documented. This one is **debug-build-only on both
  sides**: hermesc's assertion and this port's `dump_context.rs:241`
  `debug_assert_eq!` are both compiled out of release builds (the "a release
  hermesc (no assertions) dumps the incomplete scope tree instead" note
  already on file, above). A first pass of this sweep was run against a
  `--release` `sema-dump` by mistake and read **1218/190/7** with
  `computed-fn-name.js` sorted into `identical` (both sides exit 0, full
  dump, byte-identical) instead of `panic` — silently masking the
  known-defect reproduction rather than fixing anything. Rebuilding
  `sema-dump` as a plain debug binary (matching the brief's
  `cargo build`-no-`--release` instruction and `hermesc`'s own ASan+Debug
  build) restored the expected 1218/190/8. Flagged here as a sweep-tooling
  landmine for whoever reruns this: **the sweep is only meaningful with
  debug builds on both sides**, because at least one landmine
  (`computed-fn-name.js`) is itself gated on assertions being compiled in.

**Zero** of the 8 panics are attributable to S4a: none mention a module kind
this phase's visits should handle, and the eighth is a documented,
faithfully-reproduced upstream C++ defect, not a port gap. No fix was
needed.

### Step 1: the four remaining Deferred rows, re-probed

Every row in the "Deferred" table above was re-run through both binaries
(raw stdout + stderr + exit status, `hermesc -dump-sema` vs `sema-dump`,
debug builds). **None unblocked**; each row's stated reason was confirmed,
not assumed:

| File | Re-probe result |
|---|---|
| `deep-ast-err.js` | still a vacuous match (comment-only file, both exit 0, byte-identical); still excluded on purpose, not a real gap |
| `invalid-args-eval.js` | still the SAME single same-location diagnostic-order tie at `89:9` (`the variable "arguments" was not declared` warning vs `cannot declare 'arguments' in strict mode` error) — the two sides emit the identical set of messages, only in the opposite relative order, because C++'s `std::sort` over the buffered-message array is unstable and this port's `sort_by_key` is stable. Unchanged, unfixable-by-construction (`support/src/manager.rs:903-909`'s documented deviation) |
| `regress-nested-expressions-error.js` | still `10:3052` (hermesc) vs `10:6124` (sema-dump) — the same recursion-depth-counting-rate mismatch, re-verified with a fresh diff of both stderrs. Unchanged; tracked as the parser-track recursion-depth-parity follow-up, not this phase's to fix |
| `xmod-errors.js` | still panics identically: `sema: $SHBuiltin.moduleFactory needs visitModuleFactory (cpp:1320-1366) — S4 modules` at `calls.rs:312`, same call site as the seven upstream `xmod-*.js` files above. **Confirmed still blocked — S4b**, exactly as the brief requires |

No upstream `test/Sema` row newly matches (all four are still the same
established gaps: a C++ unstable-sort tie, a recursion-depth-counting
mismatch, an S4b module-protocol dependency, and a vacuous comment-only
file), so **Step 2 imports nothing**.

### Gate (unchanged)

`REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p sema
--features dump-bin --test sema_differential -- --nocapture`:
`sema differential (tests/sema_corpus): 192 corpus files matched (103
succeeded on hermesc)`, `sema differential (tests/sema_corpus_parser): 7
corpus files matched (2 succeeded on the oracle)` — both exactly as before
this task, since Step 2 imported nothing and no code changed. Full workspace
`cargo test --workspace`: all suites green (no `FAILED`, no cargo
`error[`/`warning:`, in either the `--features sema/dump-bin` or plain
config); `cargo clippy -p sema --all-targets --features dump-bin` emits
nothing for `sema` (the workspace's other warnings remain the pre-existing
ones in `parser`, untouched, same as every prior task's note).

## Parser-track T1 (recursion-depth parity): the recursion row, closed

The parser track's recursion-parity task landed the fix the Deferred
`regress-nested-expressions-error.js` row had been waiting on since S1, so
that row is now **Imported** (Deferred 4 → **3**) together with a second
upstream witness. Two corrections to what this MANIFEST previously recorded,
both measured, not assumed:

**1. The row's stated cause was wrong.** The row said the two sides'
recursion trackers "increment at different rates per grammar production …
even though both share the same `MAX_RECURSION_DEPTH = 1024`". They do not
share it, and the rates were never different. T1's audit mapped all 20
`CHECK_RECURSION` sites (`JSParserImpl.cpp` 17 + `JSParserImpl-ts.cpp` 3) plus
the per-chain-link increment at `JSParserImpl.cpp:3527-3535` to their Rust
productions **in both directions**: every site present, at the same scope,
none missing, none extra. Across 34 nesting ladders the trip points differed
by a CONSTANT 897 levels — a fixed offset, not a rate — which decomposes
exactly:

- **896** = 1024 − 128. `cmake-build-asan/bin/hermesc` is an AddressSanitizer
  build, so `HERMES_LIMIT_STACK_DEPTH` is defined
  (`include/hermes/Support/Compiler.h:106-110`) and the oracle's limits are
  `JSParserImpl::MAX_RECURSION_DEPTH` = **128** (`JSParserImpl.h:189-202`) and
  `ESTree::kASTMaxRecursionDepth` = **512** (`RecursiveVisitor.h:686-692`),
  not the 1024/1024 the port had hardcoded.
- **1** = an off-by-one: C++ `recursionDepthCheck()` (`JSParserImpl.h:699-704`)
  errors unless the POST-increment depth is still `< MAX`, i.e. at `>=`; the
  port tested `>`.

A third, independent divergence was found in this task's review and fixed with
it: the diagnostic's **caret geometry**. All three Rust emitters of
`Too many nested expressions/statements/declarations` (`js/mod.rs`'s
`check_recursion`, the member-chain loop site, and the
`MAX_NESTED_ASSIGNMENTS` guard) reported the current token's RANGE, but every
one of them corresponds to C++ `recursionDepthExceeded`
(`JSParserImpl.cpp:348-352` — the loop site reaches it via
`recursionDepthCheck()`, the assignment guard calls it directly at
`cpp:6514`), which uses `error(tok_->getStartLoc(), …)`, the `error(SMLoc,
Twine)` overload (`JSParserImpl.h:472-474`). Same message, same `line:col`,
different rendering: a bare `^` versus `^~~~~` on any trip token wider than
one character. Fixed at all three sites; pinned by the two new corpus files
below.

Both are fixed. The limits are now profile-selected on the Rust side
(`cfg!(debug_assertions)` → 128/512, matching the branch the ASan oracle
takes; 1024/1024 otherwise, matching a C++ release build), so **the harness
now depends on build-profile pairing for a second reason** — see the
BUILD-PROFILE PAIRING note in `sema_differential.rs`'s module doc, next to the
existing `--release` masking gotcha.

**2. `test/hermes/far-environment-access.js` no longer crashes.** The row's
S2-T8 upgrade ("`sema-dump` STACK-OVERFLOWS and aborts (SIGABRT/134) before
its own tracker trips") was the direct consequence of the same defect: a
1024-level budget that an unoptimized build's frames cannot afford. With the
debug limit at 512 it diagnoses instead, at hermesc's own `28:510`.

### New files (4)

| File | Source | Note |
|---|---|---|
| `nested-expressions.js` | `test/Parser/nested-expressions.js`, verbatim | the PARSER limit's error side: both sides `12:46: error: Too many nested expressions/statements/declarations`, exit 2, all three channels byte-identical. Was a sweep mismatch (SIGABRT on our side); now identical |
| `regress-nested-expressions-error.js` | `test/Sema/regress-nested-expressions-error.js`, verbatim | the RESOLVER limit's error side: both sides `10:3052`, exit 2, byte-identical. The `get<<=…` chain never touches the parser's counter at all — `parseAssignmentExpression` is iterative over an explicit stack (`JSParserImpl.cpp:6496-6522`) — so the old `3052`-vs-`6124` gap was entirely `kASTMaxRecursionDepth` 512-vs-1024 |
| `nested-unary-multichar-limit.js` | authored | the diagnostic's caret GEOMETRY on the `check_recursion` emitter: 126 `typeof` levels tripping on the 5-character identifier `xyzzy`. C++ `recursionDepthExceeded` (`JSParserImpl.cpp:348-352`) reports through `error(tok_->getStartLoc(), …)` — the `error(SMLoc, Twine)` overload (`JSParserImpl.h:472-474`) — so the caret is bare (`^`), NOT the token's underlined range (`^~~~~`). Every earlier pin trips on a one-character token, where the two are indistinguishable |
| `nested-tagged-template-limit.js` | authored | the same geometry on the OTHER emitter: the member-chain loop's per-link increment (`JSParserImpl.cpp:3527-3535`), 127 tagged-template links tripping on a whole `` `beta` `` token |

The clean side of the boundary is pinned on the parser track instead, where
the differential can compare a SUCCESS dump: `parser/tests/parser_corpus/
nested-parens-limit.js` (125 nested parens = N*−1 for that shape; 126 errors
on both sides), plus `parser/tests/recursion_depth_limit.rs`, which pins both
sides of the boundary and the rendered diagnostic without needing an oracle
present.

### Sweep re-count

Same 1416 files, same 8 upstream dirs, same method (both binaries, raw stdout
+ stderr + exit status, no extra flags, **debug builds on both sides** — the
`--release` landmine documented under S4a-T5's "Zero S4a-attributable panics"
now has a second, unrelated way to lie, see above). File count re-verified:
`find test/{Parser,IRGen,BCGen,Optimizer,hermes,AST,Driver,RA} -iname '*.js'
| wc -l` = 1416, unchanged.

| Outcome | S3-T3 | S4a-T5 | parser-T1 | Delta vs S4a-T5 |
|---|---|---|---|---|
| byte-identical | 1209 | 1218 | **1220** | +2 |
| mismatch | 190 | 190 | **188** | −2 |
| panic | 17 | 8 | **8** | 0 |

1220 + 188 + 8 = 1416. The moved set is exactly the **two** files this row
always named — `test/Parser/nested-expressions.js` and
`test/hermes/far-environment-access.js` — each independently confirmed in this
run's `identical.txt`. No other file's bucket changed: the panic bucket is
byte-for-byte the same 8 files as S4a-T5 (seven `calls.rs:312`
`$SHBuiltin.moduleFactory` S4b files plus `test/hermes/computed-fn-name.js`'s
pre-existing-C++-defect reproduction), and mismatch 190 − 2 = 188 accounts for
the rest.

One tooling note worth recording: S4a-T5's raw pass needed a reconciliation
step, because the two files above aborted (`SIGABRT`) on our side and a raw
exit-shape bucketing calls that "panic", so the convention was to reclassify
them into "mismatch". **That reconciliation is now moot** — they exit 2 like
the oracle — so this run's raw pass reads 1220 / 188 / 8 directly, with no
convention applied. The two numbers agreeing without the fixup is independent
confirmation that the convention was describing exactly these two files.

### Gate

`REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p sema
--features dump-bin --test sema_differential -- --nocapture`:
`sema differential (tests/sema_corpus): 196 corpus files matched (103
succeeded on the oracle)` — 192 → **196** (+4: two imported and two authored
above), hermesc-succeeded **103** UNCHANGED, because all four new files are
error-path pins (hermesc exit 2 on each). Arithmetic: 192 + 4 = 196;
103 + 0 = 103.
`sema differential (tests/sema_corpus_parser): 11 corpus files matched (3
succeeded on the oracle)` — unchanged, nothing imported there. Parser track:
`parser differential (tests/parser_corpus): 77 corpus files matched` — 76 →
**77** (+1, `nested-parens-limit.js`); the seven dialect corpora unchanged.

## Whole-Sema capstone fixes (2026-08-04): the untyped `-parse-flow` surface

The whole-Sema capstone review (verdict APPROVED WITH FIXES; 0 Critical,
2 Important, 3 Minor) found that S4a's untyped-`-parse-flow` claim was
materially incomplete: **ten reachable shapes** panicked at
`resolver/mod.rs`'s catch-all where hermesc exits 0 with a full dump. All ten
are `-parse-flow` only (no `-typed`), so this corpus — not the typed-dialect
track — is where they belong. Three C++-prescribed mechanisms were missing:

1. **`visit(TypeParameterInstantiationNode *)`** (SemanticResolver.cpp:
   1587-1589) — a do-nothing visit, exactly like the `TypeAlias` one next to
   it. Reachable through three parents' children walks: `CallExpression`,
   `NewExpression` and `OptionalCallExpression`. Its sibling
   `visit(TypeParameterDeclarationNode *)` (cpp:1583-1585) was ported at the
   same time for completeness even though the function/class visits
   hand-drive their children and never dispatch it.
2. **The `ObjectPattern`/`ArrayPattern` overrides** (SemanticResolver.h:
   209-214). Both had been served by `visit_node`'s override-free generic
   arm, on the argument that they reduce to it for patterns with no type
   annotation — true until untyped `-parse-flow` made ANNOTATED destructuring
   reachable (`var {a}: Obj = ...`, `function g({a}: Obj) {}`), at which
   point the generic arm's `type_annotation` walk became a real divergence.
   Ported for real in `resolver/declarations.rs` (`visit_object_pattern` /
   `visit_array_pattern`), visiting only `_properties`/`_elements`.
3. **The override-free Flow node range.** `InterfaceDeclaration`,
   `EnumDeclaration`, `OpaqueType` and the `Declare*` family appear NOWHERE
   in SemanticResolver.h's `visit` inventory, so C++ reaches them through the
   header's default `visit(ESTree::Node *node) { visitESTreeChildren(*this,
   node); }` (:191-193) — which is observable, not assumed: hermesc resolves
   the interface's `Id 'I'`, the enum's `Id 'E'` and the interface body's
   property keys as ordinary `UndeclaredGlobalProperty` identifiers.
   `resolver/mod.rs` now carries ONE arm for the whole `NodeKind::_Flow_First
   .._Flow_Last` range (the AST's Flow section, ESTree.def:854-1272, inside
   the `#if HERMES_PARSE_FLOW` block at :852-1274), placed
   after the five in-range kinds that DO have overrides (`TypeAlias`,
   `TypeParameterDeclaration`, `TypeParameterInstantiation`,
   `TypeCastExpression`, `AsExpression`). The range spelling is deliberate
   and argued at the site: enumerating ~90 kinds would only invite the next
   gap of this exact shape. `ComponentDeclaration`/`HookDeclaration` are
   OUTSIDE that range (they are function-like statement nodes needing
   `visitFunctionLike`) and correctly keep falling through to the panic —
   they need `-parse-component-syntax` and are a dialect phase.

### Four new files, one concern each

| File | Covers |
|---|---|
| `flow-type-args.js` | `f<Foo>(1)`, `new C<Bar>()`, `f?.<Baz>(1)` — all three `TypeParameterInstantiation` parents. The do-nothing visit is visible in the dump: the callees `f`/`C` resolve, `Foo`/`Bar`/`Baz` inside the type arguments do NOT (no `[D:E:...]`); a `GenericTypeAnnotation` `_id` is used rather than `number`'s childless `NumberTypeAnnotation` because the latter can't distinguish walked from unwalked. hermesc: exit 0 |
| `flow-pattern-annot.js` | annotated destructuring in five shapes — `var {a}: Obj`, `var [b]: Arr`, an annotated object pattern as a function PARAMETER (reached from `declareParams`' walk, not a variable declaration), a nested pattern, and one with a default. Every binding resolves; `Obj`/`Arr` get no annotation because the type annotation is never visited. hermesc: exit 0 |
| `flow-interface-enum.js` | `interface I { x: number }` and `enum E { A, B }`, deliberately next to a `type A = number;` so the dump shows the CONTRAST between the children walk (`I`, `E`, and the body's `x` all resolve) and `TypeAlias`'s do-nothing visit (`A` gets nothing); plus `interface J<T: typeof host, U = typeof host> { b: typeof host }` pinning `TypeParameterDeclaration`'s own do-nothing visit — `host` in `T`/`U` never resolves even though the body's `host` does. hermesc: exit 0 |
| `flow-declare-opaque.js` | `opaque type B = string;` next to `type A = string;` — near-identical syntax with OPPOSITE dispatch, so a port that lumped `OpaqueType` in with `TypeAlias` fails right here — plus `declare var dv: number;` and `declare function df(): void;` for the `Declare*` family. hermesc: exit 0 |

`flow-annotations-benign.js` was not re-verified-and-changed but its header
comment was corrected: it claimed `TypeAnnotation`/`GenericTypeAnnotation`
"are not on `visit_node`'s dispatch", which the new Flow-range arm makes
false. The FILE's behavior is unchanged and still byte-identical — the
annotations there hang off an `Identifier`, whose visit does not walk its
children, so they are never dispatched on that path at all.

### `// FLAGS:` harness note — value-taking options must use `=`

hermesc (via LLVM `cl`) accepts both `-ferror-limit=2` and the
space-separated `-ferror-limit 2`; the `command_line` crate this port's
binaries use only ever reads a value out of the SAME argv element, so only
the `=` form works on the Rust side. Since `per_file_flags` appends the FLAGS
line VERBATIM to both binaries' argv, a space-form spelling would have
hermesc apply the option while `sema-dump` died on it — comparing two
different runs rather than reporting a mismatch. **Always spell them with
`=`.** (The capstone's finding F3 was the related exit code: a usage error
used to `exit(0)`, i.e. report success with no dump; `command_line`'s
`parse_env_args` now exits 1, matching LLVM's `ParseCommandLineOptions` and
hermesc's own exit 1 on a bad option. Closing the space-form spelling gap
itself is a `command_line`-crate port item, not a Sema one.)

### Gate

`sema differential (tests/sema_corpus): 200 corpus files matched (107
succeeded on the oracle)` — 196 → **200** (+4, all authored above),
hermesc-succeeded 103 → **107** (all four are exit-0 files, the first
success-path additions since S4a T4's fix review). Arithmetic:
196 + 4 = 200; 103 + 4 = 107.
`sema differential (tests/sema_corpus_parser): 11 corpus files matched (3
succeeded on the oracle)` — unchanged; the capstone's parser-pair findings
(F2's `$SHBuiltin` `compile = false` semantics and the `with` debug-abort
landmine) are documented in that corpus's MANIFEST rather than pinned, both
because their shapes still panic or abort on one side.

---

## errorExpected geometry (Task 1) additions

C++ `JSParserImpl::errorExpected` (JSParserImpl.cpp:175-226) has two
rendering arms that the port had dropped, keeping only the message text.
Both are now ported, and both get a corpus file whose three channels
(stdout, stderr, exit status) were captured from
`cmake-build-asan/bin/hermesc -dump-sema` BEFORE the fix — each file failed
the differential at that point, on stderr.

| File | Covers |
|---|---|
| `error-expected-same-line.js` | The same-line arm (cpp:212-219). `var a = (1 + 2;` — `whatLoc` (the `(`, from the parenthesized-expression `eat` at cpp:2655-2660) shares line 1 with the offending `;`, so hermesc emits ONE diagnostic underlined with `combineIntoRange(whatLoc, errorLoc)`: `~~~~~~^` starting at the `(`. Pre-fix the port rendered a bare `^` (identical text, line:col and exit status — stderr geometry only). hermesc: exit 2 |
| `error-expected-cross-line-note.js` | The different-line arm (cpp:220-225). `try` on line 1, `xyz;` on line 2 — `whatLoc` (the `try`, from the `need` at cpp:2371) is on an earlier line than the error, so hermesc emits a bare point-caret error AND a second `note: location of 'try'` diagnostic at `whatLoc`. Pre-fix the port emitted the error but no note at all. hermesc: exit 2 |

Both files are one-liner reproducers rather than `test/Sema` imports (the
same footing as `parse-error.js`, the S1 errorExpected gap-filler): the
shapes being pinned are two-token geometries, and any extra source would
only add unrelated dump output between the port and the oracle.

### Gate

`sema differential (tests/sema_corpus): 202 corpus files matched (107
succeeded on the oracle)` — 200 → **202** (+2, both authored above),
hermesc-succeeded **107 unchanged** (both new files are error files, exit 2).
Arithmetic: 200 + 2 = 202; 107 + 0 = 107.
`sema differential (tests/sema_corpus_parser): 11 corpus files matched (3
succeeded on the oracle)` — unchanged.

## errorExpected geometry (Task 3) — sweep re-count

Full re-run of the S2-T8/S3-T3/S4a-T5 upstream sweep methodology (both
binaries, raw stdout+stderr+exit status, no extra flags, debug builds on
both sides, over the same 1416 `.js` files in `test/{Parser,IRGen,BCGen,
Optimizer,hermes,AST,Driver,RA}`) now that Task 1 (both `errorExpected`
rendering arms) and Task 2 (all 246 C++ call sites restored) have landed.
File count re-verified: `find test/{Parser,IRGen,BCGen,Optimizer,hermes,AST,
Driver,RA} -iname '*.js' | wc -l` = 1416, unchanged.

Baseline going into this task (the pre-existing `parser-T1` row already in
this table, `db8c7d2d1`): **1220 / 188 / 8**. First re-run, T1+T2 already
applied: **1337 / 71 / 8** — 117 files moved mismatch → identical, panic
bucket byte-for-byte the same 8 files, zero regressions (every file
byte-identical before Task 1 stayed byte-identical; no file moved into
mismatch or panic). 1220 + 117 = 1337; 188 − 117 = 71.

### Classifying the 71: one dominant new bug, not "small residue"

The task's own self-review notes guessed the residue would land near ~8
files. Classifying all 71 individually instead surfaced a real,
well-understood, previously-undiscovered defect family: **`error(SMLoc,
Twine)` call sites — C++'s POINT-location overload (JSParserImpl.h:472-474,
bare caret, no underline) — mis-ported as the RANGE overload** (the current
token's or a node's full extent underlined). This is the same rendering
confusion `errorExpected`'s own two arms are about, but at call sites that
never go through `errorExpected`/`need`/`eat` at all, so Task 2's 246-site
sweep had no reason to touch them. Per-message breakdown of the 71 (one
`hermesc` stderr first-line grouping):

| Count | hermesc first line |
|---|---|
| 57 | `error: ';' expected` |
| 2 | `error: unexpected token after yield expression` |
| 2 | `error: identifier, '{' or '[' expected in binding pattern` |
| 1 each | the remaining 10 (warning + 9 distinct error messages) |

**Fixed, 7 call sites, all verified against the C++ signature and rebuilt
against the full sweep after each fix (zero regressions at every step):**

1. **`eat_semi`** (`parser/src/js/statements.rs`, was `error_cur`, now
   `error_at_loc(self.cur_start(), ...)`) — C++ `eatSemi`
   (JSParserImpl.cpp:336) calls `error(tok_->getStartLoc(), "';' expected")`,
   the point overload. Single highest-value fix in the sweep: 71 → 13
   (58 files, the 57 `';' expected` files plus one `identifier, '{' or '['
   expected in binding pattern` file whose actual diff was on a later
   `eatSemi`-produced line, not its first diagnostic).
2. **`parse_binding_element`**'s no-identifier branch (`statements.rs`, was
   `error_cur`, now `error_at_loc(self.cur_start(), ...)`) — C++
   (JSParserImpl.cpp:1374-1376) calls `error(tok_->getStartLoc(), ...)`.
   Fixed 3 files: `test/IRGen/flow/function-prototype-call.js`,
   `test/Parser/flow/this-param.js`, `test/Parser/escaped-this.js`.
3. **The labeled-`FunctionDeclaration` check** (`statements.rs`, was
   `error_at(func.range(), ...)`, now `error_at_loc(func.range().start,
   ...)`) — C++ (JSParserImpl.cpp:1653-1655) calls
   `error(optFunc.getValue()->getSourceRange().Start, ...)`. Fixed
   `test/Parser/es6/generator-error.js`.
4. **The if-statement function-declaration checks** (`statements.rs`, both
   the strict-mode and the generator/async check, were `error_at(function
   .range(), ...)`, now `error_at_loc(function.range().start, ...)`) — C++
   (JSParserImpl.cpp:1716-1723) calls `error((*optFunction)->getStartLoc(),
   ...)` for both. Fixed `test/Parser/if-function-error.js` (2 diagnostics)
   and `test/Parser/if-function-gen-error.js`.
5. **The post-assignment-expression check** (`expressions.rs`, was
   `error_at(range, ...)` with `range = self.cur_range()`, now
   `error_at_loc(self.cur_start(), ...)`) — C++ (JSParserImpl.cpp:6535-6536)
   calls `error(tok_->getStartLoc(), ...)`. Fixed
   `test/Parser/regress-assign-end-error.js`.
6. **"invalid destructuring target"** (`expressions.rs`, a hand-inlined
   `combineIntoRange` that ended the range AT the key's start instead of one
   past it, now routed through the real `combine_into_range` helper
   (`support/src/manager.rs:411`, itself already correct)) — C++
   (JSParserImpl.cpp:6095-6098) is `SourceErrorManager::combineIntoRange
   (propNode->getStartLoc(), propNode->_key->getStartLoc())`, and
   `combineIntoRange` (header:601-607) always extends the end one byte past
   its later argument; the hand-inlined version dropped that `+1`, off-by-one
   shortening the underline by one character. Fixed
   `test/Parser/destr-assignment2.js`.
7. **"location of optional chain"** (`expressions.rs`'s tagged-template
   optional-chain check, was `note_at(expr.range().start, None, ...)`, now
   `note_at(expr_range.start, Some(expr_range), ...)`) — C++ (cpp:3576)
   calls `sm_.note(expr->getSourceRange(), ...)`, passing the whole RANGE,
   not a bare point — the reverse mistake from 1-5 above (a range collapsed
   to a point, not a point expanded to a range). Doesn't flip
   `test/Parser/optional-chaining-error.js` out of mismatch (see below), but
   is independently a real, now-fixed defect confirmed via an isolated
   two-statement repro (`a?.b.c\n\`abc\`;` alone) matching hermesc exactly
   after the fix.

Item 1 fixed 58 files alone (71 → 13). Items 2-5 fixed 7 more files together
(3 + 1 + 2 + 1); item 6 is the 8th file. Item 7 fixed 0 files directly (see
the residual table below). 13 − 8 = **5**. 1337 + 66 = 1403 (identical);
71 − 66 = 5 (mismatch). Panic bucket unchanged (byte-for-byte the same 8
files) at every intermediate step.

### The final 5: individually classified, none is errorExpected-geometry

| File | hermesc first line | Classification |
|---|---|---|
| `test/AST/regexp.js` | `Invalid regular expression: ...` | **Pre-existing, out of component scope.** Needs the regex engine (`lib/Regex/`), not yet ported; already documented (this file's own "S2 Task 3 additions" section above, "REGEX-ENGINE DEFERRED"). |
| `test/Parser/es6/for-of-error.js` | `unexpected token after yield expression` (ours: `';' or 'in' expected inside 'for'`) | **Pre-existing parser-logic gap, not geometry.** `for (yield x in y;;) {}` reaches C++'s dedicated `yield`-as-assignment-expression branch (JSParserImpl.cpp:6257-6266, gated on `paramYield_`); the port's `for`-head disambiguation takes a different path entirely, so the two sides diverge on MESSAGE, not just rendering. Already named in the roadmap's follow-up (a) ("three further files differ in genuine message text"). Needs its own investigation of `for`-loop head parsing, not a caret fix — out of this task's scope. |
| `test/Parser/es6/import-error.js` | (extra `note: first usage of name` on hermesc's side only) | **Pre-existing, deliberate.** `parser/src/js/modules.rs:519-520,548` documents dropping C++'s `sm_.note(insertRes.first->second->getSourceRange(), "first usage of name")` (cpp:6923-6924) "per house style" — the same established, intentional convention documented at `jsx.rs:172,303` for other unconditional `sm_.note` companions to a plain `error()` call. Not part of the `errorExpected`/`need`/`eat` geometry this plan restores (T2's own report lists this exact file's call site as one of "5 remaining comments... verified accurate"). |
| `test/Parser/es6/yield-paren-error.js` | `unexpected token after yield expression` (ours: `';' expected`) | Same root cause and classification as `for-of-error.js` above — the `paramYield_`-gated yield-as-assignment-expression branch not being reached on this input either. |
| `test/Parser/optional-chaining-error.js` | (missing a second `error`+`note` pair entirely, `28:1`/`29:1`) | **Pre-existing, already documented in-code as a deliberate deviation.** `expressions.rs`'s tagged-template-in-optional-chain branch (fixed for geometry above, item 7) has a standing comment: C++ (cpp:3566-3577) emits the diagnostic and CONTINUES, building a `TaggedTemplateExpression` anyway (error recovery), so `parseStatementList` reaches the file's SECOND `a?.b.c` / `` `abc` `` statement and fires the same diagnostic there too; this port instead `return None`s, aborting the whole statement (and, transitively, the rest of the file — no further statements are attempted), which is why hermesc emits 4 diagnostics and the port emits 3. The comment ties this to "the broader error-recovery fidelity work" — a real, sizable, pre-existing gap (matching C++'s error-and-continue shape generally, not just this one call site), correctly out of scope for a geometry-only sweep. |

None of the 5 is an `errorExpected`-geometry miss (Task 2 did not skip a
site for any of them); each has an independent, individually-verified root
cause, none newly introduced by this task, and every fix above was
regression-checked against the full 1416-file sweep before moving to the
next.

**CORRECTION (see the "Fix report (post-review)" appendix below):** the
`for-of-error.js`/`yield-paren-error.js` rows above are WRONG — a
whole-branch review found the actual cause was four missing lines
(cpp:6263-6266), not an unrelated pre-existing `for`-head parsing gap. Both
are fixed and no longer residual; the table above is kept as-written for
the historical record of what this task's first pass concluded, corrected
in place by Finding 2 of the fix report.

### New corpus imports (Step 2)

Three upstream files, chosen to represent the three fix waves above,
byte-verified against `hermesc -dump-sema` (no flags) before import — all
copied verbatim, all `hermesc` exit 2 (error-path pins):

| File | Source | Note |
|---|---|---|
| `decorator-error.js` | `test/Parser/decorator-error.js`, verbatim | T1/T2 delta — a real `need`/`errorExpected` call site (`"in decorator"`, JSParserImpl.cpp:4727) exercising the same-line `combineIntoRange` arm Task 1 restored, on a genuine upstream file rather than a synthetic one-liner |
| `method-type-error.js` | `test/Parser/method-type-error.js`, verbatim | T1/T2 delta — a different `need`/`errorExpected` call site (`"in method definition"`, cpp:3197/3219/5542/5573), same arm, different production |
| `if-function-gen-error.js` | `test/Parser/if-function-gen-error.js`, verbatim | T3 delta — pins fix #4 above (`error(SMLoc, Twine)` point-vs-range at the if-statement generator/async check), independent of `errorExpected` entirely |

#### Gate

`sema differential (tests/sema_corpus): 205 corpus files matched (107
succeeded on the oracle)` — 202 → **205** (+3, all three imports above),
hermesc-succeeded **107 unchanged** (all three are error-path files, exit
2). Arithmetic: 202 + 3 = 205; 107 + 0 = 107.
`sema differential (tests/sema_corpus_parser): 11 corpus files matched (3
succeeded on the oracle)` — unchanged; nothing imported there.
Parser differential (`tests/parser_corpus`): 77 corpus files matched —
unchanged; nothing imported there (the error-path corpus home is the sema
corpus per the global constraints).

### Sweep count after this pass

**1403 / 5 / 8** (was 1220 / 188 / 8 going into this task). 1403 + 5 + 8 =
1416. Full workspace suite green throughout (`cargo test --manifest-path
rust/Cargo.toml --workspace`); zero new clippy lints (before/after warning
counts identical, verified via `git stash`/`git stash pop` around a clippy
run on the two touched files). **Superseded by the fix report below** — a
whole-branch review found the point-vs-range class was not actually closed
(4 reviewer-probed sites plus a dozen same-shape candidates the sweep never
exercised), plus a genuinely missing check and a silently-dropped parameter.

## Fix report (post-review)

The whole-branch review of this task ran the SAME kind of mechanical
call-site audit Task 2 used for `errorExpected` — but for plain `error(...)`
calls, which the sweep above can only catch when an upstream `.js` file
happens to exercise them. It found four Important issues.

### FINDING 1 — the point-vs-range class was not closed

Reproduced Task 2's method: extracted the first argument of every plain
`error(...)` call (i.e. NOT `errorExpected`/`need`/`eat`, which Task 2
already audited) across all four `JSParserImpl*.cpp` files, classified each
as C++'s POINT overload (`error(SMLoc, Twine)`) or RANGE overload
(`error(SMRange, Twine)` / `error(Twine)` current-token-range /
`combineIntoRange(...)`), then found and inspected the Rust counterpart of
every one of the **153** real call sites (2 more are `sm_.error(...)` calls
internal to `errorExpected`'s own body, not audited sites). Automated
matching by message text plus manual resolution of every ambiguous/no-match
case (bare-name argument variables checked against their declarations, e.g.
`awaitRng`/`identRng`/`startRange` are all `SMRange`; `start`/`startLoc`/
`errorLoc`/`clauseStartLoc`/`dotdotdotLoc` are all `SMLoc`).

**16 more divergent sites found and fixed** (all `error_cur`, i.e. the
current token's RANGE, where C++'s classification is POINT):

| C++ site | Rust site | Message |
|---|---|---|
| JSParserImpl.cpp:1579 | statements.rs (`parse_binding_rest_property`) | "identifier expected after '...' in object pattern" |
| JSParserImpl.cpp:2348 | statements.rs (`parse_throw_statement`) | "'throw' argument must be on the same line" (also restored the companion `sm_.note(startLoc, "location of the 'throw'")`, previously dropped entirely — cpp:2349) |
| JSParserImpl.cpp:6878 | modules.rs (`parse_name_space_import`) | "'as' expected" (the site's own comment actively asserted `error_cur` was correct here — it wasn't; comment corrected) |
| JSParserImpl-flow.cpp:101, 2799 | flow/declarations.rs (`parse_declare_flow`, `parse_declare_export_flow`) | "'type' required in opaque type declaration" (2 call sites, same message) |
| JSParserImpl-flow.cpp:123, 2606, 2695 | flow/declarations.rs (3 call sites) | "\`async\` is not supported for declared hooks..." |
| JSParserImpl-flow.cpp:133, 2622, 2723 | flow/declarations.rs (3 call sites) | "\`async\` is not supported for declared components..." |
| JSParserImpl-flow.cpp:2841 | flow/declarations.rs (`parse_declare_export_flow`) | "expected 'from' clause in export declaration" |
| JSParserImpl-flow.cpp:1345 | flow/match_.rs (`parse_match_unary_pattern_flow`) | "invalid match unary pattern argument" |
| JSParserImpl-flow.cpp:1386 | flow/match_.rs (`parse_match_pattern_flow`) | "invalid match pattern" |
| JSParserImpl-flow.cpp:3599, JSParserImpl-ts.cpp:1055 | flow/types.rs, ts/types.rs | "unexpected token in type annotation" (2 sites, same message, one per dialect) |

Verified none of these files' fixes were false positives: every automated
"FLAG" was cross-checked by reading the actual C++ source at the cited line
(not just trusting the message-text match), and spot-checked a sample
against a live `hermesc`/`sema-dump` comparison with the correct dialect
flags (`--parse-flow --parse-component-syntax --parse-flow-match`) after
discovering `sema-dump --help`'s real flag spelling (double-dash, not the
`hermesc -X...` spelling used in ad hoc probes). All spot checks matched
byte-for-byte on the error path (successful-parse dump-format mismatches
in the probes were an artifact of comparing `-dump-ast` against `-dump-sema`
on VALID input, not a real divergence — the two dump modes agree there is
no error, which is all that matters off the error path).

Also found and fixed, in the same audit, a genuinely different class of bug
that Finding 1's method surfaced as a side effect — not a rendering
mismatch but a **dropped parameter silently disabling a whole validation
branch**: `reparse_assignment_pattern`'s `in_decl: bool` parameter (port of
C++ `reparseAssignmentPattern`'s `inDecl`) was accepted but never threaded
into either `reparse_array_assignment_pattern` or
`reparse_object_assignment_pattern` (both hardcoded `false` at their own
recursive calls, and neither function even had an `in_decl` parameter to
receive it). C++ threads `inDecl` through both (cpp:5918, 5921,
6010/6032/6122), so the object-pattern rest-property check
("identifier expected in parameter list" — cpp:6079-6086, entirely absent
from the port) and the whole `in_decl=true` path through EVERY nested
array/object sub-pattern were unreachable. Confirmed reachable and wrong
before the fix: `({...a.b}) => 1` (an arrow parameter list) reported
"invalid destructuring target" instead of "identifier expected in parameter
list"; `([...a.b]) => 1` reported nothing wrong for the rest argument at
all where it should report "identifier or pattern expected". Both now
match hermesc exactly. Fixed by adding `in_decl: bool` to both functions'
signatures, threading it through every recursive call, and porting the
missing rest-property check.

None of these 16 (nor the `in_decl` fix) moved any upstream `.js` file out
of the sweep's mismatch bucket — none is exercised by the 1416-file corpus
at all, which is exactly why Task 2's mechanical site-by-site reconciliation
(not corpus mismatches) is the right verification method here, same as it
was for `errorExpected` itself.

### FINDING 2 — `for-of-error.js`/`yield-paren-error.js` were misclassified, not pre-existing

The residual table's classification of these two files as "a pre-existing
parser-logic gap... needs its own investigation of `for`-loop head parsing"
was wrong — the missing piece was four lines. C++ `parseAssignmentExpression`
(cpp:6257-6268) has a `yieldExpr->_argument && !checkEndAssignmentExpression()`
guard AFTER a successful `parseYieldExpression` call: if the yield had an
argument and the current token can't end an assignment expression, it's an
error ("unexpected token after yield expression"), not a successful yield.
The port's `paramYield_`-gated yield branch (expressions.rs, `parse_
assignment_expression`'s `run_level` closure) had the yield-detection and
the `parseYieldExpression` call (cpp:6256-6262) but silently dropped
cpp:6263-6266 — every successful yield parse became `Terminal` unconditionally,
so a malformed continuation like `yield x in y;;` (inside a `for` head) or
`yield()e=` fell through to whatever OTHER parse path came next, producing
a different, wrong diagnostic. Ported the four lines faithfully (`check_
end_assignment_expression(OfEndsAssignment::Yes)`, already used at the
existing post-assignment-expression check for the same default), gated on
the returned `YieldExpression` node's `argument.is_some()`. Both files now
match hermesc exactly (verified byte-for-byte) and the full sweep confirms
no other file regressed. Imported neither file into the sema corpus (see
Step 2 below for what WAS imported) since both are already covered
end-to-end by the sweep itself; the fix is small enough that the two-line
"before"/"after" diff plus the sweep's zero-regression confirmation is
sufficient evidence.

### FINDING 3 — six of the seven original fixes had no CI-visible pin

Beyond the sweep (a manual process, not a committed gate), only
`if-function-gen-error.js` (imported in Step 2) covered any of the seven
fixes from the original pass. Closed the gap two ways:

**Three more upstream files imported** (hermesc-verified byte-identical on
all three channels, all exit 2, all copied verbatim) — all three happen to
pin `eat_semi` (item 1) from three distinct grammatical contexts, which is
exactly why they're useful as regression pins even though they share one
root cause:

| File | Source | Pins |
|---|---|---|
| `declare-error.js` | `test/Parser/declare-error.js` | `eat_semi` after a bare `declare` identifier (no `-parse-flow`, so `declare` is just an identifier) |
| `await-get-error.js` | `test/Parser/await-get-error.js` | `eat_semi` after `await` used as an identifier inside a non-async getter |
| `using-declaration-pattern-error.js` | `test/Parser/using-declaration-pattern-error.js` | `eat_semi` after a `using` declaration's destructuring pattern |

Gate: `sema differential (tests/sema_corpus): 208 corpus files matched (107
succeeded on the oracle)` — 205 → **208** (+3, all imports above),
hermesc-succeeded **107 unchanged** (all exit 2). Arithmetic: 205 + 3 = 208;
107 + 0 = 107. `sema_corpus_parser` and the parser differential both
unchanged (nothing imported there).

**Five new oracle-free unit tests** added to
`rust/crates/parser/tests/error_expected_range.rs` (hermesc-verified
byte-for-byte before being written), covering the fixes an upstream import
can't reach or that the reviewer called out by name:

- `binding_element_no_identifier_is_a_bare_caret` — item 2 (`parse_binding_element`).
- `labeled_function_declaration_is_a_bare_caret` — item 3 (labeled `FunctionDeclaration`).
- `unexpected_token_after_assignment_expression_is_a_bare_caret` — item 5.
- `invalid_destructuring_target_combines_start_through_key_start_plus_one` — item 6 (the `combineIntoRange` off-by-one), named explicitly in the review.
- `optional_chain_tagged_template_note_underlines_the_whole_chain` — item 7 (the note-range fix), named explicitly in the review.

Every one of the original seven fixes now has a durable, CI-visible pin —
either a sema-corpus import or a unit test — not just the manual sweep.

### FINDING 4 — the roadmap's bug-for-bug sentence was self-contradictory

`doc/superpowers/RustPortRoadmap.md`'s follow-up (a) bullet claimed the
remaining deviations were "ONLY" two named classes, then listed four more
in the very next sentence. Reworded to enumerate the actual remaining set
without the false "only" — see the roadmap for the corrected text, which
now also reflects this fix report's updated numbers (1405/3/8, not
1403/5/8) and the two files this report un-misclassified.

### Sweep count after the fix report

Full re-run, same method: **1405 / 3 / 8** (was 1403 / 5 / 8 before this
fix report; 1220 / 188 / 8 at the start of the whole task). 1405 + 3 + 8 =
1416. The 2 files that moved (`test/Parser/es6/for-of-error.js`,
`test/Parser/es6/yield-paren-error.js`) are exactly Finding 2's fix, both
independently confirmed; zero other files moved (the Finding 1 audit's 16
sites + the `in_decl` fix are correctness/geometry improvements with no
sweep-visible file, as expected — see Finding 1 above). Panic bucket
unchanged (byte-for-byte the same 8 files) throughout.

**The final 3 residual files** (down from 5; `for-of-error.js` and
`yield-paren-error.js` removed by Finding 2, `import-error.js` re-verified
unchanged): `test/AST/regexp.js` (regex-engine deferral), `test/Parser/
es6/import-error.js` (the deliberate "notes dropped per house style"
convention), and `test/Parser/optional-chaining-error.js` (the collect-scope
leak's sibling error-recovery gap — the same `return None`-vs-C++-continues
deviation, see the roadmap's tracked follow-up). All three individually
classified, none is `errorExpected`-geometry, all pre-existing and
independently confirmed non-regressions.

### Gates after the fix report (all green)

```
cargo build --manifest-path rust/Cargo.toml --workspace --all-targets
  → 0 warnings
cargo clippy --manifest-path rust/Cargo.toml -p parser --all-targets
  → warning count identical before/after (git stash/pop verified)
cargo test --manifest-path rust/Cargo.toml --workspace
  → all suites green, 0 failed (including 5 new tests in
    error_expected_range.rs, now 9/9)
REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p sema \
    --features dump-bin --test sema_differential -- --nocapture
  → "208 corpus files matched (107 succeeded on the oracle)" (driver)
  → "11 corpus files matched (3 succeeded on the oracle)" (parser-entry)
REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p parser \
    --test parser_differential
  → 8/8 green, 77 plain + all dialect corpora unchanged
```

### Files touched by the fix report

`rust/crates/parser/src/js/{statements,expressions,modules}.rs`,
`rust/crates/parser/src/js/flow/{declarations,match_,types}.rs`,
`rust/crates/parser/src/js/ts/types.rs`, `rust/crates/parser/tests/
error_expected_range.rs` (5 new tests), `rust/crates/sema/tests/
sema_corpus/{declare-error,await-get-error,using-declaration-pattern-error}.js`
(new, verbatim upstream imports), this MANIFEST, `doc/superpowers/
RustPortRoadmap.md`, `doc/superpowers/SESSION-HANDOFF.md`.

---

## C++ defect-fix propagation, Task 2 (parser mirrors) — 2 new imports

The three parser-side upstream fixes cherry-picked in Task 1 each shipped a
C++ test. Two of them are parser *error* files, and the error-path corpus
home is this one (`parser_corpus` holds only clean-parse files — its
harness compares `ast-dump` stdout, and `ast-dump`'s error contract is the
single line `ERROR <n>`, so it cannot pin a diagnostic's text or geometry;
see `parser_differential.rs`'s module doc, which already routes the
recursion-limit error side here for the same reason). The third fix
(`b21856de4`, the JSON recursion limit) has no JS-source form at all and is
pinned by unit tests only.

| File | Source | Fix pinned |
|---|---|---|
| `jsx-error-attr-member.js` | `test/Parser/jsx-error-attr-member.js` (body verbatim; lit `RUN:`/`CHECK:` lines replaced by the header comment + `// FLAGS:` line) | `37520ccef` — `parseJSXElementName` checked `MemberExpressionNode`, never true for a JSX name, so `<foo a.b="1"/>` was accepted; now `unexpected member expression` |
| `flow-match-pattern-binding-error.js` | `test/Parser/flow/match/pattern-binding-error.js`, same treatment | `550aafe33` — after `'identifier' expected in match binding pattern` the parser fell through into `parseMatchBindingIdentifierFlow` and asserted (defect 11); now it returns `None` and recovers |

Both were byte-verified BEFORE import, all three channels, exactly as the
harness compares them (`hermesc -dump-sema <FLAGS> f` vs `sema-dump <FLAGS>
f`): stdout empty on both sides (0 bytes), stderr identical (188 and 257
bytes respectively — the rendered error plus `Emitted 1 errors. exiting.`),
exit 2 on both. Their unit-level twins are the first two tests in
`parser/tests/upstream_defect_fixes.rs`.

### `// FLAGS:` harness note — the hermesc `-Xparse-*` spelling

`flow-match-pattern-binding-error.js` needs `-Xparse-flow-match`, and the
FLAGS line goes VERBATIM to both binaries, which did not share a spelling
for it: hermesc names the flag `Xparse-flow-match`, `sema-dump` named it
`parse-flow-match`. Resolved by giving `sema-dump` a hidden alias under
hermesc's long name (`sema_dump.rs`, `xparse_flow_match`), and writing the
flag `--Xparse-flow-match` with TWO dashes — LLVM `cl` accepts either dash
count, while `command_line`'s single-dash path would read `-X` as a short
option with an attached value. The other two hidden `-Xparse-*` flags can
gain the same alias when a corpus file needs them.

### Gate

Corpus size **212 → 214** (+2, both imports above); oracle successes **108
unchanged** (both are error-path files, exit 2). Arithmetic: 212 + 2 = 214;
108 + 0 = 108.

`sema_differential_s0` itself is RED at the end of Task 2, for reasons that
are none of Task 2's: exactly 2 files mismatch, `export.js` and
`module-export-plain.js`, both on upstream `f90a83146` (in-tree `4193b558a`,
"Use consistent wording for export-requires-module-mode errors"), which Task 3 mirrors (plan item
T3S4). Verified by replicating the harness comparison over all 214 files
without the first-mismatch abort: 214 compared, 2 mismatches (those two),
108 oracle successes — i.e. both new imports pass, and nothing else
regressed. `sema_parser_differential` (11 files) and the whole
`parser_differential` suite (8/8) are green.

---

## C++ defect-fix propagation, Task 3 (resolver mirrors) — 2 new imports

Four upstream sema-side fixes cherry-picked in Task 1 were mirrored in the
port: `4ad67c992` (promoter `using` crash), `9232443cf` (promoter dead
code), `6b59daf0d` (anonymous `export default async function` losing its
`async` flag) and `f90a83146` (the ExportAll error wording). Two of them
shipped a C++ test; both are `test/Sema` files and both landed here.

| File | Source | Fix pinned |
|---|---|---|
| `using-scoped-fn-promotion.js` | `test/Sema/using-scoped-fn-promotion.js`, **verbatim** (its lit `RUN:`/`CHECK:` lines are inert JS comments; no `// FLAGS:` line needed — its own RUN line is plain `-dump-sema`) | `4ad67c992` — `ScopedFunctionPromoter::extractDeclaredIdents` asserted a `VariableDeclaration`'s kind is `let`/`const`/`var`, so a `using` declaration sharing a scope with a promotable block-nested function aborted the promoter before the resolver could reject `using`. Now `var` → `Decl::Kind::Var` and EVERY other kind → `Decl::Kind::Const` |
| `export-default-anon-async.js` | `test/Sema/export-default-anon-async.js`, **verbatim** | `6b59daf0d` — rewrite #4 passed a literal `/* async */ false` instead of `funcDecl->_async`. Its own RUN line is `-commonjs -dump-transformed-ast`, which this port does not implement (S4b), so it is imported at the flagless shape: hermesc reports the module-mode error and skips the dump, exit 2, and what it pins here is that the rewritten subtree resolves without incident |

Both were byte-verified BEFORE import, all three channels, exactly as the
harness compares them (`hermesc -dump-sema f` vs `sema-dump f`): stdout
empty on both sides (0 bytes), stderr identical (341 and 219 bytes), exit 2
on both. `-commonjs` was probed too and rejected as an import shape:
`sema-dump` has no such flag (`Unknown command line argument -c`), which is
the documented S4b gap, not a new one.

**Non-degeneracy of `using-scoped-fn-promotion.js`** (the pin really can
fail): with the pre-fix `debug_assert!(kind == ident_var)` temporarily
restored in `promoter.rs`, `sema-dump` panics on this exact corpus file
(`assertion failed: kind == self.sem_ctx.kw.ident_var`, exit 101) against
the oracle's exit 2 — measured, then reverted.

### The landmine section above is closed

"A new landmine found while porting the promoter" (the `using x = 1;
{ function f() {} }` abort, exit 134 in C++ / 101 here) recorded that such a
file "cannot go in the corpus". `4ad67c992` fixed it upstream and this task
mirrored it, so that shape IS in the corpus now — it is
`using-scoped-fn-promotion.js`. The section is kept for the history; read it
together with this one.

### The two `modules.rs` quirks in the S4a T3 section are closed

That section records two bug-for-bug quirks: the ExportAll `CommonJS module
mode` wording and rewrite #4's hard-coded `/* async */ false`. Both were
fixed upstream (`f90a83146`, `6b59daf0d`) and mirrored here, so neither is a
quirk any more:

- All three export visits now emit `'export' statement requires module
  mode`. `module-export-plain.js` and upstream `export.js` were the two RED
  files at the end of Task 2 and both re-match with **no corpus-file content
  change** — the oracle moved, the port followed. (`module-export-plain.js`'s
  header comment was rewritten to describe the unified wording, preserving
  its line count so the pinned diagnostic geometry on lines 12-14 is
  untouched; re-verified byte-identical afterwards.)
- Rewrite #4 now forwards `funcDecl->_async`. Its unit pin
  `export_default_anonymous_function_is_rewritten_to_an_expression`
  (`tests/resolver.rs`) was flipped from `async == false` to `async == true`,
  and a second test,
  `export_default_anonymous_non_async_function_stays_non_async`, pins the
  other side so the fix cannot be "re-fixed" into the opposite literal.

### Deferred rows re-probed (3 of 3)

| File | Re-probe result |
|---|---|
| `deep-ast-err.js` | unchanged — still a vacuous match (comment-only file, both exit 0, byte-identical); still excluded on purpose |
| `xmod-errors.js` | unchanged — still panics at `calls.rs:312` (`$SHBuiltin.moduleFactory needs visitModuleFactory`), oracle exit 2 vs 101. **Still S4b** |
| `invalid-args-eval.js` | **now byte-identical on all three channels** (stdout 0/0, stderr 2467/2467, exit 2/2), and deterministically so. The `89:9` same-location tie is no longer unspecified on the C++ side: upstream `5f313a13a` (in-tree `7805e2103`) replaced the buffered-message `std::sort` with `std::stable_sort` (`SourceErrorManager.cpp:60-73`), so both sides now keep same-location messages in emission order — the match is by construction, not coincidence. NOT imported here anyway: the plan assigns `5f313a13a` to **Task 5**, which will import this file together with retiring the (now-obsolete) stable-sort divergence note in `support/src/manager.rs`. The Deferred row was rewritten to say this instead of the old unstable-`std::sort` reason. (Discharged: Task 5 imported it — the file is in the Imported table now.) |

None of the three is a promoter/`using`/export-default row, so no row was
unblocked BY these fixes.

### Gate

Corpus size **214 → 216** (+2, both imports above); oracle successes **108
unchanged** (both new files are error-path, exit 2). Arithmetic:
214 + 2 = 216; 108 + 0 = 108.

`REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p sema
--features dump-bin --test sema_differential -- --nocapture`:

```
sema differential (tests/sema_corpus): 216 corpus files matched (108 succeeded on the oracle)
sema differential (tests/sema_corpus_parser): 11 corpus files matched (3 succeeded on the oracle)
test result: ok. 3 passed; 0 failed
```

Task 2's interim redness is gone: `sema_differential_s0` is GREEN again.

---

## C++ defect-fix propagation, Task 4 (resolver mirrors) — 2 new imports

Two more upstream sema-side fixes cherry-picked in Task 1 were mirrored in
the port: `07efab88d` ("Fix crash on `$SHBuiltin.#privateName()`") and
`b351e1184` ("Fix scope parenting of class expressions in field
initializers"). Both shipped a `test/Sema` regression test and both landed
here.

| File | Source | Fix pinned |
|---|---|---|
| `shbuiltin-private-name.js` | `test/Sema/shbuiltin-private-name.js`, **verbatim** (its lit `RUN:`/`CHECK:` lines are inert JS comments; its own RUN line is plain `-dump-sema`, so no `// FLAGS:` line is needed) | `07efab88d` — cpp:1166-1167 `cast<IdentifierNode>`'d the member expression's property, but a non-computed member expression's property can be a `PrivateNameNode`. The `cast` is now a `dyn_cast` whose result ALSO gates the whole `$SHBuiltin` recognition, so `$SHBuiltin.#x()` leaves the identifier alone and it is reported once as `invalid use of $SHBuiltin` from `visit(IdentifierNode *)` |
| `class-field-class-expr.js` | `test/Sema/class-field-class-expr.js`, **verbatim** (its RUN line is `%shermes -dump-sema -fno-std-globals`, so its inert `CHECK:` lines are the no-std-globals dump; the harness runs it at the corpus default WITH std globals, which is why no `// FLAGS:` line was added — the same shape `field-init-bindings.js` has been imported at since S2 T4) | `b351e1184` — `visit(ClassPropertyNode *)` / `visit(ClassPrivatePropertyNode *)` now wrap the initializer VALUE visit in `SaveAndRestore<LexicalScope *>{curScope_, curFunctionInfo()->getFunctionBodyScope()}`, analogous to `visit(StaticBlockNode *)`. A class expression in a field initializer therefore parents its scope in the synthesized initializer function's body scope instead of the enclosing class's scope |

Both were byte-verified BEFORE import, all three channels, exactly as the
harness compares them (`hermesc -dump-sema f` vs `sema-dump f`):

- `shbuiltin-private-name.js`: stdout empty on both sides, stderr identical
  (the `invalid use of $SHBuiltin` error plus hermesc's driver epilogue),
  exit 2 on both.
- `class-field-class-expr.js`: stdout identical (the full 97-line dump,
  including `ClassExpression Scope %s.4` / `%s.7`), stderr empty on both,
  exit 0 on both. *(Figures as imported. The file was widened with two
  private fields by the upstream sync — see "Upstream sync task 2 step 0" at
  the end of this file; the dump is 113 lines now.)*

### Port-side changes

- `resolver/calls.rs` — `sh_builtin_property_name` returns `Option<Atom>`
  instead of panicking on a non-`Identifier` property, and the `$SHBuiltin`
  recognition is gated on `prop_ident.is_some()`, mirroring `07efab88d`'s
  restructured control flow (the property `dyn_cast` is hoisted above the
  object test, exactly as in the C++).
- `resolver/classes.rs` — both class-property visits save/restore
  `cur_scope` around the initializer value visit, restoring it BEFORE
  `exit_function` (C++'s `SaveAndRestore` is declared after the
  `FunctionContext`, so it is destroyed first).

### Non-degeneracy of both pins (measured, then reverted)

- With the pre-fix explicit panic restored in `calls.rs`, `sema-dump`
  panics on `shbuiltin-private-name.js` (`sema: $SHBuiltin.<PrivateName> —
  C++ 'cast<IdentifierNode>' ... asserts on this shape`, exit 101) against
  the oracle's exit 2, and `tests/resolver.rs`'s
  `shbuiltin_private_name_is_rejected_not_asserted` fails with that panic.
- With the `cur_scope` save/restore removed from `classes.rs`, `sema-dump`
  aborts on `class-field-class-expr.js` on `dump_context.rs`'s
  `debug_assert_eq!` (`left: 1, right: 2`, "not all scopes were visited",
  exit 101), the differential's `sema_differential_s0` reports a stdout
  mismatch for that file, and `tests/resolver.rs`'s
  `field_initializer_scopes_are_parented_in_the_initializer_function` fails
  with `left: Some(ScopeId(1)), right: Some(ScopeId(2))` — the class
  expression's scope hanging off the enclosing class scope instead of the
  initializer function's body scope.

The dumper's `processed == scopes.len()` invariant was NOT weakened; the
`debug_assert_eq!` at `dump_context.rs:241` is untouched and now holds
naturally.

### Dump-shape impact on the pre-existing corpus (checked file by file)

`b351e1184`'s mirror changes `parentScope` for scopes created inside a field
initializer, which is dump-visible. Every corpus file's `sema-dump` stdout
was captured with the mirror reverted and again with it applied, and the two
sets compared: **exactly one file's dump changed — the newly imported
`class-field-class-expr.js`.** All the pre-existing class-field files
(`field-init-bindings.js`, `class-properties.js`, `classes-derived.js`,
`error-class-field.js`, `arguments-field-error.js`, `await-field-error.js`,
`yield-field-error.js`, …) are byte-for-byte unchanged and still match the
(also-changed) oracle. That is expected: the fix only moves scopes CREATED by
the initializer expression, and a class expression is the only such creator
those files lack.

`class-properties.js` and `classes-derived.js` were "trimmed to avoid" this
shape by S2 T4 (see that section). They were deliberately left as they are:
the shape now has its own dedicated upstream regression file, and re-widening
two settled corpus files is not something this fix requires.

### The two landmine bullets above are closed

The S2 T4 "Not corpus-reachable" list carried two bullets that this task
retires (both are now marked SUPERSEDED in place, kept for the history):

- "A class expression inside a class field initializer … the shape stays out
  of the corpus" — it is `class-field-class-expr.js` now.
- "`$SHBuiltin.#x(...)` … the shape stays out of the corpus and `calls.rs`'s
  `sh_builtin_property_name` reproduces the failing `cast` as an explicit
  panic" — it is `shbuiltin-private-name.js` now, and there is no panic left
  to reproduce.

### The `computed-fn-name.js` sweep landmine is closed

The S2 T8 / S3 T3 / S4a T5 sweep sections all recorded
`test/hermes/computed-fn-name.js:71` (`[k("strClass")] = class {};`) as the
one file in the ~1400-file `test/` sweep where **hermesc itself** aborted on
`SemContext.cpp:478` and this port aborted on its own `debug_assert_eq!` —
"different abort mechanisms, so never byte-identical". Re-probed after this
task's mirror: **exit 0 on both sides, stdout 18461/18461 bytes
byte-identical, stderr empty on both.** The next full sweep should move it
out of the `panic` bucket into `identical` (and the "the sweep is only
meaningful with debug builds on both sides" note loses its only witness). It
was NOT imported into the corpus: `class-field-class-expr.js` is the minimal
upstream pin for the same defect.

### Deferred rows re-probed (3 of 3)

| File | Re-probe result |
|---|---|
| `deep-ast-err.js` | unchanged — still a vacuous match (comment-only file, both exit 0, both channels 0 bytes); still excluded on purpose |
| `xmod-errors.js` | unchanged — still panics at `calls.rs` (`$SHBuiltin.moduleFactory needs visitModuleFactory`), oracle exit 2 vs 101. **Still S4b.** Note this is the row most plausibly touched by `07efab88d`, and it is not: `moduleFactory` IS an identifier property, so the new `dyn_cast` gate passes and the branch is reached exactly as before |
| `invalid-args-eval.js` | unchanged from Task 3's re-probe — byte-identical on all three channels (stdout 0/0, stderr 2467/2467, exit 2/2) by construction since `5f313a13a`'s stable sort. Still assigned to **Task 5**, which imports it. (Discharged: Task 5 imported it — the file is in the Imported table now.) |

Neither fix unblocks any Deferred row.

### Gate

Corpus size **216 → 218** (+2, both imports above); oracle successes
**108 → 109** (+1: `class-field-class-expr.js` exits 0;
`shbuiltin-private-name.js` is an error-path file, exit 2). Arithmetic:
216 + 2 = 218; 108 + 1 = 109.

`REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p sema
--features dump-bin --test sema_differential -- --nocapture`:

```
sema differential (tests/sema_corpus): 218 corpus files matched (109 succeeded on the oracle)
sema differential (tests/sema_corpus_parser): 11 corpus files matched (3 succeeded on the oracle)
test result: ok. 3 passed; 0 failed
```

## Task 5 (defect-fix propagation): `918158cb0` + `5f313a13a`

Both upstream fixes this task mirrors land on the DUMPERS, not the resolver,
so neither changes what any existing corpus file resolves to.

- `918158cb0` ("Fix semDump crashes on ASTs resolved for a parser") is
  entirely a `resolveASTForParser` shape — see
  `sema_corpus_parser/MANIFEST.md`, which is where its two new files went.
  Nothing in this (driver, `compile = true`) corpus can reach either shape:
  `with` is a `compile_`-gated error and an anonymous `export default
  function` is rewritten to a `FunctionExpression` before the dump.
- `5f313a13a` ("Sort buffered diagnostics with a stable sort") retires the
  last reason `invalid-args-eval.js` was Deferred; see below.

### New import: `invalid-args-eval.js` (upstream `test/Sema/`, verbatim)

Oracle-verified raw bytes before importing, exactly as the global constraint
requires:

```
hermesc -dump-sema test/Sema/invalid-args-eval.js   -> exit 2, stdout 0 B, stderr 2467 B
sema-dump          test/Sema/invalid-args-eval.js   -> exit 2, stdout 0 B, stderr 2467 B
cmp: stdout identical, stderr identical
```

Deferred since S1 (three separate re-probes recorded it as matching but
"unspecified on the C++ side"), because the `89:9` same-location tie between
the strict-mode `cannot declare 'arguments'` error and the `was not declared
in function "global"` warning had no defined order in C++: `std::sort` over
the buffered-message array is not stable, so which of the two came out first
depended on the whole 24-message array. `5f313a13a` changed that to
`std::stable_sort`, which is what `disable_buffering`'s `sort_by_key` in
`support/src/manager.rs` has always been — so both sides now keep
same-location messages in emission order and the match is by construction,
not coincidence. `manager.rs`'s comment at the sort was rewritten to record
that the divergence is retired (and to cite `5f313a13a`), instead of leaving
a reader to rediscover the history from this MANIFEST.

Its Deferred row moved into the Imported table; the Task 3 and Task 4
re-probe rows that forward-referenced Task 5 are annotated as discharged.

### Deferred rows re-probed (2 of 2)

| File | Re-probe result |
|---|---|
| `deep-ast-err.js` | unchanged — still a vacuous match (comment-only file, both exit 0, both channels 0 bytes); still excluded on purpose |
| `xmod-errors.js` | unchanged — still panics in `calls.rs` (`$SHBuiltin.moduleFactory needs visitModuleFactory`), oracle exit 2 vs 101. **Still S4b.** Neither dumper fix touches the resolver, so this row could not move |

### Gate

Corpus size **218 → 219** (+1, `invalid-args-eval.js`); oracle successes
**109 unchanged** — hermesc exits **2** on it (probed above), so it lands in
the error-path bucket, not the success bucket. Arithmetic: 218 + 1 = 219;
109 + 0 = 109.

`REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p sema
--features dump-bin --test sema_differential -- --nocapture`:

```
sema differential (tests/sema_corpus): 219 corpus files matched (109 succeeded on the oracle)
sema differential (tests/sema_corpus_parser): 13 corpus files matched (5 succeeded on the oracle)
test result: ok. 3 passed; 0 failed
```

## Upstream sync task 2 step 0: widen `class-field-class-expr.js`

The task-1 review found a hole in this corpus file. `dee8c5ce0`'s fix has two
halves — `visit(ClassPropertyNode *)` and `visit(ClassPrivatePropertyNode *)`
— and the file as first imported here (55 lines, `x = class {}` and
`static y = class {}` only) pins the first half but not the second: deleting
the `cur_scope` save/restore from `visit_class_private_property` in
`resolver/classes.rs` and leaving `visit_class_property` alone left all 219
+ 13 differentials green.

Upstream's own copy of the file is 73 lines: `dee8c5ce0` added `#px = class
{}` and `static #py = class {}` in the same commit. That version was pulled
in verbatim (`git show dee8c5ce0:test/Sema/class-field-class-expr.js`) over
both the lit file and this corpus copy, so the MANIFEST's "verbatim copy of
the lit file" claim still holds.

Oracle-verified before importing, all three channels, exactly as the harness
compares them:

```
hermesc -dump-sema sema_corpus/class-field-class-expr.js -> exit 0, stdout 113 lines
sema-dump          sema_corpus/class-field-class-expr.js -> exit 0, stdout 113 lines
cmp: stdout identical, stderr empty on both, exit 0 on both
```

The lit file's `CHECK:` lines are upstream's, unmodified, and
`LIT_FILTER=class-field-class-expr check-hermes` passes against this tree's
`shermes` — upstream's expectations and ours agree on this input.

### Non-degeneracy re-measured (the point of the widening)

With the `cur_scope` save/restore deleted from `visit_class_private_property`
ONLY (`visit_class_property` untouched):

```
sema-dump sema_corpus/class-field-class-expr.js
  thread 'main' panicked at crates/sema/src/dump_context.rs:251:9:
  assertion `left == right` failed: not all scopes were visited
    left: 2
   right: 3
  exit 101
sema_differential_s0: stdout mismatch for class-field-class-expr.js  -> FAILED
```

and, on the pre-widening 55-line file, the same mutant exits **0** with a
clean dump. So the widening is exactly what closes the gap. Restored, the
gates are back to 219 (109) / 13 (5).

### Gate

Corpus size **219 unchanged** (the file was replaced, not added); oracle
successes **109 unchanged** (it exited 0 before and after).

## Upstream sync task 2: the dump gained `mayReachImplicitReturn`

Upstream `04f1f53a8` ("Add `-Xcompile` and dump `mayReachImplicitReturn`",
cherry-picked here as `1e3806f47`) appends one token to every
`Func`/`StaticBlock` line in `SemContextDumper::printFunction`:

```cpp
<< (f.mayReachImplicitReturn ? " mayReachImplicitReturn" : " noImplicitReturn")
```

`dump_context.rs::print_function` mirrors it (`de917f249`). Both spellings are
emitted, as upstream does: the flag defaults to `true`
(`SemContext.h:354`, `sem_context.rs:545`) and `CheckImplicitReturn` is
skipped after a resolution error, so an omitted-when-true encoding would
conflate "computed true" with "not computed".

**Every corpus file's expected dump changed, and none of them are stored** —
this harness compares against a live oracle, not a checked-in baseline, so
there was nothing to regenerate on this side. The ~100 checked-in `test/Sema`
lit baselines were regenerated with `update-lit`, which reproduced upstream's
own post-image for 129 of the 133 changed files (the 4 that differ do so only
in FlowChecker type annotations, from unported FlowChecker commits).

### The point: `may_reach_implicit_return` was byte-verified for the first time

The port has computed this flag since S3 but never dumped it, so nothing had
ever compared it to C++. After the mirror, **zero files mismatch**:

```
sema differential (tests/sema_corpus):        219 corpus files matched (109 succeeded)
sema differential (tests/sema_corpus_parser):  13 corpus files matched (5 succeeded)
```

**Not vacuous.** Over the driver corpus's dumps the oracle emits 359
` mayReachImplicitReturn` and 196 ` noImplicitReturn` lines — 35% of all 555
`Func`/`StaticBlock` lines take the false branch — and the port emits the
same counts in the same places. The parser corpus adds 14 / 2.

**Non-degeneracy proved by mutation** (both reverted afterwards), in
`check_implicit_return.rs`:

| Mutation | Caught by |
|---|---|
| `ReturnStatement => make_next_statement()` instead of `make_must_terminate()` | **55 of 219** driver files, **1 of 13** parser files |
| `ThrowStatement` likewise | `try-catch-finally.js` |

### Wider sweep, to make the null result credible

`hermesc -dump-sema` vs `sema-dump` over **all 1232 `.js` under `test/Sema`,
`test/Parser` and `test/hermes`**: **zero stdout differences.** 1227 identical
on all three channels; the 5 outliers are pre-existing and unrelated —

- 3 `$SHBuiltin.moduleFactory` panics (`test/Sema/xmod-errors.js`,
  `test/hermes/xmod-exec-require{,-bad-func}.js`), the Deferred row above,
  still S4b;
- 2 stderr-only parser deviations already documented as deliberate:
  `test/Parser/es6/import-error.js` (the `note: first usage of name`
  companions dropped per house style) and
  `test/Parser/optional-chaining-error.js` (error-and-continue recovery, 4
  diagnostics vs 3).

Note this sweep no longer has the `computed-fn-name.js` abort landmine (closed
by Task 4 above), so it is a clean measurement.

### Gate

Corpus size **219 unchanged**; oracle successes **109 unchanged**. Nothing was
imported or removed by this task; only the dump format moved.
