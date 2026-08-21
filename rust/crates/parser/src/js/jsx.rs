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
//! P8.0 landed the scaffolding + the self-closing-element happy path; P8.1
//! completes the rest: JSX children, fragments, attributes (incl. spread),
//! expression containers, and closing-tag matching (`tag_names_match`). The
//! only feature left guarded is the Flow `<TypeArgs>` on an opening tag, which
//! is wired faithfully via `parse_type_args_flow` but is only reachable with
//! `-parse-flow` enabled (see `parse_jsx_opening_element`).

use hermes_ast::node::{
    JSXAttribute, JSXClosingElement, JSXClosingFragment, JSXElement,
    JSXEmptyExpression, JSXExpressionContainer, JSXFragment, JSXIdentifier,
    JSXMemberExpression, JSXNamespacedName, JSXOpeningElement,
    JSXOpeningFragment, JSXSpreadAttribute, JSXSpreadChild, JSXStringLiteral,
    JSXText, Node,
};
use hermes_ast::node_child::{NodeList, NodeMetadata};

use hermes_support::location::SMLoc;

use crate::js::flow::{AllowTypedArrowFunction, CoverTypedParameters};
use crate::js::{JSParserImpl, PARAM_IN};
use crate::lexer::GrammarContext;
use crate::token_kinds::TokenKind;

