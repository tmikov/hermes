# Sema S0 (Foundations) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Sema component's foundations: `NodeId` + the freed-id log in the
`ast` crate, `PersistentScopedMap` in `support`, the `sema` crate with typed ids,
`Keywords`, `SemContext` (+ dumper), `DeclCollector`, a minimal resolver entry that
handles trivial programs, and a **live byte-for-byte `sema_differential` gate** vs
`hermesc -dump-sema`.

**Architecture:** Per `doc/superpowers/specs/2026-07-26-sema-untyped-design.md`
(READ IT FIRST). Sema decorations live in generated node `Cell`s (`SemaId`); sema
records live in id-indexed `Vec`s in `SemContext`; node-keyed side maps use the new
`NodeId`. The S0 resolver is an honest skeleton: it ports the *real* Program entry
machinery (FunctionContext, ScopeRAII, directives, ambient decls) and panics with a
clear message on any construct S1 owns.

**Tech Stack:** Rust workspace at `rust/` (edition per existing crates, toolchain
1.96.0). C++ source of truth: `lib/Sema/`, `include/hermes/Sema/`,
`include/hermes/ADT/PersistentScopedMap.h`, `include/hermes/Runtime/Libhermes.h`.

## Global Constraints

- **Never `cd`.** Use `--manifest-path rust/Cargo.toml` for cargo;
  absolute/repo-relative paths elsewhere.
- **Zero warnings** from `cargo build --manifest-path rust/Cargo.toml`; no new
  clippy lints beyond the established faithful-C-idiom set.
- **Faithful port**: keep C++ structure, names (snake_cased), and comments. Copy
  C++ comments (adapted) onto the corresponding Rust items. C++ default arguments
  are spec — read the headers. C++ templates stay generics.
- **No `unsafe`** in `sema` (`#![forbid(unsafe_code)]`) and no new unsafe in
  `support`. New unsafe in `ast` only if unavoidable inside `context.rs` (the
  sanctioned file); the NodeId work below needs none beyond what exists.
- Copyright header (from CLAUDE.md) on every new file.
- Commit per task; message style `rust(sema): <what>` (or `rust(ast):`,
  `rust(support):`), ending with the Claude co-author line used in this session.
- The differential gate command (must pass at the end of Task 8):
  `REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p sema --features dump-bin --test sema_differential -- --nocapture`
  (build the oracle first: `cmake --build cmake-build-asan --target hermesc`).
- Run the full workspace tests before each commit:
  `cargo test --manifest-path rust/Cargo.toml`.

---

### Task 1: `NodeId` + freed-id log in `ast`

**Files:**
- Modify: `rust/crates/ast/src/lib.rs` (add `NodeId` next to `SemaId` at :16)
- Modify: `rust/crates/ast/src/node_child.rs:33-75` (`NodeMetadata`)
- Modify: `rust/crates/ast/src/context.rs` (`Context` fields ~:148-220, `alloc`
  :265-292, `gc` :529-545, `AllocationScope::drop` :734-752)
- Modify: `rust/crates/ast/src/node.rs` — ONLY via `gen_nodes.py` if any generated
  code constructs `NodeMetadata` literally (check; the `new`/`new_with_debug`
  constructors are the expected only paths, in which case node.rs is untouched)
- Test: `rust/crates/ast/tests/node_id.rs` (new)

**Interfaces:**
- Produces: `ast::NodeId(pub u32)` with `NodeId::UNASSIGNED = NodeId(0)`;
  `NodeMetadata.id: Cell<NodeId>`; `Node::node_id(&self) -> NodeId`;
  `Context::take_freed_node_ids(&mut self) -> Vec<NodeId>`.
- Consumes: nothing new.

- [ ] **Step 1: Write failing tests** in `rust/crates/ast/tests/node_id.rs`.
  Follow the arena-test idioms in `rust/crates/ast/tests/` (spine/structural tests
  show how to build contexts, locks, nodes, `NodeRc`, run `ctx.gc()`, and use
  `alloc_scope`). Cover exactly:

```rust
// 1. Uniqueness + monotonicity: allocate 3 nodes under one lock; their
//    node_id()s are distinct, nonzero, increasing.
// 2. Fresh id on rebuild: build a node, transform it with a builder
//    (clone-with-one-field-changed, as tests/transform.rs does); the new
//    node's id != old id, and the old node's id is unchanged.
// 3. GC log: alloc a node, take a NodeRc to a *different* node, drop all
//    references to the first, ctx.gc(); take_freed_node_ids() contains the
//    first id exactly once and NOT the rooted one; a second take returns [].
// 4. AllocationScope log: under a lock, open unsafe { gc.alloc_scope() },
//    alloc 2 nodes, record ids, drop the scope; after releasing the lock,
//    take_freed_node_ids() == those 2 ids (order irrelevant; sort both).
// 5. NodeMetadata::new(...).id starts UNASSIGNED; after gc.alloc(...) the
//    stored node's id is != UNASSIGNED (alloc stamps unconditionally).
```

- [ ] **Step 2: Run** `cargo test --manifest-path rust/Cargo.toml -p ast --test node_id`
  — expect compile FAILURE (missing `NodeId`).

- [ ] **Step 3: Implement.**
  - `lib.rs`: next to `SemaId`:

