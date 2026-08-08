/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The Flow declaration gate (`parseFlowDeclaration`), `type`/`opaque type`
//! alias declarations, and `interface` declarations. Port of the declaration
//! entry points of `lib/Parser/JSParserImpl-flow.cpp`.

use ast::node::{
    BigIntLiteral, BlockStatement, BooleanLiteral, ComponentDeclaration,
    ComponentParameter, DeclareClass, DeclareComponent, DeclareEnum,
    DeclareExportAllDeclaration, DeclareExportDeclaration, DeclareFunction,
    DeclareHook, DeclareInterface, DeclareModule, DeclareModuleExports,
    DeclareNamespace, DeclareOpaqueType, DeclareTypeAlias, DeclareVariable,
    EnumBigIntBody, EnumBigIntMember, EnumBooleanBody, EnumBooleanMember,
    EnumDeclaration, EnumDefaultedMember, EnumNumberBody, EnumNumberMember,
    EnumStringBody, EnumStringMember, EnumSymbolBody, ExportAllDeclaration,
    ExportNamedDeclaration, FunctionExpression, FunctionTypeAnnotation,
    HookDeclaration, HookTypeAnnotation, Identifier, InterfaceDeclaration,
    InterfaceExtends, MethodDefinition, Node, NumericLiteral, OpaqueType,
    RecordDeclaration, RecordDeclarationBody, RecordDeclarationImplements,
    RecordDeclarationProperty, RecordDeclarationStaticProperty, StringLiteral,
    TypeAlias, TypeAnnotation,
};
use ast::node_child::{NodeList, NodeMetadata};
use support::location::{SMLoc, SMRange};

use crate::js::{AllowImportExport, JSParserImpl, Param, PARAM_IN};
use crate::lexer::GrammarContext;
use crate::token_kinds::TokenKind;

use super::{
    AllowAnonFunctionType, AllowProtoProperty, AllowSpreadProperty,
    AllowStaticProperty, AllowTypedArrowFunction, CoverTypedParameters,
    TypeAliasKind,
};

/// The kind of a Flow `enum`. Port of `JSParserImpl::EnumKind`
/// (JSParserImpl.h:1550-1556 — keep the SAME variant order).
#[derive(Clone, Copy, PartialEq, Eq)]
enum EnumKind {
    String,
    Number,
    BigInt,
    Boolean,
    Symbol,
}

/// The user-facing name of an enum kind. Port of
/// `JSParserImpl::enumKindStrFlow` (JSParserImpl.h:1558-1572).
fn enum_kind_str_flow(kind: EnumKind) -> &'static str {
    match kind {
        EnumKind::String => "string",
        EnumKind::Number => "number",
        EnumKind::BigInt => "bigint",
        EnumKind::Boolean => "boolean",
        EnumKind::Symbol => "symbol",
    }
}

/// The enum kind implied by a member node, or `None` for a defaulted member.
/// Port of `JSParserImpl::getMemberEnumKindFlow` (JSParserImpl.h:1574-1587).
fn get_member_enum_kind_flow(member: &Node<'_>) -> Option<EnumKind> {
    match member {
        Node::EnumStringMember(_) => Some(EnumKind::String),
        Node::EnumNumberMember(_) => Some(EnumKind::Number),
        Node::EnumBigIntMember(_) => Some(EnumKind::BigInt),
        Node::EnumBooleanMember(_) => Some(EnumKind::Boolean),
        _ => None,
    }
}

impl<'gc, 'ast, 'ctx, 'a> JSParserImpl<'gc, 'ast, 'ctx, 'a> {
    // -----------------------------------------------------------------------
    // parseFlowDeclaration — 21 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a Flow declaration (`type`/`opaque type`/`interface`/`enum`/...).
    /// Port of `JSParserImpl::parseFlowDeclaration` (flow.cpp:21-93).
    /// Reached from `parse_declaration` only when `check_declaration()` is
    /// true, so (like the C++) it never falls through: `None` means an error
    /// was already reported.
    pub(in crate::js) fn parse_flow_declaration(&mut self) -> Option<&'gc Node<'gc>> {
        // C++ 22.
        assert!(self.check_declaration(), "invalid start for Flow declaration");
        let start = self.cur_start();

        // C++ 25-30: `async component`.
        if self.parse_flow_component_syntax()
            && self.check_unescaped_name(b"async")
            && self.check_async_component_flow()
        {
            self.advance(GrammarContext::AllowRegExp); // consume 'async'
            return self.parse_component_declaration_flow(
                start, /* declare */ false, /* is_async */ true,
            );
        }

        // C++ 32-35: `component`.
        if self.parse_flow_component_syntax()
            && self.check_component_declaration_flow()
        {
            return self.parse_component_declaration_flow(
                start, /* declare */ false, /* is_async */ false,
            );
        }

        // C++ 37-41: `async hook`.
        if self.parse_flow_component_syntax()
            && self.check_unescaped_name(b"async")
            && self.check_async_hook_flow()
        {
            self.advance(GrammarContext::AllowRegExp); // consume 'async'
            return self
                .parse_hook_declaration_flow(start, /* is_async */ true);
        }

        // C++ 43-45: `hook`.
        if self.parse_flow_component_syntax()
            && self.check_hook_declaration_flow()
        {
            return self
                .parse_hook_declaration_flow(start, /* is_async */ false);
        }

        // C++ 47-49: record declarations (gated on getParseFlowRecords()).
        if self.parse_flow_records() && self.check_record_declaration_flow() {
            return self.parse_record_declaration_flow(start);
        }

        // C++ 51-56.
        if self.check(TokenKind::rw_enum) {
            // C++ 52-55. The `declare enum` / `export declare enum` routing
            // (which calls this with declare=true) lands in P6.6.
            return self.parse_enum_declaration_flow(start, /*declare*/ false);
        }

        // C++ 58-62. `checkAndEat(<ident>)` advances with the default
        // GrammarContext::AllowRegExp. NOTE: `check_declaration()` does not
        // accept `declare` as a declaration start, so (like the C++) the
        // Declare kind is unreachable through this gate — the `declare`
        // statement routing (parseDeclareFLow, C++ 95-110) lands in P6.
        let mut kind = TypeAliasKind::None;
        if self.check_name(b"declare") {
            self.advance(GrammarContext::AllowRegExp);
            kind = TypeAliasKind::Declare;
        } else if self.check_name(b"opaque") {
            self.advance(GrammarContext::AllowRegExp);
            kind = TypeAliasKind::Opaque;
        }

        // C++ 64-68.
        if kind == TypeAliasKind::Declare
            && !(self.check_name(b"type")
                || self.check_name(b"interface")
                || self.check(TokenKind::rw_interface))
        {
            self.error_cur("invalid token in type declaration");
            return None;
        }
        // C++ 69-72.
        if kind == TypeAliasKind::Opaque && !self.check_name(b"type") {
            self.error_cur("invalid token in opaque type declaration");
            return None;
        }

        // C++ 74-79.
        if self.check_name(b"type") {
            self.advance(GrammarContext::AllowRegExp);
            return self.parse_type_alias_flow(start, kind);
        }

        // C++ 81-87.
        if self.check_name(b"interface") || self.check(TokenKind::rw_interface) {
            return self.parse_interface_declaration_flow(
                if kind == TypeAliasKind::Declare {
                    Some(start)
                } else {
                    None
                },
            );
        }

        // C++ 89-92.
        unreachable!("checkDeclaration() returned true without 'type' or 'interface'");
    }

    // -----------------------------------------------------------------------
    // checkComponentDeclarationFlow — 195 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Whether the current token starts a `component` declaration. Port of
    /// `JSParserImpl::checkComponentDeclarationFlow` (flow.cpp:195-205). MUST
    /// be idempotent (it is called from `checkDeclaration`), so the lookahead
    /// passes no expected token.
    pub(in crate::js) fn check_component_declaration_flow(&mut self) -> bool {
        // C++ 196-197.
        if !self.check_name(b"component") {
            return false;
        }
        // C++ 199-204: don't pass an `expectedToken` so we don't advance on a
        // match (lets `parseComponentDeclarationFlow` reparse the token), and
        // so this stays idempotent.
        self.lexer.lookahead1::<true>(None) == Some(TokenKind::identifier)
    }

    /// Whether the current token (`async`) starts an `async component`
    /// declaration. Port of `JSParserImpl::checkAsyncComponentFlow`
    /// (flow.cpp:207-218). Callers must already have checked
    /// `check_unescaped_name(b"async")`.
    pub(in crate::js) fn check_async_component_flow(&mut self) -> bool {
        // C++ 211.
        debug_assert!(self.check_unescaped_name(b"async"));
        // C++ 212-216.
        let save_point = self.lexer.save_point();
        self.advance(GrammarContext::AllowRegExp);
        let result = !self.lexer.is_new_line_before_current_token()
            && self.check_component_declaration_flow();
        save_point.restore(&mut self.lexer);
        result
    }

    /// Whether the current token (`async`) starts an `async hook` declaration.
    /// Port of `JSParserImpl::checkAsyncHookFlow` (flow.cpp:220-231). Callers
    /// must already have checked `check_unescaped_name(b"async")`.
    pub(in crate::js) fn check_async_hook_flow(&mut self) -> bool {
        // C++ 224.
        debug_assert!(self.check_unescaped_name(b"async"));
        // C++ 225-229.
        let save_point = self.lexer.save_point();
        self.advance(GrammarContext::AllowRegExp);
        let result = !self.lexer.is_new_line_before_current_token()
            && self.check_hook_declaration_flow();
        save_point.restore(&mut self.lexer);
        result
    }

    /// Whether the current token starts a `hook` declaration. Port of
    /// `JSParserImpl::checkHookDeclarationFlow` (flow.cpp:768-778). MUST be
    /// idempotent (called from `checkDeclaration`).
    pub(in crate::js) fn check_hook_declaration_flow(&mut self) -> bool {
        // C++ 769-770.
        if !self.check_name(b"hook") {
            return false;
        }
        // C++ 772-777.
        self.lexer.lookahead1::<true>(None) == Some(TokenKind::identifier)
    }

    // -----------------------------------------------------------------------
    // parseComponentDeclarationFlow — 233 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a `component` declaration (or, when `declare`, a
    /// `DeclareComponent`), with the cursor at `component` and `start` at the
    /// start of the declaration. Port of
    /// `JSParserImpl::parseComponentDeclarationFlow` (flow.cpp:233-330).
    ///
    /// The `declare` form (which uses component-type parameters) is reachable
    /// only from the `declare component` routing in P6.6.
    pub(in crate::js) fn parse_component_declaration_flow(
        &mut self,
        start: SMLoc,
        declare: bool,
        is_async: bool,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 237-239.
        debug_assert!(self.check_name(b"component"));
        self.advance(GrammarContext::AllowRegExp);

        // C++ 241-252: components always require a name identifier.
        let Some(id) = self.parse_binding_identifier(Param::default()) else {
            // C++ 245-251: errorExpected(identifier, "after 'component'",
            // "location of 'component'", start).
            self.error_expected_msg(
                "'identifier' expected after 'component'",
                Some("location of 'component'"),
                Some(start),
            );
            return None;
        };

        // C++ 254-261.
        let mut type_params: Option<&'gc Node<'gc>> = None;
        if self.check(TokenKind::less) {
            type_params = Some(self.parse_type_params_flow()?);
        }

        // C++ 263-269.
        if !self.need_at(
            TokenKind::l_paren,
            " at start of component parameter list",
            Some("component declaration starts here"),
            start,
        ) {
            return None;
        }

        // C++ 271-282.
        let mut param_list: Vec<&'gc Node<'gc>> = Vec::new();
        let mut rest: Option<&'gc Node<'gc>> = None;
        if declare {
            rest = self.parse_component_type_parameters_flow(
                Param::default(),
                &mut param_list,
            )?;
        } else if !self
            .parse_component_parameters_flow(Param::default(), &mut param_list)
        {
            return None;
        }

        // C++ 284-290.
        let mut renders_type: Option<&'gc Node<'gc>> = None;
        if self.check_name(b"renders") {
            renders_type =
                Some(self.parse_component_render_type_flow(false)?);
        }

        let params = NodeList::from_iter(self.gc, param_list);

        // C++ 292-301: the declare form ends here with `eatSemi`.
        if declare {
            if !self.eat_semi(false) {
                return None;
            }
            let node = Node::DeclareComponent(DeclareComponent::new(
                NodeMetadata::new(self.dummy_range()),
                id,
                params,
                rest,
                type_params,
                renders_type,
            ));
            return Some(self.set_location(
                start,
                self.lexer.prev_token_end(),
                node,
            ));
        }

        // C++ 303-309.
        if !self.need_at(
            TokenKind::l_brace,
            " in component declaration",
            Some("start of component declaration"),
            start,
        ) {
            return None;
        }

        // C++ 311-318: paramAwait_ = isAsync around the body; the function
        // state is saved/restored by the body parse.
        let body = {
            let _guard_await = self.save_param_await(is_async);
            self.parse_function_body(
                Param::default(),
                false,
                false,
                is_async,
                GrammarContext::AllowRegExp,
                true,
            )?
        };

        // C++ 320-329.
        let end = body.range().end;
        let node = Node::ComponentDeclaration(ComponentDeclaration::new(
            NodeMetadata::new(self.dummy_range()),
            id,
            params,
            body,
            type_params,
            renders_type,
            is_async,
        ));
        Some(self.set_location(start, end, node))
    }

