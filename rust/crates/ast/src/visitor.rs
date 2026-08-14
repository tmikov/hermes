//! AST traversal.
use crate::node::{Node, NodeField};
use crate::context::GCLock;

/// Read-only visitor. Implementors override `visit_node`; the default recurses.
/// (Unchanged from phase 1 — used by the GC marker in `context.rs`.)
pub trait Visitor<'gc> {
    /// Called once for `node`. The default recurses into its children.
    fn visit_node(&mut self, node: &'gc Node<'gc>) {
        node.visit_children(self);
    }
}

/// The path to the node currently being visited: its parent and the field of
/// the parent it occupies. Mirrors juno's `Path`.
#[derive(Debug, Copy, Clone)]
pub struct Path<'gc> {
    /// The node that owns the field being visited.
    pub parent: &'gc Node<'gc>,
    /// Which structural child field of `parent` the visited node occupies.
    pub field: NodeField,
}

impl<'gc> Path<'gc> {
    /// Build a path from a parent node and one of its child fields.
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
    /// Visit `node`, reached via `path` (`None` at the root), and return how
    /// it should be transformed.
    fn call(
        &mut self,
        ctx: &'gc GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        path: Option<Path<'gc>>,
    ) -> TransformResult<&'gc Node<'gc>>;
}
