#![warn(missing_docs)]
//! Hermes ESTree AST — GC arena (juno-derived) + node model.
//!
//! The node set in [`node`] is generated from
//! `include/hermes/AST/ESTree.def` with every parse family enabled — Flow,
//! TypeScript, JSX, and the parser's "cover" grammar nodes — so a single
//! [`node::Node`] enum spans all of them.
//!
//! The pieces a consumer touches:
//! - [`context::Context`] and [`context::GCLock`] — the arena that owns the
//!   nodes and the lock through which they are allocated and read.
//! - [`node::Node`] — one enum arm per node kind, over `#[repr(C)]` structs
//!   whose fields mirror the `.def` entry: structural children are `&'gc`
//!   references or [`node_child::NodeList`]s, everything else is a `Cell`.
//! - [`visitor::VisitorMut`] plus `Node::visit_children_mut` — transforms
//!   that rebuild only the spine whose children changed.
//! - [`dump::ESTreeJSONDumper`] — ESTree JSON matching
//!   `hermesc -dump-ast -dump-source-location=both` byte for byte.
//!
//! See `rust/ARCHITECTURE.md` for the design rationale and
//! doc/superpowers/specs/2026-06-03-ast-design.md for the port spec.

pub mod context;
pub mod dump;
pub mod node;
pub mod node_child;
pub mod visitor;

pub use hermes_support::HeapSize;

/// Opaque handle to a resolved Sema entity (scope / decl / function info).
/// The AST only stores the raw index in a `Cell`; the `sema` crate wraps it
/// in the typed newtypes of its `ids` module and owns the interpretation.
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
    /// The id of a node that is not (yet) stored in the arena. Never returned
    /// by `Context::alloc`, whose counter starts at 1.
    pub const UNASSIGNED: NodeId = NodeId(0);
}