    // -----------------------------------------------------------------------
    // parseComponentParametersFlow — 332 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse the `( ... )` parameter list of a `component` declaration into
    /// `param_list`. Returns false if an error was reported. Port of
    /// `JSParserImpl::parseComponentParametersFlow` (flow.cpp:332-373).
    fn parse_component_parameters_flow(
        &mut self,
        param: Param,
        param_list: &mut Vec<&'gc Node<'gc>>,
    ) -> bool {
        // C++ 335-338.
        debug_assert!(self.check(TokenKind::l_paren));
        let lparen_loc = self.advance(GrammarContext::AllowRegExp).start;

        // C++ 340-360.
        while !self.check(TokenKind::r_paren) {
            if self.check(TokenKind::dotdotdot) {
                // C++ 341-348: a BindingRestElement.
                let Some(rest_elem) = self.parse_binding_rest_element(param)
                else {
                    return false;
                };
                param_list.push(rest_elem);
                self.check_and_eat(TokenKind::comma, GrammarContext::Type);
                break;
            }

            // C++ 351-356: a ComponentParameter.
            let Some(param_node) = self.parse_component_parameter_flow(param)
            else {
                return false;
            };
            param_list.push(param_node);

            // C++ 358-359.
            if !self.check_and_eat(TokenKind::comma, GrammarContext::AllowRegExp)
            {
                break;
            }
        }

        // C++ 362-372.
        self.eat_at(
            TokenKind::r_paren,
            GrammarContext::AllowRegExp,
            " at end of component parameter list",
            Some("start of component parameter list"),
            lparen_loc,
        )
    }

    // -----------------------------------------------------------------------
    // parseComponentParameterFlow — 375 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse one `component` parameter (the three shapes:
    /// `StringLiteral as BindingElement`, `IdentifierName`,
    /// `IdentifierName as BindingElement`). Port of
    /// `JSParserImpl::parseComponentParameterFlow` (flow.cpp:375-489).
    fn parse_component_parameter_flow(
        &mut self,
        param: Param,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 382.
        let param_start = self.cur_start();

        // C++ 384-409: `StringLiteral as BindingElement` — the local via `as`
        // is required.
        if self.check(TokenKind::string_literal) {
            let str_range = self.cur_range();
            let value = self.lexer.token().get_string_literal();
            let name_node = Node::StringLiteral(StringLiteral::new(
                NodeMetadata::new(self.dummy_range()),
                value,
            ));
            let name_elem =
                self.set_location(str_range.start, str_range.end, name_node);
            self.advance(GrammarContext::AllowRegExp);

            // C++ 393-398.
            if !self.check_name(b"as") {
                self.error_at(
                    name_elem.range(),
                    "string literal names require a local via `as`",
                );
                return None;
            }
            self.advance(GrammarContext::AllowRegExp);

            // C++ 400-408.
            let binding = self.parse_binding_element(Param::default())?;
            let node = Node::ComponentParameter(ComponentParameter::new(
                NodeMetadata::new(self.dummy_range()),
                name_elem,
                binding,
                false,
            ));
            return Some(self.set_location(
                param_start,
                self.lexer.prev_token_end(),
                node,
            ));
        }

        // C++ 411-481: `IdentifierName` with optional `as BindingElement` or
        // shorthand `IdentifierName?: T = init`.
        if self.check(TokenKind::identifier) || self.lexer.token().is_res_word()
        {
            let id = self.lexer.token().get_res_word_or_identifier();
            let ident_rng = self.cur_range();
            let ident_kind = self.lexer.token().kind();
            let name_node = Node::Identifier(Identifier::new(
                NodeMetadata::new(self.dummy_range()),
                id,
                None,
                false,
            ));
            let name_elem =
                self.set_location(ident_rng.start, ident_rng.end, name_node);
            self.advance(GrammarContext::AllowRegExp);

            // C++ 421-433: `IdentifierName as BindingElement`.
            if self.check_name(b"as") {
                self.advance(GrammarContext::AllowRegExp);
                let binding = self.parse_binding_element(Param::default())?;
                let node = Node::ComponentParameter(ComponentParameter::new(
                    NodeMetadata::new(self.dummy_range()),
                    name_elem,
                    binding,
                    false,
                ));
                return Some(self.set_location(
                    param_start,
                    self.lexer.prev_token_end(),
                    node,
                ));
            }

            // C++ 435-437: validate the shorthand name as a local binding.
            let id_bytes = self.gc.ctx().atom_table.bytes(id).to_owned();
            if !self.validate_binding_identifier(
                ident_rng,
                &id_bytes,
                ident_kind,
            ) {
                self.error_at(ident_rng, "Invalid local name for component");
            }

            // C++ 439-447: `IdentifierName?` optional marker.
            let mut type_annot: Option<&'gc Node<'gc>> = None;
            let mut optional = false;
            if self.check(TokenKind::question) {
                optional = true;
                self.advance(GrammarContext::Type);
            }

            // C++ 449-457: `: TypeParam`.
            if self.check(TokenKind::colon) {
                let annot_start = self.advance(GrammarContext::Type).start;
                type_annot = Some(self.parse_type_annotation(
                    Some(annot_start),
                    AllowAnonFunctionType::Yes,
                )?);
            }

            // C++ 459-462: the shorthand local is an Identifier (carrying the
            // optional marker and type annotation).
            let elem_node = Node::Identifier(Identifier::new(
                NodeMetadata::new(self.dummy_range()),
                id,
                type_annot,
                optional,
            ));
            let elem = self.set_location(
                ident_rng.start,
                self.lexer.prev_token_end(),
                elem_node,
            );

            // C++ 465-474: `= init`.
            let local_elem = if self.check(TokenKind::equal) {
                self.parse_binding_initializer(param, elem)?
            } else {
                elem
            };

            // C++ 476-480.
            let node = Node::ComponentParameter(ComponentParameter::new(
                NodeMetadata::new(self.dummy_range()),
                name_elem,
                local_elem,
                true,
            ));
            return Some(self.set_location(
                param_start,
                self.lexer.prev_token_end(),
                node,
            ));
        }

        // C++ 483-486.
        self.error_at_loc(
            self.cur_start(),
            "identifier or string literal expected in component parameter name",
        );
        None
    }

    // -----------------------------------------------------------------------
    // parseRenderTypeOperator — 489 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse the `renders`/`renders?`/`renders*` operator keyword, consuming
    /// it (and the trailing `?`/`*`), and return the interned operator label.
    /// Port of `JSParserImpl::parseRenderTypeOperator` (flow.cpp:489-523).
    pub(in crate::js) fn parse_render_type_operator(
        &mut self,
    ) -> Option<ast::node_child::NodeLabel> {
        // C++ 490.
        debug_assert!(self.check_name(b"renders"));
        // C++ 491-520: the `checkFollowingCharacter` calls must run with the
        // `renders` ident still the current token, so we can't advance until
        // after them (the ordering trap).
        let operator: &[u8];
        if self.lexer.check_following_character(b'?') {
            // C++ 492-502.
            let start = self.advance(GrammarContext::Type).start;
            if !self.eat_at(
                TokenKind::question,
                GrammarContext::Type,
                " in render type annotation",
                Some("start of render type"),
                start,
            ) {
                return None;
            }
            operator = b"renders?";
        } else if self.lexer.check_following_character(b'*') {
            // C++ 503-513.
            let start = self.advance(GrammarContext::Type).start;
            if !self.eat_at(
                TokenKind::star,
                GrammarContext::Type,
                " in render type annotation",
                Some("start of render type"),
                start,
            ) {
                return None;
            }
            operator = b"renders*";
        } else {
            // C++ 514-520: normal `renders`, but we must still eat the token.
            self.advance(GrammarContext::Type);
            operator = b"renders";
        }
        Some(self.gc.ctx().atom_table.atom_bytes(operator))
    }

