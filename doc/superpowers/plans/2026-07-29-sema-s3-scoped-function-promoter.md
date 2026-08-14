# Sema S3 (ScopedFunctionPromoter) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.
> **This plan was written in the S2 session (context-rich); it is designed to be
> executed in a FRESH session** — every task brief is self-contained, and the
> cross-phase knowledge is baked into the task texts below.

**Goal:** Port `ScopedFunctionPromoter` — loose-mode block-nested function
promotion (ES2022 B.3.3) — replacing the two S0/S1 assert seams, and unlock
the three S3-blocked corpus rows plus a promotion-shape corpus battery.

**Architecture:** The promoter is a small READ-ONLY pre-pass (a plain
`ast::Visitor`, NOT `VisitorMut` — the C++ pass never mutates the AST) run
over one function's body right after `process_collected_declarations`. It
returns the list of promotable `FunctionDeclaration`s;
`process_promoted_func_decls` (a new resolver method) then declares each as
`Var`/`GlobalProperty` and records it in `FunctionContext::promoted_func_decls`.
**The consumer side is ALREADY PORTED AND LIVE** (S1/S2): the
`FunctionContext::promoted_func_decls: HashMap<Atom, DeclId>` map, the
`SemContext` promoted-decl side-table (`set_promoted_decl`/`get_promoted_decl`,
`sem_context.rs:1196-1213`), and every redeclaration-matrix row that reads them
(`resolver/declarations.rs:584, :602, :663, :843`) — all fed today by an
always-empty map. S3 supplies only the producer.

**Tech Stack:** as S0-S2. C++ source of truth:
`lib/Sema/ScopedFunctionPromoter.{h,cpp}` (37+328 lines — read the whole .cpp)
and `lib/Sema/SemanticResolver.cpp` (call sites + `processPromotedFuncDecls`,
ranges cited per task).

## Global Constraints

- NEVER `cd`; `--manifest-path rust/Cargo.toml` / absolute paths.
- Zero warnings with AND without `--features dump-bin`; no new clippy lints;
  `#![forbid(unsafe_code)]`; 80-col new lines; copyright headers; every C++
  citation verified with grep/awk before writing (S0-S2 reviews caught repeated
  drift — the reviewers check).
- Faithful port: C++ comments carried over; bug-for-bug quirks preserved and
  flagged, never "fixed"; C++ default args are spec; templates stay generics;
  `SaveAndRestore` → the crate's established save/restore locals pattern.
- **The decorate-before-recurse invariant** (resolver/mod.rs module doc) binds
  node-`Cell` writes. The promoter itself writes NO node Cells (read-only pass)
  and `process_promoted_func_decls` mutates only `SemContext`/`FunctionContext`
  state, which no node rebuild snapshots — note that at the site, mirroring the
  T5 `decl_mut` exemption comment (classes.rs:1006-1012 precedent).
- The gate: `REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p sema --features dump-bin --test sema_differential -- --nocapture`
  (oracle `cmake-build-asan/bin/hermesc`; starts at **160 corpus files matched
  (88 succeeded on hermesc)**). EVERY new corpus file verified against hermesc
  FIRST (stdout+stderr+exit code, raw bytes); fix the Rust side, never curate
  away a fixable mismatch. Parser-note mismatches: fix the parser site
  faithfully, citing the C++ call (T2-S1/T4-S2 precedent). Promotion is
  dump-VISIBLE (decl kinds/scopes change), so the differential is the primary
  gate; unit tests cover the dump-blind map contents.
- Known landmines (all documented in `tests/sema_corpus/MANIFEST.md` — check it
  before fighting a mismatch): same-location diagnostic pairs order unstably in
  C++ (`std::sort`) vs stably in Rust; hermesc ITSELF aborts on
  `class C { x = class {}; }` (SemContext.cpp:478) and `$SHBuiltin.#x()`
  (cpp:1167 cast); per-file harness flags are not supported
  (`-enable-eval=false`, `-parse-flow`, `-lazy` — S4 scope, do NOT build them).
