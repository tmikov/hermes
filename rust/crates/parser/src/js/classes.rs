/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Class and decorator parsing for the JS parser. Port of the class-parsing
//! section of `lib/Parser/JSParserImpl.cpp` (parseDecoratorList /
//! parseDecorator / parseClassDeclaration / parseClassExpression /
//! parseClassTail / parseClassBody / parseClassBodyImpl / parseClassElement,
//! C++ lines 4688-5679).
//!
//! The non-ambiguous Flow productions (class/method type parameters,
//! super-class type arguments, `implements` clauses, field type annotations,
//! member variance, method return types) are ported (P5.4), as is the Flow
//! `declare` class-property modifier (P6, C++ 5095-5108). The TS productions
//! (modifiers, `?` optional fields, TS type params/args) are P7 — see the
//! comments at each site.

use ast::node::{
    CallExpression, ClassBody, ClassDeclaration, ClassExpression, ClassPrivateProperty,
    ClassProperty, Decorator, FunctionExpression, Identifier, MemberExpression, MethodDefinition,
    Node, PrivateName, StaticBlock, TSModifiers, Variance,
};
use ast::node_child::{NodeLabel, NodeList, NodeMetadata};
use atom_table::INVALID_ATOM_BYTES;
use support::location::{SMLoc, SMRange};

use crate::lexer::GrammarContext;
use crate::token_kinds::TokenKind;

use super::flow::{can_follow_variance_keyword_flow, AllowAnonFunctionType, AllowTypedArrowFunction, CoverTypedParameters};
use super::{
    AllowImportExport, IsClassHeritageArgument, JSParserImpl, Param, PARAM_DEFAULT, PARAM_IN,
    PARAM_RETURN,
};

/// Whether `parseClassTail` builds a `ClassDeclaration` or a `ClassExpression`.
/// Port of the C++ `enum class ClassParseKind { Expression, Declaration };`
/// (JSParserImpl.h).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum ClassParseKind {
    Expression,
    Declaration,
}

impl<'gc, 'ast, 'ctx, 'a> JSParserImpl<'gc, 'ast, 'ctx, 'a> {
    // -----------------------------------------------------------------------
    // parseDecoratorList — 4688 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a list of decorators (`@expr @expr ...`) into `list`. Port of
    /// `JSParserImpl::parseDecoratorList` (4688-4699).
    pub(super) fn parse_decorator_list(
        &mut self,
        list: &mut Vec<&'gc Node<'gc>>,
    ) -> bool {
        debug_assert!(self.check(TokenKind::at));

        while self.check(TokenKind::at) {
            match self.parse_decorator() {
                Some(decorator) => list.push(decorator),
                None => return false,
            }
        }

        true
    }

    // -----------------------------------------------------------------------
    // parseDecorator — 4701 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a single decorator. Port of `JSParserImpl::parseDecorator`
    /// (4701-4791). A decorator is either a parenthesized expression
    /// (`@( Expression )`) or an identifier member chain (`@a.b.c`) with an
    /// optional private-name property and an optional trailing call `(args)`.
    fn parse_decorator(&mut self) -> Option<&'gc Node<'gc>> {
        debug_assert!(self.check(TokenKind::at));

        let start_loc = self.advance(GrammarContext::AllowRegExp).start;
        let expr: &'gc Node<'gc>;

        if self.check(TokenKind::l_paren) {
            // DecoratorParenthesizedExpression:
            // ( Expression[+In, ?Yield, ?Await] )
            // ^
            let paren_loc = self.advance(GrammarContext::AllowRegExp).start;
            let inner = self.parse_expression(PARAM_IN, CoverTypedParameters::Yes)?;
            // C++ 4712-4718: eat(r_paren, AllowDiv, "at end of decorator
            // expression", "location of '('", parenLoc).
            if !self.eat_at(
                TokenKind::r_paren,
                GrammarContext::AllowDiv,
                " at end of decorator expression",
                Some("location of '('"),
                paren_loc,
            ) {
                return None;
            }
            expr = inner;
        } else {
            // Must be identifier (start of DecoratorMemberExpression).
            if !self.check(TokenKind::identifier) && !self.lexer.token().is_res_word() {
                // C++ 4723-4725: errorExpected(identifier, "in decorator",
                // "location of '@'", startLoc).
                self.error_expected_msg(
                    "'identifier' expected in decorator",
                    Some("location of '@'"),
                    Some(start_loc),
                );
                return None;
            }

            let tok_rng = self.lexer.token().source_range();
            let id_name = self.lexer.token().get_res_word_or_identifier();
            let mut cur = self.set_location(
                tok_rng.start,
                tok_rng.end,
                Node::Identifier(Identifier::new(
                    NodeMetadata::new(self.dummy_range()),
                    id_name,
                    None,
                    false,
                )),
            );
            self.advance(GrammarContext::AllowDiv);

            // Parse member chain:
            // DecoratorMemberExpression . IdentifierName
            //                           ^
            while self.check_and_eat(TokenKind::period, GrammarContext::AllowRegExp) {
                let property: &'gc Node<'gc>;

                if self.check(TokenKind::private_identifier) {
                    property = self.parse_private_name()?;
                } else if self.check(TokenKind::identifier)
                    || self.lexer.token().is_res_word()
                {
                    let prop_rng = self.lexer.token().source_range();
                    let prop_name = self.lexer.token().get_res_word_or_identifier();
                    property = self.set_location(
                        prop_rng.start,
                        prop_rng.end,
                        Node::Identifier(Identifier::new(
                            NodeMetadata::new(self.dummy_range()),
                            prop_name,
                            None,
                            false,
                        )),
                    );
                    self.advance(GrammarContext::AllowDiv);
                } else {
                    // C++ 4756-4761: errorExpected(identifier, "after '.' in
                    // decorator", "location of '@'", startLoc).
                    self.error_expected_msg(
                        "'identifier' expected after '.' in decorator",
                        Some("location of '@'"),
                        Some(start_loc),
                    );
                    return None;
                }

                let prop_end = property.range().end;
                cur = self.set_location(
                    start_loc,
                    prop_end,
                    Node::MemberExpression(MemberExpression::new(
                        NodeMetadata::new(self.dummy_range()),
                        cur,
                        property,
                        false,
                    )),
                );
            }

            // DecoratorCallExpression:
            // DecoratorMemberExpression Arguments
            //                           ^
            if self.check(TokenKind::l_paren) {
                let (arg_list, end_loc) = self.parse_arguments()?;
                cur = self.set_location(
                    start_loc,
                    end_loc,
                    Node::CallExpression(CallExpression::new(
                        NodeMetadata::new(self.dummy_range()),
                        cur,
                        None,
                        NodeList::from_iter(self.gc, arg_list),
                    )),
                );
            }

            expr = cur;
        }

        let end = self.lexer.prev_token_end();
        Some(self.set_location(
            start_loc,
            end,
            Node::Decorator(Decorator::new(
                NodeMetadata::new(self.dummy_range()),
                expr,
            )),
        ))
    }

