/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! JSX parsing for the JS parser. Port of `lib/Parser/JSParserImpl-jsx.cpp`.
//!
//! JSX is gated behind the independent `Context::parse_jsx` flag (the hermesc
//! `-parse-jsx` flag); it is NOT implied by `-parse-flow` or `-parse-ts`. The
//! entry point is reached from the primary-expression `<` arm (mirroring the
//! C++ `case TokenKind::less` at JSParserImpl.cpp:2691-2703), which dispatches
//! into `parse_jsx_root` when `getParseJSX()` is set.
//!
//! Almost every `advance`/`eat` in the JSX grammar uses
//! `GrammarContext::AllowJSXIdentifier`, so that `-`-containing identifiers and
//! reserved words lex as JSX identifiers. The two exceptions match the C++
//! defaults: the `self_closing` `checkAndEat(slash)` and the post-self-close
//! `advance()` use the default (`AllowRegExp`) context.
//!
//! The JSX nesting depth is tracked in `JSParserImpl::jsx_depth` (port of the
//! C++ `jsxDepth_`), saved/restored at the element/fragment boundaries via the
//! `save_jsx_depth` RAII guard so it never leaks on a `?` error early-return
//! (mirroring the C++ `llvh::SaveAndRestore<uint32_t>`).
//!
//! P8.0 lands the scaffolding + the self-closing-element happy path: the entry
//! dispatch (`parse_jsx_root`), `parse_jsx_element`/`parse_jsx_opening_element`
//! for self-closing tags, and the full element-name parser
//! (`parse_jsx_element_name`, which handles namespaced names and member
//! expressions). Fragments, children, and the attributes loop remain honest
//! parse errors (see the `// P8.1` markers) and Flow `<TypeArgs>` on an opening
//! tag is left unparsed (`type_arguments = None`).

use ast::node::{
    JSXElement, JSXIdentifier, JSXMemberExpression, JSXNamespacedName,
    JSXOpeningElement, Node,
};
use ast::node_child::{NodeList, NodeMetadata};

use support::location::SMLoc;

use crate::js::JSParserImpl;
use crate::lexer::GrammarContext;
use crate::token_kinds::TokenKind;

/// Whether a `JSXMemberExpression` (`foo.bar`) is a valid parse of the
/// `JSXElementName`. Port of `JSParserImpl::AllowJSXMemberExpression`
/// (JSParserImpl.h:1198). Runtime enum (faithful), NOT a bool.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum AllowJSXMemberExpression {
    No,
    Yes,
}

impl<'gc, 'ast, 'ctx, 'a> JSParserImpl<'gc, 'ast, 'ctx, 'a> {
    /// JSX entry point. Port of `JSParserImpl::parseJSX` (jsx.cpp:22-30).
    ///
    /// NOTE the rename: the bool accessor `parse_jsx()` (the C++
    /// `getParseJSX()`) already occupies the `parse_jsx` name, so the JSX
    /// *entry* method is `parse_jsx_root`.
    pub(super) fn parse_jsx_root(&mut self) -> Option<&'gc Node<'gc>> {
        // C++ 23.
        debug_assert!(self.check(TokenKind::less));
        // C++ 24: llvh::SaveAndRestore<uint32_t> saveDepth(jsxDepth_, 0).
        let _depth_guard = self.save_jsx_depth(0);
        // C++ 25.
        let start =
            self.advance(GrammarContext::AllowJSXIdentifier).start;
        // C++ 26-28.
        if self.check(TokenKind::greater) {
            return self.parse_jsx_fragment(start);
        }
        // C++ 29.
        self.parse_jsx_element(start)
    }

    /// Parse a `JSXElement`. Port of `JSParserImpl::parseJSXElement`
    /// (jsx.cpp:77-115).
    fn parse_jsx_element(&mut self, start: SMLoc) -> Option<&'gc Node<'gc>> {
        // C++ 78: llvh::SaveAndRestore<uint32_t> saveDepth(jsxDepth_,
        // jsxDepth_ + 1).
        let _depth_guard = self.save_jsx_depth(self.jsx_depth.get() + 1);
        // C++ 79-81.
        let opening = self.parse_jsx_opening_element(start)?;
        // C++ 82-87: self-closing element has no children/closing.
        let opening_self_closing = opening
            .as_jsx_opening_element()
            .expect("parse_jsx_opening_element returns a JSXOpeningElement")
            .self_closing
            .get();
        if opening_self_closing {
            let end = opening.metadata().range.get().end;
            let node = Node::JSXElement(JSXElement::new(
                NodeMetadata::new(self.dummy_range()),
                opening,
                NodeList::empty(),
                None,
            ));
            return Some(self.set_location(start, end, node));
        }

        // C++ 88-114: children + closing tag.
        // P8.1: JSXChildren + closing tag (parseJSXChildren, tagNamesMatch).
        self.error_cur("JSX children are not yet supported");
        None
    }

