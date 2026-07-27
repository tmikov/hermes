//! Hermes ESTree AST — GC arena (juno-derived) + node model.
//! See doc/superpowers/specs/2026-06-03-ast-design.md.

pub mod context;
pub mod dump;
pub mod node;
pub mod node_child;
pub mod visitor;

pub use support::HeapSize;

/// Placeholder for a resolved Sema entity (scope / decl / function info).
/// The real representation is pinned when Sema is ported; the AST only needs
/// an opaque, `Cell`-mutable handle until then.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemaId(pub u32);

/// Unique, never-reused identity of an AST node within its `Context`.
/// Assigned by `Context::alloc` from a monotonic counter (starting at 1);
/// `UNASSIGNED` (0) only exists on metadata not yet stored in the arena.
/// Consumers outside sema key side tables by NodeId (see the Sema design
/// spec §3.1): unlike raw addresses, ids never alias after arena slot
/// reuse; unlike NodeRc keys, they don't pin garbage. Insert entries only
/// with the node in hand under GCLock — a stored id may already be dead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u32);

impl NodeId {
    pub const UNASSIGNED: NodeId = NodeId(0);
}