    // -----------------------------------------------------------------------
    // parseClassDeclaration — 4793 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a class declaration. Port of `JSParserImpl::parseClassDeclaration`
    /// (4793-4873). The class name is required unless `param` has `+Default`
    /// (i.e. `export default class {}`).
    pub(super) fn parse_class_declaration(
        &mut self,
        param: Param,
    ) -> Option<&'gc Node<'gc>> {
        // NOTE: Class definition is always strict mode code.
        // C++ `SaveFunctionState saveFunctionState{this}; setStrictMode(true);`.
        // We add the SaveFunctionState guard (is_arrow=false, class body is
        // a regular function scope), save/restore seen_directives, and force
        // strict mode on, mirroring the C++ class-body force-strict path.
        let old_strict = self.lexer.is_strict_mode();
        // SaveFunctionState — mirrors C++ class-body force-strict path
        // (SaveFunctionState saveFunctionState{this}; setStrictMode(true);).
        let _g = self.save_function_state(false);
        let old_seen_len = self.seen_directives.len();
        self.lexer.set_strict_mode(true);
        let result = self.parse_class_declaration_inner(param);
        self.seen_directives.truncate(old_seen_len);
        self.lexer.set_strict_mode(old_strict);
        result
    }

    fn parse_class_declaration_inner(
        &mut self,
        param: Param,
    ) -> Option<&'gc Node<'gc>> {
        debug_assert!(
            self.check2(TokenKind::at, TokenKind::rw_class),
            "class must start with '@' or 'class'"
        );

        let start_loc = self.cur_start();
        let mut decorators: Vec<&'gc Node<'gc>> = Vec::new();

        if self.check(TokenKind::at) {
            if !self.parse_decorator_list(&mut decorators) {
                return None;
            }

            // C++ 4805-4811: eat(rw_class, AllowRegExp, "in class", "start
            // of class", startLoc).
            if !self.eat_at(
                TokenKind::rw_class,
                GrammarContext::AllowRegExp,
                " in class",
                Some("start of class"),
                start_loc,
            ) {
                return None;
            }
        } else {
            // No decorators, eat the 'class' token.
            debug_assert!(self.check(TokenKind::rw_class));
            self.advance(GrammarContext::AllowRegExp);
        }

        let mut name: Option<&'gc Node<'gc>> = None;

        if self.check(TokenKind::identifier) {
            match self.parse_binding_identifier(Param::default()) {
                Some(n) => name = Some(n),
                None => {
                    // C++ 4826-4831: errorExpected(identifier, "in class
                    // declaration", "location of 'class'", startLoc).
                    self.error_expected_msg(
                        "'identifier' expected in class declaration",
                        Some("location of 'class'"),
                        Some(start_loc),
                    );
                    return None;
                }
            }
        } else if !param.has(PARAM_DEFAULT) {
            // Identifier is required unless we have +Default parameter.
            // C++ 4833-4838: errorExpected(identifier, "after 'class'",
            // "location of 'class'", startLoc).
            self.error_expected_msg(
                "'identifier' expected after 'class'",
                Some("location of 'class'"),
                Some(start_loc),
            );
            return None;
        }

        // Flow class type parameters. C++ 4847-4854.
        let mut type_params: Option<&'gc Node<'gc>> = None;
        if self.parse_flow() && self.check(TokenKind::less) {
            type_params = Some(self.parse_type_params_flow()?);
        }
        // TS class type parameters. C++ 4855-4862.
        if self.parse_ts() && self.check(TokenKind::less) {
            type_params = Some(self.parse_ts_type_parameters()?);
        }

        self.parse_class_tail(
            start_loc,
            name,
            type_params,
            ClassParseKind::Declaration,
            decorators,
        )
    }

    // -----------------------------------------------------------------------
    // parseClassExpression — 4875 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a class expression. Port of `JSParserImpl::parseClassExpression`
    /// (4875-4951). The class name is optional (parsed only if the next token is
    /// neither `extends` nor `{`).
    pub(super) fn parse_class_expression(&mut self) -> Option<&'gc Node<'gc>> {
        // NOTE: A class definition is always strict mode code. See the comment
        // in `parse_class_declaration` for the save/restore rationale.
        let old_strict = self.lexer.is_strict_mode();
        // SaveFunctionState — mirrors C++ class-body force-strict path.
        let _g = self.save_function_state(false);
        let old_seen_len = self.seen_directives.len();
        self.lexer.set_strict_mode(true);
        let result = self.parse_class_expression_inner();
        self.seen_directives.truncate(old_seen_len);
        self.lexer.set_strict_mode(old_strict);
        result
    }

