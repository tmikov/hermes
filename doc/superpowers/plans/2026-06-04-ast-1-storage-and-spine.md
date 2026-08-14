# AST Phase 1 — Storage/GC spine + minimal node model

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up a new `ast` crate with juno's GC arena (`Context`/`GCLock`/`NodeRc` + mark-sweep), copied and adapted to our `support`/`atom_table` crates, plus a **minimal hand-written node model** (4 node kinds) that exercises every tricky part of the design — `#[repr(C)]` enum `Node` for deep `match`, immutable child fields, `Cell<…>` attributes, `NodeList`, a decoration `NodeList`, functional rebuild, and GC reclamation of orphans. The full ~200-node set is generated from `ESTree.def` in a later phase; this phase proves the spine.

**Architecture:** New `rust/crates/ast/`. Copy `Deque`/`HeapSize` and `context.rs` from juno verbatim, then adapt: locations use our `support::SMRange` (not juno's `source_manager`), atoms use our `atom_table` crate, `c_void`→`std::ffi`, `memoffset::offset_of!`→`core::mem::offset_of!`. The minimal `Node<'gc>` enum + per-kind structs are hand-written (they will be generated later) so `context.rs`'s GC marker has a real `visit_children`/`mark_lists` to call. `unsafe` is confined to `context.rs` (the crate denies `unsafe_code` and `context.rs` opts in with a documented inner `#![allow]`).

**Tech Stack:** Rust 2021, toolchain 1.96 (`core::mem::offset_of!` available → no `memoffset`/`libc` deps). Path deps: `support`, `atom_table`.

**Reference spec:** `doc/superpowers/specs/2026-06-03-ast-design.md` (READ IT).
**Source of truth to copy/adapt (READ THESE):**
- `unsupported/juno/crates/juno_ast/src/context.rs` — `Context`/`GCLock`/`NodeRc`/`StorageEntry`/`NodeListElement`/`gc()`. Copy then adapt per Task 3.
- `unsupported/juno/crates/juno_support/src/deque.rs`, `.../heap_size.rs` — copy verbatim (Task 1).
- `unsupported/juno/crates/juno_ast/src/node_child.rs` — `NodeList`/`NodeListElement`/`NodeMetadata`/`NodeChild` shapes (Task 2 mirrors the relevant parts).
- `include/hermes/AST/ESTree.h:73–210` (base `Node`), `:286–319` (`FunctionLikeDecoration`/`ProgramDecoration` — the two decoration `NodeList`s).
- For our `atom_table` usage, mirror the lexer: `rust/crates/parser/src/lexer/identifier.rs` (how `AtomTable`/`AtomBytes` are used).

**Porting rule:** keep structure close to juno/Hermes, copy comments. **Do NOT `cd`** out of the project root; use `--manifest-path`.

## Confirmed crate APIs (resolved — use these exact names)

- **Locations** are **not** re-exported at the crate root: use `support::location::{SMRange, SMLoc, SourceId}`.
  There is **no `invalid()`**. `SMLoc { source: SourceId, offset: u32 }`, `SMRange { start: SMLoc, end: SMLoc }`,
  both `Copy`. `SourceId::from_index(0)` builds a valid id. Tests build a dummy range with a shared helper:
  ```rust
  fn dummy_range() -> support::location::SMRange {
      let l = support::location::SMLoc { source: support::location::SourceId::from_index(0), offset: 0 };
      support::location::SMRange { start: l, end: l }
  }
  ```
- **Atoms:** `atom_table::AtomTable::atom_bytes(value: impl Into<Vec<u8>> + AsRef<[u8]>) -> AtomBytes`
  and `.bytes(AtomBytes) -> &[u8]`. `AtomBytes` is `Copy` (so `Cell<NodeLabel>` is valid). Call with a
  byte slice: `atom_bytes("+".as_bytes())`. The `Context`/`GCLock` exposes an `atom_bytes(&self, &[u8]) -> AtomBytes`
  passthrough (Task 3).
- **`core::mem::offset_of!`** is available on toolchain 1.96 — no `memoffset`/`libc` deps.

> Wherever the task code below shows `support::SMRange::invalid()` / `support::SMRange` / `gc.atom_bytes(b"+")`,
> use `dummy_range()` / `support::location::SMRange` / `gc.atom_bytes("+".as_bytes())` respectively.

---

## Phase roadmap (for context; only Phase 1 is in this plan)

1. **This plan** — `ast` crate, GC storage, minimal node model, Visitor + rebuild + GC tests.
2. Node-set codegen — generator parsing `ESTree.def` + decoration table → `nodes_generated.rs` (full ~200 nodes); replaces the hand-written model.
3. Builders (generated) + `RecursiveVisitor`/`VisitorMut` over the full set.
4. `ESTreeJSONDumper` port + golden tests. (Byte-for-byte `-dump-ast` differential lands with the Parser.)

---

## File structure

```
rust/crates/ast/
  Cargo.toml          # new crate; path deps support, atom_table; lint unsafe_code = "deny"
  src/
    lib.rs            # pub mod deque; mod heap_size; pub mod node; pub mod node_child;
                      # pub mod context; pub mod visitor; placeholder SemaId
    heap_size.rs      # COPIED verbatim from juno_support
    deque.rs          # COPIED verbatim from juno_support (depends on heap_size)
    node.rs           # minimal Node<'gc> enum + 4 kind structs + NodeKind + visit_children/mark_lists
    node_child.rs     # NodeMetadata, NodeList, NodeListElement, NodeChild trait + leaf impls
    context.rs        # COPIED + ADAPTED juno context.rs (the ONLY unsafe)
    visitor.rs        # Visitor (read) trait + Path; TransformResult (for rebuild tests)
```

`rust/Cargo.toml` workspace `members` gains `"crates/ast"`.

---

## Task 0: Crate skeleton

**Files:** Create `rust/crates/ast/Cargo.toml`, `rust/crates/ast/src/lib.rs`; Modify `rust/Cargo.toml`.

- [ ] **Step 1:** Add `"crates/ast"` to `members` in `rust/Cargo.toml`.

- [ ] **Step 2:** Create `rust/crates/ast/Cargo.toml`:

```toml
[package]
name = "ast"
version = "0.1.0"
edition = "2021"

[dependencies]
support = { path = "../support" }
atom_table = { path = "../atom_table" }

[lints.rust]
unsafe_code = "deny"
```

- [ ] **Step 3:** Create `rust/crates/ast/src/lib.rs`:

```rust
//! Hermes ESTree AST — GC arena (juno-derived) + node model.
//! See doc/superpowers/specs/2026-06-03-ast-design.md.

pub mod context;
pub mod deque;
mod heap_size;
pub mod node;
pub mod node_child;
pub mod visitor;

pub use heap_size::HeapSize;

/// Placeholder for a resolved Sema entity (scope / decl / function info).
/// The real representation is pinned when Sema is ported; the AST only needs
/// an opaque, `Cell`-mutable handle until then.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemaId(pub u32);
```

- [ ] **Step 4:** Create empty stub files `heap_size.rs`, `deque.rs`, `node.rs`, `node_child.rs`, `context.rs`, `visitor.rs`, each with a single `//!` doc line, so the crate compiles. (Real content lands in later tasks; for now `node`/`context`/`visitor` can be empty.)

- [ ] **Step 5:** `cargo build --manifest-path rust/Cargo.toml -p ast` → builds (warnings about unused empty modules are fine for this step only).

- [ ] **Step 6:** Commit: `rust(ast): scaffold ast crate (storage/GC spine — phase 1)`

---

## Task 1: Copy `HeapSize` + `Deque` verbatim

**Files:** `rust/crates/ast/src/heap_size.rs`, `rust/crates/ast/src/deque.rs`.

- [ ] **Step 1:** Copy `unsupported/juno/crates/juno_support/src/heap_size.rs` into `rust/crates/ast/src/heap_size.rs` **verbatim** (keep the copyright header). It defines `pub trait HeapSize { fn heap_size(&self) -> usize; }` and its std impls.

- [ ] **Step 2:** Copy `unsupported/juno/crates/juno_support/src/deque.rs` into `rust/crates/ast/src/deque.rs` **verbatim**. Its only dependency is `use crate::HeapSize;` — that already resolves via the `pub use heap_size::HeapSize;` in `lib.rs`. It carries its own `#[cfg(test)] mod tests` (`append`, `multi_chunks`).

- [ ] **Step 3:** The `multi_chunks` test uses `unsafe { *ptr }`. Because the crate denies `unsafe_code`, add a scoped allow at the top of `deque.rs` test module OR (preferred) the file already needs none in non-test code — add `#[allow(unsafe_code)]` on the `multi_chunks` fn with a comment `// test-only: verifies chunk stability`.

- [ ] **Step 4:** Run: `cargo test --manifest-path rust/Cargo.toml -p ast deque::` 
Expected: PASS (`append`, `multi_chunks`).

- [ ] **Step 5:** Commit: `rust(ast): copy Deque + HeapSize from juno_support`

---

## Task 2: Minimal node model (`node_child.rs`, `node.rs`)

The 4 kinds are chosen to cover every field category: a scalar leaf (`NumericLiteral.value`), a `Cell` label + a `Cell` sema placeholder (`Identifier`), two required children + a `Cell` operator (`BinaryExpression`), and a child `NodeList` + a **decoration** `NodeList` (`Program`).

**Files:** `rust/crates/ast/src/node_child.rs`, `rust/crates/ast/src/node.rs`.

- [ ] **Step 1: `node_child.rs` — metadata, leaf types, NodeList.** Write:

```rust
//! Child/leaf field types and the NodeList for the AST.
use std::cell::Cell;
use std::marker::PhantomData;
use support::location::SMRange;
use crate::context::NodeListElement;
use crate::node::Node;

/// JS identifier / operator / keyword bytes, interned in the AtomTable.
pub type NodeLabel = atom_table::AtomBytes;

/// Metadata present on every node. `range`/`parens` are attributes → `Cell`.
#[derive(Debug)]
pub struct NodeMetadata<'gc> {
    pub(crate) phantom: PhantomData<&'gc Node<'gc>>,
    pub range: Cell<SMRange>,
    /// 0, 1, or 2 (meaning "2 or more"), mirroring ESTree.h Node::parens_.
    pub parens: Cell<u8>,
}

impl<'gc> NodeMetadata<'gc> {
    pub fn new(range: SMRange) -> Self {
        NodeMetadata { phantom: PhantomData, range: Cell::new(range), parens: Cell::new(0) }
    }
}

/// Ordered list of child nodes. A `Copy` head pointer into context-allocated
/// `NodeListElement`s (juno model). Empty == null head.
#[derive(Debug, Copy, Clone)]
pub struct NodeList<'gc> {
    pub(crate) head: *const NodeListElement<'gc>,
}

impl<'gc> NodeList<'gc> {
    pub fn empty() -> Self {
        NodeList { head: std::ptr::null() }
    }
    pub fn is_empty(&self) -> bool {
        self.head.is_null()
    }
    pub fn iter(self) -> NodeListIter<'gc> {
        NodeListIter { ptr: self.head, _pd: PhantomData }
    }
}

pub struct NodeListIter<'gc> {
    ptr: *const NodeListElement<'gc>,
    _pd: PhantomData<&'gc Node<'gc>>,
}

impl<'gc> Iterator for NodeListIter<'gc> {
    type Item = &'gc Node<'gc>;
    fn next(&mut self) -> Option<&'gc Node<'gc>> {
        if self.ptr.is_null() {
            None
        } else {
            // SAFETY note: dereference is sound because list elements live in the
            // Context for the GCLock lifetime. The single `unsafe` lives in context.rs;
            // expose the deref via a context.rs helper so node_child stays safe.
            let (node, next) = crate::context::list_elem_parts(self.ptr);
            self.ptr = next;
            Some(node)
        }
    }
}
```

Note: to keep `node_child.rs` free of `unsafe`, the actual raw deref of `NodeListElement` is done by a helper `list_elem_parts(ptr) -> (&Node, *const NodeListElement)` defined in `context.rs` (Task 3). This concentrates the one deref in the unsafe-permitted module.

- [ ] **Step 2: `node.rs` — the enum + 4 structs + NodeKind + child walk.** Write:

```rust
//! Minimal hand-written node model (phase 1). Replaced by generated nodes later.
use std::cell::Cell;
use crate::SemaId;
use crate::node_child::{NodeLabel, NodeList, NodeMetadata};
use crate::visitor::Visitor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    NumericLiteral,
    Identifier,
    BinaryExpression,
    Program,
}

#[derive(Debug)]
#[repr(C)]
pub enum Node<'gc> {
    NumericLiteral(NumericLiteral<'gc>),
    Identifier(Identifier<'gc>),
    BinaryExpression(BinaryExpression<'gc>),
    Program(Program<'gc>),
}

#[derive(Debug)]
#[repr(C)]
pub struct NumericLiteral<'gc> {
    pub metadata: NodeMetadata<'gc>,
    pub value: Cell<f64>,
}

#[derive(Debug)]
#[repr(C)]
pub struct Identifier<'gc> {
    pub metadata: NodeMetadata<'gc>,
    pub name: Cell<NodeLabel>,
    /// Resolved declaration (sema decoration) — placeholder until Sema.
    pub decl: Cell<Option<SemaId>>,
}

#[derive(Debug)]
#[repr(C)]
pub struct BinaryExpression<'gc> {
    pub metadata: NodeMetadata<'gc>,
    pub left: &'gc Node<'gc>,
    pub right: &'gc Node<'gc>,
    pub operator: Cell<NodeLabel>,
}

#[derive(Debug)]
#[repr(C)]
pub struct Program<'gc> {
    pub metadata: NodeMetadata<'gc>,
    pub body: NodeList<'gc>,
    /// FunctionLike/Program decoration list (the `decorations`/`dummyParamList`
    /// case). Node container in side-data; traced by GC like any child.
    pub decorations: Cell<NodeList<'gc>>,
}

impl<'gc> Node<'gc> {
    pub fn kind(&self) -> NodeKind {
        match self {
            Node::NumericLiteral(_) => NodeKind::NumericLiteral,
            Node::Identifier(_) => NodeKind::Identifier,
            Node::BinaryExpression(_) => NodeKind::BinaryExpression,
            Node::Program(_) => NodeKind::Program,
        }
    }

    pub fn range(&self) -> support::location::SMRange {
        self.metadata().range.get()
    }

    pub fn metadata(&self) -> &NodeMetadata<'gc> {
        match self {
            Node::NumericLiteral(n) => &n.metadata,
            Node::Identifier(n) => &n.metadata,
            Node::BinaryExpression(n) => &n.metadata,
            Node::Program(n) => &n.metadata,
        }
    }

    /// Visit every child node (single children + every NodeList, **including
    /// decoration lists**). Non-recursive: calls `v.visit_node` per child.
    pub fn visit_children<V: Visitor<'gc>>(&'gc self, v: &mut V) {
        match self {
            Node::NumericLiteral(_) | Node::Identifier(_) => {}
            Node::BinaryExpression(n) => {
                v.visit_node(n.left);
                v.visit_node(n.right);
            }
            Node::Program(n) => {
                for c in n.body.iter() {
                    v.visit_node(c);
                }
                for c in n.decorations.get().iter() {
                    v.visit_node(c);
                }
            }
        }
    }

    /// Call `cb` for each NodeList element reachable from this node (for GC mark),
    /// including decoration lists.
    pub fn mark_lists<F: FnMut(&NodeList<'gc>)>(&'gc self, cb: &mut F) {
        if let Node::Program(n) = self {
            cb(&n.body);
            let d = n.decorations.get();
            cb(&d);
        }
    }
}
```

- [ ] **Step 3: failing test** `rust/crates/ast/src/node.rs` `#[cfg(test)]` — deep match + Cell mutation (no GC yet; build nodes on the stack-ish via leaked refs is awkward, so defer construction tests to Task 3 where Context exists). For now just assert the enum compiles and `NodeKind` mapping:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn kind_mapping_compiles() {
        // Compile-time shape check; real node construction needs Context (Task 3).
        fn _accepts(n: &Node) -> NodeKind { n.kind() }
        assert_eq!(NodeKind::Program, NodeKind::Program);
    }
}
```

- [ ] **Step 4:** Write a minimal `visitor.rs` so `node.rs` compiles:

```rust
//! AST traversal.
use crate::node::Node;

