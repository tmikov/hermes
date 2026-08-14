# Sema S1 (Declarations & Scopes) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port the resolver's declaration/scope/identifier core — scope creation,
var/let/const/function hoisting, parameter scopes, identifier resolution, constant
folding, and the expression validators — expanding the live `hermesc -dump-sema`
differential to a broad corpus including error files.

**Architecture:** S1 lands the spec §3.4 mechanism: the resolver becomes an
`ast::VisitorMut` (functional rebuild; `resolve_ast` returns the possibly-new root),
because the C++ walk replaces nodes generically through `Node **ppNode`
(constant folding — `astFoldBinaryExpression`/`astFoldUnaryExpression` in
`lib/Sema/ASTEval.cpp`, which is therefore UNTYPED scope, correcting the spec's
§1 table). Everything else is method-for-method porting of the C++ ranges listed
per task. S2 keeps: loops/labels/break/continue/switch, try/catch, classes/private
names, arrow functions + the four §3.4 rewrites, call-expression specials
(eval/`$SHBuiltin`), `with`, regexp validation, meta/super/yield/await/spread,
`CheckImplicitReturn`. S3 keeps: `ScopedFunctionPromoter` (S1 loose-mode corpus
must avoid block-nested function declarations; the S0 assert stays as the guard).

**Tech Stack:** as S0. C++ source of truth: `lib/Sema/SemanticResolver.{h,cpp}`,
`lib/Sema/ASTEval.{h,cpp}`, `lib/CompilerDriver/CompilerDriver.cpp` (error
epilogue), `tools/hermesc/hermesc.cpp` (exit codes).

## Global Constraints

- NEVER `cd`; `--manifest-path rust/Cargo.toml` / absolute paths.
- Zero warnings with AND without `--features dump-bin`; no new clippy lints;
  `#![forbid(unsafe_code)]` in `sema`/`support`; 80-col for new lines; copyright
  headers; C++ comments carried over with corrected file:line citations
  (verify every citation with grep/awk before writing it — S0's re-reviews
  caught repeated off-by-one drift).
- Faithful port: C++ default args are spec; templates stay generics; RAII →
  the established explicit/Drop-guard patterns; bug-for-bug quirks preserved
  (do NOT "fix" the C++; flag surprises in reports).
- TDD per task; the differential is the principal gate:
  `REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p sema --features dump-bin --test sema_differential -- --nocapture`
  (oracle: `cmake-build-asan/bin/hermesc`; every corpus file verified against
  hermesc FIRST; fix the Rust side, never curate away a fixable mismatch).
