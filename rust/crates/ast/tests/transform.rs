/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Transforming-visitor (VisitorMut / functional rebuild) tests.
//!
//! Covers every `TransformResult` case + sharing + GC:
//!   1. `Changed` rebuilds ancestors and shares unchanged subtrees
//!   2. `Unchanged` tree is pointer-identical (shared, not rebuilt)
//!   3. `Removed` drops a list element
//!   4. `Expanded` splices a list element into two
//!   5. Required single child `Removed` → `EmptyStatement`
//!   6. GC reclaims orphans after transform
//!   7. Builder unit test: no change → `Unchanged`; change one field → `Changed`
//!      with the other field pointer-shared

use hermes_ast::context::{Context, GCLock, NodeRc};
use hermes_ast::node::*;
use hermes_ast::node_child::{NodeList, NodeMetadata};
use hermes_ast::visitor::{Path, TransformResult, VisitorMut};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Build a zero-width dummy source range at source-buffer 0, offset 0.
fn r() -> hermes_support::location::SMRange {
    let l = hermes_support::location::SMLoc {
        source: hermes_support::location::SourceId::from_index(0),
        offset: 0,
    };
    hermes_support::location::SMRange { start: l, end: l }
}

/// Allocate a `NumericLiteral` with the given value.
fn num<'gc>(gc: &'gc GCLock, v: f64) -> &'gc Node<'gc> {
    gc.alloc(Node::NumericLiteral(NumericLiteral::new(
        NodeMetadata::new(r()),
        v,
    )))
}

/// Allocate a `BlockStatement` with the given body list.
/// `BlockStatement::new(metadata, body: NodeList, implicit: bool)`
fn block<'gc>(gc: &'gc GCLock, body: NodeList<'gc>) -> &'gc Node<'gc> {
    gc.alloc(Node::BlockStatement(BlockStatement::new(
        NodeMetadata::new(r()),
        body,
        false,
    )))
}

/// Allocate an `ExpressionStatement` wrapping `e`.
/// `ExpressionStatement::new(metadata, expression: &Node, directive: NodeString)`
/// NOTE: `directive` is a plain `NodeString` (not `Option`) — pass an empty interned atom.
fn expr_stmt<'gc>(gc: &'gc GCLock, e: &'gc Node<'gc>) -> &'gc Node<'gc> {
    gc.alloc(Node::ExpressionStatement(ExpressionStatement::new(
        NodeMetadata::new(r()),
        e,
        gc.atom_bytes(b""), // directive: NodeString (== AtomBytes), not Option
    )))
}

// ---------------------------------------------------------------------------
// Test 1: Changed — rebuilds ancestors, shares unchanged subtrees
// ---------------------------------------------------------------------------

/// Doubles every `NumericLiteral`; recurses+rebuilds via `visit_children_mut` otherwise.
struct Double;
impl<'gc> VisitorMut<'gc> for Double {
    fn call(
        &mut self,
        gc: &'gc GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        _p: Option<Path<'gc>>,
    ) -> TransformResult<&'gc Node<'gc>> {
        match node {
            Node::NumericLiteral(n) => {
                TransformResult::Changed(num(gc, n.value.get() * 2.0))
            }
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
        NodeMetadata::new(r()),
        one,
        two,
        gc.atom_bytes(b"+"),
    )));

    let out = bin.visit_mut(&gc, &mut Double, None).unwrap();

    // Rebuilt: both literals doubled.
    if let Node::BinaryExpression(b) = out {
        assert!(
            matches!(b.left, Node::NumericLiteral(n) if n.value.get() == 2.0),
            "left should be doubled to 2.0"
        );
        assert!(
            matches!(b.right, Node::NumericLiteral(n) if n.value.get() == 4.0),
            "right should be doubled to 4.0"
        );
    } else {
        panic!("expected BinaryExpression");
    }

    // A new node was allocated (functional rebuild), original untouched.
    assert!(!std::ptr::eq(out, bin), "transformed tree must be a new allocation");
    if let Node::BinaryExpression(b) = bin {
        assert!(
            matches!(b.left, Node::NumericLiteral(n) if n.value.get() == 1.0),
            "original left must still be 1.0"
        );
        assert!(
            matches!(b.right, Node::NumericLiteral(n) if n.value.get() == 2.0),
            "original right must still be 2.0"
        );
    }
}

// ---------------------------------------------------------------------------
// Test 2: Unchanged — tree is pointer-identical (shared, not rebuilt)
// ---------------------------------------------------------------------------

