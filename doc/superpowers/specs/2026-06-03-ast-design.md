# Hermes ESTree AST → Rust — Design

Port of Hermes' AST (`include/hermes/AST/ESTree.h` + `ESTree.def`, the `Decoration` classes,
`lib/AST/ESTree.cpp`, `RecursiveVisitor.h`, and `ESTreeJSONDumper.{h,cpp}`) to Rust. The AST is
the schema that the Parser produces and that Sema / IRGen / the AST transform passes consume, so it
is designed as its own component, independent of the Parser that will fill it.

> **Status:** design approved 2026-06-03; **IMPLEMENTED 2026-06-05 (all 4 phases complete)** —
> the AST component is done (GC spine + 271-node generated set + transforming visitor +
> `ESTreeJSONDumper` with golden tests). The byte-for-byte `-dump-ast` differential lands as the
> **Parser's** gate (the AST has no producer until the Parser). Base branch `static_h`, work on `rust`.
> **Reading order context:** `doc/superpowers/RustPortRoadmap.md` (roadmap), this spec, then the
> implementation plan(s) under `doc/superpowers/plans/`.

## Goal & scope

Faithfully port the AST **as a library**: the node types, their storage/lifetime model, traversal,
construction, and JSON dumping — everything needed for the Parser to build trees and for later
passes to read and rewrite them.

In scope (the whole AST component):
- The **node set** — every node in `ESTree.def` (core JS + Flow + JSX + TS; all `#if` families on,
  matching a full-featured `hermesc`), plus the per-node **decoration** fields from the `Decoration`
  classes in `ESTree.h`.
- **Storage**: a garbage-collected arena (`Context` + `GCLock` + `NodeRc`), copied from juno's
  proven machinery and adapted to our `support`/`atom_table` crates.
- **`NodeList`** — the ordered child-list type.
- **Traversal**: the read-only `Visitor` (port of `RecursiveVisitor`) and the transforming
  `VisitorMut` (functional rebuild) from juno.
- **Builders** — construction of nodes, and clone-with-one-field-changed (the rebuild primitive).
- **`ESTreeJSONDumper`** — the JSON serializer, which is also the differential-oracle surface.

Out of scope (separate later components; we only *declare* what the AST must hold for them):
- The **Parser** — the producer. The AST ships before it; the byte-for-byte `-dump-ast` differential
  becomes the Parser's gate (§4).
- **Sema** (`SemanticResolver`/`FlowChecker`/scope resolution) and the AST **transform passes**
  (`StripTS`, `TS2Flow`, `TransformDecorate`, `AsyncGenerator`) — consumers. The AST declares the
  decoration *fields* they need (as `Cell<…>`); it does not implement them. The sema-side structures
  those fields reference (`Decl`, `LexicalScope`, `FunctionInfo`) are *not* AST nodes and are not
  ported here — their fields are typed as placeholder id handles until Sema lands.

## Background — how the C++ AST works

- Nodes are **bump-allocated** (`Context::allocateNode`), referenced by raw `Node*`, **mutated in
  place**, and **never freed** until the whole `Context` dies. The user characterized this as "a good
  compromise."
- Each node is `class NAMENode : public BASENode, public NAMEDecoration` — i.e. a base `Node`
  (kind tag, `SMRange`, `parens`, debug loc) + the `.def` argument fields (`_argName`) + a
  **decoration** mixed in by multiple inheritance. The decoration carries Hermes-specific side-data
  (lexical scope, `FunctionInfo`, resolved declaration, strictness, label index, lazy-reparse flags…).
- `ESTree.def` is the single source of truth for node shapes; `ESTree.h`, `RecursiveVisitor.h`,
  `ESTreeJSONDumper.cpp`, and IRGen are all macro-expanded / switch over it.
- The AST is mutated **after construction**, including child links of already-embedded nodes — the
  parser's cover-grammar reparse (`reparseAssignmentPattern`), and pervasively in `SemanticResolver`
  (`tryStatement->_handler = …`, `arrowFunc->_body = …`, `assign->_left = …`, `node->_kind = …`,
  `id->_name = …`) and the transform passes. This is the central constraint the Rust model must serve.

## The model (the spine)

We adopt **juno's GC arena**, with one deliberate change to juno: node *attributes* are mutable.