    /// Parse a `JSXOpeningElement`. Port of
    /// `JSParserImpl::parseJSXOpeningElement` (jsx.cpp:117-169).
    fn parse_jsx_opening_element(
        &mut self,
        start: SMLoc,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 119-122.
        let name =
            self.parse_jsx_element_name(AllowJSXMemberExpression::Yes)?;

        // C++ 124-132: optional Flow `<TypeArgs>`.
        let type_arguments: Option<&'gc Node<'gc>> = if self.check(TokenKind::less)
        {
            // P8.1/capstone: Flow `<TypeArgs>` via parse_type_args_flow.
            self.error_cur("JSX type arguments are not yet supported");
            return None;
        } else {
            None
        };

        // C++ 134-148: attributes loop.
        // P8.1: the attributes loop (parseJSXAttribute / parseJSXSpreadAttribute).
        // For P8.0 a self-closing tag has no attributes, so require `/` or `>`.
        if !self.check2(TokenKind::slash, TokenKind::greater) {
            self.error_cur("JSX attributes are not yet supported");
            return None;
        }

        // C++ 150: default (AllowRegExp) context, matching `checkAndEat(slash)`.
        let self_closing =
            self.check_and_eat(TokenKind::slash, GrammarContext::AllowRegExp);

        // C++ 152-154.
        let end = self.cur_range().end;
        if !self.need(TokenKind::greater, " at end of JSX tag") {
            return None;
        }

        // C++ 156-162: the lexer-mode switch. The outermost self-closing tag
        // (jsxDepth_ <= 1) returns to standard JS mode; otherwise stay in JSX
        // child mode for the children that follow.
        if self_closing && self.jsx_depth.get() <= 1 {
            // C++ 158: done with JSX for now, return to standard JS mode.
            self.advance(GrammarContext::AllowRegExp);
        } else {
            // C++ 161: still in JSX, children after this element.
            self.lexer.advance_in_jsx_child();
        }

        // C++ 164-168.
        let node = Node::JSXOpeningElement(JSXOpeningElement::new(
            NodeMetadata::new(self.dummy_range()),
            name,
            NodeList::empty(),
            self_closing,
            type_arguments,
        ));
        Some(self.set_location(start, end, node))
    }

    /// Parse a `JSXFragment`. Port of `JSParserImpl::parseJSXFragment`
    /// (jsx.cpp:171-201).
    fn parse_jsx_fragment(&mut self, _start: SMLoc) -> Option<&'gc Node<'gc>> {
        // C++ 172.
        debug_assert!(self.check(TokenKind::greater));
        // P8.1: JSXFragment (opening/closing fragment + children).
        self.error_cur("JSX fragments are not yet supported");
        None
    }

