/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Port of `hermes::PersistentScopedMap`
//! (`include/hermes/ADT/PersistentScopedMap.h`): a scoped hash table similar
//! to `hermes::ScopedHashTable`, but scopes can be retained and reactivated
//! after they have been popped from the table.
//!
//! The type [`ScopePtr`], which in C++ is an intrusive reference-counting
//! smart pointer, is used to retain ownership of a scope. The pointer can be
//! used to reactivate the scope in the table using
//! [`PersistentScopedMap::activate_scope`].
//!
//! Scopes can also be re-activated even if they are currently active but are
//! not the current scope. Note however that if there are active scopes in
//! the stack, in the end we must restore the state — the top-most scope in
//! the stack must be active.
//!
//! Example (mirrors the C++ doc comment):
//! ```
//! use hermes_support::persistent_scoped_map::{PersistentScopedMap, Scope, ScopePtr};
//!
//! let table: PersistentScopedMap<&str, &str> = PersistentScopedMap::new();
//! let mut ptr: ScopePtr<&str, &str> = ScopePtr::default();
//! let a = Scope::new(&table);
//! let b = Scope::new(&table);
//! // At this point, A and B are active scopes in the table.
//! // A(active)->B(active)
//! // We can reactivate A:
//! table.activate_scope(&a.ptr());
//! // Now the state is the same as it was when A was active initially.
//! table.activate_scope(&b.ptr());
//! // The state has been restored to "normal".
//! {
//!     let c = Scope::new(&table);
//!     // A(active)->B(active)->C(active)
//!     // Save C for later.
//!     ptr = c.ptr();
//! }
//! // A(active)->B(active)
//! let d = Scope::new(&table);
//! // A(active)->B(active)->D(active).
//! // Activate C again.
//! table.activate_scope(&ptr);
//! // A(active)->B(active)->C(active)
//! // Restore normal state.
//! table.activate_scope(&d.ptr());
//! ptr.reset();
//! drop(d);
//! drop(b);
//! drop(a);
//! ```
//!
//! ## Deviations from the C++ implementation
//!
//! `support` is `#![forbid(unsafe_code)]`, so this port cannot reproduce the
//! C++ web of raw pointers and an intrusive, manually-maintained reference
//! count (`PersistentScopedMapScopeData::addRef`/`decRef`). Instead:
//!
//! - A scope's data (`ScopeData<K, V>`, the port of
//!   `detail::PersistentScopedMapScopeData`) is held behind an `Rc`.
//!   [`ScopePtr`] is `Option<ScopeRef<K, V>>`; `Rc`'s own strong count
//!   replaces the manual `refCount_`, and `Clone`/`Drop` on `Rc` replace
//!   `addRef`/`decRef` — there is nothing left to implement by hand.
//! - The C++ `Node` intrusive linked lists (`nextInScope_` links every node
//!   created in a scope; `nextShadowed_` links a node to the same-key node it
//!   shadows in an ancestor scope) become, per scope, a `Vec<Entry<K, V>>` in
//!   insertion order (`ScopeData::entries`) plus a `shadowed: Option<Slot<K,
//!   V>>` field on `Entry` (`Slot<K, V> = (Rc<ScopeData<K, V>>, usize)`) that
//!   identifies the shadowed entry by `(scope, index)` instead of by raw
//!   pointer. The map from key to innermost definition (C++ `map_:
//!   DenseMap<K, Node *>`) becomes `map: RefCell<HashMap<K, Slot<K, V>>>`.
//! - Because each key is inserted at most once per scope (`try_emplace`
//!   refuses a second insertion into the same scope), the *order* in which a
//!   scope's own entries are popped/pushed does not affect observable
//!   behavior. This port walks `entries` forward (insertion order); the C++
//!   walks `head_`/`nextInScope_`, which is reverse insertion order. Only the
//!   internal traversal order differs — not behavior.
//! - `lookup`, `find`, `find_with_depth`, and `find_in_current_scope` return
//!   an owned `Option<V>` (hence the `V: Clone` bound) rather than C++'s
//!   `V*` / default-constructed `V` (for `lookup`). Interior mutability
//!   (`RefCell`) means we cannot hand back a reference into the map that
//!   outlives the call, so callers get a clone instead. `count` still
//!   returns `u32` (0 or 1), matching `DenseMap::count`'s semantics for a
//!   unique key.
//! - All `PersistentScopedMap` methods take `&self`: the map's mutable state
//!   lives in `RefCell`s so it can be shared behind a plain reference, which
//!   is what a scope-retaining API needs (a [`Scope`] borrows the map for
//!   its whole lifetime while [`ScopePtr`]s to older scopes may outlive it).
//!
//! Every method below keeps the corresponding C++ comment (adapted) and the
//! same assertions, expressed as `debug_assert!`/`assert!`.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::hash::Hash;
use std::rc::Rc;