    fn parse_class_expression_inner(&mut self) -> Option<&'gc Node<'gc>> {
        debug_assert!(
            self.check2(TokenKind::at, TokenKind::rw_class),
            "class must start with '@' or 'class'"
        );

        let start = self.cur_start();
        let mut decorators: Vec<&'gc Node<'gc>> = Vec::new();

        if self.check(TokenKind::at) {
            if !self.parse_decorator_list(&mut decorators) {
                return None;
            }

            // C++ 4890-4896: eat(rw_class, AllowRegExp, "in class", "start
            // of class", start).
            if !self.eat_at(
                TokenKind::rw_class,
                GrammarContext::AllowRegExp,
                " in class",
                Some("start of class"),
                start,
            ) {
                return None;
            }
        } else {
            // No decorators, eat the 'class' token.
            debug_assert!(self.check(TokenKind::rw_class));
            self.advance(GrammarContext::AllowRegExp);
        }

        let mut name: Option<&'gc Node<'gc>> = None;

        // A ClassHeritage, `{`, or (with Flow) an `implements` clause or type
        // parameters, or (with TS) type parameters means there is no class name.
        // C++ 4907-4910, De Morgan'd (`!a && !b` -> `!(a || b)`) for clippy.
        if !(self.check2(TokenKind::rw_extends, TokenKind::l_brace)
            || (self.parse_flow()
                && self.check2(TokenKind::rw_implements, TokenKind::less))
            || (self.parse_ts() && self.check(TokenKind::less)))
        {
            // Try to parse a BindingIdentifier if we did not see a ClassHeritage
            // or a '{'.
            match self.parse_binding_identifier(Param::default()) {
                Some(n) => name = Some(n),
                None => {
                    // C++ 4912-4917: errorExpected(identifier, "in class
                    // expression", "location of 'class'", start).
                    self.error_expected_msg(
                        "'identifier' expected in class expression",
                        Some("location of 'class'"),
                        Some(start),
                    );
                    return None;
                }
            }
        }

        // Flow class type parameters. C++ 4925-4932.
        let mut type_params: Option<&'gc Node<'gc>> = None;
        if self.parse_flow() && self.check(TokenKind::less) {
            type_params = Some(self.parse_type_params_flow()?);
        }
        // TS class type parameters. C++ 4933-4940.
        if self.parse_ts() && self.check(TokenKind::less) {
            type_params = Some(self.parse_ts_type_parameters()?);
        }

        self.parse_class_tail(
            start,
            name,
            type_params,
            ClassParseKind::Expression,
            decorators,
        )
    }

    // -----------------------------------------------------------------------
    // parseClassTail — 4953 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse the heritage + body of a class. Port of
    /// `JSParserImpl::parseClassTail` (4953-5048). Builds either a
    /// `ClassDeclaration` or `ClassExpression` per `kind`.
    fn parse_class_tail(
        &mut self,
        start_loc: SMLoc,
        name: Option<&'gc Node<'gc>>,
        type_params: Option<&'gc Node<'gc>>,
        kind: ClassParseKind,
        decorators: Vec<&'gc Node<'gc>>,
    ) -> Option<&'gc Node<'gc>> {
        let mut super_class: Option<&'gc Node<'gc>> = None;
        let mut super_type_params: Option<&'gc Node<'gc>> = None;

        if self.check_and_eat(TokenKind::rw_extends, GrammarContext::AllowRegExp) {
            // ClassHeritage[opt] { ClassBody[opt] }
            // ^
            super_class =
                Some(self.parse_left_hand_side_expression(IsClassHeritageArgument::Yes)?);
            // Flow super-class type arguments. C++ 4970-4977. The C++ calls
            // `parseTypeArgsFlow()` with its default trailing grammar context
            // (Type, per JSParserImpl.h:1506-1508).
            if self.parse_flow() && self.check(TokenKind::less) {
                super_type_params =
                    Some(self.parse_type_args_flow(GrammarContext::Type)?);
            }
            // TS super-class type arguments. C++ 4978-4985.
            if self.parse_ts() && self.check(TokenKind::less) {
                super_type_params = Some(self.parse_ts_type_arguments()?);
            }
        }

        // Flow `implements` clause. C++ 4988-5010. In strict mode (a class is
        // always strict) `implements` lexes as `rw_implements`; the C++ also
        // accepts the `implementsIdent_` spelling, kept here for an identical
        // check.
        let mut implements: Vec<&'gc Node<'gc>> = Vec::new();
        if self.parse_flow() {
            let has_implements = self
                .check_and_eat(TokenKind::rw_implements, GrammarContext::AllowRegExp)
                || {
                    if self.check_name(b"implements") {
                        self.advance(GrammarContext::AllowRegExp);
                        true
                    } else {
                        false
                    }
                };
            if has_implements {
                while !self.check(TokenKind::l_brace) {
                    // C++ 4995-5000: need(identifier, "in class
                    // 'implements'", "start of class", startLoc).
                    if !self.need_at(
                        TokenKind::identifier,
                        " in class 'implements'",
                        Some("start of class"),
                        start_loc,
                    ) {
                        return None;
                    }
                    let impl_node = self.parse_class_implements_flow()?;
                    implements.push(impl_node);
                    if !self.check_and_eat(TokenKind::comma, GrammarContext::AllowRegExp)
                    {
                        break;
                    }
                }
            }
        }
        let implements = NodeList::from_iter(self.gc, implements);

        // C++ 5024-5029: need(l_brace, "in class definition", "start of
        // class", startLoc).
        if !self.need_at(
            TokenKind::l_brace,
            " in class definition",
            Some("start of class"),
            start_loc,
        ) {
            return None;
        }

        let body = self.parse_class_body(start_loc)?;
        let body_end = body.range().end;

        let decorator_list = NodeList::from_iter(self.gc, decorators);

        let node = match kind {
            ClassParseKind::Declaration => Node::ClassDeclaration(ClassDeclaration::new(
                NodeMetadata::new(self.dummy_range()),
                name,
                type_params,
                super_class,
                super_type_params,
                implements,
                decorator_list,
                body,
            )),
            ClassParseKind::Expression => Node::ClassExpression(ClassExpression::new(
                NodeMetadata::new(self.dummy_range()),
                name,
                type_params,
                super_class,
                super_type_params,
                implements,
                decorator_list,
                body,
            )),
        };
        Some(self.set_location(start_loc, body_end, node))
    }