/// Read-only visitor. Implementors override `visit_node`; the default recurses.
pub trait Visitor<'gc> {
    fn visit_node(&mut self, node: &'gc Node<'gc>) {
        node.visit_children(self);
    }
}

/// Result of a transforming visit (functional rebuild).
pub enum TransformResult<'gc> {
    Unchanged,
    Changed(&'gc Node<'gc>),
}
```

- [ ] **Step 5:** `cargo build --manifest-path rust/Cargo.toml -p ast` → clean (the `list_elem_parts`/`NodeListElement` references resolve once Task 3 adds them; if doing Task 2 before Task 3, temporarily stub `list_elem_parts` returning via an `unimplemented!()` guarded by `#[allow(unsafe_code)]`-free code — but prefer to land Task 2+3 together if the borrow won't compile alone). 
Expected: PASS `node::tests::kind_mapping_compiles`.

- [ ] **Step 6:** Commit: `rust(ast): minimal hand-written node model (4 kinds, Cell attrs, decoration list)`

---

## Task 3: Copy + adapt juno `context.rs` (the GC; the ONLY unsafe)

**Files:** `rust/crates/ast/src/context.rs`.

- [ ] **Step 1:** Copy `unsupported/juno/crates/juno_ast/src/context.rs` into `rust/crates/ast/src/context.rs` as the starting point.

