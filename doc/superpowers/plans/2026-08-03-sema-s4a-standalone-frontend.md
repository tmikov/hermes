# Sema S4a (Standalone-Front-End Sema) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.
> **Spec:** `doc/superpowers/specs/2026-08-03-sema-s4a-design.md` — read it first;
> this plan implements it exactly. Written in a context-rich session (facts below
> verified against the C++ and both binaries on 2026-08-02/03).

**Goal:** Make the resolver complete and byte-verified for standalone-front-end
input — module-bearing and untyped-flow files — and port `resolve_ast_for_parser`
with its own oracle, plus the `// FLAGS:` per-file-flag harness.

**Architecture:** Two new validation surfaces (a `sema-parser-dump` C++/Rust oracle
pair for the `compile = false` entry; first-line `// FLAGS:` parsing in the existing
differential), then the four module visits in a new `resolver/modules.rs` (rewrite #4
inline, per the spec §4 ruling), then corpus batteries. The untyped `-parse-flow`
diagnostics are ALREADY PORTED (verified: `expressions.rs:966` CoverTypedIdentifier
"typecast not allowed in this context"; `functions.rs:897` "'this' parameter requires
typed mode") — T4 is corpus work, not porting.

**Tech Stack:** as S0–S3. C++ sources of truth: `lib/Sema/SemanticResolver.cpp`
(cpp:874-890, 1510-1554), `lib/Sema/SemResolve.cpp` (:295-306),
`include/hermes/Sema/SemResolve.h` (:111 `semDump`),
`lib/CompilerDriver/CompilerDriver.cpp` (:969-974 the `-dump-sema` call),
`tools/hermes-parser/hermes-parser-wasm.cpp` (:104 the `resolveASTForParser` call).

## Global Constraints

- NEVER `cd`; `--manifest-path rust/Cargo.toml` / absolute paths.
- Zero warnings with AND without `--features dump-bin`; no new clippy lints;
  `#![forbid(unsafe_code)]` in sema; 80-col new lines; copyright headers; every C++
  citation verified with grep/awk before writing (reviewers re-derive them).
- Faithful port: C++ comments carried over; bug-for-bug quirks preserved and
  flagged, never "fixed" (this plan names two: the ExportAll message wording and the
  rewrite's `/* async */ false`); exact diagnostic strings; C++ default args are
  spec.
- The decorate-before-recurse invariant (resolver/mod.rs module doc) binds
  node-`Cell` writes; rewrite #4 is a functional rebuild via the generated `builder`
  (S2 rewrite #1-#3 precedent), never an in-place `Cell` write.
- Driver gate (grows from **173 files / 97 hermesc-success**):
  `REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p sema --features dump-bin --test sema_differential -- --nocapture`
  (oracle `cmake-build-asan/bin/hermesc`). EVERY new corpus file verified against the
  C++ side FIRST — **with the same flags the FLAGS line carries** — raw
  stdout+stderr+exit. Fix the Rust side; never curate away a fixable mismatch.
- FLAGS-bearing imports of upstream files cannot be byte-identical to upstream (the
  added first line); their MANIFEST rows say "upstream + FLAGS line" explicitly.
- Landmines: `tests/sema_corpus/MANIFEST.md` documents same-location diagnostic-order
  ties and THREE hermesc self-aborts — check it before fighting a mismatch.
- ONLY the four module-visit arms — plus any do-nothing or diagnostic-only visits
  the untyped `-parse-flow` paths require (spec §3.4: "whatever their surrounding
  visits need to exist"; e.g. the `visit(TypeAliasNode*)` do-nothing at
  cpp:1579-1581, needed by `type-alias-children.js`) — replace catch-all panics in
  this phase. The `$SHBuiltin` module branches (`resolver/calls.rs:310-341`) keep
  their loud S4-tagged panics — they are **S4b**. `-commonjs` is NOT implemented
  anywhere.
- TDD per task; full workspace suite before each commit; commits
  `rust(sema): <what>` + trailer
  `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.

---

### Task 1: The `// FLAGS:` harness + sema-dump flag growth

**Files:**
- Modify: `rust/crates/sema/tests/sema_differential.rs` (the per-file loop inside
  `run_differential(corpus, hermesc_extra, sema_dump_extra)` — the function already
  takes per-corpus extras; FLAGS adds per-FILE extras)
- Modify: `rust/crates/sema/src/bin/sema_dump.rs` (Options struct at :92; add
  `enable_eval: Opt<bool>` default TRUE and `fstd_globals: Opt<bool>` default TRUE,
  hermesc spellings `-enable-eval` / `-fstd-globals` with `-fno-std-globals` the
  false form — model on the existing `parse_flow` flag at :96/:125 and
  `ferror-limit` at :180-188; wire `enable_eval` into `ast::Context::enable_eval`
  (S2-T6's field) and gate the ambient-decls load at :282 on `fstd_globals`)
- Test: 3 new corpus files + `type-alias-children.js` import; MANIFEST rows

**Interfaces:**
- Consumes: `run_differential` (sema_differential.rs:~70); hermesc flags verified
  live 2026-08-03: `-parse-flow`, `-enable-eval` (bool, `=false` works),
  `-fstd-globals`/`-fno-std-globals` (verified: `-fno-std-globals -dump-sema` dumps
  without the 63 ambient decls); the `eval() is disabled at runtime` diagnostic is
  `SemanticResolver.cpp:1147` (reached when `!enableEval`; the Rust site is in
  `resolver/calls.rs` gated on `ast::Context::enable_eval` — verify with grep).
- Produces: per-file FLAGS parsing — first line exactly `// FLAGS: <args>` →
  whitespace-split, appended to BOTH binaries' argv after the per-corpus extras.
  Every later task's flag-bearing corpus files depend on this.

- [ ] **Step 1: Write the failing test.** Add to `sema_differential.rs` a helper:

```rust
/// If the file's first line is `// FLAGS: <args>`, return the args.
fn per_file_flags(src: &[u8]) -> Vec<String> {
    let first = src.split(|&b| b == b'\n').next().unwrap_or(&[]);
    let Ok(first) = std::str::from_utf8(first) else { return vec![] };
    match first.strip_prefix("// FLAGS: ") {
        Some(rest) => rest.split_whitespace().map(String::from).collect(),
        None => vec![],
    }
}
```

  and call it in the per-file loop, appending to both commands. Then add the corpus
  file `flags-enable-eval-off.js` (first line `// FLAGS: -enable-eval=false`, body
  `eval("1");` — hermesc-verify FIRST:
  `hermesc -enable-eval=false -dump-sema <file>` must show the cpp:1147 diagnostic;
  record stdout+stderr+exit). Run the gate: the new file must FAIL (sema-dump has no
  `-enable-eval` option yet → three-channel mismatch). That is the RED state.
- [ ] **Step 2: Implement** the two `sema_dump.rs` options per the Files block; wire
  them. Gate green.
- [ ] **Step 3:** Add `flags-no-std-globals.js` (`// FLAGS: -fno-std-globals`, body
  `var x; print;` — pins the ambient-decl absence AND that `print` still resolves
  UndeclaredGlobalProperty) and import `test/Sema/type-alias-children.js` with a
  prepended `// FLAGS: -parse-flow` line (hermesc-verify with the flag first; its
  deferred MANIFEST row moves Deferred → Imported, deferred table 5 → 4, noting the
  added line). Gate green with all files.
- [ ] **Step 4:** MANIFEST: new "FLAGS convention" paragraph (first-line-only, both
  binaries, spellings are hermesc's) + rows + arithmetic (173 → 176).
- [ ] **Step 5:** Full workspace suite; both-config zero-warning check. **Commit**
  `rust(sema): S4a T1 — // FLAGS: per-file harness + -enable-eval/-fstd-globals`.

---

### Task 2: The `sema-parser-dump` oracle pair + `resolve_ast_for_parser`

**Files:**
- Create: `tools/sema-parser-dump/sema-parser-dump.cpp`,
  `tools/sema-parser-dump/CMakeLists.txt` (register via `add_hermes_tool`; model
  BOTH files on `tools/preparse-dump/` — the closest precedent, a parser+sema-layer
  tool)
- Modify: `tools/CMakeLists.txt` (add_subdirectory, alphabetical position)
- Modify: `rust/crates/sema/src/resolve.rs` (add `resolve_ast_for_parser` beside
  `resolve_ast` at :43)
- Modify: `rust/crates/sema/src/bin/sema_dump.rs` (add `parser_entry: Opt<bool>`,
  long `--parser-entry`)
- Create: `rust/crates/sema/tests/sema_corpus_parser/` (seed: `plain.js`,
  `compile-false-basics.js`) + a `sema_parser_differential` test fn in
  `sema_differential.rs` reusing `run_differential`'s body with the tool pair
  (factor a binary-pair parameter; do NOT copy the function)

**Interfaces:**
- Consumes: C++ `resolveASTForParser` (`SemResolve.cpp:295-306`: constructs
  `SemanticResolver{astContext, semCtx, nullptr, nullptr, /*compile*/ false}` and
  runs it — the exact call `hermes-parser-wasm.cpp:104` makes);
  `sema::semDump(llvh::outs(), *context, semCtx, flowContext, root)`
  (`SemResolve.h:111`, called by the driver at `CompilerDriver.cpp:969-974`);
  Rust `resolve_ast` (resolve.rs:43) for the signature pattern.
- Produces: `pub fn resolve_ast_for_parser<'ast>(...)` — same signature as
  `resolve_ast` MINUS the ambient-decls parameter, passing `compile = false`
  (thread the existing internal flag; S0 ported `compile_` onto the resolver —
  verify its Rust name with grep and use it); the C++ tool: parse (honoring a
  `-parse-flow` option), call `resolveASTForParser`, then `semDump(outs, ...,
  /*flowContext*/ nullptr, root)` **unconditionally**, diagnostics to stderr, exit
  2 iff `sm.getErrorCount() != 0` else 0; `sema-dump --parser-entry` mirrors this
  exactly (dump even when diagnostics were emitted — the driver-path behavior of
  suppressing the dump on error does NOT apply here).

- [ ] **Step 1 (RED):** Write `sema_parser_differential` + the two seed files.
  `plain.js`: `var x = 1 + 2; print(x);` — pins no-ambient-globals (print is
  Undeclared… only if std-globals still register? NO: `compile = false` passes
  `ambientDecls = nullptr`, so the dump's scope %s.1 contains ONLY `x`; `print`
  resolves UndeclaredGlobalProperty) AND no `+` folding (cpp:405-436 is
  `compile_`-gated — the dump shows the unfolded tree via ASTPrinter).
  `compile-false-basics.js`: body `export default function f(){}` — pins that NO
  module-mode error is emitted under `compile = false` (export gate is
  `compile_ &&`, cpp:1511) — note this file FAILS until T3 lands the visits
  (the Rust side panics at the catch-all); park it in a `sema_corpus_parser/`
  subdirectory `pending/` excluded from the walk, with a T3 step to move it in.
  Build the C++ tool; run the Rust test: FAILS (no `--parser-entry`, no
  `resolve_ast_for_parser`). Record RED.
- [ ] **Step 2:** Implement `resolve_ast_for_parser`, `--parser-entry`, the C++
  tool. `plain.js` matches byte-for-byte (three channels).
- [ ] **Step 3:** Build-doc: add the tool to the gate preamble in
  `sema_differential.rs`'s module doc
  (`cmake --build cmake-build-asan --target sema-parser-dump`).
- [ ] **Step 4:** MANIFEST section for the parser-entry corpus (its own file list +
  counts). Full workspace; zero warnings. **Commit**
  `rust(sema): S4a T2 — sema-parser-dump oracle pair + resolve_ast_for_parser (SemResolve.cpp:295)`.

---

### Task 3: The module visits (`resolver/modules.rs`) + imports backref + corpus

**Files:**
- Create: `rust/crates/sema/src/resolver/modules.rs`
- Modify: `rust/crates/sema/src/resolver/mod.rs` (add `mod modules;`; add FOUR match
  arms — `Node::ImportDeclaration(..) | Node::ExportNamedDeclaration(..) |
  Node::ExportDefaultDeclaration(..) | Node::ExportAllDeclaration(..)` dispatching
  to the new visit methods — immediately BEFORE the catch-all panic at
  mod.rs:1324-1327)
- Modify: `rust/crates/sema/src/resolver/functions.rs` (the imports backref fixup
  beside the S1-T7 `hoisted_functions` fixup — find it with
  `grep -n hoisted_functions functions.rs` and mirror the mechanism)
- Test: `rust/crates/sema/tests/resolver.rs` (imports-list unit tests — the list is
  dump-blind EVERYWHERE, verified: zero `imports` hits in `SemContextDumper.cpp`);
  corpus imports + parser-entry module files

**Interfaces:**
- Consumes: T2's parser-entry differential (module decls are only dump-visible
  there); `extract_declared_idents_from_id`'s existing `ImportDeclaration` arm
  (declarations.rs:285 — S1 ported it; T3 makes it corpus-reachable; verify it maps
  specifier locals to `DeclKind::Import` per the C++ and fix faithfully if the arm
  was never exercised); the generated `builder` for the rewrite.
- Produces: `visit_import_declaration`, `visit_export_named_declaration`,
  `visit_export_default_declaration`, `visit_export_all_declaration` on
  `SemanticResolver` (in modules.rs); `FunctionInfo::imports: Vec<NodeRc>`
  populated + fixed up post-rebuild.

- [ ] **Step 1: Read** cpp:874-890 and cpp:1510-1554 in full. The port:
  - `visit(ImportDeclarationNode)` cpp:874-890: module-mode error
    `'import' statement requires module mode` — **unconditional** (cpp:876-879, NOT
    `compile_`-gated; bug-for-bug asymmetry vs the exports); import-assertions
    error `import assertions are not supported` gated
    `compile_ && !_attributes.empty()` (cpp:881-885); push onto
    `cur_function_info().imports` (cpp:887); children.
  - `visit(ExportNamedDeclarationNode)` cpp:1510-1517:
    `'export' statement requires module mode` gated `compile_ && !useCJSModules` —
    with no CJS support, the Rust gate is just `compile_` (leave a
    `// S4b: && !use_cjs_modules` note); children.
  - `visit(ExportDefaultDeclarationNode)` cpp:1519-1547: same gate/message; then
    **rewrite #4 inline** (cpp:1525-1544): if `compile_` and the declaration is a
    `FunctionDeclaration` with `_id == None` → rebuild the child as a
    `FunctionExpression` carrying `_id/_params/_body/_typeParameters/_returnType/
    _predicate/_generator` and **`/* async */ false` verbatim (cpp:1538 — a C++
    quirk: an anonymous `export default async function(){}` loses its async flag
    on the rewritten node; preserve and flag, never fix)**, copying `strictness`
    + location (cpp:1539-1540), then visit the REWRITTEN child (functional
    rebuild via `builder`, rewrite #1-#3 precedent; carry the C++ comment "change
    it to a FunctionExpression node for cleaner IRGen" + a
    `// S4b owns the -commonjs corpus pinning of this rewrite` note); children.
  - `visit(ExportAllDeclarationNode)` cpp:1549-1554: gate as Named but the message
    is **`'export' statement requires CommonJS module mode`** — different wording
    from Named/Default (bug-for-bug; preserve exactly); children.
- [ ] **Step 2 (RED):** hermesc-verify then add driver-corpus files
  `module-import-plain.js` (`import {a} from 'm';` → unconditional error, exit 2)
  and `module-export-plain.js` (one file exercising all three export forms → three
  `compile_`-gated errors incl. the ExportAll wording, exit 2). Both currently
  PANIC at mod.rs:1324 — record the panic text as RED. Unit test (RED):
  resolve a program with two imports via the library API, assert
  `FunctionInfo::imports.len() == 2` and that each entry is the (rebuilt)
  `ImportDeclaration` node — dump-blind, so this is the only pin.
- [ ] **Step 3:** Implement per Step 1; move T2's parked
  `pending/compile-false-basics.js` into the parser-entry corpus (no export error
  under `compile = false`); add parser-entry corpus `module-imports.js`
  (`import d, {a as b} from 'm'; import * as ns from 'n';` — hermesc-tool-verified;
  pins `Decl::Kind::Import` decls for `d`/`b`/`ns` in the dump). Both gates green.
- [ ] **Step 4:** Sweep-file imports: consult MANIFEST's S3-T3 sweep section for the
  17 panic files; for each NON-`$SHBuiltin` module file (expect ~9 — the ≤7
  protocol files and `computed-fn-name.js` stay), hermesc-verify then import with a
  MANIFEST row. Gate green; report the count.
- [ ] **Step 5:** MANIFEST arithmetic (T1's 176 + step-2/3/4 files — show it);
  full workspace; zero warnings both configs. **Commit**
  `rust(sema): S4a T3 — module visits + rewrite #4 inline + FunctionInfo::imports (cpp:874-890,1510-1554)`.

---

### Task 4: Untyped `-parse-flow` corpus battery

**Files:**
- Create: corpus files in `rust/crates/sema/tests/sema_corpus/` (driver gate, via
  `// FLAGS: -parse-flow`); possibly `sema_corpus_parser/` flow file
- Modify: MANIFEST

**Interfaces:**
- Consumes: T1's FLAGS harness; the ALREADY-PORTED sites — `expressions.rs:966`
  (`CoverTypedIdentifier` → `typecast not allowed in this context`, cpp:1576 under
  `#if HERMES_PARSE_FLOW`, ported unconditionally per the single-node-set
  precedent) and `functions.rs:897` (`'this' parameter requires typed mode`,
  cpp:1767-1771, gate `compile_ && !typed_`).
- Produces: corpus rows only (no code expected; any mismatch found IS a bug —
  fix faithfully, citing the C++).

- [ ] **Step 1:** hermesc-verify then add, each `// FLAGS: -parse-flow`, one concern
  per file: `flow-typecast-cover.js` (`(x: number);` — the CoverTypedIdentifier
  error shape; derive the exact triggering form from the C++ parser's cover
  grammar and hermesc, not this sketch); `flow-this-param.js`
  (`function f(this: number) {}` → the cpp:1771 error);
  `flow-annotations-benign.js` (`function f(x: number): number { return x; } var
  y: string;` — flow syntax resolving CLEAN untyped, exit 0, pins that
  annotations don't perturb decls/scopes).
- [ ] **Step 2:** Gate green; MANIFEST rows + arithmetic. Full workspace.
  **Commit** `rust(sema): S4a T4 — untyped -parse-flow corpus battery`.

---

### Task 5: Upstream re-probe

**Files:**
- Modify: `rust/crates/sema/tests/sema_corpus/MANIFEST.md` (+ any imports)

**Interfaces:**
- Consumes: the S3-T3 sweep method (MANIFEST records it; 1416 files over
  `test/{Parser,IRGen,BCGen,Optimizer,hermes,AST,Driver,RA}`; buckets
  1209/190/17).
- Produces: refreshed buckets. EXPECTED: panic 17 → the `$SHBuiltin`-protocol
  files (≤7, from the 2026-08-02 grep) + `computed-fn-name.js` (the C++-defect
  repro) — i.e. ≤8; every remaining panic must be S4b-tagged or the known defect.
  Zero S4a-attributable panics.

- [ ] **Step 1:** Re-run the sweep exactly as S3-T3 (build both binaries first;
  compare three channels raw). Classify; reconcile the bucket moves file-by-file
  against T3's imports (the moved set must equal T3 step-4's list plus any
  identical-but-not-imported module files — name them). Show the arithmetic.
- [ ] **Step 2:** Any upstream `test/Sema` row newly matching → import with a row
  (re-probe the remaining 4 Deferred rows; `xmod-errors.js` must still be blocked
  — S4b). Files newly panicking on something S4a should handle → fix (TDD,
  smallest repro) before closing.
- [ ] **Step 3:** Both gates + full workspace green. **Commit**
  `rust(sema): S4a T5 — upstream re-probe (module panics retired)`.

---

### Task 6: Docs

**Files:**
- Modify: `doc/superpowers/RustPortRoadmap.md` (Sema row: S4a DONE bullet in the
  S0-S3 style — what shipped, both gate counts, sweep buckets; AMEND the S4b
  bullet: rewrite #4's CODE landed in S4a per the spec §4 ruling — S4b keeps its
  corpus pinning + `-commonjs` + CJS wrapping + `$SHBuiltin`; the "Next:" line →
  S5 (lazy/`eval` + `runInScope` + capstone), noting NO S5 plan exists yet —
  brainstorm first), `doc/superpowers/specs/2026-07-26-sema-untyped-design.md`
  (§6: S4a DONE line in the S0-S3 format),
  `doc/superpowers/specs/2026-08-03-sema-s4a-design.md` (a one-line executed
  header note), `doc/superpowers/SESSION-HANDOFF.md` (S4a Update paragraph in the
  established format + NEXT → S5).
- [ ] Verify every number/citation written (reviewers re-derive them — the S2-T9 /
  S3-T4 precedent); run both gates + full workspace once; commit
  `doc(rust): Sema S4a standalone-front-end sema complete`.

---

## Self-review notes (plan-writing time)

- **Spec coverage:** §2.1 oracle pair → T2; §2.2 FLAGS → T1; §3.1 visits → T3;
  §3.2 Import decls → T3 steps 3; §3.3 imports backref → T3 (dump-blind confirmed,
  unit tests); §3.4 flow paths → T4 (discovered already ported — corpus only);
  §4 rewrite ruling → T3 step 1 (+ the async quirk found at plan time); §5 corpus
  → T1/T3/T4; §6 tasks/gates → all; §7 deferrals → Global Constraints + T5/T6.
- **Sequencing:** T1 independent; T2 before T3 (module dump-visibility needs the
  tool); T3's parked `pending/` file resolves the T2↔T3 circularity honestly;
  T4 after T1; T5 after T3/T4; T6 last.
- **Two bug-for-bug quirks named for reviewers:** the ExportAll "CommonJS module
  mode" wording; the rewrite's `/* async */ false` (cpp:1538).
- **Numbers at plan time:** driver gate 173/97; deferred rows 5 (→4 in T1);
  sweep 1209/190/17 (→ panic ≤8 in T5). Exact end-state counts derived during
  execution, never invented.
