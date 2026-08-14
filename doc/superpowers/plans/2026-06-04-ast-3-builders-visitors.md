# AST Phase 3 — Builders + transforming visitor (`VisitorMut` / functional rebuild)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the transforming-visitor machinery over the generated 271-node AST: a generated per-kind **`Builder`** (clone-with-one-field-changed — the rebuild primitive), the **`VisitorMut`** trait + full **`TransformResult{Unchanged, Removed, Changed, Expanded}`** + **`Path`/`NodeField`**, the small **`NodeChild`** field trait, and the generated **`Node::visit_children_mut`** driver that functionally rebuilds a node when (and only when) a child changes.

**Architecture:** Port juno's `VisitorMut`/builder model (`unsupported/juno/crates/juno_ast/src/{visitor.rs,node_child.rs,kind.rs}`) to our model, with the one structural difference that **our node attributes are `Cell<…>` and children are immutable `&'gc`/`Option`/`NodeList`** — so the builder copies children by reference and copies `Cell` attribute *values* into fresh `Cell`s. Builders + `visit_children_mut` + `NodeField` are **generated** by extending `gen_nodes.py`; `TransformResult`/`Path`/`VisitorMut`/`NodeChild`/`visit_mut` are hand-written. The **read `Visitor` stays exactly as-is** (phase 1; used by the GC marker) — phase 3 adds only the transform side.

**Tech Stack:** Rust 2021 / toolchain 1.96; Python 3 (the committed generator). Crate `rust/crates/ast/`.

**Reference spec:** `doc/superpowers/specs/2026-06-03-ast-design.md` §3 (traversal + builders) and §4 (validation). **juno source of truth (READ THESE):** `unsupported/juno/crates/juno_ast/src/visitor.rs` (`Path`/`TransformResult`/`VisitorMut`), `node_child.rs:234-490` (the `NodeChild` trait + `&Node`/`Option`/`NodeList` impls + `Node::visit_mut`), `kind.rs:118-160` (`visit_children_mut`) + `:360-470` (the `builder` module).

---

## Scope notes (deliberate decisions — read before starting)

1. **Full juno transform surface** (user-confirmed): `TransformResult{Unchanged, Removed, Changed(T), Expanded(Vec<T>)}`; `Path{parent, field}` + a generated `NodeField` enum; `VisitorMut`; builders; `visit_children_mut`. List children handle remove/expand (splice); a required single child that is `Removed` is replaced with an `EmptyStatement`; an optional child that is `Removed` becomes `None`; `Expanded` on a single/optional child panics.
2. **Read `Visitor` stays as-is** (user-confirmed). The phase-1 `Visitor` (`visit_node` + `visit_children`, used by the GC marker in `context.rs`) is unchanged. We do NOT add parent/`Path` to the read side. So our `NodeChild` trait carries only the *mutating* `visit_child_mut` + `duplicate` (no read `visit_child`).
3. **Our model vs juno (the key adaptation):** juno node fields are plain mutable values; ours split into **immutable children** (`&'gc Node`, `Option<&'gc Node>`, `NodeList`) and **`Cell` attributes** (everything else, incl. the two decoration `Cell<NodeList>`s). Consequences:
   - The builder's `from_node` copies children by ref/head (`.duplicate()` = identity/Copy) and copies each `Cell` attribute *value* into a fresh `Cell` (`Cell::new(node.field.get())`), and duplicates metadata likewise.
   - The builder's **setters exist only for the structural child fields** (`single`/`opt`/`list`) — those are the only fields `visit_children_mut` rebuilds. `Cell` attributes (incl. the `declist` decoration lists) are mutated in place on the existing node, never via the builder, so they get no setter and are not threaded through `visit_children_mut`.
4. **No `template` module.** juno has a separate `template::Xxx` + `build_template` for fresh construction; our phase-2 `XNode::new(metadata, …)` constructors already cover fresh construction, so we don't port templates. The required-child-`Removed`→`EmptyStatement` replacement uses `gc.alloc(Node::EmptyStatement(EmptyStatement::new(…)))` directly.
5. **`visit_child_mut` routing differs from juno (a correctness improvement):** juno's optional-child impl delegates to the `&Node` impl, which means a `Removed` on an optional child would be turned into an `EmptyStatement` rather than `None`. We instead route the optional impl through `visitor.call(...)` directly so `Removed`→`Changed(None)` works correctly. Documented in Task 3.
6. **Generated output grows; the idempotency guard stays.** `tests/generated_idempotent.rs` continues to assert committed `node.rs` == fresh generator output after each generator change.

---

## File structure

```
rust/crates/ast/
  gen_nodes.py            # +emit_node_field, +emit_builders, +emit_visit_children_mut (+ imports)
  src/
    visitor.rs            # extend TransformResult; add Path + VisitorMut (read Visitor unchanged)
    node_child.rs         # +NodeChild trait + impls (&Node/Option/NodeList) + NodeMetadata::duplicate + Node::visit_mut + empty_statement helper
    node.rs               # REGENERATED: +NodeField enum, +`pub mod builder`, +Node::visit_children_mut, +imports
  tests/
    transform.rs          # NEW — replace/share/remove/expand/required-removed/optional-removed/GC + builder unit tests
