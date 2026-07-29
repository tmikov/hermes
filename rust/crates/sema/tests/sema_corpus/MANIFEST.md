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
| `invalid-args-eval.js` | **not a port gap** — the resolver's loop/`for` support landed in S2 T1 and every diagnostic in this file is produced, with identical text and locations, but two of them collide at the *same* source location (`89:9`: the strict-mode `cannot declare 'arguments'` error and the `was not declared in function "global"` warning). C++'s buffered-message flush uses `std::sort` (`SourceErrorManager.cpp:61-71`), which is NOT stable, so their relative order is unspecified and in practice depends on the whole 24-message array; our `disable_buffering` uses a stable `sort_by_key` (`support/src/manager.rs:903-909`, a documented deviation). Minimized to two messages the two sides agree; only at this file's message count does libstdc++'s introsort reorder the tie. Not faithfully fixable (there is no defined tie order to match), and the file's actual subject is S1's `arguments`/`eval` declaration rules, so the loop-specific rows were extracted into the new `error-for-decl-strict.js` instead | n/a (C++ unstable-sort tie) |
| `regress-nested-expressions-error.js` | recursion-depth-limit mismatch: hermesc and sema-dump both correctly error `Too many nested expressions/statements/declarations` on the deeply-nested `get<<=get<<=...` chain, but at different columns (hermesc col 3052, sema-dump col 6124) — the two recursion trackers (`JSParserImpl::recursionDepth_`/`SemanticResolver`'s tracker vs our ported ones) increment at different rates per grammar production, so the exact trip point diverges even though both share the same `MAX_RECURSION_DEPTH = 1024`. Same landmine category as the S1 ledger's "parser recursion limit unported" item (S0-era finding, T6 review) — tracked together, not re-derived/fixed here. **S2 T8's sweep sharpened it: on `test/hermes/far-environment-access.js` (250-odd nested arrows) hermesc reports the error at 28:510 while `sema-dump` STACK-OVERFLOWS and aborts (SIGABRT/134) before its own tracker trips.** So the gap is not only "a different column": a debug build's frames are big enough that 1024 allowed levels outrun the 8 MB stack, i.e. deep-but-otherwise-valid input crashes instead of diagnosing. Same row, higher severity | parser-gap follow-up (recursion-depth-counting parity + a real crash) |
| `type-alias-children.js` | typed dialect (`-parse-flow` RUN flag; harness has no per-file flags) — WITHOUT the flag, hermesc and sema-dump both hit the identical `';' expected` parse error on `type A = B;`, but that's a coincidental match on a syntax error, not a test of the file's actual subject (TypeAlias children resolution); same vacuous-match category `deep-ast-err.js` was excluded for, so it does not belong in `sema_corpus/` either | dialect-corpus phase |
| `xmod-errors.js` | the `$SHBuiltin` CommonJS-module protocol: `visitModuleFactory`/`visitModuleExport`/`visitModuleImport` (cpp:1320-1453), reached from the three property-name branches of rewrite #3 (cpp:1168-1189). `CallExpression` itself landed in S2 T6, which ports those three branches as loud phase-tagged panics — its row was re-classified from "`CallExpression` / S2" accordingly. Every diagnostic in the file (`$SHBuiltin.moduleFactory requires exactly two arguments.` and 17 more) comes from those three functions | S4 modules |

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
    -p sema --features dump-bin --test sema_differential -- --nocapture
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

- **A class expression inside a class field initializer** (`class C { x =
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
- **`$SHBuiltin.#x(...)`** inside a class declaring `#x` makes the C++ assert:
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
| `promotion-var-shadows-promoted.js` | **new** — `visit(VariableDeclarationNode *)`'s `prevIsLexicalBindingOfPromotedFunc` special case (cpp:365-374, feeding the error at cpp:391-401), at both top level and function scope. Derived from the C++, not the brief's `let`-based sketch (a `let` there doesn't even reach this code path — only a `var` does, since the check is gated on `kw_.identVar`) |
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
