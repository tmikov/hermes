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
//!   `FunctionInfo` — see `sema::ids`)
//! - `include/hermes/AST/Context.h` (`Keywords`, line 168) and
//!   `include/hermes/AST/Keywords.def` (see `sema::keywords`)
//! - `lib/Sema/SemanticValidator.cpp` / `include/hermes/Sema/SemResolve.h`
//!   (the validator/resolver this crate will host as later tasks land)

pub mod decl_collector;
pub mod dump;
pub mod dump_context;
pub mod ids;
pub mod keywords;
pub mod sem_context;
