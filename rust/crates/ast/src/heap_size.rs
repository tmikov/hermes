//! HeapSize trait — measures heap-allocated memory for a type.

/// Trait for allowing users to query how much memory a type uses in the heap.
pub trait HeapSize {
    /// Return the size of the heap allocated memory for the type, in bytes.
    fn heap_size(&self) -> usize;
}
