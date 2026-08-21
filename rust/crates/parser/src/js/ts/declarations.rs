/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The TypeScript declaration gate (`parseTSDeclaration`), `type` alias,
//! `interface`, `enum`, and `namespace` declarations. Port of the declaration
//! entry points of `lib/Parser/JSParserImpl-ts.cpp`.

use hermes_ast::node::{
    Identifier, Node, TSEnumDeclaration, TSEnumMember, TSInterfaceBody,
    TSInterfaceDeclaration, TSInterfaceHeritage, TSModuleBlock, TSModuleMember,
    TSTypeAliasDeclaration, TSTypeReference,
};
use hermes_ast::node_child::{NodeList, NodeMetadata};
use hermes_support::location::SMLoc;

use crate::js::flow::{AllowTypedArrowFunction, CoverTypedParameters};
use crate::js::{AllowImportExport, JSParserImpl, Param, PARAM_IN};
use crate::lexer::GrammarContext;
use crate::token_kinds::TokenKind;

impl<'gc, 'ast, 'ctx, 'a> JSParserImpl<'gc, 'ast, 'ctx, 'a> {
    // -----------------------------------------------------------------------
    // parseTSDeclaration — 516 in JSParserImpl-ts.cpp
    // -----------------------------------------------------------------------

    /// Parse a TypeScript declaration (`type`/`interface`/`namespace`/`enum`).
    /// Port of `JSParserImpl::parseTSDeclaration` (ts.cpp:516-535).
    /// Reached from `parse_declaration` only when `check_declaration()` is
    /// true, so (like the C++) it never falls through: `None` means an error
    /// was already reported.
    pub(in crate::js) fn parse_ts_declaration(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 517.
        debug_assert!(self.check_declaration(), "invalid start for TS declaration");

        // C++ 519.
        let start = self.cur_start();

        // C++ 521-523: `interface ...`. The dual `checkN(rw_interface,
        // interfaceIdent_)` — strict mode lexes `interface` as `rw_interface`,
        // loose mode as the escape-insensitive contextual ident.
        if self.check(TokenKind::rw_interface) || self.check_name(b"interface") {
            return self.parse_ts_interface_declaration();
        }

        // C++ 525-527: `type Identifier ...`. The C++
        // `checkAndEat(typeIdent_, GrammarContext::Type)` is the
        // escape-insensitive name overload; there is no `check_and_eat_name`
        // helper, so do the check + `advance(GrammarContext::Type)` by hand.
        if self.check_name(b"type") {
            self.advance(GrammarContext::Type);
            return self.parse_ts_type_alias_declaration(start);
        }

        // C++ 529-531: `namespace ...`.
        if self.check_name(b"namespace") {
            return self.parse_ts_namespace_declaration();
        }

        // C++ 533-534: otherwise it must be `enum ...`.
        debug_assert!(self.check(TokenKind::rw_enum));
        self.parse_ts_enum_declaration()
    }

    // -----------------------------------------------------------------------
    // parseTSTypeAliasDeclaration — 537 in JSParserImpl-ts.cpp
    // -----------------------------------------------------------------------

