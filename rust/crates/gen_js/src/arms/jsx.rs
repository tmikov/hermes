/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! JSX: `JSXIdentifier`, `JSXMemberExpression`, `JSXNamespacedName`,
//! `JSXEmptyExpression`, `JSXExpressionContainer`, `JSXSpreadChild`,
//! `JSXOpeningElement`, `JSXClosingElement`, `JSXAttribute`,
//! `JSXSpreadAttribute`, `JSXStringLiteral`, `JSXText`, `JSXElement`,
//! `JSXFragment`, `JSXOpeningFragment`, `JSXClosingFragment`.
//!
//! Ported from juno `gen_js.rs:2000-2159`. This is the plan's Task 9.
//! `precedence.rs`'s `get_precedence` already classifies `JSXElement`/
//! `JSXFragment` at `PRIMARY` precedence (an earlier task), so no
//! parenthesization work is needed here — every arm below is a bare
//! `gen_node` sequence, exactly matching juno's bare `.visit(...)` calls:
//! JSX's own grammar delimiters (`<...>`, `{...}`) already disambiguate
//! every child position, the same reason juno never routes a JSX child
//! through `need_parens`.
//!
//! # `raw` fields go through `try_bytes_str`, not `ctx.str`
//!
//! `JSXIdentifier::name` is an ordinary identifier atom, handled exactly
//! like `arms/literal.rs`'s `gen_identifier`: `gc.try_bytes_str`, never
//! `gc.bytes()`/`bytes_str_lossy()` (module doc comment there has the full
//! rationale — an astral identifier is stored as a WTF-8 surrogate pair).
//!
//! `JSXStringLiteral::raw` and `JSXText::raw` are arbitrary source text (an
//! attribute value's exact spelling, including its quotes, or a run of JSX
//! child text between `<...>`/`{`/`<`), the same shape as
//! `arms/literal.rs`'s `TemplateElement::raw` — and, like that field, can
//! legitimately hold a literal astral character. `JSXStringLiteral::raw` in
//! particular is produced by the same lexer helper an ordinary string
//! literal's raw text goes through (`crates/parser/src/js/jsx.rs`'s
//! `parse_jsx_attribute`, `self.lexer.get_string_literal(...)`), which
//! WTF-8-encodes astral characters as surrogate pairs when the lexer's
//! `convert_surrogates` mode is on — so this goes through `try_bytes_str`
//! for the same reason `RegExpLiteral::pattern`/`TemplateElement::raw` do.
//! `None` becomes [`GenJsError::UnrepresentableIdentifier`], matching every
//! other raw-atom arm in the crate.
//!
//! # `JSXText`/`JSXStringLiteral` have their own escaping — no escaping at
//! all
//!
//! Both print `raw` verbatim, character by character, translating only `\n`
//! (routed through [`GenJS::force_newline_without_indent`] so indentation
//! state stays consistent, per [`GenJS::write_char`]'s "no newlines" rule —
//! not because the text itself needs re-escaping). `value` — the
//! entity-decoded ESTree string value (`&amp;` cooked to `&`) — is read by
//! neither arm and intentionally unused (`value: _`): JSX text/attribute
//! values have no escape syntax of their own to re-encode into (there is no
//! JSX equivalent of a string literal's `\n`/`\uXXXX` escapes — an `&amp;`
//! *is* the raw source spelling, not an escape sequence a generator chooses
//! to emit), so the only correct thing to print is the source text as
//! written. This is deliberately **not** [`GenJS::print_escaped_string_literal`]
//! (task brief): that method walks UTF-16 *code units* and backslash-escapes
//! control characters and the active quote for a JS string-literal context;
//! neither concept applies to a JSX text run or an unquoted-content
//! attribute value, and unifying them would either escape characters JSX
//! never expects escaped (breaking the round trip) or fail to preserve an
//! embedded literal `"`/`'` a `JSXStringLiteral`'s own quote character
//! doesn't conflict with (JSX attribute strings, unlike JS string literals,
//! don't support backslash escapes at all — the grammar has no
//! `JSXEscapeSequence` production). juno's two arms are textually identical;
//! [`GenJS::gen_jsx_raw_text`] is the one shared private helper both call
//! into, purely to avoid duplicating that identical body — it does not
//! change what either arm prints, and does not touch
//! `print_escaped_string_literal`.

