# Sema S2 (Rest of the Walk) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.
> **This plan was written in the S1 session (context-rich); it is designed to be
> executed in a FRESH session** — every task brief is self-contained, and the
> cross-phase knowledge is baked into the task texts below.

**Goal:** Port the remaining plain-JS resolver walk — loops/labels/switch,
arrows + rewrites 1-3, try/catch/with, classes + private names + static blocks +
super, call specials (eval/`$SHBuiltin`), spread/yield/await/meta, and
`CheckImplicitReturn` — expanding the differential corpus with the 36 deferred
`test/Sema` files.

**Architecture:** Everything extends the S1 `ast::VisitorMut` resolver
(`rust/crates/sema/src/resolver/`). The three remaining spec-§3.4 rewrites in
scope (arrow body, try-split, `$SHBuiltin`) each follow the C++
transform-then-visit order: build the replacement subtree at the exact C++ point,
visit the NEW subtree, return `Changed`. NOT in S2: promotion (S3), modules +
`export default` rewrite + the `$SHBuiltin.moduleFactory/export/import` branches
(S4), lazy/eval entries (S5), typed dialects + the flow corpus (S4 "flavors"),
and real regex validation (needs the regex engine — its own future component;
documented deferral in Task 3).

**Tech Stack:** as S0/S1. C++ source of truth: `lib/Sema/SemanticResolver.{h,cpp}`
(the S2 ranges cited per task), `lib/Sema/CheckImplicitReturn.cpp`.

## Global Constraints

- NEVER `cd`; `--manifest-path rust/Cargo.toml` / absolute paths.
- Zero warnings with AND without `--features dump-bin`; no new clippy lints;
  `#![forbid(unsafe_code)]`; 80-col new lines; copyright headers; every C++
  citation verified with grep/awk before writing (S0/S1 reviews caught repeated
  drift — the reviewers check).
- Faithful port: C++ comments carried over; bug-for-bug quirks preserved and
  flagged, never "fixed"; C++ default args are spec; templates stay generics;
  `SaveAndRestore` → the crate's established save/restore locals pattern.
- **The decorate-before-recurse invariant** (resolver/mod.rs module doc): a
  node `Cell` written AFTER visiting that node's children is LOST if a child
  visit returned `Changed` (builders snapshot Cells at `from_node`). The ONE
  known C++ exception is Switch (Task 1 handles it); any new one you find must
  be handled the same way (write to the node you RETURN) and reported.
- The gate: `REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p sema --features dump-bin --test sema_differential -- --nocapture`
  (oracle `cmake-build-asan/bin/hermesc`; starts at 69 files). EVERY corpus file
  verified against hermesc FIRST (stdout+stderr+exit, raw bytes); fix the Rust
  side, never curate away a fixable mismatch. Parser-note mismatches: fix the
  parser site faithfully (T2-S1 precedent), citing the C++ call.
