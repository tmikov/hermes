/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! TypeScript type-grammar parsing for the JS parser. Port of
//! `lib/Parser/JSParserImpl-ts.cpp`.
//!
//! TypeScript and Flow are mutually-exclusive dialects: `-parse-ts` does NOT
//! enable `-parse-flow`, and this module is reached only when
//! `context_.getParseTS()` is set. The structure mirrors the Flow port
//! (`js/flow/`) one-to-one so the two stay easy to compare.
//!
//! P7.0 landed the end-to-end scaffolding: the declaration gate
//! (`parse_ts_declaration`), the `type X = T;` alias pipeline, and the
//! type-annotation precedence hierarchy down to the `string`/`number`
//! primitive keyword arms. P7.1 fills in the type-grammar core: the full
//! `parse_type_annotation_ts` (predicate backtrack, conditional type), the
//! complete `parse_ts_primary_type` switch (all primitive keywords, literals,
//! `this`, `*`, tuples, `typeof`, references), type references/qualified
//! names, type queries, type parameters/arguments, and the
//! `reparse_identifier_as_ts_type_annotation` helper. The
//! parenthesized/function (`(`/`new`/`<`) → P7.2, object (`{`) → P7.3, and
//! `interface` → P7.4 arms remain honest parse errors.
//!
//! The `impl JSParserImpl` methods are split across the child modules below by
//! concern, mirroring the Flow split: `declarations` (the declaration gate and
//! `type` aliases), `types` (the annotation precedence hierarchy),
//! `function_types` (function/constructor/parenthesized types),
//! `object_types` (object-type bodies), and `params` (type
//! parameters/arguments and generic type references). The shared enums and
//! helpers live here; methods called across child-module boundaries are
//! `pub(super)`.

mod declarations;
mod function_types;
mod object_types;
mod params;
mod types;

/// Whether a parenthesized type is a constructor type (`new (...) => T`).
/// Port of `JSParserImpl::IsConstructorType` (JSParserImpl.h:1599). Runtime
/// enum (faithful), NOT a bool.
#[allow(dead_code)] // Consumed by parseTSFunctionOrParenthesizedType (P7.2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum IsConstructorType {
    No,
    Yes,
}