```rust
/// Unique, never-reused identity of an AST node within its `Context`.
/// Assigned by `Context::alloc` from a monotonic counter (starting at 1);
/// `UNASSIGNED` (0) only exists on metadata not yet stored in the arena.
/// Consumers outside sema key side tables by NodeId (see the Sema design
/// spec §3.1): unlike raw addresses, ids never alias after arena slot
/// reuse; unlike NodeRc keys, they don't pin garbage. Insert entries only
/// with the node in hand under GCLock — a stored id may already be dead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u32);

impl NodeId {
    pub const UNASSIGNED: NodeId = NodeId(0);
}
```

  - `NodeMetadata`: add `pub id: Cell<NodeId>`; `new`/`new_with_debug` init
    `Cell::new(NodeId::UNASSIGNED)`; `duplicate()` also resets to `UNASSIGNED`
    (comment: the id belongs to the arena slot's occupant; `alloc` stamps it).
  - `Context`: add `next_node_id: Cell<u32>` (init `1`) and
    `freed_node_ids: RefCell<Vec<NodeId>>` (init empty) + both in `Context::new`.
  - `alloc` (context.rs:265-292): after the entry is established (both the
    free-list-reuse and the fresh-push arm), stamp:

```rust
let id = self.next_node_id.get();
self.next_node_id.set(id.checked_add(1).expect("NodeId overflow"));
entry.inner.metadata().id.set(NodeId(id));
```

  - `gc` (:542-544): immediately BEFORE `entry.ctx_id_markbit.set(FREE_ENTRY)`
    for a freed node entry, push `entry.inner.metadata().id.get()` onto
    `freed_node_ids` (borrow the RefCell once outside the loop). List elements
    (:547-560) are NOT nodes — no logging there.
  - `AllocationScope::drop` (:734-752): before `nodes.truncate(...)`, iterate
    `nodes.iter_from(self.nodes_watermark)` and push each entry's id (the debug
    asserts already guarantee no suffix entry is free).
  - `Context::take_freed_node_ids(&mut self) -> Vec<NodeId>` via
    `std::mem::take(&mut *self.freed_node_ids.borrow_mut())`.
  - `Node::metadata()` exists (node.rs:6908); add
    `pub fn node_id(&self) -> NodeId { self.metadata().id.get() }` beside it —
    if that impl block is generated, add via `gen_nodes.py` and regenerate
    (then `REQUIRE_GEN=1 cargo test -p ast --test generated_idempotent`);
    if hand-written, edit directly.

- [ ] **Step 4: Run tests** — the new file and the whole `ast` crate
  (`cargo test --manifest-path rust/Cargo.toml -p ast`), plus the parser crate
  (`-p parser`) since it allocates nodes (`preparse_memory` in particular must
  stay green — it counts nodes, not bytes). Expect PASS, zero warnings.

- [ ] **Step 5: Commit** `rust(ast): NodeId in NodeMetadata + freed-id log (sweep + AllocationScope)`.

---

### Task 2: `PersistentScopedMap` in `support`

**Files:**
- Create: `rust/crates/support/src/persistent_scoped_map.rs`
- Modify: `rust/crates/support/src/lib.rs` (add `pub mod persistent_scoped_map;`)
- Test: inline `#[cfg(test)] mod tests` in the same file (support-crate idiom)