- TDD per task; full workspace suite before each commit; commits
  `rust(sema): <what>` + trailer `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.
- S1 left loud panics on every S2 kind — each task REPLACES its panics and must
  leave S3/S4/S5 kinds (and `$SHBuiltin.moduleFactory/export/import`) panicking
  loudly with phase-tagged messages.

---

### Task 1: Loops, labeled statements, break/continue, switch

**Files:**
- Create: `rust/crates/sema/src/resolver/statements.rs`
- Modify: `rust/crates/sema/src/resolver/mod.rs` (dispatch arms; `FunctionContext`
  label fields are already typed from S0: `label_map: HashMap<AtomBytes, Label>`,
  `current_loop`/`current_loop_or_switch` — check exact names/types and wire)
- Test: corpus + `rust/crates/sema/tests/resolver.rs`

**Interfaces:**
- Consumes: S1 dispatch, `FunctionInfo::allocate_label()`, ScopeRAII,
  `process_declarations`, `validate_assignment_target`.
- Produces: visit arms for `For`/`ForIn`/`ForOf`/`While`/`DoWhile`/`Switch`/
  `Labeled`/`Break`/`Continue`; helper `label_index_of(node) -> u32` (port of
  `getLabelDecorationBase`, cpp:681-693 — a match over the 5 label-bearing kinds
  reading the `label_index` Cell).

- [ ] **Step 1: Read** SemanticResolver.cpp:520-756 fully. The ports:
  `visit(Switch)` :520-539 — **the known decorate-after-children exception**:
  C++ visits `_discriminant` FIRST (:522), then `setLabelIndex` (:526). Under
  the rebuild mechanism you must write the label to the node you RETURN: visit
  the discriminant via `self.call`; if `Changed`, seed the builder with the new
  discriminant and write `label_index` on the node that will be returned (the
  rebuilt one) — never only on the original. Then save/restore
  `current_loop_or_switch`, ScopeRAII, conditional `process_declarations` (only
  if decls exist — :532-536, matches C++), visit cases. NOTE: `label_index` is
  NOT printed by `-dump-sema`, so the differential is BLIND to label bugs —
  unit-test label indices directly (see Step 2).
  `visitForInOf` :549-598 (label; save/restores; scope; decls; left visit; the
  init-validation matrix :571-594 — destructuring-init error, for-in loose var
  init exception; else `validate_assignment_target`; right; body).
  `visit(For)` :600-614, `DoWhile` :616-625, `While` :626-635 (label +
  save/restores; For also scope+decls).
  `visit(Labeled)` :637-678: label allocation; the target-statement walk
  :642-652; `label_map.try_emplace` with duplicate error + note (:662-668);
  the scope-exit ERASE (:670-675) — use a drop-guard or explicit erase that
  runs even on early paths (match the C++ `make_scope_exit` semantics).
  `Break` :695-721 / `Continue` :723-755: labeled + unlabeled paths, the
  loop-label check with error+note, reading target label indices via
  `label_index_of`. (Targets are ANCESTORS still being visited: their
  `label_index` Cell was written at entry, and builders copy Cells on rebuild —
  note this reasoning in a comment.)
- [ ] **Step 2: Tests first**: unit tests asserting `label_index` values directly
  (nested loops + labeled break/continue: hand-resolve, then read the Cells off
  the RETURNED tree — including a switch whose discriminant FOLDS
  (`switch (1+2) {...}`), asserting the rebuilt switch carries the label — the
  regression test for the known exception). Then corpus (hermesc-verified
  first): loop shapes, labeled loops, `break`/`continue` with and without
  labels, switch with case decls (`case 0: let x;`), and error files:
  duplicate label, undefined label, `continue` non-loop label (error+note),
  `break`/`continue` outside loop.
- [ ] **Step 3:** gate + workspace green. **Step 4: Commit**
  `rust(sema): loops, labels, break/continue, switch (cpp:520-756)`.

---

### Task 2: Arrow functions (rewrite #1) + yield/await/spread/meta + Cover errors

**Files:**
- Modify: `rust/crates/sema/src/resolver/functions.rs` (arrow arm),
  `rust/crates/sema/src/resolver/mod.rs` (dispatch), new arms in
  `statements.rs` or `expressions.rs` as fits (follow the file split by node
  family)
- Test: corpus + resolver tests

**Interfaces:**
- Consumes: T7-S1's `visit_function_like` (its arrow branches — super/arguments
  inheritance, `visitParams` await rules — are already ported and dormant);
  `FunctionContext::is_formal_params`; `forbid_await_expression` (dormant since
  S1-T4, wired here).
- Produces: `ArrowFunctionExpression`, `YieldExpression`, `AwaitExpression`,
  `SpreadElement`, `MetaProperty`, and the Cover-node error visits.

- [ ] **Step 1: Read + port** `visit(ArrowFunctionExpression)`
  SemanticResolver.cpp:249-275: **rewrite #1** — when `compile_ && _expression`,
  build `ReturnStatement(body)` + `BlockStatement([ret], true)` with the C++'s
  exact `copyLocationFrom` calls (:255,:262), then run the normal
  `visit_function_like` flow ON the new body, set `expression=false` (a
  `Cell<bool>` — write it on the node you RETURN), and return `Changed(new
  arrow)`. Then the post-visit bookkeeping :270-274 (containsArrowFunctions +
  the usingArguments propagation — note it reads the arrow's OWN sem_info,
  written during the visit). Also REMOVE the arrow panics in the S1 paths that
  guarded arrows (grep `ArrowFunctionExpression` in resolver/ — the
  `visit_params` await logic and `visit_function_like` isArrow branches become
  live).
  Port `visit(Yield)` :1476-1493 (generator check via `functionContext()->node`
  + isGenerator; isFormalParams error), `visit(Await)` :1494-1509
  (`forbid_await_expression` + isFormalParams), `visit(Spread)` :1455-1468
  (parent-kind whitelist incl. the Flow RecordExpressionProperties arm —
  unconditional in our single node set), `visit(MetaProperty)` :837-872
  (new.target checks incl. the nearestNonArrow-in-global-arrow error;
  import.meta compile error; invalid-meta error), and the Cover visits
  :1558-1578 (read them — they're error-reporting stubs for parser cover nodes;
  port each message exactly).
- [ ] **Step 2: Corpus** (hermesc-verified): arrows (expression + block bodies —
  the dump shows the REWRITTEN block+return, byte-compared), nested arrows,
  arrows capturing `arguments` (the containsArrowFunctionsUsingArguments
  propagation is dump-invisible — unit-test it), async arrows + await, arrow
  param defaults (the S1 capstone's fold-inside-arrow shape), generators with
  yield, yield/await error files (formal-params, non-generator, non-async),
  `new.target` in function vs the two error shapes, spread in calls/arrays vs
  the error shape, **the two S1-capstone corpus pins: an async-generator error
  file and a named-FunctionExpression-with-fold file**.
- [ ] **Step 3:** gate + workspace. **Step 4: Commit**
  `rust(sema): arrows (rewrite 1) + yield/await/spread/meta/cover (cpp:249-275,837-872,1455-1509,1558-1578)`.

---

### Task 3: try/catch (rewrite #2), with + Unresolver, regexp visit

**Files:**
- Modify: `rust/crates/sema/src/resolver/statements.rs`, `mod.rs`
- Create: `rust/crates/sema/src/resolver/unresolver.rs`
- Test: corpus + resolver tests

**Interfaces:**
- Consumes: the catch decl kinds (`Catch`/`ES5Catch` — S1-T5's matrix rows go
  live), ScopeRAII, `process_collected_declarations`.
- Produces: `TryStatement`/`CatchClause`/`WithStatement`/`RegExpLiteral` arms;
  `Unresolver::run(sem_ctx, depth, root)` (port of the visitor declared at
  SemanticResolver.h — find its impl via `grep -n "Unresolver::" lib/Sema/` and
  port whole: it marks identifier decls below a depth unresolvable, which flips
  the ` UNR` dump suffix — dump-VISIBLE).

- [ ] **Step 1: Port** `visit(Try)` cpp:771-811 — **rewrite #2**: when
  `compile_ && handler && finalizer`, build the nested try
  (`TryStatement(block, handler, None)` with `copyLocationFrom(tryStatement)` +
  `setEndLoc(handler end)` :797-798) wrapped in a
  `BlockStatement([nestedTry], false)` with `copyLocationFrom(nestedTry)`, and
  the outer try gets `block=newBlock, handler=None` — under VisitorMut: build
  the full new outer TryStatement THEN visit its children (block/handler=None/
  finalizer) and return Changed. The dump shows the nested structure
  byte-for-byte (S0's very first corpus already exercised the C++ side of
  this). `visit(CatchClause)` :813-819 (scope; catch-param decls via
  process_collected_declarations — the DeclCollector catch handling from S0-T7
  feeds this; body makes its own scope). `visit(With)` :757-769: compile error
  + children + `Unresolver::run(depth+1, body)` — port the Unresolver
  faithfully (its identifier visit + depth rule), and note the C++ runs it even
  after the error (error count already nonzero → resolveAST returns false, but
  the dump/diagnostics ordering still matters — check what hermesc -dump-sema
  actually outputs for a `with` file and match). `visit(RegExpLiteral)`
  :821-835: **documented deferral** — the C++ calls
  `CompiledRegExp::tryCompile` (the real regex engine, a future component).
  Port the visit with the compile_ structure but a stub validator that accepts
  everything, a `// REGEX-ENGINE DEFERRED` comment block explaining the
  boundary, and a loud panic ONLY if that ever changes dump bytes (it cannot:
  valid regexes produce no sema output; INVALID ones would — so corpus must use
  only valid regexes, and an invalid-regex error file goes to the MANIFEST as
  deferred-to-regex-component).
