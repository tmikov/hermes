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
//! predicates, and return-type annotations; P5.3 `opaque type` aliases,
//! `interface` declarations and type annotations, and class `implements`
//! entries. P6 added the rest of Flow: the ambiguous-expression grammar
//! (typed arrows, `as`/`as const`, type-casts, call/new/optional-chain
//! type-args), plus `enum`, `component`/`hook`, `record`, `match`, and the
//! `declare` statement family with `import type`/`export type` clauses. Only
//! TS (P7) and JSX remain.
//!
//! The `impl JSParserImpl` methods are split across the child modules below
//! by concern, mirroring the `lexer/` directory split: `declarations` (the
//! declaration gate, `type`/`opaque type` aliases, and `interface`
//! declarations), `types` (the
//! annotation precedence hierarchy and reparse helpers), `function_types`
//! (function types, predicates, return types), `object_types` (object-type
//! bodies), and `params` (type parameters/arguments and generic type
//! references). The shared enums and helpers live here; methods called
//! across child-module boundaries are `pub(super)`.

mod declarations;
mod function_types;
mod match_;
mod object_types;
mod params;
mod types;

use ast::node::{Node, RecordExpression, RecordExpressionProperties};
use ast::node_child::{NodeList, NodeMetadata};
use support::location::SMLoc;

use crate::js::JSParserImpl;
use crate::lexer::GrammarContext;
use crate::token_kinds::{ord, TokenKind};

impl<'gc, 'ast, 'ctx, 'a> JSParserImpl<'gc, 'ast, 'ctx, 'a> {
    // -----------------------------------------------------------------------
    // checkRecordExpressionFlow — 1929 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Whether `expr` followed by the current `{` forms a `record` expression.
    /// Port of `JSParserImpl::checkRecordExpressionFlow` (flow.cpp:1929-1946).
    ///
    /// The current token must be `{` with no newline before it; `expr` must be
    /// either an Identifier with a non-empty name whose first character is NOT
    /// a lowercase ascii letter `a`-`z`, or any MemberExpression.
    pub(in crate::js) fn check_record_expression_flow(
        &self,
        expr: &Node<'gc>,
    ) -> bool {
        // C++ 1930-1932.
        if !self.check(TokenKind::l_brace)
            || self.lexer.is_new_line_before_current_token()
        {
            return false;
        }
        // C++ 1933-1940: record expression names cannot begin with lowercase
        // 'a'-'z'.
        if let Node::Identifier(ident) = expr {
            let name = self.gc.ctx().atom_table.bytes(ident.name.get());
            if name.is_empty() || (name[0] >= b'a' && name[0] <= b'z') {
                return false;
            }
            return true;
        }
        // C++ 1941-1944: member expressions are always allowed.
        matches!(expr, Node::MemberExpression(_))
    }

    // -----------------------------------------------------------------------
    // parseRecordExpressionFlow — 1948 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a `record` expression body — `Constructor[<TypeArgs>] { props }` —
    /// with the cursor at `{`. Port of
    /// `JSParserImpl::parseRecordExpressionFlow` (flow.cpp:1948-1979).
    pub(in crate::js) fn parse_record_expression_flow(
        &mut self,
        start_loc: SMLoc,
        constructor: &'gc Node<'gc>,
        type_args: Option<&'gc Node<'gc>>,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 1952-1953.
        debug_assert!(self.check(TokenKind::l_brace));
        let properties_start_loc = self.advance(GrammarContext::AllowRegExp).start;

        // C++ 1955-1957.
        let mut elem_list: Vec<&'gc Node<'gc>> = Vec::new();
        if !self.parse_object_properties(&mut elem_list) {
            return None;
        }

        // C++ 1959-1966: the record-expression `}` is eaten in AllowDiv.
        let end_loc = self.cur_range().end;
        if !self.eat_at(
            TokenKind::r_brace,
            GrammarContext::AllowDiv,
            " at end of record expression '{...'",
            Some("location of '{'"),
            properties_start_loc,
        ) {
            return None;
        }

        // C++ 1968-1972.
        let props_node = Node::RecordExpressionProperties(
            RecordExpressionProperties::new(
                NodeMetadata::new(self.dummy_range()),
                NodeList::from_iter(self.gc, elem_list),
            ),
        );
        let properties =
            self.set_location(properties_start_loc, end_loc, props_node);

        // C++ 1974-1978.
        let node = Node::RecordExpression(RecordExpression::new(
            NodeMetadata::new(self.dummy_range()),
            constructor,
            type_args,
            properties,
        ));
        Some(self.set_location(start_loc, end_loc, node))
    }
}

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
#[allow(dead_code)] // DeclareOpaque is constructed by parseDeclareFLow (P6).
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

/// Whether a typed arrow function (`<T>(x: T): T => …` / `(x): T => …`) may be
/// recognized at this assignment-expression position. Port of the C++ runtime
/// enum `JSParserImpl::AllowTypedArrowFunction` (JSParserImpl.h:1133). Kept as a
/// runtime enum param (faithful — it is a runtime enum in C++ too), NOT a bool
/// or const-generic.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum AllowTypedArrowFunction {
    No,
    Yes,
}

/// Whether a `CoverTypedIdentifier` node (`x: T` / `x?: T` inside what might be
/// arrow parameters) may be produced at this position. Port of the C++ runtime
/// enum `JSParserImpl::CoverTypedParameters` (JSParserImpl.h:1014). Kept as a
/// runtime enum param (faithful), NOT a bool or const-generic.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum CoverTypedParameters {
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
pub(super) enum AllowSpreadProperty {
    No,
    Yes,
}