```

---

## Reference: the exact field classification the generator already has

`compose_fields` tags each field with `child_kind` ∈ `{"meta","single","opt","list","declist","none"}` and `cell: bool`. For phase 3:
- **Structural children** (get a builder setter; threaded by `visit_children_mut`; contribute a `NodeField` variant): `child_kind ∈ {single, opt, list}`.
- **Copied-but-not-threaded** (copied in `from_node`, no setter): `meta` (metadata), `declist` (`Cell<NodeList>` decorations), and every value/decoration `Cell` field (`cell == True`).

`new_arg_type`/`rust_type` per child kind:
| child_kind | `rust_type` | builder setter arg type | `from_node` copy |
|---|---|---|---|
| `single` | `&'gc Node<'gc>` | `&'gc Node<'gc>` | `node.FIELD` |
| `opt` | `Option<&'gc Node<'gc>>` | `Option<&'gc Node<'gc>>` | `node.FIELD` |
| `list` | `NodeList<'gc>` | `NodeList<'gc>` | `node.FIELD` |
| `declist` | `Cell<NodeList<'gc>>` | — (no setter) | `Cell::new(node.FIELD.get())` |
| value `cell` | `Cell<…>` | — (no setter) | `Cell::new(node.FIELD.get())` |
| `meta` | `NodeMetadata<'gc>` | — (no setter) | `node.metadata.duplicate()` |

---

## Task 1: Generate the `NodeField` enum

**Files:** `rust/crates/ast/gen_nodes.py`, regenerate `rust/crates/ast/src/node.rs`.

`Path{parent, field: NodeField}` (Task 2) needs `NodeField`, and it's derived from node field names, so generate it first.

- [ ] **Step 1: add `emit_node_field` to `gen_nodes.py`.** After the existing emit functions, add an emitter that collects the **distinct `rust_field` names of all `single`/`opt`/`list` child fields across all nodes** (sorted for determinism), and emits:

```rust
/// The name of a structural child field of an AST node (used in `Path`).
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum NodeField {
    alternate,
    argument,
    // ... one variant per distinct structural-child field name, sorted ...
    update,
    value,
}
```
Call it from `generate()` (e.g. right after `emit_node_kind`). Keep names exactly as the `rust_field` (snake, already keyword-escaped — note: a field named `r#await` etc. must appear here as `r#await` too; reuse the same `rust_field` strings).

- [ ] **Step 2: regenerate + build.**
```bash
python3 rust/crates/ast/gen_nodes.py
cargo build --manifest-path rust/Cargo.toml -p ast 2>&1 | tail -5
```
Expected: clean build, zero warnings. `grep -n 'pub enum NodeField' rust/crates/ast/src/node.rs` finds it; `grep -c '^    [a-z]' ` near it is a reasonable count (~100+ distinct field names).

- [ ] **Step 3: idempotency holds.** Run the generator twice; `git diff --stat rust/crates/ast/src/node.rs` after the 2nd run shows no change.

- [ ] **Step 4: commit.**
```bash
git add rust/crates/ast/gen_nodes.py rust/crates/ast/src/node.rs
git commit -m "rust(ast): generate NodeField enum (phase 3 prep)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: `TransformResult` + `Path` + `VisitorMut` (hand-written)

**Files:** `rust/crates/ast/src/visitor.rs`.

The current file has a read `Visitor` (KEEP) and a 2-variant `TransformResult<'gc>` (REPLACE with the generic 4-variant form). Confirm nothing else references the old `TransformResult` (`grep -rn TransformResult rust/crates/ast/` — only `visitor.rs` defines it; it's currently unused elsewhere).

- [ ] **Step 1: rewrite `visitor.rs`** to:

```rust
//! AST traversal.
use crate::node::{Node, NodeField};
use crate::context::GCLock;

/// Read-only visitor. Implementors override `visit_node`; the default recurses.
/// (Unchanged from phase 1 — used by the GC marker in `context.rs`.)
pub trait Visitor<'gc> {
    fn visit_node(&mut self, node: &'gc Node<'gc>) {
        node.visit_children(self);
    }
}

/// The path to the node currently being visited: its parent and the field of
/// the parent it occupies. Mirrors juno's `Path`.
#[derive(Debug, Copy, Clone)]
pub struct Path<'gc> {
    pub parent: &'gc Node<'gc>,
    pub field: NodeField,
}

impl<'gc> Path<'gc> {
    pub fn new(parent: &'gc Node<'gc>, field: NodeField) -> Path<'gc> {
        Path { parent, field }
    }
}

/// What a [`VisitorMut`] did to an element of the AST.
#[derive(Debug)]
pub enum TransformResult<T> {
    /// No change.
    Unchanged,
    /// Remove the element if possible. A required single child that is removed
    /// is replaced with an `EmptyStatement`; an optional child becomes `None`;
    /// a list element is dropped.
    Removed,
    /// Replace the element with the wrapped one.
    Changed(T),
    /// Replace the element with several (only valid inside a `NodeList`).
    Expanded(Vec<T>),
}

/// The transforming visitor. `call` returns how `node` should be transformed.
/// A typical impl matches specific nodes and otherwise recurses+rebuilds via
/// `node.visit_children_mut(ctx, self)`.
pub trait VisitorMut<'gc> {
    fn call(
        &mut self,
        ctx: &'gc GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        path: Option<Path<'gc>>,
    ) -> TransformResult<&'gc Node<'gc>>;
}
```

