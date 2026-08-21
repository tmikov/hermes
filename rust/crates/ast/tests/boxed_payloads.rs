/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Lifecycle of the boxed `Node` variants' payloads.
//!
//! The widest few node kinds hold their payload in a side arena rather than
//! inline, so `size_of::<Node>()` — and therefore every node slot — is not set
//! by them. Those payloads carry no mark bit: a payload is owned by exactly one
//! node, so the sweep frees it when it frees that node, and an `AllocationScope`
//! truncates the pools alongside the node deque.
//!
//! Nothing else observes that. Without these tests the free path could be
//! deleted outright and every other suite would still pass — which was true
//! when the pools were first added, and is what these tests exist to prevent.

use hermes_ast::context::{Context, GCLock};
use hermes_ast::node::{BlockStatement, FunctionExpression, Node};
use hermes_ast::node_child::{NodeList, NodeMetadata};
use hermes_support::location::{SMLoc, SMRange, SourceId};

/// A dummy source range; these tests never read locations back.
fn r() -> SMRange {
    let loc = SMLoc {
        source: SourceId::from_index(0),
        offset: 0,
    };
    SMRange {
        start: loc,
        end: loc,
    }
}

/// Allocate one boxed node (a `FunctionExpression`) plus the block it needs.
fn boxed_node<'gc>(gc: &'gc GCLock<'_, '_>) -> &'gc Node<'gc> {
    let body = gc.alloc(Node::BlockStatement(BlockStatement::new(
        NodeMetadata::new(r()),
        NodeList::empty(),
        false,
    )));
    gc.alloc(Node::new_function_expression(
        gc,
        FunctionExpression::new(
            NodeMetadata::new(r()),
            None,              // id
            NodeList::empty(), // params
            body,
            None, // type_parameters
            None, // return_type
            None, // predicate
            false,
            false,
        ),
    ))
}

/// The sweep must return payload slots of collected nodes to their pool, and
/// the next allocation must reuse them rather than grow the pool.
#[test]
fn gc_reclaims_and_reuses_boxed_payloads() {
    let mut ctx = Context::new();
    {
        let gc = GCLock::new(&mut ctx);
        for _ in 0..8 {
            // Unrooted: no NodeRc, unreachable once the lock drops.
            boxed_node(&gc);
        }
    }
    let after_alloc = ctx.num_boxed_payloads();
    assert_eq!(after_alloc, 8, "eight payloads allocated");

    ctx.gc();

    // The pool keeps its slots — freeing returns them to the free list, it
    // does not shrink the deque — so the observable effect is that the next
    // eight allocations reuse them instead of adding more.
    {
        let gc = GCLock::new(&mut ctx);
        for _ in 0..8 {
            boxed_node(&gc);
        }
    }
    assert_eq!(
        ctx.num_boxed_payloads(),
        after_alloc,
        "the sweep must have freed the eight dead payloads for reuse; \
         growth here means they leaked"
    );
}

/// A payload reachable from a root must survive the sweep, and the pool must
/// not hand its slot out again.
#[test]
fn gc_keeps_rooted_boxed_payloads() {
    let mut ctx = Context::new();
    let keep;
    {
        let gc = GCLock::new(&mut ctx);
        keep = hermes_ast::context::NodeRc::from_node(&gc, boxed_node(&gc));
    }
    assert_eq!(ctx.num_boxed_payloads(), 1);

    ctx.gc();

    {
        let gc = GCLock::new(&mut ctx);
        boxed_node(&gc);
    }
    assert_eq!(
        ctx.num_boxed_payloads(),
        2,
        "a rooted payload must not be reused; reuse here means the sweep \
         freed a live payload"
    );
    drop(keep);
}

/// Payloads allocated inside an `AllocationScope` must be dropped with it.
/// Without the pool watermarks they would outlive every node referencing them
/// and never be swept, because the sweep only frees a payload through its
/// owner.
#[test]
fn allocation_scope_truncates_boxed_payloads() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let before = gc.num_boxed_payloads();
    {
        // SAFETY: nothing allocated inside the scope escapes it.
        #[allow(unsafe_code)]
        let _scope = unsafe { gc.alloc_scope() };
        for _ in 0..5 {
            boxed_node(&gc);
        }
        assert_eq!(
            gc.num_boxed_payloads(),
            before + 5,
            "payloads allocated inside the scope"
        );
    }
    assert_eq!(
        gc.num_boxed_payloads(),
        before,
        "the scope must truncate the payload pools, not only the node deque"
    );
}
