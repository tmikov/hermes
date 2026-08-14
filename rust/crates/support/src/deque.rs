/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use crate::HeapSize;

/// Append-only deque which ensures the elements pushed into it never move.
/// Allocates chunks in doubling capacities.
#[derive(Debug)]
pub struct Deque<T> {
    storage: Vec<Vec<T>>,

    /// Capacity at which to allocate the next chunk.
    /// Doubles every chunk until reaching [`MAX_CHUNK_CAPACITY`].
    next_chunk_capacity: usize,
}

/// Minimum chunk capacity in the deque.
/// May be made configurable in the future.
const MIN_CHUNK_CAPACITY: usize = 1 << 10;

/// Maximum chunk capacity in the deque.
/// May be made configurable in the future.
const MAX_CHUNK_CAPACITY: usize = MIN_CHUNK_CAPACITY * (1 << 10);

impl<T> Default for Deque<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Deque<T> {
    pub fn new() -> Self {
        let mut result = Self {
            storage: Default::default(),
            next_chunk_capacity: MIN_CHUNK_CAPACITY,
        };
        result.new_chunk();
        result
    }

    /// Append an element to the deque and return a reference to it.
    /// The element will not move after it is allocated.
    pub fn push(&mut self, val: T) -> &T {
        let chunk = self.storage.last().unwrap();
        if chunk.len() >= chunk.capacity() {
            self.new_chunk();
        }
        let chunk = self.storage.last_mut().unwrap();
        debug_assert!(
            chunk.len() < chunk.capacity(),
            "Invalid attempt to expand a chunk"
        );
        chunk.push(val);
        chunk.last().unwrap()
    }

    /// Return the number of elements that have been appended to the deque.
    pub fn len(&self) -> usize {
        let mut result = 0;
        for chunk in &self.storage {
            result += chunk.len();
        }
        result
    }

    pub fn is_empty(&self) -> bool {
        self.storage.is_empty()
    }

    /// Iterator over every element of the deque.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.storage.iter().flatten()
    }

    /// Iterator over every element of the deque.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.storage.iter_mut().flatten()
    }

    /// Truncate the deque to `len` elements, dropping every element at
    /// index >= `len` and freeing fully-vacated trailing chunks. Surviving
    /// elements never move (only trailing elements/chunks are dropped), so
    /// references to them remain valid. Used by the AST arena's
    /// `AllocationScope` (bump-allocator save/restore semantics, mirroring
    /// the C++ `BumpPtrAllocator::pushScope`/`popScope`,
    /// hermes/Support/Allocator.h:500).
    pub fn truncate(&mut self, len: usize) {
        debug_assert!(len <= self.len(), "truncate beyond deque length");
        let mut remaining = len;
        let mut keep = 0usize; // number of chunks to keep
        for chunk in &mut self.storage {
            keep += 1;
            if remaining < chunk.len() {
                chunk.truncate(remaining);
                break;
            }
            remaining -= chunk.len();
            if remaining == 0 {
                break;
            }
        }
        // Always keep at least one chunk: `push` assumes storage is
        // non-empty (deque.rs `new()` pre-creates chunk 0).
        self.storage.truncate(keep.max(1));
    }

    /// Iterate over the elements starting at `index`. Positions by chunk
    /// arithmetic (a handful of chunk-boundary comparisons; skipped
    /// elements are not walked), so iterating a suffix is O(suffix).
    /// An `index` at or past `len()` yields an empty iterator.
    pub fn iter_from(&self, index: usize) -> impl Iterator<Item = &T> {
        let mut skip = index;
        let mut start_chunk = self.storage.len();
        for (i, chunk) in self.storage.iter().enumerate() {
            if skip < chunk.len() {
                start_chunk = i;
                break;
            }
            skip -= chunk.len();
        }
        self.storage[start_chunk..]
            .iter()
            .enumerate()
            .flat_map(move |(i, chunk)| {
                let s = if i == 0 { skip } else { 0 };
                chunk[s..].iter()
            })
    }

    /// Allocate a new chunk in the node storage.
    fn new_chunk(&mut self) {
        let capacity = self.next_chunk_capacity;
        self.storage.push(Vec::with_capacity(capacity));

        // Double the capacity if there's room.
        if capacity < MAX_CHUNK_CAPACITY {
            self.next_chunk_capacity = capacity * 2;
        }
    }
}

impl<T> HeapSize for Deque<T> {
    fn heap_size(&self) -> usize {
        let mut result = 0;
        for chunk in &self.storage {
            result += chunk.heap_size();
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append() {
        let mut d = Deque::new();
        d.push(1);
        d.push(2);
        assert_eq!(d.iter().count(), 2);
    }

    #[test]
    fn multi_chunks() {
        let mut d = Deque::<usize>::new();
        let count = MIN_CHUNK_CAPACITY * 2;
        let mut addr = 0usize;
        for i in 0..count {
            let elem = d.push(i);
            if i == 1000 {
                addr = elem as *const usize as usize;
            }
        }
        assert_eq!(d.iter().count(), count);
        // The element at index 1000 must not have moved (stable addresses):
        // re-fetch it through the iterator and confirm address + value are unchanged.
        let again = d.iter().nth(1000).unwrap();
        assert_eq!(again as *const usize as usize, addr);
        assert_eq!(*again, 1000);
    }

    #[test]
    fn truncate_within_and_across_chunks() {
        // 2500 elements spans chunk 0 (1024) and chunk 1 (2048 capacity).
        let mut d = Deque::new();
        for i in 0..2500usize {
            d.push(i);
        }
        assert_eq!(d.len(), 2500);
        // Truncate within chunk 1.
        d.truncate(1500);
        assert_eq!(d.len(), 1500);
        assert_eq!(d.iter().copied().last(), Some(1499));
        // Survivors intact and re-push works.
        assert_eq!(d.iter().nth(1023).copied(), Some(1023));
        d.push(9999);
        assert_eq!(d.len(), 1501);
        assert_eq!(d.iter().copied().last(), Some(9999));
        // Truncate dropping the whole trailing chunk.
        d.truncate(500);
        assert_eq!(d.len(), 500);
        // Truncate to zero leaves a usable deque.
        d.truncate(0);
        assert_eq!(d.len(), 0);
        d.push(1);
        assert_eq!(d.len(), 1);
        // Truncate to exactly the current length is a no-op.
        d.truncate(1);
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn iter_from_positions_correctly() {
        let mut d = Deque::new();
        for i in 0..2500usize {
            d.push(i);
        }
        // Mid-chunk-1 start.
        let v: Vec<usize> = d.iter_from(1030).copied().take(3).collect();
        assert_eq!(v, vec![1030, 1031, 1032]);
        // Exactly at a chunk boundary.
        assert_eq!(d.iter_from(1024).copied().next(), Some(1024));
        // From zero == full iteration.
        assert_eq!(d.iter_from(0).count(), 2500);
        // From len() and beyond: empty.
        assert_eq!(d.iter_from(2500).count(), 0);
        assert_eq!(d.iter_from(9999).count(), 0);
    }
}
