/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Flow type-parameter declarations (`<T: B = D>`), type arguments
//! (`<T, U>`), and (possibly qualified) generic type references. Port of the
//! corresponding sections of `lib/Parser/JSParserImpl-flow.cpp`.

use hermes_ast::node::{
    ClassImplements, GenericTypeAnnotation, Identifier, Node,
    QualifiedTypeIdentifier, TypeAnnotation, TypeParameter,
    TypeParameterDeclaration, TypeParameterInstantiation, Variance,
};
use hermes_ast::node_child::{NodeLabel, NodeList, NodeMetadata};

use crate::js::JSParserImpl;
use crate::lexer::GrammarContext;
use crate::token_kinds::TokenKind;

use super::AllowAnonFunctionType;

impl<'gc, 'ast, 'ctx, 'a> JSParserImpl<'gc, 'ast, 'ctx, 'a> {
    // -----------------------------------------------------------------------
    // parseTypeParamsFlow — 4690 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a type-parameter declaration `<T, U: B, ...>`, with the current
    /// token at `<`. At least one parameter is required (empty `<>` is an
    /// error); a trailing comma is allowed. Port of `parseTypeParamsFlow`
    /// (flow.cpp:4691-4720).
    pub(in crate::js) fn parse_type_params_flow(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        debug_assert!(self.check(TokenKind::less));
        // C++ 4692.
        let start = self.advance(GrammarContext::Type).start;

        let mut params: Vec<&'gc Node<'gc>> = Vec::new();

        // C++ 4696-4704: a do-while — at least one parameter is required.
        loop {
            params.push(self.parse_type_param_flow()?);

            if !self.check_and_eat(TokenKind::comma, GrammarContext::Type) {
                break;
            }
            if self.check(TokenKind::greater) {
                break;
            }
        }

        // C++ 4706-4713.
        let end = self.cur_range().end;
        if !self.eat_at(
            TokenKind::greater,
            GrammarContext::Type,
            " at end of type parameters",
            Some("start of type parameters"),
            start,
        ) {
            return None;
        }

        // C++ 4715-4718.
        let node = Node::TypeParameterDeclaration(
            TypeParameterDeclaration::new(
                NodeMetadata::new(self.dummy_range()),
                NodeList::from_iter(self.gc, params),
            ),
        );
        Some(self.set_location(start, end, node))
    }

