/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Literals, identifiers, templates, and string escaping.
//!
//! Ported from juno `gen_js.rs:828-867` (literals through `Super`),
//! `gen_js.rs:1260-1266` (`Directive`/`DirectiveLiteral`), `gen_js.rs:1299-1334`
//! (`Identifier`, `PrivateName`, `MetaProperty`), `gen_js.rs:1388-1441`
//! (`TemplateLiteral`, `TaggedTemplateExpression`, `TemplateElement`), and
//! `gen_js.rs:3300-3351` (`print_escaped_string_literal`). This is the plan's
//! Task 4, and the module the port's WTF-8 encoding rules bite hardest: see
//! [`GenJS::gen_identifier`] and [`GenJS::print_escaped_string_literal`].
//!
//! # The two encoding rules (spec §5)
//!
//! **Identifiers.** Our atoms hold astral characters as WTF-8 **surrogate
//! pairs**, not 4-byte UTF-8, because [`hermes_atom_table`] stores whatever
//! bytes the lexer handed it verbatim. Astral characters are legal
//! `ID_Start`/`ID_Continue` (e.g. U+1D465 MATHEMATICAL ITALIC SMALL X), so an
//! identifier atom can legitimately contain a surrogate pair. Writing those
//! bytes out verbatim (`gc.bytes(atom)`) would emit invalid UTF-8 — a
//! surrogate pair encoded as two separate 3-byte WTF-8 sequences is not the
//! same bytes as the one 4-byte UTF-8 sequence for the astral codepoint they
//! represent. [`GCLock::try_bytes_str`] re-decodes the atom surrogate-aware,
//! folding a pair back into its one astral `char`, which is exactly the
//! needed re-encoding — every arm here that emits identifier-shaped atom
//! text (`Identifier::name`, `BigIntLiteral::bigint`,
//! `RegExpLiteral::pattern`/`flags`, `TemplateElement::raw`) goes through it,
//! never `gc.bytes()` and never `gc.bytes_str_lossy()` (whose U+FFFD
//! substitution would silently emit a different program). `None` means an
//! *unpaired* surrogate, which has no JS spelling at all (not even a
//! `\uD800`-style escape — the lexer rejects that as an identifier start),
//! so it becomes [`GenJsError::UnrepresentableIdentifier`] rather than a
//! substitution.
//!
//! **String literals.** [`GenJS::print_escaped_string_literal`] walks UTF-16
//! *code units*, not `char`s, via
//! [`convert_utf8_with_surrogates_to_utf16`] on the atom's raw WTF-8 bytes.
//! This is what makes a lone (unpaired) surrogate in a string literal —
//! `"\uD800"`, a value string literals can legally hold even though
//! identifiers can't — come out as exactly one `\ud800` escape, rather than
//! being torn into three U+FFFD replacement characters the way a `char`-based
//! walk would.

use hermes_ast::context::GCLock;
use hermes_ast::node::{
    BigIntLiteral, BooleanLiteral, Directive, DirectiveLiteral, Identifier, MetaProperty, Node,
    NodeField, NumericLiteral, PrivateName, RegExpLiteral, StringLiteral, TaggedTemplateExpression,
    TemplateElement, TemplateLiteral,
};
use hermes_ast::node_child::NodeString;
use hermes_ast::visitor::Path;

use crate::precedence::{ChildPos, ForceSpace};
use crate::{out, GenJS, GenJsError};

impl<'s, 'w> GenJS<'s, 'w> {
    /// `BooleanLiteral`: `true`/`false`.
    ///
    /// juno `gen_js.rs:828-830`.
    pub(crate) fn gen_boolean_literal(
        &mut self,
        inner: &BooleanLiteral<'_>,
    ) -> Result<(), GenJsError> {
        let BooleanLiteral { metadata: _, value } = inner;
        out!(self, "{}", if value.get() { "true" } else { "false" });
        Ok(())
    }