- [ ] **Step 2: Corpus**: try/catch, try/finally, try/catch/finally (the
  rewrite, dump-visible), catch with destructuring param, the catch redecl
  error rows from S1-T5's matrix (now reachable: `catch(e) { let e; }` etc. —
  re-check each against hermesc), `with` (error file), valid regex literals.
  Unit test: Unresolver flips `unresolvable` + the dump prints ` UNR` (compare
  a hermesc `with` dump if hermesc emits it — with compile_ erroring the dump
  may not print; verify empirically what the oracle does and match).
- [ ] **Step 3:** gate + workspace. **Step 4: Commit**
  `rust(sema): try/catch rewrite, with + Unresolver, regexp visit (cpp:757-835)`.

---

### Task 4: Classes core — ClassContext, class-as-expr, properties, methods, super

**Files:**
- Create: `rust/crates/sema/src/resolver/classes.rs`
- Modify: `mod.rs` (dispatch; `cur_class_context` state), `functions.rs`
  (NARROW the MethodDefinition seam: the panic becomes the real cpp:1652-1662
  constructor-kind logic — do not delete the seam, replace it)
- Test: corpus + resolver tests

**Interfaces:**
- Consumes: ScopeRAII, `validate_declaration_name`, `new_decl_in_scope`,
  `SemContext::{new_function, get_constructor, node_is_arrow, nearest_non_arrow}`,
  `FunctionInfo::ConstructorKind`.