- TDD per task; full workspace suite (`cargo test --manifest-path rust/Cargo.toml`)
  before each commit; commits `rust(sema): <what>` + trailer
  `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.
- Replace ONLY the two S3 assert seams; S4/S5 kinds and branches (module
  visits, `$SHBuiltin.moduleFactory/export/import`, `resolve_ast_for_parser`,
  lazy/eval entries) keep their loud phase-tagged panics. The THIRD C++
  promotion call site (`runInScope`, SemanticResolver.cpp:158) arrives with
  S5's `resolve_ast_in_scope` — leave a note, do not port it.

---

### Task 1: The promoter pass + wiring both seams

**Files:**
- Create: `rust/crates/sema/src/resolver/promoter.rs`
- Modify: `rust/crates/sema/src/resolver/mod.rs` (add `mod promoter;`; replace
  the `visit_program` assert seam at mod.rs:1383-1399 — the
  `!strict` guard and its C++-comment scaffolding are already in place, only
  the assert body changes; add the `process_promoted_func_decls` method)
- Modify: `rust/crates/sema/src/resolver/functions.rs` (replace the assert seam
  at functions.rs:1040-1058 — the `block_body.is_some() && !strict` guard is
  already the faithful cpp:1904-1910 structure)
- Test: `rust/crates/sema/tests/resolver.rs` + 2 seed corpus files

**Interfaces:**
- Consumes: `DeclCollector::{scoped_func_decls, scope_decls_for_node}`
  (decl_collector.rs:149-156); `extract_declared_idents_from_id`
  (declarations.rs:246); `validate_and_declare_identifier`
  (declarations.rs:380); `Keywords` atoms `ident_let`/`ident_const`/`ident_var`
  (keywords.rs:72-74); `FunctionInfo::get_parameter_scope`
  (sem_context.rs:518); `support::PersistentScopedMap` (the resolver's own
  binding-table type — the analog of the C++ promoter's private
  `ScopedHashTable<UniqueString*, bool>`, cpp:112-117).
- Produces: `get_promoted_scoped_func_decls(...) -> Vec<&'gc Node<'gc>>`
  (port of ScopedFunctionPromoter.h:31-33 + cpp:315-325, incl. the
  early-exit when `scoped_func_decls()` is empty);
  `SemanticResolver::process_promoted_func_decls` (port of cpp:2129-2141).

- [ ] **Step 1: Read** the WHOLE `lib/Sema/ScopedFunctionPromoter.cpp` (328
  lines) + `ScopedFunctionPromoter.h`, then the call sites. The ports:
  - The visitor class :23-118 — members :99-117 (`promotedFuncDecls_` result
    vec, `funcNames_` name set, `funcDecls_` candidate set, private
    `bindingTable_`). In Rust, do NOT hold `&mut SemanticResolver`: pass the
    minimal borrowed pieces (the DeclCollector, `&Keywords`, `&SemContext`
    for the parameter scope, and whatever `extract_declared_idents_from_id`
    needs — it lives on the resolver; if borrowing fights you, a free-standing
    reimplementation scoped to the promoter is NOT the answer — restructure
    the borrow, e.g. take `&SemanticResolver` and use interior access, or
    hoist the needed data. Ask before deviating structurally).
  - `run` :120-139 — binding scope; populate both sets from
    `decls->getScopedFuncDecls()`; `processParameters`; `processDeclarations`
    on the function node itself; then children of the Program node OR of
    `getBlockStatement(funcNode)` (the Rust analog from check_implicit_return
    / functions.rs handles the body-shape distinction — reuse the established
    helper, do not re-derive).
  - `visit` arms :36-67 — default = visit children; `FunctionLikeNode` = do
    NOT descend (:43); the SEVEN scope-bearing kinds each call `visitScope`
    (:47-67: Switch, Block, For, ForIn, ForOf, With, CatchClause).
    `visitScope` :141-145 = new binding scope + `processDeclarations` +
    children. Implement as a read `ast::Visitor` using
    `Node::visit_children` (node.rs:8337).
  - `processParameters` :147-158 — walk the function sem-info's PARAMETER
    scope decls; for each `Decl::Kind::Parameter` whose name is in
    `funcNames_`, insert into the binding table (ES2022 B.3.2.1 29.a.ii —
    carry the comment).
  - `processDeclarations` :160-245 — the core. Subtleties: skip
    Flow `TypeAlias` + TS `TSTypeAliasDeclaration` (C++ `#if`-gated; port
    UNCONDITIONALLY per the single-node-set precedent, T2-S2's
    RecordExpressionProperties arm); candidate `FunctionDeclaration`s go to
    `foundDecls`, everything else contributes let-like idents to the binding
    table (but NOT `ES5Catch` — ES14.0 B.3.4, carry the comment :212-216);
    then the promotion decision loop :232-244 (`funcDecls_.erase`; promote
    iff `_id` set and NO visible binding-table entry).
    **KNOWN DEAD CODE — verify, then preserve with a comment:** `newDecls`
    (:174-206) is built and never consumed, and the .h's closing sentence
    ("The ones that can be promoted are deleted from their own scope and
    added to the function scope", h:34-36) describes behavior the current
    implementation does not have. Verify with your own read (grep for any
    consumer of `newDecls`); if truly dead, port the structure with a
    `// DEAD in C++ too:` comment (S2-T3 precedent: the dead
    `if (false && localEval)` branch, functions.rs:1011-1018). If you find a
    consumer this plan missed, follow the C++.
  - `extractDeclaredIdents` :247-311 — the promoter's OWN copy (distinct from
    declarations.rs's `extract_idents_from_decl` — do NOT merge them; C++
    keeps them separate and the kind mapping differs: FunctionDeclaration →
    `ScopedFunction` "so they can be distinguished"). VariableDeclaration
    kind via the keyword atoms (:255-262, incl. the `assert` on var);
    Hook/Component (Flow) → ScopedFunction, unconditional; Class → Class;
    CatchClause → ES5Catch iff the param is a plain Identifier (:287-294);
    the FINAL block :296-310 is an unconditional `cast<ImportDeclarationNode>`
    — any other kind aborts in C++; panic identically (unreachable for
    DeclCollector-collected scope decls; say so in the message).
  - `getPromotedScopedFuncDecls` :315-325 — the public entry; early return
    `vec![]` when `scoped_func_decls()` is empty (this is why every existing
    corpus file is unaffected).
  - `processPromotedFuncDecls` cpp:2129-2141 — kind = `GlobalProperty` if
    `functionContext()->isGlobalScope()` else `Var`;
    `validateAndDeclareIdentifier(kind, ident)`; then `try_emplace` the name
    → `semCtx_.getDeclarationDecl(ident)` into
    `functionContext()->promotedFuncDecls`. Add the SemContext-state
    invariant-exemption comment here (see Global Constraints).
  - The two seams: mod.rs `visit_program` (cpp:224-227) and functions.rs
    `visit_function_body_after_params_visited` (cpp:1904-1910) — both Rust
    guards already match; swap the assert for
    `let promoted = get_promoted_scoped_func_decls(...); self.process_promoted_func_decls(gc, &promoted);`
    keeping the C++ comment `// Promote hoisted functions.` Leave a
    `// S5: the third C++ call site is runInScope (cpp:158)` note at the
    mod.rs site.