/// A reference-counted handle to a scope's data. `Rc`'s strong count is the
/// port of the C++ intrusive `refCount_`; see the module documentation.
type ScopeRef<K, V> = Rc<ScopeData<K, V>>;

/// Identifies one entry by the scope that owns it and its index within that
/// scope's `entries`. The port of a raw `Node *`.
type Slot<K, V> = (ScopeRef<K, V>, usize);

/// A key/value pair declared in a scope. Analogous to C++
/// `detail::PersistentScopedMapNode`, minus the intrusive `nextInScope_`
/// pointer (this port keeps entries in a `Vec` on the owning scope instead).
struct Entry<K, V> {
    /// The declared key. Stored so that popping/pushing a scope can find the
    /// key's slot in the map without also storing the value there.
    key: K,
    /// The declared value. Overwritten in place by `put`/`put_in_scope`.
    value: V,
    /// The `(scope, index)` this entry shadows in an ancestor scope, if any.
    /// The port of `nextShadowed_`. Recomputed every time the owning scope
    /// is pushed (initial insertion, or reactivation via `activate_scope`),
    /// exactly like the C++ `pushEntry` overwriting `nextShadowed_`.
    shadowed: Option<Slot<K, V>>,
}

/// This is the data for a scope. It is reference counted (via `Rc`). It
/// contains the entries declared in the scope, and a pointer to the parent
/// scope. Port of `detail::PersistentScopedMapScopeData`.
struct ScopeData<K, V> {
    /// Entries declared in this scope, in insertion order. Owned by the
    /// scope (the port of the `head_`/`nextInScope_` linked list).
    entries: RefCell<Vec<Entry<K, V>>>,
    /// The scope we're shadowing.
    parent: Option<ScopeRef<K, V>>,
    /// Scope depth, starting from 0 for the outermost one.
    depth: u32,
    /// Whether this scope is active in the map or it has been popped.
    active: Cell<bool>,
}

impl<K, V> Drop for ScopeData<K, V> {
    fn drop(&mut self) {
        // The C++ destructor also asserts `refCount_ == 0`, but with `Rc`
        // that is guaranteed by construction: this destructor only runs
        // once the strong count reaches zero.
        debug_assert!(!self.active.get(), "Cannot destroy an active scope");
    }
}

/// Smart pointer retaining ownership of a scope so it can be reactivated
/// after it has been popped from the table. Port of
/// `PersistentScopedMapScopePtr`; `Rc`'s automatic reference counting
/// replaces the manual `addRef`/`decRef` pair.
pub struct ScopePtr<K, V>(Option<ScopeRef<K, V>>);

impl<K, V> ScopePtr<K, V> {
    fn new(scope: ScopeRef<K, V>) -> Self {
        ScopePtr(Some(scope))
    }

    /// Return true if this pointer is null.
    pub fn is_null(&self) -> bool {
        self.0.is_none()
    }

    /// Free the scope reference and set the pointer to null.
    pub fn reset(&mut self) {
        self.0 = None;
    }

    fn get(&self) -> Option<&ScopeRef<K, V>> {
        self.0.as_ref()
    }
}

impl<K, V> Clone for ScopePtr<K, V> {
    fn clone(&self) -> Self {
        // Not `#[derive(Clone)]`: derive would require `K: Clone, V: Clone`
        // even though `Option<Rc<_>>::clone` never needs them.
        ScopePtr(self.0.clone())
    }
}