/// Returns `Unchanged` for every node (recurses but rebuilds nothing).
struct Noop;
impl<'gc> VisitorMut<'gc> for Noop {
    fn call(
        &mut self,
        gc: &'gc GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        _p: Option<Path<'gc>>,
    ) -> TransformResult<&'gc Node<'gc>> {
        node.visit_children_mut(gc, self)
    }
}

#[test]
fn unchanged_tree_is_shared_pointer_identical() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let bin = gc.alloc(Node::BinaryExpression(BinaryExpression::new(
        NodeMetadata::new(r()),
        num(&gc, 1.0),
        num(&gc, 2.0),
        gc.atom_bytes(b"+"),
    )));

    let out = bin.visit_mut(&gc, &mut Noop, None).unwrap();
    assert!(
        std::ptr::eq(out, bin),
        "unchanged tree must be shared (pointer-identical), not rebuilt"
    );
}

// ---------------------------------------------------------------------------
// Test 3: Removed — drops a list element
// ---------------------------------------------------------------------------

/// Removes any `ExpressionStatement` whose expression is `NumericLiteral == 0.0`.
struct RemoveZeros;
impl<'gc> VisitorMut<'gc> for RemoveZeros {
    fn call(
        &mut self,
        gc: &'gc GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        _p: Option<Path<'gc>>,
    ) -> TransformResult<&'gc Node<'gc>> {
        if let Node::ExpressionStatement(e) = node {
            if let Node::NumericLiteral(n) = e.expression {
                if n.value.get() == 0.0 {
                    return TransformResult::Removed;
                }
            }
        }
        node.visit_children_mut(gc, self)
    }
}

#[test]
fn removed_drops_list_element() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);

    // body = [expr(1.0), expr(0.0), expr(2.0)]  — the middle one is removed.
    let body = NodeList::from_iter(
        &gc,
        [
            expr_stmt(&gc, num(&gc, 1.0)),
            expr_stmt(&gc, num(&gc, 0.0)), // should be removed
            expr_stmt(&gc, num(&gc, 2.0)),
        ],
    );
    let blk = block(&gc, body);
    let out = blk.visit_mut(&gc, &mut RemoveZeros, None).unwrap();

    if let Node::BlockStatement(b) = out {
        let vals: Vec<f64> = b
            .body
            .iter()
            .map(|s| match s {
                Node::ExpressionStatement(e) => match e.expression {
                    Node::NumericLiteral(n) => n.value.get(),
                    _ => panic!("expected NumericLiteral"),
                },
                _ => panic!("expected ExpressionStatement"),
            })
            .collect();
        assert_eq!(vals, vec![1.0, 2.0], "the zero statement must be removed");
    } else {
        panic!("expected BlockStatement");
    }
}

// ---------------------------------------------------------------------------
// Test 4: Expanded — splices a list element into several
// ---------------------------------------------------------------------------

