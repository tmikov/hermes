/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Integration tests proving the GC arena spine:
//!   1. Functional rebuild + orphan reclamation (rebuild_then_gc_reclaims_orphans)
//!   2. GC traces decoration NodeList (gc_traces_decoration_lists)

use hermes_ast::context::{Context, GCLock, NodeRc};
use hermes_ast::node::{BinaryExpression, Identifier, Node, NumericLiteral, Program};
use hermes_ast::node_child::{NodeList, NodeMetadata};
use std::cell::Cell;

/// Build a dummy source range (no `SMRange::invalid()` on this API).
fn dummy_range() -> hermes_support::location::SMRange {
    let l = hermes_support::location::SMLoc {
        source: hermes_support::location::SourceId::from_index(0),
        offset: 0,
    };
    hermes_support::location::SMRange { start: l, end: l }
}

/// Allocate a `NumericLiteral` node with the given value.
fn num<'gc>(gc: &'gc GCLock, v: f64) -> &'gc Node<'gc> {
    gc.alloc(Node::NumericLiteral(NumericLiteral::new(
        NodeMetadata::new(dummy_range()),
        v,
    )))
}

/// Functional transform: double every `NumericLiteral`, rebuilding ancestor
/// `BinaryExpression` nodes whose children changed. Unchanged subtrees are
/// shared (pointer-identical) rather than copied.
fn double<'gc>(gc: &'gc GCLock, n: &'gc Node<'gc>) -> &'gc Node<'gc> {
    match n {
        Node::NumericLiteral(x) => num(gc, x.value.get() * 2.0),
        Node::BinaryExpression(b) => {
            let l = double(gc, b.left);
            let r = double(gc, b.right);
            if std::ptr::eq(l, b.left) && std::ptr::eq(r, b.right) {
                // Both children are pointer-identical: share this node unchanged.
                n
            } else {
                gc.alloc(Node::BinaryExpression(BinaryExpression {
                    metadata: NodeMetadata::new(dummy_range()),
                    left: l,
                    right: r,
                    operator: Cell::new(b.operator.get()),
                }))
            }
        }
        other => other,
    }
}

/// Allocate an `Identifier` node with the given name.
fn ident<'gc>(gc: &'gc GCLock, name: &str) -> &'gc Node<'gc> {
    gc.alloc(Node::Identifier(Identifier::new(
        NodeMetadata::new(dummy_range()),
        gc.atom_bytes(name.as_bytes()),
        None,
        false,
    )))
}

/// Prove that `double` returns pointer-identical nodes for subtrees that
/// contain no `NumericLiteral`s.  The sharing branch (`std::ptr::eq` guard in
/// `double`) must fire when both children are `Identifier`s.
#[test]
fn double_shares_unchanged_subtrees() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);

    let op = gc.atom_bytes("+".as_bytes());
    let bin = gc.alloc(Node::BinaryExpression(BinaryExpression {
        metadata: NodeMetadata::new(dummy_range()),
        left: ident(&gc, "a"),
        right: ident(&gc, "b"),
        operator: Cell::new(op),
    }));

    let out = double(&gc, bin);

    assert!(
        std::ptr::eq(out, bin),
        "unchanged subtree must be shared (pointer-identical), not rebuilt"
    );
}