1. **Storage = juno's GC, copied.** `Context` (chunked node storage + free lists), `GCLock` (one
   active per thread; hands out `&'gc Node<'gc>`), `NodeRc` (refcounted root that can outlive a lock),
   and the mark-sweep collector are copied from `unsupported/juno/crates/juno_ast/src/context.rs`
   and adapted to our crates (§1). Its `unsafe` is the copied, encapsulated kind already sanctioned
   for `atom_table` — it does not leak past the storage module.

2. **`Node<'gc>` = a `#[repr(C)]` enum**, one variant per kind, each variant a `#[repr(C)]` struct
   whose first field is the common metadata (kind, `SMRange`, `parens`, debug loc). `repr(C)` keeps
   the common prefix at a fixed offset so identical match arms fold to pointer arithmetic. You
   **`match` on `&Node` directly**, including deeply nested single-child patterns — the ergonomic
   property the whole model exists to preserve.

3. **Child fields are immutable; everything else is `Cell`.** This is the rule that makes mutation
   safe without reintroducing `unsafe`, and it falls straight out of `ESTree.def`'s own type tags:

   | `.def` / decoration field | Rust type | Mutation |
   |---|---|---|
   | `NodePtr` (child) | `&'gc Node<'gc>` | immutable → **rebuild on change** |
   | `NodePtr` optional child | `Option<&'gc Node<'gc>>` | immutable → **rebuild on change** |
   | `NodeList` (child list) | `NodeList<'gc>` | immutable → **rebuild on change** |
   | `NodeBoolean` / `NodeNumber` | `Cell<bool>` / `Cell<f64>` | **in place** |
   | `NodeLabel` / `NodeString` | `Cell<NodeLabel>` / `Cell<NodeString>` | **in place** |
   | decoration scalar (parens, strictness, label idx, flags) | `Cell<…>` | **in place** |
   | decoration sema pointer (scope, `FunctionInfo`, decl) | `Cell<Option<…Id>>` | **in place** |

   - **Structural change → functional rebuild.** Changing a child (`tryStatement->_handler = X`,
     `arrowFunc->_body = X`) builds a *new* node via a Builder; in a recursive walk, each visit method
     returns the (maybe-new) child and the parent rebuilds — juno's `TransformResult` threading. The
     user's key observation: because the walks are recursive, converting the C++ in-place child edits
     into rebuilds is a *local* change at each visit method, not a rewrite. Orphaned nodes are
     reclaimed by the GC.
   - **Attribute change → in-place `Cell`.** Renames (`id->_name =`), operator/kind rewrites
     (`node->_operator =`, `vd->_kind =`), `parens`, strictness, label indices, and every sema
     decoration are plain `Cell` writes — no rebuild, no `unsafe`.

4. **No `Cell<&'gc Node>` anywhere (verified).** A full scan of `ESTree.h` confirms there are **no
   raw `Node*`/`NodePtr` side-fields**: every pointer-typed decoration member is a *sema* structure
   (`sema::LexicalScope*`, `sema::FunctionInfo*`, `Decl* decl_`) that lives outside the AST heap and
   becomes a `Cell<Option<Id>>`. The only AST-node-referencing side-data are **two `NodeList`s** —
   `FunctionLikeDecoration::decorations` (the `Hermes.decorate(...)` list, populated by
   `TransformDecorate`) and `ProgramDecoration::dummyParamList` (a dummy empty list). Both are the
   `NodeList` child type, not `&Node`, so a *mutable* list field is `Cell<NodeList>` (a `Copy`
   thin pointer — no lifetime-invariance problem), and they are reclaimed/traced like any child.
   This is what keeps us off the one rock juno hit with `unsafe` transmutes.

### Why this and not the alternatives

- **Not pure-functional-immutable (juno as-is):** Hermes attaches scalar decorations in place all over
  Sema; forcing those through rebuild would be churn for no benefit. So we keep juno's immutable
  *structure* but make *attributes* `Cell`.
