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

Total top-level files: 54. Imported: 15 (14 from `test/Sema` + 1 new
gap-filler, `expr-visit-generic.js`, added in Step 2 below). Deferred: 40
(14 + 40 = 54; counting `deep-ast-err.js`, which is listed but is a vacuous
non-gap — see its row's note below).

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
| `await-arrow-error.js` | `ArrowFunctionExpression` | S2 |
| `await-arrow.js` | `ArrowFunctionExpression` | S2 |
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
| `for-using-not-supported.js` | `ForOfStatement` | S2 |
| `function-redeclaration-error.js` | `TryStatement` | S2 |
| `invalid-args-eval.js` | `ForStatement` | S2 |
| `label-errors.js` | `BreakStatement` | S2 |
| `let-arguments-in-arrow.js` | `ArrowFunctionExpression` | S2 |
| `private-declaration-dup-error.js` | `ClassDeclaration` | S2 |
| `private-load-store-error.js` | `ClassDeclaration` | S2 |
| `private-name-in-extends-error.js` | `ClassDeclaration` | S2 |
| `private-names.js` | `ClassDeclaration` | S2 |
| `regress-ast-const-folding.js` | `ForOfStatement` | S2 |
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

Final count: **69 corpus files matched** (54 pre-existing + 14 imported from
`test/Sema` + 1 new gap-filler; 42 succeed on hermesc, 27 are hermesc-failure
files compared byte-for-byte on the error path).
