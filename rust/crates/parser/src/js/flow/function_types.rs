/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Flow function types, return-type annotations (including type
//! predicates), and `%checks` predicates. Port of the corresponding sections
//! of `lib/Parser/JSParserImpl-flow.cpp`.

use hermes_ast::node::{
    DeclaredPredicate, FunctionTypeAnnotation, FunctionTypeParam,
    HookTypeAnnotation, Identifier, InferredPredicate, Node, TypeAnnotation,
    TypePredicate,
};
use hermes_ast::node_child::{NodeList, NodeMetadata};
use hermes_atom_table::INVALID_ATOM_BYTES;
use hermes_support::location::SMLoc;

use crate::js::expressions::inc_parens;
use crate::js::{JSParserImpl, PARAM_IN};
use crate::lexer::GrammarContext;
use crate::token_kinds::TokenKind;

use super::AllowAnonFunctionType;

impl<'gc, 'ast, 'ctx, 'a> JSParserImpl<'gc, 'ast, 'ctx, 'a> {
    // -----------------------------------------------------------------------
    // parseReturnTypeAnnotationFlow — 2883 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a function return type annotation, which may be a plain type or
    /// a type predicate (`asserts x [is T]`, `implies x is T`, `x is T`).
    /// Port of `parseReturnTypeAnnotationFlow` (flow.cpp:2884-3010).
    ///
    /// \param wrapped_start like `parse_type_annotation_flow`'s: if `Some`,
    ///   the result is wrapped in a `TypeAnnotation` node.
    pub(in crate::js) fn parse_return_type_annotation_flow(
        &mut self,
        wrapped_start: Option<SMLoc>,
        allow_anon_function_type: AllowAnonFunctionType,
    ) -> Option<&'gc Node<'gc>> {
        let start = self.cur_start();
        let return_type: &'gc Node<'gc>;
        if self.check_name(b"asserts") {
            // C++ 2888-2924.
            // TypePredicate (asserts = true) or TypeAnnotation:
            //   TypeAnnotation
            //   asserts IdentifierName
            //   asserts IdentifierName is TypeAnnotation
            let opt_type = self
                .parse_type_annotation_flow(None, allow_anon_function_type)?;

            if self.check(TokenKind::identifier) {
                // Validate the "asserts" token was an identifier not a more
                // complex type (C++ 2898-2901; the reparsed node itself is
                // unused).
                self.reparse_type_annotation_as_identifier_flow(opt_type)?;
                // C++ 2902-2907.
                let id_range = self.cur_range();
                let id_node = Node::Identifier(Identifier::new(
                    NodeMetadata::new(self.dummy_range()),
                    self.lexer.token().get_identifier(),
                    None,
                    false,
                ));
                let id = self.set_location(
                    id_range.start,
                    id_range.end,
                    id_node,
                );
                self.advance(GrammarContext::Type);
                // C++ 2908-2916: checkAndEat(isIdent_, Type).
                let mut type_annotation: Option<&'gc Node<'gc>> = None;
                if self.check_name(b"is") {
                    self.advance(GrammarContext::Type);
                    // assert IdentifierName is TypeAnnotation
                    //                          ^
                    type_annotation = Some(self.parse_type_annotation_flow(
                        None,
                        allow_anon_function_type,
                    )?);
                }
                // C++ 2917-2921.
                let node = Node::TypePredicate(TypePredicate::new(
                    NodeMetadata::new(self.dummy_range()),
                    id,
                    type_annotation,
                    self.lexer.get_identifier(b"asserts"),
                ));
                return_type = self.set_location(
                    start,
                    self.lexer.prev_token_end(),
                    node,
                );
            } else {
                return_type = opt_type;
            }
        } else if self.check_name(b"implies") {
            // C++ 2925-2976.
            // TypePredicate (implies = true) or TypeAnnotation:
            //   TypeAnnotation
            //   implies IdentifierName is TypeAnnotation

            //   implies IdentifierName is TypeAnnotation
            //   ^
            let opt_type = self
                .parse_type_annotation_flow(None, allow_anon_function_type)?;

            if self.check2(TokenKind::identifier, TokenKind::rw_this) {
                // Validate the "implies" token was an identifier not a more
                // complex type (C++ 2938-2944).
                let is_bare_generic = matches!(
                    opt_type,
                    Node::GenericTypeAnnotation(generic)
                        if generic.type_parameters.is_none()
                );
                if !is_bare_generic {
                    self.error_at_loc(
                        self.cur_start(),
                        "invalid return annotation. 'implies' type guard needs to be followed by identifier",
                    );
                    return None;
                }

                //   implies IdentifierName is TypeAnnotation
                //           ^
                let id_range = self.cur_range();
                let id_node = Node::Identifier(Identifier::new(
                    NodeMetadata::new(self.dummy_range()),
                    self.lexer.token().get_res_word_or_identifier(),
                    None,
                    false,
                ));
                let id = self.set_location(
                    id_range.start,
                    id_range.end,
                    id_node,
                );
                self.advance(GrammarContext::Type);

                //   implies IdentifierName is TypeAnnotation
                //                          ^
                // C++ 2957-2962: checkAndEat(isIdent_, Type).
                if self.check_name(b"is") {
                    self.advance(GrammarContext::Type);
                } else {
                    self.error_at_loc(
                        self.cur_start(),
                        "expecting 'is' after parameter of 'implies' type guard",
                    );
                    return None;
                }
                //   implies IdentifierName is TypeAnnotation
                //                             ^
                let type_t = self.parse_type_annotation_flow(
                    None,
                    allow_anon_function_type,
                )?;
                // C++ 2968-2972.
                let node = Node::TypePredicate(TypePredicate::new(
                    NodeMetadata::new(self.dummy_range()),
                    id,
                    Some(type_t),
                    self.lexer.get_identifier(b"implies"),
                ));
                return_type = self.set_location(
                    start,
                    self.lexer.prev_token_end(),
                    node,
                );
            } else {
                // implies (as type -- okay)
                return_type = opt_type;
            }
        } else {
            // C++ 2977-2999.
            // TypePredicate (asserts = false && implies = false) or
            // TypeAnnotation:
            //   TypeAnnotation
            //   IdentifierName is TypeAnnotation
            let opt_type = self
                .parse_type_annotation_flow(None, allow_anon_function_type)?;

            // C++ 2986: checkAndEat(isIdent_, Type).
            if self.check_name(b"is") {
                self.advance(GrammarContext::Type);
                let id =
                    self.reparse_type_annotation_as_identifier_flow(opt_type)?;
                let type_annotation = self.parse_type_annotation_flow(
                    None,
                    allow_anon_function_type,
                )?;
                // C++ 2993-2996: the C++ passes a null UniqueString for
                // `kind` on an unprefixed predicate; the dumper emits
                // `"kind": null` — INVALID_ATOM_BYTES is the Rust null
                // NodeString.
                let node = Node::TypePredicate(TypePredicate::new(
                    NodeMetadata::new(self.dummy_range()),
                    id,
                    Some(type_annotation),
                    INVALID_ATOM_BYTES,
                ));
                return_type = self.set_location(
                    start,
                    self.lexer.prev_token_end(),
                    node,
                );
            } else {
                return_type = opt_type;
            }
        }

        // C++ 3002-3008.
        if let Some(wrapped_start) = wrapped_start {
            let node = Node::TypeAnnotation(TypeAnnotation::new(
                NodeMetadata::new(self.dummy_range()),
                return_type,
            ));
            return Some(self.set_location(
                wrapped_start,
                self.lexer.prev_token_end(),
                node,
            ));
        }
        Some(return_type)
    }

    /// Parse the `=> ReturnType` tail of a function type whose parameters
    /// have already been parsed. Port of
    /// `parseFunctionTypeAnnotationWithParamsFlow` (flow.cpp:3866-3898).
    pub(super) fn parse_function_type_annotation_with_params_flow(
        &mut self,
        start: SMLoc,
        params: Vec<&'gc Node<'gc>>,
        this_constraint: Option<&'gc Node<'gc>>,
        rest: Option<&'gc Node<'gc>>,
        type_params: Option<&'gc Node<'gc>>,
        hook: bool,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 3873-3874.
        debug_assert!(self.check(TokenKind::equalgreater));
        self.advance(GrammarContext::Type);

        // C++ 3876: `parseReturnTypeAnnotationFlow()` with its declaration
        // defaults (wrappedStart=None, AllowAnonFunctionType::Yes;
        // JSParserImpl.h:1283-1286).
        let return_type = self
            .parse_return_type_annotation_flow(None, AllowAnonFunctionType::Yes)?;

        // C++ 3880-3896.
        if !hook {
            let node =
                Node::FunctionTypeAnnotation(FunctionTypeAnnotation::new(
                    NodeMetadata::new(self.dummy_range()),
                    NodeList::from_iter(self.gc, params),
                    this_constraint,
                    return_type,
                    rest,
                    type_params,
                ));
            Some(self.set_location(start, self.lexer.prev_token_end(), node))
        } else {
            // C++ 3890-3895: HookTypeAnnotation.
            let node = Node::HookTypeAnnotation(HookTypeAnnotation::new(
                NodeMetadata::new(self.dummy_range()),
                NodeList::from_iter(self.gc, params),
                return_type,
                rest,
                type_params,
            ));
            Some(self.set_location(start, self.lexer.prev_token_end(), node))
        }
    }

    // -----------------------------------------------------------------------
    // parseFunctionTypeAnnotationFlow — 3823 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a (possibly generic) function type annotation
    /// `<T>(params) => R`. Port of `parseFunctionTypeAnnotationFlow`
    /// (flow.cpp:3824-3826).
    pub(super) fn parse_function_type_annotation_flow(&mut self) -> Option<&'gc Node<'gc>> {
        self.parse_function_or_hook_type_annotation_flow(false)
    }

    /// Parse a function (or, P6, hook) type annotation with the current token
    /// at `<` or `(`. Port of `parseFunctionOrHookTypeAnnotationFlow`
    /// (flow.cpp:3828-3864). `hook` is threaded like the C++ bool; the only
    /// P5 caller passes false (`parseHookTypeAnnotationFlow` is P6).
    pub(super) fn parse_function_or_hook_type_annotation_flow(
        &mut self,
        hook: bool,
    ) -> Option<&'gc Node<'gc>> {
        let start = self.cur_start();

        // C++ 3831-3837.
        let mut type_params: Option<&'gc Node<'gc>> = None;
        if self.check(TokenKind::less) {
            type_params = Some(self.parse_type_params_flow()?);
        }

        // C++ 3839-3844.
        if !self.need_at(
            TokenKind::l_paren,
            " in function type annotation",
            Some("start of annotation"),
            start,
        ) {
            return None;
        }

        // C++ 3846-3852.
        let mut params: Vec<&'gc Node<'gc>> = Vec::new();
        let mut this_constraint: Option<&'gc Node<'gc>> = None;
        let rest = self.parse_function_type_annotation_params_flow(
            &mut params,
            &mut this_constraint,
            hook,
        )?;

        // C++ 3854-3859.
        if !self.need_at(
            TokenKind::equalgreater,
            " in function type annotation",
            Some("start of annotation"),
            start,
        ) {
            return None;
        }

        // C++ 3861-3862.
        self.parse_function_type_annotation_with_params_flow(
            start,
            params,
            this_constraint,
            rest,
            type_params,
            hook,
        )
    }

    // -----------------------------------------------------------------------
    // parseFunctionOrGroupTypeAnnotationFlow — 3899 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a parenthesized group type `(T)` or a parenthesized function
    /// type `(params) => R`, with the current token at `(`. Port of
    /// `parseFunctionOrGroupTypeAnnotationFlow` (flow.cpp:3900-4033).
    pub(super) fn parse_function_or_group_type_annotation_flow(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        debug_assert!(self.check(TokenKind::l_paren));
        // This is either
        // ( Type )
        // ^
        // or
        // ( ParamList ) => Type
        // ^
        // so we use a similar approach to arrow function parameters by
        // keeping track and reparsing in certain cases.
        let start = self.advance(GrammarContext::Type).start;

        let mut is_function = false;
        let mut ty: Option<&'gc Node<'gc>> = None;
        let mut rest: Option<&'gc Node<'gc>> = None;
        let mut params: Vec<&'gc Node<'gc>> = Vec::new();
        let mut this_constraint: Option<&'gc Node<'gc>> = None;

        // C++ 3918-3937: a leading `this: T` constraint.
        if self.check(TokenKind::rw_this) {
            let opt_next = self.lexer.lookahead1::<true>(None);
            if opt_next == Some(TokenKind::colon) {
                let this_start = self.advance(GrammarContext::Type).start;
                self.advance(GrammarContext::Type);
                let type_annotation = self.parse_type_annotation_flow(
                    None,
                    AllowAnonFunctionType::Yes,
                )?;

                let ftp_node = Node::FunctionTypeParam(FunctionTypeParam::new(
                    NodeMetadata::new(self.dummy_range()),
                    None, // name
                    type_annotation,
                    false, // optional
                ));
                this_constraint = Some(self.set_location(
                    this_start,
                    self.lexer.prev_token_end(),
                    ftp_node,
                ));
                self.check_and_eat(TokenKind::comma, GrammarContext::Type);
            } else if opt_next == Some(TokenKind::question) {
                self.error_cur("'this' constraint may not be optional");
            }
        }

        // C++ 3939-3965.
        if self.allow_anon_function_type.get()
            && self.check_and_eat(TokenKind::dotdotdot, GrammarContext::Type)
        {
            is_function = true;
            // Must be parameters, and this must be the last one.
            // Rest param must be the last param.
            rest = Some(self.parse_function_type_annotation_param_flow()?);
        } else if self.check(TokenKind::r_paren) {
            is_function = true;
            // ( )
            //   ^
            // No parameters, but this must be an empty param list.
        } else {
            // Not sure yet whether this is a param or simply a type.
            let param = self.parse_function_type_annotation_param_flow()?;
            let ftp = param
                .as_function_type_param()
                .expect("param parser returns FunctionTypeParam");
            ty = Some(ftp.type_annotation);
            if ftp.name.is_some() || ftp.optional.get() {
                // Must be a param if it has a name or if it was optional.
                is_function = true;
            }
            params.push(param);
        }

        // If isFunction was already forced by something previously then we
        // have no choice but to attempt to parse as a function type
        // annotation. C++ 3969-3990.
        if (is_function || self.allow_anon_function_type.get())
            && self.check_and_eat(TokenKind::comma, GrammarContext::Type)
        {
            is_function = true;
            while !self.check(TokenKind::r_paren) {
                let is_rest = rest.is_none()
                    && self.check_and_eat(
                        TokenKind::dotdotdot,
                        GrammarContext::Type,
                    );

                let param = self.parse_function_type_annotation_param_flow()?;
                if is_rest {
                    rest = Some(param);
                    self.check_and_eat(TokenKind::comma, GrammarContext::Type);
                    break;
                } else {
                    params.push(param);
                }

                if !self.check_and_eat(TokenKind::comma, GrammarContext::Type)
                {
                    break;
                }
            }
        }

        // C++ 3992-3998.
        if !self.eat_at(
            TokenKind::r_paren,
            GrammarContext::Type,
            " at end of function annotation parameters",
            Some("start of parameters"),
            start,
        ) {
            return None;
        }

        // C++ 4000-4012.
        if is_function {
            if !self.eat_at(
                TokenKind::equalgreater,
                GrammarContext::Type,
                " in function type annotation",
                Some("start of function"),
                start,
            ) {
                return None;
            }
        } else if self.allow_anon_function_type.get()
            && self.check_and_eat(TokenKind::equalgreater, GrammarContext::Type)
        {
            is_function = true;
        }

        // C++ 4014-4017: a plain parenthesized group — return the inner type
        // with its paren count bumped.
        if !is_function {
            let ty =
                ty.expect("non-function group type must have an inner type");
            inc_parens(ty);
            return Some(ty);
        }

        // C++ 4019-4024.
        let return_type = self.parse_return_type_annotation_flow(
            None,
            if self.allow_anon_function_type.get() {
                AllowAnonFunctionType::Yes
            } else {
                AllowAnonFunctionType::No
            },
        )?;

        // C++ 4026-4031: a function type reached through the group cover
        // never has type parameters.
        let node = Node::FunctionTypeAnnotation(FunctionTypeAnnotation::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, params),
            this_constraint,
            return_type,
            rest,
            None, // typeParams
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseMethodishTypeAnnotationFlow — 4848 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a method-ish type annotation `(params): R` starting at the
    /// current `(` (used by object-type methods, accessors, call properties,
    /// and internal slots — the return type follows a `:`, not `=>`). Returns
    /// a `FunctionTypeAnnotation` node. Port of
    /// `parseMethodishTypeAnnotationFlow` (flow.cpp:4849-4880).
    pub(super) fn parse_methodish_type_annotation_flow(
        &mut self,
        start: SMLoc,
        type_params: Option<&'gc Node<'gc>>,
    ) -> Option<&'gc Node<'gc>> {
        let mut params: Vec<&'gc Node<'gc>> = Vec::new();
        let mut this_constraint: Option<&'gc Node<'gc>> = None;

        // C++ 4855-4860.
        if !self.need(TokenKind::l_paren, " at start of parameters") {
            return None;
        }
        let rest = self.parse_function_type_annotation_params_flow(
            &mut params,
            &mut this_constraint,
            false, // hook
        )?;

        // C++ 4862-4868.
        if !self.eat_at(
            TokenKind::colon,
            GrammarContext::Type,
            " in function type annotation",
            Some("start of annotation"),
            start,
        ) {
            return None;
        }

        // C++ 4870: `parseReturnTypeAnnotationFlow()` declaration defaults.
        let return_type = self
            .parse_return_type_annotation_flow(None, AllowAnonFunctionType::Yes)?;

        // C++ 4874-4878.
        let node = Node::FunctionTypeAnnotation(FunctionTypeAnnotation::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, params),
            this_constraint,
            return_type,
            rest,
            type_params,
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseFunctionTypeAnnotationParamsFlow — 4881 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse the parenthesized parameter list of a function type, with the
    /// current token at `(`. Parameters are appended to `params`, an optional
    /// leading `this: T` constraint is stored in `this_constraint`, and the
    /// optional rest parameter is returned (the C++ returns
    /// `Optional<FunctionTypeParamNode*>` — outer `None` here means an error
    /// was reported, inner `None` means no rest parameter). Port of
    /// `parseFunctionTypeAnnotationParamsFlow` (flow.cpp:4882-4945).
    pub(super) fn parse_function_type_annotation_params_flow(
        &mut self,
        params: &mut Vec<&'gc Node<'gc>>,
        this_constraint: &mut Option<&'gc Node<'gc>>,
        hook: bool,
    ) -> Option<Option<&'gc Node<'gc>>> {
        debug_assert!(self.check(TokenKind::l_paren));
        // C++ 4887.
        let start = self.advance(GrammarContext::Type).start;

        let mut rest: Option<&'gc Node<'gc>> = None;
        *this_constraint = None;

        // C++ 4892-4911: a leading `this: T` constraint.
        if self.check(TokenKind::rw_this) && !hook {
            let opt_next = self.lexer.lookahead1::<true>(None);
            if opt_next == Some(TokenKind::colon) {
                let this_start = self.advance(GrammarContext::Type).start;
                self.advance(GrammarContext::Type);
                let type_annotation = self.parse_type_annotation_flow(
                    None,
                    AllowAnonFunctionType::Yes,
                )?;

                let ftp_node = Node::FunctionTypeParam(FunctionTypeParam::new(
                    NodeMetadata::new(self.dummy_range()),
                    None, // name
                    type_annotation,
                    false, // optional
                ));
                *this_constraint = Some(self.set_location(
                    this_start,
                    self.lexer.prev_token_end(),
                    ftp_node,
                ));
                self.check_and_eat(TokenKind::comma, GrammarContext::Type);
            } else if opt_next == Some(TokenKind::question) {
                self.error_cur("'this' constraint may not be optional");
            }
        }

        // C++ 4913-4933.
        while !self.check(TokenKind::r_paren) {
            let is_rest =
                self.check_and_eat(TokenKind::dotdotdot, GrammarContext::Type);

            // C++ 4917-4918.
            let param = if hook {
                self.parse_hook_type_annotation_param_flow()?
            } else {
                self.parse_function_type_annotation_param_flow()?
            };

            if is_rest {
                // Rest param must be the last param.
                rest = Some(param);
                self.check_and_eat(TokenKind::comma, GrammarContext::Type);
                break;
            } else {
                params.push(param);
                if !self.check_and_eat(TokenKind::comma, GrammarContext::Type)
                {
                    break;
                }
            }
        }

        // C++ 4935-4941.
        if !self.eat_at(
            TokenKind::r_paren,
            GrammarContext::Type,
            " at end of function annotation parameters",
            Some("start of parameters"),
            start,
        ) {
            return None;
        }

        Some(rest)
    }

    // -----------------------------------------------------------------------
    // parseHookTypeAnnotationParamFlow — 4946 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse one hook-type parameter. Identical to a function-type parameter
    /// except that a `this` constraint is rejected. Port of
    /// `JSParserImpl::parseHookTypeAnnotationParamFlow` (flow.cpp:4947-4956).
    fn parse_hook_type_annotation_param_flow(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 4948-4953.
        if self.check(TokenKind::rw_this)
            && self.lexer.lookahead1::<true>(None) == Some(TokenKind::colon)
        {
            self.error_at(
                self.cur_range(),
                "hooks do not support 'this' constraints",
            );
        }
        // C++ 4954.
        self.parse_function_type_annotation_param_flow()
    }

    /// Parse one function-type parameter, which is either a bare type or a
    /// named `name[?]: T`. Port of `parseFunctionTypeAnnotationParamFlow`
    /// (flow.cpp:4958-5006).
    fn parse_function_type_annotation_param_flow(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        let start = self.cur_start();

        // C++ 4961-4968.
        if self.check(TokenKind::rw_this) {
            let opt_next = self.lexer.lookahead1::<true>(None);
            if opt_next == Some(TokenKind::colon) {
                self.error_cur("'this' constraint must be the first parameter");
            }
        }

        // C++ 4970-4972.
        let left = self.parse_type_annotation_before_colon_flow()?;

        let mut name: Option<&'gc Node<'gc>> = None;
        let type_annotation: &'gc Node<'gc>;
        let mut optional = false;

        // C++ 4978-4998.
        if self.check2(TokenKind::colon, TokenKind::question) {
            // The node is actually supposed to be an identifier, not a
            // TypeAnnotation.
            name = Some(self.reparse_type_annotation_as_identifier_flow(left)?);
            optional =
                self.check_and_eat(TokenKind::question, GrammarContext::Type);
            if !self.eat_at(
                TokenKind::colon,
                GrammarContext::Type,
                " in function parameter type annotation",
                Some("start of parameter"),
                start,
            ) {
                return None;
            }
            type_annotation = self
                .parse_type_annotation_flow(None, AllowAnonFunctionType::Yes)?;
        } else {
            type_annotation = left;
        }

        // C++ 5000-5004.
        let node = Node::FunctionTypeParam(FunctionTypeParam::new(
            NodeMetadata::new(self.dummy_range()),
            name,
            type_annotation,
            optional,
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parsePredicateFlow — 5078 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a `%checks` / `%checks(expr)` predicate, with the current token
    /// at the `%checks` identifier (lexed as a single identifier in Type
    /// grammar context). Port of `parsePredicateFlow` (flow.cpp:5079-5099).
    pub(in crate::js) fn parse_predicate_flow(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        debug_assert!(self.check_name(b"%checks"));
        // C++ 5080.
        let checks_rng = self.advance(GrammarContext::Type);
        // C++ 5081: `checkAndEat(l_paren)` with GrammarContext::AllowRegExp —
        // deliberate; what follows is a JS expression, not a type.
        if self.check_and_eat(TokenKind::l_paren, GrammarContext::AllowRegExp) {
            // C++ 5082: `parseConditionalExpression()` with its declaration
            // defaults (ParamIn, CoverTypedParameters::Yes).
            let cond = self.parse_conditional_expression(
                PARAM_IN,
                crate::js::flow::CoverTypedParameters::Yes,
            )?;
            // C++ 5085-5092.
            let end = self.cur_range().end;
            if !self.eat_at(
                TokenKind::r_paren,
                GrammarContext::Type,
                " in declared predicate",
                Some("start of predicate"),
                checks_rng.start,
            ) {
                return None;
            }
            // C++ 5093-5094.
            let node = Node::DeclaredPredicate(DeclaredPredicate::new(
                NodeMetadata::new(self.dummy_range()),
                cond,
            ));
            return Some(self.set_location(checks_rng.start, end, node));
        }
        // C++ 5096-5097: the InferredPredicate spans the `%checks` token.
        let node = Node::InferredPredicate(InferredPredicate::new(
            NodeMetadata::new(self.dummy_range()),
        ));
        Some(self.set_location(checks_rng.start, checks_rng.end, node))
    }
}
