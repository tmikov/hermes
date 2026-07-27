# Sema (untyped) — design

**Date:** 2026-07-26. **Component:** semantic resolution (scope resolution) for the Rust
front-end port. **Branch:** `rust`.

## 1. Goal and scope

Port the **untyped semantic-resolution path** of Hermes Sema to Rust, byte-for-byte
validated against `hermesc -dump-sema`.

**In scope** (~6.4k lines of C++):

| C++ | Rust | Notes |
|-----|------|-------|
| `lib/Sema/SemanticResolver.{h,cpp}` (723+3213) | `sema/src/resolver/` (module dir, split like the parser's `js/`) | the core walk |
| `include/hermes/Sema/SemContext.h` + `lib/Sema/SemContext.cpp` (758+573) | `sema/src/sem_context.rs` | tables, `Keywords`, dumper |
| `lib/Sema/DeclCollector.{h,cpp}` (185+202) | `sema/src/decl_collector.rs` | |
| `lib/Sema/ScopedFunctionPromoter.{h,cpp}` (37+328) | `sema/src/scoped_function_promoter.rs` | |
| `lib/Sema/CheckImplicitReturn.cpp` (335) | `sema/src/check_implicit_return.rs` | **untyped**: called at `SemanticResolver.cpp:1943` |
| `include/hermes/Sema/SemResolve.h` + `lib/Sema/SemResolve.cpp` (121+309) | `sema/src/resolve.rs` + `sema/src/dump.rs` | entry points, `semDump`/`ASTPrinter` |
| driver known-globals pre-registration | part of the `sema-dump` bin / a `sema` helper | visible in the dump (`UndeclaredGlobalProperty` list); exact C++ site located at plan time |

**Out of scope — the next component (typed path, reached only under `-typed`):**
`FlowChecker*`, `FlowContext`, `FlowTypesDumper`, `ASTEval`, `ASTLowering`, `ESTreeClone`.

## 2. Crate layout

New workspace member `rust/crates/sema/`:

- **Dependencies:** `ast`, `support`, `atom_table`. **NOT `parser`** — like C++ Sema, the
  library consumes only the AST.
- The `sema-dump` differential bin needs the parser, so `parser` is an **optional
  dependency** and the bin is gated with `required-features` — the published `sema`
  library keeps clean layering.

## 3. Architecture decisions (settled during brainstorming)

### 3.1 `NodeId` — unique per-node 32-bit id

Add `NodeId(u32)` to `ast::NodeMetadata`, assigned from a per-`Context` monotonic
counter at allocation (panic on wrap). Rationale (decided over `NodeRc`-keyed and
raw-address-keyed maps):

- **No aliasing:** a reused arena slot gets a fresh id, so a stale table entry can
  never match a live node (raw addresses can alias after GC + slot reuse).
- **No pinning:** unlike `NodeRc` keys, ids don't root nodes, so detached subtrees
  collect normally and stale entries are harmless (they just linger).
- **Reusable-parser principle:** the AST gains **no further annotation fields, ever**.
  Sema decorations (below) are grandfathered because Sema is part of the parser
  component family (the C++ declares them in `ESTree.h` itself). Everything beyond
  sema — FlowChecker's node→Type map (C++ already a `DenseMap<Node*, Type*>`), any
  future consumer — uses `HashMap<NodeId, T>` side tables.
- **Identity semantics under rebuild:** a functionally rebuilt node is a *new* node
  with a *new* id. `Cell` decorations are copied by the generated builders; id-keyed
  table entries deliberately do not follow. Consumers annotate after transforms or
  re-key on rebuild.
- Cost: ≤4 bytes/node (`NodeMetadata` is 25 bytes pre-padding). No machinery beyond
  the plain counter (no generations, no reverse lookup) — YAGNI.

**Freed-id log (drain model).** So subsystems can free data keyed by dead ids, both
node-freeing paths — the GC sweep and `AllocationScope::truncate` — append the
`NodeId` of each freed node to a `Vec<NodeId>` on the `Context`. No callbacks, no
registration: the driver that owns both the context and the side tables drains it
(`ctx.take_freed_ids()`) and hands it to each subsystem (`sem_ctx.prune(&dead)`).
`Context` never learns who its consumers are; cost with no consumer is one
empty-Vec check per collection. Notes:
- The log is purely a memory optimization — safety comes from id **non-reuse**
  (a missed prune is a lingering entry, never corruption).
- Discipline (documented on `NodeId`): subsystems insert entries only with the node
  in hand under `GCLock` — inserting by a stored id after a collection could key
  data to an already-reported dead id, leaking that entry permanently.
- Single-consumer `take` semantics for now; grows into a cursor-based log if
  multiple independent consumers ever appear.

### 3.2 Decorations live in the nodes (already generated)

The AST generator already ported the C++ `ESTree.h` decoration set 1:1 as `Cell`
fields: `scope`/`sem_info`/`decl` as `Cell<Option<SemaId>>` (22 fields), plus
`decl_state: Cell<u8>`, `unresolvable: Cell<bool>` on `Identifier`, and
`label_index: Cell<u32>` (9 nodes, `~0u` invalid sentinel). Function-likes carry two
ids (`scope` + `sem_info`); `For*`/`Switch` carry `label_index` + `scope`.

- `SemaId` stays a **single opaque u32 newtype in `ast`** (crate stays sema-agnostic).
  The `sema` crate defines typed ids (`DeclId`, `ScopeId`, `FunctionInfoId`) and
  converts at its accessor boundary, so cross-table mixups don't compile where it
  matters.
- The C++ packed `decl_` + `declState_` scheme (BitHaveDecl/BitHaveExpr; one slot for
  the common decl==expr case) is replicated bit-for-bit; the rare
  declaration-decl≠expression-decl case spills to the side map, exactly as
  `SemContext.h:552-562` does.

### 3.3 `SemContext`

- Id-indexed vectors for `FunctionInfo` / `LexicalScope` / `Decl` (juno's shape;
  C++ uses deques + pointers — pointer→index is the established port adaptation).
- The two C++ side maps `sideIdentifierDeclarationDecl_` and `promotedFunctionDecls_`
  (`SemContext.h:683-691`) become `HashMap<NodeId, DeclId>`.
- Node **backrefs** inside sema records (`LexicalScope::hoistedFunctions`,
  `FunctionInfo::imports` — plan-time audit confirmed `FunctionInfo` itself has no
  node pointer) are `NodeRc` — they are references handed to consumers (IRGen, the
  dumper), not keys; pinning attached tree nodes is free.
- `sema::Keywords` and the C++ decoration accessors (`getSemInfo`, `getScope`,
  `getDecl`, `getDeclarationDecl`/`getExpressionDecl`) become `SemContext` methods.
- `SemContextDumper` ports with its first-print-order `PtrNumbering`
  (`SemContext.cpp:565`) so `%s.N`/`%d.N` numbering matches byte-for-byte.

### 3.4 Resolver = transforming visitor (`VisitorMut`)

The C++ resolver mutates the AST in place at exactly four sites (audit for more is a
plan task):

1. arrow expression-body → block+return (`SemanticResolver.cpp:253`, `compile_ && _expression`)
2. try/catch/finally → nested try (`:794`, `compile_ && handler && finalizer`)
3. `$SHBuiltin.x` member object → `SHBuiltinNode` (`:1160`, **resolution-dependent**:
   fires only if `$SHBuiltin` resolves to `UndeclaredGlobalProperty`)
4. anonymous `export default function` → `FunctionExpression` (`:1527`)

Children stay plain immutable references (deep `match` ergonomics preserved; **no
`Cell` children**). Instead the resolver runs as a `VisitorMut` functional transform:

- Each rewrite happens at its exact C++ visit point, *before* descending, building the
  new subtree and then annotating/visiting it; the visit returns `Changed`.
- Spine rebuilds happen bottom-up on unwind, so the resolver's transient state (scope
  stack, current function, `DeclCollector` lookups) only ever references nodes it is
  currently inside. Unchanged children are reused by reference — only the spine above
  a change point gets new identity.
