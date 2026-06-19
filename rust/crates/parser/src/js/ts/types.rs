/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The TypeScript type-annotation precedence hierarchy. Port of the
//! `parseTypeAnnotationTS`/`parseTSUnionType`/... entry points of
//! `lib/Parser/JSParserImpl-ts.cpp`.
//!
//! P7.0 implements the full hierarchy `parse_type_annotation_ts` →
//! `parse_ts_union_type` → `parse_ts_intersection_type` →
//! `parse_ts_postfix_type` → `parse_ts_primary_type`, where the
//! `union`/`intersection`/`postfix` levels are final (they match the C++
//! exactly) and only `parse_ts_primary_type` is a stub: it handles the
//! `string`/`number` keyword arms, and every other case is an honest parse
//! error pending later P7 tasks. P7.1 only adds primary-type arms.

use ast::node::{
    Node, TSArrayType, TSIndexedAccessType, TSIntersectionType,
    TSNumberKeyword, TSStringKeyword, TSTypeAnnotation, TSUnionType,
};
use ast::node_child::{NodeList, NodeMetadata};
use support::location::SMLoc;

use crate::js::JSParserImpl;
use crate::lexer::GrammarContext;
use crate::token_kinds::TokenKind;

impl<'gc, 'ast, 'ctx, 'a> JSParserImpl<'gc, 'ast, 'ctx, 'a> {
    // -----------------------------------------------------------------------
    // parseTypeAnnotationTS — 21 in JSParserImpl-ts.cpp
    // -----------------------------------------------------------------------

    /// Parse a TypeScript type annotation.
    /// Port of `JSParserImpl::parseTypeAnnotationTS` (ts.cpp:21-134).
    ///
    /// \param wrapped_start if `Some`, the result is wrapped in a
    ///   `TSTypeAnnotation` node spanning from it to the previous token's end
    ///   (the C++ `wrappedStart` parameter, used for `: T` annotations).
    ///
    /// P7.0 implements only the `parse_ts_union_type` path plus the
    /// `wrappedStart` wrap. The predicate (`check(isIdent_)`), constructor
    /// type (`rw_new`), generic function type (`<`) and conditional type
    /// (`extends`) branches arrive in later P7 tasks.
    pub(in crate::js) fn parse_type_annotation_ts(
        &mut self,
        wrapped_start: Option<SMLoc>,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 23: llvh::SaveAndRestore<bool> on allowAnonFunctionType_, set to
        // true for the body. The guard restores the old value on every exit
        // path, including the `?` early returns below.
        let _guard = self.save_allow_anon_function_type(true);

        // C++ 25: `start` captured before anything is consumed. Used by the
        // predicate / `rw_new` / `<` / conditional branches (deferred to later
        // P7 tasks), hence the leading underscore until they land.
        let _start = self.cur_start();

        // C++ 28-90: the predicate / `rw_new` / `<` / union dispatch. P7.0
        // wires only the union path (C++ 84-89); the other arms are added by
        // later P7 tasks.
        let result = self.parse_ts_union_type()?;

        // C++ 127-133.
        if let Some(wrapped_start) = wrapped_start {
            let node = Node::TSTypeAnnotation(TSTypeAnnotation::new(
                NodeMetadata::new(self.dummy_range()),
                result,
            ));
            return Some(self.set_location(
                wrapped_start,
                self.lexer.prev_token_end(),
                node,
            ));
        }
        Some(result)
    }

    // -----------------------------------------------------------------------
    // parseTSUnionType — 136 in JSParserImpl-ts.cpp
    // -----------------------------------------------------------------------