    // -----------------------------------------------------------------------
    // parseComponentRenderTypeFlow — 525 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a `renders <Type>` render-type annotation. Port of
    /// `JSParserImpl::parseComponentRenderTypeFlow` (flow.cpp:525-553).
    ///
    /// \param component_type whether this is for a component TYPE annotation
    ///   (which uses `parsePrefixTypeAnnotationFlow` for the body) rather than
    ///   a component DECLARATION (which uses `parseTypeAnnotationFlow`). This
    ///   precedence asymmetry is intentional — see the C++ comment.
    pub(in crate::js) fn parse_component_render_type_flow(
        &mut self,
        component_type: bool,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 527-528.
        let annot_start = self.cur_start();
        let operator = self.parse_render_type_operator();
        // C++ 530-546: because unions have higher precedence than renders, the
        // body precedence differs between component types and declarations.
        let body = if component_type {
            self.parse_prefix_type_annotation_flow()
        } else {
            self.parse_type_annotation_flow(None, AllowAnonFunctionType::Yes)
        };
        // C++ 547-548.
        let (Some(body), Some(operator)) = (body, operator) else {
            return None;
        };
        // C++ 549-552.
        let node = Node::TypeOperator(ast::node::TypeOperator::new(
            NodeMetadata::new(self.dummy_range()),
            operator,
            body,
        ));
        Some(self.set_location(annot_start, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseHookDeclarationFlow — 780 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a `hook` declaration, with the cursor at `hook` and `start` at the
    /// start of the declaration. Port of
    /// `JSParserImpl::parseHookDeclarationFlow` (flow.cpp:780-858).
    pub(in crate::js) fn parse_hook_declaration_flow(
        &mut self,
        start: SMLoc,
        is_async: bool,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 784-785.
        debug_assert!(self.check_name(b"hook"));
        self.advance(GrammarContext::AllowRegExp);

        // C++ 787-795: hooks always require a name identifier.
        let Some(id) = self.parse_binding_identifier(Param::default()) else {
            // C++ 791-794: errorExpected(identifier, "after 'hook'",
            // "location of 'hook'", start).
            self.error_expected_msg(
                "'identifier' expected after 'hook'",
                Some("location of 'hook'"),
                Some(start),
            );
            return None;
        };

        // C++ 797-804.
        let mut type_params: Option<&'gc Node<'gc>> = None;
        if self.check(TokenKind::less) {
            type_params = Some(self.parse_type_params_flow()?);
        }

        // C++ 806-812.
        if !self.need_at(
            TokenKind::l_paren,
            " at start of hook parameter list",
            Some("hook declaration starts here"),
            start,
        ) {
            return None;
        }

        // C++ 814-851: paramAwait_ = isAsync spans the params AND the body.
        let _guard_await = self.save_param_await(is_async);

        let mut param_list: Vec<&'gc Node<'gc>> = Vec::new();
        // C++ 818-819: hooks use ORDINARY formal parameters, not component
        // parameters.
        if !self.parse_formal_parameters(Param::default(), &mut param_list) {
            return None;
        }
        let params = NodeList::from_iter(self.gc, param_list);

        // C++ 821-835: an optional `: ReturnType`; `%checks` predicates are
        // unsupported in hooks.
        let mut return_type: Option<&'gc Node<'gc>> = None;
        if self.check(TokenKind::colon) {
            let annot_start = self.advance(GrammarContext::Type).start;
            if !self.check_name(b"checks") {
                return_type = Some(self.parse_return_type_annotation_flow(
                    Some(annot_start),
                    AllowAnonFunctionType::Yes,
                )?);
            } else {
                self.error_at_loc(
                    self.cur_start(),
                    "checks predicates unsupported with hooks",
                );
                return None;
            }
        }

        // C++ 837-843.
        if !self.need_at(
            TokenKind::l_brace,
            " in hook declaration",
            Some("start of hook declaration"),
            start,
        ) {
            return None;
        }

        // C++ 845-851.
        let body = self.parse_function_body(
            Param::default(),
            false,
            false,
            is_async,
            GrammarContext::AllowRegExp,
            true,
        )?;

        // C++ 853-857.
        let end = body.range().end;
        let node = Node::HookDeclaration(HookDeclaration::new(
            NodeMetadata::new(self.dummy_range()),
            id,
            params,
            body,
            type_params,
            return_type,
            is_async,
        ));
        Some(self.set_location(start, end, node))
    }

    // -----------------------------------------------------------------------
    // checkRecordDeclarationFlow — 1618 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Whether the current token starts a `record` declaration. Port of
    /// `JSParserImpl::checkRecordDeclarationFlow` (flow.cpp:1618-1628). MUST be
    /// idempotent (it is called from the parse side, which reparses the token
    /// on a match), so the lookahead passes no expected token.
    pub(in crate::js) fn check_record_declaration_flow(&mut self) -> bool {
        // C++ 1619-1620.
        if !self.check_name(b"record") {
            return false;
        }
        // C++ 1622-1627: don't pass an `expectedToken` so we don't advance on a
        // match (lets `parseRecordDeclarationFlow` reparse the token), and so
        // this stays idempotent.
        self.lexer.lookahead1::<true>(None) == Some(TokenKind::identifier)
    }

    // -----------------------------------------------------------------------
    // parseRecordDeclarationFlow — 1630 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a `record` declaration, with the cursor at `record` and `start` at
    /// the start of the declaration. Port of
    /// `JSParserImpl::parseRecordDeclarationFlow` (flow.cpp:1630-1901).
    pub(in crate::js) fn parse_record_declaration_flow(
        &mut self,
        start: SMLoc,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 1631-1632.
        debug_assert!(self.check_name(b"record"));
        self.advance(GrammarContext::AllowRegExp);

        // C++ 1634-1640.
        let Some(id) = self.parse_binding_identifier(Param::default()) else {
            // C++ 1637-1639: errorExpected(identifier, "after 'record'",
            // "location of 'record'", start).
            self.error_expected_msg(
                "'identifier' expected after 'record'",
                Some("location of 'record'"),
                Some(start),
            );
            return None;
        };

        // C++ 1642-1649.
        let mut type_params: Option<&'gc Node<'gc>> = None;
        if self.check(TokenKind::less) {
            type_params = Some(self.parse_type_params_flow()?);
        }

        // C++ 1651-1667: an optional `implements` clause.
        let mut implements_list: Vec<&'gc Node<'gc>> = Vec::new();
        if self.check_name(b"implements") {
            self.advance(GrammarContext::Type);
            // C++ 1655-1666: a do-while.
            loop {
                if !self.need_at(
                    TokenKind::identifier,
                    " in record 'implements'",
                    Some("start of declaration"),
                    start,
                ) {
                    return None;
                }
                let implements =
                    self.parse_record_declaration_implements_flow()?;
                implements_list.push(implements);
                if !self.check_and_eat(TokenKind::comma, GrammarContext::Type) {
                    break;
                }
            }
        }

        // C++ 1669-1675.
        if !self.need_at(
            TokenKind::l_brace,
            " in record declaration",
            Some("start of record declaration"),
            start,
        ) {
            return None;
        }

        // C++ 1677-1679.
        let body_start = self.advance(GrammarContext::AllowRegExp).start;
        let mut body_elements: Vec<&'gc Node<'gc>> = Vec::new();

        // C++ 1681-1879.
        while !self.check(TokenKind::r_brace) {
            // C++ 1682-1689: errorExpected(r_brace, "in record body",
            // "start of record body", bodyStart).
            if self.check(TokenKind::eof) {
                self.error_expected_msg(
                    "'}' expected in record body",
                    Some("start of record body"),
                    Some(body_start),
                );
                return None;
            }

            // C++ 1691-1706: `isModifierKeyword` — distinguish a `static`/
            // `async` modifier keyword from a property name by looking at the
            // token that follows. If it is `:`/`<`/`(`/`}`/eof the keyword is
            // itself the property name, not a modifier.
            let prop_start_loc = self.cur_start();

            // C++ 1710-1721: modifiers `static`, `async`, generator (`*`).
            let mut is_static = false;
            if self.check_name(b"static") && self.is_record_modifier_keyword() {
                self.advance(GrammarContext::AllowRegExp);
                is_static = true;
            }
            let mut is_async = false;
            if self.check_name(b"async") && self.is_record_modifier_keyword() {
                self.advance(GrammarContext::AllowRegExp);
                is_async = true;
            }
            let is_generator =
                self.check_and_eat(TokenKind::star, GrammarContext::AllowRegExp);

            // C++ 1723-1730.
            if self.check(TokenKind::l_square) {
                self.error_at_loc(
                    self.cur_start(),
                    "records do not support computed properties",
                );
                return None;
            }
            if self.check(TokenKind::private_identifier) {
                self.error_at_loc(
                    self.cur_start(),
                    "records do not support private elements",
                );
                return None;
            }

            // C++ 1731-1742.
            let key = self.parse_property_name()?;
            if let Node::Identifier(key_ident) = key {
                let constructor_atom =
                    self.gc.ctx().atom_table.atom_bytes(b"constructor");
                let prototype_atom =
                    self.gc.ctx().atom_table.atom_bytes(b"prototype");
                if key_ident.name.get() == constructor_atom
                    || (is_static && key_ident.name.get() == prototype_atom)
                {
                    self.error_at(key.range(), "invalid record property name");
                    return None;
                }
            }

            if self.check(TokenKind::colon) {
                // C++ 1744-1797: Property.
                if is_async || is_generator {
                    self.error_at(
                        key.range(),
                        "invalid async/generator modifier for record property, expected a method definition",
                    );
                    return None;
                }
                // C++ 1752-1756: eat the colon (in Type context).
                let annot_start = self.advance(GrammarContext::Type).start;
                let type_annot = self.parse_type_annotation_flow(
                    Some(annot_start),
                    AllowAnonFunctionType::Yes,
                )?;

                // C++ 1758-1764. parseAssignmentExpression() defaults to
                // param=ParamIn (JSParserImpl.h:1132) — the `[In]` grammar
                // parameter must be set so `in` is a relational operator inside
                // a record property initializer.
                let mut value: Option<&'gc Node<'gc>> = None;
                if self.check_and_eat(TokenKind::equal, GrammarContext::AllowRegExp)
                {
                    value = Some(self.parse_assignment_expression(
                        PARAM_IN,
                        false,
                        AllowTypedArrowFunction::Yes,
                        CoverTypedParameters::Yes,
                        None,
                    )?);
                }

                // C++ 1766-1785.
                let prop = if is_static {
                    // C++ 1767-1778: a static record property requires an
                    // initializer.
                    let Some(value) = value else {
                        // C++ 1769-1772: error at key->getEndLoc().
                        self.error_at_loc(
                            key.range().end,
                            "static record properties must have an initializer",
                        );
                        return None;
                    };
                    let node = Node::RecordDeclarationStaticProperty(
                        RecordDeclarationStaticProperty::new(
                            NodeMetadata::new(self.dummy_range()),
                            key,
                            type_annot,
                            value,
                        ),
                    );
                    self.set_location(prop_start_loc, value.range().end, node)
                } else {
                    // C++ 1779-1784: the end loc is the initializer if present,
                    // else the type annotation.
                    let end = value
                        .map(|v| v.range().end)
                        .unwrap_or_else(|| type_annot.range().end);
                    let node = Node::RecordDeclarationProperty(
                        RecordDeclarationProperty::new(
                            NodeMetadata::new(self.dummy_range()),
                            key,
                            type_annot,
                            value,
                        ),
                    );
                    self.set_location(prop_start_loc, end, node)
                };
                body_elements.push(prop);

                // C++ 1788-1797: a trailing `,` is required unless `}`/eof
                // follows.
                if !self.check2(TokenKind::r_brace, TokenKind::eof)
                    && !self.eat_at(
                        TokenKind::comma,
                        GrammarContext::AllowRegExp,
                        " after property",
                        Some("start of property"),
                        prop_start_loc,
                    )
                {
                    return None;
                }
            } else if self.check(TokenKind::l_paren) || self.check(TokenKind::less)
            {
                // C++ 1798-1872: Method.
                let mut method_type_params: Option<&'gc Node<'gc>> = None;
                if self.check(TokenKind::less) {
                    method_type_params = Some(self.parse_type_params_flow()?);
                }

                // C++ 1808-1815: errorExpected(l_paren, "in method
                // parameters", "start of method", propStartLoc).
                if !self.check(TokenKind::l_paren) {
                    self.error_expected_msg(
                        "'(' expected in method parameters",
                        Some("start of method"),
                        Some(prop_start_loc),
                    );
                    return None;
                }

                // C++ 1817-1819.
                let mut param_list: Vec<&'gc Node<'gc>> = Vec::new();
                if !self
                    .parse_formal_parameters(Param::default(), &mut param_list)
                {
                    return None;
                }
                let params = NodeList::from_iter(self.gc, param_list);

                // C++ 1821-1828: an optional `: ReturnType`.
                let mut return_type: Option<&'gc Node<'gc>> = None;
                if self.check(TokenKind::colon) {
                    let annot_start = self.advance(GrammarContext::Type).start;
                    return_type = Some(self.parse_return_type_annotation_flow(
                        Some(annot_start),
                        AllowAnonFunctionType::Yes,
                    )?);
                }

                // C++ 1830-1837: errorExpected(l_brace, "in method body",
                // "start of method", propStartLoc).
                if !self.check(TokenKind::l_brace) {
                    self.error_expected_msg(
                        "'{' expected in method body",
                        Some("start of method"),
                        Some(prop_start_loc),
                    );
                    return None;
                }

                // C++ 1839-1852: paramYield_ = isGenerator, paramAwait_ =
                // isAsync around the body (mirrors the class-method pattern).
                let body = {
                    let _guard_yield = self.save_param_yield(is_generator);
                    let _guard_await = self.save_param_await(is_async);
                    self.parse_function_body(
                        Param::default(),
                        false,
                        is_generator,
                        is_async,
                        GrammarContext::AllowRegExp,
                        true,
                    )?
                };
                let body_end = body.range().end;

                // C++ 1854-1865.
                let func = Node::FunctionExpression(FunctionExpression::new(
                    NodeMetadata::new(self.dummy_range()),
                    None,
                    params,
                    body,
                    method_type_params,
                    return_type,
                    None,
                    is_generator,
                    is_async,
                ));
                let func_expr =
                    self.set_location(prop_start_loc, body_end, func);

                // C++ 1867-1872.
                let method_ident =
                    self.gc.ctx().atom_table.atom_bytes(b"method");
                let method = Node::MethodDefinition(MethodDefinition::new(
                    NodeMetadata::new(self.dummy_range()),
                    key,
                    func_expr,
                    method_ident,
                    false,
                    is_static,
                    NodeList::empty(),
                ));
                body_elements
                    .push(self.set_location(prop_start_loc, body_end, method));
            } else {
                // C++ 1873-1878.
                self.error_at_loc(
                    key.range().end,
                    "expected ':' for property, '(' for method, or '<' for method with type parameters",
                );
                return None;
            }
        }

        // C++ 1881-1888.
        if !self.eat_at(
            TokenKind::r_brace,
            GrammarContext::AllowRegExp,
            " in record body",
            Some("start of record body"),
            body_start,
        ) {
            return None;
        }

        // C++ 1890-1894.
        let body_node = Node::RecordDeclarationBody(RecordDeclarationBody::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, body_elements),
        ));
        let body = self.set_location(
            body_start,
            self.lexer.prev_token_end(),
            body_node,
        );

        // C++ 1896-1900.
        let end = body.range().end;
        let node = Node::RecordDeclaration(RecordDeclaration::new(
            NodeMetadata::new(self.dummy_range()),
            id,
            type_params,
            NodeList::from_iter(self.gc, implements_list),
            body,
        ));
        Some(self.set_location(start, end, node))
    }

    /// The `isModifierKeyword` lambda from `parseRecordDeclarationFlow`
    /// (flow.cpp:1691-1706): a `static`/`async` token is a modifier (rather
    /// than a property name) only if the FOLLOWING token is not one of
    /// `:`/`<`/`(`/`}`/eof. Idempotent (`lookahead1(None)`).
    fn is_record_modifier_keyword(&mut self) -> bool {
        let Some(next) = self.lexer.lookahead1::<true>(None) else {
            return false;
        };
        !matches!(
            next,
            TokenKind::colon // token: T
                | TokenKind::less // token<T>() {}
                | TokenKind::l_paren // token() {}
                | TokenKind::r_brace // end of record
                | TokenKind::eof // end of file
        )
    }

    // -----------------------------------------------------------------------
    // parseRecordDeclarationImplementsFlow — 1903 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse one entry of a `record` `implements` clause: an Identifier with
    /// optional `<typeArgs>`, wrapped in a `RecordDeclarationImplements`. Port
    /// of `JSParserImpl::parseRecordDeclarationImplementsFlow`
    /// (flow.cpp:1903-1927).
    fn parse_record_declaration_implements_flow(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 1905-1906.
        debug_assert!(self.check(TokenKind::identifier));
        let start = self.cur_start();

        // C++ 1908-1913.
        let id_range = self.cur_range();
        let id_node = Node::Identifier(Identifier::new(
            NodeMetadata::new(self.dummy_range()),
            self.lexer.token().get_identifier(),
            None,
            false,
        ));
        let id = self.set_location(id_range.start, id_range.end, id_node);
        self.advance(GrammarContext::Type);

        // C++ 1915-1921.
        let mut type_args: Option<&'gc Node<'gc>> = None;
        if self.check(TokenKind::less) {
            type_args = Some(self.parse_type_args_flow(GrammarContext::Type)?);
        }

        // C++ 1923-1926.
        let node = Node::RecordDeclarationImplements(
            RecordDeclarationImplements::new(
                NodeMetadata::new(self.dummy_range()),
                id,
                type_args,
            ),
        );
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseTypeAliasFlow — 1981 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a type alias, with `type` already consumed and `start` at the
    /// start of the declaration. Port of `JSParserImpl::parseTypeAliasFlow`
    /// (flow.cpp:1981-2071). All four `TypeAliasKind` paths are implemented;
    /// the `Declare`/`DeclareOpaque` kinds become reachable with the
    /// `declare` statement routing (parseDeclareFLow) in P6.
    pub(super) fn parse_type_alias_flow(
        &mut self,
        start: SMLoc,
        kind: TypeAliasKind,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 1984-1987.
        if !self.need_at(
            TokenKind::identifier,
            " in type alias",
            Some("start of type alias"),
            start,
        ) {
            return None;
        }

        // C++ 1988-1993.
        let id_range = self.cur_range();
        let id_node = Node::Identifier(Identifier::new(
            NodeMetadata::new(self.dummy_range()),
            self.lexer.token().get_identifier(),
            None,
            false,
        ));
        let id = self.set_location(id_range.start, id_range.end, id_node);
        self.advance(GrammarContext::Type);

        // C++ 1995-2002.
        let mut type_params: Option<&'gc Node<'gc>> = None;
        if self.check(TokenKind::less) {
            type_params = Some(self.parse_type_params_flow()?);
        }

        // C++ 2004-2026: the Opaque/DeclareOpaque `super`/`extends` bounds
        // and the legacy `: Supertype` (only when neither bound was given).
        let mut lower_bound: Option<&'gc Node<'gc>> = None;
        let mut upper_bound: Option<&'gc Node<'gc>> = None;
        let mut legacy_supertype: Option<&'gc Node<'gc>> = None;
        if kind == TypeAliasKind::Opaque || kind == TypeAliasKind::DeclareOpaque
        {
            // C++ 2007-2012: the lower bound is a UNION type annotation, not
            // a full one.
            if self.check_and_eat(TokenKind::rw_super, GrammarContext::Type) {
                lower_bound = Some(self.parse_union_type_annotation_flow()?);
            }
            // C++ 2013-2018.
            if self.check_and_eat(TokenKind::rw_extends, GrammarContext::Type)
            {
                upper_bound = Some(self.parse_type_annotation_flow(
                    None,
                    AllowAnonFunctionType::Yes,
                )?);
            }
            // C++ 2019-2026.
            if lower_bound.is_none()
                && upper_bound.is_none()
                && self.check_and_eat(TokenKind::colon, GrammarContext::Type)
            {
                legacy_supertype = Some(self.parse_type_annotation_flow(
                    None,
                    AllowAnonFunctionType::Yes,
                )?);
            }
        }

        // C++ 2028-2042: DeclareOpaque has no `= T` right side.
        let mut right: Option<&'gc Node<'gc>> = None;
        if kind != TypeAliasKind::DeclareOpaque {
            if !self.eat_at(
                TokenKind::equal,
                GrammarContext::Type,
                " in type alias",
                Some("start of type alias"),
                start,
            ) {
                return None;
            }
            right = Some(
                self.parse_type_annotation_flow(
                    None,
                    AllowAnonFunctionType::Yes,
                )?,
            );
        }

        // C++ 2043-2044.
        if !self.eat_semi(false) {
            return None;
        }

        // C++ 2046-2070.
        let end = self.lexer.prev_token_end();
        match kind {
            // C++ 2046-2052.
            TypeAliasKind::DeclareOpaque => {
                let node = Node::DeclareOpaqueType(DeclareOpaqueType::new(
                    NodeMetadata::new(self.dummy_range()),
                    id,
                    type_params,
                    right, // impltype (always None here)
                    lower_bound,
                    upper_bound,
                    legacy_supertype,
                ));
                Some(self.set_location(start, end, node))
            }
            // C++ 2053-2058.
            TypeAliasKind::Declare => {
                let node = Node::DeclareTypeAlias(DeclareTypeAlias::new(
                    NodeMetadata::new(self.dummy_range()),
                    id,
                    type_params,
                    right.expect("non-DeclareOpaque alias has a right side"),
                ));
                Some(self.set_location(start, end, node))
            }
            // C++ 2059-2065.
            TypeAliasKind::Opaque => {
                let node = Node::OpaqueType(OpaqueType::new(
                    NodeMetadata::new(self.dummy_range()),
                    id,
                    type_params,
                    // impltype
                    right.expect("non-DeclareOpaque alias has a right side"),
                    lower_bound,
                    upper_bound,
                    legacy_supertype,
                ));
                Some(self.set_location(start, end, node))
            }
            // C++ 2066-2070.
            TypeAliasKind::None => {
                let node = Node::TypeAlias(TypeAlias::new(
                    NodeMetadata::new(self.dummy_range()),
                    id,
                    type_params,
                    right.expect("non-DeclareOpaque alias has a right side"),
                ));
                Some(self.set_location(start, end, node))
            }
        }
    }

    // -----------------------------------------------------------------------
    // parseInterfaceDeclarationFlow — 2073 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse an interface declaration, with the current token at the
    /// `interface` keyword (`rw_interface` in strict mode, a plain identifier
    /// spelled "interface" in loose mode). Port of
    /// `JSParserImpl::parseInterfaceDeclarationFlow` (flow.cpp:2073-2118).
    ///
    /// \param declare_start if `Some`, this is a `declare interface` and the
    ///   resulting `DeclareInterface` node spans from it (the `declare`
    ///   statement routing that passes `Some` lands in P6).
    pub(super) fn parse_interface_declaration_flow(
        &mut self,
        declare_start: Option<SMLoc>,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 2075.
        debug_assert!(
            self.check(TokenKind::rw_interface)
                || self.check_name(b"interface"),
            "must be at 'interface'"
        );
        // C++ 2076.
        let start = self.advance(GrammarContext::Type).start;

        // C++ 2078-2084.
        if !self.need_at(
            TokenKind::identifier,
            " in interface declaration",
            Some("start of interface"),
            start,
        ) {
            return None;
        }

        // C++ 2086-2091.
        let id_range = self.cur_range();
        let id_node = Node::Identifier(Identifier::new(
            NodeMetadata::new(self.dummy_range()),
            self.lexer.token().get_identifier(),
            None,
            false,
        ));
        let id = self.set_location(id_range.start, id_range.end, id_node);
        self.advance(GrammarContext::Type);

        // C++ 2093-2099.
        let mut type_params: Option<&'gc Node<'gc>> = None;
        if self.check(TokenKind::less) {
            type_params = Some(self.parse_type_params_flow()?);
        }

        // C++ 2101-2104.
        let mut extends: Vec<&'gc Node<'gc>> = Vec::new();
        let body = self.parse_interface_tail_flow(start, &mut extends)?;

        // C++ 2106-2117: the end location is the body node's end.
        let end = body.metadata().range.get().end;
        if let Some(declare_start) = declare_start {
            let node = Node::DeclareInterface(DeclareInterface::new(
                NodeMetadata::new(self.dummy_range()),
                id,
                type_params,
                NodeList::from_iter(self.gc, extends),
                body,
            ));
            Some(self.set_location(declare_start, end, node))
        } else {
            let node = Node::InterfaceDeclaration(InterfaceDeclaration::new(
                NodeMetadata::new(self.dummy_range()),
                id,
                type_params,
                NodeList::from_iter(self.gc, extends),
                body,
            ));
            Some(self.set_location(start, end, node))
        }
    }

    /// Parse the `extends`-clause-and-body tail shared by interface
    /// declarations and `interface` type annotations, pushing the
    /// `InterfaceExtends` entries into `extends` and returning the
    /// `ObjectTypeAnnotation` body. Port of
    /// `JSParserImpl::parseInterfaceTailFlow` (flow.cpp:2120-2141). `start`
    /// is the interface's start location, passed through unchanged from C++
    /// as the real `whatLoc`/`what="location of interface"` hint on both
    /// `need` calls below (cpp:2125-2129, 2136).
    pub(super) fn parse_interface_tail_flow(
        &mut self,
        start: SMLoc,
        extends: &mut Vec<&'gc Node<'gc>>,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 2123: a bare `checkAndEat` — the default GrammarContext
        // (AllowRegExp), NOT Type; deliberate.
        if self.check_and_eat(TokenKind::rw_extends, GrammarContext::AllowRegExp)
        {
            // C++ 2124-2134: a do-while.
            loop {
                if !self.need_at(
                    TokenKind::identifier,
                    " in extends clause",
                    Some("location of interface"),
                    start,
                ) {
                    return None;
                }
                if !self.parse_interface_extends(extends) {
                    return None;
                }
                if !self.check_and_eat(TokenKind::comma, GrammarContext::Type)
                {
                    break;
                }
            }
        }

        // C++ 2136-2137.
        if !self.need_at(
            TokenKind::l_brace,
            " in interface",
            Some("location of interface"),
            start,
        ) {
            return None;
        }

        // C++ 2139-2140: spread properties are not allowed in interface
        // bodies.
        self.parse_object_type_annotation_flow(
            AllowProtoProperty::No,
            AllowStaticProperty::No,
            AllowSpreadProperty::No,
        )
    }

    /// Parse one entry of an interface `extends` clause: a generic type
    /// reference, unwrapped into an `InterfaceExtends` node spanning the same
    /// range, pushed onto `extends`. Returns false if an error was reported.
    /// Port of `JSParserImpl::parseInterfaceExtends` (flow.cpp:2143-2157; the
    /// C++ also takes an unused start location).
    fn parse_interface_extends(
        &mut self,
        extends: &mut Vec<&'gc Node<'gc>>,
    ) -> bool {
        // C++ 2146.
        debug_assert!(self.check(TokenKind::identifier));
        // C++ 2147-2150.
        let Some(generic) = self.parse_generic_type_flow() else {
            return false;
        };
        let Node::GenericTypeAnnotation(g) = generic else {
            unreachable!("parse_generic_type_flow returns a GenericTypeAnnotation")
        };
        // C++ 2151-2155: the InterfaceExtends node spans exactly the generic.
        let range = generic.metadata().range.get();
        let node = Node::InterfaceExtends(InterfaceExtends::new(
            NodeMetadata::new(self.dummy_range()),
            g.id,
            g.type_parameters,
        ));
        extends.push(self.set_location(range.start, range.end, node));
        true
    }

    // -----------------------------------------------------------------------
    // parseDeclareFLow — 95 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a `declare ...` statement, with `declare` already consumed and
    /// `start` at the `declare` keyword. Port of `JSParserImpl::parseDeclareFLow`
    /// (flow.cpp:95-193).
    pub(in crate::js) fn parse_declare_flow(
        &mut self,
        start: SMLoc,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 96-98: `declare type`.
        if self.check_name(b"type") {
            self.advance(GrammarContext::AllowRegExp);
            return self.parse_type_alias_flow(start, TypeAliasKind::Declare);
        }
        // C++ 99-106: `declare opaque type`.
        if self.check_name(b"opaque") {
            self.advance(GrammarContext::AllowRegExp);
            if !self.check_name(b"type") {
                // Point location, NOT the current token's range: C++
                // (flow.cpp:100-101) calls `error(tok_->getStartLoc(), ...)`
                // — the `error(SMLoc, Twine)` overload.
                self.error_at_loc(
                    self.cur_start(),
                    "'type' required in opaque type declaration",
                );
                return None;
            }
            self.advance(GrammarContext::Type);
            return self
                .parse_type_alias_flow(start, TypeAliasKind::DeclareOpaque);
        }
        // C++ 107-109: `declare interface`.
        if self.check(TokenKind::rw_interface) || self.check_name(b"interface") {
            return self.parse_interface_declaration_flow(Some(start));
        }
        // C++ 110-112: `declare class`.
        if self.check(TokenKind::rw_class) {
            return self.parse_declare_class_flow(start);
        }
        // C++ 113-115: `declare function`.
        if self.check(TokenKind::rw_function) {
            return self.parse_declare_function_flow(start);
        }

        // C++ 117-119: `declare hook`.
        if self.parse_flow_component_syntax()
            && self.check_hook_declaration_flow()
        {
            return self.parse_declare_hook_flow(start);
        }

        // C++ 121-129: `declare async hook` (an error, then parse as hook).
        if self.parse_flow_component_syntax()
            && self.check_unescaped_name(b"async")
            && self.check_async_hook_flow()
        {
            // Point location, NOT the current token's range: C++
            // (flow.cpp:122-124) calls `error(tok_->getStartLoc(), ...)` —
            // the `error(SMLoc, Twine)` overload.
            self.error_at_loc(
                self.cur_start(),
                "`async` is not supported for declared hooks. \
                 Use `declare hook` instead.",
            );
            self.advance(GrammarContext::AllowRegExp); // consume 'async'
            return self.parse_declare_hook_flow(start);
        }

        // C++ 131-139: `declare async component` (an error, then component).
        if self.parse_flow_component_syntax()
            && self.check_unescaped_name(b"async")
            && self.check_async_component_flow()
        {
            // Point location, NOT the current token's range: C++
            // (flow.cpp:132-134) calls `error(tok_->getStartLoc(), ...)` —
            // the `error(SMLoc, Twine)` overload.
            self.error_at_loc(
                self.cur_start(),
                "`async` is not supported for declared components. \
                 Use `declare component` instead.",
            );
            self.advance(GrammarContext::AllowRegExp); // consume 'async'
            return self
                .parse_component_declaration_flow(start, true, false);
        }

        // C++ 141-144: `declare component`.
        if self.parse_flow_component_syntax()
            && self.check_component_declaration_flow()
        {
            return self
                .parse_component_declaration_flow(start, true, false);
        }
        // C++ 145-147: `declare enum`.
        if self.check(TokenKind::rw_enum) {
            return self.parse_enum_declaration_flow(start, true);
        }
        // C++ 148-150: `declare module`.
        if self.check_name(b"module") {
            return self.parse_declare_module_flow(start);
        }
        // C++ 151-153: `declare namespace`.
        if self.check_name(b"namespace") {
            return self.parse_declare_namespace_flow(start);
        }

        // C++ 154-177: `declare var`/`const`/`let`. NOTE the var-kind advance
        // here is the DEFAULT GrammarContext (AllowRegExp), unlike the `Type`
        // advance in parseDeclareExportFlow.
        if self.check2(TokenKind::rw_var, TokenKind::rw_const)
            || self.check_name(b"let")
        {
            let kind = self.lexer.token().get_res_word_or_identifier();
            self.advance(GrammarContext::AllowRegExp);
            let Some(id) = self.parse_binding_identifier(Param::default())
            else {
                // C++ 158-165: errorExpected(identifier, "in var declaration",
                // "start of declaration", start).
                self.error_expected_msg(
                    "'identifier' expected in var declaration",
                    Some("start of declaration"),
                    Some(start),
                );
                return None;
            };
            // C++ 166-170.
            if self.identifier_type_annotation(id).is_none() {
                self.error_at(
                    id.range(),
                    "expected type annotation on declared var",
                );
            }
            if !self.eat_semi(false) {
                return None;
            }
            let node = Node::DeclareVariable(DeclareVariable::new(
                NodeMetadata::new(self.dummy_range()),
                id,
                kind,
            ));
            return Some(self.set_location(
                start,
                self.lexer.prev_token_end(),
                node,
            ));
        }

        // C++ 179-190: otherwise it must be `declare export`. whatLoc is
        // `start`. NOTE: the C++ list has a 5TH token (`rw_var`) that this
        // `error_expected4` call omits — `error_expected*` tops out at four
        // tokens (see `error_expected_enum_member_init` for the established
        // five-token workaround); flagged as a separate, pre-existing
        // message-text divergence, out of this task's location/range scope.
        if !self.check(TokenKind::rw_export) {
            self.error_expected4(
                TokenKind::rw_export,
                TokenKind::rw_interface,
                TokenKind::rw_function,
                TokenKind::rw_class,
                " in declared type",
                Some("start of declare"),
                start,
            );
            return None;
        }

        // C++ 192.
        self.parse_declare_export_flow(start)
    }

    /// The `(*optIdent)->_typeAnnotation` access on a parsed binding identifier
    /// (C++ flow.cpp:166 / 2775). Returns the identifier's type annotation if it
    /// has one (only `IdentifierNode`s carry one).
    fn identifier_type_annotation(
        &self,
        id: &'gc Node<'gc>,
    ) -> Option<&'gc Node<'gc>> {
        match id {
            Node::Identifier(i) => i.type_annotation,
            _ => None,
        }
    }

    // -----------------------------------------------------------------------
    // parseDeclareFunctionOrHookFlow — 2159 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a `declare function`/`declare hook` declaration, with the cursor
    /// at the `function`/`hook` keyword and `start` at `declare`. The parsed
    /// signature is attached to the id's type annotation, then wrapped in a
    /// `DeclareFunction(id, predicate)` (or `DeclareHook(id)`). Port of
    /// `JSParserImpl::parseDeclareFunctionOrHookFlow` (flow.cpp:2159-2258).
    fn parse_declare_function_or_hook_flow(
        &mut self,
        start: SMLoc,
        hook: bool,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 2162.
        self.advance(GrammarContext::Type);

        // C++ 2164-2169.
        if !self.need_at(
            TokenKind::identifier,
            " in declare function type",
            Some("location of declare"),
            start,
        ) {
            return None;
        }

        // C++ 2171-2172.
        let id_name = self.lexer.token().get_identifier();
        let id_start = self.advance(GrammarContext::Type).start;

        // C++ 2174.
        let func_start = self.cur_start();

        // C++ 2176-2182.
        let mut type_params: Option<&'gc Node<'gc>> = None;
        if self.check(TokenKind::less) {
            type_params = Some(self.parse_type_params_flow()?);
        }

        // C++ 2184-2189.
        if !self.need_at(
            TokenKind::l_paren,
            " in declare function type",
            Some("location of declare"),
            start,
        ) {
            return None;
        }

        // C++ 2191-2196.
        let mut params: Vec<&'gc Node<'gc>> = Vec::new();
        let mut this_constraint: Option<&'gc Node<'gc>> = None;
        let rest = self.parse_function_type_annotation_params_flow(
            &mut params,
            &mut this_constraint,
            hook,
        )?;

        // C++ 2198-2204.
        if !self.eat_at(
            TokenKind::colon,
            GrammarContext::Type,
            " in declare function type",
            Some("location of declare"),
            start,
        ) {
            return None;
        }

        // C++ 2206-2210: parseReturnTypeAnnotationFlow() — defaults
        // (None, AllowAnonFunctionType::Yes).
        let return_type = self.parse_return_type_annotation_flow(
            None,
            AllowAnonFunctionType::Yes,
        )?;
        let func_end = self.lexer.prev_token_end();

        // C++ 2212-2218.
        let mut predicate: Option<&'gc Node<'gc>> = None;
        if self.check_name(b"checks") && !hook {
            predicate = Some(self.parse_predicate_flow()?);
        }

        // C++ 2220-2221.
        if !self.eat_semi(false) {
            return None;
        }

        let params_list = NodeList::from_iter(self.gc, params);

        if !hook {
            // C++ 2223-2241.
            let fn_type =
                Node::FunctionTypeAnnotation(FunctionTypeAnnotation::new(
                    NodeMetadata::new(self.dummy_range()),
                    params_list,
                    this_constraint,
                    return_type,
                    rest,
                    type_params,
                ));
            let fn_type = self.set_location(func_start, func_end, fn_type);
            let annot = Node::TypeAnnotation(TypeAnnotation::new(
                NodeMetadata::new(self.dummy_range()),
                fn_type,
            ));
            let annot = self.set_location(func_start, func_end, annot);
            let ident_node = Node::Identifier(Identifier::new(
                NodeMetadata::new(self.dummy_range()),
                id_name,
                Some(annot),
                false,
            ));
            let ident =
                self.set_location(id_start, annot.range().end, ident_node);
            let node = Node::DeclareFunction(DeclareFunction::new(
                NodeMetadata::new(self.dummy_range()),
                ident,
                predicate,
            ));
            Some(self.set_location(start, self.lexer.prev_token_end(), node))
        } else {
            // C++ 2242-2257.
            let hook_type =
                Node::HookTypeAnnotation(HookTypeAnnotation::new(
                    NodeMetadata::new(self.dummy_range()),
                    params_list,
                    return_type,
                    rest,
                    type_params,
                ));
            let hook_type = self.set_location(func_start, func_end, hook_type);
            let annot = Node::TypeAnnotation(TypeAnnotation::new(
                NodeMetadata::new(self.dummy_range()),
                hook_type,
            ));
            let annot = self.set_location(func_start, func_end, annot);
            let ident_node = Node::Identifier(Identifier::new(
                NodeMetadata::new(self.dummy_range()),
                id_name,
                Some(annot),
                false,
            ));
            let ident =
                self.set_location(id_start, annot.range().end, ident_node);
            let node = Node::DeclareHook(DeclareHook::new(
                NodeMetadata::new(self.dummy_range()),
                ident,
            ));
            Some(self.set_location(start, self.lexer.prev_token_end(), node))
        }
    }

    /// `declare function`. Port of `parseDeclareFunctionFlow` (flow.cpp:2260).
    fn parse_declare_function_flow(
        &mut self,
        start: SMLoc,
    ) -> Option<&'gc Node<'gc>> {
        debug_assert!(self.check(TokenKind::rw_function));
        self.parse_declare_function_or_hook_flow(start, false)
    }

    /// `declare hook`. Port of `parseDeclareHookFlow` (flow.cpp:2265).
    fn parse_declare_hook_flow(
        &mut self,
        start: SMLoc,
    ) -> Option<&'gc Node<'gc>> {
        debug_assert!(self.check_name(b"hook"));
        self.parse_declare_function_or_hook_flow(start, true)
    }

    // -----------------------------------------------------------------------
    // parseDeclareModuleFlow — 2270 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse `declare module ...`, with the cursor at `module` and `start` at
    /// `declare`. Port of `JSParserImpl::parseDeclareModuleFlow`
    /// (flow.cpp:2270-2349).
    fn parse_declare_module_flow(
        &mut self,
        start: SMLoc,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 2271-2272.
        debug_assert!(self.check_name(b"module"));
        self.advance(GrammarContext::Type);

        // C++ 2274-2296: `declare module.exports: T`.
        if self.check_and_eat(TokenKind::period, GrammarContext::Type) {
            if !self.check_name(b"exports") {
                self.error_at(self.cur_range(), "expected module.exports declaration");
                return None;
            }
            self.advance(GrammarContext::Type);

            let annot_start = self.cur_start();
            if !self.eat_at(
                TokenKind::colon,
                GrammarContext::Type,
                " in module.exports declaration",
                Some("start of declaration"),
                start,
            ) {
                return None;
            }
            let type_annot = self.parse_type_annotation_flow(
                Some(annot_start),
                AllowAnonFunctionType::Yes,
            )?;
            // C++ 2291: eatSemi(true) — the optional form for module.exports.
            self.eat_semi(true);
            let node = Node::DeclareModuleExports(DeclareModuleExports::new(
                NodeMetadata::new(self.dummy_range()),
                type_annot,
            ));
            return Some(self.set_location(
                start,
                self.lexer.prev_token_end(),
                node,
            ));
        }

        // C++ 2298-2318: `declare module <string|ident> { ... }`.
        // Faithful to the C++ `ESTree::Node *id = nullptr;` branch-assign idiom.
        #[allow(clippy::needless_late_init)]
        let id: &'gc Node<'gc>;
        if self.check(TokenKind::string_literal) {
            let str_range = self.cur_range();
            let value = self.lexer.token().get_string_literal();
            let node = Node::StringLiteral(StringLiteral::new(
                NodeMetadata::new(self.dummy_range()),
                value,
            ));
            id = self.set_location(str_range.start, str_range.end, node);
        } else {
            if !self.need_at(
                TokenKind::identifier,
                " in module declaration",
                Some("start of declaration"),
                start,
            ) {
                return None;
            }
            let id_range = self.cur_range();
            let node = Node::Identifier(Identifier::new(
                NodeMetadata::new(self.dummy_range()),
                self.lexer.token().get_identifier(),
                None,
                false,
            ));
            id = self.set_location(id_range.start, id_range.end, node);
        }
        self.advance(GrammarContext::Type);

        // C++ 2321-2330.
        let body_start = self.cur_start();
        if !self.eat_at(
            TokenKind::l_brace,
            GrammarContext::Type,
            " in module declaration",
            Some("start of declaration"),
            start,
        ) {
            return None;
        }

        // C++ 2332-2337: the body recurses into statement-list items (which
        // include the `declare` statement branch).
        let mut declarations: Vec<&'gc Node<'gc>> = Vec::new();
        while !self.check(TokenKind::r_brace) {
            if !self.parse_statement_list_item(
                Param::default(),
                AllowImportExport::Yes,
                &mut declarations,
            ) {
                return None;
            }
        }

        // C++ 2339-2348.
        let body_end = self.advance(GrammarContext::Type).end;
        let body_node = Node::BlockStatement(BlockStatement::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, declarations),
            false,
        ));
        let body = self.set_location(body_start, body_end, body_node);
        let node = Node::DeclareModule(DeclareModule::new(
            NodeMetadata::new(self.dummy_range()),
            id,
            body,
        ));
        Some(self.set_location(start, body.range().end, node))
    }

