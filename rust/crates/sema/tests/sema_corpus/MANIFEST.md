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
| `const-reassignment.js` | `CallExpression` | S2 |
| `deep-ast-err.js` | vacuous — see note above (not a real S1 gap) | n/a |
| `diagnode_errors.js` | `CallExpression` | S2 |
| `disabled-eval.js` | `CallExpression` | S2 |
| `eval-warn.js` | `CallExpression` | S2 |
| `function-redeclaration-error.js` | loose- AND strict-mode block-nested `FunctionDeclaration` (`ScopedFunctionPromoter`) — re-probed after S2 T3 unblocked its `try`/`catch` clauses; the remaining blocker is the `sema S1: scoped function declarations are S3 scope` assert | S3 |
| `invalid-args-eval.js` | **not a port gap** — the resolver's loop/`for` support landed in S2 T1 and every diagnostic in this file is produced, with identical text and locations, but two of them collide at the *same* source location (`89:9`: the strict-mode `cannot declare 'arguments'` error and the `was not declared in function "global"` warning). C++'s buffered-message flush uses `std::sort` (`SourceErrorManager.cpp:61-71`), which is NOT stable, so their relative order is unspecified and in practice depends on the whole 24-message array; our `disable_buffering` uses a stable `sort_by_key` (`support/src/manager.rs:903-909`, a documented deviation). Minimized to two messages the two sides agree; only at this file's message count does libstdc++'s introsort reorder the tie. Not faithfully fixable (there is no defined tie order to match), and the file's actual subject is S1's `arguments`/`eval` declaration rules, so the loop-specific rows were extracted into the new `error-for-decl-strict.js` instead | n/a (C++ unstable-sort tie) |
| `let-arguments-in-arrow.js` | `CallExpression` (arrows landed in S2 T2; `print(...)` remains) | S2 |
| `private-load-store-error.js` | `CallExpression` (`sink(...)`) — the private names and the `MemberExpression`/`OptionalMemberExpression` restriction branches landed in S2 T5, and its whole subject is reproduced call-free by the new `error-private-load-store.js` | S2 T6 |
| `regress-function-promotion-decl.js` | loose-mode block-nested `FunctionDeclaration` (`ScopedFunctionPromoter`) | S3 |
| `regress-nested-expressions-error.js` | recursion-depth-limit mismatch: hermesc and sema-dump both correctly error `Too many nested expressions/statements/declarations` on the deeply-nested `get<<=get<<=...` chain, but at different columns (hermesc col 3052, sema-dump col 6124) — the two recursion trackers (`JSParserImpl::recursionDepth_`/`SemanticResolver`'s tracker vs our ported ones) increment at different rates per grammar production, so the exact trip point diverges even though both share the same `MAX_RECURSION_DEPTH = 1024`. Same landmine category as the S1 ledger's "parser recursion limit unported" item (S0-era finding, T6 review) — tracked together, not re-derived/fixed here | parser-gap follow-up (recursion-depth-counting parity) |
| `reject-with.js` | `CallExpression` (`print(a)`) — `with` itself landed in S2 T3 (see `error-with.js`) | S2 |
| `static-initialization-block-lazy-error.js` | `CallExpression` (`print(...)`) — `StaticBlock` landed in S2 T5; its subject (`forbidArgumentsAsIdentifier_` reached through three nested arrows) is covered call-free by the new `error-static-block.js`. Its `RUN:` line also wants `-lazy`, which the harness has no flag support for | S2 T6 |
| `static-initialization-block.js` | `CallExpression` (`sink(y)`) plus `IfStatement`, which no corpus file has needed yet (it has no `SemanticResolver::visit` override, so it belongs in `visit_node`'s override-free generic arm whenever a file first needs it) — `StaticBlock` itself landed in S2 T5 and its subject is covered by the new `static-blocks.js` | S2 T6 |
| `super-in-arrow.js` | `CallExpression` (`super()` and `print(...)`) — the class and `super.x` paths landed in S2 T4 | S2 T6 |
| `super-in-subclass-error.js` | `CallExpression` — its subject IS the `super() call only allowed in derived class constructor` check (cpp:1195-1202) | S2 T6 |
| `super-in-subclass.js` | `CallExpression` (`super()`) | S2 T6 |
| `type-alias-children.js` | typed dialect (`-parse-flow` RUN flag; harness has no per-file flags) — WITHOUT the flag, hermesc and sema-dump both hit the identical `';' expected` parse error on `type A = B;`, but that's a coincidental match on a syntax error, not a test of the file's actual subject (TypeAlias children resolution); same vacuous-match category `deep-ast-err.js` was excluded for, so it does not belong in `sema_corpus/` either | dialect-corpus phase |
| `undeclared-private-name-error.js` | `CallExpression` (`print(...)`) — private names landed in S2 T5 and the same diagnostic is pinned call-free by the new `error-private-load-store.js` | S2 T6 |
| `valid-super-references.js` | `CallExpression` (`super()` and `print(...)`) — `super.x` itself landed in S2 T4 | S2 T6 |
| `var-scope-redeclaration-error.js` | `CallExpression` (`something()`) — `try`/`catch` landed in S2 T3 | S2 |
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

Final count after S2 Task 3: **107 corpus files matched** (63 succeed on
hermesc, 44 are hermesc-failure files).

Final count after S2 Task 4: **120 corpus files matched** (69 succeed on
hermesc, 51 are hermesc-failure files).

Final count after S2 Task 5: **134 corpus files matched** (72 succeed on
hermesc, 62 are hermesc-failure files).

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

Five new files, the five re-probed `test/Sema` imports (`private-names.js`,
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