- **Not index handles (`NodeId`):** examined and rejected on the merits. In a never-freed arena, raw
  references are already UB-free by lifetime, so indices buy **no** memory-safety here; against Rust
  references they are *weaker* on logical validity (a `NodeId` can be fabricated, staled, or
  arithmetic'd to the wrong node; a `&'gc Node` can only be obtained from a real allocation), and
  matching `as-fast-as-pointers` index access requires `get_unchecked` — i.e. `unsafe` — so the
  "no-unsafe" badge is just accounting. The real axis was *where borrow-checker friction lands*
  (references → on mutation, indices → on every read + no deep match); we choose references for the
  deep-`match` ergonomics and because they read close to the C++ `node->field`.
- **Why keep the GC:** the rebuild churn from Sema and the transform passes is exactly what a
  collector is for; juno's is already written and tested, so it is copied, not dead weight.

## Crate layout

A new **`rust/crates/ast/`** crate (parallel to juno's separate `juno_ast`). The `parser` crate will
depend on it.

- `src/context.rs` — copied juno `Context`/`GCLock`/`NodeRc`/GC + the chunked storage (`Deque`) and
  `NodeListElement`. Adapted to use our `support` source types and `atom_table` (see below). Houses the
  crate's encapsulated `unsafe`.
- `src/node.rs` (+ generated `nodes_generated.rs`) — the `Node<'gc>` enum, the per-kind structs, the
  `NodeKind`/`NodeVariant` enum, and the field/child classification. **Generated** (§2).
- `src/node_child.rs` — `NodeList`, `NodeMetadata`, the `NodeChild` trait, leaf field types.
- `src/visitor.rs` — `Visitor` (read) + `VisitorMut`/`TransformResult`/`Path` (transform), the ported
  `RecursiveVisitor` driver.
- `src/builder.rs` — per-kind Builders: construct + clone-with-change.
- `src/dump.rs` — the `ESTreeJSONDumper` port.
- `build` glue or a committed generator script (§2) that reads `include/hermes/AST/ESTree.def`.

**Reuse, don't re-copy:** locations come from our `support` crate (`SMLoc = (SourceId, u32)`,
`SMRange`); atoms come from our `atom_table` crate (`NodeLabel` = an `AtomTable` handle, `NodeString`
= the WTF-8 `AtomBytes` handle). Only juno's *storage* utilities (`Deque`, the GC) are copied; juno's
`source_manager`/`atom_table` are **not** — we already have ours. (`Deque` + `HeapSize` are copied from
`juno_support` into the `ast` crate or a small support util.)

## 1. Storage adaptation details

- `Context<'ast>` owns: the chunked `Deque<StorageEntry<Node>>` + node free list, the
  `Deque<NodeListElement>` + its free list, the `NodeRcCounter`, a reference/handle to the shared
  `AtomTable`, and AST-global flags (strict mode, etc.). Source management stays in `support`'s
  `SourceErrorManager`; the `Context` references it rather than owning a second copy.
- `GCLock` keeps juno's single-lock-per-thread invariant (panics on a second lock) and the
  `&'gc Node<'gc>` lifetime discipline.
- The GC marker walks children via the generated `visit_children`/`mark_lists`; **it must explicitly
  include the two decoration `NodeList`s** (`FunctionLikeDecoration::decorations`,
  `ProgramDecoration::dummyParamList`), which in C++ are *outside* the generated child-visit — the
  generator emits them into the GC walk so their nodes stay live.
- `unsafe` budget: confined to `context.rs` (the copied GC: `UnsafeCell` chunks, `offset_of`
  entry↔node, lifetime transmutes inside `alloc`). It is the same encapsulated, sanctioned `unsafe`
  juno already vets, and matches the project's existing carve-outs (`atom_table`, the lexer cursor).
  Every other module in the crate is safe; the crate does **not** `forbid(unsafe_code)` but isolates
  `unsafe` to the storage module.

## 2. Node-set codegen

A **committed generator that parses `include/hermes/AST/ESTree.def`** and emits checked-in Rust
(`nodes_generated.rs`), matching the project precedent (the unicode tables via `gen_tables.py`, the
HTML entities). Chosen over a juno-style hand-transcribed `macro_rules!` DSL so that `ESTree.def`
stays the *only* place node shapes are defined — the generator and the `hermesc` oracle cannot drift.

The generator:
- Parses the `ESTREE_NODE_n_ARGS` / `ESTREE_FIRST` / `ESTREE_LAST` / `ESTREE_IGNORE_IF_EMPTY` macros,
  treating all `#if HERMES_PARSE_FLOW/JSX/TS` families as **on** (full-featured `hermesc`).
- Attaches each node's decoration fields from a **small committed table** (node→decoration mapping +
  each decoration's field list), hand-transcribed once from the `Decoration` classes / `DecoratorTrait<…>`
  specializations in `ESTree.h`. We do **not** parse `ESTree.h` — it is not practical, and the decoration
  set changes rarely, so a committed table the generator consumes is the right tradeoff. (`ESTree.def`
  remains machine-parsed and the sole source of truth for node *shapes*; only the decoration overlay is a
  table.)
