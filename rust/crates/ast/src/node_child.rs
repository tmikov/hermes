//! Child/leaf field types and the NodeList for the AST.
use std::cell::Cell;
use std::marker::PhantomData;

use support::location::{SMLoc, SMRange};

use crate::context::{GCLock, NodeListElement};
use crate::NodeId;
use crate::node::{EmptyStatement, Node};
use crate::visitor::{Path, TransformResult, VisitorMut};

/// JS identifier / operator / keyword bytes, interned in the AtomTable.
pub type NodeLabel = atom_table::AtomBytes;

/// JS string-literal bytes, interned in the AtomTable (C++ `NodeString = UniqueString*`).
pub type NodeString = atom_table::AtomBytes;

/// Function strictness state (mirrors `ESTree.h` `enum class Strictness`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strictness {
    /// Sema has not decided yet.
    NotSet,
    /// The function is not in strict mode.
    NonStrictMode,
    /// The function is in strict mode.
    StrictMode,
}

/// Sentinel for an unset label index (mirrors `LabelDecorationBase::INVALID_LABEL`, `~0u`).
pub const INVALID_LABEL: u32 = u32::MAX;

/// Metadata common to all AST nodes.
///
/// Stored inside [`Node`] and must not be constructed directly by users.
/// `range`/`parens`/`debug_loc` are attributes → `Cell`.
#[derive(Debug)]
pub struct NodeMetadata<'gc> {
    pub(crate) phantom: PhantomData<&'gc Node<'gc>>,
    /// The node's source range, mirroring ESTree.h `Node::sourceRange_`.
    pub range: Cell<SMRange>,
    /// Debug location, mirroring ESTree.h Node debug loc set by
    /// JSParserImpl::setLocation. Defaults to range start.
    pub debug_loc: Cell<SMLoc>,
    /// 0, 1, or 2 (meaning "2 or more"), mirroring ESTree.h Node::parens_.
    pub parens: Cell<u8>,
    /// Identity of the arena slot this metadata is (or will be) stored in.
    /// `UNASSIGNED` until `Context::alloc` stamps a fresh id; belongs to the
    /// slot's occupant, not to this metadata value across rebuilds.
    pub id: Cell<NodeId>,
}

impl<'gc> NodeMetadata<'gc> {
    /// Create metadata for `range`. `debug_loc` defaults to `range.start`,
    /// matching the C++ 3-arg `setLocation` overload.
    pub fn new(range: SMRange) -> Self {
        NodeMetadata {
            phantom: PhantomData,
            range: Cell::new(range),
            debug_loc: Cell::new(range.start),
            parens: Cell::new(0),
            id: Cell::new(NodeId::UNASSIGNED),
        }
    }

    /// Like `new`, but with an explicit debug location (C++ 4-arg setLocation).
    pub fn new_with_debug(range: SMRange, debug_loc: SMLoc) -> Self {
        NodeMetadata {
            phantom: PhantomData,
            range: Cell::new(range),
            debug_loc: Cell::new(debug_loc),
            parens: Cell::new(0),
            id: Cell::new(NodeId::UNASSIGNED),
        }
    }

    /// Deep-copy the metadata, copying `Cell` values into fresh `Cell`s.
    /// Used by builders when cloning a node. The id resets to `UNASSIGNED`:
    /// it belongs to the arena slot's occupant, and `Context::alloc` stamps
    /// a fresh one when the duplicate is stored.
    pub(crate) fn duplicate(&self) -> NodeMetadata<'gc> {
        NodeMetadata {
            phantom: self.phantom,
            range: Cell::new(self.range.get()),
            debug_loc: Cell::new(self.debug_loc.get()),
            parens: Cell::new(self.parens.get()),
            id: Cell::new(NodeId::UNASSIGNED),
        }
    }

    /// Expose `duplicate` for integration-test crates.
    /// Not intended for production use.
    #[doc(hidden)]
    pub fn duplicate_pub_for_test(&self) -> NodeMetadata<'gc> {
        self.duplicate()
    }
}

