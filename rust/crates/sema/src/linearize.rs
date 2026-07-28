/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Port of the operator-expression linearization helpers of
//! `namespace ESTree` (`include/hermes/AST/ESTree.h:1405-1477`):
//! `MAX_NESTED_ASSIGNMENTS`, `MAX_NESTED_BINARY`, `checkExprOperator`,
//! `linearizeLeft` and `linearizeRight`.
//!
//! They exist so that the deeply left-nested `a + b + c + ...` and
//! right-nested `a = b = c = ...` chains a real program can contain are
//! walked *iteratively* rather than recursively, which is why their limit is
//! `MAX_NESTED_*` (30000) rather than the recursive
//! `ESTree::kASTMaxRecursionDepth` (1024).
//!
//! ## Deviations from the C++
//!
//! - **Location.** C++ declares these in `ESTree.h`, i.e. what this port
//!   calls the `ast` crate. They live here instead because both consumers
//!   are in `sema` — `sema::dump`'s `ASTPrinter` (`SemResolve.cpp:76`) and
//!   `SemanticResolver`'s `visit(BinaryExpressionNode *)` /
//!   `visit(AssignmentExpressionNode *)` (`SemanticResolver.cpp:410,441`) —
//!   and a single crate-private copy is what keeps the two from drifting.
//!   Nothing here depends on `sema`; if IRGen (`ESTreeIRGen-expr.cpp:2620`,
//!   `:2814`, the remaining C++ callers) is ever ported, this module moves
//!   to `ast` unchanged.
//! - **`ops` is a list of interned atoms, not of strings.** C++ compares
//!   `n->_operator->str()` against an `ArrayRef<StringRef>`. Operators are
//!   always interned through the same `AtomTable` (the lexer's operator
//!   table), so comparing `AtomBytes` identity is both cheaper and exactly
//!   equivalent — an operator can't be spelled two different ways.
//! - **The `N` template parameter becomes the [`OperatorExpr`] trait.** C++
//!   instantiates these templates at `BinaryExpressionNode` (ops `{+,-}`)
//!   and `AssignmentExpressionNode` (ops `{=}`), relying on both having
//!   `_left`, `_right` and `_operator`; the trait spells that structural
//!   requirement out, plus the `dyn_cast<N>` that `checkExprOperator`
//!   performs.

use ast::node::{AssignmentExpression, BinaryExpression, Node};
use ast::node_child::NodeLabel;

/// An arbitrary limit to nested assignments. We handle them non-recursively,
/// so this can be very large, but we don't want to let it consume all our
/// memory. Port of `ESTree::MAX_NESTED_ASSIGNMENTS` (ESTree.h:1407).
///
/// Read by `visit(AssignmentExpressionNode *)` (SemanticResolver.cpp:442),
/// which is a later S1 task; defined here with its neighbor so that the two
/// limits stay together the way the C++ header has them.
#[allow(dead_code)]
pub(crate) const MAX_NESTED_ASSIGNMENTS: u32 = 30000;

/// An arbitrary limit to nested "+/-" binary expressions. We handle them
/// non-recursively, so this can be very large, but we don't want to let it
/// consume all our memory. Port of `ESTree::MAX_NESTED_BINARY`
/// (ESTree.h:1412).
///
/// Read by `visit(BinaryExpressionNode *)` (SemanticResolver.cpp:411), which
/// is a later S1 task.
#[allow(dead_code)]
pub(crate) const MAX_NESTED_BINARY: u32 = 30000;

/// A binary-shaped expression node that [`linearize_left`] /
/// [`linearize_right`] can chain through: it has a `_left`, a `_right` and
/// an `_operator`, and can be recognized among the `Node` variants. See the
/// module doc — this is the C++ templates' implicit `N` requirement.
pub(crate) trait OperatorExpr<'gc>: Sized {
    /// Port of `llvh::dyn_cast<N>(e)` inside `checkExprOperator`
    /// (ESTree.h:1420).
    fn cast(node: &'gc Node<'gc>) -> Option<&'gc Self>;
    /// The node's `_operator` attribute.
    fn operator(&self) -> NodeLabel;
    /// The node's `_left` child.
    fn left(&self) -> &'gc Node<'gc>;
    /// The node's `_right` child.
    fn right(&self) -> &'gc Node<'gc>;
}