- Emits, per node: the `#[repr(C)]` struct (metadata-first; child fields as `&'gc`/`Option`/`NodeList`,
  value + decoration fields as `Cell<…>`), the `Node<'gc>` enum arm, the `NodeKind`/`NodeVariant`
  entry and the `_FIRST_`/`_LAST_` ranges for `classof`-style `isa`/`dyn_cast`, the `visit_children` /
  `visit_children_mut` / `mark_lists` arms (including the decoration `NodeList`s), the Builder, and the
  dumper field list (with `IGNORE_IF_EMPTY` honored).

## 3. `NodeList`

Copy juno's `NodeListElement` linked list verbatim: a `Copy` head pointer (`NodeList<'gc>`), elements
allocated in the `Context` (`{ inner: *const Node, next: Cell<*const NodeListElement> }`), O(1) append
during construction via `Cell<next>`, GC-traced via `mark_lists`, and rebuilt with `from_iter` on
change. Lists are not deep-matchable (you iterate), matching C++ `simple_ilist`. Slices
(`&'gc [&'gc Node]`) were considered (cache-friendly, deep-matchable) but rejected here: they would
mean reworking the copied GC's list storage for a property the C++ doesn't have either.

## 4. Validation

The AST cannot be differentially tested on its own — it needs a producer. So:

1. **Now (AST component):** unit tests over hand-built trees exercising construction, the `Cell`
   in-place mutation, the functional `VisitorMut` rebuild (deep `match`, child replacement, list
   rebuild), `NodeRc` across a GC, and the GC itself (allocate, root, collect, reuse; assert the two
   decoration lists keep their nodes live). Plus a port of any `unittests/` that cover the AST/dumper
   directly, and golden tests for `ESTreeJSONDumper` output on those hand-built trees.
2. **At Parser time (the real gate):** an `-dump-ast` JSON oracle — a C++ tool emitting
   `ESTreeJSONDumper` output and the Rust `ESTreeJSONDumper` emitting **byte-for-byte identical** JSON
   over a JS corpus, asserted in a `differential` test (same `CARGO_MANIFEST_DIR` + `REQUIRE_DIFFERENTIAL=1`
   pattern as the lexer/JSON ports). Post-parse dump is the first gate; a post-sema dump (decorations
   resolved) becomes a second gate when Sema lands.

## Faithfulness notes / deliberate deviations

- **C++ class hierarchy + multiple-inheritance decorations → enum + flattened fields.** `isa`/`dyn_cast`/
  `cast` become enum matching / `as_*()` accessors over the `NodeKind` `_FIRST_`/`_LAST_` ranges; the
  decoration mixin becomes ordinary (mostly `Cell`) fields on the variant struct.
- **In-place child mutation → functional rebuild** (the one behavioral restructure, made local by the
  recursive walks). In-place *attribute* mutation is preserved exactly, via `Cell`.
- **Bump + never-free → GC arena** (juno's), because we now *do* produce garbage (rebuilds).
- **Raw `Node*` → `&'gc Node<'gc>`**; `simple_ilist` → juno `NodeListElement` list.
- **Sema side-pointers → id handles.** `LexicalScope*`/`FunctionInfo*`/`Decl*` become
  `Cell<Option<ScopeId/FunctionInfoId/DeclId>>` placeholders, defined for real when Sema is ported.
- **`getAllocator` has no analog** beyond the `Context`'s arena — consistent with the lexer/JSON ports'
  documented surface gap.

## Decisions deferred to Sema / Parser (not blockers)

- **Sema id handles are a deliberate placeholder.** Their real representation cannot be predicted now
  and will be pinned when Sema is scoped; the AST only needs the field present and `Cell`-mutable, so
  we use an opaque placeholder newtype (e.g. `Cell<Option<SemaId>>`) until then.
- `BlockStatementDecoration`'s lazy-reparse fields (`isLazyFunctionBody`, `paramYield/Await`,
  `bufferId`) — carried as `Cell` fields now; lazy parsing itself is a Parser concern.

## Validation commands (to be finalized in the plan)

```bash
cargo test  --manifest-path rust/Cargo.toml -p ast            # node model + GC + visitor + dumper unit tests
cargo build --manifest-path rust/Cargo.toml                   # expect ZERO warnings
# (Parser-time, later) byte-for-byte AST differential vs hermesc -dump-ast:
#   cmake --build cmake-build-asan --target ast-dump
#   REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p parser --test ast_differential -- --nocapture
```
