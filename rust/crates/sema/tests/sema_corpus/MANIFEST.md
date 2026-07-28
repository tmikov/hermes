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

Total top-level files: 54. Imported **as of the S1 Task 8 sweep**: 18 (17 from
`test/Sema` + 1 new gap-filler, `expr-visit-generic.js`, added in Step 2
below); deferred: 37 (17 + 37 = 54; counting `deep-ast-err.js`, which is listed
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
| `arguments-arg-let.js` | `CallExpression` (`print(...)`) | S2 |
| `break-in-nested-func.js` | loose-mode block-nested `FunctionDeclaration` (`ScopedFunctionPromoter`) | S3 |
| `catch-block-destr.js` | `TryStatement` | S2 |
| `catch-block-error.js` | `TryStatement` | S2 |
| `catch-block.js` | `TryStatement` | S2 |
| `class-children.js` | `ClassDeclaration` | S2 |
| `const-reassignment.js` | `CallExpression` | S2 |
| `deep-ast-err.js` | vacuous — see note above (not a real S1 gap) | n/a |
| `diagnode_errors.js` | `CallExpression` | S2 |
| `disabled-eval.js` | `CallExpression` | S2 |
| `eval-warn.js` | `CallExpression` | S2 |
| `field-init-bindings.js` | `ClassDeclaration` | S2 |
| `field-value-arguments-error.js` | `ClassDeclaration` | S2 |
| `function-redeclaration-error.js` | `TryStatement` | S2 |
| `invalid-args-eval.js` | **not a port gap** — the resolver's loop/`for` support landed in S2 T1 and every diagnostic in this file is produced, with identical text and locations, but two of them collide at the *same* source location (`89:9`: the strict-mode `cannot declare 'arguments'` error and the `was not declared in function "global"` warning). C++'s buffered-message flush uses `std::sort` (`SourceErrorManager.cpp:61-71`), which is NOT stable, so their relative order is unspecified and in practice depends on the whole 24-message array; our `disable_buffering` uses a stable `sort_by_key` (`support/src/manager.rs:903-909`, a documented deviation). Minimized to two messages the two sides agree; only at this file's message count does libstdc++'s introsort reorder the tie. Not faithfully fixable (there is no defined tie order to match), and the file's actual subject is S1's `arguments`/`eval` declaration rules, so the loop-specific rows were extracted into the new `error-for-decl-strict.js` instead | n/a (C++ unstable-sort tie) |
| `let-arguments-in-arrow.js` | `CallExpression` (arrows landed in S2 T2; `print(...)` remains) | S2 |
| `private-declaration-dup-error.js` | `ClassDeclaration` | S2 |
| `private-load-store-error.js` | `ClassDeclaration` | S2 |
| `private-name-in-extends-error.js` | `ClassDeclaration` | S2 |
| `private-names.js` | `ClassDeclaration` | S2 |
| `regress-function-promotion-decl.js` | loose-mode block-nested `FunctionDeclaration` (`ScopedFunctionPromoter`) | S3 |
| `regress-nested-expressions-error.js` | recursion-depth-limit mismatch: hermesc and sema-dump both correctly error `Too many nested expressions/statements/declarations` on the deeply-nested `get<<=get<<=...` chain, but at different columns (hermesc col 3052, sema-dump col 6124) — the two recursion trackers (`JSParserImpl::recursionDepth_`/`SemanticResolver`'s tracker vs our ported ones) increment at different rates per grammar production, so the exact trip point diverges even though both share the same `MAX_RECURSION_DEPTH = 1024`. Same landmine category as the S1 ledger's "parser recursion limit unported" item (S0-era finding, T6 review) — tracked together, not re-derived/fixed here | parser-gap follow-up (recursion-depth-counting parity) |
| `reject-super-references.js` | `Super` | S2 |
| `reject-with.js` | `WithStatement` | S2 |
| `static-initialization-block-error.js` | `ClassDeclaration` | S2 |
| `static-initialization-block-lazy-error.js` | `ClassDeclaration` | S2 |
| `static-initialization-block.js` | `ClassDeclaration` | S2 |
| `super-in-arrow.js` | `ClassDeclaration` | S2 |
| `super-in-subclass-error.js` | `ClassDeclaration` | S2 |
| `super-in-subclass.js` | `ClassDeclaration` | S2 |
| `type-alias-children.js` | typed dialect (`-parse-flow` RUN flag; harness has no per-file flags) — WITHOUT the flag, hermesc and sema-dump both hit the identical `';' expected` parse error on `type A = B;`, but that's a coincidental match on a syntax error, not a test of the file's actual subject (TypeAlias children resolution); same vacuous-match category `deep-ast-err.js` was excluded for, so it does not belong in `sema_corpus/` either | dialect-corpus phase |
| `undeclared-private-name-error.js` | `CallExpression` | S2 |
| `valid-super-references.js` | `Super` | S2 |
| `var-scope-redeclaration-error.js` | `TryStatement` | S2 |
| `xmod-errors.js` | `CallExpression` | S2 |

## Subdirectories (`test/Sema/flow/`, `test/Sema/flow/ffi/`, `test/Sema/lowering/`)

`test/Sema/flow/` (179 files across `flow/` and `flow/ffi/`) and
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
  parents but have their own visit override (the `eval`/`$SHBuiltin` specials),
  so they stay deferred to S2 T6.
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
  the class visits. `CoverTypedIdentifier` likewise needs `-parse-flow`
  (dialect-corpus phase).
