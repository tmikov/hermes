/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Typed ids for sema entities, backed by `ast::SemaId` (lib.rs:16).
//! Port of the identity discipline used by `hermes::sema::Decl`,
//! `hermes::sema::LexicalScope`, and `hermes::sema::FunctionInfo`
//! (`include/hermes/Sema/SemContext.h`): those C++ classes are allocated
//! once and referenced thereafter by (typed) pointer; here they are
//! referenced by a typed, `u32`-sized index instead, since the AST side
//! only has an opaque `SemaId` slot (`ast::node_child`) to store one in.

use ast::SemaId;

/// Declares a `u32` newtype that converts to/from [`SemaId`].
macro_rules! declare_sema_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name(u32);

        impl $name {
            /// Wraps a raw `SemaId` produced elsewhere (e.g. stored on an
            /// AST node) as this specific id type.
            pub fn from_sema_id(id: SemaId) -> Self {
                $name(id.0)
            }

            /// Converts back to the untyped `SemaId` the AST side stores.
            pub fn sema_id(self) -> SemaId {
                SemaId(self.0)
            }

            /// Returns the `usize` index for side-table indexing (e.g. a
            /// `Vec<...>` of per-id data owned by `SemContext`).
            pub fn index(self) -> usize {
                self.0 as usize
            }
        }
    };
}

declare_sema_id!(
    /// Identity of a `Decl` (`hermes::sema::Decl`, SemContext.h:54).
    DeclId
);
declare_sema_id!(
    /// Identity of a `LexicalScope` (`hermes::sema::LexicalScope`,
    /// SemContext.h:230).
    ScopeId
);
declare_sema_id!(
    /// Identity of a `FunctionInfo` (`hermes::sema::FunctionInfo`,
    /// SemContext.h:291).
    FunctionInfoId
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decl_id_round_trips_through_sema_id() {
        let decl = DeclId::from_sema_id(SemaId(7));
        let sema = decl.sema_id();
        assert_eq!(sema, SemaId(7));
        let back = DeclId::from_sema_id(sema);
        assert_eq!(back, decl);
        assert_eq!(back.index(), 7);
    }

    #[test]
    fn scope_id_round_trips_through_sema_id() {
        let scope = ScopeId::from_sema_id(SemaId(3));
        assert_eq!(scope.sema_id(), SemaId(3));
        assert_eq!(scope.index(), 3);
    }

    #[test]
    fn function_info_id_round_trips_through_sema_id() {
        let f = FunctionInfoId::from_sema_id(SemaId(42));
        assert_eq!(f.sema_id(), SemaId(42));
        assert_eq!(f.index(), 42);
    }

    #[test]
    fn ids_are_copy_eq_hash_debug() {
        // Compile-time check that the derives requested by the brief are
        // present: Copy (no `.clone()` needed), Eq, Hash (usable as a map
        // key), Debug (formattable).
        use std::collections::HashSet;
        let a = DeclId::from_sema_id(SemaId(1));
        let b = a; // Copy
        let mut set = HashSet::new();
        set.insert(a);
        assert!(set.contains(&b));
        assert_eq!(format!("{a:?}"), format!("{b:?}"));
    }
}