- [ ] **Step 2: build.**
```bash
cargo build --manifest-path rust/Cargo.toml -p ast 2>&1 | tail -5
```
Expected: clean (TransformResult is now generic and unused; Path/VisitorMut compile against the generated `NodeField`). Zero warnings. (`GCLock` is `crate::context::GCLock`; confirm the import path — it's `pub struct GCLock<'ast, 'ctx>`, hence `GCLock<'_, '_>`.)

- [ ] **Step 3: commit.**
```bash
git add rust/crates/ast/src/visitor.rs
git commit -m "rust(ast): TransformResult{Unchanged,Removed,Changed,Expanded} + Path + VisitorMut

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: `NodeChild` trait + impls + `NodeMetadata::duplicate` + `Node::visit_mut` (hand-written)

**Files:** `rust/crates/ast/src/node_child.rs`.

This is the field-level transform mechanism the generated `visit_children_mut` (Task 4) and builder (`from_node`) rely on. Add to `node_child.rs` (which already imports `Cell`, `GCLock`, `Node`, `NodeListElement`, `SMRange`).

- [ ] **Step 1: add `NodeMetadata::duplicate`.** In the existing `impl<'gc> NodeMetadata<'gc>` block:

```rust
    /// Deep-copy the metadata, copying `Cell` values into fresh `Cell`s.
    /// Used by builders when cloning a node.
    pub(crate) fn duplicate(&self) -> NodeMetadata<'gc> {
        NodeMetadata {
            phantom: self.phantom,
            range: Cell::new(self.range.get()),
            parens: Cell::new(self.parens.get()),
        }
    }
```

- [ ] **Step 2: add imports + the `NodeChild` trait + impls + `visit_mut`.** Add near the top: `use crate::visitor::{Path, TransformResult, VisitorMut};` and `use crate::node::{EmptyStatement, NodeField};` (NodeField only if referenced; `EmptyStatement` is needed). Then add:

```rust
/// Build a zero-width `EmptyStatement` at the start of `at`'s range, used to
/// replace a required single child that a `VisitorMut` asked to remove.
fn empty_statement<'gc>(gc: &'gc GCLock<'_, '_>, at: support::location::SMRange) -> &'gc Node<'gc> {
    let range = support::location::SMRange { start: at.start, end: at.start };
    gc.alloc(Node::EmptyStatement(EmptyStatement::new(NodeMetadata::new(range))))
}

/// The mutating field-transform trait. Implemented for the three structural
/// child field types. `visit_child_mut` transforms a child (recursing via
/// `visitor.call`); `duplicate` clones a child field without `Clone` (so callers
/// can't fabricate `Node` refs).
pub(crate) trait NodeChild<'gc>: Sized {
    type Out;
    fn visit_child_mut<V: VisitorMut<'gc>>(
        self,
        ctx: &'gc GCLock<'_, '_>,
        visitor: &mut V,
        path: Path<'gc>,
    ) -> TransformResult<Self::Out>;
    fn duplicate(self) -> Self::Out;
}

impl<'gc> NodeChild<'gc> for &'gc Node<'gc> {
    type Out = &'gc Node<'gc>;
    fn visit_child_mut<V: VisitorMut<'gc>>(
        self,
        ctx: &'gc GCLock<'_, '_>,
        visitor: &mut V,
        path: Path<'gc>,
    ) -> TransformResult<Self::Out> {
        match visitor.call(ctx, self, Some(path)) {
            // A required child cannot be null: removing it yields an EmptyStatement.
            TransformResult::Removed => TransformResult::Changed(empty_statement(ctx, self.range())),
            TransformResult::Expanded(_) => {
                panic!("cannot expand a single required child into multiple nodes")
            }
            other => other,
        }
    }
    fn duplicate(self) -> Self::Out {
        self
    }
}

impl<'gc> NodeChild<'gc> for Option<&'gc Node<'gc>> {
    type Out = Option<&'gc Node<'gc>>;
    fn visit_child_mut<V: VisitorMut<'gc>>(
        self,
        ctx: &'gc GCLock<'_, '_>,
        visitor: &mut V,
        path: Path<'gc>,
    ) -> TransformResult<Self::Out> {
        use TransformResult::*;
        match self {
            None => Unchanged,
            // Route through visitor.call directly (NOT the &Node impl) so that a
            // Removed on an optional child becomes None, not an EmptyStatement.
            Some(inner) => match visitor.call(ctx, inner, Some(path)) {
                Unchanged => Unchanged,
                Removed => Changed(None),
                Changed(new_node) => Changed(Some(new_node)),
                Expanded(_) => panic!("cannot expand a single optional child into multiple nodes"),
            },
        }
    }
    fn duplicate(self) -> Self::Out {
        self
    }
}