- [ ] **Step 2: Tests first (RED).** Two hermesc-verified seed corpus files:
  `promotion-basic.js` (loose function with a block-nested `function f(){}`
  that promotes — top-level AND inside a function, exercising both seams and
  both decl kinds GlobalProperty/Var) and `promotion-blocked-by-let.js`
  (same shape + a visible `let f` — not promoted). Verify both against
  hermesc FIRST; before the port they must FAIL (the assert fires — that IS
  the red state; record the panic text as TDD evidence). Unit tests
  (dump-blind): after resolving a promoting shape, assert
  `FunctionContext`-recorded promotion via the dump-visible decl kind AND
  `SemContext::get_promoted_decl` on the promoted identifier's NodeId;
  assert the blocked shape records nothing.
- [ ] **Step 3:** port per Step 1; gate green (162 files) + workspace green +
  clippy clean both feature configs.
- [ ] **Step 4: Commit**
  `rust(sema): ScopedFunctionPromoter + wiring (ScopedFunctionPromoter.cpp, cpp:224-227,1904-1910,2129-2141)`.

---

### Task 2: Corpus unlock — the three blocked rows + the B.3.3 battery

**Files:**
- Modify: `rust/crates/sema/tests/sema_corpus/` + `MANIFEST.md`,
  `rust/crates/sema/tests/resolver.rs` (only if a shape proves dump-blind)