- Full workspace suite green before each commit. Commit messages
  `rust(sema): <what>` + trailer `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.
- S1 boundary honesty: node kinds with C++ visit OVERRIDES that S1 does not port
  keep loud panics (`sema S1: unhandled node kind ...`); nodes WITHOUT C++
  overrides route to generic children traversal (Task 1) — the C++ inventory at
  the top of `SemanticResolver.cpp` (grep `SemanticResolver::visit`) is the
  authority on which is which.

---

### Task 1: Resolver becomes `ast::VisitorMut` (replacement-capable core)

**Files:**
- Modify: `rust/crates/sema/src/resolver/mod.rs` (the visit dispatch, `run`),
  `rust/crates/sema/src/resolve.rs` (`resolve_ast` returns the new root),
  `rust/crates/sema/src/bin/sema_dump.rs` (dump the RETURNED root)
- Test: extend `rust/crates/sema/tests/resolver.rs`

**Interfaces:**
- Consumes: `ast`'s phase-3 transform machinery — read its actual shape first
  (grep `trait VisitorMut`, `enum TransformResult`, `visit_children_mut` in
  `rust/crates/ast/src/`; `rust/crates/ast/tests/transform.rs` shows usage).
- Produces: `SemanticResolver` implements the mut-visitor protocol; internal
  `fn visit_node(&mut self, gc, node, parent: Option<&Node>) -> TransformResult`
  (or the trait's exact signature — parent comes from the trait's `Path` if that
  is its design); `resolve_ast(...) -> Option<&Node>`-style API returning the
  possibly-new root (adapt precisely; callers updated). Later tasks add match
  arms that RETURN replacements; unmatched-but-override-free kinds flow through
  generic `visit_children_mut` rebuild.
- Also: recursion-depth brackets + `MAX_NESTED_BINARY`/`MAX_NESTED_ASSIGNMENTS`.

- [ ] **Step 1: Read** the ast transform machinery (files above) and the C++
  dispatch protocol: `include/hermes/AST/RecursiveVisitor.h` — specifically how
  `visitESTreeNode(*this, child, parent)` lets an override receive
  `Node **ppNode` (the `BinaryExpressionNode` visit at `SemanticResolver.cpp:405`
  takes `Node **`), and how `incRecursionDepth`/`decRecursionDepth` bracket each
  dispatch. Find where `MAX_NESTED_BINARY`/`MAX_NESTED_ASSIGNMENTS` are defined
  (grep `lib/Sema/` + `include/hermes/`) and record their exact values.
- [ ] **Step 2: Failing tests**: (a) a resolver test where resolution of a
  hand-built tree returns a root (identity for the S0 corpus shapes — no
  replacements yet, `assert!(std::ptr::eq(...))` for the unchanged case);
  (b) recursion-depth: a program nested deeper than the resolver's limit
  (use the C++ default depth — grep `kDefaultRecursionDepth`/ctor init) errors
  with `Too many nested expressions/statements/declarations` via the byte-compatible
  SourceErrorManager (assert the message).
- [ ] **Step 3: Implement**: convert the S0 dispatch to the VisitorMut protocol
  (S0's visits return Unchanged; ScopeRAII/FunctionContext push/pop unchanged);
  move `linearize_left` where both dump.rs and the resolver reach it (one shared
  `pub(crate)` helper — no duplication) and add `linearize_right` (port from
  `include/hermes/AST/ESTree.h` next to `linearizeLeft`; verify by grep);
  add the recursion-depth brackets in the dispatch (port of RecursiveVisitor.h's
  inc/dec, S0 ledger carry-item); `resolve_ast` + bin use the returned root.
- [ ] **Step 4: Run** the S0 gate (must stay green — 6 files) + new tests + full
  workspace. **Step 5: Commit** `rust(sema): resolver as VisitorMut + recursion depth (S1 core)`.

---

### Task 2: hermesc error-epilogue parity (exit code + driver line)

**Files:**
- Modify: `rust/crates/sema/src/bin/sema_dump.rs`
- Modify: `rust/crates/sema/tests/sema_differential.rs` (drop any assumption that
  hermesc always succeeds: per-file, when hermesc FAILS, compare all three
  channels instead of asserting success — but require that at least one corpus
  file still succeeds, keeping the non-degeneracy guard)
- Test: first error corpus files under `rust/crates/sema/tests/sema_corpus/`

**Interfaces:**
- Consumes: S0 bin structure. Produces: the bin's error path is byte- and
  exit-code-identical to hermesc, so ERROR FILES BECOME ORDINARY CORPUS MEMBERS
  for every later task.

- [ ] **Step 1: Establish ground truth empirically** (scratchpad files, not repo):
  run `cmake-build-asan/bin/hermesc -dump-sema` on (a) a parse-error file
  (`var 1x;`), (b) a sema-error file (`"use strict"; delete x;` — wait, that's
  S2; use `let let;` which is S1 territory but errors in the PARSER? verify —
  if needed use `let x; let x;` post-Task-5; for THIS task a parse error + any
  currently-reachable sema error suffice). Record for each: stdout bytes,
  stderr bytes (including the `Emitted N errors. exiting.` line —
  `CompilerDriver.cpp:2078,2090` — and whether N counts errors only), exit code
  (S0's review measured 2 — confirm), and whether `-Wno-undefined-variable`-type
  flags matter (they don't for these).
  Read `tools/hermesc/hermesc.cpp`'s `main` return paths + the
  `compileFromCommandLineOptions` failure path to confirm WHERE the line prints
  and WHICH failures print it (parse vs sema vs backend — backend variants at
  CompilerDriver.cpp:1870/1927/1947 are NOT our path).
- [ ] **Step 2: Failing differential**: add `parse-error.js` to the corpus (the
  exact file from step 1a); the harness's "hermesc must succeed" assertion now
  fails → rework the harness per the Files note; the comparison then fails on
  the missing epilogue/exit code.
- [ ] **Step 3: Implement** in the bin: on nonzero error count after parse or
  resolve, print the epilogue line with the true count to stderr (exactly the
  bytes hermesc prints) and exit with hermesc's code. Document each replicated
  behavior with its CompilerDriver/hermesc citation.
- [ ] **Step 4: Gate green** (7 files incl. the error file) + workspace suite.
  **Step 5: Commit** `rust(sema): error-epilogue parity — exit code + 'Emitted N errors' (error corpus enabled)`.

---

### Task 3: `ASTEval` constant folding (untyped part)

**Files:**
- Create: `rust/crates/sema/src/ast_eval.rs`
- Modify: `rust/crates/sema/src/lib.rs`
- Test: `rust/crates/sema/tests/ast_eval.rs`

**Interfaces:**
- Produces: `ast_fold_binary_expression(gc, kw, node: &BinaryExpression-node, ...) -> Option<&'gc Node<'gc>>`
  and `ast_fold_unary_expression(...)` — Rust-shaped ports of
  `lib/Sema/ASTEval.cpp` (95 lines, read ALL): return `Some(folded literal node)`
  where C++ returns true and writes `*ppNode` (the caller substitutes via the
  Task 1 machinery), `None` where C++ returns false. Number semantics MUST be
  bit-exact (the folded literal is dumped by `-dump-ast`-style consumers and its
  presence/absence shapes the sema dump): port the exact operations the C++
  performs — no extra folds, no missing folds.
- Consumes: `Keywords`, node builders (`ast::builder`), `NodeMetadata` from the
  folded expression's range (copy what the C++ does with locations — read
  ASTEval.cpp for `copyLocationFrom`-equivalents).

- [ ] **Step 1: Read** `lib/Sema/ASTEval.cpp` fully; enumerate every operator and
  operand-type combination it folds (and, critically, the ones it declines).
- [ ] **Step 2: Failing tests**: table-driven — for each folded combination,
  build the expression, fold, assert the literal kind/value/location; for each
  declined case assert `None` (identifier operands, non-foldable operators,
  whatever the C++ declines).
- [ ] **Step 3: Implement**; **Step 4:** tests + workspace green; **Step 5: Commit**
  `rust(sema): ASTEval constant folding (ASTEval.cpp, untyped scope)`.

---

### Task 4: Identifier-resolution core

**Files:**
- Modify: `rust/crates/sema/src/resolver/mod.rs` (or a new
  `resolver/identifiers.rs` — split when the module passes ~1.5k lines, mirroring
  the parser's module-directory convention)
- Test: resolver unit tests + corpus files

**Interfaces:**
- Consumes: Task 1 dispatch. Produces:
  `resolve_identifier(&mut self, gc, node_id, ident: &Identifier, in_typeof: bool) -> DeclId`,
  `check_identifier_resolved(...) -> Option<DeclId>`, `declare_arguments(&mut self)`,
  and the resolver state flags later tasks save/restore:
  `can_reference_super`, `forbid_await_expression`, `forbid_await_as_identifier`,
  `forbid_arguments_as_identifier`, `forbid_special_arguments_reference`
  (S0 may have some already — extend, don't duplicate).

- [ ] **Step 1: Port** (each with its exact range, read first):
  `visit(IdentifierNode, parent)` `SemanticResolver.cpp:277-323` — the
  property-key / member-property / label-adjacent / typeof / `$SHBuiltin` /
  private-name-parent skip logic (private/meta/break/continue/labeled parents
  can't occur in the S1 corpus but the checks port now — they're parent-kind
  tests, not visits); `resolveIdentifier` `:1967-2031` (Arguments special +
  usesArguments, forbid-flag errors, the strict-mode `UndefinedVariable`
  warning with its EXACT message construction incl. `global`/anonymous naming
  — check `support`'s warning-kind enum for an UndefinedVariable entry and add
  it if missing, matching hermesc's warning-flag machinery so the dump/stderr
  bytes match), ambient-global creation + `tryEmplaceIntoScope(globalScope_)`;
  `checkIdentifierResolved` `:2068-2086`; `declareArguments`
  `SemanticResolver.h:349-355` (read it — it calls `funcArgumentsDecl` and
  binds); `FunctionContext::getFunctionName` (`SemanticResolver.cpp` near the
  ctors — needed by the warning text).
- [ ] **Step 2: Tests + corpus**: unit tests for expression-decl caching
  (second resolve hits the node Cell, not the table); corpus files: global
  reads/writes (`x; y = x;` loose — UndeclaredGlobalProperty + `E:` refs),
  strict-mode undefined-variable warning file (stderr!), `typeof missing;`
  (no warning), property/member identifier skips (`a.b; ({c: 1});` — with
  object literals routed generically). Every file verified vs hermesc first.
- [ ] **Step 3:** gate + workspace green. **Step 4: Commit**
  `rust(sema): identifier resolution core (cpp:277-323,1967-2086)`.

---

### Task 5: Declarations — hoisting, validation, blocks

**Files:**
- Modify: `rust/crates/sema/src/resolver/` (likely a new `declarations.rs`)
- Test: resolver unit tests + corpus files (incl. error files)

**Interfaces:**
- Consumes: Tasks 1+4. Produces:
  `extract_idents_from_decl(...) -> DeclKind` (cpp:2262-2352 — READ IT, it was
  not excerpted during planning), `extract_declared_idents_from_id(...) -> bool`
  (cpp:2353-2405), `process_declarations`/`process_collected_declarations`
  (cpp:2088-2127 — replacing the S0 panic; the `typed_` builtin branch ports
  dormant), `validate_and_declare_identifier` (cpp:2407-2639 — the redeclaration
  decision table; port the ENTIRE comment block verbatim, it is the spec),
  `validate_declaration_name` (cpp:2641-2677), `visit VariableDeclaration`
  (cpp:325-403 — the `using` error + the nested-scope var-vs-let check with
  `findWithDepth` + promotedFuncDecls interplay, dormant until S3),
  `visit BlockStatement` (cpp:502-518).

- [ ] **Step 1: Port** in the order listed (each function's tests written first
  from the C++ semantics — the redeclaration table cases become the test list:
  var/var merge, let-then-var error, var-then-let-same-scope error,
  let/let-same-scope error, parameter-then-toplevel-let error, catch cases
  dormant-noted, ES5Catch+var special note (points at cpp:336-352),
  restricted-globals (`let NaN` at global scope errors), `let let` error,
  strict-mode `arguments`/`eval` declaration errors).
- [ ] **Step 2: Corpus**: destructuring declarations
  (`let {a, b: [c], ...rest} = x;` — extract + AssignmentPattern init exprs),
  hoisting shapes (`var` above use; block `let` shadowing an outer `var`),
  blocks with decls, and ERROR files for each table row above (now first-class
  thanks to Task 2). Verify each vs hermesc; expect the `previous declaration`
  NOTE lines in stderr — the byte-compatible note machinery must match.
- [ ] **Step 3:** gate + workspace. **Step 4: Commit**
  `rust(sema): declarations — hoisting + validateAndDeclareIdentifier (cpp:2262-2677,325-403,502-518)`.

---

### Task 6: Expressions — folding integration, assignment, update, unary

**Files:**
- Modify: `rust/crates/sema/src/resolver/` (expressions arm)
- Test: corpus + unit tests

**Interfaces:**
- Consumes: Tasks 1+3+4. Produces the visit arms:
  BinaryExpression (cpp:405-436: `+`/`-` linearization + MAX_NESTED_BINARY
  guard + the left-to-right fold loop writing into `list[i+1]->_left` — in Rust
  the loop substitutes rebuilt spine nodes; think this through against Task 1's
  machinery and document the mapping), UnaryExpression (cpp:475-500: strict
  `delete`-of-variable error; `delete super.x` ports with the test262 setting
  as a documented-false constant like S0's DebugInfoSetting; fold call),
  AssignmentExpression (cpp:438-462: `linearize_right` + MAX_NESTED_ASSIGNMENTS
  + validateAssignmentTarget per link), UpdateExpression (cpp:464-473),
  `validate_assignment_target` (cpp:2679-2711), `is_lvalue` (cpp:2713-2757 —
  const-assignment error path, strict `eval`/`arguments` targets, the loose-mode
  `arguments` IMPORTANT-comment quirk). Also: extend the generic-whitelist to
  every S1-corpus expression kind WITHOUT a C++ override (Member/Object/Array/
  Conditional/Logical/Sequence/Property/TemplateLiteral — confirm each against
  the C++ inventory before whitelisting).
- [ ] **Corpus**: `1 + 2 - 3;` and mixed fold chains (folding now byte-matches
  hermesc — the S0 restriction lifts), `-5; !true;` unary folds, non-foldable
  binaries, chained assignment `a = b = c;`, const-assignment error, update-on-
  literal error, strict delete-variable error (needs a strict function — lands
  after Task 7 if ordering demands; otherwise `"use strict"` at program level).
- [ ] Gate + workspace; commit
  `rust(sema): expressions — folds wired, assignment/update/unary validation`.

---

### Task 7: Functions — parameter scopes, bodies, `arguments`

**Files:**
- Modify: `rust/crates/sema/src/resolver/` (likely `functions.rs`)
- Test: corpus + unit tests + the nested-scope unwind regression test

**Interfaces:**
- Consumes: everything above. Produces: `visit FunctionDeclaration`
  (cpp:233-243 — the `hoistedFunctions` push + dispatch), `visit
  FunctionExpression` (cpp:244-248), `visit_function_like` (cpp:1646-1683 —
  the MethodDefinition/constructor branch guards on `curClassContext_`: port as
  a documented S2 seam that panics if the parent is a MethodDefinition;
  arrow branches port but arrows themselves stay S2 — the ArrowFunctionExpression
  KIND keeps its S1 panic), `visit_function_like_in_function_context`
  (cpp:1685-1882: async-generator error branch; directives + strictness;
  function-id expression-decl + FunctionExprName validation; the LAZY body
  branch — port it reading `BlockStatement.{is_lazy_function_body,
  contains_arrow_functions, may_contain_arrow_functions_using_arguments}` Cells,
  exercised in S5; simpleParameterList/hasParameterExpressions; the
  use-strict-non-simple-params error using S0's `use_strict_node`; uniqueParams;
  `declareParams`/`visitParams` closures with param redeclaration + binding
  update semantics cpp:1762-1824; the hasParameterExpressions dual-scope layout
  cpp:1846-1881 incl. the temporary-arguments scope), 
  `visit_function_body_after_params_visited` (cpp:1884-1945: DebugInfoSetting
  documented-false; processCollectedDeclarations; the promoted-hook guarded by
  the S3 assert; the `arguments` declaration decision cpp:1919-1924 with its
  IMPORTANT non-spec-compliant comment; the dead `if (false)` Unresolver branch
  ported AS dead code with its TODO comment; `mayReachImplicitReturn` DEFERRED
  to S2 with an explicit comment — it is dump-invisible, field stays default),
  `visit_function_expression` (cpp:1947-1965 — FunctionExprName scope),
  `visit ReturnStatement` (cpp:1469-1475 — read it; `allowReturnOutsideFunction`
  is a context setting: check what the Rust ast::Context has and port the
  condition), and the FunctionContext save/restores for the Task 4 flags.
- [ ] **Corpus**: named/anonymous function declarations + expressions, nested
  functions, parameters (simple, defaults, destructuring, rest — exercising the
  dual-scope layout), `arguments` capture (`function f() { return arguments; }`),
  function-expression name self-reference, strict functions
  (directive + inherited), duplicate-param error (strict) + allowed duplicate
  (loose simple), `'use strict'` + non-simple params error, return-outside-
  function error, param named `let` (strict) error. LOOSE-mode files: no
  block-nested function declarations (S3). Verify each vs hermesc first.
- [ ] **The unwind regression test** (S0 carry-item): with functions + blocks,
  ≥2 binding scopes now nest through the public API — add the test proving a
  mid-resolution panic (e.g. an S2-kind node deep inside) unwinds cleanly
  (normal panic, no abort) with nested scopes open.
- [ ] Gate + workspace; commit
  `rust(sema): functions — param scopes, bodies, arguments (cpp:233-248,1646-1965)`.

---

### Task 8: Corpus sweep — `test/Sema` seeds + breadth

**Files:**
- Modify: `rust/crates/sema/tests/sema_corpus/` (+ a `MANIFEST.md` in the dir)
- Possibly tiny resolver fixes for anything the sweep exposes (each with a test)

- [ ] **Step 1:** For each of the 56 `test/Sema/*.js` files: run hermesc
  `-dump-sema` (with the file's lit RUN-line flags if any — read the header) and
  the Rust bin; classify: (a) matches → add to corpus; (b) uses S2+/S3/S4
  constructs → record in `MANIFEST.md` with the blocking construct and target
  phase; (c) MISMATCH on S1-scope constructs → a real S1 bug: fix it (TDD) in
  this task.
- [ ] **Step 2:** Ensure the corpus covers every feature Tasks 4-7 shipped (grep
  the corpus for each construct; add targeted files for gaps).
- [ ] **Step 3:** gate (report the final matched count) + workspace; commit
  `rust(sema): S1 corpus sweep — test/Sema seeds + MANIFEST of deferred files`.

---

### Task 9: Docs + spec corrections

**Files:**
- Modify: `doc/superpowers/RustPortRoadmap.md` (Sema row: S1 done — shipped list,
  final corpus count, S2 remainder list from this plan's Architecture section,
  S3 constraint reminder)
- Modify: `doc/superpowers/specs/2026-07-26-sema-untyped-design.md`:
  §1 table — `ASTEval.cpp` moves to IN scope (untyped folding), noted as shipped
  in S1; §3.4 — add the fifth mutation "site": generic child replacement via
  `Node **ppNode` (constant folds), with the note that the VisitorMut mechanism
  landed in S1 as designed; §6 — S1 marked done.
- Modify: `doc/superpowers/SESSION-HANDOFF.md` — status line: S1 done, S2 next.

- [ ] Make the edits (summary-level, matching each file's voice); run the gate
  once as a final sanity; commit `doc(rust): Sema S1 declarations & scopes complete`.

---

## Self-review notes (plan-writing time)

- **Coverage vs the S1 phase definition:** scope creation (T5/T7 ScopeRAII uses),
  hoisting (T5), parameter scopes (T7), identifier-resolution core (T4) — all
  present; plus the S0 review carry-items: error epilogue (T2), recursion depth
  (T1), unwind regression test (T7), VisitorMut signature change (T1), constant
  folding (T3/T6). `mayReachImplicitReturn`, arrows, classes, loops/labels,
  catch, call specials → explicitly S2; promotion → S3; lazy branches ported
  but exercised in S5.
- **Known plan-time gaps stated as read-first obligations:** cpp:2262-2352
  (`extractIdentsFromDecl`) and `ASTEval.cpp` were not excerpted during
  planning — their tasks start with a full read; `MAX_NESTED_*` values and the
  `Warning::UndefinedVariable` support-enum status are verified in-task.
- **Type consistency:** Task 1's dispatch signature is deliberately pinned to
  "whatever `ast::VisitorMut` actually is" with a read-first step, and every
  later task consumes "Task 1 dispatch" rather than restating a guessed
  signature.
