/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! TypeScript type parameters/arguments. Port of the corresponding entry
//! points of `lib/Parser/JSParserImpl-ts.cpp`:
//! `parseTSTypeParameters` (the `<T, U = V>` declaration on a generic),
//! `parseTSTypeParameter` (a single parameter with optional `extends`
//! constraint and `=` default), and `parseTSTypeArguments` (the `<A, B>`
//! instantiation on a generic reference).

use ast::node::{
    Identifier, Node, TSTypeParameter, TSTypeParameterDeclaration,
    TSTypeParameterInstantiation,
};
use ast::node_child::{NodeList, NodeMetadata};

use crate::js::JSParserImpl;
use crate::lexer::GrammarContext;
use crate::token_kinds::TokenKind;

impl<'gc, 'ast, 'ctx, 'a> JSParserImpl<'gc, 'ast, 'ctx, 'a> {
    // -----------------------------------------------------------------------
    // parseTSTypeParameters — 801 in JSParserImpl-ts.cpp
    // -----------------------------------------------------------------------

    /// Parse a TS type-parameter declaration (`<T, U = V>`).
    /// Port of `JSParserImpl::parseTSTypeParameters` (ts.cpp:801-830).
    pub(super) fn parse_ts_type_parameters(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 802-803.
        debug_assert!(self.check(TokenKind::less));
        let start = self.advance(GrammarContext::Type).start;

        let mut params: Vec<&'gc Node<'gc>> = Vec::new();

        // C++ 807-815.
        loop {
            params.push(self.parse_ts_type_parameter()?);

            if !self.check_and_eat(TokenKind::comma, GrammarContext::Type) {
                break;
            }
            if self.check(TokenKind::greater) {
                break;
            }
        }

        // C++ 817-824.
        let end = self.cur_range().end;
        if !self.eat(
            TokenKind::greater,
            GrammarContext::Type,
            " at end of type parameters",
        ) {
            return None;
        }

        // C++ 826-829.
        let node =
            Node::TSTypeParameterDeclaration(TSTypeParameterDeclaration::new(
                NodeMetadata::new(self.dummy_range()),
                NodeList::from_iter(self.gc, params),
            ));
        Some(self.set_location(start, end, node))
    }

    // -----------------------------------------------------------------------
    // parseTSTypeParameter — 832 in JSParserImpl-ts.cpp
    // -----------------------------------------------------------------------

    /// Parse a single TS type parameter (`T`, `T extends C`, `T = D`).
    /// Port of `JSParserImpl::parseTSTypeParameter` (ts.cpp:832-864).
    fn parse_ts_type_parameter(&mut self) -> Option<&'gc Node<'gc>> {
        // C++ 833.
        let start = self.cur_start();

        // C++ 835-836.
        if !self.need(TokenKind::identifier, " in type parameter") {
            return None;
        }
        // C++ 837-842.
        let name_range = self.cur_range();
        let name_node = Node::Identifier(Identifier::new(
            NodeMetadata::new(self.dummy_range()),
            self.lexer.token().get_identifier(),
            None,
            false,
        ));
        let name = self.set_location(name_range.start, name_range.end, name_node);
        self.advance(GrammarContext::Type);

        // C++ 844-850.
        let mut constraint: Option<&'gc Node<'gc>> = None;
        if self.check_and_eat(TokenKind::rw_extends, GrammarContext::Type) {
            constraint = Some(self.parse_type_annotation_ts(None)?);
        }

        // C++ 852-858.
        let mut init: Option<&'gc Node<'gc>> = None;
        if self.check_and_eat(TokenKind::equal, GrammarContext::Type) {
            init = Some(self.parse_type_annotation_ts(None)?);
        }

        // C++ 860-863.
        let node = Node::TSTypeParameter(TSTypeParameter::new(
            NodeMetadata::new(self.dummy_range()),
            name,
            constraint,
            init,
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseTSTypeArguments — 1160 in JSParserImpl-ts.cpp
    // -----------------------------------------------------------------------

    /// Parse a TS type-argument list (`<A, B>`) on a generic reference.
    /// Port of `JSParserImpl::parseTSTypeArguments` (ts.cpp:1160-1190).
    pub(super) fn parse_ts_type_arguments(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 1161-1162.
        debug_assert!(self.check(TokenKind::less));
        let start = self.advance(GrammarContext::Type).start;

        let mut params: Vec<&'gc Node<'gc>> = Vec::new();

        // C++ 1166-1174.
        while !self.check(TokenKind::greater) {
            params.push(self.parse_type_annotation_ts(None)?);

            if !self.check_and_eat(TokenKind::comma, GrammarContext::Type) {
                break;
            }
        }

        // C++ 1176-1183.
        let end = self.cur_range().end;
        if !self.eat(
            TokenKind::greater,
            GrammarContext::Type,
            " at end of type parameters",
        ) {
            return None;
        }

        // C++ 1185-1189.
        let node = Node::TSTypeParameterInstantiation(
            TSTypeParameterInstantiation::new(
                NodeMetadata::new(self.dummy_range()),
                NodeList::from_iter(self.gc, params),
            ),
        );
        Some(self.set_location(start, end, node))
    }
}
