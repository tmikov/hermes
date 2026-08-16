/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Per-topic node-printing modules. `dispatch::GenJS::gen_node` matches
//! every [`hermes_ast::node::NodeKind`] and delegates to one `gen_*` method
//! per kind, defined here; grouping those methods by topic (rather than one
//! giant `match` the way juno's `gen_js.rs` does) is the deliberate
//! structural divergence the plan's File Structure section calls out.
//!
//! Each `gen_*` method receives the already-matched inner struct (or, when
//! it needs to build a [`hermes_ast::visitor::Path`] for a recursive call,
//! the enclosing `&'gc Node<'gc>` too) and destructures every field of that
//! struct by name — never `..` — so that a field added to the AST later is
//! a compile error here, exactly as `dispatch.rs`'s own exhaustive kind
//! match makes an added *kind* a compile error.

pub(crate) mod expr;
pub(crate) mod flow_decl;
pub(crate) mod flow_type;
pub(crate) mod func;
pub(crate) mod jsx;
pub(crate) mod literal;
pub(crate) mod module;
pub(crate) mod newer;
pub(crate) mod stmt;
pub(crate) mod ts;