impl<'gc> OperatorExpr<'gc> for BinaryExpression<'gc> {
    fn cast(node: &'gc Node<'gc>) -> Option<&'gc Self> {
        node.as_binary_expression()
    }
    fn operator(&self) -> NodeLabel {
        self.operator.get()
    }
    fn left(&self) -> &'gc Node<'gc> {
        self.left
    }
    fn right(&self) -> &'gc Node<'gc> {
        self.right
    }
}

impl<'gc> OperatorExpr<'gc> for AssignmentExpression<'gc> {
    fn cast(node: &'gc Node<'gc>) -> Option<&'gc Self> {
        node.as_assignment_expression()
    }
    fn operator(&self) -> NodeLabel {
        self.operator.get()
    }
    fn left(&self) -> &'gc Node<'gc> {
        self.left
    }
    fn right(&self) -> &'gc Node<'gc> {
        self.right
    }
}

/// Check if an AST node is of the specified type and its `_operator`
/// attribute is within the set of allowed operators. Port of
/// `ESTree::checkExprOperator` (ESTree.h:1416-1425).
fn check_expr_operator<'gc, N: OperatorExpr<'gc>>(
    e: &'gc Node<'gc>,
    ops: &[NodeLabel],
) -> Option<&'gc N> {
    let n = N::cast(e)?;
    if ops.contains(&n.operator()) {
        Some(n)
    } else {
        None
    }
}

/// Convert a recursive expression of the form `(((a + b) + c) + d)` into a
/// list `a, b, c, d`. This description of the list is for exposition
/// purposes, but the actual list contains pointers to each binop node:
/// `list = [(a + b), (list[0] + c), (list[1] + d)]`. Note that the list is
/// only three elements long and the first element is accessible through the
/// `_left` pointer of `list[0]`.
///
/// Port of `ESTree::linearizeLeft` (ESTree.h:1437-1451).
///
/// \param ops the acceptable values for the `_operator` attribute of the
///   expression. Ideally it should contain all operators with the same
///   precedence: `["+", "-"]` or `["*", "/", "%"]`, etc.
pub(crate) fn linearize_left<'gc, N: OperatorExpr<'gc>>(
    e: &'gc N,
    ops: &[NodeLabel],
) -> Vec<&'gc N> {
    let mut e = e;
    let mut vec = vec![e];

    while let Some(left) = check_expr_operator::<N>(e.left(), ops) {
        e = left;
        vec.push(e);
    }

    vec.reverse();
    vec
}

/// Convert a recursive expression of the form `(a = (b = (c = d)))` into a
/// list `a, b, c, d`. This description of the list is for exposition
/// purposes, but the actual list contains pointers to each node:
/// `list = [(a = list[1]), (b = list[2]), (c = d)]`. Note that the list is
/// only three elements long and the last element is accessible through the
/// `_right` pointer of `list[2]`.
///
/// Port of `ESTree::linearizeRight` (ESTree.h:1464-1477).
///
/// \param ops the acceptable values for the `_operator` attribute of the
///   expression. Ideally it should contain all operators with the same
///   precedence, but can also be a single operator like `["="]`, if the
///   caller doesn't want to deal with the complexity.
///
/// `allow(dead_code)`: its only non-test caller,
/// `visit(AssignmentExpressionNode *)`, is a later S1 task; it is ported now
/// so the pair stays together, exactly like the two `MAX_NESTED_*` limits.
#[allow(dead_code)]
pub(crate) fn linearize_right<'gc, N: OperatorExpr<'gc>>(
    e: &'gc N,
    ops: &[NodeLabel],
) -> Vec<&'gc N> {
    let mut e = e;
    let mut vec = vec![e];

    while let Some(right) = check_expr_operator::<N>(e.right(), ops) {
        e = right;
        vec.push(e);
    }

    vec
}

#[cfg(test)]
mod tests {
    use super::*;
    use ast::context::{Context, GCLock};
    use ast::node_child::NodeMetadata;
    use support::location::{SMLoc, SMRange, SourceId};

    /// A zero-width dummy range; nothing here reads locations.
    fn r() -> SMRange {
        let l = SMLoc {
            source: SourceId::from_index(0),
            offset: 0,
        };
        SMRange { start: l, end: l }
    }