**Interfaces:**
- Produces (module `support::persistent_scoped_map`):
  - `PersistentScopedMap<K: Eq + Hash + Copy, V: Clone>` — all methods `&self`
    (interior mutability), mirroring C++ `PersistentScopedMap<K,V>`:
    `try_emplace(&self, k: K, v: V) -> bool` (false if key already in current
    scope), `try_emplace_into_scope(&self, scope: &ScopePtr<K,V>, k: K, v: V) -> bool`,
    `put(&self, k: K, v: V)`, `put_in_scope(&self, scope: &ScopePtr<K,V>, k: K, v: V)`,
    `count(&self, k: &K) -> u32`, `lookup(&self, k: &K) -> Option<V>`,
    `find(&self, k: &K) -> Option<V>`,
    `find_with_depth(&self, k: &K) -> Option<(V, u32)>`,
    `find_in_current_scope(&self, k: &K) -> Option<V>`,
    `activate_scope(&self, ptr: &ScopePtr<K,V>)`,
    `current_scope(&self) -> ScopePtr<K,V>`,
    `flatten(&self) -> HashMap<K, V>` and
    `keys_by_scope(&self) -> Vec<Vec<K>>` (ports of the `UNIT_TEST` helpers,
    kept `pub` — the resolver's tests will use them too).
  - `ScopePtr<K,V>`: `Clone + Default`, `is_null(&self) -> bool`, `reset(&mut self)`,
    `PartialEq` (pointer identity), port of `PersistentScopedMapScopePtr`.
  - `Scope<'m, K, V>` RAII (port of `PersistentScopedMapScope`):
    `Scope::new(&'m PersistentScopedMap<K,V>) -> Scope`, `depth() -> u32`,
    `ptr() -> ScopePtr<K,V>`; `Drop` pops it.
- Consumes: nothing new.

- [ ] **Step 1: Read the C++**: `include/hermes/ADT/PersistentScopedMap.h` (620
  lines, read it ALL) and `unittests/ADT/PersistentScopedMapTest.cpp` (242 lines).
  The semantics to preserve exactly: per-scope entry lists; per-key shadow chains;
  scope refcounting so a popped scope survives while a `ScopePtr` holds it;
  `activate_scope` = pop down to the nearest active ancestor of the target, then
  re-push the target chain (header :440-463); `find*` semantics incl. depth;
  `try_emplace` returns whether insertion happened; `put` overwrites in scope.

- [ ] **Step 2: Write failing tests** — port every test in
  `PersistentScopedMapTest.cpp` (names snake_cased, same scenarios: nested
  scopes/shadowing, popping restores, retained-scope reactivation after pop,
  reactivating a non-current active scope, flatten/keys-by-scope shapes). Add one
  Rust-specific test: dropping the last `ScopePtr` of a popped scope frees it
  without touching the map (no panic; a later `activate_scope` of a *different*
  retained sibling still works).

- [ ] **Step 3: Run** `cargo test --manifest-path rust/Cargo.toml -p support persistent_scoped_map`
  — expect compile FAILURE.

- [ ] **Step 4: Implement** (safe Rust; support is `#![forbid(unsafe_code)]`).
  Recommended internals (deviation from the C++ raw-pointer web, documented in the
  module doc comment): scopes are `Rc<ScopeData<K,V>>` where

```rust
struct ScopeData<K, V> {
    /// Entries declared in this scope, in insertion order.
    entries: RefCell<Vec<Entry<K, V>>>,
    parent: Option<Rc<ScopeData<K, V>>>,
    depth: u32,
    active: Cell<bool>,
}
struct Entry<K, V> {
    key: K,
    value: RefCell<V>,
    /// The (scope, index) this entry shadows, if any — the C++ nextShadowed_.
    shadowed: Option<(Rc<ScopeData<K, V>>, usize)>,
}
```

  and the map holds `map_: RefCell<HashMap<K, (Rc<ScopeData<K,V>>, usize)>>` +
  `scope_: RefCell<Option<Rc<ScopeData<K,V>>>>`. `Rc` replaces the intrusive
  refcount (`ScopePtr` wraps `Option<Rc<ScopeData>>`); everything else follows the
  C++ method-for-method (`pop_scope`, `push_child_scope`, `pop_entry`,
  `push_entry`, `insert_new_node`, `try_emplace_into_scope` — keep the C++
  comments). C++ asserts (pop-non-current, insert-under-shadow, destroy-active)
  become `debug_assert!`/`assert!` matching the original strictness.

- [ ] **Step 5: Run tests** — expect PASS; then whole workspace, zero warnings.

- [ ] **Step 6: Commit** `rust(support): PersistentScopedMap (port of hermes/ADT/PersistentScopedMap.h) + ported unit tests`.

---

### Task 3: `sema` crate scaffold, typed ids, `Keywords`

**Files:**
- Modify: `rust/Cargo.toml` (add `"crates/sema"` to members)
- Create: `rust/crates/sema/Cargo.toml`, `rust/crates/sema/src/lib.rs`,
  `rust/crates/sema/src/ids.rs`, `rust/crates/sema/src/keywords.rs`
- Test: inline unit tests in `ids.rs` / `keywords.rs`

**Interfaces:**
- Produces:
  - crate `sema` (lib deps: `ast`, `support`, `atom_table`; **optional** deps
    `parser` + `command_line` gated by feature `dump-bin`; `parser` also a
    dev-dependency for tests). `lib.rs` starts `#![forbid(unsafe_code)]`.
  - `sema::ids::{DeclId, ScopeId, FunctionInfoId}` — `u32` newtypes,
    `Copy + Eq + Hash + Debug`, each with
    `fn from_sema_id(id: ast::SemaId) -> Self` and
    `fn sema_id(self) -> ast::SemaId`, plus `fn index(self) -> usize`.
  - `sema::keywords::Keywords` — one `pub <name>: Atom` field per
    `HERMES_KEYWORD(name, string)` entry in `include/hermes/AST/Keywords.def`
    (136 entries; field names snake_cased with the established acronym rules,
    e.g. `ident##UseStrict` → `ident_use_strict`), and
    `Keywords::new(gc: &ast::context::GCLock) -> Keywords` interning each string
    via `gc.atom_bytes(...)` (match how the parser interns identifier atoms —
    grep `atom_bytes` uses in `rust/crates/parser/src/js/mod.rs` and follow).
- Consumes: `ast::SemaId` (lib.rs:16), `GCLock::atom_bytes` (context.rs:710).

- [ ] **Step 1: Scaffold** the crate: `Cargo.toml` mirroring an existing member's
  layout (see `rust/crates/ast/Cargo.toml` for edition/lints config), with:

```toml
[features]
dump-bin = ["dep:parser", "dep:command_line"]

[[bin]]
name = "sema-dump"
required-features = ["dump-bin"]
```

  (the actual bin source file arrives in Task 8 — add the `[[bin]]` section in
  Task 8 if cargo complains about a missing file before then). `lib.rs` declares
  `pub mod ids; pub mod keywords;` with the crate doc comment naming the C++
  source-of-truth files.

- [ ] **Step 2: `ids.rs` with tests**: the three newtypes + conversions. Test:
  round-trip `DeclId(7)` ↔ `SemaId` ↔ back; `index()` == 7.

- [ ] **Step 3: `keywords.rs`**: transcribe `Keywords.def` into a
  `macro_rules!`-driven struct so the list is written once:

```rust
macro_rules! declare_keywords {
    ($(($field:ident, $string:literal),)*) => {
        /// Convenient storage of "keyword" identifier atoms used by sema.
        /// Port of `hermes::Keywords` (AST/Context.h:168, Keywords.def).
        pub struct Keywords { $(pub $field: Atom,)* }
        impl Keywords {
            pub fn new(gc: &GCLock) -> Keywords {
                Keywords { $($field: gc.atom_bytes($string).into(),)* }
            }
            pub const COUNT: usize = [$(stringify!($field)),*].len();
        }
    };
}
declare_keywords! {
    (ident_arguments, "arguments"),
    (ident_eval, "eval"),
    // ... one line per HERMES_KEYWORD entry, in Keywords.def order ...
    (ident_use_strict, "use strict"),
}
```

  (Adjust the `Atom`-vs-`AtomBytes` type and the `into()` to whatever the parser
  actually stores for identifier names — `Identifier.name` is `Cell<NodeLabel>`;
  match `NodeLabel`'s underlying atom type so `directive == kw.ident_use_strict`
  compares work without conversion.) Transcribe ALL 136 entries from
  `include/hermes/AST/Keywords.def` — no sampling. Test: `Keywords::COUNT == 136`;
  spot-check `ident_use_strict` round-trips to the bytes `use strict` via
  `gc.bytes(...)`; `ident_plus` is `+`.

- [ ] **Step 4: Run** `cargo test --manifest-path rust/Cargo.toml -p sema`,
  then the workspace build for zero warnings.

- [ ] **Step 5: Commit** `rust(sema): crate scaffold, typed ids, Keywords (136 atoms from Keywords.def)`.

---

### Task 4: `SemContext` core

**Files:**
- Create: `rust/crates/sema/src/sem_context.rs`
- Modify: `rust/crates/sema/src/lib.rs`
- Test: `rust/crates/sema/tests/sem_context.rs`

**Interfaces:**
- Consumes: Task 2's `PersistentScopedMap`/`ScopePtr`/`Scope`, Task 3's ids +
  `Keywords`, `ast::{NodeId, NodeRc, SemaId}`.
- Produces (all in `sema::sem_context`, C++ names snake_cased):
  - `Binding { pub decl: DeclId, pub ident: Option<NodeRc> }` (+ `is_valid`,
    `invalidate` — model "invalid" as the resolver storing/overwriting whole
    `Binding`s; if an invalid state is needed use `Option<Binding>` at use sites,
    decided in S1 — for S0 just the struct + ctor).
  - `type BindingTable = PersistentScopedMap<Atom, Binding>` (+ `BindingTableScope`,
    `BindingTableScopePtr` aliases).
  - `DeclKind` (18 variants, exact C++ order Let..UndeclaredGlobalProperty),
    `DeclSpecial { NotSpecial, Arguments, Eval, PrivateStatic }`,
    `Constness { Never, StrictModeOnly, Always }`, and the static predicates as
    `DeclKind` methods: `is_tdz()`, `is_var_like()`,
    `is_var_like_or_scoped_function()`, `is_let_like()`, `is_global()`,
    `constness()`, `is_private_name()` — port each comparison EXACTLY from
    SemContext.h:130-189 (they rely on variant order; add a test).
  - `Decl { name: Atom, kind: DeclKind, generic: bool, special: DeclSpecial,
    scope: Option<ScopeId> }` — `customData` is deliberately NOT ported:
    consumers keep their own `DeclId`-keyed tables (spec §3.1 principle); doc-note
    this deviation on the struct.
  - `LexicalScope { depth: u32, parent_function: FunctionInfoId,
    parent_scope: Option<ScopeId>, idx_in_parent_function: u32,
    decls: Vec<DeclId>, hoisted_functions: Vec<NodeRc>, local_eval: bool,
    binding_table_scope: BindingTableScopePtr }`.
  - `FuncIsArrow { Yes, No }`, `ConstructorKind { None, Base, Derived }`,
    `SourceVisibility { Default, ShowSource, HideSource, Sensitive }` and
    `CustomDirectives { source_visibility, always_inline, no_inline, builtin }`
    (ports of AST/Context.h:128-166; they live in `sema` for now — doc-note).
  - `FunctionInfo` — all fields from SemContext.h:291-427 snake_cased
    (`scopes: Vec<ScopeId>` private + `get_scopes()`/`add_scope()`,
    `parent_function: Option<FunctionInfoId>`, `parent_scope: Option<ScopeId>`,
    `imports: Vec<NodeRc>`, `arguments_decl: Option<DeclId>` with a separate
    `arguments_decl_set: bool`? NO — port `OptValue<Decl*>` as
    `arguments_decl: Option<DeclId>` since C++ never stores nullptr in it,
    `function_body_scope_idx: u32` (`u32::MAX` sentinel + `get_function_body_scope()`,
    `get_parameter_scope()`), `strict`, `custom_directives`, `arrow`,
    `constructor_kind`, `simple_parameter_list`, `has_parameter_expressions`,
    `uses_arguments`, `contains_arrow_functions`,
    `contains_arrow_functions_using_arguments`, `may_reach_implicit_return`,
    `is_program_node`, `is_static_block`, `binding_table_scope`,
    `num_labels: u32` + `allocate_label()`).
  - `SemContext` with: `pub kw: Keywords`; storages
    `functions: Vec<FunctionInfo>`, `scopes: Vec<LexicalScope>`,
    `decls: Vec<Decl>`; `binding_table: BindingTable`;
    `binding_table_global_scope: BindingTableScopePtr`;
    `side_identifier_declaration_decl: HashMap<NodeId, DeclId>`;
    `promoted_function_decls: HashMap<NodeId, DeclId>`;
    `builtin_declarations: Vec<NodeRc>`.
    The C++ parent/child SemContext tree (shared_ptr parent, root_,
    parentLexScope_) is S5 scope — leave the fields OUT with a doc comment
    pointing at SemContext.h:638-664 and the S5 phase (accessors below therefore
    read `self` directly where C++ goes through `root_`).
    Methods (each a faithful port; C++ line refs in doc comments):
    `new(kw: Keywords) -> SemContext`,
    `node_is_arrow(node: Option<&Node>) -> FuncIsArrow` (SemContext.cpp:75),
    `nearest_non_arrow(&self, f: FunctionInfoId) -> FunctionInfoId` (cpp:82),
    `new_function(...) -> FunctionInfoId` (cpp:95),
    `new_scope(&mut self, parent_function: FunctionInfoId, parent_scope: Option<ScopeId>) -> ScopeId`
    (cpp:115 — including `add_scope` setting `idx_in_parent_function`),
    `new_decl_in_scope(&mut self, name: Atom, kind: DeclKind, scope: ScopeId, special: DeclSpecial) -> DeclId`
    (cpp:134; also a 3-arg convenience like the C++ default arg),
    `new_global(&mut self, name: Atom, kind: DeclKind) -> DeclId` (cpp:152,
    asserts `kind.is_global()`),
    `func_arguments_decl(&mut self, func: FunctionInfoId, arguments_name: Atom) -> DeclId`
    (cpp:160 — the arrow-ancestor walk, global vs function arm, caching),
    `get_global_function()/get_global_scope()`,
    accessors `function(&self, FunctionInfoId) -> &FunctionInfo` (+`_mut`),
    `scope(...)`, `decl(...)` (+`_mut`),
    and the **identifier decl-state machine** (see next step).
- The `assert_global_function_and_scope` port: `debug_assert!(!self.functions.is_empty() ...)`.

- [ ] **Step 1: Read the C++**: `include/hermes/Sema/SemContext.h` (whole file)
  and `lib/Sema/SemContext.cpp:1-415`, plus `include/hermes/AST/ESTree.h:451-505`
  (the `IdentifierDecoration` — get the EXACT bit values of `BitHaveDecl`,
  `BitHaveExpr`, `BitSideDecl` and the unresolvable encoding; the Rust
  `Identifier` node already has `unresolvable: Cell<bool>`,
  `decl_state: Cell<u8>`, `decl: Cell<Option<SemaId>>` — node.rs:1758).

- [ ] **Step 2: Write failing tests** in `tests/sem_context.rs`:

```rust
// A. DeclKind predicate table: for every variant assert is_tdz/is_var_like/
//    is_let_like/is_global/is_private_name/constness against a hand-written
//    expected table transcribed from SemContext.h:130-189.
// B. new_function/new_scope/new_decl_in_scope: ids are dense indices;
//    scope.decls records the decl; function.get_scopes() records the scope
//    with idx_in_parent_function set; global accessors return id 0.
// C. func_arguments_decl: (i) on a non-arrow function creates Var/Arguments
//    in scopes[0] and caches; (ii) on an arrow chain resolves to the
//    non-arrow ancestor; (iii) on the global function creates
//    UndeclaredGlobalProperty.
// D. Decl-state machine (the important one — drive it through a real
//    Identifier node built in a test Context): for each C++ switch arm of
//    setDeclarationDecl (cpp:215-296) and setExpressionDecl (cpp:329-405+),
//    including: set expr then same declaration (bits merge, no side table);
//    set expr then DIFFERENT declaration (side table entry appears, state
//    BitHaveExpr|BitSideDecl); update side decl back to equal (side entry
//    erased); unset paths. After each transition assert
//    get_declaration_decl/get_expression_decl return the C++-specified
//    values and side-table size matches.
```

- [ ] **Step 3: Run** — expect FAILURE.

- [ ] **Step 4: Implement** `sem_context.rs`. The decl-state machine methods take
  the identifier node: `get_declaration_decl(&self, ident: &Identifier) -> Option<DeclId>`,
  `get_expression_decl(&self, ident: &Identifier) -> Option<DeclId>` (with the
  C++ precondition `assert!(!ident.unresolvable.get())`),
  `set_declaration_decl(&mut self, node_id: NodeId, ident: &Identifier, decl: Option<DeclId>)`,
  `set_expression_decl(...)`, `set_both_decl(...)`,
  `set_promoted_decl(&mut self, node_id: NodeId, decl: DeclId)`,
  `get_promoted_decl(&self, node_id: NodeId) -> Option<DeclId>`,
  `clear_promoted_decls(&mut self)`. Port each switch arm and its comment
  verbatim from SemContext.cpp:200-296 and :329-411 (the node stores
  `decl: Cell<Option<SemaId>>` where C++ stores the raw pointer; bits live in
  `decl_state: Cell<u8>` with the exact C++ constants). `getConstructor`
  (cpp:298) is also ported here (`get_constructor(&self, class_node) -> Option<&Node>`,
  matching on `ClassDeclaration`/`ClassExpression` body and
  `MethodDefinition.kind == kw.ident_constructor`).

- [ ] **Step 5: Run tests** — PASS; workspace build zero warnings.

- [ ] **Step 6: Commit** `rust(sema): SemContext core — records, storages, decl-state machine (SemContext.{h,cpp})`.

---

### Task 5: `SemContextDumper`

**Files:**
- Create: `rust/crates/sema/src/dump_context.rs`
- Modify: `rust/crates/sema/src/lib.rs`
- Test: `rust/crates/sema/tests/dump_context.rs`

**Interfaces:**
- Consumes: Task 4's `SemContext` + records.
- Produces: `SemContextDumper` with
  `new() -> Self` and `new_annotated(f: Box<dyn Fn(&mut String, DeclId)>) -> Self`
  (the FlowChecker hook — port the shape now, unused until the typed component),
  `print_sem_context(&mut self, out: &mut String, ctx: &SemContext, root_func: Option<FunctionInfoId>)`,
  `print_scope_ref(&mut self, out: &mut String, s: ScopeId)`,
  `print_decl_ref(&mut self, out: &mut String, ctx: &SemContext, d: DeclId, print_name: bool)`.
  Numbering: two `HashMap<u32, usize>` (decl/scope id → number, next starts at 1,
  assigned on first print — port of `PtrNumberingImpl`, cpp:565).

- [ ] **Step 1: Port** `lib/Sema/SemContext.cpp:415-563` method-for-method
  (`printSemContext` child-map + recursive lambda → a recursive helper over a
  `HashMap<Option<FunctionInfoId>, Vec<FunctionInfoId>>` built in ITERATION ORDER
  of the `functions` vec — C++ `std::map` over pointers into a deque iterates in
  address order == allocation order for a deque; replicate by iterating the
  storage `Vec` in index order when both building and walking children, which
  yields the identical output order; same for scopes inside `printFunction`).
  `ind(level)` = `level * 4` spaces (cpp:15). `printDecl` kind/special strings via
  exhaustive `match` mirroring the CASE macros (cpp:504-554). Atom text printed
  via the atom table — the dumper methods therefore also take `gc: &GCLock`
  (or the resolved `&[u8]`/`str` accessor the crate settles on) wherever a name
  is printed; match the C++ `'` quoting exactly.
- [ ] **Step 2: Golden unit test**: hand-build (no parser): global function
  (loose) + global scope + 2 decls (`x` Let, `f` GlobalProperty) + a nested
  function (strict, child scope, one hoisted-function entry pointing at a
  hand-built `FunctionDeclaration` node with an `Identifier` id `g`). Assert the
  EXACT multi-line string (transcribe expected shape from the real
  `hermesc -dump-sema` output format seen in
  `test/Sema/*.js` FileCheck expectations — e.g. `Func loose`/`Func strict`,
  `    Scope %s.1`, `        Decl %d.1 'x' Let`,
  `        hoistedFunction g`).
- [ ] **Step 3: Run** — PASS; commit
  `rust(sema): SemContextDumper (SemContext.cpp:415-563) + golden test`.

---

### Task 6: `ASTPrinter` + `sem_dump`

**Files:**
- Create: `rust/crates/sema/src/dump.rs`
- Modify: `rust/crates/sema/src/lib.rs`
- Test: `rust/crates/sema/tests/dump_ast.rs`

**Interfaces:**
- Consumes: Task 5's dumper; `ast` read `Visitor` (or manual recursion);
  the node decoration Cells; `SemContext` accessors.
- Produces:
  `pub fn sem_dump(out: &mut String, gc: &GCLock, sem_ctx: &SemContext, root: &Node)` —
  port of `semDump` (SemResolve.cpp:254-293), untyped arm only (no FlowContext
  param yet; the typed arm arrives with the FlowChecker component):
  `printSemContext(root_func from a FunctionLike root's sem_info else None)` +
  `'\n'` + ASTPrinter run + the run's trailing `'\n'` (SemResolve.cpp:48).

- [ ] **Step 1: Port the `ASTPrinter`** (SemResolve.cpp:20-157) as a private
  struct in `dump.rs` doing manual recursion over `visit_children` order
  (the C++ uses `ESTreeVisit` with `shouldVisit`/`enter`/`leave`; our read
  `Visitor` trait can express it — check `ast::Visitor`'s API and pick the
  closest structure, documenting the choice). Port exactly:
  - `enter(Node)`: indent `(depth-1)*4`, node name (`node_type_str` gives the
    ESTree name — C++ `getNodeName()` — verify identical for the S0 corpus),
    then scope ref if the node's `scope` Cell is set (a
    `fn node_scope(node: &Node) -> Option<SemaId>` helper matching the 15
    scope-bearing variants — Program, function-likes, BlockStatement,
    StaticBlock, CatchClause, ClassDeclaration/Expression, For/ForIn/ForOf,
    SwitchStatement), then `'\n'`.
  - `enter(Identifier)`: `Id 'name'` + the `[D:E:...]`/`[D:... E:...]` bracket
    logic (SemResolve.cpp:96-125, using Task 4's get_declaration/expression_decl
    with the C++'s exact branch structure) + ` UNR` when `unresolvable` + `'\n'`.
  - `enter(BinaryExpression)` with `+`/`-` left-linearization
    (SemResolve.cpp:70-95): port `ESTree::linearizeLeft` (find it via
    `grep -rn linearizeLeft include/hermes/AST lib/AST` and port the exact
    condition) as a local helper returning the `Vec<&BinaryExpression>` chain;
    print `BinOp +`/`BinOp -` lines at `depth*4`; replicate the
    `parentLinearized_` skip flag so linearized children aren't re-visited.
  - `should_visit`: skip `TypeAnnotation` nodes (the Flow wrapper kind —
    SemResolve.cpp:52) — include now, it matters once dialect corpora join.
- [ ] **Step 2: Test**: hand-build `1 + 2 - 3` nested BinaryExpressions + an
  annotated Identifier chain, assert exact output including the linearized
  `BinOp` lines; plus a `sem_dump` smoke test on a hand-built empty Program with
  scope set (matches the tail of the empty-file golden:
  `Program Scope %s.1\n\n`).
- [ ] **Step 3: Run** — PASS; commit
  `rust(sema): ASTPrinter + sem_dump (SemResolve.cpp:20-157,254-293)`.

---

### Task 7: `DeclCollector`

**Files:**
- Create: `rust/crates/sema/src/decl_collector.rs`
- Modify: `rust/crates/sema/src/lib.rs`; `rust/crates/sema/Cargo.toml`
  (`[dev-dependencies] parser = { path = "../parser" }`)
- Test: `rust/crates/sema/tests/decl_collector.rs`

**Interfaces:**
- Consumes: `Keywords`, `ast::{NodeId, NodeRc}`.
- Produces: `ScopeDecls = Vec<NodeRc>`;
  `DeclCollector::run(root: &Node, gc: &GCLock, kw: &Keywords, recursion_depth: u32, recursion_depth_exceeded: &mut dyn FnMut(&Node)) -> DeclCollector`
  (covers both C++ overloads — FunctionLike and StaticBlock roots);
  `scope_decls_for_node(&self, node_id: NodeId) -> Option<&ScopeDecls>`;
  `scoped_func_decls(&self) -> &[NodeRc]`; `dump(&self, ...)` (cpp `dump`,
  used by tests). Internals: `scopes: HashMap<NodeId, ScopeDecls>`,
  `scoped_func_decls: Vec<NodeRc>`, `scope_stack: Vec<ScopeDecls>`.

- [ ] **Step 1: Read** `lib/Sema/DeclCollector.{h,cpp}` fully (185+202 lines) and
  port method-for-method: the visit set (VariableDeclaration → addToCur if
  `let/const`, addToFunc if `var` — read the cpp for the actual dispatch;
  ClassDeclaration; ImportDeclaration; FunctionDeclaration incl. the
  scoped-function tracking; the no-descend set: FunctionExpression/Arrow/
  ClassExpression/interface bodies/Binary/Assignment; scope-creating visits:
  BlockStatement, For/ForIn/ForOf, SwitchStatement, CatchClause each
  newScope→visit children→closeScope; TypeAlias/TSTypeAlias — our single node
  set has all dialect nodes, no cfg gates). `NestedRecursionDepthTracker`
  becomes an explicit `remaining_depth: u32` decremented per visit with the
  callback on hitting 0 (same observable semantics; doc-comment the mapping).
  `closeScope` attaches non-empty lists to `scopes` keyed by `node.node_id()`.
- [ ] **Step 2: Tests** (parse with the real parser as dev-dep — copy the
  parse-driver setup from `rust/crates/parser/src/bin/ast_dump.rs`, trimmed):
  for `var a; let b; function f(){var inner;} { let c; function g(){} }` assert:
  function-root ScopeDecls = [a's VariableDeclaration, f's FunctionDeclaration]
  (declaration NODES, not names — assert by kind + extracted name), block's
  ScopeDecls = [c's decl, g's decl], `scoped_func_decls` = [g], nothing from
  `inner`. Plus a switch-with-case-decls case and a catch-param case
  (transcribe expectations from DeclCollector.cpp behavior, not guessed).
- [ ] **Step 3: Run** — PASS; commit
  `rust(sema): DeclCollector (DeclCollector.{h,cpp}) keyed by NodeId`.

---

### Task 8: Minimal resolver entry, `sema-dump` bin, live differential

**Files:**
- Create: `rust/crates/sema/src/resolver/mod.rs`, `rust/crates/sema/src/resolve.rs`,
  `rust/crates/sema/src/libhermes.rs`, `rust/crates/sema/src/bin/sema_dump.rs`
- Modify: `rust/crates/sema/src/lib.rs`, `rust/crates/sema/Cargo.toml`
- Test: `rust/crates/sema/tests/sema_differential.rs` +
  corpus `rust/crates/sema/tests/sema_corpus/*.js`

**Interfaces:**
- Consumes: everything above; `parser::js::JSParserImpl` + lexer +
  `support::manager::SourceErrorManager` + `command_line` (bin only).
- Produces:
  - `resolver::SemanticResolver` (S0 subset) with
    `new(sem_ctx: &mut SemContext, sm: &mut SourceErrorManager, ambient_decls: &[NodeRc], compile: bool)`-shaped
    construction (follow the C++ ctor arg list, SemanticResolver.cpp:40-63,
    including `restricted_global_properties` init from kw NaN/undefined/Infinity)
    and `run(&mut self, gc, root: &Node) -> bool` (cpp:65-70).
  - `resolve::resolve_ast(gc, sem_ctx, sm, root, ambient_decls) -> bool` —
    the untyped `resolveAST` overload (SemResolve.cpp:159-191 minus the
    flow/lowering arm). NOTE: S0's resolver does no tree rewriting, so `run`
    returns `bool` like C++; the `VisitorMut`-returns-new-root signature change
    lands in S1/S2 when the first rewrite is ported — S1 plan owns that.
  - `libhermes::LIBHERMES: &str` — transcription of
    `include/hermes/Runtime/Libhermes.h:13-72` with the `TYPED_ARRAY` include
    expanded from `include/hermes/VM/TypedArrays.def` (verify the expanded
    name list against the 63-decl empty-file dump; the differential enforces).
  - the `sema-dump` bin + the differential test.

- [ ] **Step 1: S0 resolver skeleton** (`resolver/mod.rs`), porting faithfully:
  - Fields (S0 subset of SemanticResolver.h): `sem_ctx`, `sm`, `kw` (borrowed
    from sem_ctx), `ambient_decls`, `compile`, `cur_scope: Option<ScopeId>`,
    `global_scope: BindingTableScopePtr`, `function_stack: Vec<FunctionContext>`
    — model FunctionContext/ScopeRAII as **explicit push/pop structs** per the
    port conventions (Drop-guards don't fit borrowed `&mut self` here; use
    `enter_function(...)/exit_function()` pairs wrapped in small structs the
    way the parser's `SaveFunctionState` does — study
    `rust/crates/parser/src/js/functions.rs` for the established shape).
  - `FunctionContext` (S0 fields): `sem_info: FunctionInfoId`,
    `node: Option<NodeRc>`, `decls: Option<DeclCollector>` (run in the ctor arm
    that has a node — SemanticResolver.cpp:2963-2992), plus empty
    label/loop/promoted maps as placeholders WITH their real types
    (`HashMap<Atom, Label>` etc.) so S1 doesn't re-plumb.
  - `ScopeRAII` equivalent `enter_scope(node: Option<&Node>, is_function_body_scope: bool)`:
    `new_scope` + set `cur_scope` + write the node's `scope` Cell (via a
    `set_node_scope(node, ScopeId)` helper matching the scope-bearing variants)
    + push a `BindingTable` scope + on function-body-scope set
    `function_body_scope_idx` (SemanticResolver.cpp:2919-2950 — read the whole
    ScopeRAII ctor/dtor including the `debugInfoSetting == ALL` branch; our
    port has no debug-info setting yet → port the condition as a
    `/* DebugInfoSetting::ALL: not ported until needed */` false constant).
  - `scan_directives` (cpp:2764-2812): full port incl. the inline/noinline
    warning interplay (`sm.warning(...)`; check `support`'s warning API name).
  - `process_ambient_decls` (cpp:2846-2917): the `DeclHoisting` collector over
    each ambient Program (VariableDeclarator + FunctionDeclaration names, no
    descent into functions) + `declare_ambient_global` via
    `binding_table.count()` / `new_global` / `try_emplace_into_scope`.
  - `visit_program` (cpp:193-231): FunctionContext(strict = ctx strict_mode,
    CustomDirectives default) → scan_directives → strict update →
    `node.strictness.set(...)` (check `ast::Strictness` variant names for
    `makeStrictness`) → sourceVisibility max-merge → `is_program_node = true` →
    enter_scope(functionScope=true) → `binding_table_global_scope` set →
    `process_collected_declarations(node)` (S0 version: read the
    DeclCollector's list for this node; if non-empty →
    `panic!("sema S0: declarations are S1 scope")`) → loose-mode
    promoted-func-decls hook (`if !strict { /* S3 */ }` with the collector's
    `scoped_func_decls` asserted empty) → `process_ambient_decls` →
    visit children.
  - Generic visiting: `visit_node(&mut self, gc, node)` matching ONLY:
    `ExpressionStatement` (recurse into expression), `EmptyStatement`,
    `NumericLiteral | StringLiteral | BooleanLiteral | NullLiteral` (leaf,
    no-op). EVERYTHING else:
    `panic!("sema S0: unhandled node kind {} — S1+", node.node_type_str())`.
- [ ] **Step 2: `sema-dump` bin** (`src/bin/sema_dump.rs`, feature `dump-bin`):
  clone `ast_dump.rs`'s option/driver skeleton (same dialect flags, no
  location/pretty flags), then: build `ast::Context` with flags + strict=false →
  lock → parse `LIBHERMES` (own SourceErrorManager buffer id, Full pass; a parse
  failure there is a hard `panic!` — it is our own constant) → collect its
  Program as the one ambient decl (root it with `NodeRc`) → parse the input file
  → if parse errors: print diagnostics (they were already streamed by the SEM,
  matching hermesc) and exit(1) → `Keywords::new` + `SemContext::new` →
  `resolve_ast(...)` → on false exit(1) → `sem_dump` to a String → print to
  stdout, exit 0. OUTPUT CONTRACT comment at the top like ast_dump.rs.