    // -----------------------------------------------------------------------
    // parseClassBody — 5050 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a class body `{ ClassElement* }`. Port of
    /// `JSParserImpl::parseClassBody` (5050-5076). Tracks the constructor for the
    /// duplicate-constructor syntax-error check in `parse_class_body_impl`.
    fn parse_class_body(&mut self, start_loc: SMLoc) -> Option<&'gc Node<'gc>> {
        debug_assert!(
            self.check(TokenKind::l_brace),
            "class body must begin with '{{'"
        );
        let brace_loc = self.advance(GrammarContext::AllowRegExp).start;

        // It is a Syntax Error if PrototypePropertyNameList of ClassElementList
        // contains more than one occurrence of "constructor".
        let mut constructor: Option<&'gc Node<'gc>> = None;
        let mut body: Vec<&'gc Node<'gc>> = Vec::new();
        while !self.check(TokenKind::r_brace) {
            if !self.parse_class_body_impl(&mut body, &mut constructor, false) {
                return None;
            }
        }

        // C++ 5068-5073: need(r_brace, "at end of class definition", "start
        // of class", startLoc).
        if !self.need_at(
            TokenKind::r_brace,
            " at end of class definition",
            Some("start of class"),
            start_loc,
        ) {
            return None;
        }
        let end = self.advance(GrammarContext::AllowRegExp).end;

        Some(self.set_location(
            brace_loc,
            end,
            Node::ClassBody(ClassBody::new(
                NodeMetadata::new(self.dummy_range()),
                NodeList::from_iter(self.gc, body),
            )),
        ))
    }

    // -----------------------------------------------------------------------
    // parseClassBodyImpl — 5078 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse one class-body element and push it onto `body`. Port of
    /// `JSParserImpl::parseClassBodyImpl` (5078-5201). Handles the leading
    /// decorator list, the `static` modifier, empty `;` separators, the
    /// duplicate-constructor diagnostic, and the invalid `constructor` field
    /// name diagnostic.
    pub(super) fn parse_class_body_impl(
        &mut self,
        body: &mut Vec<&'gc Node<'gc>>,
        constructor: &mut Option<&'gc Node<'gc>>,
        eagerly: bool,
    ) -> bool {
        let mut is_static = false;
        let start_range = self.lexer.token().source_range();

        let mut decorators: Vec<&'gc Node<'gc>> = Vec::new();
        if self.check(TokenKind::at) && !self.parse_decorator_list(&mut decorators) {
            return false;
        }

        // Flow `declare` class-property modifier (C++ 5095-5108). Only a
        // modifier when followed by something that can start a class member;
        // otherwise `declare` is itself the property name. C++
        // `lookahead1(llvh::None)` uses the header default RequireNoNewLine=true.
        let mut declare = false;
        if self.parse_flow() && self.check_name(b"declare") {
            let opt_next = self.lexer.lookahead1::<true>(None);
            if matches!(
                opt_next,
                Some(
                    TokenKind::rw_static
                        | TokenKind::identifier
                        | TokenKind::plus
                        | TokenKind::private_identifier
                        | TokenKind::minus
                )
            ) {
                declare = true;
                self.advance(GrammarContext::AllowRegExp);
            }
        }

        // TS class-member modifiers. C++ 5110-5138. In TS, modifiers may appear
        // in this order: accessibility - static - readonly. All of them can be
        // used as identifiers, so each is only consumed when followed by another
        // modifier or a member name (`canFollowModifierTS`).
        let mut readonly = false;
        let mut accessibility: NodeLabel = INVALID_ATOM_BYTES;
        if self.parse_ts() {
            if self.check_n3(
                TokenKind::rw_private,
                TokenKind::rw_protected,
                TokenKind::rw_public,
            ) && can_follow_modifier_ts(self.lexer.lookahead1::<true>(None))
            {
                accessibility = self.lexer.token().get_res_word_identifier();
                self.advance(GrammarContext::AllowRegExp);
            }

            if self.check(TokenKind::rw_static)
                && can_follow_modifier_ts(self.lexer.lookahead1::<true>(None))
            {
                is_static = true;
                self.advance(GrammarContext::AllowRegExp);
            }

            if self.check_name(b"readonly")
                && can_follow_modifier_ts(self.lexer.lookahead1::<true>(None))
            {
                readonly = true;
                self.advance(GrammarContext::AllowRegExp);
            }
        }

        match self.cur_kind() {
            TokenKind::semi => {
                self.advance(GrammarContext::AllowRegExp);
            }
            _ => {
                if self.check(TokenKind::rw_static) {
                    // C++ 5146-5149: don't advance() when `readonly` or `static`
                    // is already seen, so the current one can be regarded as an
                    // identifier. `static` cannot come after `readonly` in TS.
                    if self.parse_ts() && (readonly || is_static) {
                        // Leave `static` to be parsed as a property name.
                    } else {
                        // static MethodDefinition / static FieldDefinition
                        is_static = true;
                        self.advance(GrammarContext::AllowRegExp);
                    }
                }
                // LLVM_FALLTHROUGH to default: parse the ClassElement.
                let elem = match self.parse_class_element(
                    is_static,
                    start_range,
                    declare,
                    readonly,
                    accessibility,
                    decorators,
                    eagerly,
                ) {
                    Some(e) => e,
                    None => return false,
                };

                if let Node::MethodDefinition(method) = elem {
                    let constructor_atom =
                        self.gc.ctx().atom_table.atom_bytes(b"constructor");
                    if method.kind.get() == constructor_atom {
                        if let Some(first) = *constructor {
                            // Cannot have duplicate constructors, but report the
                            // error and move on to parse the rest of the class.
                            self.error_at(
                                elem.range(),
                                "duplicate constructors in class",
                            );
                            self.lexer.get_source_mgr_mut().note_at(
                                first.range().start,
                                Some(first.range()),
                                "first constructor definition",
                                support::diag::Subsystem::Parser,
                            );
                        } else {
                            *constructor = Some(elem);
                        }
                    }
                } else if let Node::ClassProperty(prop) = elem {
                    if !prop.computed.get() {
                        let constructor_atom =
                            self.gc.ctx().atom_table.atom_bytes(b"constructor");
                        let is_ctor_name = match prop.key {
                            Node::Identifier(id) => id.name.get() == constructor_atom,
                            Node::StringLiteral(s) => s.value.get() == constructor_atom,
                            _ => false,
                        };
                        if is_ctor_name {
                            self.error_at(elem.range(), "invalid class property name");
                        }
                    }
                }

                body.push(elem);
            }
        }
        true
    }