    /// Parse a TS `type X = T;` alias declaration, with `start` at the `type`
    /// keyword. Port of `JSParserImpl::parseTSTypeAliasDeclaration`
    /// (ts.cpp:537-578).
    fn parse_ts_type_alias_declaration(
        &mut self,
        start: SMLoc,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 539-541: what/whatLoc = "start of type alias" / `start`.
        if !self.need_at(
            TokenKind::identifier,
            " in type alias",
            Some("start of type alias"),
            start,
        ) {
            return None;
        }

        // C++ 543-548.
        let id_range = self.cur_range();
        let id_node = Node::Identifier(Identifier::new(
            NodeMetadata::new(self.dummy_range()),
            self.lexer.token().get_identifier(),
            None,
            false,
        ));
        let id = self.set_location(id_range.start, id_range.end, id_node);
        self.advance(GrammarContext::Type);

        // C++ 550-556: type parameters.
        let mut type_params: Option<&'gc Node<'gc>> = None;
        if self.check(TokenKind::less) {
            type_params = Some(self.parse_ts_type_parameters()?);
        }

        // C++ 558-564: what/whatLoc = "start of type alias" / `start`.
        if !self.eat_at(
            TokenKind::equal,
            GrammarContext::Type,
            " in type alias",
            Some("start of type alias"),
            start,
        ) {
            return None;
        }

        // C++ 566-569.
        let right = self.parse_type_annotation_ts(None)?;

        // C++ 571-572.
        if !self.eat_semi(true) {
            return None;
        }

        // C++ 574-577.
        let node = Node::TSTypeAliasDeclaration(TSTypeAliasDeclaration::new(
            NodeMetadata::new(self.dummy_range()),
            id,
            type_params,
            right,
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseTSInterfaceDeclaration — 580 in JSParserImpl-ts.cpp
    // -----------------------------------------------------------------------

    /// Parse a TS `interface` declaration, with the cursor at `interface`.
    /// Port of `JSParserImpl::parseTSInterfaceDeclaration` (ts.cpp:580-677).
    pub(super) fn parse_ts_interface_declaration(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 581: the dual `checkN(rw_interface, interfaceIdent_)`.
        debug_assert!(
            self.check(TokenKind::rw_interface)
                || self.check_name(b"interface"),
            "must be at 'interface'"
        );
        // C++ 582.
        let start = self.advance(GrammarContext::Type).start;

        // C++ 584-591: the id may be a reserved word, so accept any identifier
        // or res-word and build it from `get_res_word_or_identifier`.
        if !self.check(TokenKind::identifier)
            && !self.lexer.token().is_res_word()
        {
            // C++ 585-589: bare `errorExpected` (not `need`), since `check`
            // was already tested above. what/whatLoc = "start of interface"
            // / `start`.
            self.error_expected_msg(
                "'identifier' expected in interface declaration",
                Some("start of interface"),
                Some(start),
            );
            return None;
        }

        // C++ 593-598.
        let id_range = self.cur_range();
        let id_node = Node::Identifier(Identifier::new(
            NodeMetadata::new(self.dummy_range()),
            self.lexer.token().get_res_word_or_identifier(),
            None,
            false,
        ));
        let id = self.set_location(id_range.start, id_range.end, id_node);
        self.advance(GrammarContext::Type);

        // C++ 600-606: type parameters.
        let mut type_params: Option<&'gc Node<'gc>> = None;
        if self.check(TokenKind::less) {
            type_params = Some(self.parse_ts_type_parameters()?);
        }

        // C++ 608-631: the `extends` heritage clause.
        let mut extends: Vec<&'gc Node<'gc>> = Vec::new();
        // C++ 609-610: NOTE the `extends` keyword is eaten in `AllowRegExp`,
        // NOT `Type` — the one deliberate exception (ts.cpp:610).
        if self
            .check_and_eat(TokenKind::rw_extends, GrammarContext::AllowRegExp)
        {
            // C++ 611-630: a do-while.
            loop {
                // C++ 612-615.
                let expr = self.parse_ts_type_reference()?;

                // C++ 617-621: the C++ moves the reference's `_typeParameters`
                // (the type arguments) out into a separate `typeArgs` slot and
                // nulls the reference's own. Our AST nodes are immutable, so
                // replicate by destructuring the just-parsed `TSTypeReference`
                // and rebuilding it without type-params (at the same location),
                // passing the type-args separately to `TSInterfaceHeritage`.
                let type_ref = expr
                    .as_ts_type_reference()
                    .expect("parseTSTypeReference returns a TSTypeReference");
                let type_args = type_ref.type_parameters;
                let expr = if type_args.is_some() {
                    let ref_range = expr.metadata().range();
                    let rebuilt =
                        Node::TSTypeReference(TSTypeReference::new(
                            NodeMetadata::new(self.dummy_range()),
                            type_ref.type_name,
                            None,
                        ));
                    self.set_location(ref_range.start, ref_range.end, rebuilt)
                } else {
                    expr
                };

                // C++ 623-626.
                let heritage_node =
                    Node::TSInterfaceHeritage(TSInterfaceHeritage::new(
                        NodeMetadata::new(self.dummy_range()),
                        expr,
                        type_args,
                    ));
                extends.push(self.set_location(
                    start,
                    self.lexer.prev_token_end(),
                    heritage_node,
                ));

                // C++ 628-630.
                if !self.check_and_eat(TokenKind::comma, GrammarContext::Type) {
                    break;
                }
                if self.check(TokenKind::l_brace) {
                    break;
                }
            }
        }

        // C++ 633.
        let body_start = self.cur_start();

        // C++ 635-641: what/whatLoc = "start of interface" / `start`.
        if !self.eat_at(
            TokenKind::l_brace,
            GrammarContext::Type,
            " in interface declaration",
            Some("start of interface"),
            start,
        ) {
            return None;
        }

        // C++ 643-657: the body members, separated by `,` or `;`.
        let mut members: Vec<&'gc Node<'gc>> = Vec::new();
        while !self.check(TokenKind::r_brace) {
            members.push(self.parse_ts_object_type_member()?);

            let has_next = self.check2(TokenKind::comma, TokenKind::semi);
            if has_next {
                self.advance(GrammarContext::Type);
            } else {
                break;
            }
        }

        // C++ 659-665: what/whatLoc = "start of object type" / `start`.
        if !self.eat_at(
            TokenKind::r_brace,
            GrammarContext::Type,
            " at end of object type",
            Some("start of object type"),
            start,
        ) {
            return None;
        }

        // C++ 667-670.
        let body_node = Node::TSInterfaceBody(TSInterfaceBody::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, members),
        ));
        let body =
            self.set_location(body_start, self.lexer.prev_token_end(), body_node);

        // C++ 672-676.
        let node = Node::TSInterfaceDeclaration(TSInterfaceDeclaration::new(
            NodeMetadata::new(self.dummy_range()),
            id,
            body,
            NodeList::from_iter(self.gc, extends),
            type_params,
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseTSEnumDeclaration — 679 in JSParserImpl-ts.cpp
    // -----------------------------------------------------------------------

    /// Parse a TS `enum` declaration, with the cursor at `enum`. Port of
    /// `JSParserImpl::parseTSEnumDeclaration` (ts.cpp:679-723).
    fn parse_ts_enum_declaration(&mut self) -> Option<&'gc Node<'gc>> {
        // C++ 680.
        debug_assert!(self.check(TokenKind::rw_enum));
        // C++ 681.
        let start = self.advance(GrammarContext::Type).start;

        // C++ 683-689.
        let name = match self.parse_binding_identifier(Param::default()) {
            Some(name) => name,
            None => {
                // C++ 685-689: bare `errorExpected` (not `need`), since
                // `parseBindingIdentifier` already failed. what/whatLoc =
                // "start of enum" / `start`.
                self.error_expected_msg(
                    "'identifier' expected in enum declaration",
                    Some("start of enum"),
                    Some(start),
                );
                return None;
            }
        };

        // C++ 691-697: what/whatLoc = "start of enum" / `start`.
        if !self.eat_at(
            TokenKind::l_brace,
            GrammarContext::Type,
            " in enum declaration",
            Some("start of enum"),
            start,
        ) {
            return None;
        }

        // C++ 699-709: members separated by `,`, trailing comma optional.
        let mut members: Vec<&'gc Node<'gc>> = Vec::new();
        while !self.check(TokenKind::r_brace) {
            members.push(self.parse_ts_enum_member()?);

            if !self.check_and_eat(TokenKind::comma, GrammarContext::Type) {
                break;
            }
        }

        // C++ 711-717: what/whatLoc = "start of enum" / `start`.
        if !self.eat_at(
            TokenKind::r_brace,
            GrammarContext::Type,
            " in enum declaration",
            Some("start of enum"),
            start,
        ) {
            return None;
        }

        // C++ 719-722.
        let node = Node::TSEnumDeclaration(TSEnumDeclaration::new(
            NodeMetadata::new(self.dummy_range()),
            name,
            NodeList::from_iter(self.gc, members),
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseTSEnumMember — 725 in JSParserImpl-ts.cpp
    // -----------------------------------------------------------------------

    /// Parse one TS enum member `name` or `name = init`. Port of
    /// `JSParserImpl::parseTSEnumMember` (ts.cpp:725-748).
    fn parse_ts_enum_member(&mut self) -> Option<&'gc Node<'gc>> {
        // C++ 726.
        let start = self.cur_start();

        // C++ 728-734.
        let name = match self.parse_binding_identifier(Param::default()) {
            Some(name) => name,
            None => {
                // C++ 730-734: bare `errorExpected` (not `need`), since
                // `parseBindingIdentifier` already failed. what/whatLoc =
                // "start of member" / `start`.
                self.error_expected_msg(
                    "'identifier' expected in enum member",
                    Some("start of member"),
                    Some(start),
                );
                return None;
            }
        };

        // C++ 736-742: NOTE the `=` is a bare `checkAndEat` — the DEFAULT
        // grammar context (`AllowRegExp`), NOT `Type` (ts.cpp:737); and
        // `parseAssignmentExpression()` with no args uses all header defaults
        // (ParamIn, AllowTypedArrowFunction::Yes, CoverTypedParameters::Yes,
        // typeParams = null; the C++ `forceEagerly` default has no Rust analog).
        let mut init: Option<&'gc Node<'gc>> = None;
        if self.check_and_eat(TokenKind::equal, GrammarContext::AllowRegExp) {
            init = Some(self.parse_assignment_expression(
                PARAM_IN,
                false,
                AllowTypedArrowFunction::Yes,
                CoverTypedParameters::Yes,
                None,
            )?);
        }

        // C++ 744-747.
        let node = Node::TSEnumMember(TSEnumMember::new(
            NodeMetadata::new(self.dummy_range()),
            name,
            init,
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseTSNamespaceDeclaration — 750 in JSParserImpl-ts.cpp
    // -----------------------------------------------------------------------

    /// Parse a TS `namespace` declaration, with the cursor at `namespace`. Port
    /// of `JSParserImpl::parseTSNamespaceDeclaration` (ts.cpp:750-799).
    fn parse_ts_namespace_declaration(&mut self) -> Option<&'gc Node<'gc>> {
        // C++ 751.
        debug_assert!(self.check_name(b"namespace"));
        // C++ 752.
        let start = self.advance(GrammarContext::Type).start;

        // C++ 754-762.
        let name = match self.parse_ts_qualified_name() {
            Some(name) => name,
            None => {
                // C++ 756-762: bare `errorExpected` (not `need`), since
                // `parseTSQualifiedName` already failed. what/whatLoc =
                // "start of namespace" / `start`.
                self.error_expected_msg(
                    "'identifier' expected in namespace declaration",
                    Some("start of namespace"),
                    Some(start),
                );
                return None;
            }
        };

        // C++ 765-771: what/whatLoc = "start of namespace" / `start`.
        if !self.eat_at(
            TokenKind::l_brace,
            GrammarContext::Type,
            " in namespace declaration",
            Some("start of namespace"),
            start,
        ) {
            return None;
        }

        // C++ 773-780: the body recurses into statement-list items.
        let mut members: Vec<&'gc Node<'gc>> = Vec::new();
        while !self.check(TokenKind::r_brace) {
            if !self.parse_statement_list_item(
                Param::default(),
                AllowImportExport::Yes,
                &mut members,
            ) {
                return None;
            }
        }

        // C++ 782-788: what/whatLoc = "start of namespace" / `start`.
        if !self.eat_at(
            TokenKind::r_brace,
            GrammarContext::Type,
            " in namespace declaration",
            Some("start of namespace"),
            start,
        ) {
            return None;
        }

        // C++ 790-793.
        let body_node = Node::TSModuleBlock(TSModuleBlock::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, members),
        ));
        let body =
            self.set_location(start, self.lexer.prev_token_end(), body_node);

        // C++ 795-798.
        let node = Node::TSModuleMember(TSModuleMember::new(
            NodeMetadata::new(self.dummy_range()),
            name,
            Some(body),
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }
}