- Produces: `ClassContext` (port of SemanticResolver.h:630-682 + cpp:3081-3186:
  ctor/dtor push-pop on `cur_class_context`, `has_constructor`,
  `is_derived_class`, `create_implicit_constructor_function_info`,
  `get_or_create_instance_elements_init_function_info`,
  `get_or_create_static_elements_init_function_info`,
  `create_static_block_function_info` — these create SYNTHETIC FunctionInfos
  that appear in the `-dump-sema` function tree, so the differential covers
  them; read cpp:3081-3186 for the exact FunctionInfo parameters each uses and
  WHERE the created ids are stored — the ClassLike decoration? check
  `ESTree.h`'s ClassLikeDecoration for fields and the Rust node's generated
  Cells); visit arms for `ClassDeclaration`/`ClassExpression` (untyped path =
  `visit_class_as_expr` for BOTH, cpp:891-950 — the typed_ branch ports
  dormant), `ClassProperty` (cpp:1008-1051 — computed-key canRefSuper=false;
  initializer visited inside the synthetic init FunctionContext with the three
  save/restores + declareArguments :1039), `MethodDefinition` (cpp:1094-1115 —
  computed key; the private-instance-method init hook :1109-1111 dormant until
  Task 5; body), `Super` (cpp:1086-1092).
- [ ] **Step 1: Read** all cited ranges + SemanticResolver.h:630-682 FIRST;
  enumerate in your report where each synthetic FunctionInfo's id lands (node
  Cell vs ClassContext field) — this is the load-bearing detail for the dump
  tree shape.
