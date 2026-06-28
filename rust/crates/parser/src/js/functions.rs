/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Function declaration/expression parsing for the JS parser. Port of the
//! function-parsing section of `lib/Parser/JSParserImpl.cpp`
//! (`parseFunctionHelper`, `parseFormalParameters`, `parseFunctionBody`,
//! `parseFunctionDeclaration`, `parseFunctionExpression`).
//!
//! Full-pass / eager port only: there is no PreParse/LazyParse machinery, so
//! the `pass_ == PreParse`/`pass_ == LazyParse` blocks are omitted (see the
//! individual comments). The Flow signature sites (type parameters, return
//! type, `%checks` predicate, leading `this` parameter) are ported (P5.4);
//! the TS blocks are omitted (P7).

use ast::node::{FunctionDeclaration, FunctionExpression, Identifier, Node};
use ast::node_child::{NodeList, NodeMetadata};

use crate::lexer::GrammarContext;
use crate::token_kinds::TokenKind;

use super::flow::AllowAnonFunctionType;
use super::{JSParserImpl, Param, PARAM_DEFAULT, PARAM_RETURN};

impl<'gc, 'ast, 'ctx, 'a> JSParserImpl<'gc, 'ast, 'ctx, 'a> {
    // -----------------------------------------------------------------------
    // parseFunctionHelper — 383 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a function declaration or expression (including generators and
    /// async functions). Port of `JSParserImpl::parseFunctionHelper`
    /// (lines 383-598).
    ///
    /// `force_eagerly` is threaded for fidelity (it feeds the `eagerly` arg of
    /// `parse_function_body`); in the Full-pass port the body is always parsed
    /// eagerly, so it has no observable effect.
    ///
    /// The C++ constructs a `SaveFunctionState` whose destructor restores
    /// `strictMode` (383/510). A `"use strict"` directive in the body must NOT
    /// leak strictness to the enclosing (possibly sloppy) code, so we save and
    /// restore the lexer strict-mode flag around the body. The result is
    /// computed first so the restore runs on every (including error `?`) path.
    pub(super) fn parse_function_helper(
        &mut self,
        param: Param,
        is_declaration: bool,
        force_eagerly: bool,
    ) -> Option<&'gc Node<'gc>> {
        let old_strict = self.lexer.is_strict_mode();
        // SaveFunctionState guard — mirrors C++ SaveFunctionState (cpp:510).
        // is_arrow=false: regular function resets the arrow-bookkeeping flags.
        let _g = self.save_function_state(false);
        let old_seen_len = self.seen_directives.len();
        let result =
            self.parse_function_helper_inner(param, is_declaration, force_eagerly);
        self.seen_directives.truncate(old_seen_len);
        self.lexer.set_strict_mode(old_strict);
        result
    }