- [ ] **Step 3: Corpus + differential test.** Corpus files (each first verified
  manually against `cmake-build-asan/bin/hermesc -dump-sema <f>` — all must
  exit 0):
  - `empty.js` (zero bytes)
  - `comments.js` (`// line` + `/* block */` only)
  - `literals.js` (`1;\n"not first so not a directive";\ntrue;\nnull;\n` —
    CAREFUL: a leading string literal is a directive; keep the number first)
  - `use-strict.js` (`"use strict";\n42;\n` — flips `Func strict`)
  - `empty-statements.js` (`;;;\n`)
  `tests/sema_differential.rs`: copy the `parser_differential.rs` harness
  (:28-117) with: hermesc args `["-dump-sema"]`, Rust bin
  `env!("CARGO_BIN_EXE_sema-dump")`, assert hermesc success, compare **stdout
  AND stderr AND exit status** (stderr expected empty on this corpus; the
  comparison is wired now so S1's error corpus drops in). Cargo needs the bin
  built with the feature for tests:
  add `[dev-dependencies]` on the crate's own feature via
  `sema = { path = ".", features = ["dump-bin"] }`? NO — self-deps are not
  allowed; instead make the differential test require the feature:
  `#![cfg(feature = "dump-bin")]` at the top of the test file, and document the
  gate command as
  `REQUIRE_DIFFERENTIAL=1 cargo test -p sema --features dump-bin --test sema_differential`.
  Update the Global Constraints command accordingly in the final docs task.
- [ ] **Step 4: Run the gate**:
  `REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p sema --features dump-bin --test sema_differential -- --nocapture`
  — expect: `sema differential (tests/sema_corpus): 5 corpus files matched`.
  Then the whole workspace: `cargo test --manifest-path rust/Cargo.toml` +
  zero-warning build. Iterate on byte diffs (the first run WILL differ —
  ambient-decl order, Keywords atom identity, indentation — fix the Rust side,
  never hand-patch expectations).
- [ ] **Step 5: Commit** `rust(sema): S0 resolver entry + sema-dump bin + live sema_differential (5 files)`.

---

### Task 9: Docs + roadmap

**Files:**
- Modify: `doc/superpowers/RustPortRoadmap.md` (Sema row: S0 done, what shipped,
  the gate command with `--features dump-bin`)
- Modify: `doc/superpowers/specs/2026-07-26-sema-untyped-design.md` — §5 gate
  command gains `--features dump-bin`; §6 S0 marked done
- Modify: `doc/superpowers/SESSION-HANDOFF.md` — status line: Sema S0 done,
  S1 (declarations & scopes) next, plan to be written just-in-time

- [ ] **Step 1:** Make the three edits; keep them summary-level (the roadmap is
  the source of truth, the handoff references it).
- [ ] **Step 2:** Commit `doc(rust): Sema S0 foundations complete`.

---

## Self-review notes (done at plan-writing time)

- **Spec coverage (S0 slice):** NodeId+log → T1; scaffold → T3; SemContext,
  Keywords, known-globals (= libhermes ambient path), dumper → T4/T5/T6/T8;
  differential green on trivial subset → T8. PersistentScopedMap (T2) and
  DeclCollector (T7) are pulled into S0 because the Program entry machinery
  (ScopeRAII/FunctionContext) cannot run faithfully without them.
- **Known S0→S1 seams, stated intentionally:** `process_collected_declarations`
  and all declaration/identifier visits panic with an explicit message; the
  resolver-as-`VisitorMut` signature change lands with the first ported rewrite
  (S1/S2); the SemContext parent/child tree is S5; `Binding::invalidate`
  semantics finalized in S1 with the binding-table use sites.
- **Type consistency:** `DeclId/ScopeId/FunctionInfoId` (T3) used in T4-T8;
  `NodeId` keys for `scopes` (T7) and side maps (T4); `ScopePtr` alias name
  `BindingTableScopePtr` fixed in T4 and reused in T8.
- **Verified against a live oracle during planning:** empty-file dump = 63
  ambient decls + `Program Scope %s.1`, exit 0; `-dump-sema` works with
  `-lazy`/`-commonjs`/`-parse-flow` (S1+ corpora will use them).