- Annotations survive rebuilds because they live in `Cell`s and the generated builders
  copy `Cell` values (verified in AST phase 3).
- `run()` returns the (possibly new) root — a signature adaptation from C++'s
  mutate-in-place; `SemContext` and callers use the returned root.

**Plan-time audit obligations for this scheme:**
(a) backref fixup — when a node with `sem_info`/`scope` set is on a rebuilt spine,
patch any sema-record `NodeRc` that referenced it (`LexicalScope::hoistedFunctions`,
`FunctionInfo::imports` entries); enumerate every node-pointer field in the C++
records.
(b) verify the C++ resolver never writes a node decoration *after* visiting that
node's children (a post-order write to the old node would be lost); restructure if
any site exists.
(c) grep the full C++ range for additional mutation sites beyond the four above.

### 3.5 Established port conventions that apply unchanged

C++ RAII (`ScopeRAII`, `FunctionContext`, `SaveAndRestore`) → explicit save/restore
or Drop-guards surviving `?`; C++ templates stay generics; C++ default arguments are
spec (read the headers); `check`/`checkUnescaped` distinction; diagnostics
byte-compatible via `support::SourceErrorManager`.

## 4. Entry points — full public surface

All of `SemResolve.h`, per the implement-completely rule:

| C++ | Rust | Gate |
|-----|------|------|
| `resolveAST` (+ ambient decls) | `resolve_ast` | primary differential |
| `resolveASTForParser` (compile=false, no transforms) | `resolve_ast_for_parser` | unit tests; fallback: C++ `tools/sema-dump/` oracle (decided at plan time) |
| `resolveASTLazy` | `resolve_ast_lazy` | lazy sweep + Rust-only equivalence oracle |
| `resolveASTInScope` (eval) | `resolve_ast_in_scope` | unit tests |
| `resolveCommonJSAST` | `resolve_common_js_ast` | `-commonjs` differential sweep |
| `semDump` | `sem_dump` | is itself the oracle surface |

