/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Flow type-grammar parsing for the JS parser. Port of
//! `lib/Parser/JSParserImpl-flow.cpp`.
//!
//! P5.0 implemented the Flow declaration gate (`parseFlowDeclaration`) and
//! the plain `type X = T;` alias pipeline; P5.1 the full type-annotation
//! precedence hierarchy (`parseConditionalTypeAnnotationFlow` →
//! `parsePrimaryTypeAnnotationFlow`), generic types, type arguments,
//! `typeof`/tuple/`keyof`/`infer` types, and the reparse helpers; P5.2
//! function types, object types, type-parameter declarations, variance,
//! predicates, and return-type annotations. The remaining productions emit
//! an honest "unsupported (parser phase P5.x)" error at the marked site; the
//! later sub-tasks (P5.3 interfaces / declare / opaque, P6 enum / component /
//! hook / record / match) replace those markers with the real grammar.
//!
//! The `impl JSParserImpl` methods are split across the child modules below
//! by concern, mirroring the `lexer/` directory split: `declarations` (the
//! declaration gate and `type`/`opaque type` aliases), `types` (the
//! annotation precedence hierarchy and reparse helpers), `function_types`
//! (function types, predicates, return types), `object_types` (object-type
//! bodies), and `params` (type parameters/arguments and generic type
//! references). The shared enums and helpers live here; methods called
//! across child-module boundaries are `pub(super)`.

mod declarations;
mod function_types;
mod object_types;
mod params;
mod types;

use crate::token_kinds::{ord, TokenKind};

/// Check if the given token kind can follow a contextual variance keyword
/// (`readonly` or `writeonly`) in Flow mode. Used to disambiguate the
/// keyword as a variance annotation from the keyword used as a property name.
/// Port of `JSParserImpl::canFollowVarianceKeywordFlow`
/// (JSParserImpl.h:1666-1689).
pub(super) fn can_follow_variance_keyword_flow(
    opt_token_kind: Option<TokenKind>,
) -> bool {
    let Some(kind) = opt_token_kind else {
        return false;
    };
    // Reserved words (e.g. `with`, `enum`, `default`, `new`) are valid
    // property names, so `readonly <reservedWord>:` should be parsed as a
    // variance modifier on a reserved-word property — the same way
    // `+<reservedWord>:` already is.
    if ord(kind) > ord(TokenKind::_first_resword)
        && ord(kind) < ord(TokenKind::_last_resword)
    {
        return true;
    }
    matches!(
        kind,
        TokenKind::identifier
            | TokenKind::private_identifier
            | TokenKind::string_literal
            | TokenKind::numeric_literal
            | TokenKind::bigint_literal
            | TokenKind::l_square
    )
}

/// Which alias declaration form is being parsed.
/// Port of `JSParserImpl::TypeAliasKind` (JSParserImpl.h:1390).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(dead_code)] // DeclareOpaque is constructed by parseDeclareFLow (P5.3).
pub(super) enum TypeAliasKind {
    None,
    Declare,
    Opaque,
    DeclareOpaque,
}

/// Whether an anonymous function type (`T => U` without parentheses) is
/// allowed when parsing a type annotation.
/// Port of `JSParserImpl::AllowAnonFunctionType` (JSParserImpl.h:1207).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum AllowAnonFunctionType {
    No,
    Yes,
}

/// Whether a `proto` property modifier is allowed in an object type.
/// Port of `JSParserImpl::AllowProtoProperty` (JSParserImpl.h:1444).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(dead_code)] // `Yes` is passed by declare-class bodies (P6).
pub(super) enum AllowProtoProperty {
    No,
    Yes,
}

/// Whether a `static` property modifier is allowed in an object type.
/// Port of `JSParserImpl::AllowStaticProperty` (JSParserImpl.h:1447).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(dead_code)] // `Yes` is passed by declare-class bodies (P6).
pub(super) enum AllowStaticProperty {
    No,
    Yes,
}

/// Whether a `...T` spread property is allowed in an object type.
/// Port of `JSParserImpl::AllowSpreadProperty` (JSParserImpl.h:1450).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(dead_code)] // `No` is passed by interface bodies (P5.3).
pub(super) enum AllowSpreadProperty {
    No,
    Yes,
}
