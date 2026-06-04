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