impl<'gc> NodeChild<'gc> for NodeList<'gc> {
    type Out = NodeList<'gc>;
    fn visit_child_mut<V: VisitorMut<'gc>>(
        self,
        ctx: &'gc GCLock<'_, '_>,
        visitor: &mut V,
        path: Path<'gc>,
    ) -> TransformResult<Self::Out> {
        use TransformResult::*;
        let mut index = 0;
        let mut it = self.iter();
        // Fast path: assume no change until the first element that changes.
        while let Some(elem) = it.next() {
            let res = visitor.call(ctx, elem, Some(path));
            if let Unchanged = res {
                index += 1;
                continue;
            }
            // First change found: copy the unchanged prefix, then this element,
            // then the rest, and rebuild the list.
            let mut result: Vec<&'gc Node<'gc>> = self.iter().take(index).collect();
            match res {
                Changed(new_node) => result.push(new_node),
                Expanded(new_nodes) => result.extend(new_nodes),
                Removed => {}
                Unchanged => unreachable!("checked above"),
            }
            for elem in it.by_ref() {
                match visitor.call(ctx, elem, Some(path)) {
                    Unchanged => result.push(elem),
                    Changed(new_node) => result.push(new_node),
                    Expanded(new_nodes) => result.extend(new_nodes),
                    Removed => {}
                }
            }
            return Changed(NodeList::from_iter(ctx, result));
        }
        Unchanged
    }
    fn duplicate(self) -> Self::Out {
        self
    }
}

impl<'gc> Node<'gc> {
    /// Top-level transforming entry point. Returns the (maybe-new) root, or
    /// `None` if it was removed.
    pub fn visit_mut<V: VisitorMut<'gc>>(
        &'gc self,
        ctx: &'gc GCLock<'_, '_>,
        visitor: &mut V,
        path: Option<Path<'gc>>,
    ) -> Option<&'gc Node<'gc>> {
        match visitor.call(ctx, self, path) {
            TransformResult::Unchanged => Some(self),
            TransformResult::Removed => None,
            TransformResult::Changed(new_node) => Some(new_node),
            TransformResult::Expanded(_) => panic!("cannot expand the root node into multiple"),
        }
    }
}
```

Notes for the implementer: `NodeList::from_iter(ctx, iter)` already exists and takes `&GCLock` + `IntoIterator<Item = &Node>`. `self.range()` is `Node::range() -> SMRange`. The three child field types (`&Node`, `Option<&Node>`, `NodeList`) are all `Copy`, so `visit_children_mut` (Task 4) can pass `builder.inner.FIELD` by value.

- [ ] **Step 3: build.**
```bash
cargo build --manifest-path rust/Cargo.toml -p ast 2>&1 | tail -8
```
Expected: clean, zero warnings. (`NodeChild::visit_child_mut` is `pub(crate)` and unused until Task 4 — that's fine, no dead-code warning for trait methods. If the `empty_statement` helper warns unused, it will be used in Task 4's generated code; to keep this commit warning-free, add `#[allow(dead_code)]` on `empty_statement` and the `NodeChild` impls' methods are reached via the trait so no warning. Verify with the build — if `empty_statement` warns, gate it `#[allow(dead_code)] // used by generated visit_children_mut (Task 4)`.)

- [ ] **Step 4: commit.**
```bash
git add rust/crates/ast/src/node_child.rs
git commit -m "rust(ast): NodeChild transform trait + impls, NodeMetadata::duplicate, Node::visit_mut

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Generate the `builder` module + `Node::visit_children_mut`

**Files:** `rust/crates/ast/gen_nodes.py`, regenerate `rust/crates/ast/src/node.rs`.

- [ ] **Step 1: add the new imports** to the generated `node.rs` header (in the generator's header emission). The full import block becomes:
```rust
use std::cell::Cell;
use crate::node_child::{NodeChild, NodeLabel, NodeList, NodeMetadata, NodeString, Strictness, INVALID_LABEL};
use crate::visitor::{Path, TransformResult, Visitor, VisitorMut};
use crate::SemaId;
```

- [ ] **Step 2: add `emit_visit_children_mut(nodes, out)`** — a method inside the big `impl<'gc> Node<'gc>` block. For each node, build a `builder::Builder::from_node(self)`, thread each **structural child** field (`single`/`opt`/`list`) through `visit_child_mut`, calling the setter on `Changed`, then `build`. Worked examples (match these exactly):

```rust
    /// Transform this node's children with `visitor`, rebuilding it only if a
    /// child changed. `self` is the original parent.
    pub fn visit_children_mut<V: VisitorMut<'gc>>(
        &'gc self,
        ctx: &'gc crate::context::GCLock<'_, '_>,
        visitor: &mut V,
    ) -> TransformResult<&'gc Node<'gc>> {
        let builder = builder::Builder::from_node(self);
        #[allow(unused_mut)]
        match builder {
            // ForStatement: structural children init(opt), test(opt), update(opt), body(single)
            builder::Builder::ForStatement(mut b) => {
                if let TransformResult::Changed(v) =
                    b.inner.init.visit_child_mut(ctx, visitor, Path::new(self, NodeField::init)) {
                    b.init(v);
                }
                if let TransformResult::Changed(v) =
                    b.inner.test.visit_child_mut(ctx, visitor, Path::new(self, NodeField::test)) {
                    b.test(v);
                }
                if let TransformResult::Changed(v) =
                    b.inner.update.visit_child_mut(ctx, visitor, Path::new(self, NodeField::update)) {
                    b.update(v);
                }
                if let TransformResult::Changed(v) =
                    b.inner.body.visit_child_mut(ctx, visitor, Path::new(self, NodeField::body)) {
                    b.body(v);
                }
                b.build(ctx)
            }
            // NumericLiteral: no structural children -> always Unchanged
            builder::Builder::NumericLiteral(mut b) => b.build(ctx),
            // ... one arm per node; nodes whose only node-bearing fields are
            //     `declist` decoration lists also have NO threaded children
            //     (declists are Cell, mutated in place) -> just `b.build(ctx)`.
        }
    }
