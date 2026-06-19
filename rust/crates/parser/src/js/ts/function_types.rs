/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! TypeScript function/constructor/parenthesized types. Port of the function
//! type entry points of `lib/Parser/JSParserImpl-ts.cpp`.
//!
//! P7.2 fills in the `(`-cover algorithm
//! (`parseTSFunctionOrParenthesizedType`) that disambiguates a parenthesized
//! type `( Type )` from a function/constructor type `( ParamList ) => Type`,
//! along with the function-type parameter list
//! (`parseTSFunctionTypeParams`) and the per-parameter parser
//! (`parseTSFunctionTypeParam`) which also handles TS parameter-property
//! modifiers (`readonly`/`public`/`private`/`protected`/`static`/`export`).

use ast::node::{
    Identifier, Node, RestElement, TSConstructorType, TSFunctionType,
    TSParameterProperty,
};
use ast::node_child::{NodeList, NodeMetadata};
use atom_table::INVALID_ATOM_BYTES;
use support::location::SMLoc;

use crate::js::expressions::inc_parens;
use crate::js::ts::IsConstructorType;
use crate::js::{JSParserImpl, Param};
use crate::lexer::GrammarContext;
use crate::token_kinds::TokenKind;

impl<'gc, 'ast, 'ctx, 'a> JSParserImpl<'gc, 'ast, 'ctx, 'a> {
    // -----------------------------------------------------------------------
    // parseTSFunctionOrParenthesizedType — 223 in JSParserImpl-ts.cpp
    // -----------------------------------------------------------------------

