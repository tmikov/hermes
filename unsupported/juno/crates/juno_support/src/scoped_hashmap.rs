/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! ScopedHashMap
//!
//! Conceptually ScopedHashMap provides a stack of HashMap-s, where we can:
//! - Push and pop maps (we call them scopes)
//! - Insert elements in the top HashMap
//! - Search elements from top to bottom.
//!
//! The performance of a naive implementation would be:
//! - Push/pop a scope: O(1)
//! - Search: O(number of scopes)
//! It optimizes for pushing and popping scopes instead of searching.
//!
//! Instead this implementation instead optimizes for search:
//! - Push a scope: O(1)
//! - Pop a scope: O(number of elements in the scope)
//! - Search: O(1)
//!
//! Scopes usually have only a few elements, can be deeply nested, and searching
//! is very frequent. To that end, instead of a "stack of HashMap-s" we use a
//! "HashMap of stacks". In other words, every element of the single HashMap is
//! a stack of entries from different scopes. When we perform a lookup, we can
//! get to the topmost entry in O(1). When we pop a scope, we need to pop each
//! individual element belonging to it.
//!
//! All map entries are separately heap allocated, connected through pointers.
//! Every has a pointer to a "shadowed" entry in a previous scope, and a
//! next entry in the same scope. The hash table itself points to the top-most
//! node. Additionally a stack of scopes points to the first node in every
//! scope.
//!
//! If we perform the following operations:
//! ```[rust]
//! m.push_scope();
//! m.insert("a", 1);
//! m.insert("c", 3);
//! m.push_scope();
//! m.insert("b", 20);
//! m.insert("c", 30);
//! m.push_scope();
//! m.insert("a", 100);
//! m.insert("d", 400);
//! ```
//! We will get the following state in memory:
//! ```[text]
//! Scope 1: --> "a":1 ------------------> "c": 3
//!               ^                         ^
//!               |                         |
//! Scope 2: -----+----------> "b":20 ---> "c": 30
//!               |             ^           ^
//!               |             |           |
//! Scope 3: --> "a":100 -------+-----------+--------> "d":400
//!               ^             |           |           ^
//!               |             |           |           |
//! HashTab:    ["a"]         ["b"]       ["c"]       ["d"]
//! ```

use std::borrow::Borrow;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::hash::Hash;

/// Index of a node in the node storage vec. `None` represents null.
type NodeIdx = Option<usize>;

/// A node representing a value inserted in a scope.
#[derive(Debug)]
struct Node<K, V> {
    key: K,
    value: V,
    /// A node with the same key in a previous scope, or None if no previous.
    prev_shadowed: NodeIdx,
    /// The previous node in the same scope, or None if this is the first one.
    prev_in_scope: NodeIdx,
    /// Level of the scope this node belongs to.
    depth: usize,
}

/// The scope is the head of a singly linked list of nodes belonging to the
/// scope, chained through the [Node::prev_in_scope] index.
struct Scope {
    /// The last node inserted into the scope, None initially.
    last: NodeIdx,
}

impl Debug for Scope {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scope").field("last", &self.last).finish()
    }
}

#[derive(Debug)]
pub struct ScopedHashMap<K, V>
where
    K: Eq + Hash + Clone,
{
    /// Maps from keys to the index of the most current node.
    map: HashMap<K, usize>,

    /// All nodes, indexed by position.
    nodes: Vec<Node<K, V>>,

    /// Indices of freed nodes available for reuse.
    free_list: Vec<usize>,

    /// Stack of scopes.
    scopes: Vec<Scope>,
}

impl<K, V> Default for ScopedHashMap<K, V>
where
    K: Eq + Hash + Clone,
{
    fn default() -> Self {
        let mut res = ScopedHashMap {
            map: Default::default(),
            nodes: Default::default(),
            free_list: Default::default(),
            scopes: Default::default(),
        };
        res.push_scope();
        res
    }
}