/// Prove that after rooting only the new tree and calling `gc()`, the three
/// orphaned old nodes (old 1.0, old 2.0, old `bin`) are reclaimed (moved to
/// the free list), while the new tree (2.0, 4.0, new `bin`) survives intact.
#[test]
fn rebuild_then_gc_reclaims_orphans() {
    let mut ctx = Context::new();

    // Root that escapes the GCLock scope.
    let root: NodeRc;

    {
        let gc = GCLock::new(&mut ctx);

        let op = gc.atom_bytes("+".as_bytes());
        let bin = gc.alloc(Node::BinaryExpression(BinaryExpression {
            metadata: NodeMetadata::new(dummy_range()),
            left: num(&gc, 1.0),
            right: num(&gc, 2.0),
            operator: Cell::new(op),
        }));

        // 3 old nodes allocated so far (1.0, 2.0, bin).
        let new_tree = double(&gc, bin);
        // double allocates 3 more: new 2.0, new 4.0, new bin.

        // Verify the new tree has doubled values.
        if let Node::BinaryExpression(b) = new_tree {
            assert!(
                matches!(b.left, Node::NumericLiteral(n) if n.value.get() == 2.0),
                "expected left child to be 2.0"
            );
            assert!(
                matches!(b.right, Node::NumericLiteral(n) if n.value.get() == 4.0),
                "expected right child to be 4.0"
            );
        } else {
            panic!("expected BinaryExpression from double()");
        }

        // Root ONLY the new tree; old 1.0 / old 2.0 / old bin are now unreachable.
        root = NodeRc::from_node(&gc, new_tree);

        // All 6 node slots must exist before GC.
        // Use gc.ctx() since &mut ctx is borrowed by the GCLock.
        assert_eq!(
            gc.ctx().num_nodes(),
            6,
            "should have 6 allocated node slots before gc (1.0, 2.0, bin, 2.0, 4.0, newbin)"
        );
        assert_eq!(
            gc.ctx().num_free_nodes(),
            0,
            "free list should be empty before gc"
        );
        // GCLock drops here.
    }

    // Call gc() between lock scopes (requires &mut Context).
    ctx.gc();

    // The 3 unreachable old nodes must now be on the free list.
    assert_eq!(
        ctx.num_free_nodes(),
        3,
        "gc() should have reclaimed exactly 3 orphaned nodes"
    );
    // Total slots are unchanged (gc() never shrinks the Deque).
    assert_eq!(ctx.num_nodes(), 6, "total slot count unchanged after gc");

    // Re-lock and verify the surviving tree is still correct.
    {
        let gc2 = GCLock::new(&mut ctx);
        let surviving = root.node(&gc2);
        if let Node::BinaryExpression(b) = surviving {
            assert!(
                matches!(b.left, Node::NumericLiteral(n) if n.value.get() == 2.0),
                "surviving left child must still be 2.0"
            );
            assert!(
                matches!(b.right, Node::NumericLiteral(n) if n.value.get() == 4.0),
                "surviving right child must still be 4.0"
            );
        } else {
            panic!("surviving root must be a BinaryExpression");
        }
        // Drop root while the lock is held so Context::drop() doesn't panic.
        drop(root);
    }
}

/// Prove that the GC marker walks `Program.decorations` (a Cell<NodeList>).
/// A node reachable ONLY through the decoration list must survive a GC cycle.
#[test]
fn gc_traces_decoration_lists() {
    let mut ctx = Context::new();

    let keep: NodeRc;

    {
        let gc = GCLock::new(&mut ctx);

        // The decorated node is reachable only via `decorations`, not via `body`.
        let dec = num(&gc, 42.0);
        let list = NodeList::from_iter(&gc, [dec]);
        // Build the Program with the generated constructor (defaults decorations to
        // empty), then set the decoration list before rooting.
        let prog_node = Program::new(NodeMetadata::new(dummy_range()), NodeList::empty());
        prog_node.decorations.set(list);
        let prog = gc.alloc(Node::Program(prog_node));

        keep = NodeRc::from_node(&gc, prog);
        // GCLock drops here.
    }

    // Trigger a full GC between lock scopes.
    ctx.gc();

    // Non-vacuous check: prog + its decoration-list element + the decorated node are
    // ALL reachable, so a correct marker collects NOTHING. If the marker failed to
    // walk Program.decorations, `dec` (and/or its list element) would be freed and
    // this would be >= 1. This is what actually proves decoration-list tracing.
    assert_eq!(
        ctx.num_free_nodes(),
        0,
        "nothing is unreachable; a nonzero free count means the marker didn't trace the decoration list"
    );

    {
        let gc2 = GCLock::new(&mut ctx);
        let prog_node = keep.node(&gc2);

        if let Node::Program(p) = prog_node {
            let mut it = p.decorations.get().iter();
            let d = it.next().expect("decoration node must survive gc");
            assert!(
                matches!(d, Node::NumericLiteral(n) if n.value.get() == 42.0),
                "decoration node value must still be 42.0 after gc"
            );
            assert!(it.next().is_none(), "only one decoration expected");
        } else {
            panic!("rooted node must be a Program");
        }

        // Drop keep while the lock is held.
        drop(keep);
    }
}