    // -----------------------------------------------------------------------
    // parseDeclareNamespaceFlow — 2351 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse `declare namespace ...`, with the cursor at `namespace` and `start`
    /// at `declare`. Port of `JSParserImpl::parseDeclareNamespaceFlow`
    /// (flow.cpp:2351-2398).
    fn parse_declare_namespace_flow(
        &mut self,
        start: SMLoc,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 2352-2353.
        debug_assert!(self.check_name(b"namespace"));
        self.advance(GrammarContext::Type);

        // C++ 2357-2368.
        if !self.need_at(
            TokenKind::identifier,
            " in namespace declaration",
            Some("start of declaration"),
            start,
        ) {
            return None;
        }
        let id_range = self.cur_range();
        let id_node = Node::Identifier(Identifier::new(
            NodeMetadata::new(self.dummy_range()),
            self.lexer.token().get_identifier(),
            None,
            false,
        ));
        let id = self.set_location(id_range.start, id_range.end, id_node);
        self.advance(GrammarContext::Type);

        // C++ 2372-2379.
        let body_start = self.cur_start();
        if !self.eat_at(
            TokenKind::l_brace,
            GrammarContext::Type,
            " in namespace declaration",
            Some("start of declaration"),
            start,
        ) {
            return None;
        }

        // C++ 2381-2386.
        let mut declarations: Vec<&'gc Node<'gc>> = Vec::new();
        while !self.check(TokenKind::r_brace) {
            if !self.parse_statement_list_item(
                Param::default(),
                AllowImportExport::Yes,
                &mut declarations,
            ) {
                return None;
            }
        }

        // C++ 2388-2397.
        let body_end = self.advance(GrammarContext::Type).end;
        let body_node = Node::BlockStatement(BlockStatement::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, declarations),
            false,
        ));
        let body = self.set_location(body_start, body_end, body_node);
        let node = Node::DeclareNamespace(DeclareNamespace::new(
            NodeMetadata::new(self.dummy_range()),
            id,
            body,
        ));
        Some(self.set_location(start, body.range().end, node))
    }

    // -----------------------------------------------------------------------
    // parseDeclareClassFlow — 2400 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse `declare class ...`, with the cursor at `class` and `start` at
    /// `declare`. Port of `JSParserImpl::parseDeclareClassFlow`
    /// (flow.cpp:2400-2496).
    fn parse_declare_class_flow(
        &mut self,
        start: SMLoc,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 2401-2402.
        debug_assert!(self.check(TokenKind::rw_class));
        self.advance(GrammarContext::Type);

        // C++ 2404-2406: class definitions are always strict-mode code.
        let old_strict = self.lexer.is_strict_mode();
        self.lexer.set_strict_mode(true);

        let result = self.parse_declare_class_flow_inner(start);

        // C++ 2405: SaveFunctionState restores strict mode on scope exit.
        self.lexer.set_strict_mode(old_strict);
        result
    }

    /// The body of `parseDeclareClassFlow` after the strict-mode save, factored
    /// out so the strict-mode restore survives the `?` early-returns.
    fn parse_declare_class_flow_inner(
        &mut self,
        start: SMLoc,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 2408-2420.
        if !self.need_at(
            TokenKind::identifier,
            " in class declaration",
            Some("start of declaration"),
            start,
        ) {
            return None;
        }
        let id_range = self.cur_range();
        let id_node = Node::Identifier(Identifier::new(
            NodeMetadata::new(self.dummy_range()),
            self.lexer.token().get_identifier(),
            None,
            false,
        ));
        let id = self.set_location(id_range.start, id_range.end, id_node);
        self.advance(GrammarContext::Type);

        // C++ 2422-2428.
        let mut type_params: Option<&'gc Node<'gc>> = None;
        if self.check(TokenKind::less) {
            type_params = Some(self.parse_type_params_flow()?);
        }

        // C++ 2430-2440: `extends`.
        let mut extends: Vec<&'gc Node<'gc>> = Vec::new();
        if self.check_and_eat(TokenKind::rw_extends, GrammarContext::AllowRegExp)
        {
            if !self.need_at(
                TokenKind::identifier,
                " in class 'extends'",
                Some("start of declaration"),
                start,
            ) {
                return None;
            }
            if !self.parse_interface_extends(&mut extends) {
                return None;
            }
        }

        // C++ 2442-2454: `mixins`.
        let mut mixins: Vec<&'gc Node<'gc>> = Vec::new();
        if self.check_name(b"mixins") {
            self.advance(GrammarContext::AllowRegExp);
            loop {
                if !self.need_at(
                    TokenKind::identifier,
                    " in class 'mixins'",
                    Some("start of declaration"),
                    start,
                ) {
                    return None;
                }
                if !self.parse_interface_extends(&mut mixins) {
                    return None;
                }
                if !self.check_and_eat(TokenKind::comma, GrammarContext::Type) {
                    break;
                }
            }
        }

        // C++ 2456-2470: `implements`.
        let mut implements: Vec<&'gc Node<'gc>> = Vec::new();
        if self.check_and_eat(TokenKind::rw_implements, GrammarContext::AllowRegExp)
        {
            loop {
                if !self.need_at(
                    TokenKind::identifier,
                    " in class 'implements'",
                    Some("start of declaration"),
                    start,
                ) {
                    return None;
                }
                let impl_node = self.parse_class_implements_flow()?;
                implements.push(impl_node);
                if !self.check_and_eat(TokenKind::comma, GrammarContext::Type) {
                    break;
                }
            }
        }

        // C++ 2472-2484.
        if !self.need_at(
            TokenKind::l_brace,
            " in declared class",
            Some("start of declaration"),
            start,
        ) {
            return None;
        }
        let body = self.parse_object_type_annotation_flow(
            AllowProtoProperty::Yes,
            AllowStaticProperty::Yes,
            AllowSpreadProperty::No,
        )?;

        // C++ 2486-2495.
        let end = body.range().end;
        let node = Node::DeclareClass(DeclareClass::new(
            NodeMetadata::new(self.dummy_range()),
            id,
            type_params,
            NodeList::from_iter(self.gc, extends),
            NodeList::from_iter(self.gc, implements),
            NodeList::from_iter(self.gc, mixins),
            body,
        ));
        Some(self.set_location(start, end, node))
    }

    // -----------------------------------------------------------------------
    // parseDeclareExportFlow — 2577 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse `declare export ...`, with the cursor at `export` and `start` at
    /// `declare`. Port of `JSParserImpl::parseDeclareExportFlow`
    /// (flow.cpp:2577-2881).
    fn parse_declare_export_flow(
        &mut self,
        start: SMLoc,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 2578-2580.
        debug_assert!(self.check(TokenKind::rw_export));
        self.advance(GrammarContext::Type);
        let mut declare_start = self.cur_start();

        // C++ 2582-2669: `declare export default ...`.
        if self.check_and_eat(TokenKind::rw_default, GrammarContext::Type) {
            declare_start = self.cur_start();
            // C++ 2584-2592: default function.
            if self.check(TokenKind::rw_function) {
                let func = self.parse_declare_function_flow(declare_start)?;
                return self.wrap_declare_export(start, Some(func), true);
            }
            // C++ 2594-2603: default hook.
            if self.parse_flow_component_syntax()
                && self.check_hook_declaration_flow()
            {
                let func = self.parse_declare_hook_flow(declare_start)?;
                return self.wrap_declare_export(start, Some(func), true);
            }
            // C++ 2604-2619: default async hook (error then hook).
            if self.parse_flow_component_syntax()
                && self.check_unescaped_name(b"async")
                && self.check_async_hook_flow()
            {
                // Point location, NOT the current token's range: C++
                // (flow.cpp:2607-2609) calls `error(tok_->getStartLoc(),
                // ...)` — the `error(SMLoc, Twine)` overload.
                self.error_at_loc(
                    self.cur_start(),
                    "`async` is not supported for declared hooks. \
                     Use `declare hook` instead.",
                );
                self.advance(GrammarContext::AllowRegExp); // consume 'async'
                let hook = self.parse_declare_hook_flow(declare_start)?;
                return self.wrap_declare_export(start, Some(hook), true);
            }
            // C++ 2620-2636: default async component (error then component).
            if self.parse_flow_component_syntax()
                && self.check_unescaped_name(b"async")
                && self.check_async_component_flow()
            {
                // Point location, NOT the current token's range: C++
                // (flow.cpp:2623-2625) calls `error(tok_->getStartLoc(),
                // ...)` — the `error(SMLoc, Twine)` overload.
                self.error_at_loc(
                    self.cur_start(),
                    "`async` is not supported for declared components. \
                     Use `declare component` instead.",
                );
                self.advance(GrammarContext::AllowRegExp); // consume 'async'
                let comp = self
                    .parse_component_declaration_flow(start, true, true)?;
                return self.wrap_declare_export(start, Some(comp), true);
            }
            // C++ 2637-2648: default component.
            if self.parse_flow_component_syntax()
                && self.check_component_declaration_flow()
            {
                let comp = self
                    .parse_component_declaration_flow(start, true, false)?;
                return self.wrap_declare_export(start, Some(comp), true);
            }
            // C++ 2649-2658: default class.
            if self.check(TokenKind::rw_class) {
                let cls = self.parse_declare_class_flow(declare_start)?;
                return self.wrap_declare_export(start, Some(cls), true);
            }
            // C++ 2659-2668: default type annotation. NOTE the end loc here is
            // getPrevTokenEndLoc() (the semicolon), not the type's own end.
            let ty = self.parse_type_annotation_flow(
                None,
                AllowAnonFunctionType::Yes,
            )?;
            if !self.eat_semi(false) {
                return None;
            }
            let node =
                Node::DeclareExportDeclaration(DeclareExportDeclaration::new(
                    NodeMetadata::new(self.dummy_range()),
                    Some(ty),
                    NodeList::empty(),
                    None,
                    true,
                ));
            return Some(self.set_location(
                start,
                self.lexer.prev_token_end(),
                node,
            ));
        }

        // C++ 2671-2680: function.
        if self.check(TokenKind::rw_function) {
            let func = self.parse_declare_function_flow(declare_start)?;
            return self.wrap_declare_export(start, Some(func), false);
        }
        // C++ 2682-2691: hook.
        if self.parse_flow_component_syntax()
            && self.check_hook_declaration_flow()
        {
            let func = self.parse_declare_hook_flow(declare_start)?;
            return self.wrap_declare_export(start, Some(func), false);
        }
        // C++ 2693-2708: async hook (error then hook).
        if self.parse_flow_component_syntax()
            && self.check_unescaped_name(b"async")
            && self.check_async_hook_flow()
        {
            // Point location, NOT the current token's range: C++
            // (flow.cpp:2696-2698) calls `error(tok_->getStartLoc(), ...)`
            // — the `error(SMLoc, Twine)` overload.
            self.error_at_loc(
                self.cur_start(),
                "`async` is not supported for declared hooks. \
                 Use `declare hook` instead.",
            );
            self.advance(GrammarContext::AllowRegExp); // consume 'async'
            let hook = self.parse_declare_hook_flow(declare_start)?;
            return self.wrap_declare_export(start, Some(hook), false);
        }
        // C++ 2710-2719: class.
        if self.check(TokenKind::rw_class) {
            let cls = self.parse_declare_class_flow(declare_start)?;
            return self.wrap_declare_export(start, Some(cls), false);
        }
        // C++ 2721-2737: async component (error then component).
        if self.parse_flow_component_syntax()
            && self.check_unescaped_name(b"async")
            && self.check_async_component_flow()
        {
            // Point location, NOT the current token's range: C++
            // (flow.cpp:2724-2726) calls `error(tok_->getStartLoc(), ...)`
            // — the `error(SMLoc, Twine)` overload.
            self.error_at_loc(
                self.cur_start(),
                "`async` is not supported for declared components. \
                 Use `declare component` instead.",
            );
            self.advance(GrammarContext::AllowRegExp); // consume 'async'
            let comp =
                self.parse_component_declaration_flow(start, true, true)?;
            return self.wrap_declare_export(start, Some(comp), false);
        }
        // C++ 2739-2750: component.
        if self.parse_flow_component_syntax()
            && self.check_component_declaration_flow()
        {
            let comp =
                self.parse_component_declaration_flow(start, true, false)?;
            return self.wrap_declare_export(start, Some(comp), false);
        }
        // C++ 2752-2761: enum.
        if self.check(TokenKind::rw_enum) {
            let enum_decl = self.parse_enum_declaration_flow(start, true)?;
            return self.wrap_declare_export(start, Some(enum_decl), false);
        }

        // C++ 2763-2795: var/const/let. NOTE the var-kind advance here IS
        // `Type` (asymmetric with parseDeclareFLow's default-ctx advance).
        if self.check2(TokenKind::rw_var, TokenKind::rw_const)
            || self.check_name(b"let")
        {
            let kind = self.lexer.token().get_res_word_or_identifier();
            let var_start = self.advance(GrammarContext::Type).start;
            let Some(id) = self.parse_binding_identifier(Param::default())
            else {
                // C++ 2767-2774: errorExpected(identifier, "in var
                // declaration", "start of declaration", start).
                self.error_expected_msg(
                    "'identifier' expected in var declaration",
                    Some("start of declaration"),
                    Some(start),
                );
                return None;
            };
            if self.identifier_type_annotation(id).is_none() {
                self.error_at(
                    id.range(),
                    "expected type annotation on declared var",
                );
            }
            if !self.eat_semi(false) {
                return None;
            }
            let end = self.lexer.prev_token_end();
            let var_node = Node::DeclareVariable(DeclareVariable::new(
                NodeMetadata::new(self.dummy_range()),
                id,
                kind,
            ));
            let var = self.set_location(var_start, end, var_node);
            return self.wrap_declare_export(start, Some(var), false);
        }

        // C++ 2797-2812: `declare export opaque type`.
        if self.check_name(b"opaque") {
            self.advance(GrammarContext::Type);
            if !self.check_name(b"type") {
                // Point location, NOT the current token's range: C++
                // (flow.cpp:2798-2799) calls `error(tok_->getStartLoc(),
                // ...)` — the `error(SMLoc, Twine)` overload.
                self.error_at_loc(
                    self.cur_start(),
                    "'type' required in opaque type declaration",
                );
                return None;
            }
            self.advance(GrammarContext::Type);
            let ty = self.parse_type_alias_flow(
                declare_start,
                TypeAliasKind::DeclareOpaque,
            )?;
            return self.wrap_declare_export(start, Some(ty), false);
        }

        // C++ 2814-2824: `declare export type`.
        if self.check_name(b"type") {
            self.advance(GrammarContext::Type);
            let ty = self
                .parse_type_alias_flow(declare_start, TypeAliasKind::None)?;
            return self.wrap_declare_export(start, Some(ty), false);
        }

        // C++ 2826-2835: `declare export interface` — NOTE the no-arg call
        // (→ InterfaceDeclaration, NOT DeclareInterface).
        if self.check(TokenKind::rw_interface) || self.check_name(b"interface") {
            let iface = self.parse_interface_declaration_flow(None)?;
            return self.wrap_declare_export(start, Some(iface), false);
        }

        // C++ 2837-2854: `declare export * from 'foo'`.
        if self.check_and_eat(TokenKind::star, GrammarContext::Type) {
            if !self.check_name(b"from") {
                // Point location, NOT the current token's range: C++
                // (flow.cpp:2840-2841) calls `error(tok_->getStartLoc(),
                // ...)` — the `error(SMLoc, Twine)` overload.
                self.error_at_loc(
                    self.cur_start(),
                    "expected 'from' clause in export declaration",
                );
                return None;
            }
            let source = self.parse_from_clause()?;
            if !self.eat_semi(false) {
                return None;
            }
            let node = Node::DeclareExportAllDeclaration(
                DeclareExportAllDeclaration::new(
                    NodeMetadata::new(self.dummy_range()),
                    source,
                ),
            );
            return Some(self.set_location(
                start,
                self.lexer.prev_token_end(),
                node,
            ));
        }

        // C++ 2856-2880: `declare export { ... } [from]`.
        if !self.need_at(
            TokenKind::l_brace,
            " in export specifier",
            Some("start of declare"),
            start,
        ) {
            return None;
        }
        let mut specifiers: Vec<&'gc Node<'gc>> = Vec::new();
        let mut invalids: Vec<SMRange> = Vec::new();
        if !self.parse_export_clause(&mut specifiers, &mut invalids) {
            return None;
        }
        let source = if self.check_name(b"from") {
            Some(self.parse_from_clause()?)
        } else {
            None
        };
        if !self.eat_semi(false) {
            return None;
        }
        let node = Node::DeclareExportDeclaration(DeclareExportDeclaration::new(
            NodeMetadata::new(self.dummy_range()),
            None,
            NodeList::from_iter(self.gc, specifiers),
            source,
            false,
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    /// Build a `DeclareExportDeclaration(decl, [], None, default)` spanning
    /// `start`..end-of-decl. Shared by the many `parseDeclareExportFlow` arms
    /// that wrap a single declaration (C++ e.g. 2588-2592).
    fn wrap_declare_export(
        &mut self,
        start: SMLoc,
        decl: Option<&'gc Node<'gc>>,
        default: bool,
    ) -> Option<&'gc Node<'gc>> {
        let end = decl.map_or_else(
            || self.lexer.prev_token_end(),
            |d| d.range().end,
        );
        let node = Node::DeclareExportDeclaration(DeclareExportDeclaration::new(
            NodeMetadata::new(self.dummy_range()),
            decl,
            NodeList::empty(),
            None,
            default,
        ));
        Some(self.set_location(start, end, node))
    }

    // -----------------------------------------------------------------------
    // parseExportTypeDeclarationFlow — 2498 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse the tail of `export type ...` with the cursor at `type` and
    /// `start_loc` at `export`. Port of
    /// `JSParserImpl::parseExportTypeDeclarationFlow` (flow.cpp:2498-2575).
    pub(in crate::js) fn parse_export_type_declaration_flow(
        &mut self,
        start_loc: SMLoc,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 2500-2501.
        debug_assert!(self.check_name(b"type"));
        let type_ident_loc = self.advance(GrammarContext::AllowRegExp).start;
        let type_ident = self.gc.ctx().atom_table.atom_bytes(b"type");

        if self.check_and_eat(TokenKind::star, GrammarContext::AllowRegExp) {
            // export type * FromClause; (flow.cpp:2503-2518).
            let source = self.parse_from_clause()?;
            if !self.eat_semi(false) {
                return None;
            }
            let node = Node::ExportAllDeclaration(ExportAllDeclaration::new(
                NodeMetadata::new(self.dummy_range()),
                source,
                type_ident,
            ));
            return Some(self.set_location(
                start_loc,
                self.lexer.prev_token_end(),
                node,
            ));
        }

        if self.check(TokenKind::l_brace) {
            // export type ExportClause [FromClause]; (flow.cpp:2520-2556).
            let mut specifiers: Vec<&'gc Node<'gc>> = Vec::new();
            let mut invalids: Vec<SMRange> = Vec::new();
            if !self.parse_export_clause(&mut specifiers, &mut invalids) {
                return None;
            }

            // `from` is a contextual ident (escape-insensitive). C++ 2530.
            let source = if self.check_name(b"from") {
                Some(self.parse_from_clause()?)
            } else {
                // C++ 2537-2545: no FromClause → the invalids are real errors.
                for range in &invalids {
                    self.error_at(*range, "Invalid exported name");
                }
                None
            };

            if !self.eat_semi(false) {
                return None;
            }

            let node =
                Node::ExportNamedDeclaration(ExportNamedDeclaration::new(
                    NodeMetadata::new(self.dummy_range()),
                    None,
                    NodeList::from_iter(self.gc, specifiers),
                    source,
                    type_ident,
                ));
            return Some(self.set_location(
                start_loc,
                self.lexer.prev_token_end(),
                node,
            ));
        }

        // C++ 2557-2566.
        if self.check(TokenKind::identifier) {
            let alias = self
                .parse_type_alias_flow(type_ident_loc, TypeAliasKind::None)?;
            let node =
                Node::ExportNamedDeclaration(ExportNamedDeclaration::new(
                    NodeMetadata::new(self.dummy_range()),
                    Some(alias),
                    NodeList::empty(),
                    None,
                    type_ident,
                ));
            return Some(self.set_location(
                start_loc,
                alias.range().end,
                node,
            ));
        }

        // C++ 2569-2574: errorExpected(star, l_brace, identifier,
        // "in export type declaration", "start of export", startLoc).
        self.error_expected3(
            TokenKind::star,
            TokenKind::l_brace,
            TokenKind::identifier,
            " in export type declaration",
            Some("start of export"),
            start_loc,
        );
        None
    }

    // -----------------------------------------------------------------------
    // parseEnumDeclarationFlow — 5148 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a Flow `enum` declaration, with the cursor at `enum` and `start`
    /// at the start of the declaration. Port of
    /// `JSParserImpl::parseEnumDeclarationFlow` (flow.cpp:5148-5205).
    ///
    /// \param declare whether this is a `declare enum` (the `declare` routing
    ///   that passes `true` lands in P6.6).
    pub(in crate::js) fn parse_enum_declaration_flow(
        &mut self,
        start: SMLoc,
        declare: bool,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 5151-5152.
        debug_assert!(self.check(TokenKind::rw_enum));
        self.advance(GrammarContext::AllowRegExp);

        // C++ 5154-5161: errorExpected(identifier, "in enum declaration",
        // "start of declaration", start).
        if !self.need_at(
            TokenKind::identifier,
            " in enum declaration",
            Some("start of declaration"),
            start,
        ) {
            return None;
        }
        // C++ 5162-5166.
        let id_range = self.cur_range();
        let id_node = Node::Identifier(Identifier::new(
            NodeMetadata::new(self.dummy_range()),
            self.lexer.token().get_identifier(),
            None,
            false,
        ));
        let id = self.set_location(id_range.start, id_range.end, id_node);
        // C++ 5167.
        self.advance(GrammarContext::Type);

        // C++ 5169-5185: an optional `of <kind>` explicit type.
        let mut opt_kind: Option<EnumKind> = None;
        let mut explicit_type_start: Option<SMLoc> = None;
        if self.check_name(b"of") {
            explicit_type_start = Some(self.advance(GrammarContext::AllowRegExp).start);

            // C++ 5174-5184: the five contextual kind idents. `checkAndEat` of
            // a contextual ident → `check_name` + advance (default
            // GrammarContext::AllowRegExp).
            if self.check_name(b"string") {
                self.advance(GrammarContext::AllowRegExp);
                opt_kind = Some(EnumKind::String);
            } else if self.check_name(b"number") {
                self.advance(GrammarContext::AllowRegExp);
                opt_kind = Some(EnumKind::Number);
            } else if self.check_name(b"bigint") {
                self.advance(GrammarContext::AllowRegExp);
                opt_kind = Some(EnumKind::BigInt);
            } else if self.check_name(b"boolean") {
                self.advance(GrammarContext::AllowRegExp);
                opt_kind = Some(EnumKind::Boolean);
            } else if self.check_name(b"symbol") {
                self.advance(GrammarContext::AllowRegExp);
                opt_kind = Some(EnumKind::Symbol);
            }
        }

        // C++ 5187-5192.
        if !self.need_at(
            TokenKind::l_brace,
            " in enum declaration",
            Some("start of declaration"),
            start,
        ) {
            return None;
        }

        // C++ 5194-5196.
        let body = self.parse_enum_body_flow(opt_kind, explicit_type_start)?;

        // C++ 5198-5204.
        let end = body.range().end;
        if declare {
            let node = Node::DeclareEnum(DeclareEnum::new(
                NodeMetadata::new(self.dummy_range()),
                id,
                body,
            ));
            return Some(self.set_location(start, end, node));
        }
        let node = Node::EnumDeclaration(EnumDeclaration::new(
            NodeMetadata::new(self.dummy_range()),
            id,
            body,
        ));
        Some(self.set_location(start, end, node))
    }

    // -----------------------------------------------------------------------
    // parseEnumBodyFlow — 5207 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse the `{ ... }` body of an enum, with the cursor at `{`. `opt_kind`
    /// is the explicit kind from `of <kind>` (if any) and `explicit_type_start`
    /// is the location of that explicit type (if any). Port of
    /// `JSParserImpl::parseEnumBodyFlow` (flow.cpp:5207-5352).
    fn parse_enum_body_flow(
        &mut self,
        opt_kind: Option<EnumKind>,
        explicit_type_start: Option<SMLoc>,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 5210-5211.
        debug_assert!(self.check(TokenKind::l_brace));
        let mut start = self.advance(GrammarContext::AllowRegExp).start;

        // The kind may be inferred from the members if it was not explicit.
        let mut opt_kind = opt_kind;

        // C++ 5213-5261.
        let mut members: Vec<&'gc Node<'gc>> = Vec::new();
        let mut has_unknown_members = false;
        while !self.check(TokenKind::r_brace) {
            // C++ 5216-5227: the inexact `...`, which must come last.
            if self.check(TokenKind::dotdotdot) {
                let dotdotdot_loc =
                    self.advance(GrammarContext::Type).start;
                if !self.check(TokenKind::r_brace) {
                    self.error_at_loc(
                        dotdotdot_loc,
                        "The `...` must come after all enum members. \
                         Move it to the end of the enum body.",
                    );
                    return None;
                }
                has_unknown_members = true;
                break;
            }
            // C++ 5228-5233.
            if !self.need_at(
                TokenKind::identifier,
                " in enum declaration",
                Some("start of declaration"),
                start,
            ) {
                return None;
            }

            // C++ 5235-5239.
            let member = self.parse_enum_member_flow()?;
            let opt_member_kind = get_member_enum_kind_flow(member);

            // C++ 5241-5256.
            if let Some(kind) = opt_kind {
                // We've already figured out the type of the enum, so ensure
                // that the new member is compatible with this.
                if let Some(member_kind) = opt_member_kind {
                    if kind != member_kind {
                        let range = member.range();
                        self.error_at(
                            range,
                            &format!(
                                "cannot use {} initializer in {} enum",
                                enum_kind_str_flow(member_kind),
                                enum_kind_str_flow(kind),
                            ),
                        );
                        self.lexer.get_source_mgr_mut().note_at(
                            start,
                            None,
                            "start of enum body",
                            support::diag::Subsystem::Parser,
                        );
                        return None;
                    }
                }
            } else {
                opt_kind = opt_member_kind;
            }

            // C++ 5258-5260.
            members.push(member);
            if !self.check_and_eat(TokenKind::comma, GrammarContext::AllowRegExp)
            {
                break;
            }
        }

        // C++ 5263-5292.
        if !members.is_empty() {
            // Ensure that enum members use initializers consistently.
            // This is vacuously true when `members` is empty, so just make
            // sure all members use initializers iff the first member does.
            let uses_initializers = !matches!(
                members[0],
                Node::EnumDefaultedMember(_)
            );
            for member in &members {
                let member_uses =
                    !matches!(member, Node::EnumDefaultedMember(_));
                if uses_initializers != member_uses {
                    let range = member.range();
                    self.error_at(
                        range,
                        "enum members need to consistently either all use \
                         initializers, or use no initializers",
                    );
                    let first_range = members[0].range();
                    self.lexer.get_source_mgr_mut().note_range(
                        first_range,
                        "first enum member",
                        support::diag::Subsystem::Parser,
                    );
                    return None;
                }
            }

            // C++ 5283-5291.
            if !uses_initializers {
                // It's only legal to use defaulted members for string and
                // symbol enums, because other kinds of enums can't infer
                // values from names.
                if let Some(kind) = opt_kind {
                    if kind != EnumKind::String && kind != EnumKind::Symbol {
                        self.error_at_loc(
                            start,
                            "number and boolean enums must use initializers",
                        );
                        return None;
                    }
                }
            }
        }

        // C++ 5294-5301.
        let end = self.lexer.token().end_loc();
        if !self.eat_at(
            TokenKind::r_brace,
            GrammarContext::AllowRegExp,
            " in enum body",
            Some("start of body"),
            start,
        ) {
            return None;
        }

        // C++ 5303-5306.
        let has_explicit_type = explicit_type_start.is_some();
        if let Some(ets) = explicit_type_start {
            start = ets;
        }

        let members = NodeList::from_iter(self.gc, members);

        // C++ 5308-5314: an untyped/empty enum is a string-body enum.
        let Some(kind) = opt_kind else {
            let node = Node::EnumStringBody(EnumStringBody::new(
                NodeMetadata::new(self.dummy_range()),
                members,
                has_explicit_type,
                has_unknown_members,
            ));
            return Some(self.set_location(start, end, node));
        };

        // C++ 5316-5351: there are different node kinds per enum kind.
        let node = match kind {
            EnumKind::String => Node::EnumStringBody(EnumStringBody::new(
                NodeMetadata::new(self.dummy_range()),
                members,
                has_explicit_type,
                has_unknown_members,
            )),
            EnumKind::Number => Node::EnumNumberBody(EnumNumberBody::new(
                NodeMetadata::new(self.dummy_range()),
                members,
                has_explicit_type,
                has_unknown_members,
            )),
            EnumKind::BigInt => Node::EnumBigIntBody(EnumBigIntBody::new(
                NodeMetadata::new(self.dummy_range()),
                members,
                has_explicit_type,
                has_unknown_members,
            )),
            EnumKind::Boolean => Node::EnumBooleanBody(EnumBooleanBody::new(
                NodeMetadata::new(self.dummy_range()),
                members,
                has_explicit_type,
                has_unknown_members,
            )),
            EnumKind::Symbol => {
                // C++ 5343-5344: symbol enums can only be made via explicit
                // type. EnumSymbolBody has no `explicit_type` field.
                debug_assert!(
                    has_explicit_type,
                    "symbol enums can only be made via explicit type"
                );
                Node::EnumSymbolBody(EnumSymbolBody::new(
                    NodeMetadata::new(self.dummy_range()),
                    members,
                    has_unknown_members,
                ))
            }
        };
        Some(self.set_location(start, end, node))
    }

    // -----------------------------------------------------------------------
    // parseEnumMemberFlow — 5354 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse one enum member, with the cursor at the member's identifier. Port
    /// of `JSParserImpl::parseEnumMemberFlow` (flow.cpp:5354-5432).
    fn parse_enum_member_flow(&mut self) -> Option<&'gc Node<'gc>> {
        // C++ 5355-5360.
        debug_assert!(self.check(TokenKind::identifier));
        let id_range = self.cur_range();
        let id_node = Node::Identifier(Identifier::new(
            NodeMetadata::new(self.dummy_range()),
            self.lexer.token().get_identifier(),
            None,
            false,
        ));
        let id = self.set_location(id_range.start, id_range.end, id_node);
        // C++ 5361.
        self.advance(GrammarContext::AllowRegExp);

        // C++ 5363-5428.
        let member: &'gc Node<'gc>;
        if self.check_and_eat(TokenKind::equal, GrammarContext::AllowRegExp) {
            // Parse initializer.
            let tok_range = self.cur_range();
            if self.check2(TokenKind::rw_true, TokenKind::rw_false) {
                // C++ 5366-5372.
                let value = self.check(TokenKind::rw_true);
                let init_node = Node::BooleanLiteral(BooleanLiteral::new(
                    NodeMetadata::new(self.dummy_range()),
                    value,
                ));
                let init = self.set_location(
                    tok_range.start,
                    tok_range.end,
                    init_node,
                );
                let m = Node::EnumBooleanMember(EnumBooleanMember::new(
                    NodeMetadata::new(self.dummy_range()),
                    id,
                    init,
                ));
                member = self.set_location(
                    id.range().start,
                    tok_range.end,
                    m,
                );
            } else if self.check(TokenKind::string_literal) {
                // C++ 5373-5379.
                let value = self.lexer.token().get_string_literal();
                let init_node = Node::StringLiteral(StringLiteral::new(
                    NodeMetadata::new(self.dummy_range()),
                    value,
                ));
                let init = self.set_location(
                    tok_range.start,
                    tok_range.end,
                    init_node,
                );
                let m = Node::EnumStringMember(EnumStringMember::new(
                    NodeMetadata::new(self.dummy_range()),
                    id,
                    init,
                ));
                member = self.set_location(
                    id.range().start,
                    tok_range.end,
                    m,
                );
            } else if self.check(TokenKind::minus) {
                // C++ 5380-5397: a negated numeric literal.
                let minus_start = self.cur_start();
                self.advance(GrammarContext::AllowRegExp);
                if self.check(TokenKind::numeric_literal) {
                    let num_range = self.cur_range();
                    // Negate the literal.
                    let value = -self.lexer.token().get_numeric_literal();
                    let init_node = Node::NumericLiteral(NumericLiteral::new(
                        NodeMetadata::new(self.dummy_range()),
                        value,
                    ));
                    let init = self.set_location(
                        minus_start,
                        num_range.end,
                        init_node,
                    );
                    let m = Node::EnumNumberMember(EnumNumberMember::new(
                        NodeMetadata::new(self.dummy_range()),
                        id,
                        init,
                    ));
                    member = self.set_location(
                        id.range().start,
                        num_range.end,
                        m,
                    );
                } else {
                    // C++ 5390-5396: errorExpected(numeric_literal,
                    // "in negated enum member initializer",
                    // "start of negated enum member", id->getStartLoc()).
                    // `need_at` reports at the current token without
                    // consuming, matching errorExpected.
                    self.need_at(
                        TokenKind::numeric_literal,
                        " in negated enum member initializer",
                        Some("start of negated enum member"),
                        id.range().start,
                    );
                    return None;
                }
            } else if self.check(TokenKind::numeric_literal) {
                // C++ 5398-5404.
                let value = self.lexer.token().get_numeric_literal();
                let init_node = Node::NumericLiteral(NumericLiteral::new(
                    NodeMetadata::new(self.dummy_range()),
                    value,
                ));
                let init = self.set_location(
                    tok_range.start,
                    tok_range.end,
                    init_node,
                );
                let m = Node::EnumNumberMember(EnumNumberMember::new(
                    NodeMetadata::new(self.dummy_range()),
                    id,
                    init,
                ));
                member = self.set_location(
                    id.range().start,
                    tok_range.end,
                    m,
                );
            } else if self.check(TokenKind::bigint_literal) {
                // C++ 5405-5411.
                let bigint = self.lexer.token().get_bigint_literal();
                let init_node = Node::BigIntLiteral(BigIntLiteral::new(
                    NodeMetadata::new(self.dummy_range()),
                    bigint,
                ));
                let init = self.set_location(
                    tok_range.start,
                    tok_range.end,
                    init_node,
                );
                let m = Node::EnumBigIntMember(EnumBigIntMember::new(
                    NodeMetadata::new(self.dummy_range()),
                    id,
                    init,
                ));
                member = self.set_location(
                    id.range().start,
                    tok_range.end,
                    m,
                );
            } else {
                // C++ 5412-5422: errorExpected over the five literal token
                // kinds, whatLoc = id->getStartLoc() (real).
                self.error_expected_enum_member_init(id.range().start);
                return None;
            }
            // C++ 5424.
            self.advance(GrammarContext::AllowRegExp);
        } else {
            // C++ 5425-5427.
            let m = Node::EnumDefaultedMember(EnumDefaultedMember::new(
                NodeMetadata::new(self.dummy_range()),
                id,
            ));
            member = self.set_location(id.range().start, id.range().end, m);
        }

        // C++ 5430-5431.
        Some(member)
    }

    /// Report the five-token `errorExpected` for an enum member initializer
    /// (`true`, `false`, a string, a number, or a bigint). Port of the
    /// initializer-list `errorExpected` at flow.cpp:5412-5422 (where =
    /// "in enum member initializer", what = "start of enum member", whatLoc =
    /// `id->getStartLoc()`, real). The Rust `error_expected*` family tops
    /// out at four tokens, so render the five-token list directly to stay
    /// byte-faithful to the C++ message, routed through `error_expected_msg`
    /// for the same-line combined-range caret.
    fn error_expected_enum_member_init(&mut self, what_loc: SMLoc) {
        use crate::token_kinds::token_kind_str;
        let msg = format!(
            "'{}', '{}', '{}', '{}' or '{}' expected in enum member initializer",
            token_kind_str(TokenKind::rw_true),
            token_kind_str(TokenKind::rw_false),
            token_kind_str(TokenKind::string_literal),
            token_kind_str(TokenKind::numeric_literal),
            token_kind_str(TokenKind::bigint_literal),
        );
        self.error_expected_msg(
            &msg,
            Some("start of enum member"),
            Some(what_loc),
        );
    }
}