    /// `NullLiteral`: `null`.
    ///
    /// juno `gen_js.rs:831-833`. `NullLiteral` has no fields besides
    /// `metadata`, so there is nothing to destructure.
    pub(crate) fn gen_null_literal(&mut self) -> Result<(), GenJsError> {
        out!(self, "null");
        Ok(())
    }

    /// `StringLiteral`: a quoted, escaped string.
    ///
    /// juno `gen_js.rs:834-838`.
    pub(crate) fn gen_string_literal(
        &mut self,
        ctx: &GCLock<'_, '_>,
        inner: &StringLiteral<'_>,
    ) -> Result<(), GenJsError> {
        let StringLiteral { metadata: _, value } = inner;
        let quote = self.quote().as_char();
        out!(self, "{}", quote);
        self.print_escaped_string_literal(ctx, value.get(), quote);
        out!(self, "{}", quote);
        Ok(())
    }

    /// `NumericLiteral`: a number, formatted per `Number::toString`.
    ///
    /// juno `gen_js.rs:839-841`.
    ///
    /// **DEVIATION from juno — a correctness fix, not a transcription**, for
    /// the same reason and in the same shape as
    /// [`GenJS::gen_bigint_literal`]'s below. `Number::toString` spells an
    /// infinite value `Infinity` / `-Infinity`, which is not a numeric
    /// literal at all: reparsing it yields an `Identifier` (or a
    /// `UnaryExpression` over one), a different tree, with no diagnostic.
    /// And an infinity is reachable straight from source — a literal whose
    /// value overflows `f64` is not an error, it is `+inf`
    /// (`test/Parser/extreme-numbers.js`:
    /// `55e55555555555555555555555555555555555;`, which the Tier 2 sweep
    /// caught). `1e999` is a `NumericLiteral` whose value is exactly `+inf`,
    /// so it round-trips; `-1e999` reparses as a `UnaryExpression` rather
    /// than as the negative literal, but a `NumericLiteral` node is never
    /// negative coming out of the parser (unary minus is its own node), so
    /// that spelling exists only to keep a hand-built tree's *value*
    /// correct. `NaN` is likewise unreachable from a parse — no source
    /// spelling of a numeric literal produces it — and has no literal
    /// spelling to fall back on, so it is left to `number_to_string`.
    pub(crate) fn gen_numeric_literal(
        &mut self,
        inner: &NumericLiteral<'_>,
    ) -> Result<(), GenJsError> {
        let NumericLiteral { metadata: _, value } = inner;
        let value = value.get();
        if value == f64::INFINITY {
            out!(self, "1e999");
        } else if value == f64::NEG_INFINITY {
            out!(self, "-1e999");
        } else {
            out!(self, "{}", hermes_support::json_emitter::number_to_string(value));
        }
        Ok(())
    }

    /// `BigIntLiteral`: a decimal-digit-string BigInt, plus the trailing `n`
    /// suffix that makes it re-parse as one.
    ///
    /// juno `gen_js.rs:842-848`: `self.write_utf8(ctx.str(*bigint))`, with no
    /// `n` appended.
    ///
    /// **DEVIATION from juno — a correctness fix, not a transcription.**
    /// `BigIntLiteral::bigint` is the ESTree `bigint` property, whose value
    /// is specified (and matches what `hermes-parser`'s own JS-side
    /// `BigIntLiteral` builder does, `raw: ${value}n` /
    /// `bigint: ${value}`) to be the literal's digits *without* the `n`
    /// suffix — our lexer strips it too (`lexer/number.rs`'s bigint path
    /// drops `raw[..raw.len() - 1]`, dropping exactly the trailing `n`).
    /// Emitting `bigint`'s text bare, as juno does, reprints e.g. `123n` as
    /// `123` — a `NumericLiteral` spelling, not a `BigIntLiteral` one, which
    /// reparses to a different value (a `Number`, not a `BigInt`) and fails
    /// the round-trip property spec §7.1 makes this crate's correctness bar.
    /// Per spec §1 ("assume juno's generator has bugs; the plan's job is to
    /// find them"), this is fixed here, the same way `precedence.rs` fixed
    /// juno's `**` associativity bug rather than porting it faithfully.
    pub(crate) fn gen_bigint_literal(
        &mut self,
        ctx: &GCLock<'_, '_>,
        inner: &BigIntLiteral<'_>,
    ) -> Result<(), GenJsError> {
        let BigIntLiteral {
            metadata: _,
            bigint,
        } = inner;
        let s = ctx
            .try_bytes_str(bigint.get())
            .ok_or(GenJsError::UnrepresentableIdentifier)?;
        self.write_utf8(s);
        out!(self, "n");
        Ok(())
    }

