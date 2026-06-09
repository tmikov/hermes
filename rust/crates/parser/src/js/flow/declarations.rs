/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The Flow declaration gate (`parseFlowDeclaration`) and `type` alias
//! declarations. Port of the declaration entry points of
//! `lib/Parser/JSParserImpl-flow.cpp`.

use ast::node::{Identifier, Node, TypeAlias};
use ast::node_child::NodeMetadata;
use support::location::SMLoc;

use crate::js::JSParserImpl;
use crate::lexer::GrammarContext;
use crate::token_kinds::TokenKind;

use super::{AllowAnonFunctionType, TypeAliasKind};

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

        // P6: component/hook declarations (gated on
        // getParseFlowComponentSyntax(), C++ 25-45) and record declarations
        // (gated on getParseFlowRecords(), C++ 47-49) — the Rust Context does
        // not implement those flags yet.

        // C++ 51-56.
        if self.check(TokenKind::rw_enum) {
            // P6: parseEnumDeclarationFlow (C++ 52-55).
            self.error_cur("Flow enum declarations are unsupported (parser phase P6)");
            return None;
        }

        // C++ 58-62. `checkAndEat(<ident>)` advances with the default
        // GrammarContext::AllowRegExp.
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
            // P5.3: parseInterfaceDeclarationFlow (C++ 82-86).
            self.error_cur("interface declarations are unsupported (parser phase P5.3)");
            return None;
        }

        // C++ 89-92.
        unreachable!("checkDeclaration() returned true without 'type' or 'interface'");
    }

    // -----------------------------------------------------------------------
    // parseTypeAliasFlow — 1981 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a type alias, with `type` already consumed and `start` at the
    /// start of the declaration. Port of `JSParserImpl::parseTypeAliasFlow`
    /// (flow.cpp:1981-2071). P5.0/P5.2 implement the `TypeAliasKind::None`
    /// path (a plain `TypeAlias` node, with optional type parameters).
    pub(super) fn parse_type_alias_flow(
        &mut self,
        start: SMLoc,
        kind: TypeAliasKind,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 1984-1987.
        if !self.need(TokenKind::identifier, " in type alias") {
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

        // C++ 2004-2026: the Opaque/DeclareOpaque `super`/`extends`/legacy-`:`
        // bounds; C++ 2047-2065: the DeclareOpaqueType/DeclareTypeAlias/
        // OpaqueType result nodes.
        if kind != TypeAliasKind::None {
            // P5.3: declare/opaque type aliases.
            self.error_cur(
                "declare/opaque type aliases are unsupported (parser phase P5.3)",
            );
            return None;
        }

        // C++ 2029-2041 (the `kind != DeclareOpaque` path).
        if !self.eat(TokenKind::equal, GrammarContext::Type, " in type alias") {
            return None;
        }
        let right =
            self.parse_type_annotation_flow(None, AllowAnonFunctionType::Yes)?;

        // C++ 2043-2044.
        if !self.eat_semi(false) {
            return None;
        }

        // C++ 2066-2070.
        let node = Node::TypeAlias(TypeAlias::new(
            NodeMetadata::new(self.dummy_range()),
            id,
            type_params,
            right,
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }
}