    // -----------------------------------------------------------------------
    // parseClassElement — 5203 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse one class element (method / accessor / generator / async / field /
    /// static block / private member). Port of `JSParserImpl::parseClassElement`
    /// (5203-5679). This is the most intricate part of class parsing: it detects
    /// `get`/`set`/`async`/`*` specifiers, static blocks, `static` used as a
    /// property name, private names, and distinguishes fields from methods,
    /// reporting the various special-kind syntax errors.
    #[allow(clippy::too_many_arguments)]
    fn parse_class_element(
        &mut self,
        mut is_static: bool,
        start_range: SMRange,
        declare: bool,
        readonly: bool,
        accessibility: NodeLabel,
        decorators: Vec<&'gc Node<'gc>>,
        eagerly: bool,
    ) -> Option<&'gc Node<'gc>> {
        let start_loc = self.cur_start();

        // TS `optional` (`?`) field flag; set below at the field site.
        let mut optional = false;
        let mut is_private = false;

        // SpecialKind: indicates if this method is out of the ordinary — in
        // particular getters and setters. Local to this function in C++.
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum SpecialKind {
            None,
            Get,
            Set,
            Generator,
            Async,
            AsyncGenerator,
        }

        let mut special = SpecialKind::None;

        // When true, call parsePropertyName. Set to false if 'get'/'set'/'async'
        // or `static` were already parsed as the property name.
        let mut do_parse_property_name = true;

        let mut prop: Option<&'gc Node<'gc>> = None;
        if self.check_name(b"get") {
            let range = self.advance(GrammarContext::AllowRegExp);
            // checkN(less, l_paren, r_brace, equal, colon, semi, star)
            // less/colon are Flow/TS-only and never appear in JS, but we keep the
            // full set so the detection is identical: `get` is a getter unless
            // followed by one of these.
            if !self.check_class_element_after_accessor_name() {
                // This was actually a getter.
                special = SpecialKind::Get;
            } else {
                let get_ident = self.gc.ctx().atom_table.atom_bytes(b"get");
                prop = Some(self.set_location(
                    range.start,
                    range.end,
                    Node::Identifier(Identifier::new(
                        NodeMetadata::new(self.dummy_range()),
                        get_ident,
                        None,
                        false,
                    )),
                ));
                do_parse_property_name = false;
            }
        } else if self.check_name(b"set") {
            let range = self.advance(GrammarContext::AllowRegExp);
            if !self.check_class_element_after_accessor_name() {
                // If we don't see '(' then this was actually a setter.
                special = SpecialKind::Set;
            } else {
                let set_ident = self.gc.ctx().atom_table.atom_bytes(b"set");
                prop = Some(self.set_location(
                    range.start,
                    range.end,
                    Node::Identifier(Identifier::new(
                        NodeMetadata::new(self.dummy_range()),
                        set_ident,
                        None,
                        false,
                    )),
                ));
                do_parse_property_name = false;
            }
        } else if self.check_unescaped_name(b"async") {
            let range = self.advance(GrammarContext::AllowRegExp);
            // checkN(less, l_paren, r_brace, equal, colon, semi) — note: no star,
            // since `async *` is an async generator.
            if !self.check_class_element_after_async_name()
                && !self.lexer.is_new_line_before_current_token()
            {
                // If we don't see '(' then this was actually an async method.
                // Async methods cannot have a newline between 'async' and the
                // name. These can be either Async or AsyncGenerator, so check.
                special = if self
                    .check_and_eat(TokenKind::star, GrammarContext::AllowRegExp)
                {
                    SpecialKind::AsyncGenerator
                } else {
                    SpecialKind::Async
                };
            } else {
                let async_ident = self.gc.ctx().atom_table.atom_bytes(b"async");
                prop = Some(self.set_location(
                    range.start,
                    range.end,
                    Node::Identifier(Identifier::new(
                        NodeMetadata::new(self.dummy_range()),
                        async_ident,
                        None,
                        false,
                    )),
                ));
                do_parse_property_name = false;
            }
        } else if self.check_and_eat(TokenKind::star, GrammarContext::AllowRegExp) {
            special = SpecialKind::Generator;
        } else if is_static
            && self.check_and_eat(TokenKind::l_brace, GrammarContext::AllowRegExp)
        {
            // This is a static block.
            // ES14.0 15.7
            // ClassStaticBlock :
            //   static { ClassStaticBlockBody }
            //          ^
            let brace_loc = self.cur_start();
            let mut block_body: Vec<&'gc Node<'gc>> = Vec::new();

            {
                // ClassStaticBlockStatementList :
                //   StatementList[~Yield, +Await, ~Return]opt
                //   ^
                let _guard_yield = self.save_param_yield(false);
                let _guard_await = self.save_param_await(true);
                if !self.parse_statement_list(
                    Param::default(),
                    [TokenKind::r_brace],
                    /* parse_directives */ false,
                    AllowImportExport::No,
                    &mut block_body,
                ) {
                    return None;
                }
            }
            // C++ 5331-5337: eat(r_brace, AllowRegExp, "at end of static
            // block", "static block starts here", braceLoc).
            if !self.eat_at(
                TokenKind::r_brace,
                GrammarContext::AllowRegExp,
                " at end of static block",
                Some("static block starts here"),
                brace_loc,
            ) {
                return None;
            }

            let end = self.lexer.prev_token_end();
            return Some(self.set_location(
                start_loc,
                end,
                Node::StaticBlock(StaticBlock::new(
                    NodeMetadata::new(self.dummy_range()),
                    NodeList::from_iter(self.gc, block_body),
                )),
            ));
        } else if is_static && self.static_is_property_name() {
            // This is the name of the property/method. We've already parsed
            // 'static', but it must be used as the PropertyName and not as an
            // indicator for a static function.
            let static_ident = self.gc.ctx().atom_table.atom_bytes(b"static");
            prop = Some(self.set_location(
                start_range.start,
                start_range.end,
                Node::Identifier(Identifier::new(
                    NodeMetadata::new(self.dummy_range()),
                    static_ident,
                    None,
                    false,
                )),
            ));
            is_static = false;
            do_parse_property_name = false;
        }

        // Flow member variance: `+`/`-` sigils, or the contextual
        // `readonly`/`writeonly` keywords when followed by a property name.
        // C++ 5368-5384.
        let mut variance: Option<&'gc Node<'gc>> = None;
        if self.parse_flow() && self.check2(TokenKind::plus, TokenKind::minus) {
            // C++ 5370-5376: the Variance kind is the interned "plus" /
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
            variance = Some(self.set_location(v_range.start, v_range.end, v_node));
            self.advance(GrammarContext::Type);
        } else if self.parse_flow()
            && (self.check_name(b"readonly") || self.check_name(b"writeonly"))
            && can_follow_variance_keyword_flow(self.lexer.lookahead1::<true>(None))
        {
            // C++ 5377-5383.
            let v_range = self.cur_range();
            let v_node = Node::Variance(Variance::new(
                NodeMetadata::new(self.dummy_range()),
                self.lexer.token().get_identifier(),
            ));
            variance = Some(self.set_location(v_range.start, v_range.end, v_node));
            self.advance(GrammarContext::Type);
        }

        let mut computed = false;
        if do_parse_property_name {
            if self.check(TokenKind::private_identifier) {
                is_private = true;
                let tok_rng = self.lexer.token().source_range();
                let priv_name = self.lexer.token().get_private_identifier();
                prop = Some(self.set_location(
                    tok_rng.start,
                    tok_rng.end,
                    Node::Identifier(Identifier::new(
                        NodeMetadata::new(self.dummy_range()),
                        priv_name,
                        None,
                        false,
                    )),
                ));
                self.advance(GrammarContext::AllowRegExp);
            } else {
                computed = self.check(TokenKind::l_square);
                prop = Some(self.parse_property_name()?);
            }
        }

        // prop is always set by this point (one of the branches above ran, or
        // doParsePropertyName produced it).
        let prop = prop.expect("class element property name must be set");

        // Store the propName for comparisons, used for SyntaxErrors.
        let prop_name: Option<atom_table::AtomBytes> = match prop {
            Node::Identifier(id) => Some(id.name.get()),
            Node::StringLiteral(s) => Some(s.value.get()),
            _ => None,
        };

        let constructor_atom = self.gc.ctx().atom_table.atom_bytes(b"constructor");
        let prototype_atom = self.gc.ctx().atom_table.atom_bytes(b"prototype");

        let is_constructor =
            !is_static && !computed && prop_name == Some(constructor_atom);

        // The `<`-vs-`(` check is unconditional in C++ (5416-5417): a `<` after
        // the property name always routes to the method path (where, without
        // types enabled, the missing `(` is then reported).
        if special == SpecialKind::None
            && !self.check2(TokenKind::less, TokenKind::l_paren)
        {
            // Parse a class property, because this can't be a method definition.
            // Attempt ASI after the fact, and continue on, letting the next
            // iteration error if it wasn't actually a class property.
            // FieldDefinition ;
            //                 ^
            // TS `?` optional flag. C++ 5424-5428.
            if self.parse_ts()
                && self
                    .check_and_eat(TokenKind::question, GrammarContext::AllowRegExp)
            {
                optional = true;
            }
            // `: TypeAnnotation`. C++ 5429-5437.
            let mut type_annotation: Option<&'gc Node<'gc>> = None;
            if self.parse_types() && self.check(TokenKind::colon) {
                let annot_start = self.advance(GrammarContext::Type).start;
                type_annotation = Some(self.parse_type_annotation(
                    Some(annot_start),
                    AllowAnonFunctionType::Yes,
                )?);
            }

            let mut value: Option<&'gc Node<'gc>> = None;
            if self.check_and_eat(TokenKind::equal, GrammarContext::AllowRegExp) {
                // ClassElementName Initializer[opt]
                //                  ^
                // NOTE: This is technically non-compliant, but having yield/await
                // in the field initializer doesn't make sense.
                // See https://github.com/tc39/ecma262/issues/3333
                // Do [~Yield, +Await, ~Return] as suggested and error in
                // resolution.
                let _guard_yield = self.save_param_yield(false);
                let _guard_await = self.save_param_await(true);
                value = Some(self.parse_assignment_expression(PARAM_IN, false, AllowTypedArrowFunction::Yes, CoverTypedParameters::Yes, None)?);
                if declare {
                    self.error_at(start_range, "Invalid 'declare' with initializer");
                }
            }
            // ASI is allowed for separating class elements.
            // C++ 5432-5438: errorExpected(semi, "after class property",
            // "start of class property", startRange.Start).
            if !self.eat_semi(true) && type_annotation.is_none() {
                self.error_expected_msg(
                    "';' expected after class property",
                    Some("start of class property"),
                    Some(start_range.start),
                );
                return None;
            }
            if is_private {
                // The inner Identifier holds the private name (#-stripped); the
                // `#constructor` check is on the identifier name.
                if let Node::Identifier(id) = prop {
                    if id.name.get() == constructor_atom {
                        self.error_at(
                            prop.range(),
                            "Private names cannot be '#constructor'",
                        );
                    }
                }
                if accessibility != INVALID_ATOM_BYTES {
                    self.error_at(
                        start_range,
                        "An accessibility modifier cannot be used with a private identifier",
                    );
                }
                // TS modifiers node. C++ 5475-5480: a private property carries a
                // `TSModifiers` with a null accessibility (private names can't be
                // combined with an accessibility modifier). Built without
                // `setLocation` in C++, so it has an invalid (omitted) range.
                let modifiers: Option<&'gc Node<'gc>> = if self.parse_ts() {
                    Some(self.gc.alloc(Node::TSModifiers(TSModifiers::new(
                        NodeMetadata::new(self.invalid_range()),
                        INVALID_ATOM_BYTES,
                        readonly,
                    ))))
                } else {
                    None
                };
                let end = self.lexer.prev_token_end();
                return Some(self.set_location(
                    prop.range().start,
                    end,
                    Node::ClassPrivateProperty(ClassPrivateProperty::new(
                        NodeMetadata::new(self.dummy_range()),
                        prop,
                        value,
                        is_static,
                        NodeList::from_iter(self.gc, decorators),
                        declare,
                        optional,
                        variance,
                        type_annotation,
                        modifiers,
                    )),
                ));
            }
            // TS modifiers node. C++ 5495-5501.
            let modifiers: Option<&'gc Node<'gc>> = if self.parse_ts() {
                Some(self.gc.alloc(Node::TSModifiers(TSModifiers::new(
                    NodeMetadata::new(self.invalid_range()),
                    accessibility,
                    readonly,
                ))))
            } else {
                None
            };
            if is_static && !computed && prop_name == Some(prototype_atom) {
                self.error_at(
                    prop.range(),
                    "Static class properties cannot be named 'prototype'",
                );
            }
            let end = self.lexer.prev_token_end();
            return Some(self.set_location(
                start_range.start,
                end,
                Node::ClassProperty(ClassProperty::new(
                    NodeMetadata::new(self.dummy_range()),
                    prop,
                    value,
                    computed,
                    is_static,
                    NodeList::from_iter(self.gc, decorators),
                    declare,
                    optional,
                    variance,
                    type_annotation,
                    modifiers,
                )),
            ));
        }

        if declare {
            self.error_at(start_range, "Invalid 'declare' in class method");
        }

        let func_expr_start_loc = self.cur_start();

        // Flow method type parameters. C++ 5529-5537.
        let mut type_params: Option<&'gc Node<'gc>> = None;
        if self.parse_flow() && self.check(TokenKind::less) {
            type_params = Some(self.parse_type_params_flow()?);
        }

        // C++ 5502-5507: need(l_paren, "in method definition", "start of
        // method definition", startLoc).
        if !self.need_at(
            TokenKind::l_paren,
            " in method definition",
            Some("start of method definition"),
            start_loc,
        ) {
            return None;
        }
        let mut args: Vec<&'gc Node<'gc>> = Vec::new();

        let yield_in_body =
            special == SpecialKind::Generator || special == SpecialKind::AsyncGenerator;
        let await_in_body =
            special == SpecialKind::Async || special == SpecialKind::AsyncGenerator;

        let _guard_yield = self.save_param_yield(yield_in_body);
        let _guard_await = self.save_param_await(await_in_body);

        if !self.parse_formal_parameters(Param::default(), &mut args) {
            return None;
        }

        // `: ReturnType` (no predicate on methods). C++ 5560-5569.
        let mut return_type: Option<&'gc Node<'gc>> = None;
        if self.parse_types() && self.check(TokenKind::colon) {
            let annot_start = self.advance(GrammarContext::Type).start;
            return_type = Some(self.parse_return_type_annotation(
                Some(annot_start),
                AllowAnonFunctionType::Yes,
            )?);
        }

        // C++ 5540-5545: need(l_brace, "in method definition", "start of
        // method definition", startLoc).
        if !self.need_at(
            TokenKind::l_brace,
            " in method definition",
            Some("start of method definition"),
            start_loc,
        ) {
            return None;
        }

        let body = self.parse_function_body(
            PARAM_RETURN,
            eagerly,
            yield_in_body,
            await_in_body,
            GrammarContext::AllowRegExp,
            true,
        )?;
        let body_end = body.range().end;

        let params_len = args.len();
        let func = FunctionExpression::new(
            NodeMetadata::new(self.dummy_range()),
            None,
            NodeList::from_iter(self.gc, args),
            body,
            type_params,
            return_type,
            None,          // predicate
            yield_in_body, // generator
            await_in_body, // async
        );
        debug_assert!(
            self.lexer.is_strict_mode(),
            "parseClassElement should only be used for classes"
        );
        func.is_method_definition.set(true);
        let func_expr = self.set_location(
            func_expr_start_loc,
            body_end,
            Node::FunctionExpression(func),
        );

        if special == SpecialKind::Get && params_len != 0 {
            self.error_range(
                start_loc,
                body_end,
                &format!(
                    "getter method must no one formal arguments, found {}",
                    params_len
                ),
            );
        }

        if special == SpecialKind::Set && params_len != 1 {
            self.error_range(
                start_loc,
                body_end,
                &format!(
                    "setter method must have exactly one formal argument, found {}",
                    params_len
                ),
            );
        }

        // C++ 5619-5626 (compile-time `#if HERMES_PARSE_FLOW` only — no runtime
        // context check; type_params can only be set with types enabled).
        if (special == SpecialKind::Get || special == SpecialKind::Set)
            && type_params.is_some()
        {
            self.error_range(
                start_loc,
                body_end,
                "accessor method may not have type parameters",
            );
        }

        if is_static && !is_private && !computed && prop_name == Some(prototype_atom) {
            // ClassElement : static MethodDefinition
            // It is a Syntax Error if PropName of MethodDefinition is "prototype".
            self.error_range(start_loc, body_end, "prototype method must not be static");
            return None;
        }

        if is_private && prop_name == Some(constructor_atom) {
            // ClassElementName : PrivateIdentifier
            // It is a Syntax Error if the StringValue of PrivateIdentifier is
            // "#constructor".
            self.error_range(
                start_loc,
                body_end,
                "constructor method must not be private",
            );
            return None;
        }

        let mut kind = self.gc.ctx().atom_table.atom_bytes(b"method");
        if is_constructor {
            if special != SpecialKind::None {
                // It is a Syntax Error if PropName of MethodDefinition is
                // "constructor" and SpecialMethod of MethodDefinition is true.
                self.error_range(
                    start_loc,
                    body_end,
                    "constructor method must not be a getter or setter",
                );
                return None;
            }
            kind = self.gc.ctx().atom_table.atom_bytes(b"constructor");
        } else if special == SpecialKind::Get {
            kind = self.gc.ctx().atom_table.atom_bytes(b"get");
        } else if special == SpecialKind::Set {
            kind = self.gc.ctx().atom_table.atom_bytes(b"set");
        }

        let prop = if is_private {
            self.set_location(
                start_loc,
                prop.range().end,
                Node::PrivateName(PrivateName::new(
                    NodeMetadata::new(self.dummy_range()),
                    prop,
                )),
            )
        } else {
            prop
        };

        // A variance sigil is only valid on fields, not methods. C++ 5670-5672.
        if let Some(variance) = variance {
            self.error_at(variance.range(), "Unexpected variance sigil");
        }

        Some(self.set_location(
            start_range.start,
            body_end,
            Node::MethodDefinition(MethodDefinition::new(
                NodeMetadata::new(self.dummy_range()),
                prop,
                func_expr,
                kind,
                computed,
                is_static,
                NodeList::from_iter(self.gc, decorators),
            )),
        ))
    }