```
Rules: thread fields with `child_kind ∈ {single, opt, list}` in declared order; emit `b.build(ctx)` for nodes with none. Do NOT thread `declist`/value/`meta` fields. `b.inner.FIELD` is `Copy`, so pass by value (no `&`).

- [ ] **Step 3: add `emit_builders(nodes, out)`** — emit `pub mod builder { … }` containing the `Builder` enum + one builder struct per node. Worked examples:

```rust
pub mod builder {
    use std::cell::Cell;
    use super::{Node, ForStatement /* …all node structs… */};
    use crate::node_child::{NodeChild, NodeList, NodeMetadata};
    use crate::visitor::TransformResult;

    /// One builder per node kind; clone-with-one-field-changed.
    #[derive(Debug)]
    pub enum Builder<'gc> {
        Empty(self::Empty<'gc>),
        // ... one variant per node ...
        ForStatement(self::ForStatement<'gc>),
        // ...
    }

    impl<'gc> Builder<'gc> {
        pub fn from_node(node: &'gc Node<'gc>) -> Self {
            match node {
                Node::Empty(n) => Builder::Empty(self::Empty::from_node(n)),
                // ...
                Node::ForStatement(n) => Builder::ForStatement(self::ForStatement::from_node(n)),
                // ...
            }
        }
    }

    // Example builder struct (ForStatement):
    #[derive(Debug)]
    pub struct ForStatement<'gc> {
        is_changed: bool,
        pub(super) inner: super::ForStatement<'gc>,
    }
    impl<'gc> ForStatement<'gc> {
        pub fn from_node(node: &'gc super::ForStatement<'gc>) -> Self {
            Self {
                is_changed: false,
                inner: super::ForStatement {
                    metadata: node.metadata.duplicate(),            // NodeMetadata::duplicate
                    init: node.init.duplicate(),                    // single/opt/list children: NodeChild::duplicate
                    test: node.test.duplicate(),
                    update: node.update.duplicate(),
                    body: node.body.duplicate(),
                    label_index: Cell::new(node.label_index.get()), // Cell attrs: copy value
                    scope: Cell::new(node.scope.get()),
                },
            }
        }
        pub fn build(self, gc: &'gc super::super::context::GCLock<'_, '_>) -> TransformResult<&'gc Node<'gc>> {
            if self.is_changed {
                TransformResult::Changed(self.build_forced(gc))
            } else {
                TransformResult::Unchanged
            }
        }
        pub fn build_forced(self, gc: &'gc super::super::context::GCLock<'_, '_>) -> &'gc Node<'gc> {
            gc.alloc(Node::ForStatement(self.inner))
        }
        // Setters ONLY for structural children:
        pub fn init(&mut self, init: Option<&'gc Node<'gc>>) { self.is_changed = true; self.inner.init = init; }
        pub fn test(&mut self, test: Option<&'gc Node<'gc>>) { self.is_changed = true; self.inner.test = test; }
        pub fn update(&mut self, update: Option<&'gc Node<'gc>>) { self.is_changed = true; self.inner.update = update; }
        pub fn body(&mut self, body: &'gc Node<'gc>) { self.is_changed = true; self.inner.body = body; }
    }
    // ... one struct + impl per node ...
}
```
`from_node` per field: `meta`→`node.metadata.duplicate()` (the `NodeMetadata::duplicate` method); `single`/`opt`/`list`→`node.FIELD.duplicate()` (the `NodeChild::duplicate` trait method — this is what makes that trait method live, so the Task-3 `#[allow(dead_code)]` can be removed in Step 5b); `declist`/value `cell`→`Cell::new(node.FIELD.get())`. Setters only for `single`/`opt`/`list` (arg type = the field's `rust_type`). For a node with NO fields beyond metadata (e.g. `Empty`, `EmptyStatement`), `from_node` sets only `metadata`, and there are no setters. The `GCLock` path inside the `builder` submodule is `super::super::context::GCLock` (builder is `node::builder`, so `super` = `node`, `super::super` = crate root → `crate::context::GCLock`; prefer emitting `crate::context::GCLock` for clarity). Use `crate::context::GCLock<'_, '_>` in the emitted code.

- [ ] **Step 4: wire `generate()`** to call `emit_visit_children_mut` (inside the `impl Node` emission, alongside `visit_children`/`mark_lists`) and `emit_builders` (after the `impl Node` block, at module level). Keep `EXPECTED_NODES = 271`.

- [ ] **Step 5: regenerate + build the whole crate + tests.**
```bash
python3 rust/crates/ast/gen_nodes.py
cargo build --manifest-path rust/Cargo.toml -p ast 2>&1 | tail -10
cargo test --manifest-path rust/Cargo.toml -p ast 2>&1 | grep -E 'test result|warning|error'
```
Expected: clean build (zero warnings), existing tests still pass. If the generated code fails to compile, fix `gen_nodes.py` (NOT `node.rs`) and regenerate. Likely fix points: the `GCLock` path; the `builder` module's `use super::{…all node names…}` (emit `use super::*;` to import every node struct + `Node` simply); setter arg types matching `rust_type`.