impl<K, V> ScopedHashMap<K, V>
where
    K: Eq + Hash + Clone,
{
    pub fn new() -> Self {
        Default::default()
    }

    /// Allocate a node, reusing a free slot if available.
    fn alloc_node(&mut self, node: Node<K, V>) -> usize {
        if let Some(idx) = self.free_list.pop() {
            self.nodes[idx] = node;
            idx
        } else {
            let idx = self.nodes.len();
            self.nodes.push(node);
            idx
        }
    }

    /// Free a node by adding its index to the free list.
    fn free_node(&mut self, idx: usize) {
        self.free_list.push(idx);
    }

    /// Insert a key/value pair into the scope at the specified depth if
    /// possible, return an error otherwise.
    /// If the depth is smaller than the current depth, it is not guaranteed to
    /// succeed, because values may already be present "shadowing" the value
    /// that is being inserted.
    /// If a value is already present at the specified depth, it is replaced.
    pub fn insert_into_scope(
        &mut self,
        scope_depth: usize,
        key: K,
        value: V,
    ) -> Result<(), &'static str> {
        assert!(scope_depth < self.scopes.len(), "scope_index out of range");

        // Check if the key already exists.
        let prev_shadowed = if let Some(&node_idx) = self.map.get(&key) {
            let node = &mut self.nodes[node_idx];
            match node.depth.cmp(&scope_depth) {
                Ordering::Greater => return Err("Value to be inserted is already shadowed"),
                Ordering::Equal => {
                    node.value = value;
                    return Ok(());
                }
                Ordering::Less => Some(node_idx),
            }
        } else {
            None
        };

        let scope_last = self.scopes[scope_depth].last;
        let new_idx = self.alloc_node(Node {
            key: key.clone(),
            value,
            prev_shadowed,
            prev_in_scope: scope_last,
            depth: scope_depth,
        });

        self.map.insert(key, new_idx);
        self.scopes[scope_depth].last = Some(new_idx);
        Ok(())
    }

    /// Insert a key/value pair into the current scope. If the key already exists,
    /// the value is overwritten.
    pub fn insert(&mut self, key: K, value: V) {
        self.insert_into_scope(self.scopes.len() - 1, key, value)
            .unwrap();
    }

    pub fn contains_key<Q: ?Sized>(&self, k: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.map.contains_key(k)
    }

    pub fn get<Q: ?Sized>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.map.get(k).map(|&idx| &self.nodes[idx].value)
    }

    pub fn get_mut<Q: ?Sized>(&mut self, k: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.map
            .get(k)
            .copied()
            .map(move |idx| &mut self.nodes[idx].value)
    }

    pub fn value(&self, k: K) -> Option<&V> {
        self.get(&k)
    }

    pub fn value_mut(&mut self, k: K) -> Option<&mut V> {
        self.get_mut(&k)
    }

    /// Push a new scope, run the specified callback in it, then pop the scope.
    pub fn in_new_scope<R, F: FnOnce(&mut Self) -> R>(&mut self, f: F) -> R {
        self.push_scope();
        let res = f(self);
        self.pop_scope();
        res
    }

    /// Return the depth of the current scope (the initial scope is depth 0).
    /// It can be used in order to call ['ScopedHashMap::insert_into_scope()'].
    pub fn current_scope_depth(&self) -> usize {
        debug_assert!(!self.scopes.is_empty(), "We don't allow popping of scope 0");
        self.scopes.len() - 1
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(Scope { last: None });
    }

    pub fn pop_scope(&mut self) {
        assert!(self.scopes.len() > 1, "Cannot pop the root scope");
        self.pop_scope_impl();
    }

    fn pop_scope_impl(&mut self) {
        assert!(!self.scopes.is_empty(), "No current scope to clear");
        let cur_depth = self.scopes.len() - 1;
        let mut current = self.scopes.pop().unwrap().last;
        while let Some(idx) = current {
            debug_assert!(self.nodes[idx].depth == cur_depth, "Bad scope link");
            current = self.nodes[idx].prev_in_scope;
            self.pop_node(&self.nodes[idx].key.clone());
            self.free_node(idx);
        }
    }

    /// Unlinks the innermost node for a key from the map.
    fn pop_node(&mut self, key: &K) {
        let entry = self.map.get_mut(key).unwrap();
        let idx = *entry;
        if let Some(prev) = self.nodes[idx].prev_shadowed {
            *entry = prev;
        } else {
            self.map.remove(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test1() {
        let mut map = ScopedHashMap::<i32, i32>::new();

        map.insert(1, 10);
        map.insert(2, 20);

        assert_eq!(*map.value(1).unwrap(), 10);
        assert_eq!(*map.value(2).unwrap(), 20);
        assert!(map.value(3).is_none());
    }

    #[test]
    fn test2() {
        let mut map = ScopedHashMap::<i32, i32>::new();

        map.insert(1, 10);
        map.insert(2, 20);

        assert_eq!(*map.value(1).unwrap(), 10);
        assert_eq!(*map.value(2).unwrap(), 20);
        assert!(map.value(3).is_none());

        map.in_new_scope(|map| {
            map.insert(1, 11);
            map.insert(3, 31);
            assert_eq!(*map.value(1).unwrap(), 11);
            assert_eq!(*map.value(2).unwrap(), 20);
            assert_eq!(*map.value(3).unwrap(), 31);
            map.in_new_scope(|map| {
                map.insert(1, 12);
                map.insert(3, 32);
                assert_eq!(*map.value(1).unwrap(), 12);
                assert_eq!(*map.value(2).unwrap(), 20);
                assert_eq!(*map.value(3).unwrap(), 32);
            });
            assert_eq!(*map.value(1).unwrap(), 11);
            assert_eq!(*map.value(2).unwrap(), 20);
            assert_eq!(*map.value(3).unwrap(), 31);
        });

        assert_eq!(*map.value(1).unwrap(), 10);
        assert_eq!(*map.value(2).unwrap(), 20);
        assert!(map.value(3).is_none());
    }
}