/// An ordered list of nodes used as a property in the AST.
///
/// Implemented as a linked list internally to avoid extra overhead that would
/// exist if it were to allocate a `Vec` or some other structure that required
/// allocating on the native heap.
///
/// Because this is just a `Copy` head pointer into context-allocated
/// `NodeListElement`s (juno model), it implements `Copy` much like any other
/// pointer/reference, allowing the user to handle it much like `&Node` in many
/// cases. Empty == null head.
#[derive(Debug, Copy, Clone)]
pub struct NodeList<'gc> {
    /// If non-null, pointer to the first element of the list.
    /// If null, the list is empty.
    pub(crate) head: *const NodeListElement<'gc>,
}

impl<'gc> NodeList<'gc> {
    /// Create a new empty list.
    /// Guaranteed to be fast, performs no allocations.
    pub fn empty() -> Self {
        NodeList {
            head: std::ptr::null(),
        }
    }

    /// Connect the provided pre-existing nodes into a `NodeList` via iteration.
    /// `NodeList` doesn't implement `FromIterator` directly due to the `GCLock`
    /// requirement.
    pub fn from_iter<'a, I: IntoIterator<Item = &'a Node<'a>>>(
        lock: &'a GCLock<'_, '_>,
        nodes: I,
    ) -> NodeList<'a> {
        let mut it = nodes.into_iter();
        match it.next() {
            Some(first) => {
                // At least one element in the list.
                // Allocate the `NodeListElement`s in the context.
                let head_elem: &'a NodeListElement<'a> =
                    lock.append_list_element(None, first);
                let mut prev_elem = head_elem;
                // Exhaust the rest of the iterator.
                for next in it {
                    let next_elem =
                        lock.append_list_element(Some(prev_elem), next);
                    prev_elem = next_elem;
                }
                NodeList { head: head_elem }
            }
            _ => {
                // No elements, return the empty `NodeList`.
                NodeList::empty()
            }
        }
    }

    /// Whether this `NodeList` has no elements.
    /// Cost: `O(1)`
    pub fn is_empty(&self) -> bool {
        self.head.is_null()
    }

    /// Iterate the list front to back. Cost: `O(1)` to start, `O(1)` per step.
    pub fn iter(self) -> NodeListIter<'gc> {
        NodeListIter {
            ptr: self.head,
            _pd: PhantomData,
        }
    }
}

impl<'gc> IntoIterator for NodeList<'gc> {
    type Item = &'gc Node<'gc>;
    type IntoIter = NodeListIter<'gc>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Iterator for `Node`s in the `NodeList`.
pub struct NodeListIter<'gc> {
    /// The upcoming element in the iteration order.
    /// `null` if the iteration is complete (`next` will return `None`).
    ptr: *const NodeListElement<'gc>,
    _pd: PhantomData<&'gc Node<'gc>>,
}

impl<'gc> Iterator for NodeListIter<'gc> {
    type Item = &'gc Node<'gc>;
    fn next(&mut self) -> Option<&'gc Node<'gc>> {
        if self.ptr.is_null() {
            None
        } else {
            // SAFETY note: dereference is sound because list elements live in
            // the Context for the GCLock lifetime. The single `unsafe` lives in
            // context.rs; we expose the deref via a context.rs helper so
            // node_child stays safe.
            let (node, next) = crate::context::list_elem_parts(self.ptr);
            self.ptr = next;
            Some(node)
        }
    }
}

/// Build a zero-width `EmptyStatement` at the start of `at`'s range, used to
/// replace a required single child that a `VisitorMut` asked to remove.
fn empty_statement<'gc>(gc: &'gc GCLock<'_, '_>, at: SMRange) -> &'gc Node<'gc> {
    let range = SMRange {
        start: at.start,
        end: at.start,
    };
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
            TransformResult::Removed => {
                TransformResult::Changed(empty_statement(ctx, self.range()))
            }
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
                Expanded(_) => {
                    panic!("cannot expand a single optional child into multiple nodes")
                }
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
        let mut index = 0usize;
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strictness_and_constants() {
        assert_eq!(INVALID_LABEL, u32::MAX);
        assert_ne!(Strictness::StrictMode, Strictness::NotSet);
        // NodeString and NodeLabel are the same interned-bytes handle type.
        fn _same(_a: NodeString, b: NodeLabel) -> NodeString { b }
    }
}