    /// Parse a single type parameter `[const] [variance] name [: B|extends B]
    /// [= D]`. Port of `parseTypeParamFlow` (flow.cpp:4722-4815).
    fn parse_type_param_flow(&mut self) -> Option<&'gc Node<'gc>> {
        let start = self.cur_start();
        // C++ 4723-4728.
        let mut is_const = false;
        let mut variance: Option<&'gc Node<'gc>> = None;
        if self.check(TokenKind::rw_const) {
            is_const = true;
            self.advance(GrammarContext::Type);
        }

        // `in` and `out` are both ambiguous: variance modifier (`<in T>`,
        // `<out T>`) vs name (`<in>`, `<out>`, `<in: T>`, `<in extends Foo>`).
        // Defer the decision: consume the keyword here, and below — once we
        // know the *actual* next token — either promote it to variance or
        // treat it as the name itself. (C++ 4730-4749.)
        let mut variance_keyword_range = self.dummy_range();
        let mut variance_keyword_kind: Option<NodeLabel> = None;

        if self.check2(TokenKind::plus, TokenKind::minus) {
            let kind: &[u8] = if self.check(TokenKind::plus) {
                b"plus"
            } else {
                b"minus"
            };
            let v_range = self.cur_range();
            let v_node = Node::Variance(Variance::new(
                NodeMetadata::new(self.dummy_range()),
                self.lexer.get_identifier(kind),
            ));
            variance =
                Some(self.set_location(v_range.start, v_range.end, v_node));
            self.advance(GrammarContext::Type);
        } else if self.check(TokenKind::rw_in) || self.check_name(b"out") {
            variance_keyword_kind =
                Some(self.lexer.token().get_res_word_or_identifier());
            variance_keyword_range = self.cur_range();
            self.advance(GrammarContext::Type);
        }

        // Type-param name: identifier or `in` (rw_in). `in` is accepted
        // because Flow reclassifies it to an identifier in TYPE lex mode
        // (matching `<in>`, `<in: T>`, `<in extends T>`, `<X, in, Y>`). `out`
        // is already a plain identifier in Hermes, so `<out>` etc. work
        // without special handling. (C++ 4751-4776.)
        let name: NodeLabel;
        if self.check(TokenKind::identifier) || self.check(TokenKind::rw_in) {
            if let Some(kind) = variance_keyword_kind {
                // The deferred `in` was variance, and the current token is
                // the name.
                let v_node = Node::Variance(Variance::new(
                    NodeMetadata::new(self.dummy_range()),
                    kind,
                ));
                variance = Some(self.set_location(
                    variance_keyword_range.start,
                    variance_keyword_range.end,
                    v_node,
                ));
            }
            name = self.lexer.token().get_res_word_or_identifier();
            self.advance(GrammarContext::Type);
        } else if let Some(kind) = variance_keyword_kind {
            // The deferred `in`/`out` was the type-param name itself, not
            // variance. Reached when the next token is `>`, `,`, `:`, `=`,
            // or `rw_extends` (none of which are name tokens). E.g. `<in>`,
            // `<out: T>`, `<in extends T>`, `<out = T>`, `<X, in, Y>`.
            name = kind;
        } else {
            // flow.cpp:4775: errorExpected(identifier, "in type parameter",
            // nullptr, {}) — VERIFIED whatLoc-less in C++ (unlike its
            // sibling below), so the plain `need` (no location) is correct
            // as-is.
            self.need(TokenKind::identifier, " in type parameter");
            return None;
        }

        // C++ 4778-4799.
        let mut bound: Option<&'gc Node<'gc>> = None;
        let mut uses_extends_bound = false;
        if self.check(TokenKind::colon) {
            let bound_start = self.advance(GrammarContext::Type).start;
            let bound_type = self
                .parse_type_annotation_flow(None, AllowAnonFunctionType::Yes)?;
            let bound_node = Node::TypeAnnotation(TypeAnnotation::new(
                NodeMetadata::new(self.dummy_range()),
                bound_type,
            ));
            bound = Some(self.set_location(
                bound_start,
                self.lexer.prev_token_end(),
                bound_node,
            ));
        } else if self.check(TokenKind::rw_extends) {
            uses_extends_bound = true;
            let bound_start = self.advance(GrammarContext::Type).start;
            let bound_type = self
                .parse_type_annotation_flow(None, AllowAnonFunctionType::Yes)?;
            let bound_node = Node::TypeAnnotation(TypeAnnotation::new(
                NodeMetadata::new(self.dummy_range()),
                bound_type,
            ));
            bound = Some(self.set_location(
                bound_start,
                self.lexer.prev_token_end(),
                bound_node,
            ));
        }

        // C++ 4801-4807.
        let mut initializer: Option<&'gc Node<'gc>> = None;
        if self.check_and_eat(TokenKind::equal, GrammarContext::Type) {
            initializer = Some(self.parse_type_annotation_flow(
                None,
                AllowAnonFunctionType::Yes,
            )?);
        }

        // C++ 4809-4813.
        let node = Node::TypeParameter(TypeParameter::new(
            NodeMetadata::new(self.dummy_range()),
            name,
            is_const,
            bound,
            variance,
            initializer,
            uses_extends_bound,
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseTypeArgsFlow — 4816 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse type arguments `<T, U>`, with the current token at `<`.
    /// \param trailing_grammar_context the grammar context with which the
    ///   closing `>` is consumed (the C++ parameter defaults to Type, per
    ///   JSParserImpl.h:1506-1508; Rust callers pass it explicitly).
    /// Port of `parseTypeArgsFlow` (flow.cpp:4817-4847).
    pub(in crate::js) fn parse_type_args_flow(
        &mut self,
        trailing_grammar_context: GrammarContext,
    ) -> Option<&'gc Node<'gc>> {
        debug_assert!(self.check(TokenKind::less));
        // C++ 4819.
        let start = self.advance(GrammarContext::Type).start;

        let mut params: Vec<&'gc Node<'gc>> = Vec::new();

        // C++ 4823-4831: a while-loop (not do-while) — empty `<>` IS allowed
        // for type *arguments* (unlike type-parameter declarations).
        while !self.check(TokenKind::greater) {
            params.push(self.parse_type_annotation_flow(
                None,
                AllowAnonFunctionType::Yes,
            )?);

            if !self.check_and_eat(TokenKind::comma, GrammarContext::Type) {
                break;
            }
        }

        // C++ 4833-4840: `end` is the `>` token's end, captured before
        // consuming it with the caller's trailing grammar context.
        let end = self.cur_range().end;
        if !self.eat_at(
            TokenKind::greater,
            trailing_grammar_context,
            " at end of type parameters",
            Some("start of type parameters"),
            start,
        ) {
            return None;
        }

        // C++ 4842-4845.
        let node = Node::TypeParameterInstantiation(
            TypeParameterInstantiation::new(
                NodeMetadata::new(self.dummy_range()),
                NodeList::from_iter(self.gc, params),
            ),
        );
        Some(self.set_location(start, end, node))
    }

    // -----------------------------------------------------------------------
    // parseGenericTypeFlow — 5007 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a (possibly qualified) generic type reference
    /// `Foo.Bar<Args>`, with the current token at the first identifier or
    /// reserved word. Port of `parseGenericTypeFlow` (flow.cpp:5008-5051).
    pub(super) fn parse_generic_type_flow(&mut self) -> Option<&'gc Node<'gc>> {
        debug_assert!(
            self.check(TokenKind::identifier)
                || self.lexer.token().is_res_word()
        );
        let start = self.cur_start();

        // C++ 5012-5017.
        let id_range = self.cur_range();
        let id_node = Node::Identifier(Identifier::new(
            NodeMetadata::new(self.dummy_range()),
            self.lexer.token().get_res_word_or_identifier(),
            None,
            false,
        ));
        let mut id = self.set_location(id_range.start, id_range.end, id_node);
        self.advance(GrammarContext::Type);

        // C++ 5019: GrammarContext::Type here (unlike the qualified-typeof
        // chain, which uses the default).
        while self.check_and_eat(TokenKind::period, GrammarContext::Type) {
            // C++ 5020-5027.
            if !self.check(TokenKind::identifier)
                && !self.lexer.token().is_res_word()
            {
                // flow.cpp:5021-5028: errorExpected(identifier, "in
                // qualified generic type name", "start of type name",
                // start).
                self.need_at(
                    TokenKind::identifier,
                    " in qualified generic type name",
                    Some("start of type name"),
                    start,
                );
                return None;
            }
            // C++ 5028-5033.
            let next_range = self.cur_range();
            let next_node = Node::Identifier(Identifier::new(
                NodeMetadata::new(self.dummy_range()),
                self.lexer.token().get_res_word_or_identifier(),
                None,
                false,
            ));
            let next = self.set_location(
                next_range.start,
                next_range.end,
                next_node,
            );
            self.advance(GrammarContext::Type);
            // C++ 5034-5035.
            let q_node = Node::QualifiedTypeIdentifier(
                QualifiedTypeIdentifier::new(
                    NodeMetadata::new(self.dummy_range()),
                    id,
                    next,
                ),
            );
            id = self.set_location(
                id.metadata().range.get().start,
                next_range.end,
                q_node,
            );
        }

        // C++ 5037-5044: `parseTypeArgsFlow()` is called with its default
        // trailing grammar context (Type, per JSParserImpl.h:1506).
        let mut type_parameters: Option<&'gc Node<'gc>> = None;
        if self.check(TokenKind::less) {
            type_parameters =
                Some(self.parse_type_args_flow(GrammarContext::Type)?);
        }

        // C++ 5046-5049.
        let node = Node::GenericTypeAnnotation(GenericTypeAnnotation::new(
            NodeMetadata::new(self.dummy_range()),
            id,
            type_parameters,
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseClassImplementsFlow — 5052 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse one entry of a class `implements` clause: `Name` or
    /// `Name<TypeArgs>`, with the current token at the identifier (an
    /// identifier ONLY — no reserved word, per the C++ assert). Port of
    /// `JSParserImpl::parseClassImplementsFlow` (flow.cpp:5053-5077).
    pub(in crate::js) fn parse_class_implements_flow(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 5054-5055.
        debug_assert!(self.check(TokenKind::identifier));
        let start = self.cur_start();

        // C++ 5057-5062.
        let id_range = self.cur_range();
        let id_node = Node::Identifier(Identifier::new(
            NodeMetadata::new(self.dummy_range()),
            self.lexer.token().get_identifier(),
            None,
            false,
        ));
        let id = self.set_location(id_range.start, id_range.end, id_node);
        self.advance(GrammarContext::Type);

        // C++ 5064-5069: `parseTypeArgsFlow()` is called with its default
        // trailing grammar context (Type, per JSParserImpl.h:1506).
        let mut type_parameters: Option<&'gc Node<'gc>> = None;
        if self.check(TokenKind::less) {
            type_parameters =
                Some(self.parse_type_args_flow(GrammarContext::Type)?);
        }

        // C++ 5071-5075.
        let node = Node::ClassImplements(ClassImplements::new(
            NodeMetadata::new(self.dummy_range()),
            id,
            type_parameters,
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }
}