    /// Parse a TS union type (`T | U | ...`).
    /// Port of `JSParserImpl::parseTSUnionType` (ts.cpp:136-163).
    pub(super) fn parse_ts_union_type(&mut self) -> Option<&'gc Node<'gc>> {
        // C++ 137-138: a leading `|` is allowed and ignored.
        let start = self.cur_start();
        self.check_and_eat(TokenKind::pipe, GrammarContext::Type);

        let first = self.parse_ts_intersection_type()?;

        // C++ 144-147: done with the union, move on.
        if !self.check(TokenKind::pipe) {
            return Some(first);
        }

        // C++ 149-157.
        let mut types: Vec<&'gc Node<'gc>> = vec![first];
        while self.check_and_eat(TokenKind::pipe, GrammarContext::Type) {
            types.push(self.parse_ts_intersection_type()?);
        }

        // C++ 159-162.
        let node = Node::TSUnionType(TSUnionType::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, types),
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseTSIntersectionType — 165 in JSParserImpl-ts.cpp
    // -----------------------------------------------------------------------

    /// Parse a TS intersection type (`T & U & ...`).
    /// Port of `JSParserImpl::parseTSIntersectionType` (ts.cpp:165-192).
    pub(super) fn parse_ts_intersection_type(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 166-167: a leading `&` is allowed and ignored.
        let start = self.cur_start();
        self.check_and_eat(TokenKind::amp, GrammarContext::Type);

        let first = self.parse_ts_postfix_type()?;

        // C++ 173-176: done with the intersection, move on.
        if !self.check(TokenKind::amp) {
            return Some(first);
        }

        // C++ 178-186.
        let mut types: Vec<&'gc Node<'gc>> = vec![first];
        while self.check_and_eat(TokenKind::amp, GrammarContext::Type) {
            types.push(self.parse_ts_postfix_type()?);
        }

        // C++ 188-191.
        let node = Node::TSIntersectionType(TSIntersectionType::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, types),
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseTSPostfixType — 866 in JSParserImpl-ts.cpp
    // -----------------------------------------------------------------------

    /// Parse a TS postfix type — a primary type followed by any number of `[]`
    /// (array) or `[T]` (indexed-access) suffixes.
    /// Port of `JSParserImpl::parseTSPostfixType` (ts.cpp:866-901).
    pub(super) fn parse_ts_postfix_type(&mut self) -> Option<&'gc Node<'gc>> {
        let start = self.cur_start();
        let mut result = self.parse_ts_primary_type()?;

        // C++ 874-898: parse any `[]`/`[T]` after the primary type.
        while !self.lexer.is_new_line_before_current_token()
            && self.check_and_eat(TokenKind::l_square, GrammarContext::Type)
        {
            if self.check(TokenKind::r_square) {
                // C++ 877-881: array type.
                let node = Node::TSArrayType(TSArrayType::new(
                    NodeMetadata::new(self.dummy_range()),
                    result,
                ));
                let end = self.advance(GrammarContext::Type).end;
                result = self.set_location(start, end, node);
            } else {
                // C++ 882-897: indexed-access type.
                let index_type = self.parse_type_annotation_ts(None)?;
                if !self.eat(
                    TokenKind::r_square,
                    GrammarContext::Type,
                    " in indexed access type",
                ) {
                    return None;
                }
                let node = Node::TSIndexedAccessType(TSIndexedAccessType::new(
                    NodeMetadata::new(self.dummy_range()),
                    result,
                    index_type,
                ));
                result =
                    self.set_location(start, self.lexer.prev_token_end(), node);
            }
        }

        Some(result)
    }

    // -----------------------------------------------------------------------
    // parseTSPrimaryType — 903 in JSParserImpl-ts.cpp
    // -----------------------------------------------------------------------

    /// Parse a TS primary type.
    /// Port of `JSParserImpl::parseTSPrimaryType` (ts.cpp:903-1058).
    ///
    /// P7.0 stub: only the `string`/`number` keyword idents are handled; every
    /// other token is an honest parse error pending later P7 tasks. P7.1 adds
    /// the remaining keyword/literal/reference/parenthesized/object/etc. arms.
    pub(super) fn parse_ts_primary_type(&mut self) -> Option<&'gc Node<'gc>> {
        // C++ 904: CHECK_RECURSION.
        let _recursion = self.check_recursion()?;
        let start = self.cur_start();

        // C++ 928-990: the `rw_static`/identifier arm, matched on the
        // res-word-or-identifier name. P7.0 handles only the `identifier`
        // subset for `string`/`number`; P7.1 adds the `rw_static` half and the
        // remaining keyword names.
        if self.check(TokenKind::identifier) {
            let name = self
                .lexer
                .get_string_table()
                .bytes(self.lexer.token().get_res_word_or_identifier());
            match name {
                // C++ 942-947.
                b"number" => {
                    let node = Node::TSNumberKeyword(TSNumberKeyword::new(
                        NodeMetadata::new(self.dummy_range()),
                    ));
                    let end = self.advance(GrammarContext::Type).end;
                    return Some(self.set_location(start, end, node));
                }
                // C++ 954-959.
                b"string" => {
                    let node = Node::TSStringKeyword(TSStringKeyword::new(
                        NodeMetadata::new(self.dummy_range()),
                    ));
                    let end = self.advance(GrammarContext::Type).end;
                    return Some(self.set_location(start, end, node));
                }
                _ => {}
            }
        }

        // C++ 1055: honest deferral — every other primary-type token is not
        // yet supported. Later P7 tasks replace this with the full switch.
        self.error_cur("unexpected token in type annotation");
        None
    }
}