impl<K, V> Default for ScopePtr<K, V> {
    fn default() -> Self {
        // Not `#[derive(Default)]`, for the same reason as `Clone` above.
        ScopePtr(None)
    }
}

impl<K, V> PartialEq for ScopePtr<K, V> {
    /// Pointer identity, like the C++ `operator==` (`ptr_ == other.ptr_`).
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        }
    }
}

/// RAII for creating and popping a scope. Port of
/// `PersistentScopedMapScope`.
pub struct Scope<'m, K: Eq + Hash + Copy, V: Clone> {
    base: &'m PersistentScopedMap<K, V>,
    scope: ScopeRef<K, V>,
}

impl<'m, K: Eq + Hash + Copy, V: Clone> Scope<'m, K, V> {
    /// Create (and activate) a new child scope of `base`'s current scope.
    pub fn new(base: &'m PersistentScopedMap<K, V>) -> Self {
        let parent = base.scope.borrow().clone();
        let depth = parent.as_ref().map_or(0, |p| p.depth + 1);
        let scope = Rc::new(ScopeData {
            entries: RefCell::new(Vec::new()),
            parent,
            depth,
            active: Cell::new(true),
        });
        *base.scope.borrow_mut() = Some(scope.clone());
        Scope { base, scope }
    }

    /// \return the depth of the scope.
    pub fn depth(&self) -> u32 {
        self.scope.depth
    }

    /// Return a persistent pointer that retains ownership of the scope so it
    /// can be reactivated after it has been popped.
    pub fn ptr(&self) -> ScopePtr<K, V> {
        ScopePtr::new(self.scope.clone())
    }
}

impl<'m, K: Eq + Hash + Copy, V: Clone> Drop for Scope<'m, K, V> {
    fn drop(&mut self) {
        self.base.pop_scope(&self.scope);
    }
}

/// Scoped hash table similar to `hermes::ScopedHashTable`, but scopes can be
/// retained and reactivated after they have been popped from the table. See
/// the module documentation for the full example and for the deviations
/// from the C++ implementation.
pub struct PersistentScopedMap<K, V> {
    /// Maps from keys to the (scope, index) of the innermost definition.
    map: RefCell<HashMap<K, Slot<K, V>>>,
    /// The current scope.
    scope: RefCell<Option<ScopeRef<K, V>>>,
}

impl<K: Eq + Hash + Copy, V: Clone> Default for PersistentScopedMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> Drop for PersistentScopedMap<K, V> {
    fn drop(&mut self) {
        debug_assert!(
            self.scope.borrow().is_none(),
            "Scopes remain when destructing PersistentScopedMap"
        );
        debug_assert!(
            self.map.borrow().is_empty(),
            "Elements remaining in map without scope!"
        );
    }
}

impl<K: Eq + Hash + Copy, V: Clone> PersistentScopedMap<K, V> {
    pub fn new() -> Self {
        PersistentScopedMap {
            map: RefCell::new(HashMap::new()),
            scope: RefCell::new(None),
        }
    }

    /// Return a pointer to the current scope. The pointer may be null.
    pub fn current_scope(&self) -> ScopePtr<K, V> {
        ScopePtr(self.scope.borrow().clone())
    }

    fn require_current(&self) -> ScopeRef<K, V> {
        self.scope
            .borrow()
            .clone()
            .expect("PersistentScopedMap has no current scope")
    }

    fn require_scope(ptr: &ScopePtr<K, V>) -> ScopeRef<K, V> {
        ptr.get()
            .cloned()
            .expect("PersistentScopedMapScopePtr must not be null")
    }

    /// Push the specified node/entry to the top of the stack for its key.
    /// `scope` is the scope the entry belongs to (used only for the debug
    /// check); `key`/`idx` locate the entry in `scope.entries`. Port of
    /// `pushEntry`.
    fn push_entry(&self, scope: &ScopeRef<K, V>, key: K, idx: usize) {
        let prev = self.map.borrow_mut().insert(key, (scope.clone(), idx));
        if let Some((ref prev_scope, _)) = prev {
            debug_assert!(
                prev_scope.depth < scope.depth,
                "Can't insert values under existing names"
            );
        }
        scope.entries.borrow_mut()[idx].shadowed = prev;
    }