/// Expands any `ExpressionStatement(NumericLiteral == 9.0)` into two copies of itself.
struct ExpandNines;
impl<'gc> VisitorMut<'gc> for ExpandNines {
    fn call(
        &mut self,
        gc: &'gc GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        _p: Option<Path<'gc>>,
    ) -> TransformResult<&'gc Node<'gc>> {
        if let Node::ExpressionStatement(e) = node {
            if let Node::NumericLiteral(n) = e.expression {
                if n.value.get() == 9.0 {
                    return TransformResult::Expanded(vec![
                        expr_stmt(gc, num(gc, 9.0)),
                        expr_stmt(gc, num(gc, 9.0)),
                    ]);
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

    // body = [expr(9.0), expr(1.0)]  — 9 expands to two, giving 3 total.
    let body = NodeList::from_iter(
        &gc,
        [expr_stmt(&gc, num(&gc, 9.0)), expr_stmt(&gc, num(&gc, 1.0))],
    );
    let blk = block(&gc, body);
    let out = blk.visit_mut(&gc, &mut ExpandNines, None).unwrap();

    if let Node::BlockStatement(b) = out {
        assert_eq!(
            b.body.iter().count(),
            3,
            "9 expands to two copies, plus the 1 → three elements total"
        );
    } else {
        panic!("expected BlockStatement");
    }
}

// ---------------------------------------------------------------------------
// Test 5: Required single child Removed → EmptyStatement
// ---------------------------------------------------------------------------

/// Removes any `NumericLiteral` that sits in the `test` field of an `IfStatement`.
/// Since `test` is a required single child, the framework must replace it with
/// an `EmptyStatement`.
struct RemoveIfTest;
impl<'gc> VisitorMut<'gc> for RemoveIfTest {
    fn call(
        &mut self,
        gc: &'gc GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        p: Option<Path<'gc>>,
    ) -> TransformResult<&'gc Node<'gc>> {
        if matches!(node, Node::NumericLiteral(_))
            && matches!(p, Some(path) if path.field == NodeField::test)
        {
            return TransformResult::Removed;
        }
        node.visit_children_mut(gc, self)
    }
}

#[test]
fn required_child_removed_becomes_empty_statement() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);

    // IfStatement::new(metadata, test: &Node, consequent: &Node, alternate: Option<&Node>)
    let test_node = num(&gc, 1.0);
    let cons = block(&gc, NodeList::empty());
    let if_stmt = gc.alloc(Node::IfStatement(IfStatement::new(
        NodeMetadata::new(r()),
        test_node,
        cons,
        None,
    )));

    let out = if_stmt.visit_mut(&gc, &mut RemoveIfTest, None).unwrap();
    if let Node::IfStatement(i) = out {
        assert!(
            matches!(i.test, Node::EmptyStatement(_)),
            "removing a required single child must replace it with EmptyStatement, got {:?}",
            i.test
        );
    } else {
        panic!("expected IfStatement");
    }
}

// ---------------------------------------------------------------------------
// Test 6: GC reclaims orphaned nodes after transform
// ---------------------------------------------------------------------------

#[test]
fn gc_reclaims_orphans_after_transform() {
    let mut ctx = Context::new();
    let root: NodeRc;
    {
        let gc = GCLock::new(&mut ctx);
        let bin = gc.alloc(Node::BinaryExpression(BinaryExpression::new(
            NodeMetadata::new(r()),
            num(&gc, 1.0),
            num(&gc, 2.0),
            gc.atom_bytes(b"+"),
        )));
        let out = bin.visit_mut(&gc, &mut Double, None).unwrap();
        // Root only the transformed tree; the originals become orphans.
        root = NodeRc::from_node(&gc, out);
    }
    // Pre-GC: no free nodes yet.
    assert_eq!(ctx.num_free_nodes(), 0);

    ctx.gc();

    // Post-GC: original bin + original 1.0 + original 2.0 must be reclaimed (≥ 3).
    assert!(
        ctx.num_free_nodes() >= 3,
        "orphaned pre-transform nodes must be reclaimed; got {}",
        ctx.num_free_nodes()
    );

    // Confirm the rooted transformed tree is still valid.
    {
        let gc = GCLock::new(&mut ctx);
        let node = root.node(&gc);
        assert!(
            matches!(node, Node::BinaryExpression(_)),
            "rooted transformed tree must still be accessible"
        );
    }
    drop(root);
}

// ---------------------------------------------------------------------------
// Test 7: Builder unit test — clone-with-one-field-changed
// ---------------------------------------------------------------------------

#[test]
fn builder_clone_with_one_field_changed() {
    use hermes_ast::node::builder;

    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let one = num(&gc, 1.0);
    let two = num(&gc, 2.0);
    let bin = gc.alloc(Node::BinaryExpression(BinaryExpression::new(
        NodeMetadata::new(r()),
        one,
        two,
        gc.atom_bytes(b"+"),
    )));

    // Builder with no change → Unchanged.
    let b0 = builder::Builder::from_node(bin);
    if let builder::Builder::BinaryExpression(b) = b0 {
        assert!(
            matches!(b.build(&gc), TransformResult::Unchanged),
            "unmodified builder must yield Unchanged"
        );
    } else {
        panic!("expected Builder::BinaryExpression");
    }

    // Builder changing `left` → Changed(new), with `right` pointer-shared.
    let b1 = builder::Builder::from_node(bin);
    if let builder::Builder::BinaryExpression(mut b) = b1 {
        let three = num(&gc, 3.0);
        b.left(three);
        match b.build(&gc) {
            TransformResult::Changed(n) => {
                if let Node::BinaryExpression(nb) = n {
                    assert!(
                        std::ptr::eq(nb.left, three),
                        "left must be the newly set node"
                    );
                    assert!(
                        std::ptr::eq(nb.right, two),
                        "unchanged right field must be pointer-shared with original"
                    );
                } else {
                    panic!("expected BinaryExpression inside Changed");
                }
            }
            other => panic!("expected Changed, got {:?}", other),
        }
    } else {
        panic!("expected Builder::BinaryExpression");
    }
}