    /// `RegExpLiteral`: `/pattern/flags`.
    ///
    /// juno `gen_js.rs:849-859`. juno's own comment says the parser doesn't
    /// handle escapes when lexing RegExp, so `pattern` is re-emitted
    /// verbatim; that much carries over unchanged (and spec §8's review
    /// focus area 4 flags this arm for the same suspicion as the C++
    /// `AST2JS`'s bare `// FIXME: escaping, etc?`, `AST2JS.cpp:126` — noted,
    /// not further investigated by this task). What *does* change from
    /// `ctx.str(*pattern)`/`ctx.str(*flags)` is the WTF-8 decode: a regex
    /// pattern is arbitrary source text and can contain a literal astral
    /// character (e.g. `/𝑥/`), stored the same surrogate-pair way an
    /// identifier atom is, so this goes through `try_bytes_str` for the same
    /// reason `Identifier::name` does (module doc comment).
    pub(crate) fn gen_regexp_literal(
        &mut self,
        ctx: &GCLock<'_, '_>,
        inner: &RegExpLiteral<'_>,
    ) -> Result<(), GenJsError> {
        let RegExpLiteral {
            metadata: _,
            pattern,
            flags,
        } = inner;
        out!(self, "/");
        let pattern_str = ctx
            .try_bytes_str(pattern.get())
            .ok_or(GenJsError::UnrepresentableIdentifier)?;
        self.write_utf8(pattern_str);
        out!(self, "/");
        let flags_str = ctx
            .try_bytes_str(flags.get())
            .ok_or(GenJsError::UnrepresentableIdentifier)?;
        self.write_utf8(flags_str);
        Ok(())
    }

    /// `ThisExpression`: `this`.
    ///
    /// juno `gen_js.rs:860-862`. No fields besides `metadata`.
    pub(crate) fn gen_this_expression(&mut self) -> Result<(), GenJsError> {
        out!(self, "this");
        Ok(())
    }

    /// `Super`: `super`.
    ///
    /// juno `gen_js.rs:863-865`. No fields besides `metadata`.
    pub(crate) fn gen_super(&mut self) -> Result<(), GenJsError> {
        out!(self, "super");
        Ok(())
    }