- [ ] **Step 2: Apply these exact adaptations** (each is a concrete edit, not a placeholder):

  1. Add at the very top (after the copyright header): `#![allow(unsafe_code)]` with a comment: `// The GC arena is the single sanctioned location for encapsulated unsafe in this crate (see spec §1).`
  2. Imports: replace `use juno_support::Deque;` → `use crate::deque::Deque;`; `use juno_support::HeapSize;` → `use crate::HeapSize;`. Remove `use juno_support::atom_table::{Atom, AtomTable, AtomU16};` and instead `use atom_table::AtomTable;` (our crate). Remove `use libc::c_void;` → `use std::ffi::c_void;`. Remove `use memoffset::offset_of;` (use `core::mem::offset_of!` at call sites).
  3. Replace `use crate::SourceManager;` and the `source_mgr` field/methods: drop juno's `SourceManager`. The `Context` does **not** own a source manager (ours lives in `support::SourceErrorManager`, owned by the caller). Delete the `source_mgr` field, `sm()`, `sm_mut()`, and the `source_mgr.heap_size()` line in `HeapSize`.
  4. `Node` import: `use crate::Node;` → `use crate::node::Node;`. `use crate::Path;`/`use crate::Visitor;` → `use crate::visitor::Visitor;` (Path isn't needed by the minimal marker — see step 4 of this task).
  5. Every `offset_of!(StorageEntry, inner)` → `core::mem::offset_of!(StorageEntry, inner)`.
  6. Atom convenience methods: keep `atom_table: AtomTable` field and `atom`/`str` passthroughs, adapting signatures to **our** `atom_table::AtomTable` API. Mirror how the lexer interns (`rust/crates/parser/src/lexer/identifier.rs`). If our `AtomTable` exposes `atom_bytes(&[u8]) -> AtomBytes` and `bytes(AtomBytes) -> &[u8]`, expose `Context::atom_bytes`/`Context::bytes` passthroughs. Drop juno's `atom_u16`/`str_u16` for phase 1 unless our crate has direct equivalents.
  7. Keep `StorageEntry`, `NodeListElement`, `NodeRcCounter`, `Context`, `GCLock`, `NodePtr`, `NodeRc`, `alloc`, `append_list_element`, `gc`, `num_nodes`, `storage_size` as-is (modulo the import/type fixes).

- [ ] **Step 3:** Adapt the GC marker (`gc()` inner `Marker`) to our `Visitor`/`mark_lists` shape. juno's marker calls `node.mark_lists(gc, cb)` and `node.visit_children(gc, self)`. Our `node.rs` (Task 2) defines `mark_lists(&mut F)` and `visit_children(&mut V)` **without** a `gc` arg. Rewrite the `Marker` to:

```rust
struct Marker {
    markbit_marked: bool,
}
impl<'gc> Visitor<'gc> for Marker {
    fn visit_node(&mut self, node: &'gc Node<'gc>) {
        let entry = unsafe { StorageEntry::from_node(node) };
        if entry.markbit() == self.markbit_marked {
            return; // already marked subtree
        }
        entry.set_markbit(self.markbit_marked);
        let mark = self.markbit_marked;
        node.mark_lists(&mut |list| {
            // mark each list element's storage bit
            let mut p = list.head;
            while !p.is_null() {
                let elem = unsafe { &*p };
                elem.set_markbit(mark);
                p = elem.next.get();
            }
        });
        node.visit_children(self);
    }
}
```
Then in `gc()` replace `root.inner.visit(&gc, &mut marker, None)` with `marker.visit_node(&root.inner)` (no GCLock re-entrancy needed for marking since we hold `&mut self`). Keep the rest of `gc()` (root collection, sweep of `nodes` and `list_elements`, markbit flip) unchanged.

- [ ] **Step 4: Add the `list_elem_parts` helper** that Task 2's `NodeListIter` calls (concentrates the one list deref here):

```rust
/// Dereference a `NodeListElement`, returning its node and the next pointer.
/// The single sanctioned list-deref (see node_child::NodeListIter).
pub(crate) fn list_elem_parts<'gc>(
    ptr: *const NodeListElement<'gc>,
) -> (&'gc Node<'gc>, *const NodeListElement<'gc>) {
    let elem = unsafe { &*ptr };
    (unsafe { &*elem.inner }, elem.next.get())
}
```

- [ ] **Step 5: failing test** — append `#[cfg(test)] mod tests` to `context.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::*;
    use crate::node_child::{NodeList, NodeMetadata};
    use std::cell::Cell;

    fn num<'gc>(gc: &'gc GCLock, v: f64) -> &'gc Node<'gc> {
        gc.alloc(Node::NumericLiteral(NumericLiteral {
            metadata: NodeMetadata::new(support::SMRange::invalid()),
            value: Cell::new(v),
        }))
    }

    #[test]
    fn alloc_and_deep_match() {
        let mut ctx = Context::new();
        let gc = GCLock::new(&mut ctx);
        let l = num(&gc, 1.0);
        let r = num(&gc, 2.0);
        let op = gc.atom_bytes(b"+");
        let bin = gc.alloc(Node::BinaryExpression(BinaryExpression {
            metadata: NodeMetadata::new(support::SMRange::invalid()),
            left: l, right: r, operator: Cell::new(op),
        }));
        // Deep, one-level match through &Node.
        if let Node::BinaryExpression(b) = bin {
            assert!(matches!(b.left, Node::NumericLiteral(n) if n.value.get() == 1.0));
        } else { panic!() }
    }

    #[test]
    fn cell_mutation_in_place() {
        let mut ctx = Context::new();
        let gc = GCLock::new(&mut ctx);
        let n = num(&gc, 3.0);
        if let Node::NumericLiteral(x) = n { x.value.set(9.0); }
        assert!(matches!(n, Node::NumericLiteral(x) if x.value.get() == 9.0));
    }

    #[test]
    #[should_panic(expected = "multiple GCLocks")]
    fn single_gclock_per_thread() {
        let mut a = Context::new();
        let mut b = Context::new();
        let _g1 = GCLock::new(&mut a);
        let _g2 = GCLock::new(&mut b); // must panic
    }
}
```

- [ ] **Step 6:** Run: `cargo test --manifest-path rust/Cargo.toml -p ast context::` 
Expected: PASS (`alloc_and_deep_match`, `cell_mutation_in_place`, `single_gclock_per_thread`). Adjust `gc.atom_bytes`/`SMRange::invalid()` names to the real `atom_table`/`support` API if they differ (read those crates).

- [ ] **Step 7:** Commit: `rust(ast): copy+adapt juno GC arena (Context/GCLock/NodeRc); only unsafe is here`

---

## Task 4: Prove the spine — functional rebuild + GC reclamation + list tracing

**Files:** `rust/crates/ast/tests/spine.rs` (new integration test).

- [ ] **Step 1:** Create `rust/crates/ast/tests/spine.rs` with a hand-written recursive transform that **rebuilds** any `BinaryExpression`/`Program` whose children changed and leaves unchanged subtrees **shared** (the functional model), plus a helper to build a small tree:

```rust
use ast::context::{Context, GCLock, NodeRc};
use ast::node::*;
use ast::node_child::{NodeList, NodeMetadata};
use std::cell::Cell;

fn r() -> support::SMRange { support::SMRange::invalid() }

fn num<'gc>(gc: &'gc GCLock, v: f64) -> &'gc Node<'gc> {
    gc.alloc(Node::NumericLiteral(NumericLiteral { metadata: NodeMetadata::new(r()), value: Cell::new(v) }))
}

/// Functional transform: double every NumericLiteral, rebuilding ancestors whose
/// children changed; share unchanged subtrees. Returns the (maybe-new) node.
fn double<'gc>(gc: &'gc GCLock, n: &'gc Node<'gc>) -> &'gc Node<'gc> {
    match n {
        Node::NumericLiteral(x) => num(gc, x.value.get() * 2.0),
        Node::BinaryExpression(b) => {
            let l = double(gc, b.left);
            let rr = double(gc, b.right);
            if std::ptr::eq(l, b.left) && std::ptr::eq(rr, b.right) {
                n // unchanged → shared
            } else {
                gc.alloc(Node::BinaryExpression(BinaryExpression {
                    metadata: NodeMetadata::new(r()),
                    left: l, right: rr, operator: Cell::new(b.operator.get()),
                }))
            }
        }
        other => other,
    }
}

#[test]
fn rebuild_then_gc_reclaims_orphans() {
    let mut ctx = Context::new();
    {
        let gc = GCLock::new(&mut ctx);
        let op = gc.atom_bytes(b"+");
        let bin = gc.alloc(Node::BinaryExpression(BinaryExpression {
            metadata: NodeMetadata::new(r()), left: num(&gc, 1.0), right: num(&gc, 2.0),
            operator: Cell::new(op),
        }));
        let new = double(&gc, bin);
        // new tree has doubled values; old tree still structurally intact (shared op).
        if let Node::BinaryExpression(b) = new {
            assert!(matches!(b.left, Node::NumericLiteral(n) if n.value.get() == 2.0));
            assert!(matches!(b.right, Node::NumericLiteral(n) if n.value.get() == 4.0));
        } else { panic!() }
        // Root only `new` via NodeRc so the old `bin` subtree is collectible.
        let _root = NodeRc::from_node(&gc, new);
        let before = ctx_num_nodes(&gc);
        assert!(before >= 6, "1,2 + bin + 2,4 + newbin = 6 nodes allocated");
    } // drop gc
    // Re-lock, gc; the 3 orphaned old nodes (1.0, 2.0, old bin) get freed/reused.
    // (Exact reuse assertion: num_nodes counts slots incl. free list, so assert the
    // free list grew — expose Context::num_free_nodes() if needed, or assert a
    // subsequent alloc reuses a slot. Implement whichever is cleanest.)
    // ... gc + assertion per the chosen accessor.
}

fn ctx_num_nodes(gc: &GCLock) -> usize { gc.ctx().num_nodes() }
```

  Implementation note for the agent: finish the reclamation assertion using whatever `Context` exposes (`num_nodes`, and add a `num_free_nodes()`/`gc()`-public wrapper if needed) — the *behavioral* claim to prove is "after rooting only the new tree and calling `gc()`, the 3 old nodes are no longer live (freed/reusable)."

- [ ] **Step 2:** Add a **decoration-list tracing** test in the same file: build a node reachable **only** through a `Program.decorations` list, root the program, `gc()`, and assert the decorated node survived (proving the GC marker walks decoration lists, not just `.def` children):

```rust
#[test]
fn gc_traces_decoration_lists() {
    let mut ctx = Context::new();
    let mut keep: Option<NodeRc> = None;
    {
        let gc = GCLock::new(&mut ctx);
        let dec = num(&gc, 42.0);
        let list = NodeList::from_iter(&gc, [dec]); // add from_iter to node_child (juno-style)
        let prog = gc.alloc(Node::Program(Program {
            metadata: NodeMetadata::new(r()), body: NodeList::empty(),
            decorations: Cell::new(list),
        }));
        keep = Some(NodeRc::from_node(&gc, prog));
    }
    {
        let gc = GCLock::new(&mut ctx);
        gc.ctx_gc(); // expose a gc trigger; or call ctx.gc() before locking
        // Walk the rooted program's decoration list; the node must still be 42.0.
        let prog = keep.as_ref().unwrap().node(&gc);
        if let Node::Program(p) = prog {
            let mut it = p.decorations.get().iter();
            let d = it.next().expect("decoration survived gc");
            assert!(matches!(d, Node::NumericLiteral(n) if n.value.get() == 42.0));
        } else { panic!() }
    }
    drop(keep);
}
```

  This requires `NodeList::from_iter(gc, iter)` (port juno's `node_child.rs:117–140`, using `gc.append_list_element`) and a way to trigger `gc()` (e.g. `Context::gc(&mut self)` is already public — call it between locks, not via a lock). Adjust the test to call `ctx.gc()` between the two lock scopes rather than inside a lock.

- [ ] **Step 3:** Run: `cargo test --manifest-path rust/Cargo.toml -p ast --test spine` 
Expected: PASS (`rebuild_then_gc_reclaims_orphans`, `gc_traces_decoration_lists`).

- [ ] **Step 4:** `cargo build --manifest-path rust/Cargo.toml` → **zero warnings** across the workspace. `cargo test --manifest-path rust/Cargo.toml` → whole workspace green.

- [ ] **Step 5:** Commit: `rust(ast): spine tests — functional rebuild, GC reclamation, decoration-list tracing`

---

## Task 5: Update the roadmap

**Files:** `doc/superpowers/RustPortRoadmap.md`.

- [ ] **Step 1:** In the component table, change the `Parser` row's status note or add an `AST` row marked `🚧 in progress — phase 1 (storage/GC spine) done`. Add a short subsection mirroring the lexer's "build log" style noting: `ast` crate up, juno GC arena copied+adapted (our `support`/`atom_table`, `core::mem::offset_of!`, no `source_manager` in Context), minimal 4-kind node model proving deep `match` + `Cell` attrs + immutable children + functional rebuild + GC reclamation + decoration-list tracing; spec at `specs/2026-06-03-ast-design.md`; phase 2 = node-set codegen from `ESTree.def`.

- [ ] **Step 2:** Commit: `doc(rust): roadmap — AST phase 1 (storage/GC spine) complete`

---

## Self-review checklist (run before declaring the plan done)

- **Spec coverage:** storage/GC (§1) ✓ Task 3; child-vs-`Cell` model (spine) ✓ Task 2; no-`Cell<&Node>` + decoration-list tracing (spine §4 / §1) ✓ Tasks 2+4; `NodeList` (§3) ✓ Tasks 2+4; codegen (§2), builders, dumper (§4) — **deferred to phases 2–4** (explicitly out of this plan, noted in the phase roadmap). Validation (§4): unit tests now ✓; differential is Parser-time ✓ (not in scope here).
- **Unsafe boundary:** only `context.rs` (and the test-only `unsafe` in copied `deque.rs`); crate denies `unsafe_code`. ✓
- **Type consistency:** `Node`/`NodeKind`/`NodeMetadata`/`NodeList`/`NodeLabel`/`SemaId`/`Visitor`/`TransformResult`/`GCLock`/`NodeRc`/`Context` names are used consistently across tasks. `gc.atom_bytes`/`SMRange::invalid`/`NodeList::from_iter`/`Context::gc` are flagged where the real `support`/`atom_table` API must be confirmed by reading those crates.
```