    /// Create a new entry and insert it into `scope`. Port of
    /// `insertNewNode`. Returns the new entry's index in `scope.entries`.
    fn insert_new_node(
        &self,
        scope: &ScopeRef<K, V>,
        key: K,
        value: V,
    ) -> usize {
        let idx = {
            let mut entries = scope.entries.borrow_mut();
            entries.push(Entry {
                key,
                value,
                shadowed: None,
            });
            entries.len() - 1
        };
        self.push_entry(scope, key, idx);
        idx
    }

    /// Unlinks the innermost entry for `key` (which must belong to `scope`,
    /// at index `idx`) from the map, restoring whatever it shadowed. Port of
    /// `popEntry`.
    fn pop_entry(&self, scope: &ScopeRef<K, V>, key: K, idx: usize) {
        let mut map = self.map.borrow_mut();
        let current = map
            .get(&key)
            .cloned()
            .expect("Asked to pop an empty scope value");
        debug_assert!(
            Rc::ptr_eq(&current.0, scope) && current.1 == idx,
            "Unexpected innermost value for key"
        );
        let shadowed = scope.entries.borrow()[idx].shadowed.clone();
        match shadowed {
            Some(s) => {
                map.insert(key, s);
            }
            None => {
                map.remove(&key);
            }
        }
    }

    /// Unlinks all entries in `scope` from the hash map and marks it as
    /// inactive. `scope` must be the current scope. Port of `popScope`.
    fn pop_scope(&self, scope: &ScopeRef<K, V>) {
        debug_assert!(
            scope.active.get(),
            "Attempting to pop an inactive scope"
        );
        {
            let current = self.scope.borrow();
            debug_assert!(
                matches!(current.as_ref(), Some(c) if Rc::ptr_eq(c, scope)),
                "Attempting to pop not current scope"
            );
        }

        let len = scope.entries.borrow().len();
        for idx in 0..len {
            let key = scope.entries.borrow()[idx].key;
            self.pop_entry(scope, key, idx);
        }
        scope.active.set(false);
        *self.scope.borrow_mut() = scope.parent.clone();
    }

    /// Push the specified scope, which must be a child of the current
    /// scope, into the hash map and activate it. Port of `pushChildScope`.
    fn push_child_scope(&self, scope: &ScopeRef<K, V>) {
        debug_assert!(
            !scope.active.get(),
            "Attempting to push an active scope"
        );
        {
            let current = self.scope.borrow();
            let is_child_of_current = match (&scope.parent, current.as_ref()) {
                (Some(p), Some(c)) => Rc::ptr_eq(p, c),
                (None, None) => true,
                _ => false,
            };
            debug_assert!(
                is_child_of_current,
                "Attempting to push a scope that isn't a child of the \
                 current one"
            );
        }

        let len = scope.entries.borrow().len();
        for idx in 0..len {
            let key = scope.entries.borrow()[idx].key;
            self.push_entry(scope, key, idx);
        }
        scope.active.set(true);
        *self.scope.borrow_mut() = Some(scope.clone());
    }

    /// Attempt to insert an element into the specified scope. Semantics
    /// equivalent to `std::map::try_emplace()`. Returns the entry's index in
    /// `scope.entries` and whether the insertion took place.
    /// A key may not be inserted such that it would be shadowed by another
    /// scope currently in effect. Attempting to do so results in undefined
    /// behavior.
    fn try_emplace_into_scope_impl(
        &self,
        scope: &ScopeRef<K, V>,
        key: K,
        value: V,
    ) -> (usize, bool) {
        debug_assert!(
            self.require_current().active.get(),
            "Attempting to modify an inactive scope"
        );
        let existing = self.map.borrow().get(&key).cloned();
        if let Some((ref existing_scope, existing_idx)) = existing {
            if existing_scope.depth == scope.depth {
                // The key exists in the current scope.
                return (existing_idx, false);
            }
        }
        // Otherwise, create a new entry in the current scope.
        let idx = self.insert_new_node(scope, key, value);
        (idx, true)
    }

