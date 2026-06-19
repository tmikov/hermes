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
//! P7.0 implemented the precedence skeleton `parse_type_annotation_ts` →
//! `parse_ts_union_type` → `parse_ts_intersection_type` →
//! `parse_ts_postfix_type` → `parse_ts_primary_type` with only the
//! `string`/`number` keyword arms in the primary type. P7.1 fills in the full
//! type-grammar core: the predicate/constructor/generic-function dispatch and
//! the trailing conditional type in `parse_type_annotation_ts`; the complete
//! primary-type switch (all keyword names, literals, `this`, `*`, tuples,
//! `typeof`, and references); type references and qualified names; type
//! queries; tuple types; and the `reparse_identifier_as_ts_type_annotation`
//! helper. The parenthesized/function (`(`, `new`, `<`) → P7.2, object (`{`) →
//! P7.3, and `interface` → P7.4 arms remain honest deferrals.

use ast::node::{
    BigIntLiteral, BooleanLiteral, ExistsTypeAnnotation, Identifier, Node,
    NullLiteral, NumericLiteral, StringLiteral, TSAnyKeyword, TSArrayType,
    TSBigIntKeyword, TSBooleanKeyword, TSConditionalType, TSIndexedAccessType,
    TSIntersectionType, TSLiteralType, TSNeverKeyword, TSNumberKeyword,
    TSQualifiedName, TSStringKeyword, TSSymbolKeyword, TSThisType, TSTupleType,
    TSTypeAnnotation, TSTypePredicate, TSTypeQuery, TSTypeReference,
    TSUndefinedKeyword, TSUnionType, TSUnknownKeyword, TSVoidKeyword,
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
    pub(in crate::js) fn parse_type_annotation_ts(
        &mut self,
        wrapped_start: Option<SMLoc>,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 23: llvh::SaveAndRestore<bool> on allowAnonFunctionType_, set to
        // true for the body. The guard restores the old value on every exit
        // path, including the `?` early returns below.
        let _guard = self.save_allow_anon_function_type(true);

        // C++ 25.
        let start = self.cur_start();
        let mut result: Option<&'gc Node<'gc>> = None;

        // C++ 28-50: an identifier may be the parameter name of a type
        // predicate (`id is T`); this requires backtracking via a SavePoint.
        if self.check(TokenKind::identifier) {
            let save_point = self.lexer.save_point();
            let id_range = self.cur_range();
            let id_node = Node::Identifier(Identifier::new(
                NodeMetadata::new(self.dummy_range()),
                self.lexer.token().get_identifier(),
                None,
                false,
            ));
            let id = self.set_location(id_range.start, id_range.end, id_node);
            self.advance(GrammarContext::Type);
            // C++ 37: check(isIdent_) — escape-insensitive contextual `is`.
            if self.check_name(b"is") {
                // C++ 38-46.
                let wrapped_start = self.advance(GrammarContext::Type).start;
                let _recursion = self.check_recursion()?;
                let opt_type = self.parse_type_annotation_ts(Some(wrapped_start))?;
                let node = Node::TSTypePredicate(TSTypePredicate::new(
                    NodeMetadata::new(self.dummy_range()),
                    id,
                    opt_type,
                ));
                result = Some(self.set_location(
                    start,
                    self.lexer.prev_token_end(),
                    node,
                ));
            } else {
                // C++ 48: not a predicate, restore and fall through.
                save_point.restore(&mut self.lexer);
            }
        }

        // C++ 52-90.
        let mut result = if let Some(result) = result {
            result
        } else if self.check(TokenKind::rw_new) {
            // C++ 53-72: constructor type `new <T>(...) => U`.
            self.advance(GrammarContext::Type);
            if self.check(TokenKind::less) {
                let _type_params = self.parse_ts_type_parameters()?;
            }
            // C++ 62-71: parseTSFunctionOrParenthesizedType — P7.2.
            self.error_cur(
                "TypeScript constructor types are not yet supported",
            );
            return None;
        } else if self.check(TokenKind::less) {
            // C++ 73-83: generic function type `<T>(...) => U`.
            let _type_params = self.parse_ts_type_parameters()?;
            // C++ 77-82: parseTSFunctionOrParenthesizedType — P7.2.
            self.error_cur("TypeScript function types are not yet supported");
            return None;
        } else {
            // C++ 84-89.
            self.parse_ts_union_type()?
        };

        // C++ 92-125: a trailing `extends T ? X : Y` makes a conditional type.
        if self.check_and_eat(TokenKind::rw_extends, GrammarContext::Type) {
            // C++ 94-96.
            let opt_check = self.parse_type_annotation_ts(None)?;
            // C++ 97-103.
            if !self.eat(
                TokenKind::question,
                GrammarContext::Type,
                " in conditional type",
            ) {
                return None;
            }

            // C++ 105-114.
            let opt_true = self.parse_type_annotation_ts(None)?;
            if !self.eat(
                TokenKind::colon,
                GrammarContext::Type,
                " in conditional type",
            ) {
                return None;
            }

            // C++ 116-118.
            let opt_false = self.parse_type_annotation_ts(None)?;

            // C++ 120-124.
            let node = Node::TSConditionalType(TSConditionalType::new(
                NodeMetadata::new(self.dummy_range()),
                result,
                opt_check,
                opt_true,
                opt_false,
            ));
            result = self.set_location(
                result.metadata().range.get().start,
                self.lexer.prev_token_end(),
                node,
            );
        }

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
    // parseTSTupleType — 194 in JSParserImpl-ts.cpp
    // -----------------------------------------------------------------------

    /// Parse a TS tuple type (`[A, B]`).
    /// Port of `JSParserImpl::parseTSTupleType` (ts.cpp:194-221).
    pub(super) fn parse_ts_tuple_type(&mut self) -> Option<&'gc Node<'gc>> {
        // C++ 195-196.
        debug_assert!(self.check(TokenKind::l_square));
        let start = self.advance(GrammarContext::Type).start;

        let mut types: Vec<&'gc Node<'gc>> = Vec::new();

        // C++ 200-208.
        while !self.check(TokenKind::r_square) {
            types.push(self.parse_type_annotation_ts(None)?);

            if !self.check_and_eat(TokenKind::comma, GrammarContext::Type) {
                break;
            }
        }

        // C++ 210-215.
        if !self.need(TokenKind::r_square, " at end of tuple type annotation") {
            return None;
        }

        // C++ 217-220.
        let end = self.advance(GrammarContext::Type).end;
        let node = Node::TSTupleType(TSTupleType::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, types),
        ));
        Some(self.set_location(start, end, node))
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
    pub(super) fn parse_ts_primary_type(&mut self) -> Option<&'gc Node<'gc>> {
        // C++ 904: CHECK_RECURSION.
        let _recursion = self.check_recursion()?;
        let start = self.cur_start();

        // C++ 906: switch on the current token kind.
        match self.cur_kind() {
            // C++ 907-911.
            TokenKind::star => {
                let node = Node::ExistsTypeAnnotation(ExistsTypeAnnotation::new(
                    NodeMetadata::new(self.dummy_range()),
                ));
                let end = self.advance(GrammarContext::Type).end;
                Some(self.set_location(start, end, node))
            }

            // C++ 912-914: parseTSFunctionOrParenthesizedType — P7.2.
            TokenKind::l_paren => {
                self.error_cur(
                    "TypeScript parenthesized types are not yet supported",
                );
                None
            }

            // C++ 915-916: parseTSObjectType — P7.3.
            TokenKind::l_brace => {
                self.error_cur(
                    "TypeScript object types are not yet supported",
                );
                None
            }

            // C++ 917-918: parseTSInterfaceDeclaration — P7.4.
            TokenKind::rw_interface => {
                self.error_cur(
                    "TypeScript interface declarations are not yet supported",
                );
                None
            }

            // C++ 919-920.
            TokenKind::rw_typeof => self.parse_ts_type_query(),

            // C++ 921-922.
            TokenKind::l_square => self.parse_ts_tuple_type(),

            // C++ 923-927.
            TokenKind::rw_this => {
                let node = Node::TSThisType(TSThisType::new(
                    NodeMetadata::new(self.dummy_range()),
                ));
                let end = self.advance(GrammarContext::Type).end;
                Some(self.set_location(start, end, node))
            }

            // C++ 928-990: the `rw_static`/identifier arm, matched on the
            // res-word-or-identifier name (escape-insensitive in C++; we
            // compare the interned name bytes directly). Each named-primitive
            // arm is `setLocation(start, advance(Type).End, new <Name>Node())`;
            // unmatched names fall through to a type reference.
            TokenKind::rw_static | TokenKind::identifier => {
                let md = NodeMetadata::new(self.dummy_range());
                let prim: Option<Node<'gc>> = match self
                    .lexer
                    .get_string_table()
                    .bytes(self.lexer.token().get_res_word_or_identifier())
                {
                    // C++ 930-935.
                    b"any" => Some(Node::TSAnyKeyword(TSAnyKeyword::new(md))),
                    // C++ 936-941.
                    b"boolean" => {
                        Some(Node::TSBooleanKeyword(TSBooleanKeyword::new(md)))
                    }
                    // C++ 942-947.
                    b"number" => {
                        Some(Node::TSNumberKeyword(TSNumberKeyword::new(md)))
                    }
                    // C++ 948-953.
                    b"symbol" => {
                        Some(Node::TSSymbolKeyword(TSSymbolKeyword::new(md)))
                    }
                    // C++ 954-959.
                    b"string" => {
                        Some(Node::TSStringKeyword(TSStringKeyword::new(md)))
                    }
                    // C++ 960-965.
                    b"bigint" => {
                        Some(Node::TSBigIntKeyword(TSBigIntKeyword::new(md)))
                    }
                    // C++ 966-971.
                    b"never" => {
                        Some(Node::TSNeverKeyword(TSNeverKeyword::new(md)))
                    }
                    // C++ 972-977.
                    b"undefined" => Some(Node::TSUndefinedKeyword(
                        TSUndefinedKeyword::new(md),
                    )),
                    // C++ 978-983.
                    b"unknown" => {
                        Some(Node::TSUnknownKeyword(TSUnknownKeyword::new(md)))
                    }
                    _ => None,
                };
                if let Some(prim) = prim {
                    let end = self.advance(GrammarContext::Type).end;
                    return Some(self.set_location(start, end, prim));
                }
                // C++ 985-990.
                self.parse_ts_type_reference()
            }

            // C++ 992-999: `null` → TSLiteralType wrapping a NullLiteral.
            TokenKind::rw_null => {
                let end = self.advance(GrammarContext::Type).end;
                let literal_node = Node::NullLiteral(NullLiteral::new(
                    NodeMetadata::new(self.dummy_range()),
                ));
                let literal = self.set_location(start, end, literal_node);
                let node = Node::TSLiteralType(TSLiteralType::new(
                    NodeMetadata::new(self.dummy_range()),
                    literal,
                ));
                Some(self.set_location(start, end, node))
            }

            // C++ 1001-1005.
            TokenKind::rw_void => {
                let node = Node::TSVoidKeyword(TSVoidKeyword::new(
                    NodeMetadata::new(self.dummy_range()),
                ));
                let end = self.advance(GrammarContext::Type).end;
                Some(self.set_location(start, end, node))
            }

            // C++ 1007-1015: string literal type.
            TokenKind::string_literal => {
                let str = self.lexer.token().get_string_literal();
                let end = self.advance(GrammarContext::Type).end;
                let literal_node = Node::StringLiteral(StringLiteral::new(
                    NodeMetadata::new(self.dummy_range()),
                    str,
                ));
                let literal = self.set_location(start, end, literal_node);
                let node = Node::TSLiteralType(TSLiteralType::new(
                    NodeMetadata::new(self.dummy_range()),
                    literal,
                ));
                Some(self.set_location(start, end, node))
            }

            // C++ 1017-1025: numeric literal type.
            TokenKind::numeric_literal => {
                let value = self.lexer.token().get_numeric_literal();
                let end = self.advance(GrammarContext::Type).end;
                let literal_node = Node::NumericLiteral(NumericLiteral::new(
                    NodeMetadata::new(self.dummy_range()),
                    value,
                ));
                let literal = self.set_location(start, end, literal_node);
                let node = Node::TSLiteralType(TSLiteralType::new(
                    NodeMetadata::new(self.dummy_range()),
                    literal,
                ));
                Some(self.set_location(start, end, node))
            }

            // C++ 1027-1035: bigint literal type.
            TokenKind::bigint_literal => {
                let raw = self.lexer.token().get_bigint_literal();
                let end = self.advance(GrammarContext::Type).end;
                let literal_node = Node::BigIntLiteral(BigIntLiteral::new(
                    NodeMetadata::new(self.dummy_range()),
                    raw,
                ));
                let literal = self.set_location(start, end, literal_node);
                let node = Node::TSLiteralType(TSLiteralType::new(
                    NodeMetadata::new(self.dummy_range()),
                    literal,
                ));
                Some(self.set_location(start, end, node))
            }

            // C++ 1037-1046: `true`/`false` literal type.
            TokenKind::rw_true | TokenKind::rw_false => {
                let value = self.check(TokenKind::rw_true);
                let end = self.advance(GrammarContext::Type).end;
                let literal_node = Node::BooleanLiteral(BooleanLiteral::new(
                    NodeMetadata::new(self.dummy_range()),
                    value,
                ));
                let literal = self.set_location(start, end, literal_node);
                let node = Node::TSLiteralType(TSLiteralType::new(
                    NodeMetadata::new(self.dummy_range()),
                    literal,
                ));
                Some(self.set_location(start, end, node))
            }

            // C++ 1048-1056: default — a reserved word can still start a type
            // reference (e.g. `import`/`extends`-adjacent names).
            _ => {
                if self.lexer.token().is_res_word() {
                    return self.parse_ts_type_reference();
                }
                self.error_cur("unexpected token in type annotation");
                None
            }
        }
    }

    // -----------------------------------------------------------------------
    // parseTSTypeReference — 1060 in JSParserImpl-ts.cpp
    // -----------------------------------------------------------------------

    /// Parse a TS type reference (`A.B.C<X, Y>`).
    /// Port of `JSParserImpl::parseTSTypeReference` (ts.cpp:1060-1081).
    pub(super) fn parse_ts_type_reference(&mut self) -> Option<&'gc Node<'gc>> {
        // C++ 1061-1062.
        debug_assert!(
            self.check(TokenKind::identifier)
                || self.lexer.token().is_res_word()
        );
        let start = self.cur_start();

        // C++ 1064-1067.
        let type_name = self.parse_ts_qualified_name()?;

        // C++ 1069-1075.
        let mut type_params: Option<&'gc Node<'gc>> = None;
        if self.check(TokenKind::less) {
            type_params = Some(self.parse_ts_type_arguments()?);
        }

        // C++ 1077-1080.
        let node = Node::TSTypeReference(TSTypeReference::new(
            NodeMetadata::new(self.dummy_range()),
            type_name,
            type_params,
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseTSQualifiedName — 1083 in JSParserImpl-ts.cpp
    // -----------------------------------------------------------------------

    /// Parse a (possibly dotted) TS qualified name (`A.B.C`).
    /// Port of `JSParserImpl::parseTSQualifiedName` (ts.cpp:1083-1114).
    fn parse_ts_qualified_name(&mut self) -> Option<&'gc Node<'gc>> {
        // C++ 1084-1090.
        let id_range = self.cur_range();
        let id_node = Node::Identifier(Identifier::new(
            NodeMetadata::new(self.dummy_range()),
            self.lexer.token().get_res_word_or_identifier(),
            None,
            false,
        ));
        let mut type_name =
            self.set_location(id_range.start, id_range.end, id_node);
        self.advance(GrammarContext::Type);

        // C++ 1092-1111.
        while self.check_and_eat(TokenKind::period, GrammarContext::Type) {
            // C++ 1093-1100.
            if !self.check(TokenKind::identifier)
                && !self.lexer.token().is_res_word()
            {
                self.need(TokenKind::identifier, " in qualified type name");
                return None;
            }
            // C++ 1101-1106.
            let right_range = self.cur_range();
            let right_node = Node::Identifier(Identifier::new(
                NodeMetadata::new(self.dummy_range()),
                self.lexer.token().get_res_word_or_identifier(),
                None,
                false,
            ));
            let right =
                self.set_location(right_range.start, right_range.end, right_node);
            self.advance(GrammarContext::Type);
            // C++ 1107-1110.
            let node = Node::TSQualifiedName(TSQualifiedName::new(
                NodeMetadata::new(self.dummy_range()),
                type_name,
                Some(right),
            ));
            type_name = self.set_location(
                type_name.metadata().range.get().start,
                self.lexer.prev_token_end(),
                node,
            );
        }

        Some(type_name)
    }

    // -----------------------------------------------------------------------
    // parseTSTypeQuery — 1116 in JSParserImpl-ts.cpp
    // -----------------------------------------------------------------------

    /// Parse a TS type query (`typeof A.B`).
    /// Port of `JSParserImpl::parseTSTypeQuery` (ts.cpp:1116-1158).
    fn parse_ts_type_query(&mut self) -> Option<&'gc Node<'gc>> {
        // C++ 1117-1118.
        debug_assert!(self.check(TokenKind::rw_typeof));
        let start = self.advance(GrammarContext::Type).start;

        // C++ 1120-1124.
        if !(self.lexer.token().is_res_word()
            || self.check(TokenKind::identifier))
        {
            self.need(TokenKind::identifier, " in type query");
            return None;
        }

        // C++ 1126-1131.
        let id_range = self.cur_range();
        let id_node = Node::Identifier(Identifier::new(
            NodeMetadata::new(self.dummy_range()),
            self.lexer.token().get_res_word_or_identifier(),
            None,
            false,
        ));
        let mut type_name =
            self.set_location(id_range.start, id_range.end, id_node);
        self.advance(GrammarContext::Type);

        // C++ 1133-1152.
        while self.check_and_eat(TokenKind::period, GrammarContext::Type) {
            // C++ 1134-1141.
            if !self.check(TokenKind::identifier)
                && !self.lexer.token().is_res_word()
            {
                self.need(TokenKind::identifier, " in qualified type name");
                return None;
            }
            // C++ 1142-1147.
            let right_range = self.cur_range();
            let right_node = Node::Identifier(Identifier::new(
                NodeMetadata::new(self.dummy_range()),
                self.lexer.token().get_res_word_or_identifier(),
                None,
                false,
            ));
            let right =
                self.set_location(right_range.start, right_range.end, right_node);
            self.advance(GrammarContext::Type);
            // C++ 1148-1151.
            let node = Node::TSQualifiedName(TSQualifiedName::new(
                NodeMetadata::new(self.dummy_range()),
                type_name,
                Some(right),
            ));
            type_name = self.set_location(
                type_name.metadata().range.get().start,
                self.lexer.prev_token_end(),
                node,
            );
        }

        // C++ 1154-1157.
        let node = Node::TSTypeQuery(TSTypeQuery::new(
            NodeMetadata::new(self.dummy_range()),
            type_name,
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // reparseIdentifierAsTSTypeAnnotation — 1406 in JSParserImpl-ts.cpp
    // -----------------------------------------------------------------------

    /// Reinterpret an already-parsed `Identifier` as a TS primary type: a
    /// matching primitive-keyword node, else a `TSTypeReference` to the
    /// identifier. Port of
    /// `JSParserImpl::reparseIdentifierAsTSTypeAnnotation` (ts.cpp:1406-1431).
    /// Used by `parseTSFunctionOrParenthesizedType` (P7.2).
    #[allow(dead_code)] // Consumed by parseTSFunctionOrParenthesizedType (P7.2).
    pub(super) fn reparse_identifier_as_ts_type_annotation(
        &self,
        ident: &'gc Node<'gc>,
    ) -> &'gc Node<'gc> {
        let Node::Identifier(id) = ident else {
            unreachable!("expected IdentifierNode");
        };
        let range = ident.metadata().range.get();
        let md = NodeMetadata::new(self.dummy_range());
        // C++ 1408-1427: map the known primitive names; the C++ compares the
        // interned `_name` atom, we compare its bytes.
        let prim: Option<Node<'gc>> =
            match self.lexer.get_string_table().bytes(id.name.get()) {
                b"any" => Some(Node::TSAnyKeyword(TSAnyKeyword::new(md))),
                b"boolean" => {
                    Some(Node::TSBooleanKeyword(TSBooleanKeyword::new(md)))
                }
                b"number" => {
                    Some(Node::TSNumberKeyword(TSNumberKeyword::new(md)))
                }
                b"symbol" => {
                    Some(Node::TSSymbolKeyword(TSSymbolKeyword::new(md)))
                }
                b"string" => {
                    Some(Node::TSStringKeyword(TSStringKeyword::new(md)))
                }
                _ => None,
            };
        if let Some(prim) = prim {
            return self.set_location(range.start, range.end, prim);
        }
        // C++ 1429-1430.
        let node = Node::TSTypeReference(TSTypeReference::new(
            NodeMetadata::new(self.dummy_range()),
            ident,
            None,
        ));
        self.set_location(range.start, range.end, node)
    }
}