    /// Parse either a parenthesized type `( Type )` or a function/constructor
    /// type `( ParamList ) => Type` with the current token at `(`. The cover
    /// keeps track of whether the contents must be a function and reparses an
    /// identifier as a type when it turns out to be a plain group. Port of
    /// `JSParserImpl::parseTSFunctionOrParenthesizedType` (ts.cpp:223-389).
    pub(super) fn parse_ts_function_or_parenthesized_type(
        &mut self,
        start: SMLoc,
        type_params: Option<&'gc Node<'gc>>,
        is_constructor_type: IsConstructorType,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 227.
        debug_assert!(self.check(TokenKind::l_paren));
        // This is either
        // ( Type )
        // ^
        // or
        // ( ParamList ) => Type
        // ^
        // so we use a similar approach to arrow function parameters by keeping
        // track and reparsing in certain cases.
        // C++ 236.
        self.advance(GrammarContext::Type);

        // C++ 238-241.
        let mut is_function = type_params.is_some();
        let mut has_rest = false;
        let mut ty: Option<&'gc Node<'gc>> = None;
        let mut params: Vec<&'gc Node<'gc>> = Vec::new();

        // C++ 243-263: a leading `this:`/`this?:` param.
        if self.check(TokenKind::rw_this) {
            // C++ 244: lookahead1(None) — default RequireNoNewLine=true.
            let opt_next = self.lexer.lookahead1::<true>(None);
            if opt_next == Some(TokenKind::colon) {
                // C++ 246-258.
                let this_start = self.advance(GrammarContext::Type).start;
                self.advance(GrammarContext::Type);
                let _recursion = self.check_recursion()?;
                let type_annotation = self.parse_type_annotation_ts(None)?;

                let id_node = Node::Identifier(Identifier::new(
                    NodeMetadata::new(self.dummy_range()),
                    self.lexer.get_identifier(b"this"),
                    Some(type_annotation),
                    false,
                ));
                params.push(self.set_location(
                    this_start,
                    self.lexer.prev_token_end(),
                    id_node,
                ));
                self.check_and_eat(TokenKind::comma, GrammarContext::Type);
            } else if opt_next == Some(TokenKind::question) {
                // C++ 259-261.
                self.error_cur("'this' constraint may not be optional");
                return None;
            }
        }

        // C++ 265-315.
        if self.allow_anon_function_type.get()
            && self.check_and_eat(TokenKind::dotdotdot, GrammarContext::Type)
        {
            // C++ 265-276.
            is_function = true;
            has_rest = true;
            // Must be parameters, and this must be the last one.
            let name = self.parse_ts_function_type_param()?;
            let node = Node::RestElement(RestElement::new(
                NodeMetadata::new(self.dummy_range()),
                name,
            ));
            params.push(self.set_location(
                start,
                self.lexer.prev_token_end(),
                node,
            ));
        } else if self.check(TokenKind::l_paren) {
            // C++ 277-281.
            ty = Some(self.parse_type_annotation_ts(None)?);
        } else if self.check(TokenKind::r_paren) {
            // C++ 282-286.
            is_function = true;
            // ( )
            //   ^
            // No parameters, but this must be an empty param list.
        } else {
            // C++ 287-315: not sure yet whether this is a param or a type.
            let param = self.parse_ts_function_type_param()?;
            match param {
                Node::TSParameterProperty(pp) => {
                    // C++ 294-300.
                    if pp.accessibility.get() != INVALID_ATOM_BYTES
                        || pp.export.get()
                        || pp.readonly.get()
                        || pp.r#static.get()
                    {
                        // Must be a param.
                        is_function = true;
                    }
                    params.push(param);
                }
                Node::Identifier(ident) => {
                    // C++ 301-310.
                    params.push(param);
                    ty = Some(if let Some(ta) = ident.type_annotation {
                        ta
                    } else {
                        self.reparse_identifier_as_ts_type_annotation(param)
                    });
                    if ident.type_annotation.is_some() || ident.optional.get() {
                        // Must be a param.
                        is_function = true;
                    }
                }
                _ => {
                    // C++ 311-314.
                    ty = Some(param);
                    params.push(param);
                }
            }
        }

        // If isFunction was already forced by something previously then we
        // have no choice but to attempt to parse as a function type
        // annotation. C++ 319-343.
        if (is_function || self.allow_anon_function_type.get())
            && self.check_and_eat(TokenKind::comma, GrammarContext::Type)
        {
            is_function = true;
            while !self.check(TokenKind::r_paren) {
                // C++ 323-324.
                let is_rest = !has_rest
                    && self.check_and_eat(
                        TokenKind::dotdotdot,
                        GrammarContext::Type,
                    );

                let param = self.parse_ts_function_type_param()?;
                if is_rest {
                    // C++ 329-335.
                    let node = Node::RestElement(RestElement::new(
                        NodeMetadata::new(self.dummy_range()),
                        param,
                    ));
                    params.push(self.set_location(
                        start,
                        self.lexer.prev_token_end(),
                        node,
                    ));
                    self.check_and_eat(TokenKind::comma, GrammarContext::Type);
                    break;
                } else {
                    params.push(param);
                }

                if !self.check_and_eat(TokenKind::comma, GrammarContext::Type) {
                    break;
                }
            }
        }

        // C++ 345-351.
        if !self.eat(
            TokenKind::r_paren,
            GrammarContext::Type,
            " at end of function type parameters",
        ) {
            return None;
        }

        // C++ 353-365.
        if is_function {
            if !self.eat(
                TokenKind::equalgreater,
                GrammarContext::Type,
                " in function type",
            ) {
                return None;
            }
        } else if self.allow_anon_function_type.get()
            && self.check_and_eat(TokenKind::equalgreater, GrammarContext::Type)
        {
            is_function = true;
        }

        // C++ 367-370: a plain parenthesized group — return the inner type with
        // its paren count bumped.
        if !is_function {
            let ty =
                ty.expect("non-function parenthesized type must have a type");
            inc_parens(ty);
            return Some(ty);
        }

        // C++ 372-374.
        let return_type = self.parse_type_annotation_ts(None)?;

        // C++ 376-388.
        if is_constructor_type == IsConstructorType::Yes {
            let node = Node::TSConstructorType(TSConstructorType::new(
                NodeMetadata::new(self.dummy_range()),
                NodeList::from_iter(self.gc, params),
                return_type,
                type_params,
            ));
            return Some(self.set_location(
                start,
                self.lexer.prev_token_end(),
                node,
            ));
        }

        let node = Node::TSFunctionType(TSFunctionType::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, params),
            return_type,
            type_params,
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseTSFunctionTypeParams — 391 in JSParserImpl-ts.cpp
    // -----------------------------------------------------------------------

    /// Parse the parenthesized parameter list of a function type, with the
    /// current token at `(`. Parameters are appended to `params`. Returns
    /// `true` on success, `false` if an error was reported. Port of
    /// `JSParserImpl::parseTSFunctionTypeParams` (ts.cpp:391-417). Used by the
    /// object-type call/method signatures (P7.3).
    #[allow(dead_code)] // Consumed by the object-type signatures (P7.3).
    pub(super) fn parse_ts_function_type_params(
        &mut self,
        _start: SMLoc,
        params: &mut Vec<&'gc Node<'gc>>,
    ) -> bool {
        // C++ 394.
        debug_assert!(self.check(TokenKind::l_paren));

        // C++ 396.
        self.advance(GrammarContext::Type);

        // C++ 398-406.
        while !self.check(TokenKind::r_paren) {
            match self.parse_ts_function_type_param() {
                Some(param) => params.push(param),
                None => return false,
            }

            if !self.check_and_eat(TokenKind::comma, GrammarContext::Type) {
                break;
            }
        }

        // C++ 408-414.
        if !self.eat(
            TokenKind::r_paren,
            GrammarContext::Type,
            " at end of function type parameters",
        ) {
            return false;
        }

        true
    }

    // -----------------------------------------------------------------------
    // parseTSFunctionTypeParam — 419 in JSParserImpl-ts.cpp
    // -----------------------------------------------------------------------

    /// Parse one function-type parameter, which is either a plain binding
    /// element or a binding element preceded by TS parameter-property modifiers
    /// (`readonly`/`public`/`private`/`protected`/`static`/`export`). A
    /// modifier identifier is only consumed when it is followed by another
    /// modifier or a binding identifier. Port of
    /// `JSParserImpl::parseTSFunctionTypeParam` (ts.cpp:419-514).
    fn parse_ts_function_type_param(&mut self) -> Option<&'gc Node<'gc>> {
        // C++ 420.
        let start = self.cur_start();

        // C++ 422-425: `accessibilityNode` is a NodeLabel; null is
        // INVALID_ATOM_BYTES (dumps as JSON `null`).
        let mut accessibility_node = INVALID_ATOM_BYTES;
        let mut readonly_node = false;
        let mut static_node = false;
        let mut export_node = false;

        // C++ 427-495: the modifier loop.
        while self.check_n3(
            TokenKind::identifier,
            TokenKind::rw_static,
            TokenKind::rw_export,
        ) {
            // C++ 430-439: `static` (rw_static or `static` ident).
            if !static_node
                && (self.check(TokenKind::rw_static)
                    || self.check_name(b"static"))
            {
                self.advance(GrammarContext::Type);
                if self.check_n3(
                    TokenKind::identifier,
                    TokenKind::rw_static,
                    TokenKind::rw_export,
                ) {
                    static_node = true;
                    continue;
                }
            }
            // C++ 440-449: `export` (rw_export).
            if !export_node && self.check(TokenKind::rw_export) {
                self.advance(GrammarContext::Type);
                if self.check_n3(
                    TokenKind::identifier,
                    TokenKind::rw_static,
                    TokenKind::rw_export,
                ) {
                    export_node = true;
                    continue;
                }
            }
            // C++ 450-459: `readonly` (contextual ident).
            if !readonly_node && self.check_name(b"readonly") {
                self.advance(GrammarContext::Type);
                if self.check_n3(
                    TokenKind::identifier,
                    TokenKind::rw_static,
                    TokenKind::rw_export,
                ) {
                    readonly_node = true;
                    continue;
                }
            }
            // C++ 460-491: accessibility modifiers.
            if accessibility_node == INVALID_ATOM_BYTES {
                // C++ 461-470: `public` (rw_public or `public` ident).
                if self.check(TokenKind::rw_public)
                    || self.check_name(b"public")
                {
                    self.advance(GrammarContext::Type);
                    if self.check_n3(
                        TokenKind::identifier,
                        TokenKind::rw_static,
                        TokenKind::rw_export,
                    ) {
                        accessibility_node =
                            self.lexer.get_identifier(b"public");
                        continue;
                    }
                }
                // C++ 471-480: `private` (rw_private or `private` ident).
                if self.check(TokenKind::rw_private)
                    || self.check_name(b"private")
                {
                    self.advance(GrammarContext::Type);
                    if self.check_n3(
                        TokenKind::identifier,
                        TokenKind::rw_static,
                        TokenKind::rw_export,
                    ) {
                        accessibility_node =
                            self.lexer.get_identifier(b"private");
                        continue;
                    }
                }
                // C++ 481-490: `protected` (rw_protected or `protected` ident).
                if self.check(TokenKind::rw_protected)
                    || self.check_name(b"protected")
                {
                    self.advance(GrammarContext::Type);
                    if self.check_n3(
                        TokenKind::identifier,
                        TokenKind::rw_static,
                        TokenKind::rw_export,
                    ) {
                        accessibility_node =
                            self.lexer.get_identifier(b"protected");
                        continue;
                    }
                }
            }

            // C++ 493-494: not a modifier.
            break;
        }

        // C++ 497-499.
        let param = self.parse_binding_element(Param::default())?;

        // C++ 501-511.
        if accessibility_node != INVALID_ATOM_BYTES
            || readonly_node
            || static_node
            || export_node
        {
            let node = Node::TSParameterProperty(TSParameterProperty::new(
                NodeMetadata::new(self.dummy_range()),
                param,
                accessibility_node,
                readonly_node,
                static_node,
                export_node,
            ));
            return Some(self.set_location(
                start,
                self.lexer.prev_token_end(),
                node,
            ));
        }

        // C++ 513.
        Some(param)
    }
}