    /// Attempt to insert an element into the specified scope. Returns
    /// whether the insertion took place (`false` if `key` already has a
    /// binding in `scope`). A key may not be inserted such that it would be
    /// shadowed by another scope currently in effect. Attempting to do so
    /// results in undefined behavior.
    pub fn try_emplace_into_scope(
        &self,
        scope: &ScopePtr<K, V>,
        key: K,
        value: V,
    ) -> bool {
        let scope_rc = Self::require_scope(scope);
        self.try_emplace_into_scope_impl(&scope_rc, key, value).1
    }

    /// Attempt to insert an element into the current scope. Returns whether
    /// the insertion took place.
    pub fn try_emplace(&self, key: K, value: V) -> bool {
        let scope_rc = self.require_current();
        self.try_emplace_into_scope_impl(&scope_rc, key, value).1
    }

    /// Insert or update a value in the specified scope. A key may not be
    /// inserted such that it would be shadowed by another scope currently in
    /// effect. Attempting to do so results in undefined behavior.
    pub fn put_in_scope(&self, scope: &ScopePtr<K, V>, key: K, value: V) {
        let scope_rc = Self::require_scope(scope);
        let (idx, inserted) = self.try_emplace_into_scope_impl(
            &scope_rc,
            key,
            value.clone(),
        );
        if !inserted {
            scope_rc.entries.borrow_mut()[idx].value = value;
        }
    }

    /// Insert or update an existing value in the current scope.
    pub fn put(&self, key: K, value: V) {
        let current = self.current_scope();
        self.put_in_scope(&current, key, value);
    }

    /// Returns 1 if the value is defined, 0 if it's not.
    pub fn count(&self, key: &K) -> u32 {
        u32::from(self.map.borrow().contains_key(key))
    }

    /// Gets the innermost value for a key, or `None` if none.
    pub fn lookup(&self, key: &K) -> Option<V> {
        self.find(key)
    }

    /// Return the innermost value for a key, or `None` if none.
    pub fn find(&self, key: &K) -> Option<V> {
        let map = self.map.borrow();
        let (scope, idx) = map.get(key)?;
        let value = scope.entries.borrow()[*idx].value.clone();
        Some(value)
    }

    /// \return the innermost value for a key along with its depth, or `None`
    /// if none.
    pub fn find_with_depth(&self, key: &K) -> Option<(V, u32)> {
        let map = self.map.borrow();
        let (scope, idx) = map.get(key)?;
        let value = scope.entries.borrow()[*idx].value.clone();
        Some((value, scope.depth))
    }

    /// \return the value for a key if it exists in the current scope, or
    /// `None` if none.
    pub fn find_in_current_scope(&self, key: &K) -> Option<V> {
        let map = self.map.borrow();
        let (scope, idx) = map.get(key)?;
        let current_depth = self.require_current().depth;
        // Result is not in the current scope.
        if scope.depth != current_depth {
            return None;
        }
        let value = scope.entries.borrow()[*idx].value.clone();
        Some(value)
    }

    pub fn activate_scope(&self, new_scope_ptr: &ScopePtr<K, V>) {
        let new_scope = Self::require_scope(new_scope_ptr);
        // We need to find the closest active parent of newScope. Then we
        // need to deactivate and pop all scopes between the current scope
        // and that parent. Finally, we need to push and activate all scopes
        // between newScope and the parent.

        // Keep track of scopes that need to be activated in reverse order.
        let mut activate_list: Vec<ScopeRef<K, V>> = Vec::new();
        let mut active_parent = Some(new_scope);
        while let Some(candidate) = active_parent {
            if candidate.active.get() {
                active_parent = Some(candidate);
                break;
            }
            active_parent = candidate.parent.clone();
            activate_list.push(candidate);
        }

        // Deactivate and pop all scopes between scope_ and active_parent.
        loop {
            let current = self.scope.borrow().clone();
            let reached = match (&current, &active_parent) {
                (Some(c), Some(a)) => Rc::ptr_eq(c, a),
                (None, None) => true,
                _ => false,
            };
            if reached {
                break;
            }
            let current = current.expect("ran out of scopes to pop");
            self.pop_scope(&current);
        }

        // Push and activate the scopes in activate_list in reverse order
        // (starting from the topmost).
        for scope in activate_list.into_iter().rev() {
            self.push_child_scope(&scope);
        }
    }