    /// `true` when the token following a `get`/`set` keyword indicates that it
    /// was actually a property name, not an accessor specifier. Port of the
    /// `checkN(less, l_paren, r_brace, equal, colon, semi, star)` test shared by
    /// the get/set branches of `parseClassElement` (C++ 5260-5267 / 5279-5286).
    /// `less`/`colon` are Flow/TS-only tokens that never appear in plain JS, but
    /// are kept for an identical check.
    fn check_class_element_after_accessor_name(&self) -> bool {
        let k = self.cur_kind();
        k == TokenKind::less
            || k == TokenKind::l_paren
            || k == TokenKind::r_brace
            || k == TokenKind::equal
            || k == TokenKind::colon
            || k == TokenKind::semi
            || k == TokenKind::star
    }

    /// `true` when the current token after an `async` specifier means `async`
    /// was actually the property name (not an async-method modifier). Port of
    /// `checkN(less, l_paren, r_brace, equal, colon, semi)` (JSParserImpl.cpp
    /// 5298-5304). No `star`, since `async *` is an async generator. `less`/
    /// `colon` are Flow/TS tokens, kept for an identical check.
    fn check_class_element_after_async_name(&self) -> bool {
        let k = self.cur_kind();
        k == TokenKind::less
            || k == TokenKind::l_paren
            || k == TokenKind::r_brace
            || k == TokenKind::equal
            || k == TokenKind::colon
            || k == TokenKind::semi
    }