- [ ] **Step 5b: remove the now-unused-no-more `#[allow(dead_code)]` from `node_child.rs`.** Task 3 added three `#[allow(dead_code)]` (on the `NodeChild` trait, `NodeMetadata::duplicate`, and the `empty_statement` fn) because nothing used them yet. After this task's generated `builder`/`visit_children_mut` land, all three are live (`from_node` calls `.duplicate()`; `visit_children_mut` calls `visit_child_mut`; the `&Node` impl calls `empty_statement`). Delete those three `#[allow(dead_code)]` lines and rebuild — it must stay **zero warnings** without them (proving they're genuinely used now). If any is still flagged unused, that's a real signal the generated code isn't exercising it — investigate rather than re-adding the allow.

- [ ] **Step 6: idempotency + drift guard.**
```bash
python3 rust/crates/ast/gen_nodes.py    # run twice
REQUIRE_GEN=1 cargo test --manifest-path rust/Cargo.toml -p ast --test generated_idempotent 2>&1 | tail -5
```
Expected: committed `node.rs` byte-identical to fresh output; test passes.

- [ ] **Step 7: commit.**
```bash
git add rust/crates/ast/gen_nodes.py rust/crates/ast/src/node.rs rust/crates/ast/src/node_child.rs
git commit -m "rust(ast): generate builder module + Node::visit_children_mut (functional rebuild)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: Transform tests (`tests/transform.rs`) + builder unit tests

**Files:** Create `rust/crates/ast/tests/transform.rs`.

Cover every `TransformResult` case + sharing + GC. Use the generated `new` constructors to build trees and `builder::Builder` directly for the unit test. Confirm exact constructor signatures by grepping `node.rs` (e.g. `BlockStatement::new(metadata, body, implicit)`, `Identifier::new(metadata, name, type_annotation, optional)`).

- [ ] **Step 1: write the tests.**

```rust
/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Transforming-visitor (VisitorMut / functional rebuild) tests.

use ast::context::{Context, GCLock, NodeRc};
use ast::node::*;
use ast::node_child::{NodeList, NodeMetadata};
use ast::visitor::{Path, TransformResult, VisitorMut};
use std::cell::Cell;

fn r() -> support::location::SMRange {
    let l = support::location::SMLoc { source: support::location::SourceId::from_index(0), offset: 0 };
    support::location::SMRange { start: l, end: l }
}
fn num<'gc>(gc: &'gc GCLock, v: f64) -> &'gc Node<'gc> {
    gc.alloc(Node::NumericLiteral(NumericLiteral::new(NodeMetadata::new(r()), v)))
}
fn block<'gc>(gc: &'gc GCLock, body: NodeList<'gc>) -> &'gc Node<'gc> {
    gc.alloc(Node::BlockStatement(BlockStatement::new(NodeMetadata::new(r()), body, false)))
}
fn expr_stmt<'gc>(gc: &'gc GCLock, e: &'gc Node<'gc>) -> &'gc Node<'gc> {
    gc.alloc(Node::ExpressionStatement(ExpressionStatement::new(NodeMetadata::new(r()), e, None)))
}

/// Doubles every NumericLiteral; recurses+rebuilds otherwise.
struct Double;
impl<'gc> VisitorMut<'gc> for Double {
    fn call(&mut self, gc: &'gc GCLock<'_, '_>, node: &'gc Node<'gc>, _p: Option<Path<'gc>>)
        -> TransformResult<&'gc Node<'gc>> {
        match node {
            Node::NumericLiteral(n) => TransformResult::Changed(num(gc, n.value.get() * 2.0)),
            _ => node.visit_children_mut(gc, self),
        }
    }
}

#[test]
fn changed_rebuilds_ancestors_and_shares_unchanged() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let one = num(&gc, 1.0);
    let two = num(&gc, 2.0);
    let bin = gc.alloc(Node::BinaryExpression(BinaryExpression::new(
        NodeMetadata::new(r()), one, two, gc.atom_bytes("+".as_bytes()))));
    let out = bin.visit_mut(&gc, &mut Double, None).unwrap();
    // Rebuilt: both literals doubled.
    if let Node::BinaryExpression(b) = out {
        assert!(matches!(b.left, Node::NumericLiteral(n) if n.value.get() == 2.0));
        assert!(matches!(b.right, Node::NumericLiteral(n) if n.value.get() == 4.0));
    } else { panic!() }
    // A new node was allocated (functional rebuild), original untouched.
    assert!(!std::ptr::eq(out, bin));
    if let Node::BinaryExpression(b) = bin {
        assert!(matches!(b.left, Node::NumericLiteral(n) if n.value.get() == 1.0));
    } else { panic!() }
}

/// Returns Unchanged for everything.
struct Noop;
impl<'gc> VisitorMut<'gc> for Noop {
    fn call(&mut self, gc: &'gc GCLock<'_, '_>, node: &'gc Node<'gc>, _p: Option<Path<'gc>>)
        -> TransformResult<&'gc Node<'gc>> {
        node.visit_children_mut(gc, self)
    }
}