    /// Gets all values currently in scope. Port of the `UNIT_TEST`-only
    /// `test_flatten`; kept `pub` (not test-gated) since the resolver's own
    /// tests use it too.
    pub fn flatten(&self) -> HashMap<K, V> {
        let map = self.map.borrow();
        let mut result = HashMap::with_capacity(map.len());
        for (key, (scope, idx)) in map.iter() {
            result.insert(*key, scope.entries.borrow()[*idx].value.clone());
        }
        result
    }

    /// Gets keys in each scope. This may correspond to a `ScopeChain`.
    /// Shadowed keys are ignored. Index 0 is innermost. Port of the
    /// `UNIT_TEST`-only `test_getKeysByScope`; kept `pub` for the same
    /// reason as `flatten`.
    pub fn keys_by_scope(&self) -> Vec<Vec<K>> {
        let current = self.require_current();
        let size = (current.depth + 1) as usize;
        let mut result: Vec<Vec<K>> = vec![Vec::new(); size];

        for (key, (scope, _)) in self.map.borrow().iter() {
            debug_assert!(scope.depth <= current.depth, "Node at bad depth");
            result[size - scope.depth as usize - 1].push(*key);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Ported from unittests/ADT/PersistentScopedMapTest.cpp.

    #[test]
    fn smoke_test() {
        let table: PersistentScopedMap<&str, &str> = PersistentScopedMap::new();
        let scope = Scope::new(&table);
        table.try_emplace("foo", "bar");
        assert_eq!(table.lookup(&"foo"), Some("bar"));
        drop(scope);
    }

    #[test]
    fn nesting() {
        let table: PersistentScopedMap<&str, &str> = PersistentScopedMap::new();
        let outer = Scope::new(&table);
        table.try_emplace("key", "outer");
        assert_eq!(table.lookup(&"key"), Some("outer"));
        {
            let _inner = Scope::new(&table);
            table.try_emplace("key", "inner");
            assert_eq!(table.lookup(&"key"), Some("inner"));
        }
        assert_eq!(table.lookup(&"key"), Some("outer"));
        drop(outer);
    }

    #[test]
    fn overwrite() {
        let table: PersistentScopedMap<&str, &str> = PersistentScopedMap::new();
        let outer = Scope::new(&table);
        table.put("key", "foo");
        assert_eq!(table.lookup(&"key"), Some("foo"));
        table.put("key", "outer");
        assert_eq!(table.lookup(&"key"), Some("outer"));
        {
            let _inner = Scope::new(&table);
            table.put("key", "foo");
            assert_eq!(table.lookup(&"key"), Some("foo"));
            table.put("key", "inner");
            assert_eq!(table.lookup(&"key"), Some("inner"));
        }
        assert_eq!(table.lookup(&"key"), Some("outer"));
        drop(outer);
    }

    #[test]
    fn flatten_test() {
        let table: PersistentScopedMap<&str, &str> = PersistentScopedMap::new();
        let outer = Scope::new(&table);
        table.try_emplace("out", "outer");
        {
            let _inner = Scope::new(&table);
            table.try_emplace("in", "inner");
            let map = table.flatten();
            assert_eq!(map.len(), 2);
            assert_eq!(map.get("out"), Some(&"outer"));
            assert_eq!(map.get("in"), Some(&"inner"));
        }
        drop(outer);
    }

    #[test]
    fn get_keys_by_scope() {
        let table: PersistentScopedMap<&str, &str> = PersistentScopedMap::new();
        let outer = Scope::new(&table);
        table.try_emplace("out", "outer");
        table.try_emplace("in", "trash");
        {
            let _inner = Scope::new(&table);
            table.try_emplace("in", "inner");
            let scopes = table.keys_by_scope();
            assert_eq!(scopes.len(), 2);
            assert_eq!(scopes[0].len(), 1);
            assert_eq!(scopes[1].len(), 1);
            assert_eq!(scopes[0][0], "in");
            assert_eq!(scopes[1][0], "out");
        }
        drop(outer);
    }

    #[test]
    fn put_test() {
        let table: PersistentScopedMap<&str, &str> = PersistentScopedMap::new();
        let outer = Scope::new(&table);
        table.try_emplace("foo", "true");
        {
            let _inner = Scope::new(&table);
            assert_eq!(table.lookup(&"foo"), Some("true"));
            table.try_emplace("foo", "false");
            assert_eq!(table.lookup(&"foo"), Some("false"));
            table.try_emplace("foo", "true");
            assert_eq!(table.lookup(&"foo"), Some("false"));
            table.put("foo", "false");
            assert_eq!(table.lookup(&"foo"), Some("false"));
        }
        assert_eq!(table.lookup(&"foo"), Some("true"));
        drop(outer);
    }

    #[test]
    fn find_in_current_scope() {
        let table: PersistentScopedMap<&str, &str> = PersistentScopedMap::new();
        let outer = Scope::new(&table);
        table.try_emplace("foo", "true");
        {
            let _inner = Scope::new(&table);
            table.try_emplace("bar", "true");
            assert_eq!(table.find_in_current_scope(&"foo"), None);
            assert_eq!(table.find_in_current_scope(&"bar"), Some("true"));
        }
        assert_eq!(table.find_in_current_scope(&"foo"), Some("true"));
        drop(outer);
    }

    #[test]
    fn activate() {
        let mut ptr: ScopePtr<&str, &str>;
        let table: PersistentScopedMap<&str, &str> = PersistentScopedMap::new();
        {
            let a = Scope::new(&table);
            table.try_emplace("A", "a");
            table.try_emplace("key", "keyA");
            let b = Scope::new(&table);
            table.try_emplace("B", "b");
            table.try_emplace("key", "keyB");
            {
                let c = Scope::new(&table);
                table.try_emplace("C", "c");
                table.try_emplace("key", "keyC");
                let d = Scope::new(&table);
                table.try_emplace("D", "d");
                table.try_emplace("key", "keyD");
                ptr = d.ptr();

                {
                    let map = table.flatten();
                    assert_eq!(map.len(), 5);
                    assert_eq!(map.get("A"), Some(&"a"));
                    assert_eq!(map.get("B"), Some(&"b"));
                    assert_eq!(map.get("C"), Some(&"c"));
                    assert_eq!(map.get("D"), Some(&"d"));
                    assert_eq!(map.get("key"), Some(&"keyD"));
                }
                drop(d);
                drop(c);
            }
            {
                let map = table.flatten();
                assert_eq!(map.len(), 3);
                assert_eq!(map.get("A"), Some(&"a"));
                assert_eq!(map.get("B"), Some(&"b"));
                assert_eq!(map.get("key"), Some(&"keyB"));
            }
            let e = Scope::new(&table);
            table.try_emplace("E", "e");
            table.try_emplace("key", "keyE");
            let f = Scope::new(&table);
            table.try_emplace("F", "f");
            table.try_emplace("key", "keyF");
            let g = Scope::new(&table);
            table.try_emplace("G", "g");
            table.try_emplace("key", "keyG");
            //                         -> C->D
            //                       /
            //   A(active)->B(active)
            //                       \
            //                         -> E(active)->F(active)->G(active)
            {
                let map = table.flatten();
                assert_eq!(map.len(), 6);
                assert_eq!(map.get("A"), Some(&"a"));
                assert_eq!(map.get("B"), Some(&"b"));
                assert_eq!(map.get("E"), Some(&"e"));
                assert_eq!(map.get("F"), Some(&"f"));
                assert_eq!(map.get("G"), Some(&"g"));
                assert_eq!(map.get("key"), Some(&"keyG"));
            }

            table.activate_scope(&e.ptr());
            {
                let map = table.flatten();
                assert_eq!(map.len(), 4);
                assert_eq!(map.get("A"), Some(&"a"));
                assert_eq!(map.get("B"), Some(&"b"));
                assert_eq!(map.get("E"), Some(&"e"));
                assert_eq!(map.get("key"), Some(&"keyE"));
            }

            table.activate_scope(&f.ptr());
            {
                let map = table.flatten();
                assert_eq!(map.len(), 5);
                assert_eq!(map.get("A"), Some(&"a"));
                assert_eq!(map.get("B"), Some(&"b"));
                assert_eq!(map.get("E"), Some(&"e"));
                assert_eq!(map.get("F"), Some(&"f"));
                assert_eq!(map.get("key"), Some(&"keyF"));
            }
            table.activate_scope(&g.ptr());
            {
                let map = table.flatten();
                assert_eq!(map.len(), 6);
                assert_eq!(map.get("A"), Some(&"a"));
                assert_eq!(map.get("B"), Some(&"b"));
                assert_eq!(map.get("E"), Some(&"e"));
                assert_eq!(map.get("F"), Some(&"f"));
                assert_eq!(map.get("G"), Some(&"g"));
                assert_eq!(map.get("key"), Some(&"keyG"));
            }

            // Reactivate D
            table.activate_scope(&ptr);
            {
                let map = table.flatten();
                assert_eq!(map.len(), 5);
                assert_eq!(map.get("A"), Some(&"a"));
                assert_eq!(map.get("B"), Some(&"b"));
                assert_eq!(map.get("C"), Some(&"c"));
                assert_eq!(map.get("D"), Some(&"d"));
                assert_eq!(map.get("key"), Some(&"keyD"));
            }

            table.activate_scope(&g.ptr());
            {
                let map = table.flatten();
                assert_eq!(map.len(), 6);
                assert_eq!(map.get("A"), Some(&"a"));
                assert_eq!(map.get("B"), Some(&"b"));
                assert_eq!(map.get("E"), Some(&"e"));
                assert_eq!(map.get("F"), Some(&"f"));
                assert_eq!(map.get("G"), Some(&"g"));
                assert_eq!(map.get("key"), Some(&"keyG"));
            }
            drop(g);
            drop(f);
            drop(e);
            drop(b);
            drop(a);
        }
        // This location is used to check with a debugger whether all scopes
        // have been freed.
        ptr.reset();
    }

    #[test]
    fn count_and_find() {
        let table: PersistentScopedMap<&str, &str> = PersistentScopedMap::new();
        let outer = Scope::new(&table);
        assert_eq!(table.count(&"foo"), 0);
        table.try_emplace("foo", "true");
        assert_eq!(table.count(&"foo"), 1);
        assert_eq!(table.find(&"foo"), Some("true"));
        assert_eq!(table.find_with_depth(&"foo"), Some(("true", 0)));
        {
            let _inner = Scope::new(&table);
            table.try_emplace("foo", "inner");
            assert_eq!(table.find_with_depth(&"foo"), Some(("inner", 1)));
        }
        assert_eq!(table.find_with_depth(&"foo"), Some(("true", 0)));
        drop(outer);
    }

    /// Rust-specific: dropping the last `ScopePtr` of a *popped* scope frees
    /// it (running `ScopeData`'s `Drop`, which asserts the scope is
    /// inactive) without touching the map, and a later `activate_scope` of a
    /// different retained sibling still works.
    #[test]
    fn drop_last_ptr_of_popped_scope_does_not_disturb_siblings() {
        let table: PersistentScopedMap<&str, &str> = PersistentScopedMap::new();
        let outer = Scope::new(&table);
        table.try_emplace("A", "a");

        let mut b_ptr: ScopePtr<&str, &str>;
        {
            let b = Scope::new(&table);
            table.try_emplace("B", "b");
            b_ptr = b.ptr();
            // `b` dropped here: scope B is popped but kept alive by
            // `b_ptr`.
        }
        assert_eq!(table.lookup(&"B"), None);

        // Drop the last reference to the (already popped, inactive) scope
        // B. This must not panic and must not touch the map: it is purely
        // an `Rc`/`ScopeData` deallocation.
        b_ptr.reset();

        let c_ptr;
        {
            // C is a sibling of (the now-freed) B, also a child of outer.
            let c = Scope::new(&table);
            table.try_emplace("C", "c");
            c_ptr = c.ptr();
        }

        // A different retained sibling can still be activated normally.
        table.activate_scope(&c_ptr);
        assert_eq!(table.lookup(&"A"), Some("a"));
        assert_eq!(table.lookup(&"C"), Some("c"));
        assert_eq!(table.lookup(&"B"), None);

        // Restore to `outer` so it can be popped cleanly by its `Drop`.
        table.activate_scope(&outer.ptr());
        drop(outer);
    }
}