    fn parse_function_helper_inner(
        &mut self,
        param: Param,
        is_declaration: bool,
        force_eagerly: bool,
    ) -> Option<&'gc Node<'gc>> {
        // function or async function
        // C++ 387-389.
        debug_assert!(
            self.check(TokenKind::rw_function)
                || self.check_unescaped_name(b"async"),
            "parseFunctionHelper must start with 'function' or 'async'"
        );
        let is_async = self.check_unescaped_name(b"async");

        let start_loc = self.advance(GrammarContext::AllowRegExp).start;

        if is_async {
            // async function
            //       ^
            // C++ 393-397.
            self.advance(GrammarContext::AllowRegExp);
        }

        let is_generator =
            self.check_and_eat(TokenKind::star, GrammarContext::AllowRegExp);

        // newParamYield setting per the grammar (C++ 401-413):
        // FunctionDeclaration: BindingIdentifier[?Yield, ?Await]
        // FunctionExpression: BindingIdentifier[~Yield, ~Await]
        // GeneratorFunctionDeclaration: BindingIdentifier[?Yield, ?Await]
        // GeneratorFunctionExpression: BindingIdentifier[+Yield, ~Await]
        // AsyncFunctionDeclaration: BindingIdentifier[?Yield, ?Await]
        // AsyncFunctionExpression: BindingIdentifier[+Yield, +Await]
        // AsyncGeneratorDeclaration: BindingIdentifier[?Yield, ?Await]
        // AsyncGeneratorExpression: BindingIdentifier[+Yield, +Await]
        let name_param_yield = if is_declaration {
            self.param_yield.get()
        } else {
            is_generator
        };
        let name_param_await = if is_declaration {
            self.param_await.get()
        } else {
            is_async
        };
        // RAII guards: restore on every exit path (incl. `?` early-returns),
        // mirroring `llvh::SaveAndRestore<bool>`. Held until end of function.
        let _save_name_param_yield = self.save_param_yield(name_param_yield);
        let _save_name_param_await = self.save_param_await(name_param_await);

        // identifier. C++ 415-416.
        let opt_id = self.parse_binding_identifier(Param::default());
        // If this is a default function declaration, then we can match
        // [+Default] function ( FormalParameters ) { FunctionBody }
        // so the identifier is optional and we can make it nullptr.
        // C++ 417-427.
        if is_declaration && !param.has(PARAM_DEFAULT) && opt_id.is_none() {
            // (errorExpected note args dropped per house style.)
            self.error_cur("'identifier' expected after 'function'");
            return None;
        }

        // Flow type parameters after the name. C++ 429-438.
        let mut type_parameters: Option<&'gc Node<'gc>> = None;
        if self.parse_flow() && self.check(TokenKind::less) {
            type_parameters = Some(self.parse_type_params_flow()?);
        }
        // TS type parameters after the name. C++ 440-447.
        if self.parse_ts() && self.check(TokenKind::less) {
            type_parameters = Some(self.parse_ts_type_parameters()?);
        }

        // (
        // C++ 449-457.
        if !self.need(TokenKind::l_paren, " at start of function parameter list") {
            return None;
        }

        let mut param_list: Vec<&'gc Node<'gc>> = Vec::new();

        // The params and body are parsed with paramYield/paramAwait set from the
        // generator/async-ness of THIS function. RAII guards restore on exit.
        // C++ 461-463.
        let _save_args_yield = self.save_param_yield(is_generator);
        let _save_args_await = self.save_param_await(is_async);

        if !self.parse_formal_parameters(param, &mut param_list) {
            return None;
        }

        // Flow return type and/or `%checks` predicate. C++ 468-487.
        let mut return_type: Option<&'gc Node<'gc>> = None;
        let mut predicate: Option<&'gc Node<'gc>> = None;
        if self.parse_flow() && self.check(TokenKind::colon) {
            let annot_start = self.advance(GrammarContext::Type).start;
            if !self.check_name(b"%checks") {
                return_type = Some(self.parse_return_type_annotation_flow(
                    Some(annot_start),
                    AllowAnonFunctionType::Yes,
                )?);
            }

            if self.check_name(b"%checks") {
                predicate = Some(self.parse_predicate_flow()?);
            }
        }
        // TS return type. C++ 488-498.
        if self.parse_ts() && self.check(TokenKind::colon) {
            let annot_start = self.advance(GrammarContext::Type).start;
            if !self.check_name(b"%checks") {
                return_type = Some(self.parse_type_annotation_ts(Some(annot_start))?);
            }
        }

        // {
        // C++ 500-508.
        if !self.need(
            TokenKind::l_brace,
            if is_declaration {
                " in function declaration"
            } else {
                " in function expression"
            },
        ) {
            return None;
        }

        // SaveFunctionState: lazy-compile bookkeeping, not modeled in the
        // Full-pass port. (C++ 510.)

        // Grammar context to be used when lexing the closing brace. C++ 512-514.
        let grammar_context = if is_declaration {
            GrammarContext::AllowRegExp
        } else {
            GrammarContext::AllowDiv
        };

        // Full-pass only: the C++ PreParse path (516-560) is not ported (no
        // lazy-compile pass yet); port only the eager tail (562-597).
        //
        // The body's paramYield/paramAwait are the args+body values
        // (is_generator/is_async) — in the Full-pass port these are inert, but
        // we pass them for fidelity. C++ `saveArgsAndBodyParamYield.get()`
        // returns the NEW value, i.e. is_generator/is_async.
        let body = self.parse_function_body(
            Param::default(),
            force_eagerly,
            is_generator,
            is_async,
            grammar_context,
            /* parse_directives= */ true,
        )?;

        let body_end = body.range().end;
        let params = NodeList::from_iter(self.gc, param_list);

        let node = if is_declaration {
            Node::FunctionDeclaration(FunctionDeclaration::new(
                NodeMetadata::new(self.dummy_range()),
                opt_id,
                params,
                body,
                type_parameters,
                return_type,
                predicate,
                is_generator,
                is_async,
            ))
        } else {
            Node::FunctionExpression(FunctionExpression::new(
                NodeMetadata::new(self.dummy_range()),
                opt_id,
                params,
                body,
                type_parameters,
                return_type,
                predicate,
                is_generator,
                is_async,
            ))
        };
        Some(self.set_location(start_loc, body_end, node))
    }