#[test]
fn unchanged_tree_is_shared_pointer_identical() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let bin = gc.alloc(Node::BinaryExpression(BinaryExpression::new(
        NodeMetadata::new(r()), num(&gc, 1.0), num(&gc, 2.0), gc.atom_bytes("+".as_bytes()))));
    let out = bin.visit_mut(&gc, &mut Noop, None).unwrap();
    assert!(std::ptr::eq(out, bin), "unchanged tree must be shared, not rebuilt");
}

/// Removes any ExpressionStatement whose expression is NumericLiteral == 0.
struct RemoveZeros;
impl<'gc> VisitorMut<'gc> for RemoveZeros {
    fn call(&mut self, gc: &'gc GCLock<'_, '_>, node: &'gc Node<'gc>, _p: Option<Path<'gc>>)
        -> TransformResult<&'gc Node<'gc>> {
        if let Node::ExpressionStatement(e) = node {
            if let Node::NumericLiteral(n) = e.expression {
                if n.value.get() == 0.0 { return TransformResult::Removed; }
            }
        }
        node.visit_children_mut(gc, self)
    }
}

#[test]
fn removed_drops_list_element() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let body = NodeList::from_iter(&gc, [
        expr_stmt(&gc, num(&gc, 1.0)),
        expr_stmt(&gc, num(&gc, 0.0)),   // removed
        expr_stmt(&gc, num(&gc, 2.0)),
    ]);
    let blk = block(&gc, body);
    let out = blk.visit_mut(&gc, &mut RemoveZeros, None).unwrap();
    if let Node::BlockStatement(b) = out {
        let vals: Vec<f64> = b.body.iter().map(|s| match s {
            Node::ExpressionStatement(e) => match e.expression {
                Node::NumericLiteral(n) => n.value.get(), _ => panic!() }, _ => panic!() }).collect();
        assert_eq!(vals, vec![1.0, 2.0], "the zero statement must be removed");
    } else { panic!() }
}

/// Expands an ExpressionStatement(NumericLiteral==9) into two copies.
struct ExpandNines;
impl<'gc> VisitorMut<'gc> for ExpandNines {
    fn call(&mut self, gc: &'gc GCLock<'_, '_>, node: &'gc Node<'gc>, _p: Option<Path<'gc>>)
        -> TransformResult<&'gc Node<'gc>> {
        if let Node::ExpressionStatement(e) = node {
            if let Node::NumericLiteral(n) = e.expression {
                if n.value.get() == 9.0 {
                    return TransformResult::Expanded(vec![
                        expr_stmt(gc, num(gc, 9.0)), expr_stmt(gc, num(gc, 9.0))]);
                }
            }
        }
        node.visit_children_mut(gc, self)
    }
}

#[test]
fn expanded_splices_list() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let body = NodeList::from_iter(&gc, [expr_stmt(&gc, num(&gc, 9.0)), expr_stmt(&gc, num(&gc, 1.0))]);
    let blk = block(&gc, body);
    let out = blk.visit_mut(&gc, &mut ExpandNines, None).unwrap();
    if let Node::BlockStatement(b) = out {
        assert_eq!(b.body.iter().count(), 3, "9 expands to two, plus the 1 -> three elements");
    } else { panic!() }
}

/// Removes the test of an IfStatement (a required single child) -> EmptyStatement? 
/// IfStatement.test is required; removing it must yield an EmptyStatement in its place.
struct RemoveIfTest;
impl<'gc> VisitorMut<'gc> for RemoveIfTest {
    fn call(&mut self, gc: &'gc GCLock<'_, '_>, node: &'gc Node<'gc>, p: Option<Path<'gc>>)
        -> TransformResult<&'gc Node<'gc>> {
        // Remove a NumericLiteral that sits in the `test` field.
        if matches!(node, Node::NumericLiteral(_))
            && matches!(p, Some(path) if path.field == NodeField::test) {
            return TransformResult::Removed;
        }
        node.visit_children_mut(gc, self)
    }
}

#[test]
fn required_child_removed_becomes_empty_statement() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    // IfStatement(test, consequent, alternate?) — confirm arg order from node.rs.
    let test = num(&gc, 1.0);
    let cons = block(&gc, NodeList::empty());
    let if_stmt = gc.alloc(Node::IfStatement(IfStatement::new(
        NodeMetadata::new(r()), test, cons, None)));
    let out = if_stmt.visit_mut(&gc, &mut RemoveIfTest, None).unwrap();
    if let Node::IfStatement(i) = out {
        assert!(matches!(i.test, Node::EmptyStatement(_)),
            "removing a required child replaces it with EmptyStatement");
    } else { panic!() }
}

#[test]
fn gc_reclaims_orphans_after_transform() {
    let mut ctx = Context::new();
    let root: NodeRc;
    {
        let gc = GCLock::new(&mut ctx);
        let bin = gc.alloc(Node::BinaryExpression(BinaryExpression::new(
            NodeMetadata::new(r()), num(&gc, 1.0), num(&gc, 2.0), gc.atom_bytes("+".as_bytes()))));
        let out = bin.visit_mut(&gc, &mut Double, None).unwrap();
        root = NodeRc::from_node(&gc, out);  // root only the transformed tree
        assert!(gc.ctx().num_free_nodes() == 0);
    }
    ctx.gc();
    // Original 1.0, 2.0, bin are orphaned (the doubled tree replaced them).
    assert!(ctx.num_free_nodes() >= 3, "orphaned pre-transform nodes must be reclaimed");
    {
        let gc = GCLock::new(&mut ctx);
        let _ = root.node(&gc);
        drop(root);
    }
}