- [ ] **Step 2: Port** with the strict-mode SaveAndRestore (classes force
  strict, cpp:894/:919), the ClassExprName decl semantics (cpp:923-935 — note
  it sets EXPRESSION decl so class declarations carry two decls on one
  identifier: the side-table case S0-T4 built), superclass visited before
  private names (:938), decorator errors (:914-916, :1009-1011, :1097-1099).
- [ ] **Step 3: Corpus**: classes (decl + expr, named + anonymous), methods
  (plain/computed/getter/setter/constructor), class properties (with/without
  init, computed, static), derived classes + `super.x` + `super()` (the CALL
  check is Task 6 — super() files wait for it; `super.x` member access works
  now), class-expr name self-reference, `class` TDZ/redecl error files, the
  implicit-constructor + elements-init synthetic functions visible in dumps
  (pick shapes that exercise each getter: instance init, static init, both).
- [ ] **Step 4:** gate + workspace. **Step 5: Commit**
  `rust(sema): classes core — ClassContext + class-as-expr + properties/methods/super (cpp:891-1115,3081-3186)`.

---

### Task 5: Private names + static blocks

**Files:**
- Modify: `rust/crates/sema/src/resolver/classes.rs`, `identifiers.rs`
  (declare/resolve private), `expressions.rs` or `classes.rs` (member
  private-name branches), `mod.rs`
- Test: corpus + resolver tests

**Interfaces:**
- Consumes: Task 4's ClassContext; `SemContext` decl-state machine + `decl_mut`
  (the accessor-pair port MUTATES a Decl's kind to `PrivateGetterSetter` —
  cpp:2255); the private Decl kinds + `DeclSpecial::PrivateStatic`.
- Produces: `collect_declared_private_identifiers` (cpp:2143-2261 — the whole
  early-error machinery: field dups, method dups (the typed_ overload branch
  dormant), getter/setter pairing incl. static-mismatch error and the
  kind-upgrade + `set_both_decl` on the second accessor);
  `declare_private_name`/`resolve_private_name` (cpp:2033-2066 — needs the
  private-name string mangling: find `Context::getPrivateNameIdentifier` in the
  C++ AST Context, port its exact transformation as a sema-crate helper);
  `PrivateName` visit (cpp:952-963); `ClassPrivateProperty` (cpp:965-1006);
  the `MemberExpression`/`OptionalMemberExpression` private-name branches
  (cpp:1207-1320 — read them NOW, they were only skimmed in S1: the
  parent-kind/delete/assignment restrictions on private members — S1-T4
  established the non-private path reduces to pass-through; this task ports the
  private path); `StaticBlock` (cpp:1053-1084 — synthetic FunctionInfo via
  ClassContext, FunctionContext static-block ctor (exists from S0),
  functionBodyScope, the four flag save/restores incl.
  `forbid_arguments_as_identifier=true` — **which makes S1-T4's
  documented typeof double-fire quirk reachable: add the corpus file**
  (`class C { static { typeof arguments; } }` — hermesc emits the error TWICE;
  ours must too; it already does by construction — pin it)).
- [ ] **Corpus**: private fields/methods/getters/setters, `#x in` checks if the
  parser supports them (verify), the dup-error matrix from cpp:2143-2261 (field
  dup, method dup, getter+getter, static/instance accessor mismatch, legal
  getter+setter pair), private member access + the cpp:1207-1320 restriction
  errors, static blocks (var hoisting INTO the block, `await`/`arguments`
  errors inside), the typeof double-fire pin.
- [ ] Gate + workspace. Commit:
  `rust(sema): private names + static blocks (cpp:952-1006,1053-1084,1207-1320,2033-2066,2143-2261)`.

---

### Task 6: CallExpression specials — eval, `$SHBuiltin` (rewrite #3), super()

