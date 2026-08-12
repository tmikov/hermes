/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#![forbid(unsafe_code)]

//! Hermes semantic analysis (Rust port).
//!
//! Source of truth in the C++ tree:
//! - `include/hermes/Sema/SemContext.h` (`Decl`, `LexicalScope`,
//!   `FunctionInfo` — see `hermes_sema::ids`)
//! - `include/hermes/AST/Context.h` (`Keywords`, line 168) and
//!   `include/hermes/AST/Keywords.def` (see `hermes_sema::keywords`)
//! - `lib/Sema/SemanticResolver.cpp` / `include/hermes/Sema/SemResolve.h`
//!   (the validator/resolver this crate will host as later tasks land)

pub mod ast_eval;
// Private for the same reason its C++ counterpart is declared in the internal
// `lib/Sema/SemanticResolver.h` rather than in `SemResolve.h` — see the
// module's own doc.
mod check_implicit_return;
pub mod decl_collector;
pub mod dump;
pub mod dump_context;
pub mod ids;
pub mod keywords;
pub mod libhermes;
mod linearize;
pub mod resolve;
pub mod resolver;
pub mod sem_context;
