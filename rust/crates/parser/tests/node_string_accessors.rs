/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The generated per-field atom→string accessors on AST nodes, exercised
//! against a real parse (design:
//! `doc/superpowers/specs/2026-08-15-atom-string-accessors-design.md` §5.2).
//!
//! `gen_nodes.py` emits `<field>_str` for every `NodeLabel` field and
//! `try_<field>_str` + `<field>_str_lossy` (and deliberately no plain
//! `<field>_str`) for every `NodeString` field. The asymmetry is the point: an
//! identifier that has no UTF-8 form means something is broken, while a string
//! literal that has none is legal JS a codegen tool must not silently corrupt.
//!
//! These tests live in the `hermes-parser` crate rather than beside the
//! generator in `hermes-ast` because they need a real parse, and `hermes-ast`
//! must not dev-depend on `hermes-parser`. `hermes-ast` is published *before*
//! `hermes-parser` in the documented publish order (it is the foundation the
//! parser is built on), so at its own initial publication `hermes-parser` does
//! not exist yet. Beyond that first release, pointing an earlier-published
//! crate's dev-dependencies at a crate that depends on it makes every future
//! version bump ordering-sensitive in both directions — a hazard with no
//! upside when the parser crate can host the test unchanged.

use hermes_ast::context::{Context, GCLock};
use hermes_ast::node::{Node, NodeKind};
use hermes_ast::visitor::Visitor;
use hermes_parser::js::JSParserImpl;
use hermes_parser::lexer::{GrammarContext, JSLexer};
use hermes_support::manager::SourceErrorManager;

/// Pre-order search for the first node of `kind`, stopping at the first hit.
struct FindFirst<'gc> {
    kind: NodeKind,
    found: Option<&'gc Node<'gc>>,
}

impl<'gc> Visitor<'gc> for FindFirst<'gc> {
    fn visit_node(&mut self, node: &'gc Node<'gc>) {
        if self.found.is_some() {
            return;
        }
        if node.kind() == self.kind {
            self.found = Some(node);
            return;
        }
        node.visit_children(self);
    }
}

/// The first node of `kind` in `root`, in pre-order. Panics if there is none.
fn first<'gc>(root: &'gc Node<'gc>, kind: NodeKind) -> &'gc Node<'gc> {
    let mut f = FindFirst { kind, found: None };
    f.visit_node(root);
    f.found
        .unwrap_or_else(|| panic!("no {kind:?} node in the parsed tree"))
}

/// Parse `src` as a script (no dialect flags) and hand the resulting `Program`
/// to `body` along with the lock the nodes live in.
fn with_parsed<R>(
    src: &str,
    body: impl for<'gc> FnOnce(&'gc GCLock<'_, '_>, &'gc Node<'gc>) -> R,
) -> R {
    let mut sm = SourceErrorManager::new();
    let buf_id = sm.add_buffer("accessors.js", src);
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let program = {
        let atoms = &gc.ctx().atom_table;
        let lexer = JSLexer::new(buf_id, &mut sm, atoms, GrammarContext::AllowRegExp);
        let mut parser = JSParserImpl::new(&gc, lexer);
        parser.parse().unwrap_or_else(|| panic!("`{src}` must parse"))
    };
    body(&gc, program)
}

/// The canonical first-contact case the whole design exists for: walk to an
/// identifier and read its name as a `&str`, with no `.get()`/`bytes()` dance.
#[test]
fn identifier_name_reads_back_as_a_str() {
    with_parsed("function greet() { let x = 1; }\n", |gc, program| {
        let id = first(program, NodeKind::Identifier)
            .as_identifier()
            .expect("kind() said Identifier");
        assert_eq!(id.name_str(gc), "greet");
    });
}

/// A label field that is not an identifier name: `<field>_str` is generated
/// from the field name, so `operator` gets `operator_str`.
#[test]
fn binary_operator_label_reads_back_as_a_str() {
    with_parsed("a + b;\n", |gc, program| {
        let bin = first(program, NodeKind::BinaryExpression)
            .as_binary_expression()
            .expect("kind() said BinaryExpression");
        assert_eq!(bin.operator_str(gc), "+");
    });
}

/// An ordinary string literal, read both ways: `try_` yields `Some`, and the
/// lossy accessor yields the same text (and, being pure ASCII here, the very
/// same borrowed bytes).
#[test]
fn plain_string_literal_reads_back_both_ways() {
    with_parsed("var s = \"hello\";\n", |gc, program| {
        let lit = first(program, NodeKind::StringLiteral)
            .as_string_literal()
            .expect("kind() said StringLiteral");
        assert_eq!(lit.try_value_str(gc), Some("hello"));
        assert_eq!(lit.value_str_lossy(gc), "hello");
        assert_eq!(gc.bytes(lit.value.get()), b"hello");
        // Valid UTF-8 is borrowed straight from the atom, not rebuilt.
        assert_eq!(
            lit.try_value_str(gc).unwrap().as_ptr(),
            gc.bytes(lit.value.get()).as_ptr()
        );
    });
}

/// An astral character. The lexer interns it in WTF-8 **surrogate-pair** form,
/// which is not literally valid UTF-8 — but a pair and the character it encodes
/// are two spellings of the same string, so `try_value_str` must fold and
/// answer `Some`, not report the storage detail as a failure. This is the case
/// an earlier draft of the spec got wrong.
#[test]
fn emoji_string_literal_is_representable() {
    with_parsed("var s = \"\u{1F600}\";\n", |gc, program| {
        let lit = first(program, NodeKind::StringLiteral)
            .as_string_literal()
            .expect("kind() said StringLiteral");
        // The stored bytes really are the surrogate pair, not 4-byte UTF-8.
        assert_eq!(
            gc.bytes(lit.value.get()),
            &[0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80]
        );
        assert_eq!(lit.try_value_str(gc), Some("\u{1F600}"));
        assert_eq!(lit.value_str_lossy(gc), "\u{1F600}");
    });
}

/// An **unpaired** surrogate: a legal JS string value with no UTF-8 form. This
/// is the only thing `None` reports. The guarantee a codegen tool depends on is
/// the last assert: `bytes()` still round-trips the WTF-8 unchanged, so the
/// program can be re-emitted exactly even though no `&str` can carry it.
#[test]
fn lone_surrogate_string_literal_is_not_representable_but_survives() {
    with_parsed("var s = \"\\uD800\";\n", |gc, program| {
        let lit = first(program, NodeKind::StringLiteral)
            .as_string_literal()
            .expect("kind() said StringLiteral");
        assert_eq!(lit.try_value_str(gc), None);
        // Exactly one U+FFFD for the one unpaired surrogate — not three, which
        // is what `String::from_utf8_lossy` would produce for `ED A0 80`.
        assert_eq!(lit.value_str_lossy(gc), "\u{FFFD}");
        assert_eq!(gc.bytes(lit.value.get()), &[0xED, 0xA0, 0x80]);
    });
}

/// A directive is a `NodeString` on `ExpressionStatement`, so it gets the
/// `try_`/`_lossy` pair too — one more node kind, to show the generation is
/// per-field and not special-cased to `StringLiteral`.
#[test]
fn expression_statement_directive_reads_back_as_a_str() {
    with_parsed("\"use strict\";\n", |gc, program| {
        let stmt = first(program, NodeKind::ExpressionStatement)
            .as_expression_statement()
            .expect("kind() said ExpressionStatement");
        assert_eq!(stmt.try_directive_str(gc), Some("use strict"));
        assert_eq!(stmt.directive_str_lossy(gc), "use strict");
    });
}