**Files:**
- Modify: `rust/crates/sema/src/resolver/expressions.rs` (or a `calls.rs` if
  cleaner), `mod.rs`
- Test: corpus + resolver tests

**Interfaces:**
- Consumes: `resolve_identifier`, `nearest_non_arrow`, `ConstructorKind`
  (Task 4), the binding table, `LexicalScope` records (`local_eval` field +
  parent chain).
- Produces: `CallExpression` visit (cpp:1117-1205), `OptionalCallExpression`
  (check the C++ inventory for its visit — if none, generic),
  `NewExpression` (inventory check — likely generic), `register_local_eval`
  (port of the static at SemanticResolver.h/cpp — find it: marks scope +
  ancestors localEval=true; find via grep `registerLocalEval`).

- [ ] **Step 1: Port** cpp:1117-1205: the eval detection (:1119-1150 — binding
  lookup determines "looks like global eval"; `getEnableEval` — check the Rust
  ast::Context for an enable_eval flag (S0 ported some Context flags; add
  faithfully if missing, hermesc default ON) → DirectEval warning +
  `register_local_eval`, or EvalDisabled warning); **rewrite #3** (:1153-1193):
  on `$SHBuiltin.prop(...)` with unshadowed `$SHBuiltin` —
  `resolve_identifier(ident, false)` (this CREATES the ambient decl, affecting
  decl numbering — the dump differential verifies), build `SHBuiltinNode`
  (copyLocationFrom object), rebuild the member + call, THEN: `moduleFactory`/
  `export`/`import` property branches get loud S4 panics (phase-tagged; the
  `export` branch's children-first subtlety is S4's problem — document it);
  otherwise visit children of the REBUILT call and return Changed. The super()
  constructor check (:1195-1202) via nearest_non_arrow ConstructorKind.
- [ ] **Step 2: Corpus**: plain calls (huge unlock — most deferred test/Sema
  files call functions), direct eval warning file (loose + a shadowed-eval
  no-warning file), `$SHBuiltin.foo(1)` (dump shows `SHBuiltin` node — the S0
  ASTPrinter already prints it) + shadowed `$SHBuiltin` (no rewrite; the
  `invalid use of $SHBuiltin` error from S1-T4's identifier visit fires —
  verify against hermesc), super() in derived constructor + the two error
  shapes (non-derived, outside constructor), optional calls `a?.()`.
  Unit test: `register_local_eval` marks the scope chain (`local_eval` is
  dump-invisible — unit-test it) .
- [ ] **Step 3:** gate + workspace. **Step 4: Commit**
  `rust(sema): call specials — eval, $SHBuiltin rewrite, super() (cpp:1117-1205)`.

---

### Task 7: `CheckImplicitReturn`

**Files:**
- Create: `rust/crates/sema/src/check_implicit_return.rs`
- Modify: `rust/crates/sema/src/lib.rs`, `resolver/functions.rs` (replace the
  deferred-S2 comment with the real call, cpp:1939-1944 — only when
  `sm.error_count() == 0`, per the C++ comment about break/continue resolution)
- Test: `rust/crates/sema/tests/check_implicit_return.rs`

**Interfaces:**
- Consumes: the resolved AST (label indices from Task 1 — the C++ comment says
  it relies on break/continue being resolved), `FunctionInfo::may_reach_implicit_return`.
- Produces: `may_reach_implicit_return(root: &Node) -> bool` — port of
  `lib/Sema/CheckImplicitReturn.cpp` (335 lines, read ALL; entry at :320).

- [ ] **Step 1: Read the whole file**; port the internal
  reachability analysis structure-for-structure (whatever visitor/state shape
  it uses — keep its form, its comments, its conservatism).
- [ ] **Step 2: Tests**: the result is DUMP-INVISIBLE (no differential
  coverage) — table-driven unit tests are the gate: hand-resolve function
  shapes and assert the flag: falls-off-end → true; unconditional return →
  false; if/else both-return → false; if-without-else → true; while(true)
  without break → whatever the C++ computes (derive each expected value from
  READING the C++, then verify the interesting ones against C++ behavior via
  IRGen-observable effects if simple, else trust the source reading and say
  so); loops with labeled breaks; try/finally shapes; switch with/without
  default. Enumerate every statement kind the C++ analysis handles and cover
  each.