    fn num<'gc>(gc: &'gc GCLock, v: f64) -> &'gc Node<'gc> {
        gc.alloc(Node::NumericLiteral(ast::node::NumericLiteral::new(
            NodeMetadata::new(r()),
            v,
        )))
    }

    fn bin<'gc>(
        gc: &'gc GCLock,
        left: &'gc Node<'gc>,
        right: &'gc Node<'gc>,
        op: &[u8],
    ) -> &'gc Node<'gc> {
        gc.alloc(Node::BinaryExpression(BinaryExpression::new(
            NodeMetadata::new(r()),
            left,
            right,
            gc.atom_bytes(op),
        )))
    }

    fn assign<'gc>(
        gc: &'gc GCLock,
        left: &'gc Node<'gc>,
        right: &'gc Node<'gc>,
        op: &[u8],
    ) -> &'gc Node<'gc> {
        gc.alloc(Node::AssignmentExpression(AssignmentExpression::new(
            NodeMetadata::new(r()),
            gc.atom_bytes(op),
            left,
            right,
        )))
    }

    /// `((1 + 2) - 3) + 4` linearizes to the three binops, innermost first;
    /// the leftmost operand is reached through `list[0].left`.
    #[test]
    fn linearize_left_collects_the_left_spine() {
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let one_plus_two = bin(&gc, num(&gc, 1.0), num(&gc, 2.0), b"+");
        let minus_three = bin(&gc, one_plus_two, num(&gc, 3.0), b"-");
        let plus_four = bin(&gc, minus_three, num(&gc, 4.0), b"+");
        let ops = [gc.atom_bytes(b"+"), gc.atom_bytes(b"-")];

        let list =
            linearize_left(plus_four.as_binary_expression().unwrap(), &ops);
        let expected = [one_plus_two, minus_three, plus_four];
        assert_eq!(list.len(), expected.len());
        for (got, want) in list.iter().zip(expected.iter()) {
            assert!(std::ptr::eq(
                *got,
                want.as_binary_expression().unwrap()
            ));
        }
        // The leftmost operand is reached through `list[0].left`.
        let innermost = one_plus_two.as_binary_expression().unwrap();
        assert!(std::ptr::eq(list[0].left(), innermost.left()));
    }

    /// An operator outside `ops` stops the chain: in `(1 * 2) + 3` with
    /// `{+,-}`, only the outer node is collected.
    #[test]
    fn linearize_left_stops_at_a_foreign_operator() {
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let times = bin(&gc, num(&gc, 1.0), num(&gc, 2.0), b"*");
        let plus = bin(&gc, times, num(&gc, 3.0), b"+");
        let ops = [gc.atom_bytes(b"+"), gc.atom_bytes(b"-")];

        let list = linearize_left(plus.as_binary_expression().unwrap(), &ops);
        assert_eq!(list.len(), 1);
        assert!(std::ptr::eq(list[0], plus.as_binary_expression().unwrap()));
    }

    /// `a = (b = (c = 1))` linearizes to the three assignments, outermost
    /// first (NOT reversed — that is the whole difference from
    /// `linearizeLeft`); the last operand is `list.last().right()`.
    #[test]
    fn linearize_right_collects_the_right_spine() {
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let last = num(&gc, 1.0);
        let inner = assign(&gc, num(&gc, 3.0), last, b"=");
        let middle = assign(&gc, num(&gc, 2.0), inner, b"=");
        let outer = assign(&gc, num(&gc, 4.0), middle, b"=");
        let ops = [gc.atom_bytes(b"=")];

        let list =
            linearize_right(outer.as_assignment_expression().unwrap(), &ops);
        let expected = [outer, middle, inner];
        assert_eq!(list.len(), expected.len());
        for (got, want) in list.iter().zip(expected.iter()) {
            assert!(std::ptr::eq(
                *got,
                want.as_assignment_expression().unwrap()
            ));
        }
        assert!(std::ptr::eq(list.last().unwrap().right(), last));
    }

    /// A compound assignment is not `=`, so it terminates the chain.
    #[test]
    fn linearize_right_stops_at_a_foreign_operator() {
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let inner = assign(&gc, num(&gc, 1.0), num(&gc, 2.0), b"+=");
        let outer = assign(&gc, num(&gc, 3.0), inner, b"=");
        let ops = [gc.atom_bytes(b"=")];

        let list =
            linearize_right(outer.as_assignment_expression().unwrap(), &ops);
        assert_eq!(list.len(), 1);
        assert!(std::ptr::eq(
            list[0],
            outer.as_assignment_expression().unwrap()
        ));
    }
}