## 5. Validation

1. **Primary — `sema_differential`** (`REQUIRE_DIFFERENTIAL=1 cargo test -p sema
   --features dump-bin --test sema_differential`): Rust `sema-dump` bin vs `hermesc -dump-sema`,
   byte-for-byte on **stdout + stderr + exit status**. Comparing stderr makes
   error/warning files first-class corpus members (sema diagnostics flow through the
   byte-compatible `SourceErrorManager`). Corpora:
   - the existing 76-file plain parser corpus;
   - the Flow/TS/JSX corpora under their dialect flags (untyped mode — empirically
     verified `-parse-flow -dump-sema` works);
   - the 56 `test/Sema/*.js` lit files imported as a `sema_corpus/` seed;
   - new files for under-covered areas: scoped-function promotion loose/strict,
     `$SHBuiltin` shadowing, private class names, `with`, generators/async arrows +
     `arguments` capture, label edge cases;
   - module-syntax files additionally swept under `-commonjs`.
2. **Lazy:** `hermesc -lazy -dump-sema` differential sweep (exact driver semantics
   confirmed at plan time), plus a Rust-only oracle in the parser's Oracle-A style:
   `parse_lazy_function` + `resolve_ast_lazy`, final sem dump must equal the eager
   run's.
3. **Unit tests** for CLI-less surfaces (`resolve_ast_in_scope`,
   `resolve_ast_for_parser`). If too weak for compile=false, add a small C++
   `tools/sema-dump/` calling `resolveASTForParser` + `semDump` (the
   js-lexer-dump/preparse-dump precedent).

There is no C++ Sema gtest suite to port.

## 6. Phasing

Plans written just-in-time, executed subagent-driven, two-stage review per phase +
whole-component capstone (including the structural-fidelity template grep). Phase
boundaries may shift when plans are written against actual C++ line ranges; the gate
contents are the commitment, not the exact split.

- **S0 — foundations (DONE, 2026-07-26):** `NodeId` in `ast` (+ `gen_nodes.py`) + the freed-id log
  (sweep + `AllocationScope::truncate`, §3.1); `sema` crate scaffold;
  `SemContext` + `Keywords` (133 atoms) + known-globals + `SemContextDumper`; `sema-dump` bin +
  differential harness green (6-file corpus, stdout+stderr+exit status). See the roadmap's Sema
  row for the full what-shipped and the gate command.
- **S1 — declarations & scopes:** `DeclCollector`; scope creation; var/let/const/
  function hoisting; parameter scopes; identifier-resolution core.
- **S2 — rest of the walk:** labels/break/continue; catch; classes + private names;
  the four rewrites; eval/`arguments`/`with`; strict-mode checks;
  `mayReachImplicitReturn`.
- **S3 — `ScopedFunctionPromoter`** + loose-mode promotion + `promotedFunctionDecls_`.
- **S4 — modules & flavors:** CommonJS wrapping; ambient decls;
  `resolve_ast_for_parser`; dialect corpora green.
- **S5 — lazy + eval entry points; capstone.**

## 7. Deliberate deviations (to record in the roadmap on completion)

1. `NodeId(u32)` in `NodeMetadata` — infrastructure the C++ lacks; justified by the
   reusable-parser annotation model (§3.1).
2. Resolver returns a new root (functional transform) instead of mutating in place
   (§3.4) — behaviorally identical tree + tables, gated by the differential.
3. Pointer→index adaptations: sema records referenced by typed u32 ids; the two C++
   `DenseMap<IdentifierNode*, Decl*>` side maps keyed by `NodeId`.

All other structure follows the C++ file-for-file.