    /// `Identifier`: its name, an optional `?`, and an optional type
    /// annotation.
    ///
    /// juno `gen_js.rs:1299-1320`. See the module doc comment for why `name`
    /// goes through `try_bytes_str` rather than `ctx.str`/`gc.bytes`.
    /// `self.annotate_identifier(ctx, node)` (juno `gen_js.rs:1307`) is
    /// [`GenJS::annotate_identifier`] in `annotate.rs` (Task 14).
    pub(crate) fn gen_identifier<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &Identifier<'gc>,
    ) -> Result<(), GenJsError> {
        let Identifier {
            metadata: _,
            name,
            type_annotation,
            optional,
            // Sema decorations (grandfathered onto the AST node itself, per
            // `ast-annotation-principle`): not read until Task 14 wires up
            // `annotate_identifier`.
            unresolvable: _,
            decl_state: _,
            decl: _,
        } = inner;
        let s = ctx
            .try_bytes_str(name.get())
            .ok_or(GenJsError::UnrepresentableIdentifier)?;
        self.write_utf8(s);
        self.annotate_identifier(ctx, node);
        if optional.get() {
            out!(self, "?");
        }
        if let Some(type_annotation) = type_annotation {
            out!(self, ":");
            self.space(ForceSpace::No);
            self.gen_node(
                ctx,
                type_annotation,
                Some(Path::new(node, NodeField::type_annotation)),
            )?;
        }
        Ok(())
    }

    /// `PrivateName`: `#` followed by its identifier.
    ///
    /// juno `gen_js.rs:1321-1324`.
    pub(crate) fn gen_private_name<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &PrivateName<'gc>,
    ) -> Result<(), GenJsError> {
        let PrivateName { metadata: _, id } = inner;
        out!(self, "#");
        self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))?;
        Ok(())
    }

    /// `MetaProperty`: `meta.property` (e.g. `new.target`, `import.meta`).
    ///
    /// juno `gen_js.rs:1325-1334`.
    pub(crate) fn gen_meta_property<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &MetaProperty<'gc>,
    ) -> Result<(), GenJsError> {
        let MetaProperty {
            metadata: _,
            meta,
            property,
        } = inner;
        self.gen_node(ctx, meta, Some(Path::new(node, NodeField::meta)))?;
        out!(self, ".");
        self.gen_node(ctx, property, Some(Path::new(node, NodeField::property)))?;
        Ok(())
    }

    /// `Directive`: a directive-prologue entry, printed as its
    /// `DirectiveLiteral` child (the statement-level trailing `;` is
    /// `ExpressionStatement`/statement-list machinery, a later task).
    ///
    /// juno `gen_js.rs:1260-1262`.
    pub(crate) fn gen_directive<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &Directive<'gc>,
    ) -> Result<(), GenJsError> {
        let Directive { metadata: _, value } = inner;
        self.gen_node(ctx, value, Some(Path::new(node, NodeField::value)))?;
        Ok(())
    }

    /// `DirectiveLiteral`: the quoted string of a directive-prologue entry
    /// (e.g. `"use strict"`).
    ///
    /// juno `gen_js.rs:1263-1265`: `unimplemented!("No escaping for
    /// directive literals")`.
    ///
    /// **DEVIATION from juno — a completeness fix, not a transcription.**
    /// `DirectiveLiteral::value` is a `NodeString`, the exact same shape as
    /// `StringLiteral::value` (`ESTree.def`'s `DirectiveLiteral` and
    /// `StringLiteral` both take one `NodeString value`), and a directive
    /// prologue entry is syntactically an ordinary string literal — there is
    /// no structural reason this can't reuse
    /// [`GenJS::print_escaped_string_literal`] the same way `StringLiteral`'s
    /// arm does. juno's `unimplemented!` reads as an unfinished stub rather
    /// than a deliberate gap. Per spec §1 ("assume juno's generator has
    /// bugs") and `implement-components-completely`, this is implemented
    /// here rather than carried over as a panic or a
    /// [`GenJsError::UnsupportedKind`].
    pub(crate) fn gen_directive_literal(
        &mut self,
        ctx: &GCLock<'_, '_>,
        inner: &DirectiveLiteral<'_>,
    ) -> Result<(), GenJsError> {
        let DirectiveLiteral { metadata: _, value } = inner;
        let quote = self.quote().as_char();
        out!(self, "{}", quote);
        self.print_escaped_string_literal(ctx, value.get(), quote);
        out!(self, "{}", quote);
        Ok(())
    }

    /// `TemplateLiteral`: `` `raw0${expr0}raw1${expr1}raw2` ``.
    ///
    /// juno `gen_js.rs:1388-1421`. `quasis`' raw text goes through
    /// `try_bytes_str`, for the same reason `RegExpLiteral::pattern` does
    /// (a template's raw spelling is arbitrary source text and can hold a
    /// literal astral character, WTF-8-encoded as a surrogate pair) — juno's
    /// `ctx.str(*raw)` has no equivalent concern since juno's atoms aren't
    /// WTF-8. A raw `\n` still forces a newline without indentation rather
    /// than being written as a literal `\n` byte, matching juno.
    pub(crate) fn gen_template_literal<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TemplateLiteral<'gc>,
    ) -> Result<(), GenJsError> {
        let TemplateLiteral {
            metadata: _,
            quasis,
            expressions,
        } = inner;
        out!(self, "`");
        let mut it_expr = expressions.iter();
        let mut buf = [0u8; 4];
        for quasi in quasis.iter() {
            let Node::TemplateElement(TemplateElement {
                metadata: _,
                tail: _,
                cooked: _,
                raw,
            }) = quasi
            else {
                // `quasis` holds only `TemplateElement`s by construction
                // (`ESTree.def`'s `TemplateLiteral`, `NodeList quasis`); a
                // non-`TemplateElement` here is a malformed input tree.
                return Err(GenJsError::UnsupportedKind(quasi.kind()));
            };
            let raw_str = ctx
                .try_bytes_str(raw.get())
                .ok_or(GenJsError::UnrepresentableIdentifier)?;
            for c in raw_str.chars() {
                if c == '\n' {
                    self.force_newline_without_indent();
                    continue;
                }
                self.write_char(c, &mut buf);
            }
            if let Some(expr) = it_expr.next() {
                out!(self, "${{");
                self.gen_node(ctx, expr, Some(Path::new(node, NodeField::expressions)))?;
                out!(self, "}}");
            }
        }
        out!(self, "`");
        Ok(())
    }

    /// `TaggedTemplateExpression`: `tag\`quasi\``.
    ///
    /// juno `gen_js.rs:1422-1436`.
    pub(crate) fn gen_tagged_template_expression<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TaggedTemplateExpression<'gc>,
    ) -> Result<(), GenJsError> {
        let TaggedTemplateExpression {
            metadata: _,
            tag,
            quasi,
        } = inner;
        self.print_child(
            ctx,
            Some(tag),
            Path::new(node, NodeField::tag),
            ChildPos::Left,
        )?;
        self.print_child(
            ctx,
            Some(quasi),
            Path::new(node, NodeField::quasi),
            ChildPos::Right,
        )?;
        Ok(())
    }

    // `TemplateElement` (juno `gen_js.rs:1437-1439`,
    // `unreachable!("TemplateElement is handled in TemplateLiteral case")`)
    // has no arm here: it is only ever printed inline by
    // `gen_template_literal` above, iterating `quasis` directly rather than
    // dispatching through `gen_node`. `dispatch::GenJS::gen_node`'s own
    // `Node::TemplateElement(_)` arm reports `GenJsError::UnsupportedKind`
    // rather than juno's `unreachable!()` panic, per spec §4's "never panic
    // on a malformed input tree" rule — a `TemplateElement` reached directly
    // through `gen_node` (never done by any arm in this crate) is exactly
    // such a malformed tree.

    /// Print `value`'s UTF-16 code units, backslash-escaped for a string
    /// literal delimited by `esc`.
    ///
    /// juno `gen_js.rs:3300-3351`: `ctx.str_u16(value)`. We have no
    /// `str_u16`; see the module doc comment's "String literals" section for
    /// why [`convert_utf8_with_surrogates_to_utf16`] on the atom's raw bytes
    /// is the right replacement (a lone surrogate becomes one `\udXXX`
    /// escape, not three U+FFFD). The escape set (`\\`, `\b`, `\f`, `\n`,
    /// `\r`, `\t`, `\v`, the active quote) and the "printable is
    /// `0x20..=0x7f`, everything else is `\u{:04x}`" rule carry over
    /// unchanged.
    pub(crate) fn print_escaped_string_literal(
        &mut self,
        ctx: &GCLock<'_, '_>,
        value: NodeString,
        esc: char,
    ) {
        let units = hermes_support::utf8::convert_utf8_with_surrogates_to_utf16(ctx.bytes(value));
        for c in units {
            if c <= u8::MAX as u16 {
                match char::from(c as u8) {
                    '\\' => {
                        out!(self, "\\\\");
                        continue;
                    }
                    '\x08' => {
                        out!(self, "\\b");
                        continue;
                    }
                    '\x0c' => {
                        out!(self, "\\f");
                        continue;
                    }
                    '\n' => {
                        out!(self, "\\n");
                        continue;
                    }
                    '\r' => {
                        out!(self, "\\r");
                        continue;
                    }
                    '\t' => {
                        out!(self, "\\t");
                        continue;
                    }
                    '\x0b' => {
                        out!(self, "\\v");
                        continue;
                    }
                    _ => {}
                };
            }
            if c == esc as u16 {
                out!(self, "\\");
            }
            if (0x20..=0x7f).contains(&c) {
                // Printable.
                out!(self, "{}", char::from(c as u8));
            } else {
                out!(self, "\\u{:04x}", c);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use hermes_ast::node::{Program, VariableDeclaration, VariableDeclarator};
    use hermes_parser::{parse, ParseFlags};

    use super::*;
    use crate::Opt;

    /// Parse `src` (expected to be a single `var <id> = <init>;`
    /// declaration) and hand the declarator's `id` and `init` nodes, plus
    /// the locked `GCLock`, to `f`.
    ///
    /// Routing through a full `generate()` call (as the plan's Task 4 brief
    /// sketches these two tests) isn't possible yet: that would require
    /// `VariableDeclaration` and `ExpressionStatement` to already have
    /// dispatch arms, and those are Task 6's job (see the module doc
    /// comment on the free-standing `tests` note in `roundtrip.rs`). Parsing
    /// still works today — parsing needs no printing support at all — so
    /// this extracts the specific leaf node each test cares about and calls
    /// `gen_node` on just that node, the same workaround `precedence.rs`'s
    /// own `with_expr` test helper uses for the same reason.
    fn with_declarator<R>(
        src: &str,
        f: impl for<'gc> FnOnce(&'gc GCLock<'static, '_>, &'gc Node<'gc>, &'gc Node<'gc>) -> R,
    ) -> R {
        let mut parsed = parse(src, ParseFlags::default()).expect("test source must parse");
        parsed.with_program(|gc, node| {
            let Node::Program(Program {
                metadata: _,
                body,
                scope: _,
                sem_info: _,
                strictness: _,
                is_method_definition: _,
                decorations: _,
                dummy_param_list: _,
            }) = node
            else {
                panic!("root is not a Program");
            };
            let stmt = body.iter().next().expect("source has a statement");
            let Node::VariableDeclaration(VariableDeclaration {
                metadata: _,
                kind: _,
                declarations,
            }) = stmt
            else {
                panic!("statement is not a VariableDeclaration: {stmt:?}");
            };
            let decl = declarations.iter().next().expect("has a declarator");
            let Node::VariableDeclarator(VariableDeclarator {
                metadata: _,
                init,
                id,
            }) = decl
            else {
                panic!("declaration is not a VariableDeclarator: {decl:?}");
            };
            let init = init.expect("declarator has an initializer");
            f(gc, id, init)
        })
    }

    /// Generate just `node` (not a whole program) and decode the result as a
    /// `String`, the same "always valid UTF-8" guarantee `generate()` itself
    /// gives (spec §5).
    fn gen_node_to_string<'gc>(gc: &GCLock<'static, '_>, node: &'gc Node<'gc>) -> String {
        let mut sink = Vec::new();
        {
            let mut gen_js = GenJS::for_test(&mut sink, Opt::new());
            gen_js.gen_node(gc, node, None).expect("node generates");
        }
        String::from_utf8(sink).expect("generator output is always valid UTF-8 (spec §5)")
    }

    // -----------------------------------------------------------------
    // The two required encoding tests (spec §5 / §8's review focus area 3):
    // the port's WTF-8 atom rules bite hardest right here, in `Identifier`
    // and `print_escaped_string_literal`. Both are named so a regression
    // that swaps `try_bytes_str` for `bytes()`/`bytes_str_lossy` (or breaks
    // the UTF-16 code-unit walk) fails a specific, identifiable test rather
    // than an assertion buried in a bigger corpus run later.
    // -----------------------------------------------------------------

    /// An astral identifier is legal JS and our atoms hold it as a WTF-8
    /// surrogate PAIR, so emitting raw atom bytes would produce invalid
    /// UTF-8.
    #[test]
    fn astral_identifier_round_trips_as_valid_utf8() {
        with_declarator("var \u{1D465} = 1;", |gc, id, _init| {
            assert!(matches!(id, Node::Identifier(_)));
            let js = gen_node_to_string(gc, id);
            assert!(js.contains('\u{1D465}'), "{js}");
            assert!(std::str::from_utf8(js.as_bytes()).is_ok());
        });
    }

    /// A lone surrogate is a legal JS string value with no literal spelling;
    /// it must survive as exactly one `\udXXX` escape, not three U+FFFD.
    #[test]
    fn lone_surrogate_string_literal_survives_as_one_escape() {
        with_declarator(r#"var s = "\uD800";"#, |gc, _id, init| {
            assert!(matches!(init, Node::StringLiteral(_)));
            let js = gen_node_to_string(gc, init);
            assert!(js.contains("\\ud800"), "{js}");
            assert_eq!(js.matches('\u{FFFD}').count(), 0, "{js}");
        });
    }

    /// [`GenJS::gen_bigint_literal`]'s doc comment traces a real juno bug: it
    /// drops the `n` suffix, turning `123n` into `123` on regeneration — a
    /// `NumericLiteral` spelling that reparses as a `Number`, not a
    /// `BigInt`. This is the named test that fails without the fix
    /// (`prove-checks-can-fail`).
    #[test]
    fn bigint_literal_round_trips_with_n_suffix() {
        with_declarator("var b = 123n;", |gc, _id, init| {
            assert!(matches!(init, Node::BigIntLiteral(_)));
            let js = gen_node_to_string(gc, init);
            assert_eq!(js, "123n");
        });
    }

    /// juno leaves `DirectiveLiteral` an `unimplemented!()` stub
    /// (`gen_js.rs:1263-1265`); [`GenJS::gen_directive_literal`]'s doc
    /// comment explains why it's implemented here instead, reusing the same
    /// escaping path as `StringLiteral`. There is no source construct that
    /// parses straight to a bare `DirectiveLiteral` outside a directive
    /// prologue's `Directive` wrapper, so this builds one directly with
    /// `GCLock::alloc`, mirroring `precedence.rs`'s
    /// `unknown_binary_operator_spelling_is_a_gen_js_error_not_a_panic` test.
    #[test]
    fn directive_literal_escapes_like_string_literal() {
        with_declarator(r#"var s = "unused";"#, |gc, _id, init| {
            let Node::StringLiteral(StringLiteral { metadata, value: _ }) = init else {
                panic!("expected a StringLiteral: {init:?}");
            };
            let value = gc.atom_bytes(&b"a\\b"[..]);
            let hand_built = gc.alloc(Node::DirectiveLiteral(DirectiveLiteral::new(
                hermes_ast::node_child::NodeMetadata::new(metadata.range()),
                value,
            )));
            let js = gen_node_to_string(gc, hand_built);
            assert_eq!(js, "'a\\\\b'");
        });
    }
}
