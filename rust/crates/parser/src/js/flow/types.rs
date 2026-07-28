/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The Flow type-annotation precedence hierarchy (`parseTypeAnnotationFlow`
//! → `parsePrimaryTypeAnnotationFlow`), the `typeof`/tuple productions, and
//! the reparse helpers. Port of the corresponding sections of
//! `lib/Parser/JSParserImpl-flow.cpp`.

use ast::node::{
    AnyTypeAnnotation, ArrayTypeAnnotation, BigIntLiteralTypeAnnotation,
    BigIntTypeAnnotation, BooleanLiteralTypeAnnotation, BooleanTypeAnnotation,
    ComponentTypeAnnotation, ComponentTypeParameter, ConditionalTypeAnnotation,
    EmptyTypeAnnotation, ExistsTypeAnnotation,
    FunctionTypeParam, GenericTypeAnnotation, Identifier, IndexedAccessType,
    InferTypeAnnotation,
    InterfaceTypeAnnotation, IntersectionTypeAnnotation, KeyofTypeAnnotation,
    MixedTypeAnnotation,
    NeverTypeAnnotation, Node, NullLiteralTypeAnnotation,
    NullableTypeAnnotation, NumberLiteralTypeAnnotation, NumberTypeAnnotation,
    OptionalIndexedAccessType, QualifiedTypeofIdentifier, StringLiteral,
    StringLiteralTypeAnnotation, StringTypeAnnotation, SymbolTypeAnnotation,
    TupleTypeAnnotation, TupleTypeLabeledElement, TupleTypeSpreadElement,
    TypeAnnotation, TypeOperator, TypeParameter, TypeofTypeAnnotation,
    UndefinedTypeAnnotation, UnionTypeAnnotation, UnknownTypeAnnotation,
    Variance, VoidTypeAnnotation,
};
use ast::node_child::{NodeLabel, NodeList, NodeMetadata};
use support::location::SMLoc;

use crate::js::expressions::inc_parens;
use crate::js::{JSParserImpl, Param};
use crate::lexer::GrammarContext;
use crate::token_kinds::TokenKind;

use super::{
    can_follow_variance_keyword_flow, AllowAnonFunctionType,
    AllowProtoProperty, AllowSpreadProperty, AllowStaticProperty,
};

impl<'gc, 'ast, 'ctx, 'a> JSParserImpl<'gc, 'ast, 'ctx, 'a> {
    // -----------------------------------------------------------------------
    // parseTypeAnnotationFlow — 3078 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a type annotation.
    /// Port of `JSParserImpl::parseTypeAnnotationFlow` (flow.cpp:3078-3094).
    ///
    /// \param wrapped_start if `Some`, the result is wrapped in a
    ///   `TypeAnnotation` node spanning from it to the previous token's end
    ///   (the C++ `wrappedStart` parameter, used for `: T` annotations).
    /// \param allow_anon_function_type value for `allow_anon_function_type`
    ///   while parsing this annotation (saved/restored around the parse).
    pub(in crate::js) fn parse_type_annotation_flow(
        &mut self,
        wrapped_start: Option<SMLoc>,
        allow_anon_function_type: AllowAnonFunctionType,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 3081-3083: llvh::SaveAndRestore<bool> on allowAnonFunctionType_.
        // The guard restores the old value on every exit path, including the
        // `?` early return below.
        let _guard = self.save_allow_anon_function_type(
            allow_anon_function_type == AllowAnonFunctionType::Yes,
        );
        let opt_type = self.parse_conditional_type_annotation_flow()?;
        if let Some(start) = wrapped_start {
            // C++ 3087-3092.
            let node = Node::TypeAnnotation(TypeAnnotation::new(
                NodeMetadata::new(self.dummy_range()),
                opt_type,
            ));
            return Some(self.set_location(
                start,
                self.lexer.prev_token_end(),
                node,
            ));
        }
        Some(opt_type)
    }

    // -----------------------------------------------------------------------
    // The type-annotation precedence hierarchy:
    // conditional → union → intersection → anon-fn-without-parens → prefix →
    // postfix → primary.
    // -----------------------------------------------------------------------

    /// Parse a type annotation that may be used where a colon could follow
    /// (e.g. a possibly-labeled tuple element or function parameter). Port of
    /// `parseTypeAnnotationBeforeColonFlow` (flow.cpp:3011-3076).
    pub(super) fn parse_type_annotation_before_colon_flow(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 3012-3073: if the identifier name is a known keyword we need to
        // look ahead to see if it's a type or an identifier, otherwise it
        // could fail to parse. Gated on getParseFlowComponentSyntax().
        if self.check(TokenKind::identifier) && self.parse_flow_component_syntax()
        {
            let name = self
                .gc
                .ctx()
                .atom_table
                .bytes(self.lexer.token().get_res_word_or_identifier())
                .to_owned();
            let renders_q =
                name == b"renders" && self.lexer.check_following_character(b'?');
            if (name == b"component" || name == b"hook")
                || (name == b"renders" && !renders_q)
            {
                // C++ 3016-3035: `component`/`hook`/`renders` (no following
                // `?`) followed by `:` or `?` is a label, parsed as a generic
                // type whose id is the keyword.
                let opt_next = self.lexer.lookahead1::<true>(None);
                if opt_next == Some(TokenKind::colon)
                    || opt_next == Some(TokenKind::question)
                {
                    let id = self.make_keyword_generic_type();
                    self.advance(GrammarContext::Type);
                    return Some(id);
                }
            } else if renders_q {
                // C++ 3036-3071: `renders?` — either a `renders?` label on a
                // `:`-typed element, or the `renders?` type operator.
                let start_loc = self.cur_start();
                let id = self.make_keyword_generic_type();
                self.advance(GrammarContext::Type);
                let opt_next = self.lexer.lookahead1::<true>(None);
                if opt_next == Some(TokenKind::colon) {
                    return Some(id);
                }
                // C++ 3055-3062.
                if !self.eat(
                    TokenKind::question,
                    GrammarContext::Type,
                    " in render type annotation",
                ) {
                    return None;
                }
                let body = self.parse_prefix_type_annotation_flow()?;
                let operator =
                    self.gc.ctx().atom_table.atom_bytes(b"renders?");
                let node = Node::TypeOperator(TypeOperator::new(
                    NodeMetadata::new(self.dummy_range()),
                    operator,
                    body,
                ));
                return Some(self.set_location(
                    start_loc,
                    self.lexer.prev_token_end(),
                    node,
                ));
            }
        }

        // C++ 3075.
        self.parse_type_annotation_flow(None, AllowAnonFunctionType::Yes)
    }

    /// Build a `GenericTypeAnnotation` whose id is the current token's keyword
    /// identifier (a `component`/`hook`/`renders` contextual keyword used as a
    /// labelled-element name). Helper for the label-disambiguation paths of
    /// `parseTypeAnnotationBeforeColonFlow` (flow.cpp:3023-3032 / 3040-3049).
    /// Does NOT advance — the caller advances after.
    fn make_keyword_generic_type(&mut self) -> &'gc Node<'gc> {
        let range = self.cur_range();
        let id_node = Node::Identifier(Identifier::new(
            NodeMetadata::new(self.dummy_range()),
            self.lexer.token().get_res_word_or_identifier(),
            None,
            false,
        ));
        let id = self.set_location(range.start, range.end, id_node);
        let generic = Node::GenericTypeAnnotation(GenericTypeAnnotation::new(
            NodeMetadata::new(self.dummy_range()),
            id,
            None,
        ));
        self.set_location(range.start, range.end, generic)
    }

    /// Port of `parseConditionalTypeAnnotationFlow` (flow.cpp:3096-3145).
    fn parse_conditional_type_annotation_flow(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 3098: conditional types are allowed while parsing the check
        // type.
        let _guard = self.save_allow_conditional_type(true);
        let check_type = self.parse_union_type_annotation_flow()?;
        // C++ 3102-3104.
        if !self.check_and_eat(TokenKind::rw_extends, GrammarContext::Type) {
            return Some(check_type);
        }

        let extends_type = {
            // C++ 3106-3110: We need to enter the state of parsing the
            // extends_type disallowing conditional types not wrapped by
            // parantheses, so that the following sequence
            // `A extends infer B extends C ? D : E` will be interpreted
            // as `A extends (infer B extends C) ? D : E`.
            let _guard = self.save_allow_conditional_type(false);
            self.parse_union_type_annotation_flow()
        }?;

        // C++ 3117-3123.
        if !self.eat(
            TokenKind::question,
            GrammarContext::Type,
            " in conditional type",
        ) {
            return None;
        }

        // C++ 3125-3126.
        let true_type =
            self.parse_type_annotation_flow(None, AllowAnonFunctionType::Yes)?;

        // C++ 3128-3134.
        if !self.eat(
            TokenKind::colon,
            GrammarContext::Type,
            " in conditional type",
        ) {
            return None;
        }

        // C++ 3136-3138.
        let false_type =
            self.parse_type_annotation_flow(None, AllowAnonFunctionType::Yes)?;

        // C++ 3140-3144: located from the check type's start (NOT the start
        // of this production — they only differ if error recovery moved us).
        let node = Node::ConditionalTypeAnnotation(
            ConditionalTypeAnnotation::new(
                NodeMetadata::new(self.dummy_range()),
                check_type,
                extends_type,
                true_type,
                false_type,
            ),
        );
        Some(self.set_location(
            check_type.metadata().range.get().start,
            self.lexer.prev_token_end(),
            node,
        ))
    }

    /// Port of `parseUnionTypeAnnotationFlow` (flow.cpp:3147-3174).
    pub(super) fn parse_union_type_annotation_flow(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 3148-3149: `start` is captured BEFORE the optional leading `|`.
        let start = self.cur_start();
        self.check_and_eat(TokenKind::pipe, GrammarContext::Type);

        let first = self.parse_intersection_type_annotation_flow()?;

        if !self.check(TokenKind::pipe) {
            // Done with the union, move on.
            return Some(first);
        }

        let mut types: Vec<&'gc Node<'gc>> = vec![first];
        while self.check_and_eat(TokenKind::pipe, GrammarContext::Type) {
            types.push(self.parse_intersection_type_annotation_flow()?);
        }

        // C++ 3170-3173.
        let node = Node::UnionTypeAnnotation(UnionTypeAnnotation::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, types),
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    /// Port of `parseIntersectionTypeAnnotationFlow` (flow.cpp:3176-3204).
    fn parse_intersection_type_annotation_flow(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 3177-3178: `start` is captured BEFORE the optional leading `&`.
        let start = self.cur_start();
        self.check_and_eat(TokenKind::amp, GrammarContext::Type);

        let first =
            self.parse_anon_function_without_parens_type_annotation_flow()?;

        if !self.check(TokenKind::amp) {
            // Done with the union, move on.
            return Some(first);
        }

        let mut types: Vec<&'gc Node<'gc>> = vec![first];
        while self.check_and_eat(TokenKind::amp, GrammarContext::Type) {
            types.push(
                self.parse_anon_function_without_parens_type_annotation_flow()?,
            );
        }

        // C++ 3199-3202.
        let node =
            Node::IntersectionTypeAnnotation(IntersectionTypeAnnotation::new(
                NodeMetadata::new(self.dummy_range()),
                NodeList::from_iter(self.gc, types),
            ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    /// Port of `parseAnonFunctionWithoutParensTypeAnnotationFlow`
    /// (flow.cpp:3206-3230).
    fn parse_anon_function_without_parens_type_annotation_flow(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        let start = self.cur_start();
        let param = self.parse_prefix_type_annotation_flow()?;

        // C++ 3212-3228.
        if self.allow_anon_function_type.get()
            && self.check(TokenKind::equalgreater)
        {
            // ParamType => ReturnType
            //           ^
            // "Reparse" the param into a FunctionTypeParam so it can be used
            // for parseFunctionTypeAnnotationWithParamsFlow. C++ 3216-3221:
            // it spans exactly the param's range.
            let param_range = param.metadata().range.get();
            let ftp_node = Node::FunctionTypeParam(FunctionTypeParam::new(
                NodeMetadata::new(self.dummy_range()),
                None, // name
                param,
                false, // optional
            ));
            let ftp =
                self.set_location(param_range.start, param_range.end, ftp_node);
            return self.parse_function_type_annotation_with_params_flow(
                start,
                vec![ftp],
                None,  // this constraint
                None,  // rest
                None,  // type params
                false, // hook
            );
        }

        Some(param)
    }

    /// Port of `parsePrefixTypeAnnotationFlow` (flow.cpp:3232-3244).
    pub(super) fn parse_prefix_type_annotation_flow(&mut self) -> Option<&'gc Node<'gc>> {
        let start = self.cur_start();
        // C++ 3234-3242: nullable `?T` (right-recursive, so `??T` nests).
        if self.check_and_eat(TokenKind::question, GrammarContext::Type) {
            let prefix = self.parse_prefix_type_annotation_flow()?;
            let node =
                Node::NullableTypeAnnotation(NullableTypeAnnotation::new(
                    NodeMetadata::new(self.dummy_range()),
                    prefix,
                ));
            return Some(self.set_location(
                start,
                self.lexer.prev_token_end(),
                node,
            ));
        }
        self.parse_postfix_type_annotation_flow()
    }

    /// Port of `parsePostfixTypeAnnotationFlow` (flow.cpp:3246-3303).
    fn parse_postfix_type_annotation_flow(&mut self) -> Option<&'gc Node<'gc>> {
        let start = self.cur_start();
        let mut result = self.parse_primary_type_annotation_flow()?;
        let mut seen_optional_indexed_access = false;

        // C++ 3255-3256.
        while self.check2(TokenKind::l_square, TokenKind::questiondot)
            && !self.lexer.is_new_line_before_current_token()
        {
            // C++ 3257: `checkAndEat(questiondot)` uses the DEFAULT grammar
            // context (AllowRegExp), NOT Type — deliberate; keep it.
            let optional = self.check_and_eat(
                TokenKind::questiondot,
                GrammarContext::AllowRegExp,
            );
            seen_optional_indexed_access =
                seen_optional_indexed_access || optional;

            // C++ 3260-3266.
            if !self.eat(
                TokenKind::l_square,
                GrammarContext::Type,
                " in indexed access type or postfix array type syntax",
            ) {
                return None;
            }

            if !optional
                && self.check_and_eat(TokenKind::r_square, GrammarContext::Type)
            {
                // Legacy Array syntax `T[]` (C++ 3268-3274; spans from this
                // production's start).
                let node = Node::ArrayTypeAnnotation(ArrayTypeAnnotation::new(
                    NodeMetadata::new(self.dummy_range()),
                    result,
                ));
                result = self.set_location(
                    start,
                    self.lexer.prev_token_end(),
                    node,
                );
            } else {
                // Indexed Access `T[K]` (`T?.[K]` if `optional`),
                // C++ 3276-3298.
                let index_type = self.parse_type_annotation_flow(
                    None,
                    AllowAnonFunctionType::Yes,
                )?;
                if !self.need(TokenKind::r_square, " in indexed access type") {
                    return None;
                }
                // Once a `?.[` has been seen, all the enclosing accesses
                // become OptionalIndexedAccessType (with optional=false for
                // the plain `[` ones).
                if seen_optional_indexed_access {
                    let node = Node::OptionalIndexedAccessType(
                        OptionalIndexedAccessType::new(
                            NodeMetadata::new(self.dummy_range()),
                            result,
                            index_type,
                            optional,
                        ),
                    );
                    let end = self.advance(GrammarContext::Type).end;
                    result = self.set_location(start, end, node);
                } else {
                    let node = Node::IndexedAccessType(IndexedAccessType::new(
                        NodeMetadata::new(self.dummy_range()),
                        result,
                        index_type,
                    ));
                    let end = self.advance(GrammarContext::Type).end;
                    result = self.set_location(start, end, node);
                }
            }
        }

        Some(result)
    }

    // -----------------------------------------------------------------------
    // parsePrimaryTypeAnnotationFlow — 3305 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a primary type annotation. Port of
    /// `JSParserImpl::parsePrimaryTypeAnnotationFlow` (flow.cpp:3305-3602).
    fn parse_primary_type_annotation_flow(&mut self) -> Option<&'gc Node<'gc>> {
        let start = self.cur_start();
        match self.cur_kind() {
            // C++ 3308-3312.
            TokenKind::star => {
                let node = Node::ExistsTypeAnnotation(ExistsTypeAnnotation::new(
                    NodeMetadata::new(self.dummy_range()),
                ));
                let end = self.advance(GrammarContext::Type).end;
                Some(self.set_location(start, end, node))
            }

            // C++ 3313-3314.
            TokenKind::less => self.parse_function_type_annotation_flow(),

            // C++ 3315-3316.
            TokenKind::l_paren => {
                self.parse_function_or_group_type_annotation_flow()
            }

            // C++ 3317-3322.
            TokenKind::l_brace | TokenKind::l_bracepipe => self
                .parse_object_type_annotation_flow(
                    AllowProtoProperty::No,
                    AllowStaticProperty::No,
                    AllowSpreadProperty::Yes,
                ),

            // C++ 3323-3334. `interface` lexes as rw_interface only in
            // strict mode (it is a future reserved word); in loose mode it is
            // an identifier and reaches the NamedType::Interface arm below.
            TokenKind::rw_interface => {
                self.advance(GrammarContext::Type);
                let mut extends: Vec<&'gc Node<'gc>> = Vec::new();
                let body = self.parse_interface_tail_flow(&mut extends)?;
                // The end location is the body node's end.
                let end = body.metadata().range.get().end;
                let node = Node::InterfaceTypeAnnotation(
                    InterfaceTypeAnnotation::new(
                        NodeMetadata::new(self.dummy_range()),
                        NodeList::from_iter(self.gc, extends),
                        Some(body),
                    ),
                );
                Some(self.set_location(start, end, node))
            }

            // C++ 3335-3336.
            TokenKind::rw_typeof => self.parse_typeof_type_annotation_flow(),

            // C++ 3338-3339.
            TokenKind::l_square => self.parse_tuple_type_annotation_flow(),

            // C++ 3340-3511. The C++ compares `tok_->getResWordOrIdentifier()`
            // against the pre-interned `anyIdent_`/`mixedIdent_`/... atoms
            // (escape-insensitive); we compare the token's interned name bytes
            // directly. Each named-primitive arm is
            // `setLocation(start, advance(GrammarContext::Type).End, new
            // <Name>Node())` (C++ 3343-3408).
            TokenKind::rw_static | TokenKind::rw_this | TokenKind::identifier => {
                /// Dispatch outcome of the named-type match below: either a
                /// finished primitive node (consume the token and return), or
                /// one of the multi-token productions.
                enum NamedType<'gc> {
                    Prim(Node<'gc>),
                    Keyof,
                    Renders,
                    Component,
                    Hook,
                    Interface,
                    Infer,
                    Generic,
                }
                // C++ 3420/3432/3439: the `renders`/`component`/`hook` arms
                // are gated on getParseFlowComponentSyntax().
                let component_syntax = self.parse_flow_component_syntax();
                let arm = {
                    let name = self.lexer.get_string_table().bytes(
                        self.lexer.token().get_res_word_or_identifier(),
                    );
                    let md = NodeMetadata::new(self.dummy_range());
                    match name {
                        // C++ 3343-3347.
                        b"any" => NamedType::Prim(Node::AnyTypeAnnotation(
                            AnyTypeAnnotation::new(md),
                        )),
                        // C++ 3348-3353.
                        b"mixed" => NamedType::Prim(Node::MixedTypeAnnotation(
                            MixedTypeAnnotation::new(md),
                        )),
                        // C++ 3354-3359.
                        b"empty" => NamedType::Prim(Node::EmptyTypeAnnotation(
                            EmptyTypeAnnotation::new(md),
                        )),
                        // C++ 3360-3365.
                        b"unknown" => NamedType::Prim(
                            Node::UnknownTypeAnnotation(
                                UnknownTypeAnnotation::new(md),
                            ),
                        ),
                        // C++ 3366-3371.
                        b"never" => NamedType::Prim(Node::NeverTypeAnnotation(
                            NeverTypeAnnotation::new(md),
                        )),
                        // C++ 3372-3377.
                        b"undefined" => NamedType::Prim(
                            Node::UndefinedTypeAnnotation(
                                UndefinedTypeAnnotation::new(md),
                            ),
                        ),
                        // C++ 3378-3384.
                        b"boolean" | b"bool" => NamedType::Prim(
                            Node::BooleanTypeAnnotation(
                                BooleanTypeAnnotation::new(md),
                            ),
                        ),
                        // C++ 3385-3390.
                        b"number" => NamedType::Prim(
                            Node::NumberTypeAnnotation(
                                NumberTypeAnnotation::new(md),
                            ),
                        ),
                        // C++ 3391-3396.
                        b"symbol" => NamedType::Prim(
                            Node::SymbolTypeAnnotation(
                                SymbolTypeAnnotation::new(md),
                            ),
                        ),
                        // C++ 3397-3402.
                        b"string" => NamedType::Prim(
                            Node::StringTypeAnnotation(
                                StringTypeAnnotation::new(md),
                            ),
                        ),
                        // C++ 3403-3408.
                        b"bigint" => NamedType::Prim(
                            Node::BigIntTypeAnnotation(
                                BigIntTypeAnnotation::new(md),
                            ),
                        ),
                        // C++ 3410-3418.
                        b"keyof" => NamedType::Keyof,
                        // C++ 3420-3431.
                        b"renders" if component_syntax => NamedType::Renders,
                        // C++ 3432-3438.
                        b"component" if component_syntax => {
                            NamedType::Component
                        }
                        // C++ 3439-3445.
                        b"hook" if component_syntax => NamedType::Hook,
                        // C++ 3447-3457.
                        b"interface" => NamedType::Interface,
                        // C++ 3459-3504.
                        b"infer" => NamedType::Infer,
                        // C++ 3506-3511.
                        _ => NamedType::Generic,
                    }
                };
                match arm {
                    NamedType::Prim(node) => {
                        let end = self.advance(GrammarContext::Type).end;
                        Some(self.set_location(start, end, node))
                    }
                    NamedType::Keyof => {
                        // C++ 3410-3418.
                        self.advance(GrammarContext::Type);
                        let body = self.parse_prefix_type_annotation_flow()?;
                        let node =
                            Node::KeyofTypeAnnotation(KeyofTypeAnnotation::new(
                                NodeMetadata::new(self.dummy_range()),
                                body,
                            ));
                        Some(self.set_location(
                            start,
                            self.lexer.prev_token_end(),
                            node,
                        ))
                    }
                    NamedType::Renders => {
                        // C++ 3420-3431.
                        let operator = self.parse_render_type_operator();
                        let body = self.parse_prefix_type_annotation_flow();
                        let (Some(body), Some(operator)) = (body, operator)
                        else {
                            return None;
                        };
                        let node = Node::TypeOperator(TypeOperator::new(
                            NodeMetadata::new(self.dummy_range()),
                            operator,
                            body,
                        ));
                        Some(self.set_location(
                            start,
                            self.lexer.prev_token_end(),
                            node,
                        ))
                    }
                    NamedType::Component => {
                        // C++ 3432-3438.
                        self.parse_component_type_annotation_flow()
                    }
                    NamedType::Hook => {
                        // C++ 3439-3445.
                        self.parse_hook_type_annotation_flow()
                    }
                    NamedType::Interface => {
                        // C++ 3447-3457 (the loose-mode spelling, where
                        // `interface` is an identifier rather than
                        // rw_interface).
                        self.advance(GrammarContext::Type);
                        let mut extends: Vec<&'gc Node<'gc>> = Vec::new();
                        let body =
                            self.parse_interface_tail_flow(&mut extends)?;
                        // The end location is the body node's end.
                        let end = body.metadata().range.get().end;
                        let node = Node::InterfaceTypeAnnotation(
                            InterfaceTypeAnnotation::new(
                                NodeMetadata::new(self.dummy_range()),
                                NodeList::from_iter(self.gc, extends),
                                Some(body),
                            ),
                        );
                        Some(self.set_location(start, end, node))
                    }
                    NamedType::Infer => {
                        // C++ 3459-3504.
                        self.advance(GrammarContext::Type);

                        // C++ 3461-3462.
                        if !self.need(
                            TokenKind::identifier,
                            " in type parameter",
                        ) {
                            return None;
                        }
                        let name = self.lexer.token().get_identifier();
                        self.advance(GrammarContext::Type);

                        let mut bound: Option<&'gc Node<'gc>> = None;
                        if self.check(TokenKind::rw_extends) {
                            // When we see an extends keyword,
                            // we enter the parsing logic that might need
                            // backtracking.
                            //
                            // For `infer A extends B ...`, is the `extends B`
                            // part of an infer type, or part of a larger
                            // conditional type like
                            // `infer A extends B ? C : D`?
                            //
                            // We don't know, so we assume it's part of the
                            // infer type for now, and later backtrack if the
                            // assumption is wrong.
                            //
                            // NOTE: like the C++, diagnostics are NOT
                            // suppressed during the speculative bound parse —
                            // a failed bound emits its errors and then still
                            // restores.
                            let save_point = self.lexer.save_point();
                            self.advance(GrammarContext::Type);
                            let parsed_bound =
                                self.parse_union_type_annotation_flow();
                            if (self.allow_conditional_type.get()
                                && self.check(TokenKind::question))
                                || parsed_bound.is_none()
                            {
                                // If we look ahead and see `?`, it might be
                                // the case that we are parsing a conditional
                                // type like `infer A extends B ? C : D`. If
                                // the current context allow parsing
                                // conditional type, then we must backtrack so
                                // that only `infer A` is treated as part of
                                // the infer type.
                                //
                                // Of course, if we fail to parse the type
                                // after extends, we also need to backtrack.
                                save_point.restore(&mut self.lexer);
                            } else {
                                bound = parsed_bound;
                            }
                        }

                        // C++ 3496-3503: the TypeParameter spans the same
                        // range as the InferTypeAnnotation.
                        let end = self.lexer.prev_token_end();
                        let type_param_node =
                            Node::TypeParameter(TypeParameter::new(
                                NodeMetadata::new(self.dummy_range()),
                                name,
                                false, // const
                                bound,
                                None, // variance
                                None, // default
                                true, // usesExtendsBound
                            ));
                        let type_param =
                            self.set_location(start, end, type_param_node);
                        let node =
                            Node::InferTypeAnnotation(InferTypeAnnotation::new(
                                NodeMetadata::new(self.dummy_range()),
                                type_param,
                            ));
                        Some(self.set_location(start, end, node))
                    }
                    NamedType::Generic => self.parse_generic_type_flow(),
                }
            }

            // C++ 3513-3517.
            TokenKind::rw_null => {
                let node = Node::NullLiteralTypeAnnotation(
                    NullLiteralTypeAnnotation::new(NodeMetadata::new(
                        self.dummy_range(),
                    )),
                );
                let end = self.advance(GrammarContext::Type).end;
                Some(self.set_location(start, end, node))
            }

            // C++ 3519-3523.
            TokenKind::rw_void => {
                let node = Node::VoidTypeAnnotation(VoidTypeAnnotation::new(
                    NodeMetadata::new(self.dummy_range()),
                ));
                let end = self.advance(GrammarContext::Type).end;
                Some(self.set_location(start, end, node))
            }

            // C++ 3525-3532.
            TokenKind::string_literal => {
                let value = self.lexer.token().get_string_literal();
                // C++: `lexer_.getStringLiteral(tok_->inputStr())` — the raw
                // SOURCE text of the token (including the quotes), interned.
                let raw = self.cur_token_source_atom();
                let node = Node::StringLiteralTypeAnnotation(
                    StringLiteralTypeAnnotation::new(
                        NodeMetadata::new(self.dummy_range()),
                        value,
                        raw,
                    ),
                );
                let end = self.advance(GrammarContext::Type).end;
                Some(self.set_location(start, end, node))
            }

            // C++ 3534-3541.
            TokenKind::numeric_literal => {
                let value = self.lexer.token().get_numeric_literal();
                let raw = self.cur_token_source_atom();
                let node = Node::NumberLiteralTypeAnnotation(
                    NumberLiteralTypeAnnotation::new(
                        NodeMetadata::new(self.dummy_range()),
                        value,
                        raw,
                    ),
                );
                let end = self.advance(GrammarContext::Type).end;
                Some(self.set_location(start, end, node))
            }

            // C++ 3543-3549.
            TokenKind::bigint_literal => {
                let raw = self.lexer.token().get_bigint_literal_raw_value();
                let node = Node::BigIntLiteralTypeAnnotation(
                    BigIntLiteralTypeAnnotation::new(
                        NodeMetadata::new(self.dummy_range()),
                        raw,
                    ),
                );
                let end = self.advance(GrammarContext::Type).end;
                Some(self.set_location(start, end, node))
            }

            // C++ 3551-3581.
            TokenKind::minus => {
                self.advance(GrammarContext::Type);
                if self.check(TokenKind::numeric_literal) {
                    // Negate the literal (C++ 3553-3563). The raw text spans
                    // from the `-` through the end of the literal token.
                    let value = -self.lexer.token().get_numeric_literal();
                    let raw =
                        self.source_bytes_atom(start, self.cur_range().end);
                    let node = Node::NumberLiteralTypeAnnotation(
                        NumberLiteralTypeAnnotation::new(
                            NodeMetadata::new(self.dummy_range()),
                            value,
                            raw,
                        ),
                    );
                    let end = self.advance(GrammarContext::Type).end;
                    Some(self.set_location(start, end, node))
                } else if self.check(TokenKind::bigint_literal) {
                    // C++ 3564-3572: the BigInt raw keeps the `-` prefix.
                    let raw =
                        self.source_bytes_atom(start, self.cur_range().end);
                    let node = Node::BigIntLiteralTypeAnnotation(
                        BigIntLiteralTypeAnnotation::new(
                            NodeMetadata::new(self.dummy_range()),
                            raw,
                        ),
                    );
                    let end = self.advance(GrammarContext::Type).end;
                    Some(self.set_location(start, end, node))
                } else {
                    // C++ 3573-3578: errorExpected(numeric_literal,
                    // "in type annotation", "start of annotation", start).
                    // `start` is real, so this routes through `need_at`.
                    self.need_at(
                        TokenKind::numeric_literal,
                        " in type annotation",
                        start,
                    );
                    None
                }
            }

            // C++ 3583-3591.
            TokenKind::rw_true | TokenKind::rw_false => {
                let value = self.check(TokenKind::rw_true);
                let raw = self.lexer.token().get_res_word_identifier();
                let node = Node::BooleanLiteralTypeAnnotation(
                    BooleanLiteralTypeAnnotation::new(
                        NodeMetadata::new(self.dummy_range()),
                        value,
                        raw,
                    ),
                );
                let end = self.advance(GrammarContext::Type).end;
                Some(self.set_location(start, end, node))
            }

            // C++ 3592-3600.
            _ => {
                if self.lexer.token().is_res_word() {
                    // C++ 3593-3597.
                    return self.parse_generic_type_flow();
                }
                self.error_cur("unexpected token in type annotation");
                None
            }
        }
    }

    // -----------------------------------------------------------------------
    // parseTypeofTypeAnnotationFlow — 3604 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a `typeof X.Y<Args>` type annotation, with the current token at
    /// `typeof`. Port of `parseTypeofTypeAnnotationFlow`
    /// (flow.cpp:3604-3666).
    fn parse_typeof_type_annotation_flow(&mut self) -> Option<&'gc Node<'gc>> {
        debug_assert!(self.check(TokenKind::rw_typeof));
        // C++ 3606: a bare `advance()` — the default GrammarContext
        // (AllowRegExp), NOT Type; deliberate.
        let start = self.advance(GrammarContext::AllowRegExp).start;
        let mut paren_count: u32 = 0;

        // C++ 3609-3610: default grammar context again.
        while self.check_and_eat(TokenKind::l_paren, GrammarContext::AllowRegExp)
        {
            paren_count += 1;
        }

        // C++ 3612-3613: whatLoc is `startLoc` (the 'typeof' keyword).
        if !self.need_at(TokenKind::identifier, " in typeof type", start) {
            return None;
        }

        // C++ 3615-3620.
        let ident_range = self.cur_range();
        let ident_node = Node::Identifier(Identifier::new(
            NodeMetadata::new(self.dummy_range()),
            self.lexer.token().get_identifier(),
            None,
            false,
        ));
        let mut ident =
            self.set_location(ident_range.start, ident_range.end, ident_node);
        self.advance(GrammarContext::Type);

        // C++ 3622: `checkAndEat(period)` with the default grammar context.
        while self.check_and_eat(TokenKind::period, GrammarContext::AllowRegExp)
        {
            // C++ 3623-3630.
            if !self.check(TokenKind::identifier)
                && !self.lexer.token().is_res_word()
            {
                // flow.cpp:3623-3630: errorExpected(identifier, "in
                // qualified typeof type", "start of type", startLoc).
                // `start` is real, so this routes through `need_at`.
                self.need_at(
                    TokenKind::identifier,
                    " in qualified typeof type",
                    start,
                );
                return None;
            }
            // C++ 3631-3636.
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
            // C++ 3637-3640: spans from the qualification's start to the new
            // id's end.
            let q_node = Node::QualifiedTypeofIdentifier(
                QualifiedTypeofIdentifier::new(
                    NodeMetadata::new(self.dummy_range()),
                    ident,
                    next,
                ),
            );
            ident = self.set_location(
                ident.metadata().range.get().start,
                next_range.end,
                q_node,
            );
        }

        // C++ 3643-3651: close the wrapping parens, recording them on the
        // (possibly qualified) identifier node.
        for _ in 0..paren_count {
            if !self.eat(
                TokenKind::r_paren,
                GrammarContext::Type,
                " in typeof type",
            ) {
                return None;
            }
            inc_parens(ident);
        }

        // C++ 3653-3660: `parseTypeArgsFlow()` is called with its default
        // trailing grammar context (Type, per JSParserImpl.h:1506).
        let mut type_arguments: Option<&'gc Node<'gc>> = None;
        if self.check(TokenKind::less)
            && !self.lexer.is_new_line_before_current_token()
        {
            type_arguments =
                Some(self.parse_type_args_flow(GrammarContext::Type)?);
        }

        // C++ 3662-3665.
        let node = Node::TypeofTypeAnnotation(TypeofTypeAnnotation::new(
            NodeMetadata::new(self.dummy_range()),
            ident,
            type_arguments,
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseTupleTypeAnnotationFlow — 3668 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a tuple type annotation, with the current token at `[`.
    /// Port of `parseTupleTypeAnnotationFlow` (flow.cpp:3668-3712).
    fn parse_tuple_type_annotation_flow(&mut self) -> Option<&'gc Node<'gc>> {
        debug_assert!(self.check(TokenKind::l_square));
        // C++ 3670.
        let start = self.advance(GrammarContext::Type).start;

        let mut element_types: Vec<&'gc Node<'gc>> = Vec::new();
        let mut inexact = false;

        // C++ 3675-3698.
        while !self.check(TokenKind::r_square) {
            let elem_start = self.cur_start();
            let starts_with_dotdotdot =
                self.check_and_eat(TokenKind::dotdotdot, GrammarContext::Type);

            if starts_with_dotdotdot && self.check(TokenKind::r_square) {
                // ...]
                inexact = true;
            } else if starts_with_dotdotdot && self.check(TokenKind::comma) {
                // ...,
                self.error_cur(
                    "trailing commas after inexact tuple types are not allowed",
                );
                self.advance(GrammarContext::Type);
            } else {
                let elem = self.parse_tuple_element_flow(
                    elem_start,
                    starts_with_dotdotdot,
                )?;
                element_types.push(elem);

                if !self.check_and_eat(TokenKind::comma, GrammarContext::Type) {
                    break;
                }
            }
        }

        // C++ 3700-3705.
        if !self.need(TokenKind::r_square, " at end of tuple type annotation") {
            return None;
        }

        // C++ 3707-3711.
        let node = Node::TupleTypeAnnotation(TupleTypeAnnotation::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, element_types),
            inexact,
        ));
        let end = self.advance(GrammarContext::Type).end;
        Some(self.set_location(start, end, node))
    }

    /// Parse one tuple type element, with `start` at the element's start
    /// (including a leading `...`, already consumed iff
    /// `starts_with_dotdotdot`). Port of `parseTupleElementFlow`
    /// (flow.cpp:3714-3814).
    fn parse_tuple_element_flow(
        &mut self,
        start: SMLoc,
        starts_with_dotdotdot: bool,
    ) -> Option<&'gc Node<'gc>> {
        let mut variance: Option<&'gc Node<'gc>> = None;

        // ...Identifier : Type
        // ...Type
        // ^
        if starts_with_dotdotdot {
            // C++ 3725-3748.
            let ty = self.parse_type_annotation_before_colon_flow()?;
            if self.check_and_eat(TokenKind::colon, GrammarContext::Type) {
                let label =
                    self.reparse_type_annotation_as_identifier_flow(ty)?;
                let element_type = self.parse_type_annotation_flow(
                    None,
                    AllowAnonFunctionType::Yes,
                )?;
                let node = Node::TupleTypeSpreadElement(
                    TupleTypeSpreadElement::new(
                        NodeMetadata::new(self.dummy_range()),
                        Some(label),
                        element_type,
                    ),
                );
                return Some(self.set_location(
                    start,
                    self.lexer.prev_token_end(),
                    node,
                ));
            }

            let node = Node::TupleTypeSpreadElement(
                TupleTypeSpreadElement::new(
                    NodeMetadata::new(self.dummy_range()),
                    None, // label
                    ty,
                ),
            );
            return Some(self.set_location(
                start,
                self.lexer.prev_token_end(),
                node,
            ));
        }

        // +Identifier : Type
        // -Identifier : Type
        // readonly Identifier : Type
        // writeonly Identifier : Type
        // ^
        if self.check2(TokenKind::plus, TokenKind::minus) {
            // C++ 3756-3762: the Variance kind is the interned "plus" /
            // "minus" atom (plusIdent_ / minusIdent_).
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
        } else if (self.check_name(b"readonly")
            || self.check_name(b"writeonly"))
            && can_follow_variance_keyword_flow(
                self.lexer.lookahead1::<true>(None),
            )
        {
            // C++ 3763-3768.
            let v_range = self.cur_range();
            let v_node = Node::Variance(Variance::new(
                NodeMetadata::new(self.dummy_range()),
                self.lexer.token().get_identifier(),
            ));
            variance =
                Some(self.set_location(v_range.start, v_range.end, v_node));
            self.advance(GrammarContext::Type);
        }

        // Identifier [?] : Type
        // Type
        // ^
        let ty = self.parse_type_annotation_before_colon_flow()?;

        // Identifier [?] : Type
        //             ^
        if self.check2(TokenKind::colon, TokenKind::question) {
            // C++ 3781-3783.
            let optional =
                self.check_and_eat(TokenKind::question, GrammarContext::Type);

            // C++ 3785-3792.
            if !self.eat(
                TokenKind::colon,
                GrammarContext::Type,
                " in labeled tuple type element",
            ) {
                return None;
            }

            let label = self.reparse_type_annotation_as_identifier_flow(ty)?;
            let element_type = self
                .parse_type_annotation_flow(None, AllowAnonFunctionType::Yes)?;

            // C++ 3800-3804.
            let node = Node::TupleTypeLabeledElement(
                TupleTypeLabeledElement::new(
                    NodeMetadata::new(self.dummy_range()),
                    label,
                    element_type,
                    optional,
                    variance,
                ),
            );
            return Some(self.set_location(
                start,
                self.lexer.prev_token_end(),
                node,
            ));
        }

        // C++ 3806-3811.
        if let Some(variance) = variance {
            let range = variance.metadata().range.get();
            self.error_at(
                range,
                "Variance can only be used with labeled tuple elements",
            );
        }

        Some(ty)
    }

    // -----------------------------------------------------------------------
    // reparseTypeAnnotationAsIdFlow — 5100 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Map a type-annotation node back to the identifier atom it would have
    /// parsed as, reporting "identifier expected" at the node if impossible.
    /// Port of `reparseTypeAnnotationAsIdFlow` (flow.cpp:5100-5133).
    ///
    /// NOTE: BooleanTypeAnnotation maps to "boolean" even when the source
    /// spelled it `bool` — the C++ maps both spellings to booleanIdent_ the
    /// same way (5105-5106).
    pub(super) fn reparse_type_annotation_as_id_flow(
        &mut self,
        type_annotation: &'gc Node<'gc>,
    ) -> Option<NodeLabel> {
        let id: Option<NodeLabel> = match type_annotation {
            Node::AnyTypeAnnotation(_) => {
                Some(self.lexer.get_identifier(b"any"))
            }
            Node::EmptyTypeAnnotation(_) => {
                Some(self.lexer.get_identifier(b"empty"))
            }
            Node::BooleanTypeAnnotation(_) => {
                Some(self.lexer.get_identifier(b"boolean"))
            }
            Node::NumberTypeAnnotation(_) => {
                Some(self.lexer.get_identifier(b"number"))
            }
            Node::StringTypeAnnotation(_) => {
                Some(self.lexer.get_identifier(b"string"))
            }
            Node::SymbolTypeAnnotation(_) => {
                Some(self.lexer.get_identifier(b"symbol"))
            }
            Node::NullLiteralTypeAnnotation(_) => {
                Some(self.lexer.get_identifier(b"null"))
            }
            // C++ 5117-5125: a generic without type arguments whose id is a
            // plain Identifier reparses as that identifier.
            Node::GenericTypeAnnotation(generic)
                if generic.type_parameters.is_none() =>
            {
                if let Node::Identifier(generic_id) = generic.id {
                    Some(generic_id.name.get())
                } else {
                    None
                }
            }
            _ => None,
        };

        if id.is_none() {
            // C++ 5127-5131.
            let range = type_annotation.metadata().range.get();
            self.error_at(range, "identifier expected");
        }
        id
    }

    /// Reparse a type-annotation node as an `Identifier` node spanning the
    /// original node's source range. Port of
    /// `reparseTypeAnnotationAsIdentifierFlow` (flow.cpp:5135-5146).
    pub(super) fn reparse_type_annotation_as_identifier_flow(
        &mut self,
        type_annotation: &'gc Node<'gc>,
    ) -> Option<&'gc Node<'gc>> {
        let id = self.reparse_type_annotation_as_id_flow(type_annotation)?;

        // C++ 5141-5145.
        let range = type_annotation.metadata().range.get();
        let node = Node::Identifier(Identifier::new(
            NodeMetadata::new(self.dummy_range()),
            id,
            None,
            false,
        ));
        Some(self.set_location(range.start, range.end, node))
    }

    // -----------------------------------------------------------------------
    // parseComponentTypeAnnotationFlow — 555 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a `component(params) renders T` TYPE annotation, with the cursor
    /// at `component`. Port of
    /// `JSParserImpl::parseComponentTypeAnnotationFlow` (flow.cpp:555-604).
    pub(super) fn parse_component_type_annotation_flow(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 557-558.
        debug_assert!(self.check_name(b"component"));
        let start = self.advance(GrammarContext::Type).start;

        // C++ 560-566: component type annotations should not contain a name.
        if self.check(TokenKind::identifier) {
            self.error_at(
                self.cur_range(),
                "component type annotations should not contain a name",
            );
            self.advance(GrammarContext::Type);
        }

        // C++ 568-575.
        let mut type_params: Option<&'gc Node<'gc>> = None;
        if self.check(TokenKind::less) {
            type_params = Some(self.parse_type_params_flow()?);
        }

        // C++ 577-583.
        if !self.need(
            TokenKind::l_paren,
            " at start of component parameter list",
        ) {
            return None;
        }

        // C++ 585-589.
        let mut param_list: Vec<&'gc Node<'gc>> = Vec::new();
        let rest = self.parse_component_type_parameters_flow(
            Param::default(),
            &mut param_list,
        )?;

        // C++ 591-597.
        let mut renders_type: Option<&'gc Node<'gc>> = None;
        if self.check_name(b"renders") {
            renders_type = Some(self.parse_component_render_type_flow(true)?);
        }

        // C++ 599-603.
        let node = Node::ComponentTypeAnnotation(ComponentTypeAnnotation::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, param_list),
            rest,
            type_params,
            renders_type,
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseComponentTypeParametersFlow — 606 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse the `( ... )` parameter list of a component TYPE annotation into
    /// `param_list`, returning the optional rest parameter (outer `None` =
    /// error already reported, `Some(None)` = no rest parameter). Port of
    /// `JSParserImpl::parseComponentTypeParametersFlow` (flow.cpp:606-648).
    pub(super) fn parse_component_type_parameters_flow(
        &mut self,
        param: Param,
        param_list: &mut Vec<&'gc Node<'gc>>,
    ) -> Option<Option<&'gc Node<'gc>>> {
        // C++ 609-613.
        debug_assert!(self.check(TokenKind::l_paren));
        self.advance(GrammarContext::Type);
        let mut rest: Option<&'gc Node<'gc>> = None;

        // C++ 616-635.
        while !self.check(TokenKind::r_paren) {
            if self.check(TokenKind::dotdotdot) {
                // C++ 617-624: a ComponentTypeRestParameter.
                rest = Some(self.parse_component_type_rest_parameter_flow(param)?);
                break;
            }

            // C++ 626-631.
            let param_node = self.parse_component_type_parameter_flow(param)?;
            param_list.push(param_node);

            // C++ 633-634.
            if !self.check_and_eat(TokenKind::comma, GrammarContext::Type) {
                break;
            }
        }

        // C++ 637-645.
        if !self.eat(
            TokenKind::r_paren,
            GrammarContext::Type,
            " at end of component type parameter list",
        ) {
            return None;
        }

        Some(rest)
    }

    // -----------------------------------------------------------------------
    // parseComponentTypeRestParameterFlow — 650 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a `...IdentifierName: T` / `...T` rest parameter of a component
    /// TYPE annotation, with the cursor at `...`. Port of
    /// `JSParserImpl::parseComponentTypeRestParameterFlow` (flow.cpp:650-698).
    fn parse_component_type_rest_parameter_flow(
        &mut self,
        _param: Param,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 657-659.
        debug_assert!(self.check(TokenKind::dotdotdot));
        let start = self.advance(GrammarContext::Type).start;

        // C++ 661-663.
        let left = self.parse_type_annotation_before_colon_flow()?;

        // C++ 665-689.
        let mut name: Option<&'gc Node<'gc>> = None;
        let type_annotation: &'gc Node<'gc>;
        let mut optional = false;
        if self.check2(TokenKind::colon, TokenKind::question) {
            // C++ 670-686: the node is actually supposed to be an identifier,
            // not a TypeAnnotation.
            name = Some(self.reparse_type_annotation_as_identifier_flow(left)?);
            optional =
                self.check_and_eat(TokenKind::question, GrammarContext::Type);
            if !self.eat(
                TokenKind::colon,
                GrammarContext::Type,
                " in component parameter type annotation",
            ) {
                return None;
            }
            type_annotation =
                self.parse_type_annotation_flow(None, AllowAnonFunctionType::Yes)?;
        } else {
            type_annotation = left;
        }

        // C++ 691.
        self.check_and_eat(TokenKind::comma, GrammarContext::Type);

        // C++ 693-697.
        let node = Node::ComponentTypeParameter(ComponentTypeParameter::new(
            NodeMetadata::new(self.dummy_range()),
            name,
            type_annotation,
            optional,
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseComponentTypeParameterFlow — 700 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse one `Name?: T` parameter of a component TYPE annotation (the name
    /// is a string literal or identifier; `as` is rejected). Port of
    /// `JSParserImpl::parseComponentTypeParameterFlow` (flow.cpp:700-766).
    fn parse_component_type_parameter_flow(
        &mut self,
        _param: Param,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 706.
        let param_start = self.cur_start();
        let name_elem: &'gc Node<'gc>;

        // C++ 708-730.
        if self.check(TokenKind::string_literal) {
            // C++ 708-715.
            let str_range = self.cur_range();
            let value = self.lexer.token().get_string_literal();
            let node = Node::StringLiteral(StringLiteral::new(
                NodeMetadata::new(self.dummy_range()),
                value,
            ));
            name_elem =
                self.set_location(str_range.start, str_range.end, node);
            self.advance(GrammarContext::Type);
        } else if self.check(TokenKind::identifier)
            || self.lexer.token().is_res_word()
        {
            // C++ 716-724.
            let ident_rng = self.cur_range();
            let id = self.lexer.token().get_res_word_or_identifier();
            let node = Node::Identifier(Identifier::new(
                NodeMetadata::new(self.dummy_range()),
                id,
                None,
                false,
            ));
            name_elem =
                self.set_location(ident_rng.start, ident_rng.end, node);
            self.advance(GrammarContext::Type);
        } else {
            // C++ 725-729.
            self.error_at_loc(
                self.cur_start(),
                "identifier or string literal expected in component type parameter name",
            );
            return None;
        }

        // C++ 732-735: `as` is not allowed in component type parameters.
        if self.check_name(b"as") {
            self.error_at_loc(
                self.cur_start(),
                "'as' not allowed in component type parameter",
            );
            return None;
        }

        // C++ 737-743.
        let mut optional = false;
        if self.check_and_eat(TokenKind::question, GrammarContext::Type) {
            optional = true;
        }

        // C++ 745-753.
        if !self.eat(
            TokenKind::colon,
            GrammarContext::Type,
            " in component type parameter",
        ) {
            return None;
        }

        // C++ 755-759: parseTypeAnnotation() with default args
        // (wrappedStart=None, AllowAnonFunctionType::Yes).
        let ty =
            self.parse_type_annotation(None, AllowAnonFunctionType::Yes)?;

        // C++ 761-765.
        let node = Node::ComponentTypeParameter(ComponentTypeParameter::new(
            NodeMetadata::new(self.dummy_range()),
            Some(name_elem),
            ty,
            optional,
        ));
        Some(self.set_location(param_start, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseHookTypeAnnotationFlow — 3816 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a `hook(params) => R` TYPE annotation, with the cursor at `hook`.
    /// Port of `JSParserImpl::parseHookTypeAnnotationFlow` (flow.cpp:3816-3821).
    pub(super) fn parse_hook_type_annotation_flow(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 3818-3819.
        debug_assert!(self.check_name(b"hook"));
        self.advance(GrammarContext::Type);
        // C++ 3820.
        self.parse_function_or_hook_type_annotation_flow(true)
    }

    /// Intern the raw source text of the current token. The Rust equivalent
    /// of the C++ `lexer_.getStringLiteral(tok_->inputStr())` idiom used by
    /// the literal type annotations.
    fn cur_token_source_atom(&self) -> NodeLabel {
        let range = self.lexer.token().source_range();
        self.source_bytes_atom(range.start, range.end)
    }
}
