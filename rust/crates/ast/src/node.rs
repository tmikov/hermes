//! Minimal hand-written node model (phase 1). Replaced by generated nodes later.
use std::cell::Cell;

use crate::node_child::{NodeLabel, NodeList, NodeMetadata};
use crate::visitor::Visitor;
use crate::SemaId;

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
    pub fn visit_children<V: Visitor<'gc> + ?Sized>(&'gc self, v: &mut V) {
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

    /// Call `cb` for each NodeList reachable from this node (for GC mark),
    /// including decoration lists.
    pub fn mark_lists<F: FnMut(&NodeList<'gc>)>(&'gc self, cb: &mut F) {
        if let Node::Program(n) = self {
            cb(&n.body);
            let d = n.decorations.get();
            cb(&d);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn kind_mapping_compiles() {
        // Compile-time shape check; real node construction needs Context
        // (see context::tests).
        fn _accepts(n: &Node) -> NodeKind {
            n.kind()
        }
        assert_eq!(NodeKind::Program, NodeKind::Program);
    }
}