**Interfaces:**
- Consumes: Task 1's live promotion; the already-ported matrix rows
  (declarations.rs:584 = cpp:2554-2561 Var/ScopedFunction cross-scope reuse;
  :602 = cpp:2569-2577 ES5Catch/ScopedFunction; :663 = cpp:2611-2620 the
  two-declarations `put` path; :843 = the cpp:365-374
  `prevIsLexicalBindingOfPromotedFunc` error row).
- Produces: the S3-blocked MANIFEST rows imported; a promotion battery.

- [ ] **Step 1:** Re-verify each of the three S3-blocked `test/Sema` rows
  against hermesc (stdout+stderr+exit, raw bytes), then import:
  `break-in-nested-func.js`, `function-redeclaration-error.js`,
  `regress-function-promotion-decl.js`. Their MANIFEST rows move
  Deferred → Imported (deferred table 8 → 5).
- [ ] **Step 2:** Feature battery, each file hermesc-verified FIRST, one
  concern per file, comments citing the C++ range it exercises:
  - param-name shadowing blocks promotion (cpp:147-158);
  - catch-param: `Catch` (destructuring) blocks, `ES5Catch` (plain ident)
    does NOT block (cpp:212-216 + :287-294);
  - `const`/`class` blockers (cpp:215 isKindLetLike);
  - nested-scope visibility: a `let` in an OUTER block blocks an inner
    candidate; a `let` in a SIBLING block does not;
  - same-scope `var f` + `function f(){}` in a block (cpp:2554-2561 reuse);
  - promoted function then `var f` at function scope (the reuse row);
  - the cpp:365-374 lexical-binding-of-promoted-func error shape
    (`function f(){} { function g(){} let g; }`-family — derive the exact
    shape from the C++ and hermesc, don't trust this sketch);
  - strict-mode negative (same shape + `"use strict"` → no promotion — decl
    kinds differ in the dump);
  - a `switch`-case-scope candidate (the promoter's Switch arm, cpp:47-49);
  - `with`-scope arm (cpp:62-64) ONLY if a compile-mode shape reaches it —
    probe hermesc first; if unreachable (the `with` error aborts before
    promotion matters), document as not-corpus-reachable instead.
  - The S1-T5 matrix row that was S3-blocked (ES5Catch/ScopedFunction
    promotion, declarations.rs:593-605 = cpp:2569-2577): add the corpus
    shape that reaches it (`try{}catch(e){ { function e(){} } }`-family —
    again derive from the C++ and verify).
- [ ] **Step 3:** MANIFEST accounting updated (counts + per-file citations);
  gate green (report the new count); workspace green.
- [ ] **Step 4: Commit**
  `rust(sema): S3 corpus — promotion battery + the three unblocked test/Sema rows`.

---

### Task 3: Upstream re-probe

**Files:**
- Modify: `rust/crates/sema/tests/sema_corpus/MANIFEST.md` (+ any imports)

**Interfaces:**
- Consumes: S2-T8's probe method + its recorded buckets (MANIFEST "upstream
  sweep" section: 1416 files → 1203 identical / 190 mismatched / 23 panicked).
- Produces: refreshed bucket accounting; any newly-matching imports.

- [ ] **Step 1:** Re-run the S2-T8 sweep over the same 8 upstream dirs
  (`test/{Parser,IRGen,BCGen,Optimizer,hermes,AST,Driver,RA}` — 1416 files;
  hermesc `-dump-sema` vs `rust/target/debug/sema-dump`, compare
  stdout+stderr+exit raw). Panics attributable to the S3 asserts must now be
  ZERO; classify what remains (S4 module kinds, recursion overflow, parser
  diagnostic geometry — the known buckets). Update the MANIFEST bucket
  arithmetic EXACTLY (S2-T8's fix round was needed precisely because these
  numbers drifted — count carefully, show the arithmetic).
- [ ] **Step 2:** Any upstream `test/Sema` file that newly matches → import
  with a MANIFEST row. Files that newly PANIC on something S3 should handle →
  fix (TDD, smallest repro) before closing the task.
- [ ] **Step 3:** Gate + workspace green. **Step 4: Commit**
  `rust(sema): S3 upstream re-probe — sweep buckets refreshed`.

---

### Task 4: Docs

**Files:**
- Modify: `doc/superpowers/RustPortRoadmap.md` (Sema row: S3 done — what
  shipped, final corpus count, the S4/S5 remainder list updated: S4 = modules
  + export-default rewrite #4 + `$SHBuiltin` module branches +
  `FunctionInfo::imports` backref + `resolve_ast_for_parser` + typed
  dialects/flow corpus (178 files) + per-file-flag harness; S5 = lazy/eval
  entries + the third promotion call site `runInScope` cpp:158),
  `doc/superpowers/specs/2026-07-26-sema-untyped-design.md` (§6: S3 done),
  `doc/superpowers/SESSION-HANDOFF.md` (S3 Update line matching the S0/S1/S2
  format + NEXT pointer → S4, noting NO S4 plan exists yet — brainstorm then
  write just-in-time).
- [ ] Edits (each file keeps its own voice; roadmap is source of truth; verify
  every number/citation you write — S2-T9's reviewer re-derived all of them);
  run the gate once + full workspace; commit
  `doc(rust): Sema S3 ScopedFunctionPromoter complete`.

---

## Self-review notes (plan-writing time)

- **Coverage vs the S3 remainder list** (roadmap Sema row): the promoter
  (T1), `promotedFunctionDecls_` production (T1), both assert seams (T1),
  the three blocked corpus rows (T2), the S1-T5 dormant matrix row (T2),
  upstream panic retirement (T3), docs (T4). The third call site
  (`runInScope`) explicitly deferred to S5 with a code note (T1) and a
  roadmap line (T4).
- **Read-first obligations flagged**: the whole 328-line .cpp (T1 Step 1);
  the dead-`newDecls` verification (T1); the exact shapes for the
  cpp:365-374 and cpp:2569-2577 rows (T2 — derived from the C++, not this
  plan's sketches).
- **Dump-blindness**: promotion is dump-visible (primary gate = differential),
  but the side-table (`get_promoted_decl`) and map contents are not — unit
  tests mandated in T1 Step 2.
- **Sequencing**: T1 before T2 (corpus needs the pass); T2 before T3 (the
  battery may absorb shapes the re-probe would otherwise surface); T4 last.
- **Landmine most likely to bite**: a promotion shape whose diagnostics
  collide at one source location (the unstable-sort tie) — if T2 hits one,
  the MANIFEST's `invalid-args-eval.js` row is the precedent: extract the
  loop-specific rows into a verified file and document, don't curate.