- [ ] **Step 3:** wire the call; full suite + gate (unchanged corpus must stay
  green — the flag changes no bytes). **Step 4: Commit**
  `rust(sema): CheckImplicitReturn (CheckImplicitReturn.cpp) + wiring`.

---

### Task 8: Corpus sweep round 2

**Files:**
- Modify: `rust/crates/sema/tests/sema_corpus/` + `MANIFEST.md`
- Possibly small fixes (TDD) for anything the sweep exposes

- [ ] For every MANIFEST row deferred to S2 (36 files): re-run both binaries;
  import the matches; for mismatches on S2-scope constructs, FIX (smallest
  repro, TDD). Files still blocked (S3 promotion shapes, S4 modules, the
  recursion-limit file, invalid-regex if any) get updated MANIFEST reasons.
  Also fix the MANIFEST's flow-count off-by-one (178 files in flow/**, not
  179 — the S1 capstone caught it).
- [ ] Feature-coverage grep per Task 1-7 feature; add targeted files for gaps.
- [ ] Gate green (report final count); workspace green. Commit:
  `rust(sema): S2 corpus sweep — deferred test/Sema files imported`.

---

### Task 9: Docs

**Files:**
- Modify: `doc/superpowers/RustPortRoadmap.md` (Sema row: S2 done — summary,
  final corpus count, remaining S3/S4/S5 list + the regex-engine deferral +
  parser follow-up items), `doc/superpowers/specs/2026-07-26-sema-untyped-design.md`
  (§3.4: rewrites 1-3 shipped; §6 S2 done), `doc/superpowers/SESSION-HANDOFF.md`
  (S3 ScopedFunctionPromoter next).

- [ ] Edits (each file's voice; roadmap is source of truth); run the gate once;
  commit `doc(rust): Sema S2 rest-of-walk complete`.

---

## Self-review notes (plan-writing time)

- **Coverage vs the S2 remainder list** (roadmap + S1 capstone): loops/labels/
  switch (T1, with the label-to-rebuilt-node fix); try/catch + with + regexp
  (T3, rewrite 2); arrows + rewrite 1 + yield/await/spread/meta/cover (T2);
  classes + private + static (T4/T5); call specials + rewrite 3 (T6);
  CheckImplicitReturn (T7); MethodDefinition seam narrowed (T4);
  the two capstone corpus pins (T2); typeof double-fire pin (T5);
  async-generator pin (T2). Deferred with reasons: regex engine (T3, documented),
  modules + export-default rewrite + SHBuiltin module branches (S4), promotion
  (S3), typed dialects + flow corpus + per-file harness flags (S4), lazy/eval
  (S5), Unresolver's full `localEval` branch (the dead `if(false)` in
  visitFunctionBodyAfterParamsVisited stays dead — only the `with` path runs
  Unresolver, T3).
- **Read-first obligations flagged where planning didn't excerpt**: cpp:1207-1320
  (member private branches), cpp:3081-3186 (ClassContext impl), Unresolver impl,
  `getPrivateNameIdentifier`, `registerLocalEval`, CheckImplicitReturn.cpp,
  Cover visits.
- **Sequencing**: T1 before T7 (C++ comment: CheckImplicitReturn relies on
  break/continue resolution). T4 before T5 (ClassContext) and before T6
  (ConstructorKind for super()). T2's arrows before T6 makes arrow-in-derived
  super() corpus richer but is not a hard dependency.
- **Dump-blindness called out per task** (label indices, local_eval,
  containsArrowFunctionsUsingArguments, may_reach_implicit_return) with unit
  tests mandated — the S1 lesson that the differential proves only what it can
  see.
