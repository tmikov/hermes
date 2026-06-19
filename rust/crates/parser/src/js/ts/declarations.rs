/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The TypeScript declaration gate (`parseTSDeclaration`) and `type` alias
//! declarations. Port of the declaration entry points of
//! `lib/Parser/JSParserImpl-ts.cpp`.
//!
//! P7.0 wires only the `type X = T;` alias path; `interface`/`namespace`/
//! `enum` arrive in P7.4. Type-parameter declarations (`type X<...> = T;`) are
//! an honest parse error until they land.

use ast::node::{Identifier, Node, TSTypeAliasDeclaration};
use ast::node_child::NodeMetadata;
use support::location::SMLoc;

use crate::js::JSParserImpl;
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
    ///
    /// P7.0 dispatches only the `type` alias case; the `interface`/`namespace`/
    /// `enum` arms arrive in P7.4.
    pub(in crate::js) fn parse_ts_declaration(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 517.
        debug_assert!(self.check_declaration(), "invalid start for TS declaration");

        // C++ 519.
        let start = self.cur_start();

        // C++ 525-527: `type Identifier ...`. The C++
        // `checkAndEat(typeIdent_, GrammarContext::Type)` is the
        // escape-insensitive name overload; there is no `check_and_eat_name`
        // helper, so do the check + `advance(GrammarContext::Type)` by hand.
        if self.check_name(b"type") {
            self.advance(GrammarContext::Type);
            return self.parse_ts_type_alias_declaration(start);
        }

        // C++ 521-523/529-534: interface/namespace/enum. P7.4.
        self.error_cur("unsupported TypeScript declaration");
        None
    }

    // -----------------------------------------------------------------------
    // parseTSTypeAliasDeclaration — 537 in JSParserImpl-ts.cpp
    // -----------------------------------------------------------------------

    /// Parse a TS `type X = T;` alias declaration, with `start` at the `type`
    /// keyword. Port of `JSParserImpl::parseTSTypeAliasDeclaration`
    /// (ts.cpp:537-578).
    ///
    /// P7.0 defers type parameters (`type X<...> = T;`): a `<` after the name
    /// is an honest parse error pending P7's `parseTSTypeParameters`.
    fn parse_ts_type_alias_declaration(
        &mut self,
        start: SMLoc,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 539-541.
        if !self.need(TokenKind::identifier, " in type alias") {
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

        // C++ 550-556: type parameters. Deferred in P7.0 (honest error).
        let type_params: Option<&'gc Node<'gc>> = None;
        if self.check(TokenKind::less) {
            self.error_cur(
                "TypeScript type parameters are not yet supported",
            );
            return None;
        }

        // C++ 558-564.
        if !self.eat(TokenKind::equal, GrammarContext::Type, " in type alias") {
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
}
