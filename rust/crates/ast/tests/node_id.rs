/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! `NodeId` + freed-id log integration tests
//! (doc/superpowers/specs/2026-07-26-sema-untyped-design.md §3.1). Covers:
//!   1. Uniqueness + monotonicity of ids allocated under one lock
//!   2. Fresh id on rebuild via the generated builder
//!   3. GC sweep logs the freed id exactly once, not the rooted one
//!   4. `AllocationScope` truncation logs the reclaimed ids
//!   5. `NodeMetadata::new` starts `UNASSIGNED`; `alloc` stamps unconditionally

use ast::context::{Context, GCLock, NodeRc};
use ast::node::{BinaryExpression, Node, NumericLiteral};
use ast::node_child::NodeMetadata;
use ast::NodeId;
use std::cell::Cell;

/// Build a dummy source range (no `SMRange::invalid()` on this API).
fn dummy_range() -> support::location::SMRange {
    let l = support::location::SMLoc {
        source: support::location::SourceId::from_index(0),
        offset: 0,
    };
    support::location::SMRange { start: l, end: l }
}

/// Allocate a `NumericLiteral` node with the given value.
fn num<'gc>(gc: &'gc GCLock, v: f64) -> &'gc Node<'gc> {
    gc.alloc(Node::NumericLiteral(NumericLiteral::new(
        NodeMetadata::new(dummy_range()),
        v,
    )))
}

/// Uniqueness + monotonicity: 3 nodes allocated under one lock get
/// distinct, nonzero, increasing ids.
#[test]
fn ids_are_unique_and_monotonic() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);

    let a = num(&gc, 1.0).node_id();
    let b = num(&gc, 2.0).node_id();
    let c = num(&gc, 3.0).node_id();

    assert_ne!(a, NodeId::UNASSIGNED);
    assert_ne!(b, NodeId::UNASSIGNED);
    assert_ne!(c, NodeId::UNASSIGNED);
    assert!(a.0 < b.0, "expected increasing ids: {a:?} < {b:?}");
    assert!(b.0 < c.0, "expected increasing ids: {b:?} < {c:?}");
}

/// Fresh id on rebuild: transforming a node via the generated builder
/// (clone-with-one-field-changed, as tests/transform.rs does) must produce a
/// node with a brand-new id, while the original node's id is untouched.
#[test]
fn builder_rebuild_gets_fresh_id() {
    use ast::node::builder;

    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let one = num(&gc, 1.0);
    let two = num(&gc, 2.0);
    let bin = gc.alloc(Node::BinaryExpression(BinaryExpression::new(
        NodeMetadata::new(dummy_range()),
        one,
        two,
        gc.atom_bytes(b"+"),
    )));
    let old_id = bin.node_id();

    let b = builder::Builder::from_node(bin);
    let new_node = if let builder::Builder::BinaryExpression(mut b) = b {
        let three = num(&gc, 3.0);
        b.left(three);
        match b.build(&gc) {
            ast::visitor::TransformResult::Changed(n) => n,
            other => panic!("expected Changed, got {:?}", other),
        }
    } else {
        panic!("expected Builder::BinaryExpression");
    };

    assert_ne!(new_node.node_id(), old_id, "rebuilt node must get a fresh id");
    assert_eq!(bin.node_id(), old_id, "original node's id must be unchanged");
}

/// GC log: an unrooted node's id is logged exactly once by `gc()`; the
/// rooted node's id is not logged; a second drain returns an empty vec.
#[test]
fn gc_logs_freed_node_id_once_and_not_the_root() {
    let mut ctx = Context::new();
    let root: NodeRc;
    let orphan_id: NodeId;
    let root_id: NodeId;
    {
        let gc = GCLock::new(&mut ctx);
        let orphan = num(&gc, 1.0);
        orphan_id = orphan.node_id();
        let kept = num(&gc, 2.0);
        root_id = kept.node_id();
        root = NodeRc::from_node(&gc, kept);
        // `orphan` has no NodeRc: it becomes unreachable once the lock drops.
    }

    ctx.gc();

    let mut freed = ctx.take_freed_node_ids();
    assert_eq!(
        freed.iter().filter(|&&id| id == orphan_id).count(),
        1,
        "orphan id must be logged exactly once: {freed:?}"
    );
    assert!(
        !freed.contains(&root_id),
        "rooted node's id must not be logged: {freed:?}"
    );
    freed.clear();

    assert_eq!(
        ctx.take_freed_node_ids(),
        Vec::<NodeId>::new(),
        "a second drain must return nothing new"
    );

    // Drop the NodeRc while a lock is held, so Context::drop() doesn't panic.
    {
        let gc2 = GCLock::new(&mut ctx);
        let _ = &gc2;
        drop(root);
    }
}

/// `AllocationScope` log: nodes allocated and reclaimed inside a scope
/// have their ids appended to the freed-id log at scope drop.
#[test]
fn alloc_scope_logs_reclaimed_node_ids() {
    let mut ctx = Context::new();
    let mut scope_ids: Vec<NodeId>;
    {
        let gc = GCLock::new(&mut ctx);
        scope_ids = Vec::new();
        {
            // SAFETY: no reference into the scope's allocations escapes it.
            #[allow(unsafe_code)] // alloc_scope mirrors C++ AllocationScope
            let _scope = unsafe { gc.alloc_scope() };
            let a = num(&gc, 1.0);
            let b = num(&gc, 2.0);
            scope_ids.push(a.node_id());
            scope_ids.push(b.node_id());
            // `_scope` drops here, reclaiming `a` and `b`.
        }
        // `gc` (the GCLock) drops here.
    }

    let mut freed = ctx.take_freed_node_ids();
    freed.sort();
    scope_ids.sort();
    assert_eq!(freed, scope_ids, "AllocationScope drop must log both reclaimed ids");
}

/// `NodeMetadata::new` starts `UNASSIGNED`; `Context::alloc` stamps an id
/// unconditionally once the node is stored in the arena.
#[test]
fn metadata_starts_unassigned_alloc_stamps_unconditionally() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);

    let metadata = NodeMetadata::new(dummy_range());
    assert_eq!(metadata.id.get(), NodeId::UNASSIGNED);

    let n = gc.alloc(Node::NumericLiteral(NumericLiteral {
        metadata,
        value: Cell::new(1.0),
    }));
    assert_ne!(n.node_id(), NodeId::UNASSIGNED, "alloc must stamp a real id");
}