#[test]
fn builder_clone_with_one_field_changed() {
    use ast::node::builder;
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let one = num(&gc, 1.0);
    let two = num(&gc, 2.0);
    let bin = gc.alloc(Node::BinaryExpression(BinaryExpression::new(
        NodeMetadata::new(r()), one, two, gc.atom_bytes("+".as_bytes()))));
    // Builder with no change -> Unchanged.
    let b0 = builder::Builder::from_node(bin);
    if let builder::Builder::BinaryExpression(b) = b0 {
        assert!(matches!(b.build(&gc), TransformResult::Unchanged));
    } else { panic!() }
    // Builder changing `left` -> Changed(new) sharing `right`.
    let b1 = builder::Builder::from_node(bin);
    if let builder::Builder::BinaryExpression(mut b) = b1 {
        let three = num(&gc, 3.0);
        b.left(three);
        match b.build(&gc) {
            TransformResult::Changed(n) => {
                if let Node::BinaryExpression(nb) = n {
                    assert!(std::ptr::eq(nb.left, three));
                    assert!(std::ptr::eq(nb.right, two), "unchanged field shared");
                } else { panic!() }
            }
            _ => panic!("expected Changed"),
        }
    } else { panic!() }
}
```

- [ ] **Step 2: confirm constructor signatures + run.** Grep `node.rs` for `IfStatement::new`, `ExpressionStatement::new`, `BlockStatement::new` arg orders and adjust the test calls to match (the compiler will pinpoint mismatches). Then:
```bash
cargo test --manifest-path rust/Cargo.toml -p ast --test transform 2>&1 | tail -30
```
Expected: all tests pass (`changed_rebuilds_ancestors_and_shares_unchanged`, `unchanged_tree_is_shared_pointer_identical`, `removed_drops_list_element`, `expanded_splices_list`, `required_child_removed_becomes_empty_statement`, `gc_reclaims_orphans_after_transform`, `builder_clone_with_one_field_changed`).

- [ ] **Step 3: whole workspace green, zero warnings.**
```bash
cargo build --manifest-path rust/Cargo.toml 2>&1 | grep -c '^warning'   # 0
cargo test --manifest-path rust/Cargo.toml 2>&1 | grep 'test result'
```

- [ ] **Step 4: commit.**
```bash
git add rust/crates/ast/tests/transform.rs
git commit -m "rust(ast): VisitorMut transform tests (change/share/remove/expand/required-removed/GC) + builder unit test

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: Update the roadmap

**Files:** `doc/superpowers/RustPortRoadmap.md`.

- [ ] **Step 1:** Update the AST status (table row + the AST subsection): phases 1–3 complete. Phase 3 added the transforming-visitor surface — generated `builder` module (clone-with-one-field-changed; `from_node` copies children by ref and `Cell` attributes by value), `VisitorMut` + `TransformResult{Unchanged,Removed,Changed,Expanded}` + `Path`/generated `NodeField` + the `NodeChild` field trait, and generated `Node::visit_children_mut` (functional rebuild: rebuilds a node only when a child changes; required-child Removed→`EmptyStatement`, optional→`None`, list remove/expand/splice). Read `Visitor` unchanged. Tests in `tests/transform.rs`. Note phase 4 next (`ESTreeJSONDumper` + golden tests), then the Parser.

- [ ] **Step 2: commit.**
```bash
git add doc/superpowers/RustPortRoadmap.md
git commit -m "doc(rust): roadmap — AST phase 3 (builders + VisitorMut) complete

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Self-review checklist (run before declaring the plan done)

- **Spec §3 coverage:** builders (construct via phase-2 `new` + clone-with-change via `Builder`) ✓ Task 4; read `Visitor` (port of RecursiveVisitor) ✓ already present (unchanged); transforming `VisitorMut` (functional rebuild) ✓ Tasks 2-4; `NodeList` rebuild incl. remove/expand ✓ Task 3. Spec §4 validation: transform unit tests over hand-built trees ✓ Task 5 (the byte-for-byte differential is Parser-time, not here).
- **Model fidelity:** the immutable-children-+-`Cell`-attributes split is honored — builder `from_node` copies children by ref, `Cell` attrs by value; setters only for structural children; `declist` decoration lists treated as in-place `Cell` (copied, not threaded). ✓
- **Compile ordering:** NodeField (T1) → visitor.rs uses it (T2) → node_child traits use VisitorMut (T3) → generated builder/vcm use NodeChild+VisitorMut+NodeField (T4). Each task builds clean. ✓
- **Placeholder scan:** hand-written files given in full; generated parts specified by exact worked examples (ForStatement/NumericLiteral/builder struct) + per-field rules; tests given in full (with a noted "confirm constructor arg order" step). ✓
- **Type consistency:** `TransformResult`/`Path`/`VisitorMut`/`NodeChild`/`NodeField`/`Builder`/`visit_children_mut`/`visit_mut`/`duplicate`/`from_node`/`build`/`build_forced` names consistent across Tasks 2-5. `GCLock<'_, '_>` used uniformly. Read `Visitor` untouched (GC marker unaffected). ✓