/// Whether the opening and closing tag names match, which is needed to define
/// a JSXElement. Port of the file-static `tagNamesMatch` (jsx.cpp:32-75).
///
/// Recursively compares the `opening`/`closing` name nodes, walking the
/// `JSXMemberExpression._object` chain in lock-step. The C++ `cast<>` of a
/// namespace/property to `JSXIdentifierNode` is infallible (the ESTree spec
/// guarantees those slots are JSXIdentifier); we mirror that with the matching
/// `as_jsx_identifier().unwrap()`.
fn tag_names_match<'gc>(
    opening_name: &'gc Node<'gc>,
    closing_name: &'gc Node<'gc>,
) -> bool {
    // C++ 39-40.
    let mut name1 = opening_name;
    let mut name2 = closing_name;
    // C++ 41: for (;;).
    loop {
        if let Node::JSXIdentifier(name1_id) = name1 {
            // C++ 42-46.
            if let Node::JSXIdentifier(name2_id) = name2 {
                return name1_id.name.get() == name2_id.name.get();
            }
            return false;
        } else if let Node::JSXNamespacedName(name1_ns) = name1 {
            // C++ 47-57.
            if let Node::JSXNamespacedName(name2_ns) = name2 {
                // ESTree spec dictates that both namespace and name are
                // JSXIdentifier.
                let name1_ns_id =
                    name1_ns.namespace.as_jsx_identifier().unwrap();
                let name1_id = name1_ns.name.as_jsx_identifier().unwrap();
                let name2_ns_id =
                    name2_ns.namespace.as_jsx_identifier().unwrap();
                let name2_id = name2_ns.name.as_jsx_identifier().unwrap();
                return name1_ns_id.name.get() == name2_ns_id.name.get()
                    && name1_id.name.get() == name2_id.name.get();
            }
            return false;
        } else {
            // C++ 58-73: JSXMemberExpression.
            let name1_me = name1.as_jsx_member_expression().unwrap();
            if let Node::JSXMemberExpression(name2_me) = name2 {
                let name1_id =
                    name1_me.property.as_jsx_identifier().unwrap();
                let name2_id =
                    name2_me.property.as_jsx_identifier().unwrap();
                if name1_id.name.get() != name2_id.name.get() {
                    return false;
                }
                // Both names are JSXMemberExpression with matching property
                // names. Compare the object names.
                name1 = name1_me.object;
                name2 = name2_me.object;
                continue;
            }
            return false;
        }
    }
}

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
        // `parse_jsx_opening_element` always returns a `JSXOpeningElement`
        // (the C++ signature is typed `JSXOpeningElementNode*`); our untyped
        // `&Node` return needs the cast back, so bind it once here.
        let opening_el = opening
            .as_jsx_opening_element()
            .expect("parse_jsx_opening_element returns a JSXOpeningElement");
        // C++ 82-87: self-closing element has no children/closing.
        if opening_el.self_closing.get() {
            let end = opening.metadata().range().end;
            let node = Node::JSXElement(JSXElement::new(
                NodeMetadata::new(self.dummy_range()),
                opening,
                NodeList::empty(),
                None,
            ));
            return Some(self.set_location(start, end, node));
        }

        // C++ 90-95: parse JSXChildren; the returned node is the closing tag.
        let mut children: Vec<&'gc Node<'gc>> = Vec::new();
        let closing = self.parse_jsx_children(&mut children)?;

        // C++ 97-108: check the closing is not a fragment and the name matches.
        // The C++ `sm_.note` secondary diagnostics are dropped per house style.
        if let Node::JSXClosingElement(closing_el) = closing {
            let opening_name = opening_el.name;
            if !tag_names_match(opening_name, closing_el.name) {
                let range = closing.metadata().range();
                self.error_at(range, "Closing tag must match opening");
            }
        } else {
            let range = closing.metadata().range();
            self.error_at(range, "Closing tag must not be a fragment");
        }

        // C++ 110-114.
        let end = closing.metadata().range().end;
        let node = Node::JSXElement(JSXElement::new(
            NodeMetadata::new(self.dummy_range()),
            opening,
            NodeList::from_iter(self.gc, children),
            Some(closing),
        ));
        Some(self.set_location(start, end, node))
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
        //
        // FAITHFUL-PORT NOTE: `<TypeArgs>` is a Flow-only feature. The leading
        // `<` is only produced here when Flow type-argument syntax follows the
        // tag name, which the standalone `-parse-jsx` lexer never emits in a
        // JSX context (it stays in element-name mode). So this branch is only
        // reachable with `-parse-flow` *and* `-parse-jsx` both enabled. It is
        // wired exactly as the C++ via `parse_type_args_flow` (which does not
        // assert `parse_flow()`), so flow+jsx input parses identically; the
        // standalone JSX corpus simply cannot reach it.
        let type_arguments: Option<&'gc Node<'gc>> =
            if self.check(TokenKind::less) {
                // C++ 126-131.
                Some(self.parse_type_args_flow(
                    GrammarContext::AllowJSXIdentifier,
                )?)
            } else {
                None
            };

        // C++ 134-148: attributes loop.
        let mut attributes: Vec<&'gc Node<'gc>> = Vec::new();
        while !self.check2(TokenKind::slash, TokenKind::greater) {
            // C++ 136-142: spread attribute `{ ... expr }`.
            if self.check(TokenKind::l_brace) {
                attributes.push(self.parse_jsx_spread_attribute()?);
                continue;
            }

            // C++ 144-147.
            attributes.push(self.parse_jsx_attribute()?);
        }

        // C++ 150: default (AllowRegExp) context for `checkAndEat(slash)`.
        let self_closing =
            self.check_and_eat(TokenKind::slash, GrammarContext::AllowRegExp);

        // C++ 152-154.
        let end = self.cur_range().end;
        if !self.need_at(
            TokenKind::greater,
            " at end of JSX tag",
            Some("start of tag"),
            start,
        ) {
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
            NodeList::from_iter(self.gc, attributes),
            self_closing,
            type_arguments,
        ));
        Some(self.set_location(start, end, node))
    }

    /// Parse a `JSXFragment`. Port of `JSParserImpl::parseJSXFragment`
    /// (jsx.cpp:171-201).
    fn parse_jsx_fragment(&mut self, start: SMLoc) -> Option<&'gc Node<'gc>> {
        // C++ 172.
        debug_assert!(self.check(TokenKind::greater));
        // JSXFragment:
        // < > JSXChildren[opt] < / >
        //   ^
        // C++ 176: llvh::SaveAndRestore<uint32_t> saveDepth(jsxDepth_,
        // jsxDepth_ + 1).
        let _depth_guard = self.save_jsx_depth(self.jsx_depth.get() + 1);
        // C++ 177-178.
        let frag_end = self.cur_range().end;
        let opening = self.set_location(
            start,
            frag_end,
            Node::JSXOpeningFragment(JSXOpeningFragment::new(
                NodeMetadata::new(self.dummy_range()),
            )),
        );
        // C++ 179.
        self.lexer.advance_in_jsx_child();

        // C++ 181-186: parse JSXChildren.
        let mut children: Vec<&'gc Node<'gc>> = Vec::new();
        let closing = self.parse_jsx_children(&mut children)?;

        // C++ 188-194: check that the closing is a fragment. The C++ `note`
        // secondary diagnostic is dropped per house style.
        if !matches!(closing, Node::JSXClosingFragment(_)) {
            let range = closing.metadata().range();
            self.error_at(range, "Closing tag must be a fragment");
            return None;
        }

        // C++ 196-200.
        let end = closing.metadata().range().end;
        let node = Node::JSXFragment(JSXFragment::new(
            NodeMetadata::new(self.dummy_range()),
            opening,
            NodeList::from_iter(self.gc, children),
            closing,
        ));
        Some(self.set_location(start, end, node))
    }

    /// Parse the children of a JSXElement/JSXFragment, returning the closing
    /// tag node (a `JSXClosingElement` or `JSXClosingFragment`). Port of
    /// `JSParserImpl::parseJSXChildren` (jsx.cpp:203-270).
    fn parse_jsx_children(
        &mut self,
        children: &mut Vec<&'gc Node<'gc>>,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 206: keep looping until we encounter a closing element or a
        // JSXClosingFragment.
        loop {
            if self.check(TokenKind::less) {
                // C++ 207-226: JSXElement or closing tag.
                let start =
                    self.advance(GrammarContext::AllowJSXIdentifier).start;
                if self.check(TokenKind::slash) {
                    // < /
                    //   ^
                    // C++ 210-218: start of a JSXClosingElement or
                    // JSXClosingFragment. Return it as the closing.
                    return self.parse_jsx_closing(start);
                }
                // C++ 219-226: using a JSXFragment as a child node appears to
                // be disallowed by the spec, but code frequently uses this
                // pattern and all parsers appear to support it.
                let elem = if self.check(TokenKind::greater) {
                    self.parse_jsx_fragment(start)?
                } else {
                    self.parse_jsx_element(start)?
                };
                children.push(elem);
            } else if self.check(TokenKind::l_brace) {
                // C++ 227-257: { JSXChildExpression[opt] }
                //               ^
                // C++ 230-231: default (AllowRegExp) context.
                let start_range = self.advance(GrammarContext::AllowRegExp);
                let start = start_range.start;
                if self.check(TokenKind::r_brace) {
                    // { }
                    //   ^
                    // C++ 232-242: the empty expression is zero-width between
                    // the braces.
                    let end_range = self.cur_range();
                    let empty = self.set_location(
                        start_range.end,
                        end_range.start,
                        Node::JSXEmptyExpression(JSXEmptyExpression::new(
                            NodeMetadata::new(self.dummy_range()),
                        )),
                    );
                    let container = self.set_location(
                        start,
                        end_range.end,
                        Node::JSXExpressionContainer(
                            JSXExpressionContainer::new(
                                NodeMetadata::new(self.dummy_range()),
                                empty,
                            ),
                        ),
                    );
                    children.push(container);
                } else {
                    // C++ 243-256: { JSXChildExpression }
                    //                 ^
                    let child_expr = self.parse_jsx_child_expression(start)?;
                    if !self.need_at(
                        TokenKind::r_brace,
                        " in JSX child expression",
                        Some("start of expression"),
                        start,
                    ) {
                        return None;
                    }
                    children.push(child_expr);
                }
                // C++ 257.
                self.lexer.advance_in_jsx_child();
            } else {
                // C++ 259-267: JSXText handled by the lexer. C++ 260 passes
                // `nullptr, {}` — a genuine no-hint site — so this stays the
                // plain `need(kind, where)` form; there is no note to
                // restore here.
                if !self.need(TokenKind::jsx_text, " in JSX child expression")
                {
                    return None;
                }
                let tok_range = self.cur_range();
                let value = self.lexer.token().get_jsx_text_value();
                let raw = self.lexer.token().get_jsx_text_raw();
                let text = self.set_location(
                    tok_range.start,
                    tok_range.end,
                    Node::JSXText(JSXText::new(
                        NodeMetadata::new(self.dummy_range()),
                        value,
                        raw,
                    )),
                );
                children.push(text);
                // C++ 267.
                self.lexer.advance_in_jsx_child();
            }
        }
    }

    /// Parse the expression inside a JSX child `{ ... }`: either a
    /// `JSXSpreadChild` (`{...expr}`) or a `JSXExpressionContainer` (`{expr}`).
    /// Port of `JSParserImpl::parseJSXChildExpression` (jsx.cpp:272-287).
    fn parse_jsx_child_expression(
        &mut self,
        start: SMLoc,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 273-279: spread child `{ ... expr }`.
        if self.check_and_eat(TokenKind::dotdotdot, GrammarContext::AllowRegExp)
        {
            let assign = self.parse_assignment_expression(
                PARAM_IN,
                false,
                AllowTypedArrowFunction::Yes,
                CoverTypedParameters::Yes,
                None,
            )?;
            let end = self.cur_range().end;
            return Some(self.set_location(
                start,
                end,
                Node::JSXSpreadChild(JSXSpreadChild::new(
                    NodeMetadata::new(self.dummy_range()),
                    assign,
                )),
            ));
        }
        // C++ 280-286.
        let assign = self.parse_assignment_expression(
            PARAM_IN,
            false,
            AllowTypedArrowFunction::Yes,
            CoverTypedParameters::Yes,
            None,
        )?;
        let end = self.cur_range().end;
        Some(self.set_location(
            start,
            end,
            Node::JSXExpressionContainer(JSXExpressionContainer::new(
                NodeMetadata::new(self.dummy_range()),
                assign,
            )),
        ))
    }

    /// Parse a `{ ...expr }` spread attribute. Port of
    /// `JSParserImpl::parseJSXSpreadAttribute` (jsx.cpp:289-319).
    fn parse_jsx_spread_attribute(&mut self) -> Option<&'gc Node<'gc>> {
        // C++ 290.
        debug_assert!(self.check(TokenKind::l_brace));
        // C++ 291: default (AllowRegExp) context.
        let start = self.advance(GrammarContext::AllowRegExp).start;

        // { ... AssignmentExpression }
        //   ^
        // C++ 296-302.
        if !self.eat_at(
            TokenKind::dotdotdot,
            GrammarContext::AllowRegExp,
            " in JSX spread attribute",
            Some("location of attribute"),
            start,
        ) {
            return None;
        }

        // C++ 304-306.
        let assign = self.parse_assignment_expression(
            PARAM_IN,
            false,
            AllowTypedArrowFunction::Yes,
            CoverTypedParameters::Yes,
            None,
        )?;

        // C++ 308-315.
        let end = self.cur_range().end;
        if !self.eat_at(
            TokenKind::r_brace,
            GrammarContext::AllowJSXIdentifier,
            " in JSX spread attribute",
            Some("location of attribute"),
            start,
        ) {
            return None;
        }

        // C++ 317-318.
        Some(self.set_location(
            start,
            end,
            Node::JSXSpreadAttribute(JSXSpreadAttribute::new(
                NodeMetadata::new(self.dummy_range()),
                assign,
            )),
        ))
    }

    /// Parse a single JSX attribute (`name`, `name="str"`, or `name={expr}`).
    /// Port of `JSParserImpl::parseJSXAttribute` (jsx.cpp:321-384).
    fn parse_jsx_attribute(&mut self) -> Option<&'gc Node<'gc>> {
        // C++ 322.
        let start = self.cur_start();

        // C++ 324-327.
        let name = self.parse_jsx_element_name(AllowJSXMemberExpression::No)?;

        // C++ 329-335: no `=` → bare attribute. The `=` is eaten in
        // AllowJSXIdentifier context.
        if !self.check_and_eat(
            TokenKind::equal,
            GrammarContext::AllowJSXIdentifier,
        ) {
            let name_range = name.metadata().range();
            return Some(self.set_location(
                name_range.start,
                name_range.end,
                Node::JSXAttribute(JSXAttribute::new(
                    NodeMetadata::new(self.dummy_range()),
                    name,
                    None,
                )),
            ));
        }

        // JSXAttributeInitializer:
        // = JSXAttributeValue
        //   ^
        // C++ 340-378.
        let value: &'gc Node<'gc> = if self.check(TokenKind::string_literal) {
            // C++ 341-348.
            let raw =
                self.lexer.get_string_literal(self.lexer.token_input_str());
            let str_value = self.lexer.token().get_string_literal();
            let tok_range = self.cur_range();
            let v = self.set_location(
                tok_range.start,
                tok_range.end,
                Node::JSXStringLiteral(JSXStringLiteral::new(
                    NodeMetadata::new(self.dummy_range()),
                    str_value,
                    raw,
                )),
            );
            self.advance(GrammarContext::AllowJSXIdentifier);
            v
        } else {
            // { AssignmentExpression }
            // ^
            // C++ 349-377.
            if !self.need_at(
                TokenKind::l_brace,
                " in JSX attribute",
                Some("location of attribute"),
                start,
            ) {
                return None;
            }
            // C++ 359: default (AllowRegExp) context.
            let value_start = self.advance(GrammarContext::AllowRegExp).start;

            let assign = self.parse_assignment_expression(
                PARAM_IN,
                false,
                AllowTypedArrowFunction::Yes,
                CoverTypedParameters::Yes,
                None,
            )?;

            let value_end = self.cur_range().end;
            if !self.eat_at(
                TokenKind::r_brace,
                GrammarContext::AllowJSXIdentifier,
                " in JSX attribute",
                Some("location of attribute"),
                start,
            ) {
                return None;
            }

            self.set_location(
                value_start,
                value_end,
                Node::JSXExpressionContainer(JSXExpressionContainer::new(
                    NodeMetadata::new(self.dummy_range()),
                    assign,
                )),
            )
        };

        // C++ 382-383.
        let value_end = value.metadata().range().end;
        Some(self.set_location(
            start,
            value_end,
            Node::JSXAttribute(JSXAttribute::new(
                NodeMetadata::new(self.dummy_range()),
                name,
                Some(value),
            )),
        ))
    }

    /// Parse a JSX closing tag (`</name>` or `</>`), returning a
    /// `JSXClosingElement` or `JSXClosingFragment`. Port of
    /// `JSParserImpl::parseJSXClosing` (jsx.cpp:386-423). `start` is the `<`.
    fn parse_jsx_closing(&mut self, start: SMLoc) -> Option<&'gc Node<'gc>> {
        // C++ 387.
        debug_assert!(self.check(TokenKind::slash));
        // C++ 388.
        self.advance(GrammarContext::AllowJSXIdentifier);

        // C++ 390-400: `</>` is a JSXClosingFragment.
        if self.check(TokenKind::greater) {
            let end = self.cur_range().end;
            // C++ 392-397: the depth-driven lexer-mode switch.
            if self.jsx_depth.get() > 1 {
                self.lexer.advance_in_jsx_child();
            } else {
                // Done with JSX, advance normally.
                self.advance(GrammarContext::AllowRegExp);
            }
            return Some(self.set_location(
                start,
                end,
                Node::JSXClosingFragment(JSXClosingFragment::new(
                    NodeMetadata::new(self.dummy_range()),
                )),
            ));
        }

        // C++ 402-404.
        let name = self.parse_jsx_element_name(AllowJSXMemberExpression::Yes)?;

        // C++ 406-411.
        if !self.need_at(
            TokenKind::greater,
            " at end of JSX closing tag",
            Some("start of tag"),
            start,
        ) {
            return None;
        }

        // C++ 413-419: the same depth-driven lexer-mode switch.
        let end = self.cur_range().end;
        if self.jsx_depth.get() > 1 {
            self.lexer.advance_in_jsx_child();
        } else {
            // Done with JSX, advance normally.
            self.advance(GrammarContext::AllowRegExp);
        }

        // C++ 421-422.
        Some(self.set_location(
            start,
            end,
            Node::JSXClosingElement(JSXClosingElement::new(
                NodeMetadata::new(self.dummy_range()),
                name,
            )),
        ))
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
            // C++ 430: "as JSX element name", what = nullptr, whatLoc = {}
            // — a genuine no-hint site; no note to restore.
            self.error_expected_jsx_element_name(
                " as JSX element name",
                None,
                None,
            );
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
                // C++ 446-450: "in JSX element name", what = "start of JSX
                // element name", whatLoc = `start`.
                self.error_expected_jsx_element_name(
                    " in JSX element name",
                    Some("start of JSX element name"),
                    Some(start),
                );
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
            let child_end = child.metadata().range().end;
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
                // C++ 472-476: "in JSX element name", what = "start of JSX
                // element name", whatLoc = `start`.
                self.error_expected_jsx_element_name(
                    " in JSX element name",
                    Some("start of JSX element name"),
                    Some(start),
                );
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
            let child_end = child.metadata().range().end;
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
        // HISTORY: the C++ used to check `isa<ESTree::MemberExpressionNode>`,
        // which is ALWAYS false here — `JSXMemberExpression` derives from the
        // `JSX` base, not `MemberExpression` (ESTree.def:782-785), and the only
        // nodes built above are `JSXIdentifier`/`JSXNamespacedName`/
        // `JSXMemberExpression` — so the diagnostic never fired and
        // `<foo a.b="1"/>` was accepted. The port mirrored that dead check;
        // upstream fixed it in `37520ccef` ("Fix rejection of member
        // expressions as JSX attribute names") by testing
        // `JSXMemberExpressionNode`, and this is the mirror of that fix.
        if matches!(name, Node::JSXMemberExpression(_))
            && allow_jsx_member_expression == AllowJSXMemberExpression::No
        {
            let range = name.metadata().range();
            self.error_at(range, "unexpected member expression");
        }

        Some(name)
    }

    /// Emit the C++ `errorExpected(TokenKind::identifier, where_, what,
    /// whatLoc)` diagnostic for a JSX element name. The C++ uses two
    /// distinct `where_` strings AND two distinct `what`/`whatLoc` pairs:
    /// `"as JSX element name"` at the leading name (jsx.cpp:430) passes
    /// `nullptr, {}` — no hint at all, so the caller passes `None, None`.
    /// The `:`/`.` continuation sites (jsx.cpp:446-450 / 472-476,
    /// `"in JSX element name"`) pass a real hint, `"start of JSX element
    /// name"` at `start`, so the caller passes `Some("start of JSX element
    /// name"), Some(start)`. Rendered via the same "'<tok>' expected<where_>"
    /// idiom as `need`/`error_expected*` — like those, `where_` itself must
    /// carry the leading space (C++'s `errorExpected` inserts it via
    /// `ss << " " << where`); callers pass `" as JSX element name"`/`" in
    /// JSX element name"`, not the bare C++ literal.
    fn error_expected_jsx_element_name(
        &mut self,
        where_: &str,
        what: Option<&str>,
        what_loc: Option<SMLoc>,
    ) {
        let msg = format!(
            "'{}' expected{}",
            crate::token_kinds::token_kind_str(TokenKind::identifier),
            where_,
        );
        self.error_expected_msg(&msg, what, what_loc);
    }
}
