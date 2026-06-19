/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! TypeScript object-type bodies: the `{ ... }` type literal with its
//! property/method/call/index-signature members. Port of the object type entry
//! points of `lib/Parser/JSParserImpl-ts.cpp`.

use ast::node::{
    Identifier, Node, TSCallSignatureDeclaration, TSIndexSignature,
    TSMethodSignature, TSPropertySignature, TSTypeLiteral,
};
use ast::node_child::{NodeList, NodeMetadata};

use crate::js::flow::{AllowTypedArrowFunction, CoverTypedParameters};
use crate::js::{JSParserImpl, Param, PARAM_IN};
use crate::lexer::GrammarContext;
use crate::token_kinds::TokenKind;

impl<'gc, 'ast, 'ctx, 'a> JSParserImpl<'gc, 'ast, 'ctx, 'a> {
    // -----------------------------------------------------------------------
    // parseTSObjectType — 1192 in JSParserImpl-ts.cpp
    // -----------------------------------------------------------------------

    /// Parse a TS object type literal `{ member; member; ... }`, with the
    /// current token at `{`. Port of `JSParserImpl::parseTSObjectType`
    /// (ts.cpp:1192-1224).
    pub(super) fn parse_ts_object_type(&mut self) -> Option<&'gc Node<'gc>> {
        // C++ 1193-1194.
        debug_assert!(self.check(TokenKind::l_brace));
        let start = self.advance(GrammarContext::Type).start;

        let mut members: Vec<&'gc Node<'gc>> = Vec::new();

        // C++ 1198-1210.
        while !self.check(TokenKind::r_brace) {
            members.push(self.parse_ts_object_type_member()?);

            // C++ 1204-1209: members are separated by `,` or `;`; the trailing
            // separator is optional.
            let has_next = self.check2(TokenKind::comma, TokenKind::semi);
            if has_next {
                self.advance(GrammarContext::Type);
            } else {
                break;
            }
        }

        // C++ 1212-1218.
        if !self.eat(
            TokenKind::r_brace,
            GrammarContext::Type,
            " at end of object type",
        ) {
            return None;
        }

        // C++ 1220-1223.
        let node = Node::TSTypeLiteral(TSTypeLiteral::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, members),
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseTSObjectTypeMember — 1226 in JSParserImpl-ts.cpp
    // -----------------------------------------------------------------------

    /// Parse one member of a TS object type: a call signature, property
    /// signature, method signature, or index signature. Port of
    /// `JSParserImpl::parseTSObjectTypeMember` (ts.cpp:1226-1363).
    pub(super) fn parse_ts_object_type_member(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 1227.
        let start = self.cur_start();

        // C++ 1229-1245: a call signature `(params): R`.
        if self.check(TokenKind::l_paren) {
            let mut params: Vec<&'gc Node<'gc>> = Vec::new();
            if !self.parse_ts_function_type_params(start, &mut params) {
                return None;
            }
            let mut return_type: Option<&'gc Node<'gc>> = None;
            if self.check_and_eat(TokenKind::colon, GrammarContext::Type) {
                return_type = Some(self.parse_type_annotation_ts(None)?);
            }
            let node = Node::TSCallSignatureDeclaration(
                TSCallSignatureDeclaration::new(
                    NodeMetadata::new(self.dummy_range()),
                    NodeList::from_iter(self.gc, params),
                    return_type,
                ),
            );
            return Some(self.set_location(
                start,
                self.lexer.prev_token_end(),
                node,
            ));
        }

        // C++ 1247-1248.
        let mut optional = false;
        let mut computed = false;

        // C++ 1250-1253: TODO: Parse modifiers.
        // TODO: Parse modifiers.
        let readonly = false;
        let is_static = false;
        let is_export = false;

        let key: &'gc Node<'gc>;

        // C++ 1257-1258: TODO: Parse initializer.
        // TODO: Parse initializer.
        let init: Option<&'gc Node<'gc>> = None;

        // C++ 1260-1300.
        if self.check_and_eat(TokenKind::l_square, GrammarContext::Type) {
            computed = true;

            // C++ 1263-1269.
            if self.check(TokenKind::identifier) {
                // C++ 1264: lookahead1(None) — default RequireNoNewLine=true.
                let opt_next = self.lexer.lookahead1::<true>(None);
                if opt_next == Some(TokenKind::colon) {
                    // Unambiguously an index signature.
                    return self.parse_ts_index_signature(start);
                }
            }

            // C++ 1271-1274: parseAssignmentExpression(ParamIn).
            key = self.parse_assignment_expression(
                PARAM_IN,
                AllowTypedArrowFunction::Yes,
                CoverTypedParameters::Yes,
                None,
            )?;

            // C++ 1276-1282.
            if !self.eat(
                TokenKind::r_square,
                GrammarContext::Type,
                " at end of computed property type",
            ) {
                return None;
            }

            // C++ 1284-1286.
            if self.check_and_eat(TokenKind::question, GrammarContext::Type) {
                optional = true;
            }
        } else {
            // C++ 1288-1295.
            if !self.need(TokenKind::identifier, " in property") {
                return None;
            }
            let key_range = self.cur_range();
            let key_node = Node::Identifier(Identifier::new(
                NodeMetadata::new(self.dummy_range()),
                self.lexer.token().get_identifier(),
                None,
                false,
            ));
            key = self.set_location(key_range.start, key_range.end, key_node);
            self.advance(GrammarContext::Type);

            // C++ 1297-1299.
            if self.check_and_eat(TokenKind::question, GrammarContext::Type) {
                optional = true;
            }
        }

        // C++ 1302-1319: a property signature with an explicit type.
        if self.check(TokenKind::colon) {
            let annot_start = self.advance(GrammarContext::Type).start;
            let opt_type = self.parse_type_annotation_ts(Some(annot_start))?;
            let node =
                Node::TSPropertySignature(TSPropertySignature::new(
                    NodeMetadata::new(self.dummy_range()),
                    key,
                    Some(opt_type),
                    init,
                    optional,
                    computed,
                    readonly,
                    is_static,
                    is_export,
                ));
            return Some(self.set_location(
                start,
                self.lexer.prev_token_end(),
                node,
            ));
        }

        // C++ 1321-1340: a method signature `(params): R`.
        if self.check(TokenKind::l_paren) {
            let mut params: Vec<&'gc Node<'gc>> = Vec::new();
            if !self.parse_ts_function_type_params(start, &mut params) {
                return None;
            }

            let mut return_type: Option<&'gc Node<'gc>> = None;
            if self.check(TokenKind::colon) {
                let annot_start = self.advance(GrammarContext::Type).start;
                return_type =
                    Some(self.parse_type_annotation_ts(Some(annot_start))?);
            }

            let node = Node::TSMethodSignature(TSMethodSignature::new(
                NodeMetadata::new(self.dummy_range()),
                key,
                NodeList::from_iter(self.gc, params),
                return_type,
                computed,
            ));
            return Some(self.set_location(
                start,
                self.lexer.prev_token_end(),
                node,
            ));
        }

        // C++ 1342-1362: a bare property signature with an optional type.
        let mut return_type: Option<&'gc Node<'gc>> = None;
        if self.check(TokenKind::colon) {
            let annot_start = self.advance(GrammarContext::Type).start;
            return_type =
                Some(self.parse_type_annotation_ts(Some(annot_start))?);
        }

        let node = Node::TSPropertySignature(TSPropertySignature::new(
            NodeMetadata::new(self.dummy_range()),
            key,
            return_type,
            init,
            optional,
            computed,
            readonly,
            is_static,
            is_export,
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseTSIndexSignature — 1365 in JSParserImpl-ts.cpp
    // -----------------------------------------------------------------------

    /// Parse a TS index signature `[id, ...]: R`, with `start` the location of
    /// the opening `[` (already consumed by the caller) and the current token
    /// at the first parameter. Port of `JSParserImpl::parseTSIndexSignature`
    /// (ts.cpp:1365-1404).
    fn parse_ts_index_signature(
        &mut self,
        start: support::location::SMLoc,
    ) -> Option<&'gc Node<'gc>> {
        let mut params: Vec<&'gc Node<'gc>> = Vec::new();

        // C++ 1368-1380.
        while !self.check(TokenKind::r_square) {
            match self.parse_binding_identifier(Param::default()) {
                Some(key) => params.push(key),
                None => {
                    // C++ 1371-1374: errorExpected(identifier, "in property",
                    // ...). The `what`/`whatLoc` note args are dropped per house
                    // style.
                    self.need(TokenKind::identifier, " in property");
                    return None;
                }
            }

            if !self.check_and_eat(TokenKind::comma, GrammarContext::Type) {
                break;
            }
        }

        // C++ 1382-1388.
        if !self.eat(
            TokenKind::r_square,
            GrammarContext::Type,
            " at end of indexer type annotation",
        ) {
            return None;
        }

        // C++ 1390-1397.
        let mut return_type: Option<&'gc Node<'gc>> = None;
        if self.check(TokenKind::colon) {
            let annot_start = self.advance(GrammarContext::Type).start;
            return_type =
                Some(self.parse_type_annotation_ts(Some(annot_start))?);
        }

        // C++ 1399-1403.
        let node = Node::TSIndexSignature(TSIndexSignature::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, params),
            return_type,
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }
}