    /// Parse a `JSXElementName`: a plain `JSXIdentifier`, a `JSXNamespacedName`
    /// (`ns:name`), or a `.`-chained `JSXMemberExpression` (`a.b.c`). Port of
    /// `JSParserImpl::parseJSXElementName` (jsx.cpp:425-499).
    fn parse_jsx_element_name(
        &mut self,
        allow_jsx_member_expression: AllowJSXMemberExpression,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 427.
        let start = self.cur_start();

        // C++ 429-432: leading JSXIdentifier (reserved words allowed).
        if !self.check(TokenKind::identifier)
            && !self.lexer.token().is_res_word()
        {
            // C++ 430: "as JSX element name".
            self.error_expected_jsx_element_name("as JSX element name");
            return None;
        }

        // C++ 434-438.
        let name_range = self.cur_range();
        let mut name: &'gc Node<'gc> = self.set_location(
            name_range.start,
            name_range.end,
            Node::JSXIdentifier(JSXIdentifier::new(
                NodeMetadata::new(self.dummy_range()),
                self.lexer.token().get_res_word_or_identifier(),
            )),
        );
        self.advance(GrammarContext::AllowJSXIdentifier);

        // C++ 440-464: JSXNamespacedName (`JSXIdentifier : JSXIdentifier`).
        if self.check(TokenKind::colon) {
            // C++ 444.
            self.advance(GrammarContext::AllowJSXIdentifier);
            if !self.check(TokenKind::identifier)
                && !self.lexer.token().is_res_word()
            {
                // C++ 446-450: "in JSX element name".
                self.error_expected_jsx_element_name("in JSX element name");
                return None;
            }

            // C++ 454-459.
            let child_range = self.cur_range();
            let child = self.set_location(
                child_range.start,
                child_range.end,
                Node::JSXIdentifier(JSXIdentifier::new(
                    NodeMetadata::new(self.dummy_range()),
                    self.lexer.token().get_res_word_or_identifier(),
                )),
            );
            let child_end = child.metadata().range.get().end;
            self.advance(GrammarContext::AllowJSXIdentifier);
            // C++ 460-463.
            return Some(self.set_location(
                start,
                child_end,
                Node::JSXNamespacedName(JSXNamespacedName::new(
                    NodeMetadata::new(self.dummy_range()),
                    name,
                    child,
                )),
            ));
        }

        // C++ 466-491: JSXMemberExpression chain
        // (`JSXMemberExpression . JSXIdentifier`).
        while self.check(TokenKind::period) {
            // C++ 470.
            self.advance(GrammarContext::AllowJSXIdentifier);
            if !self.check(TokenKind::identifier)
                && !self.lexer.token().is_res_word()
            {
                // C++ 472-476: "in JSX element name".
                self.error_expected_jsx_element_name("in JSX element name");
                return None;
            }

            // C++ 480-485.
            let child_range = self.cur_range();
            let child = self.set_location(
                child_range.start,
                child_range.end,
                Node::JSXIdentifier(JSXIdentifier::new(
                    NodeMetadata::new(self.dummy_range()),
                    self.lexer.token().get_res_word_or_identifier(),
                )),
            );
            let child_end = child.metadata().range.get().end;
            self.advance(GrammarContext::AllowJSXIdentifier);

            // C++ 487-490.
            name = self.set_location(
                start,
                child_end,
                Node::JSXMemberExpression(JSXMemberExpression::new(
                    NodeMetadata::new(self.dummy_range()),
                    name,
                    child,
                )),
            );
        }

        // C++ 493-496: a JSXMemberExpression is invalid where only a plain
        // name/namespaced-name is allowed (e.g. attribute names).
        //
        // FAITHFUL-PORT NOTE: the C++ checks `isa<ESTree::MemberExpressionNode>`,
        // NOT `JSXMemberExpressionNode` — and `JSXMemberExpression` derives from
        // the `JSX` base, not `MemberExpression` (ESTree.def:782-785). So this
        // `isa<>` is ALWAYS false here (the only nodes built above are
        // `JSXIdentifier`/`JSXNamespacedName`/`JSXMemberExpression`), making the
        // C++ check effectively dead — the diagnostic never fires. We mirror the
        // C++ exactly by matching `Node::MemberExpression` (never produced by
        // this function), so the behavior stays byte-for-byte identical.
        if matches!(name, Node::MemberExpression(_))
            && allow_jsx_member_expression == AllowJSXMemberExpression::No
        {
            let range = name.metadata().range.get();
            self.error_at(range, "unexpected member expression");
        }

        Some(name)
    }

    /// Emit the C++ `errorExpected(TokenKind::identifier, where_, ...)`
    /// diagnostic for a JSX element name. The C++ uses two distinct `where_`
    /// strings: `"as JSX element name"` at the leading name (jsx.cpp:430) and
    /// `"in JSX element name"` at the `:`/`.` continuation sites
    /// (jsx.cpp:446-450 / 472-476). Rendered via the same "'<tok>' expected
    /// <where>" idiom as `need`/`error_expected*`.
    fn error_expected_jsx_element_name(&mut self, where_: &str) {
        let msg = format!(
            "'{}' expected {}",
            crate::token_kinds::token_kind_str(TokenKind::identifier),
            where_,
        );
        self.error_cur(&msg);
    }
}