    /// `true` when the current token indicates that `static` was actually the
    /// property name and not a modifier. Port of the `staticIsPropertyName`
    /// closure (C++ 5241-5255). e.g. `static() {}` returns true (current tok is
    /// `(`), but `static x;` returns false. With types enabled, `static: T;`
    /// (field type) and `static<T>() {}` (method type params) also make
    /// `static` the property name.
    fn static_is_property_name(&self) -> bool {
        if self.check_n4(
            TokenKind::l_paren,
            TokenKind::equal,
            TokenKind::r_brace,
            TokenKind::semi,
        ) {
            return true;
        }
        // C++ 5249-5252.
        if self.parse_types() && self.check2(TokenKind::less, TokenKind::colon) {
            return true;
        }
        false
    }

    /// Report an error spanning `[start, end]`. Convenience wrapper around
    /// `error_at` for the `error({startLoc, endLoc}, msg)` call sites in
    /// `parseClassElement`.
    fn error_range(&mut self, start: SMLoc, end: SMLoc, msg: &str) {
        self.error_at(SMRange { start, end }, msg);
    }
}

/// Check whether a token following a TS class-member modifier can itself be a
/// modifier or a member name, used to disambiguate the modifier keyword from a
/// property name of the same spelling. Port of the static
/// `JSParserImpl::canFollowModifierTS` (JSParserImpl.h 1645-1660).
fn can_follow_modifier_ts(opt_token_kind: Option<TokenKind>) -> bool {
    matches!(
        opt_token_kind,
        Some(
            TokenKind::identifier
                | TokenKind::private_identifier
                | TokenKind::rw_private
                | TokenKind::rw_protected
                | TokenKind::rw_public
                | TokenKind::rw_static
        )
    )
}