    // -----------------------------------------------------------------------
    // parseFormalParameters — 600 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a parenthesized formal parameter list, appending each parameter
    /// node to `param_list`. Port of `JSParserImpl::parseFormalParameters`
    /// (lines 600-667). Returns false on error.
    pub(super) fn parse_formal_parameters(
        &mut self,
        param: Param,
        param_list: &mut Vec<&'gc Node<'gc>>,
    ) -> bool {
        debug_assert!(
            self.check(TokenKind::l_paren),
            "FormalParameters must start with '('"
        );
        // (
        self.advance(GrammarContext::AllowRegExp);

        // The first parameter can be 'this' in Flow and TypeScript.
        // C++ 607-633.
        if self.parse_types() && self.check(TokenKind::rw_this) {
            let name = self.lexer.token().get_res_word_or_identifier();
            let this_param_start = self.advance(GrammarContext::AllowRegExp).start;

            let annot_start = self.cur_start();
            if !self.eat(
                TokenKind::colon,
                GrammarContext::Type,
                " in 'this' type annotation",
            ) {
                // (eat note args "start of 'this'" dropped per house style.)
                return false;
            }

            let Some(type_annotation) = self.parse_type_annotation(
                Some(annot_start),
                AllowAnonFunctionType::Yes,
            ) else {
                return false;
            };
            let node = Node::Identifier(Identifier::new(
                NodeMetadata::new(self.dummy_range()),
                name,
                Some(type_annotation),
                false,
            ));
            let end = self.lexer.prev_token_end();
            param_list.push(self.set_location(this_param_start, end, node));

            self.check_and_eat(TokenKind::comma, GrammarContext::AllowRegExp);
        }

        // C++ 635-654.
        while !self.check(TokenKind::r_paren) {
            if self.check(TokenKind::dotdotdot) {
                // BindingRestElement.
                match self.parse_binding_rest_element(param) {
                    Some(rest) => param_list.push(rest),
                    None => return false,
                }
                break;
            }

            // BindingElement.
            match self.parse_binding_element(param) {
                Some(elem) => param_list.push(elem),
                None => return false,
            }

            if !self.check_and_eat(TokenKind::comma, GrammarContext::AllowRegExp)
            {
                break;
            }
        }

        // )
        // C++ 656-664.
        self.eat(
            TokenKind::r_paren,
            GrammarContext::AllowRegExp,
            " at end of function parameter list",
        )
    }

    // -----------------------------------------------------------------------
    // parseFunctionBody — 740 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a function body (a brace-enclosed block in the `[Return]` context).
    /// Port of `JSParserImpl::parseFunctionBody` (lines 740-813).
    ///
    /// Full-pass / eager port only: the `pass_ == LazyParse && !eagerly` block
    /// (747-797) and the `pass_ == PreParse` store (803-810) are omitted; only
    /// the eager `parseBlock(ParamReturn, ...)` tail (799-801) is ported. The
    /// `eagerly`/`param_yield`/`param_await` params are threaded for fidelity
    /// (callers pass them) but in the Full-pass port the body is always parsed
    /// eagerly, so they have no observable effect.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn parse_function_body(
        &mut self,
        _param: Param,
        _eagerly: bool,
        _param_yield: bool,
        _param_await: bool,
        grammar_context: GrammarContext,
        parse_directives: bool,
    ) -> Option<&'gc Node<'gc>> {
        self.parse_block(PARAM_RETURN, grammar_context, parse_directives)
    }

    // -----------------------------------------------------------------------
    // parseFunctionDeclaration — JSParserImpl.h ~717 (thin wrapper)
    // -----------------------------------------------------------------------

    /// Parse a function declaration. Port of the thin wrapper
    /// `JSParserImpl::parseFunctionDeclaration` which calls
    /// `parseFunctionHelper(param, /*isDeclaration*/ true,
    /// /*forceEagerly*/ false)`.
    pub(super) fn parse_function_declaration(
        &mut self,
        param: Param,
    ) -> Option<&'gc Node<'gc>> {
        self.parse_function_helper(param, /* is_declaration= */ true, false)
    }

    // -----------------------------------------------------------------------
    // parseFunctionExpression — JSParserImpl.cpp 3417 (thin wrapper)
    // -----------------------------------------------------------------------

    /// Parse a function expression. Port of the thin wrapper
    /// `JSParserImpl::parseFunctionExpression(forceEagerly=false)` which calls
    /// `parseFunctionHelper(Param{}, /*isDeclaration*/ false, forceEagerly)`.
    pub(super) fn parse_function_expression(&mut self) -> Option<&'gc Node<'gc>> {
        self.parse_function_helper(
            Param::default(),
            /* is_declaration= */ false,
            false,
        )
    }
}
