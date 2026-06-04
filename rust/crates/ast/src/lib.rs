//! Hermes ESTree AST — GC arena (juno-derived) + node model.
//! See doc/superpowers/specs/2026-06-03-ast-design.md.

pub mod context;
pub mod node;
pub mod node_child;
pub mod visitor;

pub use support::HeapSize;

/// Placeholder for a resolved Sema entity (scope / decl / function info).
/// The real representation is pinned when Sema is ported; the AST only needs
/// an opaque, `Cell`-mutable handle until then.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemaId(pub u32);
