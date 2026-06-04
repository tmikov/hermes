//! Child/leaf field types and the NodeList for the AST.
use std::cell::Cell;
use std::marker::PhantomData;

use support::location::SMRange;

use crate::context::{GCLock, NodeListElement};
use crate::node::Node;

/// JS identifier / operator / keyword bytes, interned in the AtomTable.
pub type NodeLabel = atom_table::AtomBytes;

/// JS string-literal bytes, interned in the AtomTable (C++ `NodeString = UniqueString*`).
pub type NodeString = atom_table::AtomBytes;

/// Function strictness state (mirrors `ESTree.h` `enum class Strictness`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strictness {
    NotSet,
    NonStrictMode,
    StrictMode,
}

/// Sentinel for an unset label index (mirrors `LabelDecorationBase::INVALID_LABEL`, `~0u`).
pub const INVALID_LABEL: u32 = u32::MAX;

/// Metadata common to all AST nodes.
///
/// Stored inside [`Node`] and must not be constructed directly by users.
/// `range`/`parens` are attributes → `Cell`.
#[derive(Debug)]
pub struct NodeMetadata<'gc> {
    pub(crate) phantom: PhantomData<&'gc Node<'gc>>,
    pub range: Cell<SMRange>,
    /// 0, 1, or 2 (meaning "2 or more"), mirroring ESTree.h Node::parens_.
    pub parens: Cell<u8>,
}

impl<'gc> NodeMetadata<'gc> {
    pub fn new(range: SMRange) -> Self {
        NodeMetadata {
            phantom: PhantomData,
            range: Cell::new(range),
            parens: Cell::new(0),
        }
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