use hermes_ast::context::GCLock;
use hermes_ast::node::{
    JSXAttribute, JSXClosingElement, JSXElement, JSXExpressionContainer, JSXFragment,
    JSXIdentifier, JSXMemberExpression, JSXNamespacedName, JSXOpeningElement, JSXSpreadAttribute,
    JSXSpreadChild, JSXStringLiteral, JSXText, Node, NodeField,
};
use hermes_ast::node_child::NodeLabel;
use hermes_ast::visitor::Path;

use crate::precedence::ForceSpace;
use crate::{out, GenJS, GenJsError};

impl<'s, 'w> GenJS<'s, 'w> {
    /// `JSXIdentifier`: its `name`, verbatim.
    ///
    /// juno `gen_js.rs:2000-2002`.
    pub(crate) fn gen_jsx_identifier(
        &mut self,
        ctx: &GCLock<'_, '_>,
        inner: &JSXIdentifier<'_>,
    ) -> Result<(), GenJsError> {
        let JSXIdentifier { metadata: _, name } = inner;
        let s = ctx
            .try_bytes_str(name.get())
            .ok_or(GenJsError::UnrepresentableIdentifier)?;
        self.write_utf8(s);
        Ok(())
    }

    /// `JSXMemberExpression`: `object.property`.
    ///
    /// juno `gen_js.rs:2003-2011`.
    pub(crate) fn gen_jsx_member_expression<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &JSXMemberExpression<'gc>,
    ) -> Result<(), GenJsError> {
        let JSXMemberExpression {
            metadata: _,
            object,
            property,
        } = inner;
        self.gen_node(ctx, object, Some(Path::new(node, NodeField::object)))?;
        out!(self, ".");
        self.gen_node(ctx, property, Some(Path::new(node, NodeField::property)))
    }

    /// `JSXNamespacedName`: `namespace:name`.
    ///
    /// juno `gen_js.rs:2012-2020`.
    pub(crate) fn gen_jsx_namespaced_name<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &JSXNamespacedName<'gc>,
    ) -> Result<(), GenJsError> {
        let JSXNamespacedName {
            metadata: _,
            namespace,
            name,
        } = inner;
        self.gen_node(ctx, namespace, Some(Path::new(node, NodeField::namespace)))?;
        out!(self, ":");
        self.gen_node(ctx, name, Some(Path::new(node, NodeField::name)))
    }

    /// `JSXEmptyExpression`: prints nothing — the zero-width content of a
    /// bare `{}` JSX child.
    ///
    /// juno `gen_js.rs:2021`. No fields besides `metadata`.
    pub(crate) fn gen_jsx_empty_expression(&mut self) -> Result<(), GenJsError> {
        Ok(())
    }

    /// `JSXExpressionContainer`: `{expression}`.
    ///
    /// juno `gen_js.rs:2022-2029`.
    pub(crate) fn gen_jsx_expression_container<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &JSXExpressionContainer<'gc>,
    ) -> Result<(), GenJsError> {
        let JSXExpressionContainer {
            metadata: _,
            expression,
        } = inner;
        out!(self, "{{");
        self.gen_node(
            ctx,
            expression,
            Some(Path::new(node, NodeField::expression)),
        )?;
        out!(self, "}}");
        Ok(())
    }

    /// `JSXSpreadChild`: `{...expression}`.
    ///
    /// juno `gen_js.rs:2030-2037`.
    pub(crate) fn gen_jsx_spread_child<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &JSXSpreadChild<'gc>,
    ) -> Result<(), GenJsError> {
        let JSXSpreadChild {
            metadata: _,
            expression,
        } = inner;
        out!(self, "{{...");
        self.gen_node(
            ctx,
            expression,
            Some(Path::new(node, NodeField::expression)),
        )?;
        out!(self, "}}");
        Ok(())
    }

    /// `JSXOpeningElement`: `<name[<type_arguments>][ attr]* />` or
    /// `<name[<type_arguments>][ attr]*>`.
    ///
    /// juno `gen_js.rs:2038-2063`.
    pub(crate) fn gen_jsx_opening_element<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &JSXOpeningElement<'gc>,
    ) -> Result<(), GenJsError> {
        let JSXOpeningElement {
            metadata: _,
            name,
            attributes,
            self_closing,
            type_arguments,
        } = inner;
        out!(self, "<");
        self.gen_node(ctx, name, Some(Path::new(node, NodeField::name)))?;
        if let Some(type_arguments) = type_arguments {
            self.gen_node(
                ctx,
                type_arguments,
                Some(Path::new(node, NodeField::type_arguments)),
            )?;
        }
        for attr in attributes.iter() {
            self.space(ForceSpace::Yes);
            self.gen_node(ctx, attr, Some(Path::new(node, NodeField::attributes)))?;
        }
        if self_closing.get() {
            out!(self, " />");
        } else {
            out!(self, ">");
        }
        Ok(())
    }

    /// `JSXClosingElement`: `</name>`.
    ///
    /// juno `gen_js.rs:2064-2068`.
    pub(crate) fn gen_jsx_closing_element<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &JSXClosingElement<'gc>,
    ) -> Result<(), GenJsError> {
        let JSXClosingElement { metadata: _, name } = inner;
        out!(self, "</");
        self.gen_node(ctx, name, Some(Path::new(node, NodeField::name)))?;
        out!(self, ">");
        Ok(())
    }

    /// `JSXAttribute`: `name` or `name=value`.
    ///
    /// juno `gen_js.rs:2069-2079`.
    pub(crate) fn gen_jsx_attribute<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &JSXAttribute<'gc>,
    ) -> Result<(), GenJsError> {
        let JSXAttribute {
            metadata: _,
            name,
            value,
        } = inner;
        self.gen_node(ctx, name, Some(Path::new(node, NodeField::name)))?;
        if let Some(value) = value {
            self.space_before_equals("=");
            out!(self, "=");
            self.gen_node(ctx, value, Some(Path::new(node, NodeField::value)))?;
        }
        Ok(())
    }

    /// `JSXSpreadAttribute`: `{...argument}`.
    ///
    /// juno `gen_js.rs:2080-2087`.
    pub(crate) fn gen_jsx_spread_attribute<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &JSXSpreadAttribute<'gc>,
    ) -> Result<(), GenJsError> {
        let JSXSpreadAttribute {
            metadata: _,
            argument,
        } = inner;
        out!(self, "{{...");
        self.gen_node(ctx, argument, Some(Path::new(node, NodeField::argument)))?;
        out!(self, "}}");
        Ok(())
    }

    /// Print `raw`'s text verbatim, translating a literal `\n` into
    /// [`GenJS::force_newline_without_indent`] (never a raw newline byte —
    /// [`GenJS::write_char`] forbids that) and every other character through
    /// unchanged. The one body shared by [`GenJS::gen_jsx_string_literal`]
    /// and [`GenJS::gen_jsx_text`] — see the module doc comment for why
    /// neither goes through [`GenJS::print_escaped_string_literal`].
    ///
    /// juno `gen_js.rs:2093-2100` / `2107-2114` (identical bodies, inlined at
    /// both call sites there).
    fn gen_jsx_raw_text(&mut self, ctx: &GCLock<'_, '_>, raw: NodeLabel) -> Result<(), GenJsError> {
        let s = ctx
            .try_bytes_str(raw)
            .ok_or(GenJsError::UnrepresentableIdentifier)?;
        let mut buf = [0u8; 4];
        for ch in s.chars() {
            if ch == '\n' {
                self.force_newline_without_indent();
                continue;
            }
            self.write_char(ch, &mut buf);
        }
        Ok(())
    }

    /// `JSXStringLiteral`: an attribute string value, printed as its `raw`
    /// spelling (quotes included) verbatim — JSX attribute strings have no
    /// backslash-escape syntax, so there is nothing to escape.
    ///
    /// juno `gen_js.rs:2088-2101`. See the module doc comment for why
    /// `value` (the entity-decoded cooked value) is unused.
    pub(crate) fn gen_jsx_string_literal(
        &mut self,
        ctx: &GCLock<'_, '_>,
        inner: &JSXStringLiteral<'_>,
    ) -> Result<(), GenJsError> {
        let JSXStringLiteral {
            metadata: _,
            value: _,
            raw,
        } = inner;
        self.gen_jsx_raw_text(ctx, raw.get())
    }

    /// `JSXText`: a run of JSX child text, printed as its `raw` spelling
    /// verbatim (including any literal `&entity;` spellings — `value`'s
    /// entity-decoded form is not what re-parses back to the same text).
    ///
    /// juno `gen_js.rs:2102-2115`. See the module doc comment for why
    /// `value` is unused.
    pub(crate) fn gen_jsx_text(
        &mut self,
        ctx: &GCLock<'_, '_>,
        inner: &JSXText<'_>,
    ) -> Result<(), GenJsError> {
        let JSXText {
            metadata: _,
            value: _,
            raw,
        } = inner;
        self.gen_jsx_raw_text(ctx, raw.get())
    }

    /// `JSXElement`: `opening_element[children...][closing_element]`. A
    /// self-closing element (`closing_element: None`) prints only the
    /// opening tag — `children` is unconditionally empty in that shape (the
    /// parser never populates it otherwise,
    /// `crates/parser/src/js/jsx.rs`'s `parse_jsx_element`), matching juno's
    /// own choice to gate the whole children loop on `closing_element` being
    /// present rather than on `children` being non-empty.
    ///
    /// juno `gen_js.rs:2116-2133`.
    pub(crate) fn gen_jsx_element<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &JSXElement<'gc>,
    ) -> Result<(), GenJsError> {
        let JSXElement {
            metadata: _,
            opening_element,
            children,
            closing_element,
        } = inner;
        self.gen_node(
            ctx,
            opening_element,
            Some(Path::new(node, NodeField::opening_element)),
        )?;
        if let Some(closing_element) = closing_element {
            for child in children.iter() {
                self.gen_node(ctx, child, Some(Path::new(node, NodeField::children)))?;
            }
            self.gen_node(
                ctx,
                closing_element,
                Some(Path::new(node, NodeField::closing_element)),
            )?;
        }
        Ok(())
    }

    /// `JSXFragment`: `<>children...</>`.
    ///
    /// juno `gen_js.rs:2134-2153`.
    pub(crate) fn gen_jsx_fragment<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &JSXFragment<'gc>,
    ) -> Result<(), GenJsError> {
        let JSXFragment {
            metadata: _,
            opening_fragment,
            children,
            closing_fragment,
        } = inner;
        self.gen_node(
            ctx,
            opening_fragment,
            Some(Path::new(node, NodeField::opening_fragment)),
        )?;
        for child in children.iter() {
            self.gen_node(ctx, child, Some(Path::new(node, NodeField::children)))?;
        }
        self.gen_node(
            ctx,
            closing_fragment,
            Some(Path::new(node, NodeField::closing_fragment)),
        )
    }

    /// `JSXOpeningFragment`: `<>`.
    ///
    /// juno `gen_js.rs:2154-2156`. No fields besides `metadata`.
    pub(crate) fn gen_jsx_opening_fragment(&mut self) -> Result<(), GenJsError> {
        out!(self, "<>");
        Ok(())
    }

    /// `JSXClosingFragment`: `</>`.
    ///
    /// juno `gen_js.rs:2157-2159`. No fields besides `metadata`.
    pub(crate) fn gen_jsx_closing_fragment(&mut self) -> Result<(), GenJsError> {
        out!(self, "</>");
        Ok(())
    }
}
