/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Round-trip tests: parse source, generate it back, check the result.
//!
//! This file is created by Task 2 of
//! `doc/superpowers/plans/2026-08-15-gen-js-port.md` with a single smoke
//! test exercising the core machinery (`generate`, `Opt`, `GenJS`'s output
//! primitives) end to end. Task 6 (`arms/stmt.rs`) is the first to grow it
//! with real statement-shaped cases, now that `Program`/`ExpressionStatement`/
//! `VariableDeclaration`/`IfStatement`/etc. have dispatch arms: the ASI and
//! dangling-else hazard cases its brief's Step 2 requires, the two Task 4
//! encoding fixtures re-run through the full pipeline (Task 6's Obligation
//! 3), and a directive-fidelity case (Obligation 2). Later tasks grow this
//! further with juno's remaining ported unit cases and the Tier 1 corpus
//! gate (see the plan's File Structure section).

use hermes_ast::context::GCLock;
use hermes_ast::node::{
    ArrayTypeAnnotation, ArrowFunctionExpression, AssignmentExpression, BigIntLiteralTypeAnnotation,
    BinaryExpression, BlockStatement, ClassBody, ClassDeclaration,
    DeclareClass, DeclareExportDeclaration, DeclareModule,
    DoWhileStatement, EnumBooleanBody, EnumDeclaration,
    EnumNumberBody, EnumStringBody, EnumSymbolBody,
    ExportDefaultDeclaration, ExportNamedDeclaration, ExportNamespaceSpecifier,
    ExpressionStatement, ForStatement, FunctionDeclaration, FunctionTypeAnnotation,
    FunctionTypeParam, GenericTypeAnnotation, Identifier, IfStatement, ImportAttribute,
    ImportDeclaration, ImportSpecifier, IndexedAccessType, InterfaceDeclaration,
    IntersectionTypeAnnotation, JSXAttribute, JSXElement, JSXExpressionContainer, JSXFragment,
    JSXMemberExpression, JSXNamespacedName, JSXOpeningElement, JSXSpreadAttribute,
    JSXStringLiteral, JSXText, LabeledStatement, LogicalExpression, MemberExpression,
    MethodDefinition, Node, NullableTypeAnnotation, ObjectTypeAnnotation,
    ObjectTypeProperty,
    OpaqueType, OptionalIndexedAccessType, Program, QualifiedTypeIdentifier, TupleTypeAnnotation,
    TypeAnnotation, TypeParameter, TypeParameterDeclaration, TypeParameterInstantiation,
    TypeofTypeAnnotation, UnionTypeAnnotation, Variance, VariableDeclaration, VariableDeclarator,
};
use hermes_gen_js::{generate, Opt, Pretty};
use hermes_parser::{ParseFlags, ParsedJS};

/// Generates `parsed`'s program with `opt` into a `String`.
///
/// `ParsedJS` exposes the AST only through
/// [`ParsedJS::with_program`](hermes_parser::ParsedJS::with_program), which
/// hands out a `&GCLock`/`&Node` pair rather than a `&mut Context`/`NodeRc`
/// — see `generate`'s doc comment in `hermes_gen_js` for why that shaped
/// this crate's `generate` the way it is. `out` is captured by the closure
/// rather than returned from it, since `with_program`'s bound is
/// `for<'gc> FnOnce(...) -> R` and a `Vec<u8>` (unlike a `NodeRc`) has no
/// reason to be tied to the arena's lifetime.
fn gen(parsed: &mut ParsedJS, opt: Opt) -> String {
    let mut out = Vec::new();
    parsed.with_program(|gc, root| {
        generate(&mut out, gc, root, opt).expect("generation succeeds");
    });
    String::from_utf8(out).expect("generator output is always valid UTF-8 (spec §5)")
}

/// juno unconditionally appends a trailing newline in `gen_root`
/// (`gen_js.rs:299`), even when nothing else was printed, so an empty
/// program's output is a single `"\n"`, not `""`.
#[test]
fn empty_program_generates_trailing_newline_only() {
    let mut parsed = hermes_parser::parse("", Default::default()).expect("parses");
    let js = gen(&mut parsed, Opt::default());
    assert_eq!(js, "\n");
}

/// Parse `src`, panicking with a message that includes `src` on failure.
fn parse_ok(src: &str) -> ParsedJS {
    hermes_parser::parse(src, Default::default())
        .unwrap_or_else(|e| panic!("{src:?} must parse: {e:?}"))
}

/// Run `check(src_under_test)` under both [`Pretty::Yes`] and
/// [`Pretty::No`] — the plan's Task 6 brief, Step 2, requires every ASI/
/// dangling-else case to be checked in both, since `Pretty::No` is where
/// ASI hazards actually bite (no whitespace to accidentally separate two
/// tokens that would otherwise merge or misparse).
fn for_each_pretty_mode(mut check: impl FnMut(Pretty)) {
    check(Pretty::Yes);
    check(Pretty::No);
}

/// Generate `src` under `pretty`, then parse the result back. Panics
/// (naming `pretty` and the generated text) if the regenerated source
/// fails to parse at all — the minimum bar every case in this file must
/// clear.
fn round_trip(src: &str, pretty: Pretty) -> ParsedJS {
    let mut parsed = parse_ok(src);
    let js = gen(
        &mut parsed,
        Opt {
            pretty,
            ..Opt::default()
        },
    );
    hermes_parser::parse(&js, Default::default()).unwrap_or_else(|e| {
        panic!("regenerated source {js:?} (from {src:?}, {pretty:?}) must parse: {e:?}")
    })
}

/// Hands `parsed`'s `Program`'s first top-level statement to `f`, along
/// with the locked `GCLock`.
fn with_first_stmt<R>(
    parsed: &mut ParsedJS,
    f: impl for<'gc> FnOnce(&'gc GCLock<'static, '_>, &'gc Node<'gc>) -> R,
) -> R {
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
        let stmt = body.iter().next().expect("program has a statement");
        f(gc, stmt)
    })
}

// ---------------------------------------------------------------------------
// Obligation 1: the `gen_root` empty-`Program` shortcut is gone. Every case
// in the brief is a single top-level statement and would pass even with the
// shortcut still in place (see `arms/stmt.rs`'s module doc comment); this is
// the one test in this file with more than one.
// ---------------------------------------------------------------------------

/// `gen_root`'s Task 2 shortcut called raw `gen_node` per top-level
/// statement instead of `visit_stmt_in_block`, so a multi-statement program
/// would have silently lost every semicolon and inter-statement newline —
/// invisible under every single-statement test any earlier task wrote.
#[test]
fn three_statements_get_semicolons_and_separation() {
    for_each_pretty_mode(|pretty| {
        let mut parsed = parse_ok("a();b();c();");
        let js = gen(
            &mut parsed,
            Opt {
                pretty,
                ..Opt::default()
            },
        );
        // Exactly 3 statement-terminating `;`, regardless of pretty mode.
        assert_eq!(js.matches(';').count(), 3, "{pretty:?}: {js:?}");
        match pretty {
            Pretty::Yes => {
                // Pretty mode separates statements with a newline (juno
                // `gen_js.rs:3556-3558`'s `visit_stmt_list`, ported as
                // `GenJS::visit_stmt_list`).
                assert_eq!(js.matches('\n').count(), 3, "{pretty:?}: {js:?}");
                assert!(js.contains("a();\nb();\nc();"), "{js:?}");
            }
            Pretty::No => {
                assert_eq!(js, "a();b();c();\n", "{js:?}");
            }
        }
        // And it must still round-trip: 3 statements in, 3 statements out.
        let mut reparsed =
            hermes_parser::parse(&js, Default::default()).expect("regenerated text parses");
        reparsed.with_program(|_gc, node| {
            let Node::Program(Program { body, .. }) = node else {
                panic!("root is not a Program");
            };
            assert_eq!(body.iter().count(), 3, "{pretty:?}: {js:?}");
        });
    });
}

// ---------------------------------------------------------------------------
// Obligation 2: `ExpressionStatement.directive` fidelity.
// ---------------------------------------------------------------------------

/// A string literal spelled with an escape that cooks to exactly `"use
/// strict"` is *not* a use-strict directive (ECMA-262's rule keys on the
/// raw source spelling, not the string's semantic value) — reprinting it
/// from the cooked value alone would flip strictness on reparse. This is
/// the exact scenario `GenJS::gen_expression_statement`'s doc comment
/// walks through.
///
/// The second statement is a legacy octal literal (`0123`), legal only
/// outside strict mode: it is the decisive, purely-syntactic witness that
/// strict mode did *not* get silently switched on by reprinting — no need
/// to reach into `directive`'s own reparsed value, which (confirmed while
/// writing this test) is populated for *any* leading string-literal
/// statement, escaped or not, and so cannot by itself distinguish "is a
/// directive" from "is specifically `use strict`".
///
/// This is also this task's "prove a check can fail" case (per
/// `prove-checks-can-fail`): temporarily reverting
/// `gen_expression_statement` to juno's `directive: _`/bare
/// `print_child(expression)` behavior — confirmed by hand during this
/// task, reverting is not something a passing test suite can demonstrate
/// about itself — makes this test fail at its first assertion:
/// `assertion failed: !js.contains("'use strict'")`, panic message
/// `Yes: "'use strict';\n83;\n"` (`0123` prints back as decimal `83`,
/// unrelated to this bug — plain `NumericLiteral` printing, already
/// correct; the point is the escape is gone and `'use strict'` is now
/// literal).
#[test]
fn escaped_use_strict_directive_does_not_become_a_real_directive() {
    for_each_pretty_mode(|pretty| {
        let mut parsed = parse_ok("\"use\\u0020strict\";\n0123;");
        let js = gen(
            &mut parsed,
            Opt {
                pretty,
                ..Opt::default()
            },
        );
        // The escape must survive verbatim: this must NOT print as the bare
        // (and therefore genuinely directive-triggering) `'use strict'`.
        assert!(!js.contains("'use strict'"), "{pretty:?}: {js:?}");
        assert!(!js.contains("\"use strict\""), "{pretty:?}: {js:?}");
        assert!(js.contains("use\\u0020strict"), "{pretty:?}: {js:?}");

        // And the fix must not be a no-op that merely mangles output: the
        // regenerated source must still parse — which it only does if
        // strict mode was *not* switched on, since `0123` is a syntax
        // error in strict mode.
        let _ = hermes_parser::parse(&js, Default::default())
            .unwrap_or_else(|e| panic!("{pretty:?}: regenerated {js:?} must parse: {e:?}"));
    });
}

/// A genuine `"use strict";` directive (no escapes) must still round-trip
/// as one — the positive counterpart to the test above.
#[test]
fn genuine_use_strict_directive_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut parsed = parse_ok("\"use strict\";\n42;");
        let js = gen(
            &mut parsed,
            Opt {
                pretty,
                ..Opt::default()
            },
        );
        assert!(js.contains("use strict"), "{pretty:?}: {js:?}");
        let mut reparsed = hermes_parser::parse(&js, Default::default())
            .unwrap_or_else(|e| panic!("{pretty:?}: regenerated {js:?} must parse: {e:?}"));
        reparsed.with_program(|gc, node| {
            let Node::Program(Program { body, .. }) = node else {
                panic!("root is not a Program");
            };
            let Node::ExpressionStatement(ExpressionStatement { directive, .. }) =
                body.iter().next().expect("has a first statement")
            else {
                panic!("first statement is not an ExpressionStatement");
            };
            assert_eq!(
                gc.try_bytes_str(directive.get()),
                Some("use strict"),
                "{pretty:?}: {js:?}"
            );
        });
    });
}

// ---------------------------------------------------------------------------
// Obligation 3: Task 4's two encoding fixtures, now through the full
// pipeline (they previously called `gen_node` on an extracted leaf, since
// `VariableDeclaration`/`ExpressionStatement` had no arm yet).
// ---------------------------------------------------------------------------

/// An astral identifier is legal JS and our atoms hold it as a WTF-8
/// surrogate PAIR; emitting raw atom bytes would produce invalid UTF-8.
/// `arms/literal.rs`'s `astral_identifier_round_trips_as_valid_utf8` covers
/// the same encoding path at the single-node level; this is the full-`generate()`
/// version the plan's Task 6 brief's Obligation 3 asks for.
#[test]
fn astral_identifier_full_pipeline_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut parsed = parse_ok("var \u{1D465} = 1;");
        let js = gen(
            &mut parsed,
            Opt {
                pretty,
                ..Opt::default()
            },
        );
        assert!(js.contains('\u{1D465}'), "{pretty:?}: {js:?}");
        assert!(std::str::from_utf8(js.as_bytes()).is_ok());
        let mut reparsed = hermes_parser::parse(&js, Default::default())
            .unwrap_or_else(|e| panic!("{pretty:?}: regenerated {js:?} must parse: {e:?}"));
        reparsed.with_program(|gc, node| {
            let Node::Program(Program { body, .. }) = node else {
                panic!("root is not a Program");
            };
            let Node::VariableDeclaration(VariableDeclaration { declarations, .. }) =
                body.iter().next().expect("has a first statement")
            else {
                panic!("first statement is not a VariableDeclaration");
            };
            let Node::VariableDeclarator(VariableDeclarator { id, .. }) =
                declarations.iter().next().expect("has a declarator")
            else {
                panic!("declaration is not a VariableDeclarator");
            };
            let Node::Identifier(Identifier { name, .. }) = id else {
                panic!("id is not an Identifier");
            };
            assert_eq!(
                gc.try_bytes_str(name.get()),
                Some("\u{1D465}"),
                "{pretty:?}: {js:?}"
            );
        });
    });
}

/// A lone surrogate is a legal JS string value with no literal spelling; it
/// must survive as exactly one `\udXXX` escape, not three U+FFFD. Full-
/// pipeline counterpart to `arms/literal.rs`'s
/// `lone_surrogate_string_literal_survives_as_one_escape`.
#[test]
fn lone_surrogate_string_literal_full_pipeline_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut parsed = parse_ok(r#"var s = "\uD800";"#);
        let js = gen(
            &mut parsed,
            Opt {
                pretty,
                ..Opt::default()
            },
        );
        assert!(js.contains("\\ud800"), "{pretty:?}: {js:?}");
        assert_eq!(js.matches('\u{FFFD}').count(), 0, "{pretty:?}: {js:?}");
        // Must still parse: a lone surrogate escape is legal JS.
        let _ = hermes_parser::parse(&js, Default::default())
            .unwrap_or_else(|e| panic!("{pretty:?}: regenerated {js:?} must parse: {e:?}"));
    });
}

// ---------------------------------------------------------------------------
// Step 2: the ASI and dangling-else hazard tests the brief requires.
// ---------------------------------------------------------------------------

/// `if (a) { if (b) c(); } else d();` — the `else` belongs to the *outer*
/// `if` (explicit braces put the inner `if` in its own block). Printed
/// without re-adding a block around the outer `if`'s consequent, the
/// `else` would silently move to the inner `if` on reparse (the classic
/// dangling-else hazard) — `GenJS::gen_if_statement`'s `is_if_without_else`
/// check exists precisely to prevent that.
#[test]
fn dangling_else_binds_to_outer_if() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip("if (a) { if (b) c(); } else d();", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::IfStatement(IfStatement {
                consequent,
                alternate,
                ..
            }) = stmt
            else {
                panic!("{pretty:?}: statement is not an IfStatement");
            };
            assert!(
                alternate.is_some(),
                "{pretty:?}: else must still belong to the outer if"
            );
            // The consequent must still be (or contain, if wrapped in a
            // block) an IfStatement with no alternate of its own.
            let inner_has_no_else = match consequent {
                Node::IfStatement(IfStatement { alternate, .. }) => alternate.is_none(),
                Node::BlockStatement(BlockStatement { body, .. }) => {
                    matches!(
                        body.iter().next(),
                        Some(Node::IfStatement(IfStatement {
                            alternate: None,
                            ..
                        }))
                    )
                }
                other => panic!("{pretty:?}: unexpected consequent shape: {other:?}"),
            };
            assert!(
                inner_has_no_else,
                "{pretty:?}: inner if must still have no else of its own"
            );
        });
    });
}

/// `for (var i = (a in b);;);` — the parenthesized `a in b` is only legal
/// syntax at all because of the literal parens (ECMA-262 14.7.4's `[~In]`
/// restriction on a `for` head's `VariableDeclarationList`); the AST keeps
/// no record of them, so the generator must decide to re-add them from
/// shape alone. See `precedence.rs`'s `VariableDeclarator` branch of
/// `need_parens`.
#[test]
fn for_init_declarator_in_expression_gets_parens() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip("for (var i = (a in b);;);", pretty);
        with_first_stmt(&mut reparsed, |gc, stmt| {
            let Node::ForStatement(ForStatement { init, .. }) = stmt else {
                panic!("{pretty:?}: statement is not a ForStatement");
            };
            let Some(Node::VariableDeclaration(VariableDeclaration { declarations, .. })) = init
            else {
                panic!("{pretty:?}: init is not a VariableDeclaration: {init:?}");
            };
            let Node::VariableDeclarator(VariableDeclarator { init, .. }) =
                declarations.iter().next().expect("has a declarator")
            else {
                panic!("{pretty:?}: declaration is not a VariableDeclarator");
            };
            let Some(Node::BinaryExpression(BinaryExpression { operator, .. })) = init else {
                panic!("{pretty:?}: declarator init is not a BinaryExpression: {init:?}");
            };
            assert_eq!(
                gc.bytes_str_lossy(operator.get()),
                "in",
                "{pretty:?}: must still be the `in` operator, not reinterpreted"
            );
        });
    });
}

/// `for (var i = (a && b in c);;);` — a *nested* bare `in`, as the right
/// operand of `&&` rather than the whole declarator init. This is the case
/// `precedence.rs`'s `contains_bare_in` was added to fix: a direct
/// `is_binary_op(child, In)` check (this crate's first version of the
/// `VariableDeclarator` branch) sees `child` as a `LogicalExpression`, not
/// a `BinaryExpression`, and adds no parens at all — the regenerated
/// source (`for(var i = a && b in c;;)`) then fails to reparse with
/// `')' expected after 'for(... in/of ...'`, a live round-trip break, not
/// just redundant-but-valid output.
#[test]
fn for_init_declarator_nested_in_expression_gets_parens() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip("for (var i = (a && b in c);;);", pretty);
        with_first_stmt(&mut reparsed, |gc, stmt| {
            let Node::ForStatement(ForStatement { init, .. }) = stmt else {
                panic!("{pretty:?}: statement is not a ForStatement");
            };
            let Some(Node::VariableDeclaration(VariableDeclaration { declarations, .. })) = init
            else {
                panic!("{pretty:?}: init is not a VariableDeclaration: {init:?}");
            };
            let Node::VariableDeclarator(VariableDeclarator { init, .. }) =
                declarations.iter().next().expect("has a declarator")
            else {
                panic!("{pretty:?}: declaration is not a VariableDeclarator");
            };
            let Some(Node::LogicalExpression(LogicalExpression {
                operator, right, ..
            })) = init
            else {
                panic!("{pretty:?}: declarator init is not a LogicalExpression: {init:?}");
            };
            assert_eq!(gc.bytes_str_lossy(operator.get()), "&&", "{pretty:?}");
            let Node::BinaryExpression(BinaryExpression { operator, .. }) = right else {
                panic!("{pretty:?}: right operand is not a BinaryExpression: {right:?}");
            };
            assert_eq!(
                gc.bytes_str_lossy(operator.get()),
                "in",
                "{pretty:?}: must still be the `in` operator, not reinterpreted"
            );
        });
    });
}

/// Deeper nesting than the case above: `x || (y && (a in b))` — the bare
/// `in` is now the right operand of `&&`, which is itself the right
/// operand of `||`, two levels down from the declarator's init.
/// `contains_bare_in` is a full-subtree walk specifically so depth like
/// this doesn't matter.
#[test]
fn for_init_declarator_doubly_nested_in_expression_gets_parens() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip("for (var i = (x || (y && (a in b)));;);", pretty);
        with_first_stmt(&mut reparsed, |gc, stmt| {
            let Node::ForStatement(ForStatement { init, .. }) = stmt else {
                panic!("{pretty:?}: statement is not a ForStatement");
            };
            let Some(Node::VariableDeclaration(VariableDeclaration { declarations, .. })) = init
            else {
                panic!("{pretty:?}: init is not a VariableDeclaration: {init:?}");
            };
            let Node::VariableDeclarator(VariableDeclarator { init, .. }) =
                declarations.iter().next().expect("has a declarator")
            else {
                panic!("{pretty:?}: declaration is not a VariableDeclarator");
            };
            let Some(Node::LogicalExpression(LogicalExpression {
                operator: outer_op,
                right: outer_right,
                ..
            })) = init
            else {
                panic!("{pretty:?}: declarator init is not a LogicalExpression: {init:?}");
            };
            assert_eq!(gc.bytes_str_lossy(outer_op.get()), "||", "{pretty:?}");
            let Node::LogicalExpression(LogicalExpression {
                operator: inner_op,
                right: inner_right,
                ..
            }) = outer_right
            else {
                panic!("{pretty:?}: outer right is not a LogicalExpression: {outer_right:?}");
            };
            assert_eq!(gc.bytes_str_lossy(inner_op.get()), "&&", "{pretty:?}");
            let Node::BinaryExpression(BinaryExpression { operator, .. }) = inner_right else {
                panic!("{pretty:?}: inner right is not a BinaryExpression: {inner_right:?}");
            };
            assert_eq!(
                gc.bytes_str_lossy(operator.get()),
                "in",
                "{pretty:?}: must still be the `in` operator, not reinterpreted"
            );
        });
    });
}

/// The same nested-`in` hazard, but as a *bare* `for` init (no `var`) —
/// exercises `precedence.rs`'s sibling `ForStatement` branch fix
/// (`contains_bare_in` replaces that branch's own old direct-child-only
/// check too, not just `VariableDeclarator`'s).
#[test]
fn for_init_bare_nested_in_expression_gets_parens() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip("for ((a && b in c);;);", pretty);
        with_first_stmt(&mut reparsed, |gc, stmt| {
            let Node::ForStatement(ForStatement { init, .. }) = stmt else {
                panic!("{pretty:?}: statement is not a ForStatement");
            };
            let Some(Node::LogicalExpression(LogicalExpression {
                operator, right, ..
            })) = init
            else {
                panic!("{pretty:?}: init is not a LogicalExpression: {init:?}");
            };
            assert_eq!(gc.bytes_str_lossy(operator.get()), "&&", "{pretty:?}");
            let Node::BinaryExpression(BinaryExpression { operator, .. }) = right else {
                panic!("{pretty:?}: right operand is not a BinaryExpression: {right:?}");
            };
            assert_eq!(
                gc.bytes_str_lossy(operator.get()),
                "in",
                "{pretty:?}: must still be the `in` operator, not reinterpreted"
            );
        });
    });
}

/// `do x(); while (y)` — no trailing `;` (EOF-driven ASI). The generator
/// always emits one (`DoWhileStatement` is not in `stmt_skip_semi`'s skip
/// list — juno agrees, `gen_js.rs:4093-4149`), which is always safe; this
/// checks the body/`while` clause round-trips regardless of the missing
/// source semicolon.
#[test]
fn do_while_without_trailing_semicolon_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip("do x(); while (y)", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::DoWhileStatement(DoWhileStatement { test, body, .. }) = stmt else {
                panic!("{pretty:?}: statement is not a DoWhileStatement");
            };
            assert!(matches!(test, Node::Identifier(_)), "{pretty:?}");
            assert!(matches!(body, Node::ExpressionStatement(_)), "{pretty:?}");
        });
    });
}

/// A labeled statement wrapping a block: `foo: { x(); y(); }`.
#[test]
fn labeled_statement_wrapping_block_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip("foo: { x(); y(); }", pretty);
        with_first_stmt(&mut reparsed, |gc, stmt| {
            let Node::LabeledStatement(LabeledStatement { label, body, .. }) = stmt else {
                panic!("{pretty:?}: statement is not a LabeledStatement");
            };
            let Node::Identifier(Identifier { name, .. }) = label else {
                panic!("{pretty:?}: label is not an Identifier");
            };
            assert_eq!(gc.try_bytes_str(name.get()), Some("foo"), "{pretty:?}");
            let Node::BlockStatement(BlockStatement { body, .. }) = body else {
                panic!("{pretty:?}: labeled body is not a BlockStatement");
            };
            assert_eq!(body.iter().count(), 2, "{pretty:?}");
        });
    });
}

/// An expression statement beginning with `(` (a bare `SequenceExpression`,
/// which needs no protection at all — no grammar production confuses a
/// leading `(...)`  with anything else).
#[test]
fn expression_statement_starting_with_paren_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip("(a, b);", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::ExpressionStatement(ExpressionStatement { expression, .. }) = stmt else {
                panic!("{pretty:?}: statement is not an ExpressionStatement");
            };
            assert!(
                matches!(expression, Node::SequenceExpression(_)),
                "{pretty:?}"
            );
        });
    });
}

/// An expression statement beginning with `[` (a bare `ArrayExpression`,
/// also unambiguous — unlike `{`, `[` never opens a block).
#[test]
fn expression_statement_starting_with_bracket_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip("[1, 2, 3];", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::ExpressionStatement(ExpressionStatement { expression, .. }) = stmt else {
                panic!("{pretty:?}: statement is not an ExpressionStatement");
            };
            assert!(matches!(expression, Node::ArrayExpression(_)), "{pretty:?}");
        });
    });
}

/// An expression statement beginning with `let[` — a *sloppy*-mode `let`
/// used as an ordinary variable name, immediately indexed. The source
/// under test writes it as `(let)[0] = 1;`: bare `let[0] = 1;` doesn't
/// even parse (our parser's own `is_let_followed_by_decl_start` lookahead
/// commits to a `LexicalDeclaration` the moment it sees `let[` at
/// statement start, then fails — `0` is not a valid `ArrayBindingPattern`
/// element — confirming the hazard is real at the source level, not just a
/// generator nicety); the explicit parens sidestep that lookahead (it only
/// fires when the statement *starts* with the bare `let` token) to build
/// the `MemberExpression{object: Identifier("let"), computed: true, ...}`
/// AST this test actually wants, the same shape `let[0] = 1;` would
/// produce if it could parse. `precedence.rs`'s `starts_with_let_bracket`
/// is what has to reintroduce protection when *this* AST — parens gone,
/// since Hermes doesn't retain them as a node — gets printed back out.
#[test]
fn expression_statement_starting_with_let_bracket_gets_parens() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip("(let)[0] = 1;", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::ExpressionStatement(ExpressionStatement { expression, .. }) = stmt else {
                panic!("{pretty:?}: statement is not an ExpressionStatement");
            };
            let Node::AssignmentExpression(AssignmentExpression { left, .. }) = expression else {
                panic!("{pretty:?}: expression is not an AssignmentExpression: {expression:?}");
            };
            let Node::MemberExpression(MemberExpression {
                object, computed, ..
            }) = left
            else {
                panic!("{pretty:?}: assignment target is not a MemberExpression: {left:?}");
            };
            assert!(computed.get(), "{pretty:?}: must still be computed (`[0]`)");
            assert!(
                matches!(object, Node::Identifier(_)),
                "{pretty:?}: object must still be a plain identifier `let`, not reinterpreted \
                 as the start of a declaration"
            );
        });
    });
}

// ---------------------------------------------------------------------------
// Task 7's brief, Step 2: functions, arrows, classes, methods, properties.
// ---------------------------------------------------------------------------

/// `async x => x` — the arrow single-parameter shortcut (sole param is a
/// bare `Identifier`, no type annotation, not optional) omits the
/// parenthesized parameter list.
#[test]
fn async_arrow_single_ident_param_round_trips_without_parens() {
    for_each_pretty_mode(|pretty| {
        let mut parsed = parse_ok("async x => x;");
        let js = gen(
            &mut parsed,
            Opt {
                pretty,
                ..Opt::default()
            },
        );
        assert!(js.contains("async x"), "{pretty:?}: {js:?}");
        assert!(!js.contains('('), "{pretty:?}: no parens expected: {js:?}");
        let mut reparsed = hermes_parser::parse(&js, Default::default())
            .unwrap_or_else(|e| panic!("{pretty:?}: regenerated {js:?} must parse: {e:?}"));
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::ExpressionStatement(ExpressionStatement { expression, .. }) = stmt else {
                panic!("{pretty:?}: statement is not an ExpressionStatement");
            };
            let Node::ArrowFunctionExpression(ArrowFunctionExpression {
                r#async, params, ..
            }) = expression
            else {
                panic!("{pretty:?}: expression is not an ArrowFunctionExpression: {expression:?}");
            };
            assert!(r#async.get(), "{pretty:?}");
            assert_eq!(params.iter().count(), 1, "{pretty:?}");
        });
    });
}

/// `async (x) => x` — written with a redundant parenthesized single
/// parameter. The AST retains no memory of the original parens (a
/// non-typed, non-optional single `Identifier` param is indistinguishable
/// from the `async x => x` spelling once parsed), so this must round-trip
/// to the exact same shape as the case above, regardless of how it was
/// originally written.
#[test]
fn async_arrow_written_with_redundant_parens_round_trips_to_same_shape() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip("async (x) => x;", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::ExpressionStatement(ExpressionStatement { expression, .. }) = stmt else {
                panic!("{pretty:?}: statement is not an ExpressionStatement");
            };
            let Node::ArrowFunctionExpression(ArrowFunctionExpression {
                r#async, params, ..
            }) = expression
            else {
                panic!("{pretty:?}: expression is not an ArrowFunctionExpression: {expression:?}");
            };
            assert!(r#async.get(), "{pretty:?}");
            assert_eq!(params.iter().count(), 1, "{pretty:?}");
            assert!(
                matches!(params.iter().next(), Some(Node::Identifier(_))),
                "{pretty:?}"
            );
        });
    });
}

/// `force_async_arrow_space` only has an observable effect when the
/// single-parameter shortcut is *not* taken (here: two params, so the
/// parenthesized-param-list branch runs) — the shortcut branch's own
/// `need_sep` bookkeeping already adds the space unconditionally before the
/// lone parameter, so `async x => x` above prints identically regardless of
/// this option (confirmed while writing this test: flipping the option left
/// that case's output unchanged). `async (a, b) => a` is the case that
/// actually distinguishes `true` from `false`.
#[test]
fn force_async_arrow_space_controls_space_before_parenthesized_params() {
    let mut parsed_true = parse_ok("async (a, b) => a;");
    let js_true = gen(
        &mut parsed_true,
        Opt {
            pretty: Pretty::No,
            force_async_arrow_space: true,
            ..Opt::default()
        },
    );
    assert!(js_true.contains("async ("), "{js_true:?}");

    let mut parsed_false = parse_ok("async (a, b) => a;");
    let js_false = gen(
        &mut parsed_false,
        Opt {
            pretty: Pretty::No,
            force_async_arrow_space: false,
            ..Opt::default()
        },
    );
    assert!(js_false.contains("async("), "{js_false:?}");
    assert!(!js_false.contains("async ("), "{js_false:?}");

    // Both spellings must still round-trip to an equivalent async arrow with
    // two parameters — the option only changes bytes, never meaning.
    for js in [&js_true, &js_false] {
        let mut reparsed = hermes_parser::parse(js, Default::default())
            .unwrap_or_else(|e| panic!("regenerated {js:?} must parse: {e:?}"));
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::ExpressionStatement(ExpressionStatement { expression, .. }) = stmt else {
                panic!("statement is not an ExpressionStatement");
            };
            let Node::ArrowFunctionExpression(ArrowFunctionExpression {
                r#async, params, ..
            }) = expression
            else {
                panic!("expression is not an ArrowFunctionExpression: {expression:?}");
            };
            assert!(r#async.get());
            assert_eq!(params.iter().count(), 2);
        });
    }
}

/// `(a) => ({})` — a single-parameter arrow whose body is an object literal.
/// Without the parens, `(a) => {}` would parse the `{}` as an empty block
/// body, not an object literal (`need_parens`'s `ArrowFunctionExpression`
/// branch, `precedence.rs`, ported in Task 3).
#[test]
fn arrow_single_param_object_literal_body_gets_parens() {
    for_each_pretty_mode(|pretty| {
        let mut parsed = parse_ok("(a) => ({});");
        let js = gen(
            &mut parsed,
            Opt {
                pretty,
                ..Opt::default()
            },
        );
        assert!(js.contains("({}"), "{pretty:?}: {js:?}");
        let mut reparsed = hermes_parser::parse(&js, Default::default())
            .unwrap_or_else(|e| panic!("{pretty:?}: regenerated {js:?} must parse: {e:?}"));
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::ExpressionStatement(ExpressionStatement { expression, .. }) = stmt else {
                panic!("{pretty:?}: statement is not an ExpressionStatement");
            };
            let Node::ArrowFunctionExpression(ArrowFunctionExpression { body, .. }) = expression
            else {
                panic!("{pretty:?}: expression is not an ArrowFunctionExpression: {expression:?}");
            };
            assert!(
                matches!(body, Node::ObjectExpression(_)),
                "{pretty:?}: body is not an ObjectExpression, the parens were load-bearing: \
                 {body:?}"
            );
        });
    });
}

/// `class C { [x]() {} }` — a computed method name.
#[test]
fn class_with_computed_method_name_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip("class C { [x]() {} }", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::ClassDeclaration(ClassDeclaration { body, .. }) = stmt else {
                panic!("{pretty:?}: statement is not a ClassDeclaration");
            };
            let Node::ClassBody(ClassBody { body, .. }) = body else {
                panic!("{pretty:?}: class body is not a ClassBody");
            };
            let Node::MethodDefinition(MethodDefinition { computed, .. }) =
                body.iter().next().expect("has a member")
            else {
                panic!("{pretty:?}: member is not a MethodDefinition");
            };
            assert!(computed.get(), "{pretty:?}: computed must survive");
        });
    });
}

/// `class C { get x() { return 1; } }` — a getter.
#[test]
fn class_getter_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip("class C { get x() { return 1; } }", pretty);
        with_first_stmt(&mut reparsed, |gc, stmt| {
            let Node::ClassDeclaration(ClassDeclaration { body, .. }) = stmt else {
                panic!("{pretty:?}: statement is not a ClassDeclaration");
            };
            let Node::ClassBody(ClassBody { body, .. }) = body else {
                panic!("{pretty:?}: class body is not a ClassBody");
            };
            let Node::MethodDefinition(MethodDefinition { kind, .. }) =
                body.iter().next().expect("has a member")
            else {
                panic!("{pretty:?}: member is not a MethodDefinition");
            };
            assert_eq!(gc.bytes_str_lossy(kind.get()), "get", "{pretty:?}");
        });
    });
}

/// `class C { set x(v) {} }` — a setter.
#[test]
fn class_setter_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip("class C { set x(v) {} }", pretty);
        with_first_stmt(&mut reparsed, |gc, stmt| {
            let Node::ClassDeclaration(ClassDeclaration { body, .. }) = stmt else {
                panic!("{pretty:?}: statement is not a ClassDeclaration");
            };
            let Node::ClassBody(ClassBody { body, .. }) = body else {
                panic!("{pretty:?}: class body is not a ClassBody");
            };
            let Node::MethodDefinition(MethodDefinition { kind, .. }) =
                body.iter().next().expect("has a member")
            else {
                panic!("{pretty:?}: member is not a MethodDefinition");
            };
            assert_eq!(gc.bytes_str_lossy(kind.get()), "set", "{pretty:?}");
        });
    });
}

/// `class C { static m() {} }` — a static method.
#[test]
fn class_static_method_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip("class C { static m() {} }", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::ClassDeclaration(ClassDeclaration { body, .. }) = stmt else {
                panic!("{pretty:?}: statement is not a ClassDeclaration");
            };
            let Node::ClassBody(ClassBody { body, .. }) = body else {
                panic!("{pretty:?}: class body is not a ClassBody");
            };
            let Node::MethodDefinition(MethodDefinition { r#static, .. }) =
                body.iter().next().expect("has a member")
            else {
                panic!("{pretty:?}: member is not a MethodDefinition");
            };
            assert!(r#static.get(), "{pretty:?}: static must survive");
        });
    });
}

/// `class C { #x = 1; #m() { return this.#x; } }` — a `#private` field and a
/// `#private` method together, since our parser represents their keys
/// differently: `ClassPrivateProperty::key` is a bare `Identifier` (the `#`
/// stripped, printed back by `GenJS::gen_class_private_property`'s own
/// `out!(self, "#")`), while `MethodDefinition::key` for a private method is
/// a `PrivateName`-wrapped `Identifier` (the `#` printed by
/// `arms/literal.rs`'s `gen_private_name` instead) — see `arms/func.rs`'s
/// module doc comment. This test exercises both key shapes in one program,
/// and the `!js.contains("##")` assertion is the empirical check that
/// neither path double-prints the `#`.
#[test]
fn class_private_field_and_private_method_round_trip() {
    for_each_pretty_mode(|pretty| {
        let mut parsed = parse_ok("class C { #x = 1; #m() { return this.#x; } }");
        let js = gen(
            &mut parsed,
            Opt {
                pretty,
                ..Opt::default()
            },
        );
        assert!(js.contains("#x"), "{pretty:?}: {js:?}");
        assert!(js.contains("#m"), "{pretty:?}: {js:?}");
        assert!(!js.contains("##"), "{pretty:?}: {js:?}");
        let mut reparsed = hermes_parser::parse(&js, Default::default())
            .unwrap_or_else(|e| panic!("{pretty:?}: regenerated {js:?} must parse: {e:?}"));
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::ClassDeclaration(ClassDeclaration { body, .. }) = stmt else {
                panic!("{pretty:?}: statement is not a ClassDeclaration");
            };
            let Node::ClassBody(ClassBody { body, .. }) = body else {
                panic!("{pretty:?}: class body is not a ClassBody");
            };
            let mut members = body.iter();
            let first = members.next().expect("has a first member");
            assert!(
                matches!(first, Node::ClassPrivateProperty(_)),
                "{pretty:?}: first member is not a ClassPrivateProperty: {first:?}"
            );
            let second = members.next().expect("has a second member");
            let Node::MethodDefinition(MethodDefinition { key, .. }) = second else {
                panic!("{pretty:?}: second member is not a MethodDefinition: {second:?}");
            };
            assert!(
                matches!(key, Node::PrivateName(_)),
                "{pretty:?}: private method key is not a PrivateName: {key:?}"
            );
        });
    });
}

/// `(class {});` — a class expression in expression-statement position
/// needs parens, or `class {}` at statement start would parse as a
/// `ClassDeclaration` instead (`need_parens`'s `ExpressionStatement` branch,
/// `precedence.rs`, ported in Task 3 — the same `root_starts_with` check
/// `arms/expr.rs`'s `iife_as_expression_statement_round_trips_with_parens`
/// exercises for `FunctionExpression`).
#[test]
fn class_expression_in_expression_statement_position_round_trips_with_parens() {
    for_each_pretty_mode(|pretty| {
        let mut parsed = parse_ok("(class {});");
        let js = gen(
            &mut parsed,
            Opt {
                pretty,
                ..Opt::default()
            },
        );
        assert!(
            js.trim_start().starts_with('('),
            "{pretty:?}: {js:?}"
        );
        let mut reparsed = hermes_parser::parse(&js, Default::default())
            .unwrap_or_else(|e| panic!("{pretty:?}: regenerated {js:?} must parse: {e:?}"));
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::ExpressionStatement(ExpressionStatement { expression, .. }) = stmt else {
                panic!("{pretty:?}: statement is not an ExpressionStatement: {stmt:?}");
            };
            assert!(
                matches!(expression, Node::ClassExpression(_)),
                "{pretty:?}: expression is not a ClassExpression: {expression:?}"
            );
        });
    });
}

/// `class C extends <heritage> {}` — the heritage slot is
/// `ClassHeritage : extends LeftHandSideExpression`, a strictly narrower
/// tier than the full expression grammar
/// (`crates/parser/src/js/classes.rs:437-438` calls
/// `parse_left_hand_side_expression`), so anything looser reached the field
/// only through explicit source parens. `super_class` was printed with a
/// bare `gen_node` (Task 7), dropping them (review-round-5 regression
/// test). Every case below **failed to reparse** before the fix.
#[test]
fn class_heritage_parenthesizes_non_left_hand_side_expressions() {
    for_each_pretty_mode(|pretty| {
        // The four cases named in the round-5 review, plus three more of
        // the same shape. Each asserts the `super_class` kind survived, not
        // merely that the source reparsed.
        let mut reparsed = round_trip("class C extends (a = b) {}", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            assert!(
                matches!(class_super(stmt, pretty), Node::AssignmentExpression(_)),
                "{pretty:?}: super_class must stay an AssignmentExpression"
            );
        });
        let mut reparsed = round_trip("class C extends (a ? b : c) {}", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            assert!(
                matches!(class_super(stmt, pretty), Node::ConditionalExpression(_)),
                "{pretty:?}: super_class must stay a ConditionalExpression"
            );
        });
        let mut reparsed = round_trip("class C extends (a + b) {}", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            assert!(
                matches!(class_super(stmt, pretty), Node::BinaryExpression(_)),
                "{pretty:?}: super_class must stay a BinaryExpression"
            );
        });
        let mut reparsed = round_trip("class C extends (() => 1) {}", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            assert!(
                matches!(class_super(stmt, pretty), Node::ArrowFunctionExpression(_)),
                "{pretty:?}: super_class must stay an ArrowFunctionExpression"
            );
        });
        let mut reparsed = round_trip("class C extends (a || b) {}", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            assert!(
                matches!(class_super(stmt, pretty), Node::LogicalExpression(_)),
                "{pretty:?}: super_class must stay a LogicalExpression"
            );
        });
        let mut reparsed = round_trip("class C extends (!a) {}", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            assert!(
                matches!(class_super(stmt, pretty), Node::UnaryExpression(_)),
                "{pretty:?}: super_class must stay a UnaryExpression"
            );
        });
        let mut reparsed = round_trip("class C extends (a++) {}", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            assert!(
                matches!(class_super(stmt, pretty), Node::UpdateExpression(_)),
                "{pretty:?}: super_class must stay an UpdateExpression"
            );
        });
        // A `ClassExpression`'s heritage goes through the same branch.
        let mut reparsed = round_trip("x = class extends (a = b) {};", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::ExpressionStatement(ExpressionStatement { expression, .. }) = stmt else {
                panic!("{pretty:?}: not an ExpressionStatement: {stmt:?}");
            };
            let Node::AssignmentExpression(AssignmentExpression { right, .. }) = expression else {
                panic!("{pretty:?}: not an AssignmentExpression: {expression:?}");
            };
            let Node::ClassExpression(hermes_ast::node::ClassExpression { super_class, .. }) =
                right
            else {
                panic!("{pretty:?}: not a ClassExpression: {right:?}");
            };
            assert!(
                matches!(
                    super_class.expect("has a super_class"),
                    Node::AssignmentExpression(_)
                ),
                "{pretty:?}: ClassExpression super_class must stay an AssignmentExpression"
            );
        });
        // No over-wrapping: every LeftHandSideExpression kind is legal bare
        // in heritage position and must not gain parens. Confirmed against
        // the parser as shipped for each of these spellings.
        for src in [
            "class C extends a {}",
            "class C extends a.b.c {}",
            "class C extends a() {}",
            "class C extends new Foo() {}",
            "class C extends a`t` {}",
            "class C extends class {} {}",
            "class C extends [1] {}",
            "class C extends null {}",
        ] {
            let mut parsed = parse_ok(src);
            let js = gen(&mut parsed, Opt { pretty, ..Opt::default() });
            assert!(
                !js.contains("extends ("),
                "{pretty:?}: {src}: a LeftHandSideExpression heritage must stay bare: {js:?}"
            );
        }
    });
}

/// The `super_class` of `stmt`, which must be a `ClassDeclaration` with one.
fn class_super<'gc>(stmt: &'gc Node<'gc>, pretty: Pretty) -> &'gc Node<'gc> {
    let Node::ClassDeclaration(ClassDeclaration { super_class, .. }) = stmt else {
        panic!("{pretty:?}: statement is not a ClassDeclaration: {stmt:?}");
    };
    super_class.expect("class has a super_class")
}

/// A `RecordExpression` in heritage position must keep its parens too, for
/// an independent reason: the parser disables the record-expression branch
/// entirely when `isClassHeritageArgument == Yes`
/// (`lib/Parser/JSParserImpl.cpp:4049-4053` and `:4077-4080`). Unlike the plain-JS
/// cases above, dropping these parens corrupts **silently** — before the
/// fix `class C extends (R {p: 1}) {}` regenerated as
/// `class C extends R {p: 1} {}`, which reparses with no error at all to a
/// `super_class` of just `Identifier(R)`, a `ClassProperty p: 1`, and a
/// stray empty `BlockStatement` (review-round-5 regression test).
#[test]
fn class_heritage_parenthesizes_record_expression() {
    for_each_pretty_mode(|pretty| {
        let src = "record R { p: number }\nclass C extends (R {p: 1}) {}";
        let mut reparsed = round_trip_records(src, pretty);
        reparsed.with_program(|_gc, node| {
            let Node::Program(Program { body, .. }) = node else {
                panic!("{pretty:?}: root is not a Program");
            };
            let stmts: Vec<&Node> = body.iter().collect();
            assert_eq!(
                stmts.len(),
                2,
                "{pretty:?}: the class body must not leak extra statements: {stmts:?}"
            );
            let Node::ClassDeclaration(ClassDeclaration { super_class, body, .. }) = stmts[1]
            else {
                panic!("{pretty:?}: not a ClassDeclaration: {:?}", stmts[1]);
            };
            assert!(
                matches!(
                    super_class.expect("class has a super_class"),
                    Node::RecordExpression(_)
                ),
                "{pretty:?}: super_class must stay a RecordExpression, not collapse to \
                 Identifier(R) with the properties leaking into the class body"
            );
            let Node::ClassBody(hermes_ast::node::ClassBody { body, .. }) = body else {
                panic!("{pretty:?}: not a ClassBody: {body:?}");
            };
            assert_eq!(
                body.iter().count(),
                0,
                "{pretty:?}: the class body must stay empty — `p: 1` belongs to the record"
            );
        });
    });
}

/// A `MatchExpression` in heritage position, by contrast, needs NO parens:
/// it is `PRIMARY` and the parser accepts it bare there. Companion to the
/// two tests above, so the threshold is pinned from both sides.
#[test]
fn class_heritage_leaves_match_expression_bare() {
    for_each_pretty_mode(|pretty| {
        let src = "class C extends (match (x) { _ => 1 }) {}";
        let mut reparsed = round_trip_match(src, pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            assert!(
                matches!(class_super(stmt, pretty), Node::MatchExpression(_)),
                "{pretty:?}: super_class must stay a MatchExpression"
            );
        });
        let mut parsed = parse_ok_match(src);
        let js = gen(&mut parsed, Opt { pretty, ..Opt::default() });
        assert!(
            !js.contains("extends ("),
            "{pretty:?}: a match expression heritage must stay bare: {js:?}"
        );
    });
}

/// `class C { declare #x; }` (parsed under Flow) — the
/// `arms/func.rs`-documented juno bug regression test: a private field's
/// `declare` modifier must print back as `declare`, not a second `static`.
/// See `arms/func.rs`'s module doc comment ("A juno correctness bug found
/// and fixed here") for the full evidence trail; task-7-report.md has the
/// "revert the fix, watch this fail" transcript.
#[test]
fn class_private_property_declare_modifier_prints_declare_not_static() {
    let flow = ParseFlags {
        parse_flow: true,
        ..Default::default()
    };
    for_each_pretty_mode(|pretty| {
        let mut parsed = hermes_parser::parse("class C { declare #x; }", flow)
            .expect("Flow `declare` private field must parse");
        let js = gen(
            &mut parsed,
            Opt {
                pretty,
                ..Opt::default()
            },
        );
        assert!(js.contains("declare #x"), "{pretty:?}: {js:?}");
        assert!(!js.contains("static #x"), "{pretty:?}: {js:?}");
        let _ = hermes_parser::parse(&js, flow)
            .unwrap_or_else(|e| panic!("{pretty:?}: regenerated {js:?} must parse: {e:?}"));
    });
}

// ---------------------------------------------------------------------------
// Task 8 (`arms/module.rs`): `import`/`export` declarations.
// ---------------------------------------------------------------------------

/// `import Def, {A, B as C} from 'm';` — a default specifier and a named
/// group (one plain, one aliased) in a single declaration.
#[test]
fn import_default_and_multiple_named_specifiers_round_trip() {
    for_each_pretty_mode(|pretty| {
        let mut parsed = parse_ok("import Def, {A, B as C} from 'm';");
        let js = gen(
            &mut parsed,
            Opt {
                pretty,
                ..Opt::default()
            },
        );
        let mut reparsed = hermes_parser::parse(&js, Default::default())
            .unwrap_or_else(|e| panic!("{pretty:?}: regenerated {js:?} must parse: {e:?}"));
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::ImportDeclaration(ImportDeclaration {
                specifiers,
                attributes,
                ..
            }) = stmt
            else {
                panic!("{pretty:?}: statement is not an ImportDeclaration: {stmt:?}");
            };
            assert!(attributes.is_empty(), "{pretty:?}: {js:?}");
            let specs: Vec<&Node> = specifiers.iter().collect();
            assert_eq!(specs.len(), 3, "{pretty:?}: {js:?}");
            assert!(
                matches!(specs[0], Node::ImportDefaultSpecifier(_)),
                "{pretty:?}: {js:?}"
            );
            assert!(
                matches!(specs[1], Node::ImportSpecifier(_)),
                "{pretty:?}: {js:?}"
            );
            assert!(
                matches!(specs[2], Node::ImportSpecifier(_)),
                "{pretty:?}: {js:?}"
            );
        });
    });
}

/// `import x, {y as z} from 'm';` — a default specifier plus a single
/// aliased named specifier, the task brief's own example.
#[test]
fn import_default_and_aliased_named_specifier_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut parsed = parse_ok("import x, {y as z} from 'm';");
        let js = gen(
            &mut parsed,
            Opt {
                pretty,
                ..Opt::default()
            },
        );
        let mut reparsed = hermes_parser::parse(&js, Default::default())
            .unwrap_or_else(|e| panic!("{pretty:?}: regenerated {js:?} must parse: {e:?}"));
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::ImportDeclaration(ImportDeclaration { specifiers, .. }) = stmt else {
                panic!("{pretty:?}: statement is not an ImportDeclaration: {stmt:?}");
            };
            let specs: Vec<&Node> = specifiers.iter().collect();
            assert_eq!(specs.len(), 2, "{pretty:?}: {js:?}");
            assert!(
                matches!(specs[0], Node::ImportDefaultSpecifier(_)),
                "{pretty:?}: {js:?}"
            );
            let Node::ImportSpecifier(ImportSpecifier {
                imported, local, ..
            }) = specs[1]
            else {
                panic!("{pretty:?}: second specifier is not an ImportSpecifier: {:?}", specs[1]);
            };
            assert!(matches!(imported, Node::Identifier(_)), "{pretty:?}: {js:?}");
            assert!(matches!(local, Node::Identifier(_)), "{pretty:?}: {js:?}");
        });
    });
}

/// `import x from 'm' with { type: "json" };` — an import attribute.
///
/// Also the regression test for `arms/module.rs`'s module doc comment "the
/// import-attributes keyword is `with`, not `assert`" fix: our parser
/// (`crates/parser/src/js/modules.rs`'s `parse_with_clause`) only recognizes
/// `with`, so printing juno's `assert` spelling would make the regenerated
/// source fail to reparse at all.
#[test]
fn import_attribute_with_clause_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut parsed = parse_ok("import x from 'm' with { type: \"json\" };");
        let js = gen(
            &mut parsed,
            Opt {
                pretty,
                ..Opt::default()
            },
        );
        assert!(js.contains("with"), "{pretty:?}: {js:?}");
        assert!(!js.contains("assert"), "{pretty:?}: {js:?}");
        let mut reparsed = hermes_parser::parse(&js, Default::default())
            .unwrap_or_else(|e| panic!("{pretty:?}: regenerated {js:?} must parse: {e:?}"));
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::ImportDeclaration(ImportDeclaration { attributes, .. }) = stmt else {
                panic!("{pretty:?}: statement is not an ImportDeclaration: {stmt:?}");
            };
            let mut it = attributes.iter();
            let only = it.next().expect("one attribute");
            assert!(it.next().is_none(), "{pretty:?}: more than one attribute: {js:?}");
            let Node::ImportAttribute(ImportAttribute { key, value, .. }) = only else {
                panic!("{pretty:?}: attribute is not an ImportAttribute: {only:?}");
            };
            assert!(matches!(key, Node::Identifier(_)), "{pretty:?}: {js:?}");
            assert!(matches!(value, Node::StringLiteral(_)), "{pretty:?}: {js:?}");
        });
    });
}

/// `export * as ns from 'm';` — the regression test for `arms/module.rs`'s
/// module doc comment "`export * as ns from 'm'` form must not be wrapped in
/// `{ ... }`" fix. Without the fix, this prints `export {* as ns} from
/// 'm';`, which contains a `{` and fails to reparse at all (`*` cannot
/// appear inside an `ExportsList`) — see `arms/module.rs`'s module doc
/// comment for the full grammar accounting.
#[test]
fn export_namespace_specifier_round_trips_without_braces() {
    for_each_pretty_mode(|pretty| {
        let mut parsed = parse_ok("export * as ns from 'm';");
        let js = gen(
            &mut parsed,
            Opt {
                pretty,
                ..Opt::default()
            },
        );
        assert!(!js.contains('{'), "{pretty:?}: {js:?}");
        let mut reparsed = hermes_parser::parse(&js, Default::default())
            .unwrap_or_else(|e| panic!("{pretty:?}: regenerated {js:?} must parse: {e:?}"));
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::ExportNamedDeclaration(ExportNamedDeclaration {
                declaration,
                specifiers,
                source,
                ..
            }) = stmt
            else {
                panic!("{pretty:?}: statement is not an ExportNamedDeclaration: {stmt:?}");
            };
            assert!(declaration.is_none(), "{pretty:?}: {js:?}");
            assert!(source.is_some(), "{pretty:?}: {js:?}");
            let mut it = specifiers.iter();
            let only = it.next().expect("one specifier");
            assert!(it.next().is_none(), "{pretty:?}: more than one specifier: {js:?}");
            let Node::ExportNamespaceSpecifier(ExportNamespaceSpecifier { exported, .. }) = only
            else {
                panic!("{pretty:?}: specifier is not an ExportNamespaceSpecifier: {only:?}");
            };
            assert!(matches!(exported, Node::Identifier(_)), "{pretty:?}: {js:?}");
        });
    });
}

/// `export default function(){}` — a real `FunctionDeclaration`, prints
/// back unparenthesized.
#[test]
fn export_default_function_declaration_round_trips_without_parens() {
    for_each_pretty_mode(|pretty| {
        let mut parsed = parse_ok("export default function(){}");
        let js = gen(
            &mut parsed,
            Opt {
                pretty,
                ..Opt::default()
            },
        );
        assert!(
            !js.contains("default (function"),
            "{pretty:?}: {js:?}"
        );
        let mut reparsed = hermes_parser::parse(&js, Default::default())
            .unwrap_or_else(|e| panic!("{pretty:?}: regenerated {js:?} must parse: {e:?}"));
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::ExportDefaultDeclaration(ExportDefaultDeclaration { declaration, .. }) =
                stmt
            else {
                panic!("{pretty:?}: statement is not an ExportDefaultDeclaration: {stmt:?}");
            };
            assert!(
                matches!(declaration, Node::FunctionDeclaration(_)),
                "{pretty:?}: declaration is not a FunctionDeclaration: {declaration:?}"
            );
        });
    });
}

/// `export default (function(){});` — an anonymous `FunctionExpression`
/// used as the default export value. The regression test for
/// `precedence.rs`'s new `ExportDefaultDeclaration` branch of `need_parens`
/// (see `arms/module.rs`'s module doc comment): without it, this prints
/// `export default function(){}` (parens dropped), which reparses as a
/// `FunctionDeclaration` instead — a silent node-kind flip.
#[test]
fn export_default_function_expression_round_trips_with_parens() {
    for_each_pretty_mode(|pretty| {
        let mut parsed = parse_ok("export default (function(){});");
        let js = gen(
            &mut parsed,
            Opt {
                pretty,
                ..Opt::default()
            },
        );
        assert!(js.contains("default (function"), "{pretty:?}: {js:?}");
        let mut reparsed = hermes_parser::parse(&js, Default::default())
            .unwrap_or_else(|e| panic!("{pretty:?}: regenerated {js:?} must parse: {e:?}"));
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::ExportDefaultDeclaration(ExportDefaultDeclaration { declaration, .. }) =
                stmt
            else {
                panic!("{pretty:?}: statement is not an ExportDefaultDeclaration: {stmt:?}");
            };
            assert!(
                matches!(declaration, Node::FunctionExpression(_)),
                "{pretty:?}: declaration is not a FunctionExpression (parens were dropped, \
                 so it reparsed as a FunctionDeclaration instead): {declaration:?}"
            );
        });
    });
}

// ---------------------------------------------------------------------------
// Task 9 (`arms/jsx.rs`): JSX. Every case here needs `parse_jsx: true` — the
// rest of this file's helpers (`parse_ok`/`round_trip`) hard-code
// `ParseFlags::default()`, which rejects JSX syntax outright, so this
// section has its own `parse_ok_jsx`/`round_trip_jsx` instead of reusing
// them.
// ---------------------------------------------------------------------------

fn jsx_flags() -> ParseFlags {
    ParseFlags {
        parse_jsx: true,
        ..Default::default()
    }
}

/// Parse `src` under [`jsx_flags`], panicking with a message that includes
/// `src` on failure. The JSX-flagged counterpart to `parse_ok`.
fn parse_ok_jsx(src: &str) -> ParsedJS {
    hermes_parser::parse(src, jsx_flags())
        .unwrap_or_else(|e| panic!("{src:?} must parse under -parse-jsx: {e:?}"))
}

/// Generate `src` under `pretty` (with [`jsx_flags`] on both the initial
/// parse and the reparse), then parse the result back. Panics (naming
/// `pretty` and the generated text) if the regenerated source fails to
/// reparse. The JSX-flagged counterpart to `round_trip`.
fn round_trip_jsx(src: &str, pretty: Pretty) -> ParsedJS {
    let mut parsed = parse_ok_jsx(src);
    let js = gen(
        &mut parsed,
        Opt {
            pretty,
            ..Opt::default()
        },
    );
    hermes_parser::parse(&js, jsx_flags()).unwrap_or_else(|e| {
        panic!("regenerated source {js:?} (from {src:?}, {pretty:?}) must parse under -parse-jsx: {e:?}")
    })
}

/// Hands `parsed`'s `Program`'s first top-level statement's
/// `var <ident> = <jsx>;` initializer to `f`, along with the locked
/// `GCLock`. Every JSX case below is written this shape (`var x = <.../>;`)
/// specifically so JSX's leading `<` is never at statement-expression
/// start, where it would need `need_parens`/`root_starts_with` protection
/// that isn't this task's concern (`get_precedence`'s `PRIMARY`
/// classification for `JSXElement`/`JSXFragment` was already ported by an
/// earlier task; this file only exercises the JSX arms themselves).
fn with_jsx_init<R>(
    parsed: &mut ParsedJS,
    f: impl for<'gc> FnOnce(&'gc GCLock<'static, '_>, &'gc Node<'gc>) -> R,
) -> R {
    with_first_stmt(parsed, |gc, stmt| {
        let Node::VariableDeclaration(VariableDeclaration {
            metadata: _,
            kind: _,
            declarations,
        }) = stmt
        else {
            panic!("statement is not a VariableDeclaration: {stmt:?}");
        };
        let Node::VariableDeclarator(VariableDeclarator {
            metadata: _,
            init,
            id: _,
        }) = declarations.iter().next().expect("has a declarator")
        else {
            panic!("declaration is not a VariableDeclarator");
        };
        f(gc, init.expect("declarator has an initializer"))
    })
}

/// `<div id="x" />` — a self-closing element with one plain (string-valued)
/// attribute.
#[test]
fn jsx_self_closing_element_with_attribute_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip_jsx(r#"var x = <div id="x" />;"#, pretty);
        with_jsx_init(&mut reparsed, |gc, init| {
            let Node::JSXElement(JSXElement {
                metadata: _,
                opening_element,
                children,
                closing_element,
            }) = init
            else {
                panic!("{pretty:?}: init is not a JSXElement: {init:?}");
            };
            assert!(
                children.is_empty(),
                "{pretty:?}: self-closing has no children"
            );
            assert!(
                closing_element.is_none(),
                "{pretty:?}: self-closing has no closing element"
            );
            let Node::JSXOpeningElement(JSXOpeningElement {
                metadata: _,
                name,
                attributes,
                self_closing,
                type_arguments: _,
            }) = opening_element
            else {
                panic!("{pretty:?}: opening_element is not a JSXOpeningElement");
            };
            assert!(self_closing.get(), "{pretty:?}");
            let name = name.as_jsx_identifier().expect("name is a JSXIdentifier");
            assert_eq!(name.name_str(gc), "div", "{pretty:?}");
            let mut attrs = attributes.iter();
            let Node::JSXAttribute(JSXAttribute {
                metadata: _,
                name: attr_name,
                value,
            }) = attrs.next().expect("has one attribute")
            else {
                panic!("{pretty:?}: attribute is not a JSXAttribute");
            };
            assert!(attrs.next().is_none(), "{pretty:?}: exactly one attribute");
            let attr_name = attr_name
                .as_jsx_identifier()
                .expect("attribute name is a JSXIdentifier");
            assert_eq!(attr_name.name_str(gc), "id", "{pretty:?}");
            let Some(Node::JSXStringLiteral(JSXStringLiteral {
                metadata: _,
                value: _,
                raw,
            })) = value
            else {
                panic!("{pretty:?}: attribute value is not a JSXStringLiteral: {value:?}");
            };
            assert_eq!(gc.bytes_str_lossy(raw.get()), "\"x\"", "{pretty:?}");
        });
    });
}

/// `<svg:rect />` — a namespaced element name.
#[test]
fn jsx_namespaced_name_element_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip_jsx("var x = <svg:rect />;", pretty);
        with_jsx_init(&mut reparsed, |gc, init| {
            let Node::JSXElement(JSXElement {
                opening_element, ..
            }) = init
            else {
                panic!("{pretty:?}: init is not a JSXElement: {init:?}");
            };
            let name = opening_element
                .as_jsx_opening_element()
                .expect("opening_element is a JSXOpeningElement")
                .name;
            let Node::JSXNamespacedName(JSXNamespacedName {
                metadata: _,
                namespace,
                name,
            }) = name
            else {
                panic!("{pretty:?}: name is not a JSXNamespacedName: {name:?}");
            };
            assert_eq!(
                namespace.as_jsx_identifier().unwrap().name_str(gc),
                "svg",
                "{pretty:?}"
            );
            assert_eq!(
                name.as_jsx_identifier().unwrap().name_str(gc),
                "rect",
                "{pretty:?}"
            );
        });
    });
}

/// `<Foo.Bar.Baz></Foo.Bar.Baz>` — a member-expression element name, on
/// both the opening and (matching) closing tag.
#[test]
fn jsx_member_expression_name_element_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip_jsx("var x = <Foo.Bar.Baz></Foo.Bar.Baz>;", pretty);
        with_jsx_init(&mut reparsed, |gc, init| {
            let Node::JSXElement(JSXElement {
                opening_element,
                closing_element,
                children,
                ..
            }) = init
            else {
                panic!("{pretty:?}: init is not a JSXElement: {init:?}");
            };
            assert!(children.is_empty(), "{pretty:?}");
            assert!(closing_element.is_some(), "{pretty:?}");
            let name = opening_element
                .as_jsx_opening_element()
                .expect("opening_element is a JSXOpeningElement")
                .name;
            // `Foo.Bar.Baz` nests as `(Foo.Bar).Baz`.
            let Node::JSXMemberExpression(JSXMemberExpression {
                metadata: _,
                object,
                property,
            }) = name
            else {
                panic!("{pretty:?}: name is not a JSXMemberExpression: {name:?}");
            };
            assert_eq!(
                property.as_jsx_identifier().unwrap().name_str(gc),
                "Baz",
                "{pretty:?}"
            );
            let Node::JSXMemberExpression(JSXMemberExpression {
                object: inner_object,
                property: inner_property,
                ..
            }) = object
            else {
                panic!("{pretty:?}: object is not a JSXMemberExpression: {object:?}");
            };
            assert_eq!(
                inner_object.as_jsx_identifier().unwrap().name_str(gc),
                "Foo",
                "{pretty:?}"
            );
            assert_eq!(
                inner_property.as_jsx_identifier().unwrap().name_str(gc),
                "Bar",
                "{pretty:?}"
            );
        });
    });
}

/// `<div {...props} />` — a spread attribute.
#[test]
fn jsx_spread_attribute_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip_jsx("var x = <div {...props} />;", pretty);
        with_jsx_init(&mut reparsed, |_gc, init| {
            let Node::JSXElement(JSXElement {
                opening_element, ..
            }) = init
            else {
                panic!("{pretty:?}: init is not a JSXElement: {init:?}");
            };
            let attributes = opening_element
                .as_jsx_opening_element()
                .expect("opening_element is a JSXOpeningElement")
                .attributes;
            let mut attrs = attributes.iter();
            let Node::JSXSpreadAttribute(JSXSpreadAttribute {
                metadata: _,
                argument,
            }) = attrs.next().expect("has one attribute")
            else {
                panic!("{pretty:?}: attribute is not a JSXSpreadAttribute");
            };
            assert!(attrs.next().is_none(), "{pretty:?}: exactly one attribute");
            assert!(matches!(argument, Node::Identifier(_)), "{pretty:?}");
        });
    });
}

/// `<>hello</>` — a fragment.
#[test]
fn jsx_fragment_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip_jsx("var x = <>hello</>;", pretty);
        with_jsx_init(&mut reparsed, |gc, init| {
            let Node::JSXFragment(JSXFragment {
                metadata: _,
                opening_fragment,
                children,
                closing_fragment,
            }) = init
            else {
                panic!("{pretty:?}: init is not a JSXFragment: {init:?}");
            };
            assert!(
                matches!(opening_fragment, Node::JSXOpeningFragment(_)),
                "{pretty:?}"
            );
            assert!(
                matches!(closing_fragment, Node::JSXClosingFragment(_)),
                "{pretty:?}"
            );
            let mut it = children.iter();
            let Node::JSXText(JSXText {
                metadata: _,
                value: _,
                raw,
            }) = it.next().expect("has one child")
            else {
                panic!("{pretty:?}: child is not JSXText");
            };
            assert!(it.next().is_none(), "{pretty:?}: exactly one child");
            assert_eq!(gc.bytes_str_lossy(raw.get()), "hello", "{pretty:?}");
        });
    });
}

/// `<div>{value}</div>` — an expression-container child.
#[test]
fn jsx_expression_container_child_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip_jsx("var x = <div>{value}</div>;", pretty);
        with_jsx_init(&mut reparsed, |_gc, init| {
            let Node::JSXElement(JSXElement { children, .. }) = init else {
                panic!("{pretty:?}: init is not a JSXElement: {init:?}");
            };
            let mut it = children.iter();
            let Node::JSXExpressionContainer(JSXExpressionContainer {
                metadata: _,
                expression,
            }) = it.next().expect("has one child")
            else {
                panic!("{pretty:?}: child is not a JSXExpressionContainer");
            };
            assert!(it.next().is_none(), "{pretty:?}: exactly one child");
            assert!(matches!(expression, Node::Identifier(_)), "{pretty:?}");
        });
    });
}

/// `<div>a}b&c\nd{e}<span/>f</div>` — the brief's "text containing `{`, `}`,
/// `<`, `&`, and a newline" case.
///
/// A `JSXText` node itself can never directly contain a literal `{` or `<`
/// — both are structural in JSX child position (`crates/parser/src/lexer/jsx.rs`'s
/// `advance_in_jsx_child` ends the current text run the instant it sees
/// either byte, the same way `>`/EOF do), so there is no way to construct a
/// single `JSXText` node holding all five characters literally. This test
/// gets as close as the grammar allows: one `JSXText` child (`"a}b&c\nd"`)
/// that legally holds `}`, a literal `&` that fails HTML-entity lookup (`&c`
/// is not a named entity and has no terminating `;`, so
/// `consume_html_entity_optional` backs off and the `&` is copied through
/// literally — `crates/parser/src/lexer/jsx.rs`'s `jsx_child` test exercises the
/// same fallback), and an embedded newline — immediately followed by an
/// `JSXExpressionContainer` (`{e}`) and a nested self-closing `JSXElement`
/// (`<span/>`) as sibling children, exercising the `{`/`<` boundary the
/// text run stops at. This is exactly the printer hazard the task
/// description points at: `write_char` (juno `gen_js.rs:322-330`,
/// `arms/gen.rs`'s port) forbids embedding a literal `\n` byte directly, so
/// `gen_jsx_raw_text`'s per-character loop must route `\n` through
/// `force_newline_without_indent` instead of `write_char`, or this panics in
/// a debug build (the `debug_assert!(ch != '\n', ...)` in `GenJS::write_char`)
/// rather than merely mis-printing.
#[test]
fn jsx_text_with_brace_amp_and_newline_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip_jsx("var x = <div>a}b&c\nd{e}<span/>f</div>;", pretty);
        with_jsx_init(&mut reparsed, |gc, init| {
            let Node::JSXElement(JSXElement { children, .. }) = init else {
                panic!("{pretty:?}: init is not a JSXElement: {init:?}");
            };
            let kids: Vec<&Node> = children.iter().collect();
            assert_eq!(kids.len(), 4, "{pretty:?}: {kids:?}");

            let Node::JSXText(JSXText {
                metadata: _,
                value: _,
                raw,
            }) = kids[0]
            else {
                panic!("{pretty:?}: first child is not JSXText: {:?}", kids[0]);
            };
            assert_eq!(gc.bytes_str_lossy(raw.get()), "a}b&c\nd", "{pretty:?}");

            assert!(
                matches!(kids[1], Node::JSXExpressionContainer(_)),
                "{pretty:?}: second child is not a JSXExpressionContainer: {:?}",
                kids[1]
            );
            assert!(
                matches!(kids[2], Node::JSXElement(_)),
                "{pretty:?}: third child is not a JSXElement: {:?}",
                kids[2]
            );

            let Node::JSXText(JSXText {
                metadata: _,
                value: _,
                raw,
            }) = kids[3]
            else {
                panic!("{pretty:?}: fourth child is not JSXText: {:?}", kids[3]);
            };
            assert_eq!(gc.bytes_str_lossy(raw.get()), "f", "{pretty:?}");
        });
    });
}

// ---------------------------------------------------------------------------
// Task 11 (`arms/flow_decl.rs`): Flow declarations, object types, and enums.
//
// The `round_trip_return_flow_type` section below is the migration promised
// by Task 10's review: those tests used to live in `arms/flow_type.rs`'s own
// `#[cfg(test)]` module with a hand-rolled unwrap/re-embed workaround,
// because no real Flow type could reach `generate()`'s public entry point
// until `Node::TypeAnnotation` (this task's own kind) had a dispatch arm.
// Every test body below is the original assertion logic, unchanged; only the
// plumbing that reaches it changed, from `gen_node` called directly on an
// isolated, hand-unwrapped type to the crate's ordinary `generate()`-based
// pipeline on a whole `function f(): T {}` program, with the return type's
// own printed text re-extracted from the full program text via
// `extract_return_type_text` (see its own doc comment for why that is safe
// even though this file drives full-program `generate()`, not
// `gen_node`-on-a-fragment the way `arms/flow_type.rs`'s workaround did).
// ---------------------------------------------------------------------------

fn flow_flags() -> ParseFlags {
    ParseFlags {
        parse_flow: true,
        ..Default::default()
    }
}

/// Parse `src` under [`flow_flags`], panicking with a message that includes
/// `src` on failure. The Flow-flagged counterpart to `parse_ok`.
fn parse_ok_flow(src: &str) -> ParsedJS {
    hermes_parser::parse(src, flow_flags())
        .unwrap_or_else(|e| panic!("{src:?} must parse under -parse-flow: {e:?}"))
}

/// Generate `src` under `pretty` (with [`flow_flags`] on both the initial
/// parse and the reparse), then parse the result back. Panics (naming
/// `pretty` and the generated text) if the regenerated source fails to
/// reparse. The Flow-flagged counterpart to `round_trip`.
fn round_trip_flow(src: &str, pretty: Pretty) -> ParsedJS {
    let mut parsed = parse_ok_flow(src);
    let js = gen(
        &mut parsed,
        Opt {
            pretty,
            ..Opt::default()
        },
    );
    hermes_parser::parse(&js, flow_flags()).unwrap_or_else(|e| {
        panic!("regenerated source {js:?} (from {src:?}, {pretty:?}) must parse under -parse-flow: {e:?}")
    })
}

/// [`round_trip_flow`], but also returns the generated program text — needed
/// by [`round_trip_return_flow_type`] below, which re-extracts just the
/// return-type slice from it.
fn round_trip_flow_text(src: &str, pretty: Pretty) -> (ParsedJS, String) {
    let mut parsed = parse_ok_flow(src);
    let js = gen(
        &mut parsed,
        Opt {
            pretty,
            ..Opt::default()
        },
    );
    let reparsed = hermes_parser::parse(&js, flow_flags()).unwrap_or_else(|e| {
        panic!("regenerated source {js:?} (from {src:?}, {pretty:?}) must parse under -parse-flow: {e:?}")
    });
    (reparsed, js)
}

/// Hands `parsed`'s `Program`'s first top-level statement's
/// `function f(): T {}` return type — unwrapped past its `TypeAnnotation`
/// node — to `f`, along with the locked `GCLock`. `FunctionDeclaration`'s
/// own `returnType` is always `TypeAnnotation`-wrapped (confirmed against
/// `crates/parser/src/js/flow/function_types.rs`'s
/// `parse_return_type_annotation_flow`, `wrapped_start: Some(..)`). Panics if
/// the statement is not a `function` declaration with a declared return
/// type.
fn with_return_flow_type<R>(
    parsed: &mut ParsedJS,
    f: impl for<'gc> FnOnce(&'gc GCLock<'static, '_>, &'gc Node<'gc>) -> R,
) -> R {
    with_first_stmt(parsed, |gc, stmt| {
        let Node::FunctionDeclaration(FunctionDeclaration {
            metadata: _,
            id: _,
            params: _,
            body: _,
            type_parameters: _,
            return_type,
            predicate: _,
            generator: _,
            r#async: _,
            scope: _,
            sem_info: _,
            strictness: _,
            is_method_definition: _,
            decorations: _,
        }) = stmt
        else {
            panic!("statement is not a FunctionDeclaration: {stmt:?}");
        };
        let return_type = return_type.expect("function has a declared return type");
        let Node::TypeAnnotation(TypeAnnotation {
            metadata: _,
            type_annotation,
        }) = return_type
        else {
            panic!("return type is not wrapped in a TypeAnnotation: {return_type:?}");
        };
        f(gc, type_annotation)
    })
}

/// Extract the isolated return-type text from a full `"function f(): T {}"`
/// (or, non-pretty, `"function f():T{}"`) program's generated text — the
/// exact substring `T` occupies, with no surrounding boilerplate. This is
/// what lets every migrated test below keep asserting on `generated` exactly
/// as it did when `arms/flow_type.rs`'s workaround generated only the
/// isolated type directly: the full-program text has a fixed, known prefix
/// (`"function f():"`/`"function f(): "`) and suffix (`"{}\n"`/`" {}\n"`,
/// confirmed empirically — Task 7's function-declaration printer with an
/// empty body and no other modifiers), so stripping them recovers exactly
/// the same text `gen_node` on the bare type alone would have produced.
/// Panics (naming `generated`/`pretty`) if the text doesn't have the
/// expected shape — a stronger check than `str::contains` would give: a
/// redundantly parenthesized type (the exact bug several of these tests
/// guard against) would still satisfy `contains`, since the wrapped text is
/// a superstring of the unwrapped one, but it would shift what
/// `strip_prefix`/`strip_suffix` return, so exact-equality assertions below
/// still catch it.
fn extract_return_type_text(generated: &str, pretty: Pretty) -> &str {
    let (prefix, suffix) = match pretty {
        Pretty::Yes => ("function f(): ", " {}\n"),
        Pretty::No => ("function f():", "{}\n"),
    };
    generated
        .strip_prefix(prefix)
        .and_then(|s| s.strip_suffix(suffix))
        .unwrap_or_else(|| {
            panic!(
                "generated {generated:?} ({pretty:?}) doesn't have the expected \
                 \"function f(): T {{}}\" shape"
            )
        })
}

/// Parse `src` (a `function f(): T {}` declaration) under Flow, generate the
/// whole program back under `pretty`, reparse *that* under Flow too (the
/// ordinary full round trip every other test in this file does), then hand
/// the reparsed return-type node, its `GCLock`, and the isolated
/// (`extract_return_type_text`) generated return-type text to `check`.
/// Panics (naming `src`/`pretty`/the generated text) if either parse fails.
fn round_trip_return_flow_type<R>(
    src: &str,
    pretty: Pretty,
    check: impl for<'gc> FnOnce(&'gc GCLock<'static, '_>, &'gc Node<'gc>, &str) -> R,
) -> R {
    let (mut reparsed, generated) = round_trip_flow_text(src, pretty);
    let isolated = extract_return_type_text(&generated, pretty).to_string();
    with_return_flow_type(&mut reparsed, |gc, ty| check(gc, ty, &isolated))
}

/// A union of one member from every primitive-keyword and literal Flow type
/// arm Task 10 ports: `ExistsTypeAnnotation` (`*`) through
/// `VoidTypeAnnotation`. Covers all 15 in one round trip rather than one test
/// apiece, since each arm is a single `out!` call with no branching to
/// exercise individually.
#[test]
fn all_primitive_and_literal_flow_types_round_trip_in_one_union() {
    for_each_pretty_mode(|pretty| {
        round_trip_return_flow_type(
            "function f(): *|empty|string|bigint|number|'lit'|42|123n|boolean|true|null|symbol|any|mixed|void {}",
            pretty,
            |gc, ty, generated| {
                let Node::UnionTypeAnnotation(UnionTypeAnnotation { types, .. }) = ty else {
                    panic!("{pretty:?}: not a UnionTypeAnnotation: {ty:?} ({generated:?})");
                };
                let members: Vec<&Node> = types.iter().collect();
                assert_eq!(members.len(), 15, "{pretty:?}: {generated:?}");
                assert!(matches!(members[0], Node::ExistsTypeAnnotation(_)), "{pretty:?}");
                assert!(matches!(members[1], Node::EmptyTypeAnnotation(_)), "{pretty:?}");
                assert!(matches!(members[2], Node::StringTypeAnnotation(_)), "{pretty:?}");
                assert!(matches!(members[3], Node::BigIntTypeAnnotation(_)), "{pretty:?}");
                assert!(matches!(members[4], Node::NumberTypeAnnotation(_)), "{pretty:?}");
                assert!(
                    matches!(members[5], Node::StringLiteralTypeAnnotation(_)),
                    "{pretty:?}"
                );
                assert!(
                    matches!(members[6], Node::NumberLiteralTypeAnnotation(_)),
                    "{pretty:?}"
                );
                let Node::BigIntLiteralTypeAnnotation(BigIntLiteralTypeAnnotation {
                    raw,
                    ..
                }) = members[7]
                else {
                    panic!("{pretty:?}: member 7 is not a BigIntLiteralTypeAnnotation");
                };
                assert!(
                    gc.bytes_str_lossy(raw.get()).ends_with('n'),
                    "{pretty:?}: BigInt type literal must keep its `n` suffix: {:?}",
                    gc.bytes_str_lossy(raw.get())
                );
                assert!(matches!(members[8], Node::BooleanTypeAnnotation(_)), "{pretty:?}");
                assert!(
                    matches!(members[9], Node::BooleanLiteralTypeAnnotation(_)),
                    "{pretty:?}"
                );
                assert!(
                    matches!(members[10], Node::NullLiteralTypeAnnotation(_)),
                    "{pretty:?}"
                );
                assert!(matches!(members[11], Node::SymbolTypeAnnotation(_)), "{pretty:?}");
                assert!(matches!(members[12], Node::AnyTypeAnnotation(_)), "{pretty:?}");
                assert!(matches!(members[13], Node::MixedTypeAnnotation(_)), "{pretty:?}");
                assert!(matches!(members[14], Node::VoidTypeAnnotation(_)), "{pretty:?}");
            },
        );
    });
}

/// `?(a | b)` — `?` must wrap the whole union, not just `a`: unparenthesized
/// `?a | b` parses as `(?a) | b` instead (`NullableTypeAnnotation` is
/// tighter-binding `UNARY` precedence than `UnionTypeAnnotation`'s
/// `UNION_TYPE` — `precedence.rs:707-723`), a genuinely different type, not
/// merely different-looking source.
#[test]
fn nullable_wrapping_union_round_trips_preserving_which_side_the_nullable_is_on() {
    for_each_pretty_mode(|pretty| {
        round_trip_return_flow_type(
            "function f(): ?(a | b) {}",
            pretty,
            |_gc, ty, generated| {
                assert!(generated.starts_with("?("), "{pretty:?}: {generated:?}");
                let Node::NullableTypeAnnotation(NullableTypeAnnotation {
                    type_annotation,
                    ..
                }) = ty
                else {
                    panic!("{pretty:?}: not a NullableTypeAnnotation: {ty:?} ({generated:?})");
                };
                let Node::UnionTypeAnnotation(UnionTypeAnnotation { types, .. }) =
                    *type_annotation
                else {
                    panic!(
                    "{pretty:?}: nullable's inner type is not a UnionTypeAnnotation: {type_annotation:?}"
                );
                };
                let members: Vec<&Node> = types.iter().collect();
                assert_eq!(members.len(), 2, "{pretty:?}: {generated:?}");
                assert!(
                    matches!(members[0], Node::GenericTypeAnnotation(_)),
                    "{pretty:?}"
                );
                assert!(
                    matches!(members[1], Node::GenericTypeAnnotation(_)),
                    "{pretty:?}"
                );
            },
        );
    });
}

/// `(a & b) | c` — the intersection itself needs no literal parens as a
/// whole: `&`'s `INTERSECTION_TYPE` precedence is tighter than `|`'s
/// `UNION_TYPE` (`precedence.rs:707-723`). Asserted below both structurally
/// (still 2 union members, not 3, which a flattened intersection would
/// produce) and as exactly `a&b|c` with zero parens.
#[test]
fn intersection_inside_union_round_trips_without_needing_literal_parens() {
    round_trip_return_flow_type(
        "function f(): (a & b) | c {}",
        Pretty::No,
        |_gc, ty, generated| {
            assert_eq!(generated, "a&b|c", "{generated:?}");
            let Node::UnionTypeAnnotation(UnionTypeAnnotation { types, .. }) = ty else {
                panic!("not a UnionTypeAnnotation: {ty:?} ({generated:?})");
            };
            let members: Vec<&Node> = types.iter().collect();
            assert_eq!(members.len(), 2, "{generated:?}");
            let Node::IntersectionTypeAnnotation(IntersectionTypeAnnotation {
                types: intersection_types,
                ..
            }) = members[0]
            else {
                panic!("member 0 is not an IntersectionTypeAnnotation: {generated:?}");
            };
            assert_eq!(intersection_types.iter().count(), 2, "{generated:?}");
            assert!(
                matches!(members[1], Node::GenericTypeAnnotation(_)),
                "{generated:?}"
            );
        },
    );
}

/// A function type with a `this:` parameter, an optional parameter, and a
/// rest parameter whose own type is a generic instantiation
/// (`Array<number>`, exercising `TypeParameterInstantiation` —
/// `GenJS::gen_type_parameter_list`), plus `this:` for extra coverage of
/// `GenJS::visit_func_type_params`'s special-cased branch.
#[test]
fn function_type_with_this_optional_and_rest_params_round_trips() {
    for_each_pretty_mode(|pretty| {
        round_trip_return_flow_type(
            "function f(): (this: Foo, a: number, b?: string, ...c: Array<number>) => void {}",
            pretty,
            |_gc, ty, generated| {
                let Node::FunctionTypeAnnotation(FunctionTypeAnnotation {
                    params,
                    this,
                    return_type,
                    rest,
                    type_parameters,
                    ..
                }) = ty
                else {
                    panic!("{pretty:?}: not a FunctionTypeAnnotation: {ty:?} ({generated:?})");
                };
                assert!(type_parameters.is_none(), "{pretty:?}");
                assert!(
                    matches!(*return_type, Node::VoidTypeAnnotation(_)),
                    "{pretty:?}"
                );

                let this = this.expect("has a this param");
                let Node::FunctionTypeParam(FunctionTypeParam {
                    name,
                    type_annotation,
                    ..
                }) = this
                else {
                    panic!("{pretty:?}: this is not a FunctionTypeParam: {this:?}");
                };
                assert!(name.is_none(), "{pretty:?}: this param prints no name");
                assert!(
                    matches!(type_annotation, Node::GenericTypeAnnotation(_)),
                    "{pretty:?}"
                );

                let params: Vec<&Node> = params.iter().collect();
                assert_eq!(params.len(), 2, "{pretty:?}: {generated:?}");
                let Node::FunctionTypeParam(FunctionTypeParam {
                    optional: a_opt, ..
                }) = params[0]
                else {
                    panic!(
                        "{pretty:?}: param a is not a FunctionTypeParam: {:?}",
                        params[0]
                    );
                };
                assert!(!a_opt.get(), "{pretty:?}: `a` must not be optional");
                let Node::FunctionTypeParam(FunctionTypeParam {
                    optional: b_opt, ..
                }) = params[1]
                else {
                    panic!(
                        "{pretty:?}: param b is not a FunctionTypeParam: {:?}",
                        params[1]
                    );
                };
                assert!(b_opt.get(), "{pretty:?}: `b?` must round-trip as optional");

                let rest = rest.expect("has a rest param");
                let Node::FunctionTypeParam(FunctionTypeParam {
                    type_annotation: rest_type,
                    ..
                }) = rest
                else {
                    panic!("{pretty:?}: rest is not a FunctionTypeParam: {rest:?}");
                };
                let Node::GenericTypeAnnotation(GenericTypeAnnotation {
                    type_parameters, ..
                }) = rest_type
                else {
                    panic!(
                        "{pretty:?}: rest type is not a GenericTypeAnnotation: {rest_type:?}"
                    );
                };
                assert!(
                    type_parameters.is_some(),
                    "{pretty:?}: Array<number> must keep its type argument"
                );
            },
        );
    });
}

/// `typeof x` — `TypeofTypeAnnotation` wrapping a plain `Identifier`. A
/// *dotted* `typeof x.y` is deliberately not tested here: its `argument`
/// would be a `QualifiedTypeofIdentifier`, with no dispatch arm until Task
/// 12 (`arms/flow_type.rs`'s own module doc comment).
#[test]
fn typeof_identifier_round_trips() {
    for_each_pretty_mode(|pretty| {
        round_trip_return_flow_type(
            "function f(): typeof x {}",
            pretty,
            |_gc, ty, generated| {
                let Node::TypeofTypeAnnotation(TypeofTypeAnnotation {
                    argument,
                    type_arguments,
                    ..
                }) = ty
                else {
                    panic!("{pretty:?}: not a TypeofTypeAnnotation: {ty:?} ({generated:?})");
                };
                assert!(matches!(argument, Node::Identifier(_)), "{pretty:?}");
                assert!(type_arguments.is_none(), "{pretty:?}");
            },
        );
    });
}

/// `Array<?string>` — a `GenericTypeAnnotation` instantiated with a
/// `NullableTypeAnnotation` type argument. Angle brackets already delimit
/// each type argument, so `?string` never needs (and never gets) wrapping
/// parens the way `?(a | b)` does at statement-type level.
#[test]
fn generic_type_with_nullable_type_argument_round_trips() {
    round_trip_return_flow_type(
        "function f(): Array<?string> {}",
        Pretty::No,
        |_gc, ty, generated| {
            assert_eq!(generated, "Array<?string>", "{generated:?}");
            let Node::GenericTypeAnnotation(GenericTypeAnnotation {
                id,
                type_parameters,
                ..
            }) = ty
            else {
                panic!("not a GenericTypeAnnotation: {ty:?} ({generated:?})");
            };
            assert!(matches!(id, Node::Identifier(_)), "{generated:?}");
            let type_parameters = type_parameters.expect("has type parameters");
            let Node::TypeParameterInstantiation(TypeParameterInstantiation { params, .. }) =
                type_parameters
            else {
                panic!(
                    "type_parameters is not a TypeParameterInstantiation: {type_parameters:?}"
                );
            };
            let params: Vec<&Node> = params.iter().collect();
            assert_eq!(params.len(), 1, "{generated:?}");
            assert!(
                matches!(params[0], Node::NullableTypeAnnotation(_)),
                "{generated:?}"
            );
        },
    );
}

/// `A.B` — `GenericTypeAnnotation`'s `id` is a `QualifiedTypeIdentifier`
/// (dotted type reference), not a plain `Identifier`.
#[test]
fn qualified_type_identifier_round_trips() {
    for_each_pretty_mode(|pretty| {
        round_trip_return_flow_type("function f(): A.B {}", pretty, |_gc, ty, generated| {
            let Node::GenericTypeAnnotation(GenericTypeAnnotation { id, .. }) = ty else {
                panic!("{pretty:?}: not a GenericTypeAnnotation: {ty:?} ({generated:?})");
            };
            let Node::QualifiedTypeIdentifier(QualifiedTypeIdentifier {
                qualification,
                id,
                ..
            }) = id
            else {
                panic!("{pretty:?}: id is not a QualifiedTypeIdentifier: {id:?}");
            };
            assert!(matches!(qualification, Node::Identifier(_)), "{pretty:?}");
            assert!(matches!(id, Node::Identifier(_)), "{pretty:?}");
        });
    });
}

/// `A['b']['c']` — a left-associative chain of two `IndexedAccessType`s,
/// each keyed by a `StringLiteralTypeAnnotation`, with a bare
/// `GenericTypeAnnotation` (`A`) as the base. Also asserts the compact
/// output has *no* redundant parens around `A`.
#[test]
fn indexed_access_type_chain_round_trips() {
    for_each_pretty_mode(|pretty| {
        round_trip_return_flow_type(
            "function f(): A['b']['c'] {}",
            pretty,
            |_gc, ty, generated| {
                if pretty == Pretty::No {
                    assert_eq!(generated, "A['b']['c']", "{pretty:?}: {generated:?}");
                }
                let Node::IndexedAccessType(IndexedAccessType {
                    object_type: outer_object,
                    index_type: outer_index,
                    ..
                }) = ty
                else {
                    panic!("{pretty:?}: not an IndexedAccessType: {ty:?} ({generated:?})");
                };
                assert!(
                    matches!(outer_index, Node::StringLiteralTypeAnnotation(_)),
                    "{pretty:?}"
                );
                let Node::IndexedAccessType(IndexedAccessType {
                    object_type: inner_object,
                    index_type: inner_index,
                    ..
                }) = outer_object
                else {
                    panic!(
                        "{pretty:?}: outer object is not an IndexedAccessType: {outer_object:?}"
                    );
                };
                assert!(
                    matches!(inner_object, Node::GenericTypeAnnotation(_)),
                    "{pretty:?}"
                );
                assert!(
                    matches!(inner_index, Node::StringLiteralTypeAnnotation(_)),
                    "{pretty:?}"
                );
            },
        );
    });
}

/// `A?.['b']` — `OptionalIndexedAccessType`.
#[test]
fn optional_indexed_access_type_round_trips() {
    for_each_pretty_mode(|pretty| {
        round_trip_return_flow_type(
            "function f(): A?.['b'] {}",
            pretty,
            |_gc, ty, generated| {
                let Node::OptionalIndexedAccessType(OptionalIndexedAccessType {
                    object_type,
                    index_type,
                    optional,
                    ..
                }) = ty
                else {
                    panic!(
                        "{pretty:?}: not an OptionalIndexedAccessType: {ty:?} ({generated:?})"
                    );
                };
                assert!(
                    matches!(object_type, Node::GenericTypeAnnotation(_)),
                    "{pretty:?}"
                );
                assert!(
                    matches!(index_type, Node::StringLiteralTypeAnnotation(_)),
                    "{pretty:?}"
                );
                assert!(optional.get(), "{pretty:?}");
            },
        );
    });
}

/// `number[] | [string, boolean] | [string, ...]` — `ArrayTypeAnnotation`,
/// an exact `TupleTypeAnnotation`, and an inexact one (the trailing bare
/// `...`, `TupleTypeAnnotation::inexact`) all in one union.
#[test]
fn array_and_tuple_type_annotations_round_trip() {
    for_each_pretty_mode(|pretty| {
        round_trip_return_flow_type(
            "function f(): number[] | [string, boolean] | [string, ...] {}",
            pretty,
            |_gc, ty, generated| {
                let Node::UnionTypeAnnotation(UnionTypeAnnotation { types, .. }) = ty else {
                    panic!("{pretty:?}: not a UnionTypeAnnotation: {ty:?} ({generated:?})");
                };
                let members: Vec<&Node> = types.iter().collect();
                assert_eq!(members.len(), 3, "{pretty:?}: {generated:?}");

                let Node::ArrayTypeAnnotation(ArrayTypeAnnotation { element_type, .. }) =
                    members[0]
                else {
                    panic!(
                        "{pretty:?}: member 0 is not an ArrayTypeAnnotation: {:?}",
                        members[0]
                    );
                };
                assert!(
                    matches!(element_type, Node::NumberTypeAnnotation(_)),
                    "{pretty:?}"
                );

                let Node::TupleTypeAnnotation(TupleTypeAnnotation {
                    element_types: exact_types,
                    inexact: exact_inexact,
                    ..
                }) = members[1]
                else {
                    panic!(
                        "{pretty:?}: member 1 is not a TupleTypeAnnotation: {:?}",
                        members[1]
                    );
                };
                assert!(!exact_inexact.get(), "{pretty:?}: exact tuple");
                assert_eq!(exact_types.iter().count(), 2, "{pretty:?}");

                let Node::TupleTypeAnnotation(TupleTypeAnnotation {
                    element_types: inexact_types,
                    inexact: inexact_inexact,
                    ..
                }) = members[2]
                else {
                    panic!(
                        "{pretty:?}: member 2 is not a TupleTypeAnnotation: {:?}",
                        members[2]
                    );
                };
                assert!(inexact_inexact.get(), "{pretty:?}: inexact tuple");
                assert_eq!(
                    inexact_types.iter().count(),
                    1,
                    "{pretty:?}: {:?}",
                    inexact_types
                );
            },
        );
    });
}

/// `(?a)[]` — `ArrayTypeAnnotation{element_type: Nullable(a)}`. Regression
/// guard for a real corruption bug fixed in Task 10 review round 2: printing
/// `element_type` through a bare `gen_node` (never `print_child`) lost the
/// parens, reparsing as `NullableTypeAnnotation{type_annotation:
/// ArrayTypeAnnotation(a)}` — a different type.
#[test]
fn array_of_parenthesized_nullable_round_trips_preserving_structure() {
    for_each_pretty_mode(|pretty| {
        round_trip_return_flow_type("function f(): (?a)[] {}", pretty, |_gc, ty, generated| {
            let Node::ArrayTypeAnnotation(ArrayTypeAnnotation { element_type, .. }) = ty else {
                panic!(
                    "{pretty:?}: reparsed as {ty:?}, not an ArrayTypeAnnotation \
                         (corruption bug regressed): {generated:?}"
                );
            };
            assert!(
                matches!(*element_type, Node::NullableTypeAnnotation(_)),
                "{pretty:?}: element_type is not a NullableTypeAnnotation: \
                     {element_type:?} ({generated:?})"
            );
        });
    });
}

/// `(?a)['b']` — the same corruption-bug regression guard as above, for
/// `IndexedAccessType`.
#[test]
fn indexed_access_of_parenthesized_nullable_round_trips_preserving_structure() {
    for_each_pretty_mode(|pretty| {
        round_trip_return_flow_type(
            "function f(): (?a)['b'] {}",
            pretty,
            |_gc, ty, generated| {
                let Node::IndexedAccessType(IndexedAccessType {
                    object_type,
                    index_type,
                    ..
                }) = ty
                else {
                    panic!(
                        "{pretty:?}: reparsed as {ty:?}, not an IndexedAccessType \
                         (corruption bug regressed): {generated:?}"
                    );
                };
                assert!(
                    matches!(*object_type, Node::NullableTypeAnnotation(_)),
                    "{pretty:?}: object_type is not a NullableTypeAnnotation: \
                     {object_type:?} ({generated:?})"
                );
                assert!(
                    matches!(*index_type, Node::StringLiteralTypeAnnotation(_)),
                    "{pretty:?}: {generated:?}"
                );
            },
        );
    });
}

/// `(?a)?.['b']` — the same corruption-bug regression guard as above, for
/// `OptionalIndexedAccessType`.
#[test]
fn optional_indexed_access_of_parenthesized_nullable_round_trips_preserving_structure() {
    for_each_pretty_mode(|pretty| {
        round_trip_return_flow_type(
            "function f(): (?a)?.['b'] {}",
            pretty,
            |_gc, ty, generated| {
                let Node::OptionalIndexedAccessType(OptionalIndexedAccessType {
                    object_type,
                    index_type,
                    optional,
                    ..
                }) = ty
                else {
                    panic!(
                        "{pretty:?}: reparsed as {ty:?}, not an OptionalIndexedAccessType \
                         (corruption bug regressed): {generated:?}"
                    );
                };
                assert!(
                    matches!(*object_type, Node::NullableTypeAnnotation(_)),
                    "{pretty:?}: object_type is not a NullableTypeAnnotation: \
                     {object_type:?} ({generated:?})"
                );
                assert!(
                    matches!(*index_type, Node::StringLiteralTypeAnnotation(_)),
                    "{pretty:?}: {generated:?}"
                );
                assert!(optional.get(), "{pretty:?}: {generated:?}");
            },
        );
    });
}

/// `Array<T>['b']` — `GenericTypeAnnotation` *with* a type argument (`<T>`)
/// as the base of an `IndexedAccessType`. Confirms the `<...>` delimiter
/// doesn't change anything about `GenericTypeAnnotation`'s `PRIMARY`
/// classification being safe (Task 10 review round 3).
#[test]
fn generic_type_with_type_argument_indexed_round_trips_without_redundant_parens() {
    for_each_pretty_mode(|pretty| {
        round_trip_return_flow_type(
            "function f(): Array<T>['b'] {}",
            pretty,
            |_gc, ty, generated| {
                if pretty == Pretty::No {
                    assert_eq!(generated, "Array<T>['b']", "{pretty:?}: {generated:?}");
                }
                let Node::IndexedAccessType(IndexedAccessType {
                    object_type,
                    index_type,
                    ..
                }) = ty
                else {
                    panic!("{pretty:?}: not an IndexedAccessType: {ty:?} ({generated:?})");
                };
                let Node::GenericTypeAnnotation(GenericTypeAnnotation {
                    id,
                    type_parameters,
                    ..
                }) = object_type
                else {
                    panic!(
                        "{pretty:?}: object_type is not a GenericTypeAnnotation: \
                         {object_type:?} ({generated:?})"
                    );
                };
                assert!(matches!(id, Node::Identifier(_)), "{pretty:?}");
                assert!(
                    type_parameters.is_some(),
                    "{pretty:?}: Array<T> must keep its type argument"
                );
                assert!(
                    matches!(*index_type, Node::StringLiteralTypeAnnotation(_)),
                    "{pretty:?}"
                );
            },
        );
    });
}

/// `Array<Array<T>>['b']` — a nested generic as the base of an
/// `IndexedAccessType`. Confirms the `PRIMARY` entry doesn't just work for
/// the outer generic; pins the whole nested shape round-trips with no parens
/// anywhere.
#[test]
fn nested_generic_type_indexed_round_trips_without_redundant_parens() {
    for_each_pretty_mode(|pretty| {
        round_trip_return_flow_type(
            "function f(): Array<Array<T>>['b'] {}",
            pretty,
            |_gc, ty, generated| {
                if pretty == Pretty::No {
                    assert_eq!(
                        generated, "Array<Array<T>>['b']",
                        "{pretty:?}: {generated:?}"
                    );
                }
                let Node::IndexedAccessType(IndexedAccessType { object_type, .. }) = ty else {
                    panic!("{pretty:?}: not an IndexedAccessType: {ty:?} ({generated:?})");
                };
                let Node::GenericTypeAnnotation(GenericTypeAnnotation {
                    type_parameters, ..
                }) = object_type
                else {
                    panic!(
                        "{pretty:?}: object_type is not a GenericTypeAnnotation: \
                         {object_type:?} ({generated:?})"
                    );
                };
                let type_parameters = type_parameters.expect("outer has type parameters");
                let Node::TypeParameterInstantiation(TypeParameterInstantiation {
                    params, ..
                }) = type_parameters
                else {
                    panic!("{pretty:?}: not a TypeParameterInstantiation: {type_parameters:?}");
                };
                let params: Vec<&Node> = params.iter().collect();
                assert_eq!(params.len(), 1, "{pretty:?}: {generated:?}");
                assert!(
                    matches!(params[0], Node::GenericTypeAnnotation(_)),
                    "{pretty:?}: inner type argument must still be a GenericTypeAnnotation: \
                     {:?}",
                    params[0]
                );
            },
        );
    });
}

/// `[number, string]['x']` — a `TupleTypeAnnotation` as the base of an
/// `IndexedAccessType`, the same `PRIMARY` fix as `GenericTypeAnnotation`.
#[test]
fn tuple_type_indexed_round_trips_without_redundant_parens() {
    for_each_pretty_mode(|pretty| {
        round_trip_return_flow_type(
            "function f(): [number, string]['x'] {}",
            pretty,
            |_gc, ty, generated| {
                if pretty == Pretty::No {
                    assert_eq!(
                        generated, "[number,string]['x']",
                        "{pretty:?}: {generated:?}"
                    );
                }
                let Node::IndexedAccessType(IndexedAccessType {
                    object_type,
                    index_type,
                    ..
                }) = ty
                else {
                    panic!("{pretty:?}: not an IndexedAccessType: {ty:?} ({generated:?})");
                };
                let Node::TupleTypeAnnotation(TupleTypeAnnotation { element_types, .. }) =
                    object_type
                else {
                    panic!(
                        "{pretty:?}: object_type is not a TupleTypeAnnotation: \
                         {object_type:?} ({generated:?})"
                    );
                };
                assert_eq!(element_types.iter().count(), 2, "{pretty:?}: {generated:?}");
                assert!(
                    matches!(*index_type, Node::StringLiteralTypeAnnotation(_)),
                    "{pretty:?}"
                );
            },
        );
    });
}

/// `typeof x['y']` — a `TypeofTypeAnnotation` as the base of an
/// `IndexedAccessType`, the same `PRIMARY` fix.
#[test]
fn typeof_type_indexed_round_trips_without_redundant_parens() {
    for_each_pretty_mode(|pretty| {
        round_trip_return_flow_type(
            "function f(): typeof x['y'] {}",
            pretty,
            |_gc, ty, generated| {
                if pretty == Pretty::No {
                    assert_eq!(generated, "typeof x['y']", "{pretty:?}: {generated:?}");
                }
                let Node::IndexedAccessType(IndexedAccessType {
                    object_type,
                    index_type,
                    ..
                }) = ty
                else {
                    panic!("{pretty:?}: not an IndexedAccessType: {ty:?} ({generated:?})");
                };
                let Node::TypeofTypeAnnotation(TypeofTypeAnnotation { argument, .. }) =
                    object_type
                else {
                    panic!(
                        "{pretty:?}: object_type is not a TypeofTypeAnnotation: \
                         {object_type:?} ({generated:?})"
                    );
                };
                assert!(matches!(*argument, Node::Identifier(_)), "{pretty:?}");
                assert!(
                    matches!(*index_type, Node::StringLiteralTypeAnnotation(_)),
                    "{pretty:?}"
                );
            },
        );
    });
}

// ---------------------------------------------------------------------------
// Task 11's own coverage: Flow declarations, object types, and enums. Brief
// Step 2's six required cases, plus two regression tests for real bugs found
// and fixed in `arms/flow_decl.rs` (see that module's doc comment for the
// full evidence trail for each).
// ---------------------------------------------------------------------------

/// `declare class Base {}` then `declare class Sub extends Base {}` — a
/// `DeclareClass` with a non-empty `extends` clause (`InterfaceExtends`, the
/// shared-arm sibling of `ClassImplements`).
#[test]
fn declare_class_with_extends_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip_flow(
            "declare class Base {} declare class Sub extends Base {}",
            pretty,
        );
        reparsed.with_program(|gc, node| {
            let Node::Program(Program { body, .. }) = node else {
                panic!("{pretty:?}: root is not a Program");
            };
            let stmts: Vec<&Node> = body.iter().collect();
            assert_eq!(stmts.len(), 2, "{pretty:?}");
            let Node::DeclareClass(DeclareClass { id, extends, .. }) = stmts[1] else {
                panic!("{pretty:?}: second statement is not a DeclareClass: {:?}", stmts[1]);
            };
            assert!(matches!(id, Node::Identifier(_)), "{pretty:?}");
            let extends: Vec<&Node> = extends.iter().collect();
            assert_eq!(extends.len(), 1, "{pretty:?}");
            assert!(matches!(extends[0], Node::InterfaceExtends(_)), "{pretty:?}");
            let _ = gc;
        });
    });
}

/// `declare module "x" { declare export default class Base {} }` —
/// `DeclareModule` wrapping a `DeclareExportDeclaration` whose own `default`
/// is set, holding a `DeclareClass` as its `declaration` — `default` accepts
/// only `function`/`hook`/`component`/`class`/a bare type annotation
/// (confirmed against our own parser's `parse_declare_export_flow`,
/// `crates/parser/src/js/flow/declarations.rs:2256-2330`; `opaque
/// type`/`var`/`enum`/`interface` are not among them), so `class` doubles as
/// coverage that `declare_prefix_needed` correctly omits the nested
/// `declare ` inside `declare export default`.
#[test]
fn declare_module_with_declare_export_default_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip_flow(
            r#"declare module "x" { declare export default class Base {} }"#,
            pretty,
        );
        reparsed.with_program(|gc, node| {
            let Node::Program(Program { body, .. }) = node else {
                panic!("{pretty:?}: root is not a Program");
            };
            let stmt = body.iter().next().expect("{pretty:?}: has a statement");
            let Node::DeclareModule(DeclareModule { id, body, .. }) = stmt else {
                panic!("{pretty:?}: statement is not a DeclareModule: {stmt:?}");
            };
            assert!(matches!(id, Node::StringLiteral(_)), "{pretty:?}");
            let Node::BlockStatement(inner) = body else {
                panic!("{pretty:?}: module body is not a BlockStatement: {body:?}");
            };
            let inner_stmt = inner.body.iter().next().expect("{pretty:?}: module has a member");
            let Node::DeclareExportDeclaration(DeclareExportDeclaration {
                declaration,
                default,
                ..
            }) = inner_stmt
            else {
                panic!(
                    "{pretty:?}: module member is not a DeclareExportDeclaration: {inner_stmt:?}"
                );
            };
            assert!(default.get(), "{pretty:?}: must round-trip as `default`");
            let declaration = declaration.expect("{pretty:?}: has a declaration");
            assert!(
                matches!(declaration, Node::DeclareClass(_)),
                "{pretty:?}: declaration is not a DeclareClass: {declaration:?}"
            );
            let _ = gc;
        });
    });
}

/// `opaque type T: number = number;` — the legacy supertype bound
/// (`OpaqueType::supertype`, only reachable when neither `super`/`extends`
/// bound is present).
#[test]
fn opaque_type_with_supertype_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip_flow("opaque type T: number = number;", pretty);
        reparsed.with_program(|gc, node| {
            let Node::Program(Program { body, .. }) = node else {
                panic!("{pretty:?}: root is not a Program");
            };
            let stmt = body.iter().next().expect("{pretty:?}: has a statement");
            let Node::OpaqueType(OpaqueType {
                id,
                lower_bound,
                upper_bound,
                supertype,
                impltype,
                ..
            }) = stmt
            else {
                panic!("{pretty:?}: statement is not an OpaqueType: {stmt:?}");
            };
            assert!(matches!(id, Node::Identifier(_)), "{pretty:?}");
            assert!(lower_bound.is_none(), "{pretty:?}");
            assert!(upper_bound.is_none(), "{pretty:?}");
            assert!(
                matches!(supertype, Some(Node::NumberTypeAnnotation(_))),
                "{pretty:?}: supertype is not a NumberTypeAnnotation: {supertype:?}"
            );
            assert!(matches!(impltype, Node::NumberTypeAnnotation(_)), "{pretty:?}");
            let _ = gc;
        });
    });
}

/// `opaque type T super Empty extends Box<T> = Impl;` — the current
/// `super`/`extends` bound syntax (`OpaqueType::lower_bound`/`upper_bound`, a
/// field grown since juno was frozen — see `arms/flow_decl.rs`'s module doc
/// comment). Source shape confirmed against
/// `test/Parser/flow/type-alias.js:97`.
#[test]
fn opaque_type_with_lower_and_upper_bound_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip_flow(
            "opaque type Counter super Empty extends Box<T> = Container<T>;",
            pretty,
        );
        reparsed.with_program(|gc, node| {
            let Node::Program(Program { body, .. }) = node else {
                panic!("{pretty:?}: root is not a Program");
            };
            let stmt = body.iter().next().expect("{pretty:?}: has a statement");
            let Node::OpaqueType(OpaqueType {
                lower_bound,
                upper_bound,
                supertype,
                ..
            }) = stmt
            else {
                panic!("{pretty:?}: statement is not an OpaqueType: {stmt:?}");
            };
            assert!(
                matches!(lower_bound, Some(Node::GenericTypeAnnotation(_))),
                "{pretty:?}: lower_bound is not a GenericTypeAnnotation: {lower_bound:?}"
            );
            assert!(
                matches!(upper_bound, Some(Node::GenericTypeAnnotation(_))),
                "{pretty:?}: upper_bound is not a GenericTypeAnnotation: {upper_bound:?}"
            );
            assert!(supertype.is_none(), "{pretty:?}: bounds and legacy supertype are exclusive");
            let _ = gc;
        });
    });
}

/// An object type with an indexer, an internal slot, a call property, and a
/// spread property all in one `ObjectTypeAnnotation` — the brief's own
/// headline case for the five member kinds.
#[test]
fn object_type_with_indexer_internal_slot_call_property_and_spread_round_trips() {
    for_each_pretty_mode(|pretty| {
        round_trip_return_flow_type(
            "function f(): {[string]: number, [[Foo]]: string, (x: number): void, ...Base} {}",
            pretty,
            |_gc, ty, generated| {
                let Node::ObjectTypeAnnotation(ObjectTypeAnnotation {
                    indexers,
                    internal_slots,
                    call_properties,
                    properties,
                    ..
                }) = ty
                else {
                    panic!("{pretty:?}: not an ObjectTypeAnnotation: {ty:?} ({generated:?})");
                };
                let indexers: Vec<&Node> = indexers.iter().collect();
                assert_eq!(indexers.len(), 1, "{pretty:?}: {generated:?}");
                assert!(matches!(indexers[0], Node::ObjectTypeIndexer(_)), "{pretty:?}");

                let slots: Vec<&Node> = internal_slots.iter().collect();
                assert_eq!(slots.len(), 1, "{pretty:?}: {generated:?}");
                assert!(matches!(slots[0], Node::ObjectTypeInternalSlot(_)), "{pretty:?}");

                let calls: Vec<&Node> = call_properties.iter().collect();
                assert_eq!(calls.len(), 1, "{pretty:?}: {generated:?}");
                assert!(matches!(calls[0], Node::ObjectTypeCallProperty(_)), "{pretty:?}");

                let props: Vec<&Node> = properties.iter().collect();
                assert_eq!(props.len(), 1, "{pretty:?}: {generated:?}");
                assert!(matches!(props[0], Node::ObjectTypeSpreadProperty(_)), "{pretty:?}");
            },
        );
    });
}

/// `<+T: Base = Default>` — a variance-annotated (`Variance::Plus`) type
/// parameter with both a `bound` and a `default`, as one element of a
/// `TypeParameterDeclaration` (`InterfaceDeclaration::type_parameters`, since
/// it is the simplest real construct with a non-empty `TypeParameterDeclaration`).
#[test]
fn variance_annotated_type_parameter_with_default_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip_flow(
            "interface Box<+T: Base = Default> { x: T; }",
            pretty,
        );
        reparsed.with_program(|gc, node| {
            let Node::Program(Program { body, .. }) = node else {
                panic!("{pretty:?}: root is not a Program");
            };
            let stmt = body.iter().next().expect("{pretty:?}: has a statement");
            let Node::InterfaceDeclaration(InterfaceDeclaration { type_parameters, .. }) = stmt
            else {
                panic!("{pretty:?}: statement is not an InterfaceDeclaration: {stmt:?}");
            };
            let type_parameters = type_parameters.expect("{pretty:?}: has type parameters");
            let Node::TypeParameterDeclaration(TypeParameterDeclaration { params, .. }) =
                type_parameters
            else {
                panic!("{pretty:?}: not a TypeParameterDeclaration: {type_parameters:?}");
            };
            let params: Vec<&Node> = params.iter().collect();
            assert_eq!(params.len(), 1, "{pretty:?}");
            let Node::TypeParameter(TypeParameter {
                r#const,
                bound,
                variance,
                default,
                ..
            }) = params[0]
            else {
                panic!("{pretty:?}: not a TypeParameter: {:?}", params[0]);
            };
            assert!(!r#const.get(), "{pretty:?}: not `const`");
            let variance = variance.expect("{pretty:?}: has a variance sigil");
            let Node::Variance(Variance { kind, .. }) = variance else {
                panic!("{pretty:?}: variance is not a Variance node: {variance:?}");
            };
            assert_eq!(gc.bytes_str_lossy(kind.get()), "plus", "{pretty:?}");
            assert!(bound.is_some(), "{pretty:?}: `: Base` must round-trip");
            assert!(default.is_some(), "{pretty:?}: `= Default` must round-trip");
        });
    });
}

/// Each `enum` body kind (`EnumStringBody`/`EnumNumberBody`/
/// `EnumBooleanBody`/`EnumSymbolBody`), including a defaulted member
/// (`EnumDefaultedMember`, the symbol enum's only member shape). Four
/// separate declarations, one per body kind, checked in one test since each
/// is a single-shape assertion.
#[test]
fn each_enum_body_kind_round_trips_including_a_defaulted_member() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip_flow(
            "enum S { A = 'a', B = 'b' } \
             enum N { A = 1, B = 2 } \
             enum Bl { A = true, B = false } \
             enum Sy of symbol { A, B }",
            pretty,
        );
        reparsed.with_program(|gc, node| {
            let Node::Program(Program { body, .. }) = node else {
                panic!("{pretty:?}: root is not a Program");
            };
            let stmts: Vec<&Node> = body.iter().collect();
            assert_eq!(stmts.len(), 4, "{pretty:?}");

            let Node::EnumDeclaration(EnumDeclaration { body: s_body, .. }) = stmts[0] else {
                panic!("{pretty:?}: stmt 0 is not an EnumDeclaration: {:?}", stmts[0]);
            };
            let Node::EnumStringBody(EnumStringBody { members, .. }) = s_body else {
                panic!("{pretty:?}: not an EnumStringBody: {s_body:?}");
            };
            assert_eq!(members.iter().count(), 2, "{pretty:?}");
            assert!(matches!(members.iter().next().unwrap(), Node::EnumStringMember(_)), "{pretty:?}");

            let Node::EnumDeclaration(EnumDeclaration { body: n_body, .. }) = stmts[1] else {
                panic!("{pretty:?}: stmt 1 is not an EnumDeclaration: {:?}", stmts[1]);
            };
            let Node::EnumNumberBody(EnumNumberBody { members, .. }) = n_body else {
                panic!("{pretty:?}: not an EnumNumberBody: {n_body:?}");
            };
            assert_eq!(members.iter().count(), 2, "{pretty:?}");
            assert!(matches!(members.iter().next().unwrap(), Node::EnumNumberMember(_)), "{pretty:?}");

            let Node::EnumDeclaration(EnumDeclaration { body: b_body, .. }) = stmts[2] else {
                panic!("{pretty:?}: stmt 2 is not an EnumDeclaration: {:?}", stmts[2]);
            };
            let Node::EnumBooleanBody(EnumBooleanBody { members, .. }) = b_body else {
                panic!("{pretty:?}: not an EnumBooleanBody: {b_body:?}");
            };
            assert_eq!(members.iter().count(), 2, "{pretty:?}");
            assert!(matches!(members.iter().next().unwrap(), Node::EnumBooleanMember(_)), "{pretty:?}");

            let Node::EnumDeclaration(EnumDeclaration { body: sy_body, .. }) = stmts[3] else {
                panic!("{pretty:?}: stmt 3 is not an EnumDeclaration: {:?}", stmts[3]);
            };
            let Node::EnumSymbolBody(EnumSymbolBody { members, .. }) = sy_body else {
                panic!("{pretty:?}: not an EnumSymbolBody: {sy_body:?}");
            };
            assert_eq!(members.iter().count(), 2, "{pretty:?}");
            assert!(
                matches!(members.iter().next().unwrap(), Node::EnumDefaultedMember(_)),
                "{pretty:?}: symbol enum member must be defaulted"
            );
            let _ = gc;
        });
    });
}

// ---------------------------------------------------------------------------
// Regression tests for the two real round-trip bugs `arms/flow_decl.rs`
// fixes (see that module's doc comment, "Deviations from juno" section, for
// the full evidence trail for each).
// ---------------------------------------------------------------------------

/// `{ get foo(): number, set foo(v: number): void }` — without the fix,
/// juno's logic prints this as `{foo:() => number,foo:(v:number) => void}`
/// (dropping `kind`, so both members regenerate as `method: false, kind:
/// "init"`, plain `FunctionTypeAnnotation`-valued properties — a genuinely
/// different `ObjectTypeAnnotation`, not merely different-looking source).
#[test]
fn object_type_getter_and_setter_round_trip_preserving_kind() {
    for_each_pretty_mode(|pretty| {
        round_trip_return_flow_type(
            "function f(): {get foo(): number, set foo(v: number): void} {}",
            pretty,
            |gc, ty, generated| {
                let Node::ObjectTypeAnnotation(ObjectTypeAnnotation { properties, .. }) = ty
                else {
                    panic!("{pretty:?}: not an ObjectTypeAnnotation: {ty:?} ({generated:?})");
                };
                let props: Vec<&Node> = properties.iter().collect();
                assert_eq!(props.len(), 2, "{pretty:?}: {generated:?}");

                let Node::ObjectTypeProperty(ObjectTypeProperty { kind, method, .. }) = props[0]
                else {
                    panic!("{pretty:?}: prop 0 is not an ObjectTypeProperty: {:?}", props[0]);
                };
                assert_eq!(
                    gc.bytes_str_lossy(kind.get()),
                    "get",
                    "{pretty:?}: getter must keep kind \"get\": {generated:?}"
                );
                assert!(!method.get(), "{pretty:?}: getter is not `method`");

                let Node::ObjectTypeProperty(ObjectTypeProperty { kind, method, .. }) = props[1]
                else {
                    panic!("{pretty:?}: prop 1 is not an ObjectTypeProperty: {:?}", props[1]);
                };
                assert_eq!(
                    gc.bytes_str_lossy(kind.get()),
                    "set",
                    "{pretty:?}: setter must keep kind \"set\": {generated:?}"
                );
                assert!(!method.get(), "{pretty:?}: setter is not `method`");
            },
        );
    });
}

/// `declare var x: number;`'s `DeclareVariable` node, generated as
/// `generate()`'s own **root** (so its dispatch call receives `path: None`
/// — confirmed against `gen.rs`'s `gen_root`, `gen_js.gen_node(ctx, root,
/// None)`) — the exact shape juno's `DeclareVariable` arm gets wrong (see
/// `arms/flow_decl.rs`'s module doc comment): both `declare ` and the
/// `var`/`let`/`const` keyword must still print, not just the bare `x:
/// number`. This is deliberately *not* a `round_trip_flow`-based test: an
/// ordinary `declare var x: number;` parsed as a whole *program* has its
/// `DeclareVariable` nested one level down (`path: Some(Program-path)`,
/// which was never the buggy branch), so this passes the `DeclareVariable`
/// node itself — not the `Program` wrapping it — straight to `generate()`.
#[test]
fn declare_variable_with_no_parent_still_prints_declare_and_kind() {
    let mut parsed = parse_ok_flow("declare var x: number;");
    let mut out = Vec::new();
    parsed.with_program(|gc, node| {
        let Node::Program(Program { body, .. }) = node else {
            panic!("root is not a Program");
        };
        let stmt = body.iter().next().expect("has a statement");
        assert!(matches!(stmt, Node::DeclareVariable(_)), "not a DeclareVariable: {stmt:?}");
        generate(&mut out, gc, stmt, Opt::default()).expect("DeclareVariable generates as a root");
    });
    let js = String::from_utf8(out).expect("generator output is always valid UTF-8");
    assert!(js.starts_with("declare var "), "{js:?}");
    assert!(js.contains('x'), "{js:?}");
}

// ---------------------------------------------------------------------------
// Task 12: the 53 ES/Flow kinds juno's generator predates. One named
// round-trip test per kind (spec's Step 6) — see `arms/newer.rs`'s module
// doc comment for the derivation of every arm's syntax from the parser.
// ---------------------------------------------------------------------------

use hermes_ast::node::{
    AsConstExpression, AsExpression, ComponentDeclaration, ComponentParameter,
    ComponentTypeAnnotation, ComponentTypeParameter, ConditionalTypeAnnotation, DeclareComponent,
    DeclareNamespace, Decorator, EnumBigIntBody, EnumBigIntMember, HookDeclaration,
    HookTypeAnnotation, InferTypeAnnotation, KeyofTypeAnnotation, MatchArrayPattern,
    MatchAsPattern, MatchBindingPattern, MatchExpression, MatchExpressionCase,
    MatchInstanceObjectPattern, MatchInstancePattern, MatchLiteralPattern, MatchMemberPattern,
    MatchObjectPattern, MatchObjectPatternProperty, MatchOrPattern, MatchRestPattern,
    MatchStatement, MatchStatementCase, MatchUnaryPattern, ObjectTypeMappedTypeProperty,
    QualifiedTypeofIdentifier, RecordDeclaration, RecordDeclarationBody,
    RecordDeclarationImplements, RecordDeclarationProperty, RecordExpression,
    RecordExpressionProperties, TupleTypeLabeledElement, TupleTypeSpreadElement, TypeOperator,
    TypePredicate,
};

fn match_flags() -> ParseFlags {
    ParseFlags { parse_flow_match: true, ..Default::default() }
}

fn parse_ok_match(src: &str) -> ParsedJS {
    hermes_parser::parse(src, match_flags())
        .unwrap_or_else(|e| panic!("{src:?} must parse under -parse-flow-match: {e:?}"))
}

fn round_trip_match(src: &str, pretty: Pretty) -> ParsedJS {
    let mut parsed = parse_ok_match(src);
    let js = gen(&mut parsed, Opt { pretty, ..Opt::default() });
    hermes_parser::parse(&js, match_flags()).unwrap_or_else(|e| {
        panic!("regenerated {js:?} (from {src:?}, {pretty:?}) must parse under -parse-flow-match: {e:?}")
    })
}

fn records_flags() -> ParseFlags {
    ParseFlags { parse_flow_records: true, ..Default::default() }
}

fn parse_ok_records(src: &str) -> ParsedJS {
    hermes_parser::parse(src, records_flags())
        .unwrap_or_else(|e| panic!("{src:?} must parse under -parse-flow-records: {e:?}"))
}

fn round_trip_records(src: &str, pretty: Pretty) -> ParsedJS {
    let mut parsed = parse_ok_records(src);
    let js = gen(&mut parsed, Opt { pretty, ..Opt::default() });
    hermes_parser::parse(&js, records_flags()).unwrap_or_else(|e| {
        panic!("regenerated {js:?} (from {src:?}, {pretty:?}) must parse under -parse-flow-records: {e:?}")
    })
}

fn component_flags() -> ParseFlags {
    ParseFlags { parse_flow_component_syntax: true, ..Default::default() }
}

fn parse_ok_component(src: &str) -> ParsedJS {
    hermes_parser::parse(src, component_flags()).unwrap_or_else(|e| {
        panic!("{src:?} must parse under -Xparse-component-syntax: {e:?}")
    })
}

fn round_trip_component(src: &str, pretty: Pretty) -> ParsedJS {
    let mut parsed = parse_ok_component(src);
    let js = gen(&mut parsed, Opt { pretty, ..Opt::default() });
    hermes_parser::parse(&js, component_flags()).unwrap_or_else(|e| {
        panic!(
            "regenerated {js:?} (from {src:?}, {pretty:?}) must parse under \
             -Xparse-component-syntax: {e:?}"
        )
    })
}

// --- Step 1: ES-level kinds -------------------------------------------------

/// `StaticBlock`: plain ES2022, no Flow flag needed.
#[test]
fn static_block_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip("class C { static { x = 1; } }", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::ClassDeclaration(ClassDeclaration { body, .. }) = stmt else {
                panic!("{pretty:?}: not a ClassDeclaration: {stmt:?}");
            };
            let Node::ClassBody(ClassBody { body: members, .. }) = body else {
                panic!("{pretty:?}: class body missing");
            };
            let member = members.iter().next().expect("has a member");
            assert!(matches!(member, Node::StaticBlock(_)), "{pretty:?}: {member:?}");
        });
    });
}

/// `Decorator` on a class: `@dec class C {}`.
#[test]
fn decorator_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip("@dec class C {}", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::ClassDeclaration(ClassDeclaration { decorators, .. }) = stmt else {
                panic!("{pretty:?}: not a ClassDeclaration: {stmt:?}");
            };
            let dec = decorators.iter().next().expect("has a decorator");
            let Node::Decorator(Decorator { expression, .. }) = dec else {
                panic!("{pretty:?}: not a Decorator: {dec:?}");
            };
            assert!(matches!(expression, Node::Identifier(_)), "{pretty:?}: {expression:?}");
        });
    });
}

/// Task 7's carry-forward obligation: `func.rs`'s member-level `decorators`
/// print path (previously always `UnsupportedKind(Decorator)`, since no
/// `Decorator` arm existed) is now live — a decorated class member must
/// round-trip.
#[test]
fn decorated_class_member_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip("class C { @dec x = 1; }", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::ClassDeclaration(ClassDeclaration { body, .. }) = stmt else {
                panic!("{pretty:?}: not a ClassDeclaration: {stmt:?}");
            };
            let Node::ClassBody(ClassBody { body: members, .. }) = body else {
                panic!("{pretty:?}: class body missing");
            };
            let member = members.iter().next().expect("has a member");
            let Node::ClassProperty(hermes_ast::node::ClassProperty { decorators, .. }) = member
            else {
                panic!("{pretty:?}: not a ClassProperty: {member:?}");
            };
            assert_eq!(decorators.iter().count(), 1, "{pretty:?}: {member:?}");
        });
    });
}

/// `AsExpression`: `x as string;`.
#[test]
fn as_expression_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip_flow("x as string;", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::ExpressionStatement(hermes_ast::node::ExpressionStatement {
                expression,
                ..
            }) = stmt
            else {
                panic!("{pretty:?}: not an ExpressionStatement: {stmt:?}");
            };
            let Node::AsExpression(AsExpression { expression, type_annotation, .. }) = expression
            else {
                panic!("{pretty:?}: not an AsExpression: {expression:?}");
            };
            assert!(matches!(expression, Node::Identifier(_)), "{pretty:?}");
            assert!(matches!(type_annotation, Node::StringTypeAnnotation(_)), "{pretty:?}");
        });
    });
}

/// `AsExpression`'s `expression` needs `print_child`: `(a, b) as string`
/// must keep its parens (a bare `SequenceExpression` there would corrupt
/// the parse — `a, (b as string)` is a different tree). This is the test
/// that fails if `gen_as_expression` used a bare `gen_node` instead of
/// `print_child` for `expression`.
#[test]
fn as_expression_parenthesizes_sequence_operand() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip_flow("(a, b) as string;", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::ExpressionStatement(hermes_ast::node::ExpressionStatement {
                expression,
                ..
            }) = stmt
            else {
                panic!("{pretty:?}: not an ExpressionStatement: {stmt:?}");
            };
            let Node::AsExpression(AsExpression { expression, .. }) = expression else {
                panic!(
                    "{pretty:?}: not an AsExpression -- the (a, b) was not kept as the \
                     `as` operand: {expression:?}"
                );
            };
            assert!(
                matches!(expression, Node::SequenceExpression(_)),
                "{pretty:?}: {expression:?}"
            );
        });
    });
}

/// `AsConstExpression`: `x as const;`.
#[test]
fn as_const_expression_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip_flow("x as const;", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::ExpressionStatement(hermes_ast::node::ExpressionStatement {
                expression,
                ..
            }) = stmt
            else {
                panic!("{pretty:?}: not an ExpressionStatement: {stmt:?}");
            };
            let Node::AsConstExpression(AsConstExpression { expression, .. }) = expression else {
                panic!("{pretty:?}: not an AsConstExpression: {expression:?}");
            };
            assert!(matches!(expression, Node::Identifier(_)), "{pretty:?}");
        });
    });
}

// --- Step 2: the Flow `match` family (18 kinds) -----------------------------

/// `MatchExpression`: `match(x) { 1 => "a", _ => "b" };`.
#[test]
fn match_expression_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip_match(r#"const y = match (x) { 1 => "a", _ => "b" };"#, pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::VariableDeclaration(hermes_ast::node::VariableDeclaration {
                declarations,
                ..
            }) = stmt
            else {
                panic!("{pretty:?}: not a VariableDeclaration: {stmt:?}");
            };
            let Node::VariableDeclarator(hermes_ast::node::VariableDeclarator { init, .. }) =
                declarations.iter().next().expect("has a declarator")
            else {
                panic!("{pretty:?}: not a VariableDeclarator");
            };
            let Node::MatchExpression(MatchExpression { cases, .. }) =
                init.expect("has an initializer")
            else {
                panic!("{pretty:?}: not a MatchExpression: {init:?}");
            };
            assert_eq!(cases.iter().count(), 2, "{pretty:?}");
        });
    });
}

/// A `MatchExpression` at the start of an `ExpressionStatement` must keep
/// its parens (review-round-4 regression test). A statement beginning with
/// `match` + `(` is taken by `try_parse_match_statement_flow` as a match
/// *statement*, whose cases take block bodies — so an unparenthesized match
/// *expression* there does not merely reparse as a different node kind, it
/// **panics the parser** (`assertion failed: self.check(TokenKind::l_brace)`,
/// `crates/parser/src/js/statements.rs:1196`). Both cases below were
/// reproduced as panics before the fix.
///
/// This is the same statement-start hazard `FunctionExpression`/
/// `ClassExpression`/`ObjectExpression` already guard, so the fix is an
/// entry in `need_parens`'s `ExpressionStatement` branch and the parens go
/// around the *whole* statement expression (hence `(match (x) {…}.foo);`,
/// not `(match (x) {…}).foo;`) — verified to reparse identically for every
/// tail shape.
#[test]
fn match_expression_at_statement_start_keeps_its_parens() {
    for_each_pretty_mode(|pretty| {
        // Bare: the whole statement is the match expression.
        let mut reparsed = round_trip_match("(match (x) { _ => 1 });", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::ExpressionStatement(ExpressionStatement { expression, .. }) = stmt else {
                panic!("{pretty:?}: not an ExpressionStatement: {stmt:?}");
            };
            assert!(
                matches!(expression, Node::MatchExpression(_)),
                "{pretty:?}: must stay a MatchExpression in an ExpressionStatement, not flip \
                 to a MatchStatement: {expression:?}"
            );
        });
        // With a member tail: `root_starts_with`'s left-spine walk must find
        // the `MatchExpression` through the `MemberExpression`.
        let mut reparsed = round_trip_match("(match (x) { _ => 1 }).foo;", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::ExpressionStatement(ExpressionStatement { expression, .. }) = stmt else {
                panic!("{pretty:?}: not an ExpressionStatement: {stmt:?}");
            };
            let Node::MemberExpression(MemberExpression { object, .. }) = expression else {
                panic!("{pretty:?}: not a MemberExpression: {expression:?}");
            };
            assert!(
                matches!(object, Node::MatchExpression(_)),
                "{pretty:?}: object must stay a MatchExpression: {object:?}"
            );
        });
        // A call tail, a binary tail and a conditional tail exercise the
        // other three `expr_starts_with` spine arms. Each asserts both the
        // parent kind and that the `MatchExpression` survived underneath it.
        let mut reparsed = round_trip_match("(match (x) { _ => 1 })();", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::ExpressionStatement(ExpressionStatement { expression, .. }) = stmt else {
                panic!("{pretty:?}: not an ExpressionStatement: {stmt:?}");
            };
            let Node::CallExpression(hermes_ast::node::CallExpression { callee, .. }) = expression
            else {
                panic!("{pretty:?}: not a CallExpression: {expression:?}");
            };
            assert!(matches!(callee, Node::MatchExpression(_)), "{pretty:?}: {callee:?}");
        });
        let mut reparsed = round_trip_match("(match (x) { _ => 1 }) + 1;", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::ExpressionStatement(ExpressionStatement { expression, .. }) = stmt else {
                panic!("{pretty:?}: not an ExpressionStatement: {stmt:?}");
            };
            let Node::BinaryExpression(BinaryExpression { left, .. }) = expression else {
                panic!("{pretty:?}: not a BinaryExpression: {expression:?}");
            };
            assert!(matches!(left, Node::MatchExpression(_)), "{pretty:?}: {left:?}");
        });
        let mut reparsed = round_trip_match("(match (x) { _ => 1 }) ? a : b;", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::ExpressionStatement(ExpressionStatement { expression, .. }) = stmt else {
                panic!("{pretty:?}: not an ExpressionStatement: {stmt:?}");
            };
            let Node::ConditionalExpression(hermes_ast::node::ConditionalExpression {
                test, ..
            }) = expression
            else {
                panic!("{pretty:?}: not a ConditionalExpression: {expression:?}");
            };
            assert!(matches!(test, Node::MatchExpression(_)), "{pretty:?}: {test:?}");
        });
        // A match expression NOT at statement start must not gain parens —
        // every non-statement-start position parses bare, including with a
        // postfix tail.
        let mut parsed = parse_ok_match("x = match (x) { _ => 1 }.foo;");
        let js = gen(&mut parsed, Opt { pretty, ..Opt::default() });
        assert!(
            !js.contains("(match"),
            "{pretty:?}: a match expression off statement start must stay bare: {js:?}"
        );
    });
}

/// `MatchStatement`: `match(x) { 1 => { y(); } _ => { z(); } }` — no comma
/// required between statement cases.
#[test]
fn match_statement_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed =
            round_trip_match("match (x) { 1 => { y(); } _ => { z(); } }", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::MatchStatement(MatchStatement { cases, .. }) = stmt else {
                panic!("{pretty:?}: not a MatchStatement: {stmt:?}");
            };
            assert_eq!(cases.iter().count(), 2, "{pretty:?}");
        });
    });
}

/// `MatchExpressionCase`: `pattern if (guard) => body`.
#[test]
fn match_expression_case_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed =
            round_trip_match("const y = match (x) { n if (n > 0) => 1 };", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::VariableDeclaration(hermes_ast::node::VariableDeclaration {
                declarations,
                ..
            }) = stmt
            else {
                panic!("{pretty:?}: not a VariableDeclaration");
            };
            let Node::VariableDeclarator(hermes_ast::node::VariableDeclarator { init, .. }) =
                declarations.iter().next().expect("has a declarator")
            else {
                panic!("{pretty:?}: not a VariableDeclarator");
            };
            let Node::MatchExpression(MatchExpression { cases, .. }) =
                init.expect("has an initializer")
            else {
                panic!("{pretty:?}: not a MatchExpression");
            };
            let Node::MatchExpressionCase(MatchExpressionCase { guard, .. }) =
                cases.iter().next().expect("has a case")
            else {
                panic!("{pretty:?}: not a MatchExpressionCase");
            };
            assert!(guard.is_some(), "{pretty:?}: guard must round-trip");
        });
    });
}

/// `MatchStatementCase`: `pattern if (guard) => { body }`.
#[test]
fn match_statement_case_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip_match("match (x) { n if (n > 0) => { y(); } }", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::MatchStatement(MatchStatement { cases, .. }) = stmt else {
                panic!("{pretty:?}: not a MatchStatement: {stmt:?}");
            };
            let Node::MatchStatementCase(MatchStatementCase { guard, body, .. }) =
                cases.iter().next().expect("has a case")
            else {
                panic!("{pretty:?}: not a MatchStatementCase");
            };
            assert!(guard.is_some(), "{pretty:?}: guard must round-trip");
            assert!(matches!(body, Node::BlockStatement(_)), "{pretty:?}");
        });
    });
}

/// Parse/round-trip `const y = match (x) { <pattern_src> => 1 };` as a
/// match EXPRESSION (whose case body is a bare expression, unlike a match
/// *statement* case's `{ ... }` block), then hand the first case's
/// `pattern` node to `check`. Shared by every simple single-pattern test
/// below.
fn with_match_expr_pattern<R>(
    pattern_src: &str,
    pretty: Pretty,
    check: impl for<'gc> FnOnce(&'gc GCLock<'static, '_>, &'gc Node<'gc>) -> R,
) -> R {
    let src = format!("const y = match (x) {{ {pattern_src} => 1 }};");
    let mut reparsed = round_trip_match(&src, pretty);
    with_first_stmt(&mut reparsed, |gc, stmt| {
        let Node::VariableDeclaration(hermes_ast::node::VariableDeclaration {
            declarations, ..
        }) = stmt
        else {
            panic!("{pretty:?}: not a VariableDeclaration: {stmt:?} ({src:?})");
        };
        let Node::VariableDeclarator(hermes_ast::node::VariableDeclarator { init, .. }) =
            declarations.iter().next().expect("has a declarator")
        else {
            panic!("{pretty:?}: not a VariableDeclarator ({src:?})");
        };
        let Node::MatchExpression(MatchExpression { cases, .. }) =
            init.expect("has an initializer")
        else {
            panic!("{pretty:?}: not a MatchExpression: {init:?} ({src:?})");
        };
        let Node::MatchExpressionCase(MatchExpressionCase { pattern, .. }) =
            cases.iter().next().expect("has a case")
        else {
            panic!("{pretty:?}: not a MatchExpressionCase ({src:?})");
        };
        check(gc, pattern)
    })
}

/// Generate `const y = match (x) { <pattern_src> => 1 };` under `pretty`
/// and return just the pattern's own printed text — the substring between
/// the fixed `"const y = match (x) {"` case-list prefix and the `" => 1"`
/// that follows it. Used by the review-round-3 tests below, which assert
/// on the *absence* of a redundant paren pair (a `contains` check cannot
/// tell "unparenthesized" from "parenthesized", since the latter's text is
/// a superstring). Panics if the generated text doesn't have the expected
/// shape.
fn match_expr_pattern_text(pattern_src: &str, pretty: Pretty) -> String {
    let src = format!("const y = match (x) {{ {pattern_src} => 1 }};");
    let mut parsed = parse_ok_match(&src);
    let js = gen(&mut parsed, Opt { pretty, ..Opt::default() });
    let (prefix, suffix) = match pretty {
        Pretty::Yes => ("const y = match (x) {\n  ", " => 1\n};\n"),
        Pretty::No => ("const y=match(x){", "=>1};\n"),
    };
    js.strip_prefix(prefix)
        .and_then(|s| s.strip_suffix(suffix))
        .unwrap_or_else(|| {
            panic!("generated {js:?} ({pretty:?}) doesn't have the expected match-case shape")
        })
        .to_string()
}

/// `MatchAsPattern`'s `pattern` needs `print_child`, but with a *different*
/// answer than `MatchOrPattern`'s element list gives for the same two child
/// kinds (review-round-3 regression test; round 2's re-audit asserted this
/// field was safe as a plain `gen_node`, which was wrong):
///
/// - A **nested `MatchAsPattern`** must be re-wrapped. It can only reach
///   this field through an explicit `( MatchPattern )` group, and printed
///   bare it does not merely change the tree — `(a as y) as z` becomes
///   `a as y as z`, which fails to reparse at all (`'=>' expected after
///   match pattern`), since `parse_match_pattern_flow`'s `as` branch runs
///   once and takes a binding target, never another pattern.
/// - A **`MatchOrPattern` must NOT be wrapped**: the `|`-loop runs before
///   the `as` check inside one `parse_match_pattern_flow` call, so
///   `a | b as z` already parses to `MatchAsPattern(MatchOrPattern, z)`.
///   The naive fix (reusing round 2's `ALWAYS_PAREN` classification for
///   both kinds) regresses this into a redundant `(a | b) as z`.
#[test]
fn match_as_pattern_parenthesizes_nested_as_pattern_but_not_or_pattern() {
    for_each_pretty_mode(|pretty| {
        // The corruption: a nested `MatchAsPattern` must survive as one.
        with_match_expr_pattern("(a as y) as z", pretty, |_gc, pattern| {
            let Node::MatchAsPattern(MatchAsPattern { pattern, target, .. }) = pattern else {
                panic!("{pretty:?}: not a MatchAsPattern: {pattern:?}");
            };
            assert!(
                matches!(target, Node::Identifier(_)),
                "{pretty:?}: outer target must stay `z`: {target:?}"
            );
            let Node::MatchAsPattern(MatchAsPattern { pattern: inner, .. }) = pattern else {
                panic!(
                    "{pretty:?}: inner pattern must stay a MatchAsPattern, not be flattened: \
                     {pattern:?}"
                );
            };
            assert!(
                matches!(inner, Node::MatchIdentifierPattern(_)),
                "{pretty:?}: innermost pattern must stay `a`: {inner:?}"
            );
        });
        // ... and it must actually be spelled with parens.
        assert_eq!(
            match_expr_pattern_text("(a as y) as z", pretty),
            match pretty {
                Pretty::Yes => "(a as y) as z",
                Pretty::No => "(a as y) as z",
            },
            "{pretty:?}: a nested MatchAsPattern must be re-wrapped"
        );
        // The over-wrapping regression: a `MatchOrPattern` here is legal
        // bare, and must not gain parens.
        assert_eq!(
            match_expr_pattern_text("a | b as z", pretty),
            match pretty {
                Pretty::Yes => "a | b as z",
                Pretty::No => "a|b as z",
            },
            "{pretty:?}: a MatchOrPattern in MatchAsPattern's `pattern` must NOT gain parens"
        );
        // Same tree either way — the source parens in `(a | b) as z` are
        // pure decoration, so dropping them is correct, not lossy.
        with_match_expr_pattern("(a | b) as z", pretty, |_gc, pattern| {
            let Node::MatchAsPattern(MatchAsPattern { pattern, .. }) = pattern else {
                panic!("{pretty:?}: not a MatchAsPattern: {pattern:?}");
            };
            let Node::MatchOrPattern(MatchOrPattern { patterns, .. }) = pattern else {
                panic!("{pretty:?}: pattern must stay a MatchOrPattern: {pattern:?}");
            };
            assert_eq!(patterns.iter().count(), 2, "{pretty:?}");
        });
        // Both rules at once: the or-pattern stays bare, its As element
        // gets wrapped by `MatchOrPattern`'s own (round-2) `print_child`.
        with_match_expr_pattern("((a as y) | b) as z", pretty, |_gc, pattern| {
            let Node::MatchAsPattern(MatchAsPattern { pattern, .. }) = pattern else {
                panic!("{pretty:?}: not a MatchAsPattern: {pattern:?}");
            };
            let Node::MatchOrPattern(MatchOrPattern { patterns, .. }) = pattern else {
                panic!("{pretty:?}: pattern must stay a MatchOrPattern: {pattern:?}");
            };
            let mut it = patterns.iter();
            assert!(
                matches!(it.next().expect("element 0"), Node::MatchAsPattern(_)),
                "{pretty:?}: element 0 must stay a MatchAsPattern"
            );
            assert_eq!(patterns.iter().count(), 2, "{pretty:?}");
        });
        // The same defect reached through a full-tier position: an array
        // element is parsed by the *full* `parse_match_pattern_flow`, so
        // the element itself needs no parens, but the As-inside-As inside
        // it still does.
        with_match_expr_pattern("[(a as y) as z]", pretty, |_gc, pattern| {
            let Node::MatchArrayPattern(MatchArrayPattern { elements, .. }) = pattern else {
                panic!("{pretty:?}: not a MatchArrayPattern: {pattern:?}");
            };
            let Node::MatchAsPattern(MatchAsPattern { pattern, .. }) =
                elements.iter().next().expect("element 0")
            else {
                panic!("{pretty:?}: element 0 is not a MatchAsPattern");
            };
            assert!(
                matches!(pattern, Node::MatchAsPattern(_)),
                "{pretty:?}: inner pattern must stay a MatchAsPattern: {pattern:?}"
            );
        });
    });
}

/// `MatchArrayPattern`: `[a, b, ...const rest]`.
#[test]
fn match_array_pattern_round_trips() {
    for_each_pretty_mode(|pretty| {
        with_match_expr_pattern("[a, b, ...const rest]", pretty, |_gc, pattern| {
            let Node::MatchArrayPattern(MatchArrayPattern { elements, rest, .. }) = pattern
            else {
                panic!("{pretty:?}: not a MatchArrayPattern: {pattern:?}");
            };
            assert_eq!(elements.iter().count(), 2, "{pretty:?}");
            assert!(rest.is_some(), "{pretty:?}");
        });
    });
}

/// `MatchAsPattern`: `1 as x`.
#[test]
fn match_as_pattern_round_trips() {
    for_each_pretty_mode(|pretty| {
        with_match_expr_pattern("1 as y", pretty, |_gc, pattern| {
            let Node::MatchAsPattern(MatchAsPattern { pattern, target, .. }) = pattern else {
                panic!("{pretty:?}: not a MatchAsPattern: {pattern:?}");
            };
            assert!(matches!(pattern, Node::MatchLiteralPattern(_)), "{pretty:?}");
            assert!(matches!(target, Node::Identifier(_)), "{pretty:?}");
        });
    });
}

/// `MatchBindingPattern`: `const x`.
#[test]
fn match_binding_pattern_round_trips() {
    for_each_pretty_mode(|pretty| {
        with_match_expr_pattern("const y", pretty, |gc, pattern| {
            let Node::MatchBindingPattern(MatchBindingPattern { kind, .. }) = pattern else {
                panic!("{pretty:?}: not a MatchBindingPattern: {pattern:?}");
            };
            assert_eq!(gc.bytes_str_lossy(kind.get()), "const", "{pretty:?}");
        });
    });
}

/// `MatchIdentifierPattern`: a bare identifier pattern.
#[test]
fn match_identifier_pattern_round_trips() {
    for_each_pretty_mode(|pretty| {
        with_match_expr_pattern("y", pretty, |_gc, pattern| {
            assert!(matches!(pattern, Node::MatchIdentifierPattern(_)), "{pretty:?}: {pattern:?}");
        });
    });
}

/// `MatchInstanceObjectPattern` / `MatchInstancePattern`: `Point { x: a, y:
/// b }`.
#[test]
fn match_instance_object_pattern_round_trips() {
    for_each_pretty_mode(|pretty| {
        with_match_expr_pattern("Point { x: a, y: b }", pretty, |_gc, pattern| {
            let Node::MatchInstancePattern(MatchInstancePattern { properties, .. }) = pattern
            else {
                panic!("{pretty:?}: not a MatchInstancePattern: {pattern:?}");
            };
            let Node::MatchInstanceObjectPattern(MatchInstanceObjectPattern {
                properties, ..
            }) = properties
            else {
                panic!("{pretty:?}: not a MatchInstanceObjectPattern: {properties:?}");
            };
            assert_eq!(properties.iter().count(), 2, "{pretty:?}");
        });
    });
}

/// `MatchInstancePattern`: `targetConstructor { ... }`, `targetConstructor`
/// itself a `MatchMemberPattern` chain (`a.Point { x: b }`), exercising the
/// `Identifier | MatchMemberPattern` union `target_constructor` allows.
#[test]
fn match_instance_pattern_round_trips() {
    for_each_pretty_mode(|pretty| {
        with_match_expr_pattern("a.Point { x: b }", pretty, |_gc, pattern| {
            let Node::MatchInstancePattern(MatchInstancePattern { target_constructor, .. }) =
                pattern
            else {
                panic!("{pretty:?}: not a MatchInstancePattern: {pattern:?}");
            };
            assert!(
                matches!(target_constructor, Node::MatchMemberPattern(_)),
                "{pretty:?}: {target_constructor:?}"
            );
        });
    });
}

/// `MatchLiteralPattern`: a bare `42`.
#[test]
fn match_literal_pattern_round_trips() {
    for_each_pretty_mode(|pretty| {
        with_match_expr_pattern("42", pretty, |_gc, pattern| {
            let Node::MatchLiteralPattern(MatchLiteralPattern { literal, .. }) = pattern else {
                panic!("{pretty:?}: not a MatchLiteralPattern: {pattern:?}");
            };
            assert!(matches!(literal, Node::NumericLiteral(_)), "{pretty:?}");
        });
    });
}

/// `MatchMemberPattern`: both the dot (`a.b`, property an `Identifier`) and
/// bracket (`a[0]`, property a `NumericLiteral`) spellings — the two
/// shapes [`GenJS::gen_match_member_pattern`] must distinguish purely from
/// `property`'s own node kind (no `computed` flag exists on this kind).
#[test]
fn match_member_pattern_round_trips() {
    for_each_pretty_mode(|pretty| {
        with_match_expr_pattern("a.b", pretty, |_gc, pattern| {
            let Node::MatchMemberPattern(MatchMemberPattern { property, .. }) = pattern else {
                panic!("{pretty:?}: not a MatchMemberPattern: {pattern:?}");
            };
            assert!(matches!(property, Node::Identifier(_)), "{pretty:?}: {property:?}");
        });
        with_match_expr_pattern("a[0]", pretty, |_gc, pattern| {
            let Node::MatchMemberPattern(MatchMemberPattern { property, .. }) = pattern else {
                panic!("{pretty:?}: not a MatchMemberPattern: {pattern:?}");
            };
            assert!(matches!(property, Node::NumericLiteral(_)), "{pretty:?}: {property:?}");
        });
    });
}

/// `MatchObjectPattern`: `{ a: 1, ...const rest }`.
#[test]
fn match_object_pattern_round_trips() {
    for_each_pretty_mode(|pretty| {
        with_match_expr_pattern("{ a: 1, ...const rest }", pretty, |_gc, pattern| {
            let Node::MatchObjectPattern(MatchObjectPattern { properties, rest, .. }) = pattern
            else {
                panic!("{pretty:?}: not a MatchObjectPattern: {pattern:?}");
            };
            assert_eq!(properties.iter().count(), 1, "{pretty:?}");
            assert!(rest.is_some(), "{pretty:?}");
        });
    });
}

/// `MatchObjectPatternProperty`: both the normal `key: pattern` and
/// `shorthand` (`const x`) forms.
#[test]
fn match_object_pattern_property_round_trips() {
    for_each_pretty_mode(|pretty| {
        with_match_expr_pattern("{ a: 1, const b }", pretty, |_gc, pattern| {
            let Node::MatchObjectPattern(MatchObjectPattern { properties, .. }) = pattern else {
                panic!("{pretty:?}: not a MatchObjectPattern: {pattern:?}");
            };
            let mut it = properties.iter();
            let Node::MatchObjectPatternProperty(MatchObjectPatternProperty {
                shorthand: sh0,
                ..
            }) = it.next().expect("prop 0")
            else {
                panic!("{pretty:?}: prop 0 not a MatchObjectPatternProperty");
            };
            assert!(!sh0.get(), "{pretty:?}: prop 0 must not be shorthand");
            let Node::MatchObjectPatternProperty(MatchObjectPatternProperty {
                shorthand: sh1,
                ..
            }) = it.next().expect("prop 1")
            else {
                panic!("{pretty:?}: prop 1 not a MatchObjectPatternProperty");
            };
            assert!(sh1.get(), "{pretty:?}: prop 1 must be shorthand");
        });
    });
}

/// `MatchOrPattern`: `1 | 2 | 3`.
#[test]
fn match_or_pattern_round_trips() {
    for_each_pretty_mode(|pretty| {
        with_match_expr_pattern("1 | 2 | 3", pretty, |_gc, pattern| {
            let Node::MatchOrPattern(MatchOrPattern { patterns, .. }) = pattern else {
                panic!("{pretty:?}: not a MatchOrPattern: {pattern:?}");
            };
            assert_eq!(patterns.iter().count(), 3, "{pretty:?}");
        });
    });
}

/// `MatchOrPattern`'s elements need parens exactly when the element is
/// itself a `MatchAsPattern`/`MatchOrPattern` — reachable there only through
/// an explicit `( MatchPattern )` group (review-round-2 regression test;
/// the module doc comment previously claimed this was structurally
/// impossible, which was wrong). Dropping the parens around a
/// `MatchAsPattern` element makes the regenerated source **fail to
/// reparse** (`a as x | b` is not valid match-pattern syntax: `as` only
/// takes a binding target, not a full pattern); dropping them around a
/// nested `MatchOrPattern` element silently flattens two elements into
/// three, a different tree.
#[test]
fn match_or_pattern_parenthesizes_as_pattern_and_nested_or_pattern_elements() {
    for_each_pretty_mode(|pretty| {
        with_match_expr_pattern("(a as x) | b", pretty, |_gc, pattern| {
            let Node::MatchOrPattern(MatchOrPattern { patterns, .. }) = pattern else {
                panic!("{pretty:?}: not a MatchOrPattern: {pattern:?}");
            };
            let mut it = patterns.iter();
            let first = it.next().expect("element 0");
            assert!(
                matches!(first, Node::MatchAsPattern(_)),
                "{pretty:?}: element 0 must stay a MatchAsPattern: {first:?}"
            );
            assert_eq!(patterns.iter().count(), 2, "{pretty:?}");
        });
        with_match_expr_pattern("(a | b) | c", pretty, |_gc, pattern| {
            let Node::MatchOrPattern(MatchOrPattern { patterns, .. }) = pattern else {
                panic!("{pretty:?}: not a MatchOrPattern: {pattern:?}");
            };
            assert_eq!(
                patterns.iter().count(),
                2,
                "{pretty:?}: must stay nested (2 elements: the inner MatchOrPattern and `c`), \
                 not flatten to 3"
            );
            let first = patterns.iter().next().expect("element 0");
            assert!(
                matches!(first, Node::MatchOrPattern(_)),
                "{pretty:?}: element 0 must stay a nested MatchOrPattern: {first:?}"
            );
        });
    });
}

/// `MatchRestPattern`: both the bound (`...const rest`) and bare (`...`)
/// forms, inside an array pattern.
#[test]
fn match_rest_pattern_round_trips() {
    for_each_pretty_mode(|pretty| {
        with_match_expr_pattern("[...const rest]", pretty, |_gc, pattern| {
            let Node::MatchArrayPattern(MatchArrayPattern { rest, .. }) = pattern else {
                panic!("{pretty:?}: not a MatchArrayPattern: {pattern:?}");
            };
            let Node::MatchRestPattern(MatchRestPattern { argument, .. }) =
                rest.expect("has a rest")
            else {
                panic!("{pretty:?}: rest is not a MatchRestPattern");
            };
            assert!(argument.is_some(), "{pretty:?}: bound rest must keep its argument");
        });
        with_match_expr_pattern("[...]", pretty, |_gc, pattern| {
            let Node::MatchArrayPattern(MatchArrayPattern { rest, .. }) = pattern else {
                panic!("{pretty:?}: not a MatchArrayPattern: {pattern:?}");
            };
            let Node::MatchRestPattern(MatchRestPattern { argument, .. }) =
                rest.expect("has a rest")
            else {
                panic!("{pretty:?}: rest is not a MatchRestPattern");
            };
            assert!(argument.is_none(), "{pretty:?}: bare rest must stay bare");
        });
    });
}

/// `MatchUnaryPattern`: `-5`.
#[test]
fn match_unary_pattern_round_trips() {
    for_each_pretty_mode(|pretty| {
        with_match_expr_pattern("-5", pretty, |gc, pattern| {
            let Node::MatchUnaryPattern(MatchUnaryPattern { operator, argument, .. }) = pattern
            else {
                panic!("{pretty:?}: not a MatchUnaryPattern: {pattern:?}");
            };
            assert_eq!(gc.bytes_str_lossy(operator.get()), "-", "{pretty:?}");
            assert!(matches!(argument, Node::NumericLiteral(_)), "{pretty:?}");
        });
    });
}

/// `MatchWildcardPattern`: the bare `_`.
#[test]
fn match_wildcard_pattern_round_trips() {
    for_each_pretty_mode(|pretty| {
        with_match_expr_pattern("_", pretty, |_gc, pattern| {
            assert!(matches!(pattern, Node::MatchWildcardPattern(_)), "{pretty:?}: {pattern:?}");
        });
    });
}

// --- Step 3: the Flow `record` family (7 kinds) -----------------------------

/// `RecordDeclaration`: `record Point<T> implements A, B { ... }`.
#[test]
fn record_declaration_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip_records(
            "record Point<T> implements A, B { x: number, }",
            pretty,
        );
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::RecordDeclaration(RecordDeclaration { type_parameters, implements, .. }) =
                stmt
            else {
                panic!("{pretty:?}: not a RecordDeclaration: {stmt:?}");
            };
            assert!(type_parameters.is_some(), "{pretty:?}");
            assert_eq!(implements.iter().count(), 2, "{pretty:?}");
        });
    });
}

/// `RecordDeclarationBody`: a property (mandatory trailing `,`) followed by
/// a method (no trailing separator at all) — the two different
/// end-of-element rules [`GenJS::gen_record_declaration_body`] must apply
/// based on element kind, not list position.
#[test]
fn record_declaration_body_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed =
            round_trip_records("record R { x: number, m(): void {} }", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::RecordDeclaration(RecordDeclaration { body, .. }) = stmt else {
                panic!("{pretty:?}: not a RecordDeclaration: {stmt:?}");
            };
            let Node::RecordDeclarationBody(RecordDeclarationBody { elements, .. }) = body else {
                panic!("{pretty:?}: not a RecordDeclarationBody: {body:?}");
            };
            let mut it = elements.iter();
            assert!(
                matches!(it.next(), Some(Node::RecordDeclarationProperty(_))),
                "{pretty:?}"
            );
            assert!(matches!(it.next(), Some(Node::MethodDefinition(_))), "{pretty:?}");
        });
    });
}

/// `RecordDeclarationImplements`: `A<T>` (one `implements` entry, with type
/// arguments).
#[test]
fn record_declaration_implements_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip_records("record R implements A<number> { }", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::RecordDeclaration(RecordDeclaration { implements, .. }) = stmt else {
                panic!("{pretty:?}: not a RecordDeclaration: {stmt:?}");
            };
            let Node::RecordDeclarationImplements(RecordDeclarationImplements {
                type_arguments,
                ..
            }) = implements.iter().next().expect("has an implements entry")
            else {
                panic!("{pretty:?}: not a RecordDeclarationImplements");
            };
            assert!(type_arguments.is_some(), "{pretty:?}");
        });
    });
}

/// `RecordDeclarationProperty`: `key: type = default_value`.
#[test]
fn record_declaration_property_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip_records("record R { x: number = 1, }", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::RecordDeclaration(RecordDeclaration { body, .. }) = stmt else {
                panic!("{pretty:?}: not a RecordDeclaration: {stmt:?}");
            };
            let Node::RecordDeclarationBody(RecordDeclarationBody { elements, .. }) = body else {
                panic!("{pretty:?}: not a RecordDeclarationBody: {body:?}");
            };
            let Node::RecordDeclarationProperty(RecordDeclarationProperty {
                default_value, ..
            }) = elements.iter().next().expect("has a property")
            else {
                panic!("{pretty:?}: not a RecordDeclarationProperty");
            };
            assert!(default_value.is_some(), "{pretty:?}");
        });
    });
}

/// `RecordDeclarationStaticProperty`: `static key: type = value`
/// (initializer mandatory).
#[test]
fn record_declaration_static_property_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip_records("record R { static x: number = 1, }", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::RecordDeclaration(RecordDeclaration { body, .. }) = stmt else {
                panic!("{pretty:?}: not a RecordDeclaration: {stmt:?}");
            };
            let Node::RecordDeclarationBody(RecordDeclarationBody { elements, .. }) = body else {
                panic!("{pretty:?}: not a RecordDeclarationBody: {body:?}");
            };
            assert!(
                matches!(
                    elements.iter().next(),
                    Some(Node::RecordDeclarationStaticProperty(_))
                ),
                "{pretty:?}"
            );
        });
    });
}

/// `RecordExpression`: `Point { x: 1, y: 2 }` — the record-ness comes from
/// `Point` starting uppercase, no `record` keyword at expression position.
#[test]
fn record_expression_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip_records("const p = Point { x: 1, y: 2 };", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::VariableDeclaration(hermes_ast::node::VariableDeclaration {
                declarations,
                ..
            }) = stmt
            else {
                panic!("{pretty:?}: not a VariableDeclaration: {stmt:?}");
            };
            let Node::VariableDeclarator(hermes_ast::node::VariableDeclarator { init, .. }) =
                declarations.iter().next().expect("has a declarator")
            else {
                panic!("{pretty:?}: not a VariableDeclarator");
            };
            let Node::RecordExpression(RecordExpression { record_constructor, .. }) =
                init.expect("has an initializer")
            else {
                panic!("{pretty:?}: not a RecordExpression: {init:?}");
            };
            assert!(matches!(record_constructor, Node::Identifier(_)), "{pretty:?}");
        });
    });
}

/// A `RecordExpression` under any postfix operator must keep its parens
/// (review-round-4 regression test). `parseLeftHandSideExpressionTail`
/// builds the record expression in a trailing `else if` and returns
/// immediately, never looping back into the member-select tail
/// (`lib/Parser/JSParserImpl.cpp:4026-4089`, ported at
/// `crates/parser/src/js/expressions.rs:2752`), so `R {p: 1}.foo`,
/// `R {p: 1}()`, `R {p: 1}[0]` and ``R {p: 1}`t` `` fail to parse
/// *anywhere* — while the parser happily *builds*
/// `MemberExpression{object: RecordExpression}` and friends from the
/// parenthesized source. Before the fix, `RecordExpression` was classified
/// `PRIMARY` (above `MEMBER`) and `(R {p: 1}).foo;` regenerated as the
/// unparseable `R {p: 1}.foo;`.
///
/// Note this is a *precedence* fix, not a statement-start one: see
/// `record_expression_at_statement_start_needs_no_parens` below.
#[test]
fn record_expression_under_postfix_operators_keeps_its_parens() {
    const DECL: &str = "record R { p: number }\n";
    for_each_pretty_mode(|pretty| {
        // Member select.
        let src = format!("{DECL}(R {{p: 1}}).foo;");
        let mut reparsed = round_trip_records(&src, pretty);
        reparsed.with_program(|_gc, node| {
            let stmt = second_stmt(node, pretty);
            let Node::ExpressionStatement(ExpressionStatement { expression, .. }) = stmt else {
                panic!("{pretty:?}: not an ExpressionStatement: {stmt:?}");
            };
            let Node::MemberExpression(MemberExpression { object, .. }) = expression else {
                panic!("{pretty:?}: not a MemberExpression: {expression:?}");
            };
            assert!(
                matches!(object, Node::RecordExpression(_)),
                "{pretty:?}: object must stay a RecordExpression: {object:?}"
            );
        });
        // Call.
        let src = format!("{DECL}(R {{p: 1}})();");
        let mut reparsed = round_trip_records(&src, pretty);
        reparsed.with_program(|_gc, node| {
            let stmt = second_stmt(node, pretty);
            let Node::ExpressionStatement(ExpressionStatement { expression, .. }) = stmt else {
                panic!("{pretty:?}: not an ExpressionStatement: {stmt:?}");
            };
            let Node::CallExpression(hermes_ast::node::CallExpression { callee, .. }) = expression
            else {
                panic!("{pretty:?}: not a CallExpression: {expression:?}");
            };
            assert!(
                matches!(callee, Node::RecordExpression(_)),
                "{pretty:?}: callee must stay a RecordExpression: {callee:?}"
            );
        });
        // Computed member.
        let src = format!("{DECL}(R {{p: 1}})[0];");
        let mut reparsed = round_trip_records(&src, pretty);
        reparsed.with_program(|_gc, node| {
            let stmt = second_stmt(node, pretty);
            let Node::ExpressionStatement(ExpressionStatement { expression, .. }) = stmt else {
                panic!("{pretty:?}: not an ExpressionStatement: {stmt:?}");
            };
            let Node::MemberExpression(MemberExpression { object, .. }) = expression else {
                panic!("{pretty:?}: not a MemberExpression: {expression:?}");
            };
            assert!(
                matches!(object, Node::RecordExpression(_)),
                "{pretty:?}: object must stay a RecordExpression: {object:?}"
            );
        });
        // A dotted-constructor record and a type-argument record reach the
        // same `print_child`; both must survive too.
        for src in [
            format!("{DECL}(a.B {{p: 1}}).foo;"),
            format!("{DECL}(R<number> {{p: 1}}).foo;"),
            format!("{DECL}x = new (R {{p: 1}});"),
            format!("{DECL}x = (R {{p: 1}})`t`;"),
        ] {
            // The bar these clear is the minimum one: the regenerated source
            // must reparse at all. Before the fix each dropped its parens
            // and failed.
            round_trip_records(&src, pretty);
        }
        // No over-wrapping: a record expression under an operator that CAN
        // take it bare must not gain parens.
        for (src, spelling) in [
            (format!("{DECL}x = typeof R {{p: 1}};"), "typeof R"),
            (format!("{DECL}x = -R {{p: 1}};"), "-R"),
        ] {
            let mut parsed = parse_ok_records(&src);
            let js = gen(&mut parsed, Opt { pretty, ..Opt::default() });
            assert!(
                js.contains(spelling),
                "{pretty:?}: a record expression under a unary operator must stay bare \
                 (expected {spelling:?}): {js:?}"
            );
        }
    });
}

/// A `RecordExpression` at the start of an `ExpressionStatement` needs NO
/// parens, and this is deliberate (review round 4). Unlike
/// `MatchExpression`, a bare `R {p: 1};` at statement start parses to the
/// identical tree — measured, not assumed — so adding this kind to
/// `need_parens`'s statement-start guard would emit parens the grammar does
/// not require. It would also not fix the real defect: that guard wraps the
/// *whole* statement expression, and `(R {p: 1}.foo);` does not parse
/// either. The parens have to land on the record expression itself, which
/// is what its `get_precedence` entry does.
#[test]
fn record_expression_at_statement_start_needs_no_parens() {
    const DECL: &str = "record R { p: number }\n";
    for_each_pretty_mode(|pretty| {
        let src = format!("{DECL}(R {{p: 1}});");
        let mut reparsed = round_trip_records(&src, pretty);
        reparsed.with_program(|_gc, node| {
            let stmt = second_stmt(node, pretty);
            let Node::ExpressionStatement(ExpressionStatement { expression, .. }) = stmt else {
                panic!("{pretty:?}: not an ExpressionStatement: {stmt:?}");
            };
            assert!(
                matches!(expression, Node::RecordExpression(_)),
                "{pretty:?}: must stay a RecordExpression: {expression:?}"
            );
        });
        let mut parsed = parse_ok_records(&src);
        let js = gen(&mut parsed, Opt { pretty, ..Opt::default() });
        assert!(
            !js.contains("(R"),
            "{pretty:?}: a bare record expression statement must not gain parens: {js:?}"
        );
    });
}

/// Hands the program's SECOND top-level statement to the caller — the
/// record tests above all lead with a `record R {...}` declaration, which
/// `with_first_stmt` would return instead.
fn second_stmt<'gc>(node: &'gc Node<'gc>, pretty: Pretty) -> &'gc Node<'gc> {
    let Node::Program(Program { body, .. }) = node else {
        panic!("{pretty:?}: root is not a Program");
    };
    body.iter().nth(1).expect("program has a second statement")
}

/// `RecordExpressionProperties`: `{ x: 1, ...y }` (a spread element mixed
/// with a property — the same shape `ObjectExpression` allows).
#[test]
fn record_expression_properties_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip_records("const p = Point { x: 1, ...y };", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::VariableDeclaration(hermes_ast::node::VariableDeclaration {
                declarations,
                ..
            }) = stmt
            else {
                panic!("{pretty:?}: not a VariableDeclaration: {stmt:?}");
            };
            let Node::VariableDeclarator(hermes_ast::node::VariableDeclarator { init, .. }) =
                declarations.iter().next().expect("has a declarator")
            else {
                panic!("{pretty:?}: not a VariableDeclarator");
            };
            let Node::RecordExpression(RecordExpression { properties, .. }) =
                init.expect("has an initializer")
            else {
                panic!("{pretty:?}: not a RecordExpression: {init:?}");
            };
            let Node::RecordExpressionProperties(RecordExpressionProperties {
                properties, ..
            }) = properties
            else {
                panic!("{pretty:?}: not a RecordExpressionProperties: {properties:?}");
            };
            let mut it = properties.iter();
            assert!(matches!(it.next(), Some(Node::Property(_))), "{pretty:?}");
            assert!(matches!(it.next(), Some(Node::SpreadElement(_))), "{pretty:?}");
        });
    });
}

// --- Step 4: Flow `component`/`hook` (8 kinds) ------------------------------

/// `ComponentDeclaration`: `component Foo(x: number) renders X { return x; }`.
#[test]
fn component_declaration_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip_component(
            "component Foo(x: number) renders X { return x; }",
            pretty,
        );
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::ComponentDeclaration(ComponentDeclaration { params, renders_type, .. }) =
                stmt
            else {
                panic!("{pretty:?}: not a ComponentDeclaration: {stmt:?}");
            };
            assert_eq!(params.iter().count(), 1, "{pretty:?}");
            assert!(renders_type.is_some(), "{pretty:?}");
        });
    });
}

/// `ComponentParameter`: both `name as local` and the shorthand form.
#[test]
fn component_parameter_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed =
            round_trip_component("component Foo(\"data-x\" as x, y) { return x; }", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::ComponentDeclaration(ComponentDeclaration { params, .. }) = stmt else {
                panic!("{pretty:?}: not a ComponentDeclaration: {stmt:?}");
            };
            let mut it = params.iter();
            let Node::ComponentParameter(ComponentParameter { shorthand: sh0, .. }) =
                it.next().expect("param 0")
            else {
                panic!("{pretty:?}: param 0 not a ComponentParameter");
            };
            assert!(!sh0.get(), "{pretty:?}: \"data-x\" as x must not be shorthand");
            let Node::ComponentParameter(ComponentParameter { shorthand: sh1, .. }) =
                it.next().expect("param 1")
            else {
                panic!("{pretty:?}: param 1 not a ComponentParameter");
            };
            assert!(sh1.get(), "{pretty:?}: bare `y` must be shorthand");
        });
    });
}

/// `ComponentTypeAnnotation` / `ComponentTypeParameter`: `component(x:
/// number, ...rest: string) renders X` as a type value.
#[test]
fn component_type_annotation_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip_component(
            "type F = component(x: number, ...rest: string) renders X;",
            pretty,
        );
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::TypeAlias(hermes_ast::node::TypeAlias { right, .. }) = stmt else {
                panic!("{pretty:?}: not a TypeAlias: {stmt:?}");
            };
            let Node::ComponentTypeAnnotation(ComponentTypeAnnotation {
                params,
                rest,
                renders_type,
                ..
            }) = right
            else {
                panic!("{pretty:?}: not a ComponentTypeAnnotation: {right:?}");
            };
            assert_eq!(params.iter().count(), 1, "{pretty:?}");
            let Node::ComponentTypeParameter(ComponentTypeParameter { name, .. }) =
                params.iter().next().expect("param 0")
            else {
                panic!("{pretty:?}: param 0 not a ComponentTypeParameter");
            };
            assert!(name.is_some(), "{pretty:?}");
            assert!(rest.is_some(), "{pretty:?}");
            assert!(renders_type.is_some(), "{pretty:?}");
        });
    });
}

/// `ComponentTypeParameter`: a `...T` rest parameter with no name (the
/// unlabeled form — `name` is `None`).
#[test]
fn component_type_parameter_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed =
            round_trip_component("type F = component(...string);", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::TypeAlias(hermes_ast::node::TypeAlias { right, .. }) = stmt else {
                panic!("{pretty:?}: not a TypeAlias: {stmt:?}");
            };
            let Node::ComponentTypeAnnotation(ComponentTypeAnnotation { rest, .. }) = right
            else {
                panic!("{pretty:?}: not a ComponentTypeAnnotation: {right:?}");
            };
            let Node::ComponentTypeParameter(ComponentTypeParameter { name, .. }) =
                rest.expect("has a rest")
            else {
                panic!("{pretty:?}: rest not a ComponentTypeParameter");
            };
            assert!(name.is_none(), "{pretty:?}: unlabeled rest must have no name");
        });
    });
}

/// `DeclareComponent`: `declare component Foo(x: number): renders X;`.
#[test]
fn declare_component_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed =
            round_trip_component("declare component Foo(x: number) renders X;", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::DeclareComponent(DeclareComponent { params, renders_type, .. }) = stmt
            else {
                panic!("{pretty:?}: not a DeclareComponent: {stmt:?}");
            };
            assert_eq!(params.iter().count(), 1, "{pretty:?}");
            assert!(renders_type.is_some(), "{pretty:?}");
        });
    });
}

/// `DeclareComponent` nested inside `declare export`: `declare ` must print
/// only once (`declare_prefix_needed`'s whole point).
#[test]
fn declare_export_component_round_trips_without_doubling_declare() {
    for_each_pretty_mode(|pretty| {
        let mut parsed = parse_ok_component("declare export component Foo();");
        let js = gen(&mut parsed, Opt { pretty, ..Opt::default() });
        assert!(!js.contains("declare declare"), "{pretty:?}: {js:?}");
        assert!(js.contains("declare export"), "{pretty:?}: {js:?}");
        hermes_parser::parse(&js, component_flags())
            .unwrap_or_else(|e| panic!("{pretty:?}: regenerated {js:?} must reparse: {e:?}"));
    });
}

/// `DeclareHook`: `declare hook useFoo(x: number): void;`.
#[test]
fn declare_hook_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip_component("declare hook useFoo(x: number): void;", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            assert!(matches!(stmt, Node::DeclareHook(_)), "{pretty:?}: {stmt:?}");
        });
    });
}

/// `HookDeclaration`: `hook useFoo(x: number): number { return x; }`.
#[test]
fn hook_declaration_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip_component(
            "hook useFoo(x: number): number { return x; }",
            pretty,
        );
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::HookDeclaration(HookDeclaration { params, return_type, .. }) = stmt else {
                panic!("{pretty:?}: not a HookDeclaration: {stmt:?}");
            };
            assert_eq!(params.iter().count(), 1, "{pretty:?}");
            assert!(return_type.is_some(), "{pretty:?}");
        });
    });
}

/// `HookTypeAnnotation`: `hook(x: number) => number` as a type value.
#[test]
fn hook_type_annotation_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed =
            round_trip_component("type F = hook(x: number) => number;", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::TypeAlias(hermes_ast::node::TypeAlias { right, .. }) = stmt else {
                panic!("{pretty:?}: not a TypeAlias: {stmt:?}");
            };
            let Node::HookTypeAnnotation(HookTypeAnnotation { params, .. }) = right else {
                panic!("{pretty:?}: not a HookTypeAnnotation: {right:?}");
            };
            assert_eq!(params.iter().count(), 1, "{pretty:?}");
        });
    });
}

// --- Step 5: the remaining type kinds (16) ----------------------------------

/// `ConditionalTypeAnnotation`: `A extends B ? C : D`.
#[test]
fn conditional_type_annotation_round_trips() {
    for_each_pretty_mode(|pretty| {
        round_trip_return_flow_type("function f(): A extends B ? C : D {}", pretty, |_gc, ty, generated| {
            let Node::ConditionalTypeAnnotation(ConditionalTypeAnnotation { .. }) = ty else {
                panic!("{pretty:?}: not a ConditionalTypeAnnotation: {ty:?} ({generated:?})");
            };
        });
    });
}

/// `ConditionalTypeAnnotation.check_type`/`.extends_type` are parsed at
/// union tier, not the full type grammar (review-round-2 regression test):
/// a parenthesized nested conditional in either position must keep its
/// parens, or the regenerated source reparses to a DIFFERENT tree (the
/// nested conditional's `?`/`:` get absorbed by the outer one instead of
/// staying grouped).
#[test]
fn conditional_type_annotation_parenthesizes_restricted_check_and_extends_type() {
    for_each_pretty_mode(|pretty| {
        round_trip_return_flow_type(
            "function f(): (A extends B ? C : D) extends E ? F : G {}",
            pretty,
            |_gc, ty, generated| {
                let Node::ConditionalTypeAnnotation(ConditionalTypeAnnotation {
                    check_type, ..
                }) = ty
                else {
                    panic!("{pretty:?}: not a ConditionalTypeAnnotation: {ty:?} ({generated:?})");
                };
                assert!(
                    matches!(check_type, Node::ConditionalTypeAnnotation(_)),
                    "{pretty:?}: check_type must stay a ConditionalTypeAnnotation, not get \
                     absorbed into the outer one: {check_type:?} ({generated:?})"
                );
            },
        );
        round_trip_return_flow_type(
            "function f(): A extends (B extends C ? D : E) ? F : G {}",
            pretty,
            |_gc, ty, generated| {
                let Node::ConditionalTypeAnnotation(ConditionalTypeAnnotation {
                    extends_type,
                    ..
                }) = ty
                else {
                    panic!("{pretty:?}: not a ConditionalTypeAnnotation: {ty:?} ({generated:?})");
                };
                assert!(
                    matches!(extends_type, Node::ConditionalTypeAnnotation(_)),
                    "{pretty:?}: extends_type must stay a ConditionalTypeAnnotation, not get \
                     absorbed into the outer one: {extends_type:?} ({generated:?})"
                );
            },
        );
    });
}

/// `InferTypeAnnotation`: `infer A extends B` — the bug-avoidance case
/// (`arms/newer.rs`'s module doc comment): must NOT come back as `infer A:
/// B`, which would not reparse as an `InferTypeAnnotation` with a bound at
/// all.
#[test]
fn infer_type_annotation_round_trips() {
    for_each_pretty_mode(|pretty| {
        round_trip_return_flow_type(
            "function f(): A extends infer B extends C ? B : never {}",
            pretty,
            |_gc, ty, generated| {
                let Node::ConditionalTypeAnnotation(ConditionalTypeAnnotation {
                    extends_type,
                    ..
                }) = ty
                else {
                    panic!("{pretty:?}: not a ConditionalTypeAnnotation: {ty:?} ({generated:?})");
                };
                let Node::InferTypeAnnotation(InferTypeAnnotation { type_parameter, .. }) =
                    extends_type
                else {
                    panic!("{pretty:?}: extends_type is not an InferTypeAnnotation: {extends_type:?}");
                };
                let Node::TypeParameter(TypeParameter { bound, .. }) = type_parameter else {
                    panic!("{pretty:?}: not a TypeParameter: {type_parameter:?}");
                };
                assert!(
                    bound.is_some(),
                    "{pretty:?}: `infer B extends C`'s bound must round-trip: {generated:?}"
                );
                assert!(
                    !generated.contains("infer B:") && !generated.contains("infer B :"),
                    "{pretty:?}: must not print `infer B: C` (unreparseable as a bound): {generated:?}"
                );
            },
        );
    });
}

/// `InferTypeAnnotation`'s `bound` is parsed at union tier too
/// (review-round-2 regression test) — a parenthesized conditional bound
/// must keep its parens, or the regenerated source **fails to reparse
/// entirely** (worse than the other four defects, which merely change the
/// tree): the speculative-bound backtrack bails out to a bare `infer B`
/// the moment a bare `extends` follows the union-tier parse, leaving the
/// rest of the source as dangling tokens.
#[test]
fn infer_type_annotation_parenthesizes_restricted_conditional_bound() {
    for_each_pretty_mode(|pretty| {
        round_trip_return_flow_type(
            "function f(): A extends infer B extends (C extends D ? E : F) ? B : never {}",
            pretty,
            |_gc, ty, generated| {
                let Node::ConditionalTypeAnnotation(ConditionalTypeAnnotation {
                    extends_type,
                    ..
                }) = ty
                else {
                    panic!("{pretty:?}: not a ConditionalTypeAnnotation: {ty:?} ({generated:?})");
                };
                let Node::InferTypeAnnotation(InferTypeAnnotation { type_parameter, .. }) =
                    extends_type
                else {
                    panic!("{pretty:?}: extends_type is not an InferTypeAnnotation: {extends_type:?}");
                };
                let Node::TypeParameter(TypeParameter { bound, .. }) = type_parameter else {
                    panic!("{pretty:?}: not a TypeParameter: {type_parameter:?}");
                };
                let Some(bound) = bound else {
                    panic!("{pretty:?}: bound must round-trip: {generated:?}");
                };
                assert!(
                    matches!(bound, Node::ConditionalTypeAnnotation(_)),
                    "{pretty:?}: bound must stay a ConditionalTypeAnnotation: {bound:?} ({generated:?})"
                );
            },
        );
    });
}

/// An `InferTypeAnnotation` with **no bound** is a plain
/// `parsePrimaryTypeAnnotationFlow` production — `infer` plus one
/// identifier, nothing more — and must NOT be parenthesized anywhere
/// (review-round-3 regression test; round 2 grouped it into
/// `ConditionalTypeAnnotation`'s `ALWAYS_PAREN` arm, which wrapped every
/// `infer` reached through any `print_child`, so the canonical unnested
/// idiom `A extends infer B ? B : never` regenerated as
/// `A extends (infer B) ? B : never`).
///
/// The one position where the following token could in principle be
/// absorbed — a `ConditionalTypeAnnotation`'s `check_type`, the only place
/// this printer emits `extends` right after a child — is covered by the
/// parser's own speculative-bound backtrack: `check_type` is parsed with
/// `allow_conditional_type = true`, so on seeing the `?` after the
/// speculative bound the `infer` arm restores and re-reads the `extends` as
/// the conditional's (`crates/parser/src/js/flow/types.rs:733-747`). The
/// last two cases below are that one, checked directly.
#[test]
fn infer_type_annotation_without_bound_needs_no_parens() {
    for_each_pretty_mode(|pretty| {
        // The everyday idiom the round-2 over-wrap disfigured.
        round_trip_return_flow_type(
            "function f(): A extends infer B ? B : never {}",
            pretty,
            |_gc, ty, generated| {
                assert_eq!(
                    generated,
                    match pretty {
                        Pretty::Yes => "A extends infer B ? B : never",
                        Pretty::No => "A extends infer B?B:never",
                    },
                    "{pretty:?}: an unbounded `infer` must not gain parens"
                );
                let Node::ConditionalTypeAnnotation(ConditionalTypeAnnotation {
                    extends_type,
                    ..
                }) = ty
                else {
                    panic!("{pretty:?}: not a ConditionalTypeAnnotation: {ty:?} ({generated:?})");
                };
                assert!(
                    matches!(extends_type, Node::InferTypeAnnotation(_)),
                    "{pretty:?}: extends_type must stay an InferTypeAnnotation: {extends_type:?}"
                );
            },
        );
        // Every other `print_child` position an unbounded `infer` can land
        // in: union member, `?`-nullable operand, postfix `[]` base, and
        // `keyof` operand. Each source below writes the parens explicitly;
        // each must come back without them AND with the same tree.
        round_trip_return_flow_type(
            "function f(): (infer A) | B {}",
            pretty,
            |_gc, ty, generated| {
                assert_eq!(
                    generated,
                    match pretty {
                        Pretty::Yes => "infer A | B",
                        Pretty::No => "infer A|B",
                    },
                    "{pretty:?}"
                );
                let Node::UnionTypeAnnotation(UnionTypeAnnotation { types, .. }) = ty else {
                    panic!("{pretty:?}: not a UnionTypeAnnotation: {ty:?} ({generated:?})");
                };
                let mut it = types.iter();
                assert!(
                    matches!(it.next().expect("member 0"), Node::InferTypeAnnotation(_)),
                    "{pretty:?}: member 0 must stay an InferTypeAnnotation"
                );
                assert_eq!(types.iter().count(), 2, "{pretty:?}");
            },
        );
        round_trip_return_flow_type("function f(): ?(infer A) {}", pretty, |_gc, ty, generated| {
            assert_eq!(generated, "?infer A", "{pretty:?}");
            let Node::NullableTypeAnnotation(NullableTypeAnnotation { type_annotation, .. }) = ty
            else {
                panic!("{pretty:?}: not a NullableTypeAnnotation: {ty:?} ({generated:?})");
            };
            assert!(
                matches!(type_annotation, Node::InferTypeAnnotation(_)),
                "{pretty:?}: operand must stay an InferTypeAnnotation: {type_annotation:?}"
            );
        });
        round_trip_return_flow_type("function f(): (infer A)[] {}", pretty, |_gc, ty, generated| {
            assert_eq!(generated, "infer A[]", "{pretty:?}");
            let Node::ArrayTypeAnnotation(ArrayTypeAnnotation { element_type, .. }) = ty else {
                panic!("{pretty:?}: not an ArrayTypeAnnotation: {ty:?} ({generated:?})");
            };
            assert!(
                matches!(element_type, Node::InferTypeAnnotation(_)),
                "{pretty:?}: element_type must stay an InferTypeAnnotation: {element_type:?}"
            );
        });
        round_trip_return_flow_type(
            "function f(): keyof (infer A) {}",
            pretty,
            |_gc, ty, generated| {
                assert_eq!(generated, "keyof infer A", "{pretty:?}");
                let Node::KeyofTypeAnnotation(KeyofTypeAnnotation { argument, .. }) = ty else {
                    panic!("{pretty:?}: not a KeyofTypeAnnotation: {ty:?} ({generated:?})");
                };
                assert!(
                    matches!(argument, Node::InferTypeAnnotation(_)),
                    "{pretty:?}: argument must stay an InferTypeAnnotation: {argument:?}"
                );
            },
        );
        // The backtrack case: an unbounded `infer` as a conditional's
        // `check_type`, printed bare, immediately followed by the
        // conditional's own `extends`. The parser must NOT absorb `C` as
        // the infer's bound.
        round_trip_return_flow_type(
            "function f(): (infer B) extends C ? D : E {}",
            pretty,
            |_gc, ty, generated| {
                assert_eq!(
                    generated,
                    match pretty {
                        Pretty::Yes => "infer B extends C ? D : E",
                        Pretty::No => "infer B extends C?D:E",
                    },
                    "{pretty:?}"
                );
                let Node::ConditionalTypeAnnotation(ConditionalTypeAnnotation {
                    check_type,
                    ..
                }) = ty
                else {
                    panic!("{pretty:?}: not a ConditionalTypeAnnotation: {ty:?} ({generated:?})");
                };
                let Node::InferTypeAnnotation(InferTypeAnnotation { type_parameter, .. }) =
                    check_type
                else {
                    panic!("{pretty:?}: check_type is not an InferTypeAnnotation: {check_type:?}");
                };
                let Node::TypeParameter(TypeParameter { bound, .. }) = type_parameter else {
                    panic!("{pretty:?}: not a TypeParameter: {type_parameter:?}");
                };
                assert!(
                    bound.is_none(),
                    "{pretty:?}: the conditional's `extends C` must NOT become the infer's \
                     bound: {generated:?}"
                );
            },
        );
        // The same, one level down: the unbounded `infer` is a member of
        // the `check_type`'s union rather than the whole `check_type`.
        round_trip_return_flow_type(
            "function f(): ((infer B) | C) extends D ? E : F {}",
            pretty,
            |_gc, ty, generated| {
                let Node::ConditionalTypeAnnotation(ConditionalTypeAnnotation {
                    check_type,
                    ..
                }) = ty
                else {
                    panic!("{pretty:?}: not a ConditionalTypeAnnotation: {ty:?} ({generated:?})");
                };
                let Node::UnionTypeAnnotation(UnionTypeAnnotation { types, .. }) = check_type
                else {
                    panic!("{pretty:?}: check_type is not a UnionTypeAnnotation: {check_type:?}");
                };
                assert_eq!(types.iter().count(), 2, "{pretty:?}: {generated:?}");
                assert!(
                    matches!(types.iter().next().expect("member 0"), Node::InferTypeAnnotation(_)),
                    "{pretty:?}: member 0 must stay an InferTypeAnnotation: {generated:?}"
                );
            },
        );
    });
}

/// An `InferTypeAnnotation` that DOES have a bound stays at `ALWAYS_PAREN`
/// (review-round-3): the bound is parsed by the speculative
/// `parse_union_type_annotation_flow()` call in the `infer` arm, so the
/// construct extends rightwards over a whole union and binds *looser* than
/// `UnionTypeAnnotation` itself. Without the wrap,
/// `?(infer B extends C) | D` regenerates as `?infer B extends C | D`,
/// whose bound greedily swallows `C | D` — a different tree.
#[test]
fn infer_type_annotation_with_bound_stays_parenthesized() {
    for_each_pretty_mode(|pretty| {
        round_trip_return_flow_type(
            "function f(): ?(infer B extends C) | D {}",
            pretty,
            |_gc, ty, generated| {
                assert!(
                    generated.contains("(infer B extends C)"),
                    "{pretty:?}: a bounded `infer` must keep its parens: {generated:?}"
                );
                let Node::UnionTypeAnnotation(UnionTypeAnnotation { types, .. }) = ty else {
                    panic!("{pretty:?}: not a UnionTypeAnnotation: {ty:?} ({generated:?})");
                };
                assert_eq!(types.iter().count(), 2, "{pretty:?}: {generated:?}");
                let Node::NullableTypeAnnotation(NullableTypeAnnotation {
                    type_annotation,
                    ..
                }) = types.iter().next().expect("member 0")
                else {
                    panic!("{pretty:?}: member 0 is not a NullableTypeAnnotation ({generated:?})");
                };
                let Node::InferTypeAnnotation(InferTypeAnnotation { type_parameter, .. }) =
                    type_annotation
                else {
                    panic!("{pretty:?}: not an InferTypeAnnotation: {type_annotation:?}");
                };
                let Node::TypeParameter(TypeParameter { bound, .. }) = type_parameter else {
                    panic!("{pretty:?}: not a TypeParameter: {type_parameter:?}");
                };
                let Some(bound) = bound else {
                    panic!("{pretty:?}: bound must round-trip: {generated:?}");
                };
                assert!(
                    matches!(bound, Node::GenericTypeAnnotation(_)),
                    "{pretty:?}: the bound must stay just `C`, not swallow `| D`: {bound:?} \
                     ({generated:?})"
                );
            },
        );
        round_trip_return_flow_type(
            "function f(): (infer B extends C) | D {}",
            pretty,
            |_gc, ty, generated| {
                let Node::UnionTypeAnnotation(UnionTypeAnnotation { types, .. }) = ty else {
                    panic!("{pretty:?}: not a UnionTypeAnnotation: {ty:?} ({generated:?})");
                };
                assert_eq!(
                    types.iter().count(),
                    2,
                    "{pretty:?}: the bound must not swallow `| D`: {generated:?}"
                );
            },
        );
    });
}

/// `KeyofTypeAnnotation`: `keyof T`.
#[test]
fn keyof_type_annotation_round_trips() {
    for_each_pretty_mode(|pretty| {
        round_trip_return_flow_type("function f(): keyof T {}", pretty, |_gc, ty, generated| {
            let Node::KeyofTypeAnnotation(KeyofTypeAnnotation { argument, .. }) = ty else {
                panic!("{pretty:?}: not a KeyofTypeAnnotation: {ty:?} ({generated:?})");
            };
            assert!(matches!(argument, Node::GenericTypeAnnotation(_)), "{pretty:?}");
        });
    });
}

/// `KeyofTypeAnnotation.argument` is parsed at prefix tier, not the full
/// type grammar (review-round-2 regression test): `keyof (A | B)` must keep
/// its parens, or the regenerated source reparses with a DIFFERENT
/// top-level kind (`Union[Keyof(A), B]` instead of `Keyof(Union[A, B])`).
#[test]
fn keyof_type_annotation_parenthesizes_restricted_union_argument() {
    for_each_pretty_mode(|pretty| {
        round_trip_return_flow_type(
            "function f(): keyof (A | B) {}",
            pretty,
            |_gc, ty, generated| {
                let Node::KeyofTypeAnnotation(KeyofTypeAnnotation { argument, .. }) = ty else {
                    panic!("{pretty:?}: not a KeyofTypeAnnotation: {ty:?} ({generated:?})");
                };
                assert!(
                    matches!(argument, Node::UnionTypeAnnotation(_)),
                    "{pretty:?}: argument must stay the whole Union, not get flattened into an \
                     outer union: {argument:?} ({generated:?})"
                );
            },
        );
    });
}

/// `NeverTypeAnnotation`: `never`.
#[test]
fn never_type_annotation_round_trips() {
    for_each_pretty_mode(|pretty| {
        round_trip_return_flow_type("function f(): never {}", pretty, |_gc, ty, generated| {
            assert!(matches!(ty, Node::NeverTypeAnnotation(_)), "{pretty:?}: {ty:?} ({generated:?})");
        });
    });
}

/// `UndefinedTypeAnnotation`: `undefined`.
#[test]
fn undefined_type_annotation_round_trips() {
    for_each_pretty_mode(|pretty| {
        round_trip_return_flow_type("function f(): undefined {}", pretty, |_gc, ty, generated| {
            assert!(
                matches!(ty, Node::UndefinedTypeAnnotation(_)),
                "{pretty:?}: {ty:?} ({generated:?})"
            );
        });
    });
}

/// `UnknownTypeAnnotation`: `unknown`.
#[test]
fn unknown_type_annotation_round_trips() {
    for_each_pretty_mode(|pretty| {
        round_trip_return_flow_type("function f(): unknown {}", pretty, |_gc, ty, generated| {
            assert!(
                matches!(ty, Node::UnknownTypeAnnotation(_)),
                "{pretty:?}: {ty:?} ({generated:?})"
            );
        });
    });
}

/// `TypeOperator`: all three `renders`/`renders?`/`renders*` spellings, via
/// a component's `renders` clause.
#[test]
fn type_operator_renders_round_trips() {
    for_each_pretty_mode(|pretty| {
        for (src, expected) in [
            ("component Foo() renders X { return null; }", "renders"),
            ("component Foo() renders? X { return null; }", "renders?"),
            ("component Foo() renders* X { return null; }", "renders*"),
        ] {
            let mut reparsed = round_trip_component(src, pretty);
            with_first_stmt(&mut reparsed, |gc, stmt| {
                let Node::ComponentDeclaration(ComponentDeclaration { renders_type, .. }) = stmt
                else {
                    panic!("{pretty:?}: not a ComponentDeclaration: {stmt:?}");
                };
                let Node::TypeOperator(TypeOperator { operator, .. }) =
                    renders_type.expect("has a renders type")
                else {
                    panic!("{pretty:?}: renders_type not a TypeOperator");
                };
                assert_eq!(gc.bytes_str_lossy(operator.get()), expected, "{pretty:?}: {src:?}");
            });
        }
    });
}

/// `TypeOperator.type_annotation` is parsed at prefix tier via
/// `ComponentTypeAnnotation`'s `renders_type` specifically (a component
/// TYPE annotation's `renders` clause, `component_type: true` in
/// `parse_component_render_type_flow` — as opposed to
/// `ComponentDeclaration`'s own `renders_type`, parsed at full tier;
/// review-round-2 regression test, since the AST cannot tell which of the
/// two built a given `TypeOperator` so this crate must protect
/// conservatively for both): `component() renders (A | B)` must keep its
/// parens, or the regenerated source's `renders_type` reparses as a bare
/// `UnionTypeAnnotation` — a different top-level kind for that field.
#[test]
fn type_operator_parenthesizes_restricted_union_in_component_type_renders() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip_component("type F = component() renders (A | B);", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::TypeAlias(hermes_ast::node::TypeAlias { right, .. }) = stmt else {
                panic!("{pretty:?}: not a TypeAlias: {stmt:?}");
            };
            let Node::ComponentTypeAnnotation(ComponentTypeAnnotation { renders_type, .. }) =
                right
            else {
                panic!("{pretty:?}: not a ComponentTypeAnnotation: {right:?}");
            };
            let Node::TypeOperator(TypeOperator { type_annotation, .. }) =
                renders_type.expect("has a renders type")
            else {
                panic!("{pretty:?}: renders_type not a TypeOperator");
            };
            assert!(
                matches!(type_annotation, Node::UnionTypeAnnotation(_)),
                "{pretty:?}: type_annotation must stay the whole Union: {type_annotation:?}"
            );
        });
    });
}

/// `TypePredicate`: all three shapes — `asserts x`, `asserts x is T`, and
/// the bare `x is T` (no prefix keyword — the `kind` field is the
/// `INVALID_ATOM_BYTES` optional-`NodeString` sentinel).
#[test]
fn type_predicate_round_trips() {
    for_each_pretty_mode(|pretty| {
        round_trip_return_flow_type(
            "function f(): asserts x {}",
            pretty,
            |gc, ty, generated| {
                let Node::TypePredicate(TypePredicate { type_annotation, kind, .. }) = ty else {
                    panic!("{pretty:?}: not a TypePredicate: {ty:?} ({generated:?})");
                };
                assert_eq!(gc.bytes_str_lossy(kind.get()), "asserts", "{pretty:?}");
                assert!(type_annotation.is_none(), "{pretty:?}");
            },
        );
        round_trip_return_flow_type(
            "function f(): asserts x is number {}",
            pretty,
            |gc, ty, generated| {
                let Node::TypePredicate(TypePredicate { type_annotation, kind, .. }) = ty else {
                    panic!("{pretty:?}: not a TypePredicate: {ty:?} ({generated:?})");
                };
                assert_eq!(gc.bytes_str_lossy(kind.get()), "asserts", "{pretty:?}");
                assert!(type_annotation.is_some(), "{pretty:?}");
            },
        );
        round_trip_return_flow_type(
            "function f(): x is number {}",
            pretty,
            |gc, ty, generated| {
                let Node::TypePredicate(TypePredicate { type_annotation, kind, .. }) = ty else {
                    panic!("{pretty:?}: not a TypePredicate: {ty:?} ({generated:?})");
                };
                assert!(gc.try_bytes_str(kind.get()).is_none(), "{pretty:?}: bare predicate must keep no kind prefix");
                assert!(type_annotation.is_some(), "{pretty:?}");
            },
        );
    });
}

/// `ObjectTypeMappedTypeProperty`: `[K in T]`, `+[K in T]+?`, and `-[K in
/// T]-?` — the variance prefix and the three `optional` sigil translations.
#[test]
fn object_type_mapped_type_property_round_trips() {
    for_each_pretty_mode(|pretty| {
        for (src, has_variance, expected_optional) in [
            ("type F = { [K in T]: number };", false, None),
            ("type F = { +[K in T]+?: number };", true, Some("PlusOptional")),
            ("type F = { -[K in T]-?: number };", true, Some("MinusOptional")),
            ("type F = { [K in T]?: number };", false, Some("Optional")),
        ] {
            let mut reparsed = round_trip_flow(src, pretty);
            with_first_stmt(&mut reparsed, |gc, stmt| {
                let Node::TypeAlias(hermes_ast::node::TypeAlias { right, .. }) = stmt else {
                    panic!("{pretty:?}: not a TypeAlias: {stmt:?}");
                };
                let Node::ObjectTypeAnnotation(hermes_ast::node::ObjectTypeAnnotation {
                    properties,
                    ..
                }) = right
                else {
                    panic!("{pretty:?}: not an ObjectTypeAnnotation: {right:?}");
                };
                let Node::ObjectTypeMappedTypeProperty(ObjectTypeMappedTypeProperty {
                    variance,
                    optional,
                    ..
                }) = properties.iter().next().expect("has a property")
                else {
                    panic!("{pretty:?}: not an ObjectTypeMappedTypeProperty: {src:?}");
                };
                assert_eq!(variance.is_some(), has_variance, "{pretty:?}: {src:?}");
                assert_eq!(
                    gc.try_bytes_str(optional.get()),
                    expected_optional,
                    "{pretty:?}: {src:?}"
                );
            });
        }
    });
}

/// `QualifiedTypeofIdentifier`: `typeof a.b`.
#[test]
fn qualified_typeof_identifier_round_trips() {
    for_each_pretty_mode(|pretty| {
        round_trip_return_flow_type("function f(): typeof a.b {}", pretty, |_gc, ty, generated| {
            let Node::TypeofTypeAnnotation(hermes_ast::node::TypeofTypeAnnotation {
                argument,
                ..
            }) = ty
            else {
                panic!("{pretty:?}: not a TypeofTypeAnnotation: {ty:?} ({generated:?})");
            };
            let Node::QualifiedTypeofIdentifier(QualifiedTypeofIdentifier {
                qualification,
                id,
                ..
            }) = argument
            else {
                panic!("{pretty:?}: argument not a QualifiedTypeofIdentifier: {argument:?}");
            };
            assert!(matches!(qualification, Node::Identifier(_)), "{pretty:?}");
            assert!(matches!(id, Node::Identifier(_)), "{pretty:?}");
        });
    });
}

/// `TupleTypeLabeledElement`: `[+foo?: number]`.
#[test]
fn tuple_type_labeled_element_round_trips() {
    for_each_pretty_mode(|pretty| {
        round_trip_return_flow_type(
            "function f(): [+foo?: number] {}",
            pretty,
            |_gc, ty, generated| {
                let Node::TupleTypeAnnotation(hermes_ast::node::TupleTypeAnnotation {
                    element_types,
                    ..
                }) = ty
                else {
                    panic!("{pretty:?}: not a TupleTypeAnnotation: {ty:?} ({generated:?})");
                };
                let Node::TupleTypeLabeledElement(TupleTypeLabeledElement {
                    optional,
                    variance,
                    ..
                }) = element_types.iter().next().expect("has an element")
                else {
                    panic!("{pretty:?}: not a TupleTypeLabeledElement");
                };
                assert!(optional.get(), "{pretty:?}");
                assert!(variance.is_some(), "{pretty:?}");
            },
        );
    });
}

/// `TupleTypeSpreadElement`: both the labeled (`...foo: number`) and bare
/// (`...number`) forms.
#[test]
fn tuple_type_spread_element_round_trips() {
    for_each_pretty_mode(|pretty| {
        round_trip_return_flow_type(
            "function f(): [...foo: number] {}",
            pretty,
            |_gc, ty, generated| {
                let Node::TupleTypeAnnotation(hermes_ast::node::TupleTypeAnnotation {
                    element_types,
                    ..
                }) = ty
                else {
                    panic!("{pretty:?}: not a TupleTypeAnnotation: {ty:?} ({generated:?})");
                };
                let Node::TupleTypeSpreadElement(TupleTypeSpreadElement { label, .. }) =
                    element_types.iter().next().expect("has an element")
                else {
                    panic!("{pretty:?}: not a TupleTypeSpreadElement");
                };
                assert!(label.is_some(), "{pretty:?}: {generated:?}");
            },
        );
        round_trip_return_flow_type(
            "function f(): [...number] {}",
            pretty,
            |_gc, ty, generated| {
                let Node::TupleTypeAnnotation(hermes_ast::node::TupleTypeAnnotation {
                    element_types,
                    ..
                }) = ty
                else {
                    panic!("{pretty:?}: not a TupleTypeAnnotation: {ty:?} ({generated:?})");
                };
                let Node::TupleTypeSpreadElement(TupleTypeSpreadElement { label, .. }) =
                    element_types.iter().next().expect("has an element")
                else {
                    panic!("{pretty:?}: not a TupleTypeSpreadElement");
                };
                assert!(label.is_none(), "{pretty:?}: {generated:?}");
            },
        );
    });
}

/// `DeclareEnum`: `declare enum E { A, B }`.
#[test]
fn declare_enum_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip_flow("declare enum E { A, B }", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            assert!(matches!(stmt, Node::DeclareEnum(_)), "{pretty:?}: {stmt:?}");
        });
    });
}

/// `DeclareEnum` nested inside `declare export`: `declare ` must print only
/// once.
#[test]
fn declare_export_enum_round_trips_without_doubling_declare() {
    for_each_pretty_mode(|pretty| {
        let mut parsed = parse_ok_flow("declare export enum E { A, B }");
        let js = gen(&mut parsed, Opt { pretty, ..Opt::default() });
        assert!(!js.contains("declare declare"), "{pretty:?}: {js:?}");
        assert!(js.contains("declare export"), "{pretty:?}: {js:?}");
        hermes_parser::parse(&js, flow_flags())
            .unwrap_or_else(|e| panic!("{pretty:?}: regenerated {js:?} must reparse: {e:?}"));
    });
}

/// `DeclareNamespace`: `declare namespace N { declare function f(): void; }`.
#[test]
fn declare_namespace_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed =
            round_trip_flow("declare namespace N { declare function f(): void; }", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::DeclareNamespace(DeclareNamespace { body, .. }) = stmt else {
                panic!("{pretty:?}: not a DeclareNamespace: {stmt:?}");
            };
            assert!(matches!(body, Node::BlockStatement(_)), "{pretty:?}");
        });
    });
}

/// `EnumBigIntBody`: `enum E of bigint { A = 1n, B = 2n }`.
#[test]
fn enum_bigint_body_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip_flow("enum E of bigint { A = 1n, B = 2n }", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::EnumDeclaration(hermes_ast::node::EnumDeclaration { body, .. }) = stmt
            else {
                panic!("{pretty:?}: not an EnumDeclaration: {stmt:?}");
            };
            let Node::EnumBigIntBody(EnumBigIntBody { members, explicit_type, .. }) = body
            else {
                panic!("{pretty:?}: not an EnumBigIntBody: {body:?}");
            };
            assert!(explicit_type.get(), "{pretty:?}");
            assert_eq!(members.iter().count(), 2, "{pretty:?}");
        });
    });
}

/// `EnumBigIntMember`: `A = 1n`.
#[test]
fn enum_bigint_member_round_trips() {
    for_each_pretty_mode(|pretty| {
        let mut reparsed = round_trip_flow("enum E of bigint { A = 1n }", pretty);
        with_first_stmt(&mut reparsed, |_gc, stmt| {
            let Node::EnumDeclaration(hermes_ast::node::EnumDeclaration { body, .. }) = stmt
            else {
                panic!("{pretty:?}: not an EnumDeclaration: {stmt:?}");
            };
            let Node::EnumBigIntBody(EnumBigIntBody { members, .. }) = body else {
                panic!("{pretty:?}: not an EnumBigIntBody: {body:?}");
            };
            let Node::EnumBigIntMember(EnumBigIntMember { init, .. }) =
                members.iter().next().expect("has a member")
            else {
                panic!("{pretty:?}: not an EnumBigIntMember");
            };
            assert!(matches!(init, Node::BigIntLiteral(_)), "{pretty:?}");
        });
    });
}

// ---------------------------------------------------------------------------
// Task 13 (`arms/ts.rs`): the 46 TypeScript kinds.
//
// These cases assert something strictly stronger than the Flow/JSX sections
// above: not "the reparsed node has kind K and the children I remembered to
// look at", but "the reparsed program's **entire** ESTree — every kind, every
// field, at every depth — is byte-identical to the original's", plus the exact
// generated text in both pretty modes. `hermes_ast::dump::dump_estree_json`
// without a `SourceErrorManager` emits no locations
// (`LocationDumpMode::None`), so the comparison is purely structural; a
// dropped modifier, a lost paren that re-associates a type, or a `TSUnionType`
// that came back as a `TSIntersectionType` all fail it. Pinning the text as
// well is what catches the opposite defect — *redundant* parens, which a
// structural comparison alone would happily accept.
// ---------------------------------------------------------------------------

/// TypeScript is a separate dialect flag from Flow and they are mutually
/// exclusive (`ParseFlags::parse_ts`'s doc comment), so the TS cases need
/// their own flag set rather than `flow_flags()`.
fn ts_flags() -> ParseFlags {
    ParseFlags {
        parse_ts: true,
        ..Default::default()
    }
}

/// The whole program's ESTree as JSON, with no source locations — the
/// structural fingerprint the TS cases compare before against after.
fn ast_json(parsed: &mut ParsedJS) -> String {
    parsed.with_program(|gc, root| {
        let mut s = String::new();
        hermes_ast::dump::dump_estree_json(
            &mut s,
            root,
            true,
            hermes_ast::dump::ESTreeDumpMode::HideEmpty,
            gc.ctx().atom_table(),
        );
        s
    })
}

/// Parse `src` under [`ts_flags`], generate it back under `pretty`, reparse
/// that, and assert the two ASTs are structurally identical. Returns the
/// generated text so the caller can pin it exactly.
///
/// Panics (naming `src`, `pretty` and the generated text) if either parse
/// fails or the trees differ.
fn ts_round_trip(src: &str, pretty: Pretty) -> String {
    let mut parsed = hermes_parser::parse(src, ts_flags())
        .unwrap_or_else(|e| panic!("{src:?} must parse under -parse-ts: {e:?}"));
    let before = ast_json(&mut parsed);
    let js = gen(
        &mut parsed,
        Opt {
            pretty,
            ..Opt::default()
        },
    );
    let mut reparsed = hermes_parser::parse(&js, ts_flags()).unwrap_or_else(|e| {
        panic!("regenerated {js:?} (from {src:?}, {pretty:?}) must parse under -parse-ts: {e:?}")
    });
    let after = ast_json(&mut reparsed);
    assert_eq!(
        before, after,
        "{pretty:?}: {src:?} regenerated as {js:?} reparses to a DIFFERENT AST"
    );
    js
}

/// [`ts_round_trip`] in both pretty modes, pinning the exact output of each.
fn ts_case(src: &str, pretty_text: &str, compact_text: &str) {
    assert_eq!(ts_round_trip(src, Pretty::Yes), pretty_text, "{src:?}");
    assert_eq!(ts_round_trip(src, Pretty::No), compact_text, "{src:?}");
}

// --- the primitive keyword types (10) + `this` -----------------------------

/// `TSAnyKeyword`.
#[test]
fn ts_any_keyword_round_trips() {
    ts_case("type T = any;", "type T = any;\n", "type T=any;\n");
}

/// `TSNumberKeyword`.
#[test]
fn ts_number_keyword_round_trips() {
    ts_case("type T = number;", "type T = number;\n", "type T=number;\n");
}

/// `TSBooleanKeyword`.
#[test]
fn ts_boolean_keyword_round_trips() {
    ts_case("type T = boolean;", "type T = boolean;\n", "type T=boolean;\n");
}

/// `TSStringKeyword`.
#[test]
fn ts_string_keyword_round_trips() {
    ts_case("type T = string;", "type T = string;\n", "type T=string;\n");
}

/// `TSSymbolKeyword`.
#[test]
fn ts_symbol_keyword_round_trips() {
    ts_case("type T = symbol;", "type T = symbol;\n", "type T=symbol;\n");
}

/// `TSVoidKeyword` — the one primitive spelled with a reserved word.
#[test]
fn ts_void_keyword_round_trips() {
    ts_case("type T = void;", "type T = void;\n", "type T=void;\n");
}

/// `TSUndefinedKeyword`.
#[test]
fn ts_undefined_keyword_round_trips() {
    ts_case(
        "type T = undefined;",
        "type T = undefined;\n",
        "type T=undefined;\n",
    );
}

/// `TSUnknownKeyword`.
#[test]
fn ts_unknown_keyword_round_trips() {
    ts_case("type T = unknown;", "type T = unknown;\n", "type T=unknown;\n");
}

/// `TSNeverKeyword`.
#[test]
fn ts_never_keyword_round_trips() {
    ts_case("type T = never;", "type T = never;\n", "type T=never;\n");
}

/// `TSBigIntKeyword`.
#[test]
fn ts_bigint_keyword_round_trips() {
    ts_case("type T = bigint;", "type T = bigint;\n", "type T=bigint;\n");
}

/// `TSThisType`.
#[test]
fn ts_this_type_round_trips() {
    ts_case("type T = this;", "type T = this;\n", "type T=this;\n");
}

// --- annotations, literals, references -------------------------------------

/// `TSTypeAnnotation` — the transparent `: T` wrapper. The generated text
/// pins that it adds no parens of its own (`let x: (A);` would still reparse
/// to the same tree under Flow, but not under TS).
#[test]
fn ts_type_annotation_round_trips() {
    ts_case("let x: A;", "let x: A;\n", "let x:A;\n");
}

/// `TSLiteralType` — all five literal kinds
/// `parse_ts_primary_type` can wrap, in one union.
#[test]
fn ts_literal_type_round_trips() {
    ts_case(
        "type T = 'lit' | 42 | 123n | true | null;",
        "type T = 'lit' | 42 | 123n | true | null;\n",
        "type T='lit'|42|123n|true|null;\n",
    );
}

/// `TSTypeReference` with a type-argument list.
#[test]
fn ts_type_reference_round_trips() {
    ts_case("type T = A<X>;", "type T = A<X>;\n", "type T=A<X>;\n");
}

/// `TSQualifiedName` — left-nested one node per `.`.
#[test]
fn ts_qualified_name_round_trips() {
    ts_case("type T = A.B.C;", "type T = A.B.C;\n", "type T=A.B.C;\n");
}

/// `TSTypeParameterInstantiation` — the `<X, Y>` on a reference.
#[test]
fn ts_type_parameter_instantiation_round_trips() {
    ts_case("type T = A<X, Y>;", "type T = A<X, Y>;\n", "type T=A<X,Y>;\n");
}

/// `TSTypeParameterDeclaration` — the `<X, Y>` on a generic.
#[test]
fn ts_type_parameter_declaration_round_trips() {
    // The compact spelling carries one space that looks gratuitous:
    // `type T<X,Y> =X;`. It is `gen.rs`'s `space_before_equals` (defect 35),
    // which refuses to let a `=` be munched onto a preceding `>`. See that
    // function's doc comment for why `>` is in its byte set even though the
    // parser happens to lex *this* `>` in `GrammarContext::Type`, where
    // `>=` is split: the same byte ends a self-closing JSX tag, where it is
    // not split, and a byte-level guard cannot tell the two apart.
    ts_case("type T<X, Y> = X;", "type T<X, Y> = X;\n", "type T<X,Y> =X;\n");
}

/// `TSTypeParameter` with both a constraint and a default.
#[test]
fn ts_type_parameter_round_trips() {
    ts_case(
        "type T<X extends A = B> = X;",
        "type T<X extends A = B> = X;\n",
        // The inner `=` (the type parameter's default) follows `A` and needs
        // no separator; only the outer one follows `>`. See
        // `ts_type_parameter_declaration_round_trips`.
        "type T<X extends A=B> =X;\n",
    );
}

// --- type constructors ------------------------------------------------------

/// `TSArrayType`.
#[test]
fn ts_array_type_round_trips() {
    ts_case("type T = A[];", "type T = A[];\n", "type T=A[];\n");
}

/// `TSIndexedAccessType`.
#[test]
fn ts_indexed_access_type_round_trips() {
    ts_case("type T = A['k'];", "type T = A['k'];\n", "type T=A['k'];\n");
}

/// `TSUnionType`.
#[test]
fn ts_union_type_round_trips() {
    ts_case("type T = A | B;", "type T = A | B;\n", "type T=A|B;\n");
}

/// `TSIntersectionType`.
#[test]
fn ts_intersection_type_round_trips() {
    ts_case("type T = A & B;", "type T = A & B;\n", "type T=A&B;\n");
}

/// `TSTupleType`.
#[test]
fn ts_tuple_type_round_trips() {
    ts_case("type T = [A, B];", "type T = [A, B];\n", "type T=[A,B];\n");
}

/// `TSTypeQuery` — `typeof` plus its own dotted-name loop. The compact form
/// pins that the space after `typeof` is forced: `typeofx.y` would lex as one
/// identifier.
#[test]
fn ts_type_query_round_trips() {
    ts_case(
        "type T = typeof x.y;",
        "type T = typeof x.y;\n",
        "type T=typeof x.y;\n",
    );
}

/// `TSConditionalType`.
#[test]
fn ts_conditional_type_round_trips() {
    ts_case(
        "type T = A extends B ? C : D;",
        "type T = A extends B ? C : D;\n",
        "type T=A extends B?C:D;\n",
    );
}

/// `TSFunctionType`, with an optional parameter.
#[test]
fn ts_function_type_round_trips() {
    ts_case(
        "type T = (a: A, b?: B) => C;",
        "type T = (a: A, b?: B) => C;\n",
        "type T=(a:A,b?:B)=>C;\n",
    );
}

/// `TSConstructorType`.
#[test]
fn ts_constructor_type_round_trips() {
    ts_case(
        "type T = new (a: A) => B;",
        "type T = new (a: A) => B;\n",
        "type T=new (a:A)=>B;\n",
    );
}

/// `TSTypePredicate` — `a is B` as a function type's return type. The compact
/// form pins the forced spaces around `is`.
#[test]
fn ts_type_predicate_round_trips() {
    ts_case(
        "type T = (a: A) => a is B;",
        "type T = (a: A) => a is B;\n",
        "type T=(a:A)=>a is B;\n",
    );
}

/// `TSParameterProperty`, with every modifier the parser's loop accepts, in
/// the canonical order this arm prints.
#[test]
fn ts_parameter_property_round_trips() {
    ts_case(
        "type T = (public static export readonly a: A) => B;",
        "type T = (public static export readonly a: A) => B;\n",
        "type T=(public static export readonly a:A)=>B;\n",
    );
}

// --- object types and their members ----------------------------------------

/// `TSTypeLiteral`.
#[test]
fn ts_type_literal_round_trips() {
    ts_case(
        "type T = { a: A };",
        "type T = {\n  a: A;\n};\n",
        "type T={a:A;};\n",
    );
}

/// `TSPropertySignature`, optional.
#[test]
fn ts_property_signature_round_trips() {
    ts_case(
        "type T = { a?: A };",
        "type T = {\n  a?: A;\n};\n",
        "type T={a?:A;};\n",
    );
}

/// `TSMethodSignature`.
#[test]
fn ts_method_signature_round_trips() {
    ts_case(
        "type T = { m(a: A): B };",
        "type T = {\n  m(a: A): B;\n};\n",
        "type T={m(a:A):B;};\n",
    );
}

/// `TSIndexSignature`.
#[test]
fn ts_index_signature_round_trips() {
    ts_case(
        "type T = { [k: string]: A };",
        "type T = {\n  [k: string]: A;\n};\n",
        "type T={[k:string]:A;};\n",
    );
}

/// `TSCallSignatureDeclaration`.
#[test]
fn ts_call_signature_declaration_round_trips() {
    ts_case(
        "type T = { (a: A): B };",
        "type T = {\n  (a: A): B;\n};\n",
        "type T={(a:A):B;};\n",
    );
}

// --- declarations -----------------------------------------------------------

/// `TSTypeAliasDeclaration` — the `;` comes from `visit_stmt_in_block`, not
/// from the arm, so this also pins that it is NOT in `stmt_skip_semi`.
#[test]
fn ts_type_alias_declaration_round_trips() {
    ts_case("type T = A;", "type T = A;\n", "type T=A;\n");
}

/// `TSInterfaceDeclaration` with type parameters and two heritage entries.
/// The absence of a trailing `;` pins that it IS in `stmt_skip_semi`.
#[test]
fn ts_interface_declaration_round_trips() {
    ts_case(
        "interface I<X> extends J<Y>, K { a: A }",
        "interface I<X> extends J<Y>, K {\n  a: A;\n}\n",
        "interface I<X> extends J<Y>,K{a:A;}\n",
    );
}

/// `TSInterfaceHeritage` — the parser moves the reference's own type
/// arguments onto the heritage node, so this pins that `J<Y>` comes back
/// whole rather than as a bare `J`.
#[test]
fn ts_interface_heritage_round_trips() {
    ts_case(
        "interface I extends J<Y> {}",
        "interface I extends J<Y> {}\n",
        "interface I extends J<Y>{}\n",
    );
}

/// `TSInterfaceDeclaration` used as a *type*, not a statement —
/// `parse_ts_primary_type`'s `rw_interface` arm. Only reachable in strict
/// mode, where `interface` lexes as the reserved word rather than a
/// contextual identifier; this is the fixture behind its `TS_PRIMARY_TYPE`
/// classification in `precedence.rs`.
#[test]
fn ts_interface_declaration_as_a_type_round_trips() {
    ts_case(
        "'use strict';\ntype T = interface Foo { a: A };",
        "'use strict';\ntype T = interface Foo {\n  a: A;\n};\n",
        "'use strict';type T=interface Foo{a:A;};\n",
    );
}

/// `TSInterfaceBody`.
#[test]
fn ts_interface_body_round_trips() {
    ts_case(
        "interface I { a: A }",
        "interface I {\n  a: A;\n}\n",
        "interface I{a:A;}\n",
    );
}

/// `TSEnumDeclaration` — `,`-separated members, unlike an object type's `;`.
#[test]
fn ts_enum_declaration_round_trips() {
    ts_case(
        "enum E { A, B = 2 }",
        "enum E {\n  A,\n  B = 2\n}\n",
        "enum E{A,B=2}\n",
    );
}

/// `TSEnumMember` with an initializer.
#[test]
fn ts_enum_member_round_trips() {
    ts_case("enum E { A = 'x' }", "enum E {\n  A = 'x'\n}\n", "enum E{A='x'}\n");
}

/// `TSModuleMember` (what `namespace X { … }` actually parses to) and
/// `TSModuleBlock` (its body) — one statement inside, so the block's
/// `visit_stmt_list` path is exercised rather than its `{}` shortcut.
#[test]
fn ts_module_member_and_module_block_round_trip() {
    ts_case(
        "namespace N { let x = 1; }",
        "namespace N {\n  let x = 1;\n}\n",
        "namespace N{let x=1;}\n",
    );
}

/// `TSModuleBlock`'s empty-body `{}` shortcut, with a dotted namespace name
/// (a `TSQualifiedName` in `id` position rather than a plain `Identifier`).
#[test]
fn ts_module_block_empty_with_qualified_name_round_trips() {
    ts_case("namespace N.M {}", "namespace N.M {}\n", "namespace N.M{}\n");
}

// --- expressions ------------------------------------------------------------

/// `TSAsExpression`.
#[test]
fn ts_as_expression_round_trips() {
    ts_case("let a = x as A;", "let a = x as A;\n", "let a=x as A;\n");
}

/// `TSTypeAssertion`.
#[test]
fn ts_type_assertion_round_trips() {
    ts_case("let b = <A>y;", "let b = <A>y;\n", "let b=<A>y;\n");
}

/// `TSModifiers` — always present on a TS class property, and printed as two
/// halves either side of `static` (accessibility before, `readonly` after),
/// which is the only order our parser accepts.
#[test]
fn ts_modifiers_round_trip_in_accessibility_static_readonly_order() {
    ts_case(
        "class C { public static readonly x: A = 1; }",
        "class C {\n  public static readonly x: A = 1;\n  }\n\n",
        "class C{public static readonly x:A=1;}\n",
    );
}

/// `TSModifiers` on a `ClassPrivateProperty` — the other of the two class
/// members that carries the field (a private name can never take an
/// accessibility modifier, so only the `readonly` half can be set here).
#[test]
fn ts_modifiers_on_a_private_class_property_round_trip() {
    ts_case(
        "class C { readonly #p = 1; }",
        "class C {\n  readonly #p = 1;\n  }\n\n",
        "class C{readonly #p=1;}\n",
    );
}

// ---------------------------------------------------------------------------
// Task 13 parenthesization regressions. Each one FAILS (with the exact
// wrong output recorded in the relevant doc comment) if its fix is reverted;
// see `task-13-report.md` for the mutation transcripts.
// ---------------------------------------------------------------------------

/// `TSArrayType::element_type` is a *primary*-tier position
/// (`parse_ts_postfix_type`), so a function-type base keeps its parens.
/// Without `print_child`, this prints `(a: A) => B[]`, which reparses as one
/// `TSFunctionType` returning `B[]`.
#[test]
fn ts_array_type_keeps_parens_around_a_function_type_element() {
    ts_case(
        "type T = ((a: A) => B)[];",
        "type T = ((a: A) => B)[];\n",
        "type T=((a:A)=>B)[];\n",
    );
}

/// Same for `TSIndexedAccessType::object_type`.
#[test]
fn ts_indexed_access_type_keeps_parens_around_a_function_type_object() {
    ts_case(
        "type T = ((a: A) => B)['k'];",
        "type T = ((a: A) => B)['k'];\n",
        "type T=((a:A)=>B)['k'];\n",
    );
}

/// A union member is parsed at *intersection* tier, so a function-type member
/// keeps its parens — in the middle of the list, where dropping them lets the
/// function type's return type swallow the rest of the union.
#[test]
fn ts_union_type_keeps_parens_around_a_function_type_member() {
    ts_case(
        "type T = A | ((a: X) => Y) | B;",
        "type T = A | ((a: X) => Y) | B;\n",
        "type T=A|((a:X)=>Y)|B;\n",
    );
}

/// Same for an intersection member, parsed at *postfix* tier.
#[test]
fn ts_intersection_type_keeps_parens_around_a_function_type_member() {
    ts_case(
        "type T = A & ((a: X) => Y) & B;",
        "type T = A & ((a: X) => Y) & B;\n",
        "type T=A&((a:X)=>Y)&B;\n",
    );
}

/// `TSConditionalType::check_type` accepts union tier and tighter only, so a
/// function type there keeps its parens. Without the `need_parens` threshold
/// branch this prints `(a: X) => Y extends B ? C : D`, in which the function
/// type's *return type* takes the whole `extends` clause.
#[test]
fn ts_conditional_type_check_type_keeps_parens_around_a_function_type() {
    ts_case(
        "type T = ((a: X) => Y) extends B ? C : D;",
        "type T = ((a: X) => Y) extends B ? C : D;\n",
        "type T=((a:X)=>Y) extends B?C:D;\n",
    );
}

/// A union member that is *itself* a union can only have got there through
/// explicit source parens, so it keeps them — `ChildPos::Anywhere` is what
/// makes an equal-precedence member wrap. (Our parser cannot currently read
/// `A | (B | C)` back — its `(`-cover only accepts a parenthesized type whose
/// contents themselves start with `(` — so this asserts on the generated text
/// alone rather than round-tripping; see `task-13-report.md`'s "parser
/// limitations met along the way".)
#[test]
fn ts_union_type_member_that_is_itself_a_union_keeps_its_parens() {
    // Reachable shape: the inner union is a *function type's return type*,
    // which the outer union then wraps. Built by nesting rather than by
    // source parens, since the parser cannot read the parenthesized form.
    let js = ts_round_trip(
        "type T = A | ((a: X) => Y | Z) | B;",
        Pretty::No,
    );
    assert_eq!(js, "type T=A|((a:X)=>Y|Z)|B;\n");
}

/// A typed arrow's return type must NOT be parenthesized. juno's arrow arm
/// prints it through `print_child` and neither juno nor this crate classified
/// the `TypeAnnotation`/`TSTypeAnnotation` wrapper, so it landed in
/// `ALWAYS_PAREN`: before the fix this printed `let f = (a: A): (B) => a;`,
/// which our TS parser rejects outright (`';' expected`).
#[test]
fn ts_typed_arrow_return_type_keeps_no_parens() {
    ts_case(
        "let f = (a: A): B => a;",
        "let f = (a: A): B => a;\n",
        "let f=(a:A):B=>a;\n",
    );
}

/// The Flow half of the same fix. Here the bad output *did* reparse to the
/// identical tree (a parenthesized Flow type is still that type), so only the
/// text pin catches it.
#[test]
fn flow_typed_arrow_return_type_keeps_no_parens() {
    for_each_pretty_mode(|pretty| {
        let mut parsed = parse_ok_flow("let f = (a: number): string => a;");
        let js = gen(
            &mut parsed,
            Opt {
                pretty,
                ..Opt::default()
            },
        );
        assert_eq!(
            js,
            match pretty {
                Pretty::Yes => "let f = (a: number): string => a;\n",
                Pretty::No => "let f=(a:number):string=>a;\n",
            },
            "{pretty:?}"
        );
    });
}

/// An `as`-expression's right operand is a *type*, and the type grammar keeps
/// reading `|`/`&`/`<` past the end of it — so the whole as-expression must be
/// parenthesized under any binary operator. `<` is the case that does not even
/// reparse without the parens.
#[test]
fn ts_as_expression_operand_of_bitwise_and_relational_operators_keeps_parens() {
    ts_case(
        "let a = (x as A) | B;",
        "let a = (x as A) | B;\n",
        "let a=(x as A)|B;\n",
    );
    ts_case(
        "let a = (x as A) & B;",
        "let a = (x as A) & B;\n",
        "let a=(x as A)&B;\n",
    );
    ts_case(
        "let a = (x as A) < B;",
        "let a = (x as A) < B;\n",
        "let a=(x as A)<B;\n",
    );
    // The grandparent case: the token that follows `x as A` here belongs to
    // the OUTER `|`, which is why the fix is a precedence-table entry rather
    // than a `need_parens` branch keyed on the direct parent's operator.
    ts_case(
        "let a = b | (x as A) | c;",
        "let a = b | (x as A) | c;\n",
        "let a=b|(x as A)|c;\n",
    );
}

/// The Flow half of the same defect — the one Task 12 actually shipped.
#[test]
fn as_expression_operand_of_bitwise_and_relational_operators_keeps_parens() {
    for_each_pretty_mode(|pretty| {
        for (src, expected_pretty, expected_compact) in [
            ("(x as A) | B;", "(x as A) | B;\n", "(x as A)|B;\n"),
            ("(x as A) & B;", "(x as A) & B;\n", "(x as A)&B;\n"),
            ("(x as A) < B;", "(x as A) < B;\n", "(x as A)<B;\n"),
            (
                "b | (x as A) | c;",
                "b | (x as A) | c;\n",
                "b|(x as A)|c;\n",
            ),
            (
                "(x as const) | B;",
                "(x as const) | B;\n",
                "(x as const)|B;\n",
            ),
        ] {
            let mut parsed = parse_ok_flow(src);
            let js = gen(
                &mut parsed,
                Opt {
                    pretty,
                    ..Opt::default()
                },
            );
            assert_eq!(
                js,
                match pretty {
                    Pretty::Yes => expected_pretty,
                    Pretty::No => expected_compact,
                },
                "{pretty:?} {src:?}"
            );
            hermes_parser::parse(&js, flow_flags())
                .unwrap_or_else(|e| panic!("{pretty:?} {src:?}: {js:?} must reparse: {e:?}"));
        }
    });
}

/// An as-expression stays bare wherever no operator token can follow it —
/// the counterpart to the test above, guarding against over-parenthesizing.
#[test]
fn ts_as_expression_stays_bare_where_nothing_can_follow_it() {
    ts_case("let a = x as A;", "let a = x as A;\n", "let a=x as A;\n");
    ts_case(
        "let a = x as A as B;",
        "let a = x as A as B;\n",
        "let a=x as A as B;\n",
    );
    ts_case(
        "let a = x as A ? b : c;",
        "let a = x as A ? b : c;\n",
        "let a=x as A?b:c;\n",
    );
    ts_case(
        "let a = () => x as A;",
        "let a = () => x as A;\n",
        "let a=()=>x as A;\n",
    );
    ts_case("let a = [x as A];", "let a = [x as A];\n", "let a=[x as A];\n");
    // NOT a call argument: `f(x as A)` prints `f((x as A))`, because juno
    // routes every call argument through `print_child` against the call's own
    // `MEMBER` precedence (`gen_js.rs:955-964`, ported verbatim in
    // `arms/expr.rs`), which wraps *any* non-primary argument — `f(a + b)`
    // prints `f((a + b))` too, and has since Task 5. Pre-existing and
    // unrelated to this kind; noted here so a future reader does not mistake
    // it for an as-expression problem.
}

/// `TSAsExpression::expression` is the `as` operator's left operand, so
/// anything looser than the as-expression's own tier keeps its parens.
#[test]
fn ts_as_expression_left_operand_keeps_parens_for_looser_expressions() {
    ts_case(
        "let a = (b ? c : d) as A;",
        "let a = (b ? c : d) as A;\n",
        "let a=(b?c:d) as A;\n",
    );
    ts_case(
        "let a = (b = c) as A;",
        "let a = (b = c) as A;\n",
        "let a=(b=c) as A;\n",
    );
    ts_case(
        "let a = (() => 1) as A;",
        "let a = (() => 1) as A;\n",
        "let a=(()=>1) as A;\n",
    );
}

/// `TSTypeAssertion::expression` is a `parse_unary_expression` position:
/// looser operands keep their parens, and unary-or-tighter ones stay bare.
#[test]
fn ts_type_assertion_operand_parenthesizes_only_looser_expressions() {
    ts_case(
        "let a = <A>(a + b);",
        "let a = <A>(a + b);\n",
        "let a=<A>(a+b);\n",
    );
    ts_case(
        "let a = <A>(a = b);",
        "let a = <A>(a = b);\n",
        "let a=<A>(a=b);\n",
    );
    // Unary tier and tighter stay bare.
    ts_case("let a = <A>-x;", "let a = <A>-x;\n", "let a=<A>-x;\n");
    ts_case("let a = <A><B>x;", "let a = <A><B>x;\n", "let a=<A><B>x;\n");
    ts_case("let a = <A>x.y;", "let a = <A>x.y;\n", "let a=<A>x.y;\n");
    ts_case("let a = <A>x++;", "let a = <A>x++;\n", "let a=<A>x++;\n");
}

/// A `TSEnumMember` initializer is printed through `print_comma_expression`,
/// so a `SequenceExpression` keeps parens and its comma cannot end the
/// member. (The doubled parens are `gen_sequence_expression` printing its own
/// pair on top — the same pre-existing double-wrap `[(a, b)]` and every other
/// `print_comma_expression` site in this crate already produces.)
#[test]
fn ts_enum_member_sequence_initializer_keeps_parens() {
    ts_case(
        "enum E { A = (1, 2) }",
        "enum E {\n  A = ((1, 2))\n}\n",
        "enum E{A=((1,2))}\n",
    );
}

// ---------------------------------------------------------------------------
// Task 13 fix round 1: an expression-space node in a TypeScript type slot.
//
// `parse_ts_function_or_parenthesized_type` routes the contents of `(`
// through the type grammar only when they themselves start with `(`
// (`crates/parser/src/js/ts/function_types.rs`); everything else goes to
// `parse_binding_element` and, absent a `=>`, is handed back AS the type. So
// `type T = ({ a: A })[];` parses to `TSArrayType { element_type:
// ObjectPattern }` — a faithful port of the C++, and upstream behaves the
// same. Printed bare, that regenerates as `type T={a:A}[];`, which reparses
// to a `TSTypeLiteral`: a different tree, no diagnostic.
//
// Precedence could not catch it. `ObjectPattern` is `PRIMARY` (32) in the
// *expression* numbering space and every TS tier is 1-6, so `32 >= tier`
// always held. The fix is `precedence.rs`'s `is_ts_type_node` allow-list plus
// the two `is_*_ts_type_field` position predicates, and routing every TS
// type-position field through `print_child` so the rule can run at all.
// ---------------------------------------------------------------------------

/// An `ObjectPattern` reaching a **narrowed** TS type field keeps its parens.
/// These five are the reviewer's original cases for those fields.
#[test]
fn ts_object_pattern_in_a_narrowed_type_field_keeps_its_parens() {
    ts_case(
        "type T = ({ a: A })[];",
        "type T = ({a: A})[];\n",
        "type T=({a:A})[];\n",
    );
    ts_case(
        "type T = ({ a: A })['k'];",
        "type T = ({a: A})['k'];\n",
        "type T=({a:A})['k'];\n",
    );
    ts_case(
        "type T = ({ a: A }) | B;",
        "type T = ({a: A}) | B;\n",
        "type T=({a:A})|B;\n",
    );
    ts_case(
        "type T = ({ a: A }) & B;",
        "type T = ({a: A}) & B;\n",
        "type T=({a:A})&B;\n",
    );
    ts_case(
        "type T = ({ a: A }) extends B ? C : D;",
        "type T = ({a: A}) extends B ? C : D;\n",
        "type T=({a:A}) extends B?C:D;\n",
    );
    // Shorthand properties, which print differently but break identically.
    ts_case(
        "type T = ({ a, b })[];",
        "type T = ({a, b})[];\n",
        "type T=({a,b})[];\n",
    );
}

/// The same hazard in the **full**-`Type` fields, which until this fix printed
/// with a bare `gen_node` on the (wrong) reasoning that "nothing can need
/// parens" there. Every field in `is_full_ts_type_field`'s match is
/// represented.
#[test]
fn ts_object_pattern_in_a_full_type_field_keeps_its_parens() {
    for (src, pretty_text, compact_text) in [
        // TSTypeAliasDeclaration::type_annotation
        ("type T = ({ a: A });", "type T = ({a: A});\n", "type T=({a:A});\n"),
        // TSIndexedAccessType::index_type
        (
            "type T = A[({ b: B })];",
            "type T = A[({b: B})];\n",
            "type T=A[({b:B})];\n",
        ),
        // TSTupleType::element_types
        (
            "type T = [({a: A})];",
            "type T = [({a: A})];\n",
            "type T=[({a:A})];\n",
        ),
        // TSConditionalType::true_type (extends_type/false_type in the corpus)
        (
            "type T = A extends B ? ({ c: C }) : D;",
            "type T = A extends B ? ({c: C}) : D;\n",
            "type T=A extends B?({c:C}):D;\n",
        ),
        // TSTypeParameterInstantiation::params
        (
            "type T = A<({ b: B })>;",
            "type T = A<({b: B})>;\n",
            "type T=A<({b:B})>;\n",
        ),
        // TSTypeParameter::constraint
        (
            "type T<X extends ({ b: B })> = X;",
            "type T<X extends ({b: B})> = X;\n",
            "type T<X extends ({b:B})> =X;\n",
        ),
        // TSFunctionType::return_type
        (
            "type T = () => ({ a: A });",
            "type T = () => ({a: A});\n",
            "type T=()=>({a:A});\n",
        ),
        // TSPropertySignature::type_annotation
        (
            "type T = { p: ({ a: A }) };",
            "type T = {\n  p: ({a: A});\n};\n",
            "type T={p:({a:A});};\n",
        ),
        // TSAsExpression::type_annotation
        (
            "let y = x as ({ a: A });",
            "let y = x as ({a: A});\n",
            "let y=x as ({a:A});\n",
        ),
        // TSTypeAssertion::type_annotation
        (
            "let y = <({ a: A })>x;",
            "let y = <({a: A})>x;\n",
            "let y=<({a:A})>x;\n",
        ),
        // TSTypePredicate::type_annotation
        (
            "type T = (a: A) => a is ({ b: B });",
            "type T = (a: A) => a is ({b: B});\n",
            "type T=(a:A)=>a is ({b:B});\n",
        ),
        // TSTypeAnnotation::type_annotation — the `: T` choke point, reached
        // here through `Identifier::type_annotation`.
        (
            "let x: ({ a: A })[];",
            "let x: ({a: A})[];\n",
            "let x:({a:A})[];\n",
        ),
        (
            "type T = (a: ({b: B})) => C;",
            "type T = (a: ({b: B})) => C;\n",
            "type T=(a:({b:B}))=>C;\n",
        ),
    ] {
        ts_case(src, pretty_text, compact_text);
    }
}

/// The other two intruder kinds the `(`-cover can hand back. `ArrayPattern`
/// used to survive the *narrowed* fields only by accident — it is
/// unclassified, so it fell into `ALWAYS_PAREN` and was always wrapped — but
/// broke in the full fields all the same; `AssignmentPattern` broke
/// everywhere. Neither is an accident now: both are simply absent from
/// `is_ts_type_node`'s allow-list.
#[test]
fn ts_array_and_assignment_patterns_in_a_type_position_keep_their_parens() {
    ts_case(
        "type T = ([a, b]);",
        "type T = ([a, b]);\n",
        "type T=([a,b]);\n",
    );
    ts_case(
        "type T = ([a, b]) | B;",
        "type T = ([a, b]) | B;\n",
        "type T=([a,b])|B;\n",
    );
    ts_case("type T = (a = 1);", "type T = (a = 1);\n", "type T=(a=1);\n");
}

/// The complement: a genuine TS type in a full-`Type` field must NOT gain
/// parens from the new rule. `is_ts_type_node` returning `true` makes
/// `need_parens` answer `No` outright there, so this pins that routing those
/// fields through `print_child` changed nothing for real types — including
/// the two shapes that would be actively broken by a wrap:
/// `*` (`ExistsTypeAnnotation`, which the TS grammar produces and which
/// `(*)` cannot re-parse) and a nested conditional in `true_type`.
#[test]
fn ts_genuine_types_in_full_type_fields_gain_no_parens() {
    ts_case("type T = *;", "type T = *;\n", "type T=*;\n");
    ts_case("type T = A[];", "type T = A[];\n", "type T=A[];\n");
    ts_case(
        "type T = A extends B ? C extends D ? E : F : G;",
        "type T = A extends B ? C extends D ? E : F : G;\n",
        "type T=A extends B?C extends D?E:F:G;\n",
    );
    ts_case(
        "type T = () => A | B;",
        "type T = () => A | B;\n",
        "type T=()=>A|B;\n",
    );
    ts_case(
        "type T = A<B | C, (d: D) => E>;",
        "type T = A<B | C, (d: D) => E>;\n",
        "type T=A<B|C,(d:D)=>E>;\n",
    );
    ts_case(
        "type T = [A | B, (c: C) => D];",
        "type T = [A | B, (c: C) => D];\n",
        "type T=[A|B,(c:C)=>D];\n",
    );
}

// ---------------------------------------------------------------------------
// The Task 13 parenthesization audit, kept in-tree so it stays reproducible.
//
// Every source below was run through parse -> generate -> reparse ->
// full-ESTree comparison in BOTH pretty modes while deriving `arms/ts.rs`,
// and every one of the six `print_child` decisions in that module was then
// mutation-tested by reverting it to a bare `gen_node` and watching specific
// entries here fail (transcripts in `task-13-report.md`). The named per-kind
// tests above are the primary coverage; this is the breadth net that found
// the two `precedence.rs` defects Task 13 fixed.
// ---------------------------------------------------------------------------

/// The TypeScript sources the audit covers. Deliberately *not* including the
/// shapes our parser cannot currently read at all — see
/// `task-13-report.md`'s "parser limitations met along the way" for the 13
/// that were tried and rejected by the parser itself (parenthesized unions,
/// intersections and conditional types; a parenthesized constructor type; a
/// generic method signature; `interface` as a type expression).
const TS_AUDIT_CORPUS: &[&str] = &[
    // --- keywords / primaries -------------------------------------------
    "type T = any;",
    "type T = number;",
    "type T = boolean;",
    "type T = string;",
    "type T = symbol;",
    "type T = void;",
    "type T = undefined;",
    "type T = unknown;",
    "type T = never;",
    "type T = bigint;",
    "type T = this;",
    "type T = *;",
    // --- literal types ---------------------------------------------------
    "type T = 'lit';",
    "type T = 42;",
    "type T = 123n;",
    "type T = true;",
    "type T = false;",
    "type T = null;",
    "type T = 1e21;",
    "type T = 0.5;",
    // --- references / qualified names ------------------------------------
    "type T = A;",
    "type T = A.B;",
    "type T = A.B.C;",
    "type T = A<X>;",
    "type T = A.B<X, Y>;",
    "type T = A<B<C>>;",
    // --- postfix ---------------------------------------------------------
    "type T = A[];",
    "type T = A[][];",
    "type T = A['k'];",
    "type T = A['k']['j'];",
    "type T = A['k'][];",
    "type T = A[][ 'k' ];",
    "type T = ((a: A) => B)[];",
    "type T = ((a: A) => B)['k'];",
    "type T = A[B | C];",
    "type T = A[(a: X) => Y];",
    "type T = typeof x[];",
    "type T = { a: A }[];",
    "type T = [A, B][];",
    // --- union / intersection --------------------------------------------
    "type T = A | B;",
    "type T = A & B;",
    "type T = A | B & C;",
    "type T = A | ((a: X) => Y);",
    "type T = ((a: X) => Y) | A;",
    "type T = A & ((a: X) => Y);",
    "type T = A | B | C | D;",
    "type T = | A | B;",
    "type T = ((a: X) => Y) & A;",
    "type T = A & ((a: X) => Y) & B;",
    "type T = A | ((a: X) => Y) | B;",
    "type T = A[] | B[];",
    // --- function / constructor types ------------------------------------
    "type T = () => void;",
    "type T = (a: A) => B;",
    "type T = (a: A, b?: B) => C;",
    "type T = (...rest: A) => B;",
    "type T = (a: A, ...rest: B) => C;",
    "type T = (this: A, b: B) => C;",
    "type T = <X>(a: X) => X;",
    "type T = <X, Y = Z>(a: X) => Y;",
    "type T = new (a: A) => B;",
    "type T = new <X>(a: X) => X;",
    "type T = (a: A) => (b: B) => C;",
    "type T = ({ a, b }: A) => B;",
    "type T = ([a, b]: A) => B;",
    "type T = (a: A) => B | C;",
    // --- parameter properties --------------------------------------------
    "type T = (readonly a: A) => B;",
    "type T = (public a: A) => B;",
    "type T = (private a: A) => B;",
    "type T = (protected a: A) => B;",
    "type T = (static a: A) => B;",
    "type T = (export a: A) => B;",
    "type T = (public static export readonly a: A) => B;",
    // --- predicates ------------------------------------------------------
    "type T = (a: A) => a is B;",
    "type T = (a: A) => a is B | C;",
    // --- conditional types -----------------------------------------------
    "type T = A extends B ? C : D;",
    "type T = A extends B ? C : D extends E ? F : G;",
    "type T = A | B extends C ? D : E;",
    "type T = ((a: X) => Y) extends B ? C : D;",
    "type T = A extends B ? C | D : E & F;",
    // --- tuples ----------------------------------------------------------
    "type T = [];",
    "type T = [A];",
    "type T = [A, B];",
    "type T = [A | B, (c: C) => D];",
    "type T = [A extends B ? C : D];",
    // --- type queries ----------------------------------------------------
    "type T = typeof x;",
    "type T = typeof x.y.z;",
    "type T = typeof x | A;",
    // --- object types ----------------------------------------------------
    "type T = {};",
    "type T = { a: A };",
    "type T = { a: A; b: B };",
    "type T = { a?: A };",
    "type T = { a };",
    "type T = { [k: string]: A };",
    "type T = { [k: string, j: number]: A };",
    "type T = { (a: A): B };",
    "type T = { (a: A) };",
    "type T = { m(a: A): B };",
    "type T = { m() };",
    "type T = { ['computed']: A };",
    "type T = { ['computed']?: A };",
    "type T = { a: A } | B;",
    "type T = { a: { b: B } };",
    // --- interface as a type ---------------------------------------------
    // --- type aliases / params -------------------------------------------
    "type T<X> = X;",
    "type T<X extends A> = X;",
    "type T<X = A> = X;",
    "type T<X extends A = B> = X;",
    "type T<X, Y> = X;",
    "type T<X extends A | B> = X;",
    "type T<X extends (a: A) => B> = X;",
    // --- interface as a type (strict mode only) ---------------------------
    "'use strict';\ntype T = interface Foo { a: A };",
    "'use strict';\ntype T = interface Foo { a: A }[];",
    // --- interface declarations ------------------------------------------
    "interface I {}",
    "interface I { a: A }",
    "interface I<X> { a: X }",
    "interface I extends J {}",
    "interface I extends J, K {}",
    "interface I extends J<X> {}",
    "interface I<X> extends J<X>, K { m(): void }",
    // --- enums -----------------------------------------------------------
    "enum E {}",
    "enum E { A }",
    "enum E { A, B }",
    "enum E { A = 1, B = 2 }",
    "enum E { A = 'x' }",
    "enum E { A = (1, 2) }",
    "enum E { A = f() }",
    // --- namespaces ------------------------------------------------------
    "namespace N {}",
    "namespace N { let x = 1; }",
    "namespace N.M { let x = 1; }",
    "namespace N { namespace M { let x = 1; } }",
    "namespace N { function f() {} }",
    // --- as-expressions --------------------------------------------------
    "let a = x as A;",
    "let a = x as A | B;",
    "let a = (x as A) | B;",
    "let a = (x as A) & B;",
    "let a = (x as A) < B;",
    "let a = b | (x as A) | c;",
    "let a = (x as A) + 1;",
    "let a = (x as A).b;",
    "let a = (x as A)[0];",
    "let a = (x as A)();",
    "let a = typeof (x as A);",
    "let a = x as A as B;",
    "let a = (x as A) ? b : c;",
    "let a = y = x as A;",
    "let a = (x, y as A);",
    "let a = () => x as A;",
    "let a = [x as A];",
    "let a = f(x as A);",
    "let a = x as (a: A) => B;",
    "let a = x as A[];",
    "let a = (b ? c : d) as A;",
    "let a = (b = c) as A;",
    "let a = (b, c) as A;",
    "let a = (() => 1) as A;",
    "let a = (b + c) as A;",
    "let a = x as const;",
    // --- type assertions -------------------------------------------------
    "let a = <A>x;",
    "let a = <A>-x;",
    "let a = <A><B>x;",
    "let a = <A>(a + b);",
    "let a = <A>(a, b);",
    "let a = <A>(a = b);",
    "let a = <A>x.y;",
    "let a = <A>x++;",
    "let a = <A | B>x;",
    "let a = <(a: A) => B>x;",
    "let a = <A<B>>x;",
    "let a = <A>x + 1;",
    "(<A>x).y;",
    // --- class members ---------------------------------------------------
    "class C { x = 1; }",
    "f(a + b);",
    "f(a, b);",
    "f(x as A);",
    "f(a ? b : c);",
    "class C { x: A = 1; }",
    "class C { static x = 1; }",
    "class C { public x = 1; }",
    "class C { private x = 1; }",
    "class C { protected x = 1; }",
    "class C { readonly x = 1; }",
    "class C { public static readonly x: A = 1; }",
    "class C { #p = 1; }",
    "class C { readonly #p = 1; }",
    "class C { static readonly #p = 1; }",
    "class C { m(): A {} }",
    "class C { m(a: A): B {} }",
    "class C<X> { m(a: X): X {} }",
    "class C extends D {}",
    "class C { x?: A; }",
    // --- functions with TS annotations ------------------------------------
    "function f(a: A): B {}",
    "function f<X>(a: X): X {}",
    "function f(a: A = 1): B {}",
    "let f = (a: A): B => a;",
    "function f(a: A): a is B {}",
    // --- expression-space intruders in TS type slots (fix round 1) --------
    // `parse_ts_function_or_parenthesized_type` hands `parse_binding_element`
    // results back AS the type when no `=>` follows, so these are the shapes
    // that put an `ObjectPattern`/`ArrayPattern`/`AssignmentPattern` in a
    // type field. Absent from the original corpus, which is what made the
    // family invisible.
    "type T = ({ a: A })[];",
    "type T = ({ a: A })['k'];",
    "type T = ({ a: A }) | B;",
    "type T = ({ a: A }) & B;",
    "type T = ({ a, b })[];",
    "type T = ({ a: A }) extends B ? C : D;",
    "type T = ({ a: A });",
    "type T = ({});",
    "type T = A[({ b: B })];",
    "type T = [({a: A})];",
    "type T = A extends ({ b: B }) ? C : D;",
    "type T = A extends B ? ({ c: C }) : D;",
    "type T = ({ a: A }) extends ({ b: B }) ? ({ c: C }) : ({ d: D });",
    "type T = A<({ b: B })>;",
    "type T = A<({ b: B }), C>;",
    "type T<X extends ({ b: B })> = X;",
    "type T<X = ({ b: B })> = X;",
    "type T = () => ({ a: A });",
    "type T = new () => ({ a: A });",
    "type T = (a: A) => a is ({ b: B });",
    "type T = { p: ({ a: A }) };",
    "type T = { m(): ({ a: A }) };",
    "type T = { (): ({ a: A }) };",
    "type T = { m(a: ({ b: B })) };",
    "type T = { [k: string]: ({ a: A }) };",
    "type T = ({ a: A })[] | B;",
    "type T = typeof x | ({ a: A });",
    "type T = (a: ({b: B})) => C;",
    "type T = ([A, B])[];",
    "type T = ([A])[];",
    "type T = ([a, b]);",
    "type T = ([a, b]) | B;",
    "type T = [([a, b])];",
    "type T = A<([a, b])>;",
    "type T = ([]);",
    "type T = (a = 1);",
    "type T = ((a = 1))[];",
    "type T = (A)[];",
    "type T = ((A))[];",
    "let x: ({ a: A })[];",
    "let x: ({ a: A });",
    "let x: ([a, b]);",
    "let y = x as ({ a: A });",
    "let y = <({ a: A })>x;",
    "function f(): ({ a: A }) {}",
    "function f(a: ({ b: B })) {}",
    "class C { x: ({ a: A }); }",
    "class C { x: ({ a: A }) = 1; }",
    "class C { m(a: ({ b: B })): ({ c: C }) {} }",
    "for (let i: ({ a: A }) of z) {}",
    "namespace N { type T = ({ a: A }); }",
    // --- genuine types in the same fields, guarding against over-wrapping --
    "type T = *;",
    "type T = A extends B ? C extends D ? E : F : G;",
    "type T = A<B | C, (d: D) => E>;",
    // --- variable / misc --------------------------------------------------
    "let x: A;",
    "let x: A | B = y;",
    "let x: (a: A) => B;",
    "for (let i: number = 0; i < 10; i++) {}",
];

/// Every source in [`TS_AUDIT_CORPUS`], in both pretty modes, must regenerate
/// to source that reparses to a **structurally identical** AST.
///
/// Reports every failure at once rather than stopping at the first, so a
/// regression shows its whole blast radius.
#[test]
fn ts_audit_corpus_round_trips_structurally() {
    let mut failures = Vec::new();
    for src in TS_AUDIT_CORPUS {
        for pretty in [Pretty::Yes, Pretty::No] {
            let mut parsed = hermes_parser::parse(src, ts_flags())
                .unwrap_or_else(|e| panic!("{src:?} must parse under -parse-ts: {e:?}"));
            let before = ast_json(&mut parsed);
            let js = gen(
                &mut parsed,
                Opt {
                    pretty,
                    ..Opt::default()
                },
            );
            match hermes_parser::parse(&js, ts_flags()) {
                Err(e) => failures.push(format!("{pretty:?} {src:?} -> {js:?}: {e:?}")),
                Ok(mut reparsed) => {
                    if ast_json(&mut reparsed) != before {
                        failures.push(format!(
                            "{pretty:?} {src:?} -> {js:?}: reparses to a DIFFERENT AST"
                        ));
                    }
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} audit cases regressed:\n{}",
        failures.len(),
        TS_AUDIT_CORPUS.len() * 2,
        failures.join("\n")
    );
}

// ===========================================================================
// juno's own round-trip suite, ported.
//
// Task 15, Steps 1-2 of `doc/superpowers/plans/2026-08-15-gen-js-port.md`.
// The source is `unsupported/juno/crates/juno/tests/gen_js/mod.rs` (746
// lines): its four harness functions (`:13-70`) and every one of its
// `test_roundtrip*` cases (`:87-570`). juno's two sourcemap tests
// (`test_sourcemap` `:572-640`, `test_sourcemap_merged` `:642-746`) are
// deleted, not ported: this crate drops sourcemap generation outright (spec
// §6, and the plan's Adaptation Rules row `out_token!` -> `out!`), so there
// is no `SourceMap` to assert on. Every other case is here. Nothing was
// dropped for failing.
//
// juno's `test_literals` also opens with a hand-built `StringLiteral`
// (`:89-106`) fed straight to `do_gen`, checking that `['A', U+1234, TAB]`
// prints as `'Aሴ\t'`. Ours reaches the same node through the parser
// rather than through `builder::StringLiteral::build_template`, which keeps
// this file free of AST-construction machinery; see
// [`juno_literal_escaping_matches_junos_hand_built_case`].
// ===========================================================================

/// juno `tests/gen_js/mod.rs:13-27` — `do_gen`. Generate `parsed`'s program
/// under `pretty` with otherwise default options.
///
/// Distinct from this file's older [`gen`] only in taking a [`Pretty`]
/// rather than a whole [`Opt`], matching juno's signature.
fn do_gen(parsed: &mut ParsedJS, pretty: Pretty) -> String {
    gen(
        parsed,
        Opt {
            pretty,
            ..Opt::default()
        },
    )
}

/// The AST oracle for the ported cases: the ESTree dump with `"raw"`
/// **omitted**, and no locations.
///
/// juno compares `dump_json(.., Pretty::Yes)` of the two trees
/// (`mod.rs:35-56`). Ours drops one property juno's dumper does not emit at
/// all: `"raw"`, the verbatim source text of a numeric literal. A generator
/// prints a `NumericLiteral` from its `f64` value, so `50.` comes back as
/// `50`, `0x10293` as `66195`, and `1e100` as `1e+100` — the `value` is
/// identical, only the original spelling is gone. No correct generator can
/// preserve it, and the C++ round-trip harness normalizes it away the same
/// way (`-Xinclude-raw-ast-prop=0`). This is the only normalization; the
/// whole rest of the tree is compared byte for byte.
///
/// This matters for real cases here: juno's `test_members` deliberately
/// includes `50..toString()`, `1e100.toString()`, and `0x10293.toString()`.
fn juno_ast_json(parsed: &mut ParsedJS) -> String {
    parsed.to_estree_json_with(
        true,
        hermes_ast::dump::ESTreeDumpMode::HideEmpty,
        hermes_ast::dump::LocationDumpMode::None,
        hermes_ast::dump::ESTreeRawProp::Exclude,
    )
}

/// juno `tests/gen_js/mod.rs:30-58` — `test_roundtrip_with_flags`.
///
/// Parse `src1` under `flags`, generate it back under **both** [`Pretty`]
/// modes, reparse each, and require the reparsed AST to equal the original.
/// Panics naming the mode, the original source, and the generated source —
/// juno's message, kept verbatim in spirit because it is the one thing you
/// read when a case breaks.
#[track_caller]
fn test_roundtrip_with_flags(flags: ParseFlags, src1: &str) {
    for pretty in [Pretty::Yes, Pretty::No] {
        let mut ast1 = hermes_parser::parse(src1, flags)
            .unwrap_or_else(|e| panic!("Original source must parse:\n{src1}\n{e:?}"));
        let ast1_json = juno_ast_json(&mut ast1);

        let src2 = do_gen(&mut ast1, pretty);
        let mut ast2 = hermes_parser::parse(&src2, flags).unwrap_or_else(|e| {
            panic!(
                "Invalid JS generated: Pretty={pretty:?}\n\
                 Original Source:\n{src1}\n\
                 Generated Source:\n{src2}\n\
                 Parse error: {e:?}"
            )
        });
        let ast2_json = juno_ast_json(&mut ast2);

        assert_eq!(
            ast1_json, ast2_json,
            "AST mismatch: Pretty={pretty:?}\n\
             Original Source:\n{src1}\n\
             Generated Source:\n{src2}"
        );
    }
}

/// juno `tests/gen_js/mod.rs:60-62` — `test_roundtrip`: plain JavaScript.
#[track_caller]
fn test_roundtrip(src1: &str) {
    test_roundtrip_with_flags(ParseFlags::default(), src1)
}

/// juno `tests/gen_js/mod.rs:64-74` — `test_roundtrip_flow`: `-parse-flow`.
///
/// juno spells the dialect `ParserDialect::Flow` with `strict_mode: false`
/// and `enable_jsx: false`; ours is the equivalent `ParseFlags`, whose
/// `parse_flow` also turns on the ambiguous-expression grammar exactly as
/// `hermesc -parse-flow` does (see `ParseFlags::parse_flow`'s doc comment).
#[track_caller]
fn test_roundtrip_flow(src1: &str) {
    test_roundtrip_with_flags(
        ParseFlags {
            parse_flow: true,
            ..ParseFlags::default()
        },
        src1,
    )
}

/// juno `tests/gen_js/mod.rs:76-85` — `test_roundtrip_jsx`: `-parse-jsx`,
/// plain-JavaScript dialect.
#[track_caller]
fn test_roundtrip_jsx(src1: &str) {
    test_roundtrip_with_flags(
        ParseFlags {
            parse_jsx: true,
            ..ParseFlags::default()
        },
        src1,
    )
}

/// juno `tests/gen_js/mod.rs:89-106`, reached through the parser instead of
/// through `builder::StringLiteral::build_template`.
///
/// juno builds a `StringLiteral` whose value is the three UTF-16 code units
/// `['A', U+1234, TAB]` and asserts `do_gen` prints it as `'A\u1234\t'` —
/// the non-ASCII BMP unit escaped as `\uXXXX` rather than emitted raw, and
/// the TAB escaped as `\t` rather than emitted as a literal tab. Parsing a
/// source string with those three units reaches the same node without this
/// file needing the AST builder API.
///
/// The literal is placed on the right of an assignment on purpose. A bare
/// `"...";` at the top of a program is a **directive**, and a directive
/// prints from its verbatim source spelling, not from the string's value
/// (that fidelity is itself pinned by
/// [`genuine_use_strict_directive_round_trips`] and
/// [`escaped_use_strict_directive_does_not_become_a_real_directive`]) — so
/// juno's literal placed at statement position would test the directive
/// path and never reach `print_escaped_string_literal` at all.
#[test]
fn juno_literal_escaping_matches_junos_hand_built_case() {
    let mut parsed = parse_ok("x = \"A\\u1234\\u0009\"");
    assert_eq!(do_gen(&mut parsed, Pretty::Yes).trim(), r#"x = 'A\u1234\t';"#);
}

/// juno `tests/gen_js/mod.rs:87-137` — `test_literals`.
///
/// The `\ud83d\udcd5` / `\u060b` / `\ud800` spellings are juno's, kept
/// **verbatim** rather than written as the characters they denote. The
/// distinction is load-bearing here, not cosmetic: each of these cases is a
/// bare string or template at statement position, so a string one exercises
/// the *directive* path — which reprints the literal's verbatim source text,
/// escapes and all — and a template one exercises `TemplateElement`'s own
/// `raw`. Substituting the decoded character would still pass, but it would
/// be testing a different input. (Round 1 of this port did substitute them,
/// and review caught it.)
#[test]
fn juno_test_literals() {
    test_roundtrip("1");
    test_roundtrip("1n");
    test_roundtrip("11298379123162378326187361n");
    test_roundtrip("\"abc\"");
    test_roundtrip(r#" "\ud800" "#);
    test_roundtrip(r#" "\ud83d\udcd5" "#);
    test_roundtrip(r#" "\u060b" "#);
    test_roundtrip("true");
    test_roundtrip("false");
    test_roundtrip("null");
    test_roundtrip("undefined");

    test_roundtrip("/abc/");
    test_roundtrip("/abc/gi");
    test_roundtrip("/abc/gi");
    test_roundtrip(r#"/ab\/cd/gi"#);
    test_roundtrip("/😹/");

    test_roundtrip(r#" `abc` "#);
    test_roundtrip(r#" `abc\ndef` "#);
    test_roundtrip(
        r#" `abc
        def` "#,
    );
    test_roundtrip(r#" `abc \ud800 def` "#);
    test_roundtrip(r#" `abc \ud800 def` "#);
    test_roundtrip(r#" `\ud83d\udcd5` "#);
    test_roundtrip(r#" `escape backtick: \` should work` "#);
    test_roundtrip(r#" `😹` "#);
}

/// juno `tests/gen_js/mod.rs:139-143` — `test_identifier`.
#[test]
fn juno_test_identifier() {
    test_roundtrip("foo");
    test_roundtrip("class C { #foo() {} }");
}

/// juno `tests/gen_js/mod.rs:145-153` — `test_binop`.
#[test]
fn juno_test_binop() {
    test_roundtrip("null + null");
    test_roundtrip("1 + 1");
    test_roundtrip("1 * 2 + (3 + 4)");
    test_roundtrip("1 ** 2 ** 3 ** 4");
    test_roundtrip("1 in 2 + (2 - 4) / 3");
    test_roundtrip("1 instanceof 2 + (2 - 4) / 3");
}

/// juno `tests/gen_js/mod.rs:155-162` — `test_conditional`.
#[test]
fn juno_test_conditional() {
    test_roundtrip("a ? b : c");
    test_roundtrip("a ? b : c ? d : e");
    test_roundtrip("(a ? b : c) ? d : e");
    test_roundtrip("a ? b : (c ? d : e)");
    test_roundtrip("a?.3:.4");
}

/// juno `tests/gen_js/mod.rs:164-168` — `test_vars`.
#[test]
fn juno_test_vars() {
    test_roundtrip("var x=3;");
    test_roundtrip("var x=3, y=4;");
}

/// juno `tests/gen_js/mod.rs:170-208` — `test_functions`.
#[test]
fn juno_test_functions() {
    test_roundtrip("function foo() {}");
    test_roundtrip("function foo(x, y) {}");
    test_roundtrip("function foo(x, y=3) {}");
    test_roundtrip("function foo([x, y], {z}) {}");
    test_roundtrip("function foo([x, y] = [1,2], {z:q}, {w = 1}) {}");
    test_roundtrip("function foo() { return this; }");
    test_roundtrip("function *foo() {}");
    test_roundtrip("function *foo() { yield 1; }");
    test_roundtrip("function *foo() { yield* f(); }");
    test_roundtrip("async function foo() {}");
    test_roundtrip("async function foo() { await f(); }");
    test_roundtrip("async function *foo() {}");
    test_roundtrip("async function *foo() { await f(); yield 1; }");
    test_roundtrip("x => 3");
    test_roundtrip("(x) => 3");
    test_roundtrip("(x,y) => 3");
    test_roundtrip("x => {3}");
    test_roundtrip("x => ({y: 10})");
    test_roundtrip("x => ({y: 10}[z])");
    test_roundtrip("async x => {3}");
    test_roundtrip("async (x,y) => {3}");
    test_roundtrip("(x => 1) + (y => 1)");
    test_roundtrip("x = y => 1");
    test_roundtrip("x = (y => 1)");
    test_roundtrip("x = (({a, b}) => 1)");
    test_roundtrip_flow("var x = (): (number=>string) => 1");
    test_roundtrip(
        "function foo() {
        return (y => 1);
    }",
    );
    test_roundtrip(
        "function* foo() {
        yield y => 1;
    }",
    );
}

/// juno `tests/gen_js/mod.rs:210-232` — `test_calls`.
#[test]
fn juno_test_calls() {
    test_roundtrip("f();");
    test_roundtrip("f(1);");
    test_roundtrip("f(1, 2);");
    test_roundtrip("f(1, (2,3), 4);");
    test_roundtrip("(f?.(1, 2))(3);");
    test_roundtrip("f?.(1, 2)?.(3)(5);");
    test_roundtrip("f(...x)");
    test_roundtrip("new f();");
    test_roundtrip("new f(1);");
    test_roundtrip("new f(...x)");
    test_roundtrip("new(a.b);");
    test_roundtrip("new(a.b());");
    test_roundtrip("new(a.b())();");
    test_roundtrip("new(a.b())(c);");
    test_roundtrip("new(a?.b())(c);");
    test_roundtrip("new(1 + 2);");
    test_roundtrip("new(fn(foo)[bar])()");
    test_roundtrip("new(fn(foo)[bar])(c)");
    test_roundtrip("new(fn(foo).bar)()");
    test_roundtrip("new(fn(foo).bar)(c)");
    test_roundtrip("import('foo')");
}

/// juno `tests/gen_js/mod.rs:234-303` — `test_statements`.
#[test]
fn juno_test_statements() {
    test_roundtrip("while (1) {}");
    test_roundtrip("while (1) { fn(); }");
    test_roundtrip("while (1) fn();");
    test_roundtrip("while (1) fn()");
    test_roundtrip("for (;;) { fn(); }");
    test_roundtrip("for (;;) fn();");
    test_roundtrip("for (x;;) { fn(); }");
    test_roundtrip("for (;x;) { fn(); }");
    test_roundtrip("for (;;x) { fn(); }");
    test_roundtrip("for (var x=1;x<10;++x) { fn(); }");
    test_roundtrip("for (x in y) { fn(); }");
    test_roundtrip("for (var x of y) { fn(); }");
    test_roundtrip(
        "async () => {
            for await (x of y) { fn(); }
        }",
    );
    test_roundtrip("do {fn();} while (1)");
    test_roundtrip("do fn(); while (1)");
    test_roundtrip("do x, y, z; while (1)");
    test_roundtrip("do if (x) y; while (1)");
    test_roundtrip("debugger");
    test_roundtrip("{fn(); fn();}");
    test_roundtrip("for (;;) { break; }");
    test_roundtrip("for (;;) { continue; }");
    test_roundtrip("function f() { return; }");
    test_roundtrip("function f() { return 3; }");
    test_roundtrip(
        "switch(x) {
            case 1:
                break;
            case 2:
            case 3:
                break;
            default:
                break;
        }",
    );
    test_roundtrip("a: var x = 3;");
    test_roundtrip(
        "try {
            fn();
        } catch {
            fn();
        }",
    );
    test_roundtrip(
        "try {
            fn();
        } catch (e) {
            fn();
        }",
    );
    test_roundtrip(
        "try {
            fn();
        } catch (e) {
            fn();
        } finally {
            fn();
        }",
    );
    test_roundtrip("if (x) {fn();}");
    test_roundtrip("if (x) {fn();} else {fn();}");
    test_roundtrip("if (x) fn(); else fn();");
    test_roundtrip(
        "if (x)
          try { } catch (e) { }
        else
          fn();",
    );
}

/// juno `tests/gen_js/mod.rs:305-312` — `test_logical`.
#[test]
fn juno_test_logical() {
    test_roundtrip("a && b || c");
    test_roundtrip("a || b && c");
    test_roundtrip("(a || b) && c");
    test_roundtrip("(a || b) ?? c");
    test_roundtrip("(a ?? b) || c");
}

/// juno `tests/gen_js/mod.rs:314-318` — `test_sequences`.
#[test]
fn juno_test_sequences() {
    test_roundtrip("var x = (1, 2, 3);");
    test_roundtrip("foo((1, 2, 3), 4);");
}

/// juno `tests/gen_js/mod.rs:320-337` — `test_objects`.
#[test]
fn juno_test_objects() {
    test_roundtrip("({ })");
    test_roundtrip(
        "({
            a: 1,
            [x]: 1,
            fn() {},
            b,
            ...from,
        })",
    );
    test_roundtrip_flow(
        "({
            foo<T>() {},
        })",
    );
}

/// juno `tests/gen_js/mod.rs:339-345` — `test_arrays`.
#[test]
fn juno_test_arrays() {
    test_roundtrip("([])");
    test_roundtrip("var x = [, 1, , 3]");
    test_roundtrip("var x = [1, 2, 3, ...from]");
    test_roundtrip("var x = [1, 2, 3, ...from, 4, 5, 6]");
}

/// juno `tests/gen_js/mod.rs:347-370` — `test_assignment`.
#[test]
fn juno_test_assignment() {
    test_roundtrip("x = 1");
    test_roundtrip("x = y = 1");
    test_roundtrip("x += 1");
    test_roundtrip("x -= 1");
    test_roundtrip("x *= 1");
    test_roundtrip("x /= 1");
    test_roundtrip("x **= 1");
    test_roundtrip("x |= 1");
    test_roundtrip("x &= 1");
    test_roundtrip("x ||= 1");
    test_roundtrip("x &&= 1");
    test_roundtrip("x ??= 1");
    test_roundtrip("foo()[1] = 1");
    test_roundtrip("a = b && c");
    test_roundtrip("(a = b) && c");
    test_roundtrip("a && b = c");
    test_roundtrip("a && (b = c)");
    test_roundtrip("var {x: {y: [{z}]}} = foo;");
    test_roundtrip("({x: {y: [{z}]}} = foo);");
    test_roundtrip("var [x, y] = foo;");
    test_roundtrip("([x, y] = foo);");
}

/// juno `tests/gen_js/mod.rs:372-387` — `test_unary`.
#[test]
fn juno_test_unary() {
    test_roundtrip("+x");
    test_roundtrip("-x");
    test_roundtrip("!x");
    test_roundtrip("~x");
    test_roundtrip("-(-x)");
    test_roundtrip("-(-5)");
    test_roundtrip("--x");
    test_roundtrip("x--");
    test_roundtrip("++x");
    test_roundtrip("x++");
    test_roundtrip("+!-x");
    test_roundtrip("delete x");
    test_roundtrip("typeof x");
}

/// juno `tests/gen_js/mod.rs:389-397` — `test_update`.
#[test]
fn juno_test_update() {
    test_roundtrip("x++");
    test_roundtrip("x--");
    test_roundtrip("++x");
    test_roundtrip("--x");
    test_roundtrip("--(-x)");
    test_roundtrip("+x++");
}

/// juno `tests/gen_js/mod.rs:399-419` — `test_members`.
#[test]
fn juno_test_members() {
    test_roundtrip("a.b");
    test_roundtrip("a.b.c");
    test_roundtrip("a?.b");
    test_roundtrip("a?.[b]");
    test_roundtrip("(a?.b).c");
    test_roundtrip("a?.b().c");
    test_roundtrip("(a?.b()).c");
    test_roundtrip("a?.().b");
    test_roundtrip("a?.().b");
    test_roundtrip("a?.b?.c?.()");
    test_roundtrip("(a?.b?.c?.()).d");
    test_roundtrip("(a?.b?.c?.())?.d");
    test_roundtrip("(a?.b?.c?.())(d)");
    test_roundtrip("(a?.b?.c?.())?.(d)");
    test_roundtrip("class C { constructor() { new.target; } }");
    test_roundtrip("50..toString()");
    test_roundtrip("1.5.toString()");
    test_roundtrip("1e100.toString()");
    test_roundtrip("-1e100.toString()");
    test_roundtrip("0x10293.toString()");
}

/// juno `tests/gen_js/mod.rs:421-462` — `test_classes`.
#[test]
fn juno_test_classes() {
    test_roundtrip("class C {}");
    test_roundtrip("class C extends D {}");
    test_roundtrip(
        "class C extends D {
            prop1;
            #prop2;
            constructor() {}
            a() {}
            #b() {}
            c(x, y) {}
            static d() {}
        }",
    );
    test_roundtrip_flow(
        "class C<T> extends D<T> {
            prop1: ?number = null;
            +prop2: number;
            -prop3;
            declare prop4;
            #prop5;
            #prop5: ?number = null;
            declare +prop6;
            static +prop7;
            static +[prop8];
            declare static +prop9;
            foo<T>() {}
        }",
    );
    test_roundtrip(
        "var cls = (class C extends D {
            prop1;
            #prop2;
            constructor() {}
            a() {}
            #b() {}
            c(x, y) {}
            static d() {}
            get e() {}
            set e(v) {}
            ;
        })",
    );
}

/// juno `tests/gen_js/mod.rs:464-478` — `test_import`.
#[test]
fn juno_test_import() {
    test_roundtrip("import x from 'foo'");
    test_roundtrip("import x, {y} from 'foo'");
    test_roundtrip("import * as Foo from 'foo'");
    test_roundtrip("import x, {y as z, a as b} from 'foo'");
    test_roundtrip("import {a, b, c} from 'foo'");
    test_roundtrip("import 'foo';");
    // juno's line is `import 'foo' assert {kind: 'json'};`
    // (`tests/gen_js/mod.rs:471`). `assert` was the *withdrawn* spelling of
    // the import-attributes proposal; Hermes only ever implemented the
    // current `with` spelling, so `assert` is a hard parse error here
    // (`';' expected` at column 14) — in the parser, before this crate is
    // reached. Adapted rather than deleted: the node kinds under test
    // (`ImportDeclaration::attributes`, `ImportAttribute`) both exist and
    // are exactly what the `with` form builds
    // (`crates/parser/src/js/modules.rs:93-195`, keyed on
    // `TokenKind::rw_with`), so respelling keeps juno's coverage instead of
    // dropping it. Same shape as `parser_corpus/import_attributes.js`.
    test_roundtrip("import 'foo' with {kind: 'json'};");
    test_roundtrip(
        "
        import 'foo';
        import 'bar';
        ",
    );
}

/// juno `tests/gen_js/mod.rs:480-489` — `test_export`.
#[test]
fn juno_test_export() {
    test_roundtrip("export var x = 3;");
    test_roundtrip("export function foo() {}");
    test_roundtrip("export default function foo() {}");
    test_roundtrip("export {x as y};");
    test_roundtrip("export * from 'foo';");
    test_roundtrip_flow("export type Foo = number;");
    test_roundtrip_flow("export type { x as y } from 'foo';");
}

/// juno `tests/gen_js/mod.rs:491-536` — `test_types`.
#[test]
fn juno_test_types() {
    test_roundtrip_flow("number | boolean & string");
    test_roundtrip_flow("type A = number");
    test_roundtrip_flow("type A = ?number");
    test_roundtrip_flow("type A = ?(number | string)");
    test_roundtrip_flow("type A = string");
    test_roundtrip_flow("type A = \"foo\"");
    test_roundtrip_flow("type A = 'foo'");
    test_roundtrip_flow("type A = 3");
    test_roundtrip_flow("type A = 3n");
    test_roundtrip_flow("type A = boolean");
    test_roundtrip_flow("type A = true | false");
    test_roundtrip_flow("type A = true & false");
    test_roundtrip_flow("type A = (X | Y) & Z");
    test_roundtrip_flow("type A = X | Y & Z");
    test_roundtrip_flow("type A = X<Y, Z>");
    test_roundtrip_flow("type A = X<Y>");
    test_roundtrip_flow("type A<X: Y, Z> = T");
    test_roundtrip_flow("type A = symbol");
    test_roundtrip_flow("type A = mixed");
    test_roundtrip_flow("type A = any");
    test_roundtrip_flow("type A = void");
    test_roundtrip_flow("type A = null");
    test_roundtrip_flow("type A = number => number");
    test_roundtrip_flow("type A = X.Y");
    test_roundtrip_flow("type A = X.Y<Z>");
    test_roundtrip_flow("type A = typeof X");
    test_roundtrip_flow("type A = [number, string]");
    test_roundtrip_flow("type A = []");
    test_roundtrip_flow("type A = number[]");
    test_roundtrip_flow("type A = number[string]");
    test_roundtrip_flow("type A = number?.[string]");
    test_roundtrip_flow("type A = [number, string][]");
    test_roundtrip_flow("type A = (foo: number) => number");
    test_roundtrip_flow("type A = (foo?: number) => number");
    test_roundtrip_flow("type A = (foo?: ?number) => number");
    test_roundtrip_flow("type A = (number, string) => number");
    test_roundtrip_flow("type A = (?number) => number");
    test_roundtrip_flow("type A = ?(number, string) => number");
    test_roundtrip_flow("type A = (this: number, number, string) => number");
    test_roundtrip_flow("interface A { }");
    test_roundtrip_flow("interface A extends B { }");
    test_roundtrip_flow("interface A extends B, C, D { }");
    test_roundtrip_flow("type A = { x: number }");
    test_roundtrip_flow("type A = { readonly x: number }");
    test_roundtrip_flow("type A = {| x: number |}");
    test_roundtrip_flow(
        "
        type A = {
            a?: number,
            b: ?string,
            +[c]: string,
            (d?: number): number;
            [[e]]: number,
            [[f]]?: number,
            [[g]](a: T): number,
            ...h,
            static (i?: number): number;
            +proto: number,
            ...
        };
        ",
    );
}

/// juno `tests/gen_js/mod.rs:538-552` — `test_declare`.
#[test]
fn juno_test_declare() {
    test_roundtrip_flow("declare function foo(): number;");
    test_roundtrip_flow("declare var x : number;");
    test_roundtrip_flow("declare export var x: number;");
    test_roundtrip_flow("declare opaque type x;");
    test_roundtrip_flow("declare export opaque type x: y;");
    test_roundtrip_flow("declare type x = number;");
    test_roundtrip_flow("declare interface Foo {}");
    test_roundtrip_flow("declare class A extends B {}");
    test_roundtrip_flow("declare class A extends B mixins C, D implements E {}");
    test_roundtrip_flow("declare export class A extends B {}");
    test_roundtrip_flow("declare module A {}");
    test_roundtrip_flow("declare module.exports: number;");
    test_roundtrip_flow("declare export function foo(): number;");
}

/// juno `tests/gen_js/mod.rs:554-562` — `test_enum`.
#[test]
fn juno_test_enum() {
    test_roundtrip_flow("enum Foo {}");
    test_roundtrip_flow("enum Foo of string {A = 'A', B = 'B'}");
    test_roundtrip_flow("enum Foo of string {A, B, C}");
    test_roundtrip_flow("enum Foo of string {A = 'A', B = 'B', ...}");
    test_roundtrip_flow("enum Foo of number {A = 1}");
    test_roundtrip_flow("enum Foo of boolean {A = true}");
}

/// juno `tests/gen_js/mod.rs:564-568` — `test_typecast`.
#[test]
fn juno_test_typecast() {
    test_roundtrip_flow("async function foo() { return (x: any); }");
    test_roundtrip_flow("var x = (y: number | number => string)");
}

/// juno `tests/gen_js/mod.rs:570-578` — `test_predicate`.
#[test]
fn juno_test_predicate() {
    test_roundtrip_flow("function foo(): %checks {}");
    test_roundtrip_flow("function foo(): number %checks {}");
    test_roundtrip_flow("function foo(): number %checks(bar) {}");
    test_roundtrip_flow("((x): %checks => 3)");
    test_roundtrip_flow("((x): number %checks => 3)");
    test_roundtrip_flow("((x): number %checks(bar) => 3)");
}

/// juno `tests/gen_js/mod.rs:580-586` — `test_this_param`.
#[test]
fn juno_test_this_param() {
    test_roundtrip_flow("function foo(this: number): number {}");
    test_roundtrip_flow("function foo(this: number, x: number): number {}");
    test_roundtrip_flow("declare function foo(this: number): number;");
    test_roundtrip_flow("declare function foo(this: number, x: number): number;");
}

/// juno `tests/gen_js/mod.rs:588-613` — `test_jsx`.
#[test]
fn juno_test_jsx() {
    test_roundtrip_jsx("<foo />");
    test_roundtrip_jsx("<foo></foo>");
    test_roundtrip_jsx("<foo>abc</foo>");
    test_roundtrip_jsx(
        r#"
        <asdf desc="foo
            bar"
            prop2='foo """ bar'>
            body
        </asdf>
        "#,
    );
    test_roundtrip_jsx("<></>");
    test_roundtrip_jsx(
        "
        <foo>
            <bar x={1} y='3' {...z} />
            abcdef and an emoji: 😹
            &gt; &x1f639;
            end of test text
            <baz.bar />
            <hello:goodbye />
        </foo>
        ",
    );
}

// ===========================================================================
// Task 15 regression pins: one named test per defect the round-trip harness
// and the Tier 1 corpus gate found. Each of these is a case the whole suite
// through Task 14 passed. They are pinned individually, not only inside the
// big ported juno functions above, so that a failure names the defect.
// ===========================================================================

/// Defect 21 (inherited from juno). juno's `OptionalCallExpression` arm
/// (`gen_js.rs:1056-1069`) prints `callee`, then `type_arguments`, then
/// `?.`, so `f?.<T>(1)` came out as `f<T>?.(1)` — which does not parse at
/// all. `?.` introduces the optional-chain link and the type arguments
/// belong to the call it introduces.
///
/// Found by the corpus gate on `sema/tests/sema_corpus/flow-type-args.js`.
#[test]
fn optional_call_with_type_arguments_puts_the_question_dot_first() {
    test_roundtrip_flow("f?.<Baz>(1);");
    // The non-optional link in an optional chain has no `?.` of its own,
    // and must not grow one.
    test_roundtrip_flow("f?.<A>(1)<B>(2);");
    // Plain (non-chain) calls with type arguments are unaffected.
    test_roundtrip_flow("f<Baz>(1);");
}

/// Defect 22 (ours). `Variance::kind` can be the token spelling `in` or
/// `out` — Flow's TypeScript-style type-parameter variance — and
/// `VarianceKind::from_label` knew only `plus`/`minus`/`readonly`/
/// `writeonly`, so generation failed outright with
/// `UnknownOperator { kind: "Variance", spelling: "in" }`.
///
/// Found by the corpus gate on
/// `parser/tests/parser_corpus_flow/type_params.js`.
#[test]
fn type_parameter_in_out_variance_round_trips() {
    test_roundtrip_flow("type F<in T> = T;");
    test_roundtrip_flow("type G<out T> = T;");
    // (Hermes takes at most one variance modifier per type parameter —
    // `parse_type_param_flow` consumes exactly one `in`/`out` before the
    // name — so there is no `<in out T>` case to pin.)
    // `in` with no following name is the parameter's NAME, not variance:
    // the parser builds no `Variance` node at all here.
    test_roundtrip_flow("type I<in> = X;");
    test_roundtrip_flow("type J<out> = X;");
    // The `+`/`-` spellings must keep working.
    test_roundtrip_flow("type K<+T, -U> = [T, U];");
}

/// Defect 23 (inherited from juno). `TypeParameter::uses_extends_bound`
/// records whether the source wrote `<T extends B>` or `<T: B>`, and it is
/// an **ESTree property** (`ESTree.def:1160-1161`), not just an internal
/// marker — so juno's `uses_extends_bound: _` (`gen_js.rs:3049`), which
/// always reprints the bound as `T: B`, flips `usesExtendsBound` from `true`
/// to `false` and produces a different AST.
///
/// Found by the corpus gate on
/// `parser/tests/parser_corpus_flow/type_params.js`.
#[test]
fn type_parameter_extends_bound_spelling_round_trips() {
    test_roundtrip_flow("type I<T extends U> = T;");
    test_roundtrip_flow("type B<T: number> = T;");
    // Both spellings alongside the other modifiers they combine with.
    test_roundtrip_flow("type C<const T extends U = V> = T;");
    test_roundtrip_flow("type D<+T extends U> = T;");
}

/// Defect 24 (ours, a regression introduced by Task 13's `TypeAnnotation`
/// precedence entry). An arrow's Flow return type is read with
/// `AllowAnonFunctionType::No`, so an unparenthesized function type there is
/// not a function type at all — the `=>` belongs to the arrow. Regenerating
/// `(): (number=>string) => 1` without the parens produced
/// `(): (number) => string => 1`, which parses **silently** to return type
/// `number` with body `string => 1`.
///
/// Found by juno's own ported case
/// (`tests/gen_js/mod.rs:196`); see
/// `precedence.rs`'s `flow_return_type_spine_has_function_type`.
#[test]
fn arrow_flow_return_function_type_keeps_its_parens() {
    test_roundtrip_flow("var x = (): (number=>string) => 1");
    // The same hazard reached through each edge of the unbracketed spine
    // the `No` flag propagates down.
    test_roundtrip_flow("var x = (): ?(number=>string) => 1");
    test_roundtrip_flow("var x = (): (A | (number=>string)) => 1");
    test_roundtrip_flow("var x = (): (A & (number=>string)) => 1");
    // A return type with no function type on that spine must NOT grow
    // parens that break TypeScript (the reason Task 13 removed the blanket
    // wrap in the first place) or change the Flow tree.
    test_roundtrip_flow("var x = (): number => 1");
    test_roundtrip_flow("var x = (): Array<number=>string> => 1");
}

/// Defect 25 (ours, and juno's in a milder form). `raw` on
/// `StringLiteralTypeAnnotation`/`NumberLiteralTypeAnnotation` is an
/// unconditional ESTree property holding the verbatim source spelling, so
/// printing the decoded `value` instead corrupts it: `type A = 0x10` came
/// back as `type A = 16`, with `raw` `"0x10"` → `"16"`.
///
/// Found by juno's own ported case `test_roundtrip_flow("type A = \"foo\"")`
/// (`tests/gen_js/mod.rs:496`), then generalized to the numeric spellings.
#[test]
fn literal_type_annotations_preserve_their_raw_spelling() {
    test_roundtrip_flow("type A = \"foo\";");
    test_roundtrip_flow("type A = 'foo';");
    test_roundtrip_flow("type A = 0x10;");
    test_roundtrip_flow("type A = 1e3;");
    test_roundtrip_flow("type A = 1_0;");
    test_roundtrip_flow("type A = 0b101;");
    test_roundtrip_flow("type A = .5;");
    test_roundtrip_flow("type A = 123n;");
    // `BooleanLiteralTypeAnnotation` still prints `value`; `raw` cannot
    // disagree with it.
    test_roundtrip_flow("type A = true;");
    test_roundtrip_flow("type A = false;");
}

// ===========================================================================
// Task 15, review round 2. Defect 24's first fix was incomplete: the helper
// it added enumerated the spine edges it followed and claimed the list was
// complete, and the list was missing two node families. What follows replaces
// the claim with a measurement.
// ===========================================================================

/// Defect 26 — a Flow `TypePredicate`'s operand is in the same
/// `AllowAnonFunctionType::No` region as the arrow return type that contains
/// it, and was printed with a bare `gen_node`.
///
/// `parse_return_type_annotation_flow` threads its own
/// `allow_anon_function_type` argument into all three predicate forms'
/// operands (`crates/parser/src/js/flow/function_types.rs:79-82`, `:155-159`,
/// `:190-193`), so under an arrow the operand inherits `No`. Measured against
/// the generator as shipped in `dc764513b`:
///
/// | source | regenerated | outcome |
/// |---|---|---|
/// | `(x: mixed): x is (number => string) => 1` | `x is (number) => string => 1` | different AST |
/// | `(x: mixed): asserts x is (number => string) => 1` | same shape | different AST |
/// | `(x: mixed): implies x is (number => string) => 1` | same shape | different AST |
/// | `(x: mixed): x is (A extends B ? C : D) => true` | parens dropped | does not parse |
///
/// The parens go on the *operand*, not the predicate: `x is T` is not a type,
/// so `(): (x is T) => 1` does not parse.
#[test]
fn flow_type_predicate_operand_keeps_its_parens() {
    test_roundtrip_flow("var f = (x: mixed): x is (number => string) => 1");
    test_roundtrip_flow("var f = (x: mixed): asserts x is (number => string) => 1");
    test_roundtrip_flow("var f = (x: mixed): implies x is (number => string) => 1");
    test_roundtrip_flow("var f = (x: mixed): x is (A extends B ? C : D) => true");
    test_roundtrip_flow("var f = (x: mixed): asserts x is (A extends B ? C : D) => true");
    test_roundtrip_flow("var f = (x: mixed): implies x is (A extends B ? C : D) => true");
    // The operand's own unbracketed spine, one edge at a time.
    test_roundtrip_flow("var f = (x: mixed): x is ?(number => string) => 1");
    test_roundtrip_flow("var f = (x: mixed): x is (A | (number => string)) => 1");
    test_roundtrip_flow("var f = (x: mixed): x is (A & (number => string)) => 1");
    // Operands that must NOT grow parens, i.e. the rule is not a blanket wrap.
    test_roundtrip_flow("var f = (x: mixed): x is number => 1");
    test_roundtrip_flow("var f = (x: mixed): x is Array<number => string> => 1");
    // The same predicates in a NON-arrow return type, where the region is
    // `Yes` and no parens are needed: over-wrapping there must still
    // round-trip (a Flow `( Type )` group builds no node).
    test_roundtrip_flow("function g(x: mixed): x is (number => string) { return z; }");
    test_roundtrip_flow("declare function h(x: mixed): x is (number => string);");
}

/// Defect 27 — a Flow `ConditionalTypeAnnotation` in an arrow return type
/// needs parens **unconditionally**, with no function type anywhere in it.
///
/// `parse_conditional_type_annotation_flow` parses its `true_type` and
/// `false_type` with an explicit `AllowAnonFunctionType::Yes`
/// (`crates/parser/src/js/flow/types.rs:222-223`, `:237-238`), so an
/// unparenthesized conditional's `false_type` swallows the arrow's own `=>`.
/// Measured against `dc764513b`:
///
/// ```text
/// var x = (): (A extends B ? C : D) => 1
///   -> var x = (): A extends B ? C : D => 1;      does not parse
/// ```
///
/// This is the row that showed the first fix was aimed at the wrong thing:
/// no function type is involved at all, so a helper that looks only for
/// `FunctionTypeAnnotation` cannot see it.
#[test]
fn flow_arrow_return_conditional_type_keeps_its_parens() {
    test_roundtrip_flow("var x = (): (A extends B ? C : D) => 1");
    test_roundtrip_flow("var x = (): (A extends B ? (number => string) : C) => 1");
    test_roundtrip_flow("var x = (): (A extends B ? C : (D => E)) => 1");
    // Reached through each inheriting edge.
    test_roundtrip_flow("var x = (): ?(A extends B ? C : D) => 1");
    test_roundtrip_flow("var x = (): (Z | (A extends B ? C : D)) => 1");
    test_roundtrip_flow("var x = (): (Z & (A extends B ? C : D)) => 1");
    // A conditional type in a NON-arrow return type needs no parens, and
    // must survive the redundant ones.
    test_roundtrip_flow("function g(): (A extends B ? C : D) { return z; }");
    test_roundtrip_flow("type Q = A extends B ? C : D;");
}

/// Every Flow arrow-return-type shape this crate can be asked for, checked by
/// **execution over a generated cross-product** rather than by anybody's
/// reading of the grammar.
///
/// This test exists because of how defect 24's first fix failed review. That
/// fix shipped with a doc comment enumerating the `AllowAnonFunctionType::No`
/// spine edges and asserting "those are exactly the edges followed here" —
/// and the enumeration was missing `TypePredicate` and
/// `ConditionalTypeAnnotation`. An enumeration written by the same person who
/// wrote the code cannot catch that; only running the shapes can.
///
/// So: {27 type-context wrappers} x {15 payloads} x {9 statement templates},
/// each round-tripped in both [`Pretty`] modes. The wrappers cover both the
/// unbracketed spine (`?%s`, `A | %s`, `%s & A`, `x is %s`,
/// `asserts x is %s`, `implies x is %s`, and nestings of those) and the
/// bracketed positions that must **not** need a wrap (`Array<%s>`, `[%s]`,
/// `{ p: %s }`, `%s[]`, `keyof %s`). The payloads include both hazard
/// families (`(number => string)`, `(A extends B ? C : D)`), function types
/// that would survive unwrapped (`((a: number) => string)`, `(() => string)`),
/// and ordinary types that must be left alone. The templates cover arrows
/// (plain, `async`, and type-parameterized), the non-arrow return types where
/// the region is `Yes` and a redundant wrap must still round-trip, and plain
/// type positions.
///
/// A shape whose *source* does not parse is skipped and **counted**: those
/// counts are asserted below, so a shape cannot pass this test by silently
/// dropping out of it. That is the failure mode this whole test is a reaction
/// to.
///
/// Result at the time of writing: **3645 shapes probed, 0 failures, 240
/// skipped.** Before the round-2 fix the same probe reported 22 failures out
/// of a 342-shape subset. The 240 skips are exactly two explainable groups
/// and nothing else, which the assertions pin: 180 are a type predicate in a
/// position where Flow has no predicates at all (`type Q = x is T;`,
/// `var v: x is T;`), and 60 are the `(%s) => void` wrapper in an arrow
/// return type — itself an unparenthesized anon function type in a `No`
/// region, i.e. the parser rule under test, confirming the region is real.
#[test]
fn flow_arrow_return_type_shapes_all_round_trip() {
    /// Type contexts the payload is dropped into. `%s` is the payload.
    const WRAPPERS: &[&str] = &[
        "%s",
        "?%s",
        "??%s",
        "A | %s",
        "%s | A",
        "A & %s",
        "%s & A",
        "A | B & %s",
        "?(A | %s)",
        "x is %s",
        "asserts x is %s",
        "implies x is %s",
        "x is ?%s",
        "x is A | %s",
        "asserts x is A | ?%s",
        // Bracketed positions: the flag resets on entry, so none of these
        // may need the wrap.
        "Array<%s>",
        "[%s]",
        "{ p: %s }",
        "%s[]",
        "keyof %s",
        "{ p: %s, q: A }",
        "[%s, A]",
        // A function type whose PARAMETER is the payload: legal wherever the
        // region is `Yes`, and itself the hazard wherever it is not.
        "(%s) => void",
        // Deeper spine nestings.
        "A | ?%s",
        "?A | %s",
        "%s & ?A",
        // A wrapper with no payload hole at all, as a control: whatever the
        // payload is, this shape must round-trip.
        "typeof x",
    ];

    /// Types dropped into each wrapper.
    const PAYLOADS: &[&str] = &[
        "(number => string)",
        "((a: number) => string)",
        "(() => string)",
        "(A extends B ? C : D)",
        "(A extends B ? (number => string) : C)",
        "(A extends B ? C : (D => E))",
        "(?(number => string))",
        "((A | (number => string)))",
        "number",
        "Array<number>",
        "?number",
        "A | B",
        "A & B",
        "{ p: number }",
        "[number, string]",
    ];

    /// Statements the resulting type is dropped into. `TY` is the type.
    const TEMPLATES: &[&str] = &[
        "var f = (x: mixed): TY => 1",
        "var f = (x: mixed): TY => { return z; }",
        "var f = async (x: mixed): TY => 1",
        "var f = <T>(x: mixed): TY => 1",
        // Non-arrow return types: the region is `Yes`, so these pin that a
        // redundant wrap does not corrupt anything.
        "function g(x: mixed): TY { return z; }",
        "class K { m(x: mixed): TY { return z; } }",
        "declare function h(x: mixed): TY;",
        // Plain type positions, no return type at all.
        "type Q = TY;",
        "var v: TY;",
    ];

    let flags = ParseFlags {
        parse_flow: true,
        ..ParseFlags::default()
    };
    let mut failures: Vec<String> = Vec::new();
    let mut skipped_predicate = 0usize;
    let mut skipped_anon_fn = 0usize;
    let mut skipped_other: Vec<String> = Vec::new();
    let mut probed = 0usize;

    for wrapper in WRAPPERS {
        for payload in PAYLOADS {
            let ty = wrapper.replace("%s", payload);
            for template in TEMPLATES {
                let src = template.replace("TY", &ty);
                probed += 1;
                let mut parsed = match hermes_parser::parse(&src, flags) {
                    Ok(p) => p,
                    Err(_) => {
                        // Not a legal program. Classify it, so that a shape
                        // can never leave this test by accident.
                        if ty.contains(" is ") && !template.contains("):") {
                            skipped_predicate += 1;
                        } else if wrapper.starts_with('(') && wrapper.contains(") => ") {
                            skipped_anon_fn += 1;
                        } else {
                            skipped_other.push(src);
                        }
                        continue;
                    }
                };
                let before = juno_ast_json(&mut parsed);
                for pretty in [Pretty::Yes, Pretty::No] {
                    let js = do_gen(&mut parsed, pretty);
                    match hermes_parser::parse(&js, flags) {
                        Err(e) => failures.push(format!(
                            "{src}\n    [{pretty:?}] -> {:?} DOES NOT PARSE: {e:?}",
                            js.trim()
                        )),
                        Ok(mut reparsed) => {
                            if juno_ast_json(&mut reparsed) != before {
                                failures.push(format!(
                                    "{src}\n    [{pretty:?}] -> {:?} DIFFERENT AST",
                                    js.trim()
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} of {probed} shapes failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert!(
        skipped_other.is_empty(),
        "unclassified skips (source did not parse for a reason this test does \
         not account for) — investigate before pinning:\n{}",
        skipped_other.join("\n")
    );
    // Pin the shape counts, so the sweep cannot quietly shrink.
    assert_eq!(probed, WRAPPERS.len() * PAYLOADS.len() * TEMPLATES.len());
    assert_eq!(probed, 3645);
    assert_eq!(skipped_predicate, 180, "type predicate in a non-predicate position");
    assert_eq!(skipped_anon_fn, 60, "bare anon function type in a No region");
    eprintln!(
        "flow arrow return types: {probed} shapes probed, 0 failures, {} skipped",
        skipped_predicate + skipped_anon_fn
    );
}

// ===========================================================================
// Task 17. Two kinds the Tier 2 sweep found had **zero** coverage anywhere:
// not in the 1934 `test/` files, not in the Tier 1 corpus, and not in this
// file. See `MANIFEST.md`'s per-kind coverage table.
// ===========================================================================

/// `InferredPredicate` (`%checks`) and `DeclaredPredicate` (`%checks(expr)`)
/// — Flow's predicate-function annotations.
///
/// Both arms (`arms/flow_decl.rs`'s `gen_inferred_predicate` and
/// `gen_declared_predicate`) shipped with no test of their own. The Tier 2
/// sweep is what surfaced that: both kinds have count 0 over all 1934 `.js`
/// files under `test/`, and the reason is a **parser defect** (see
/// `MANIFEST.md`, PD-3) that makes the two files which *would* have produced
/// them — `test/Parser/flow/predicate-checks.js` and
/// `declare-function-location.js` — fail to parse.
///
/// The defect is confined to `declare function`/`declare hook`
/// (`crates/parser/src/js/flow/declarations.rs:741`, `:1805` test
/// `check_name(b"checks")` where the interned token text is `%checks`), so
/// the ordinary function-declaration spelling below does parse and does
/// reach both arms. Verified against the parser as shipped: the first case
/// yields `predicate: {"type": "InferredPredicate"}` and the second
/// `predicate: {"type": "DeclaredPredicate", "value": …}`.
#[test]
fn flow_predicate_annotations_round_trip() {
    test_roundtrip_flow("function f(x): boolean %checks { return !!x; }");
    test_roundtrip_flow("function f(x): boolean %checks(!!x) { return !!x; }");
    // A declared predicate whose value needs its own parenthesization
    // decision, so the `value` field is not merely an identifier.
    test_roundtrip_flow("function f(x, y): boolean %checks(x && y) { return x && y; }");
    // An arrow — the other function form that carries a `predicate`. A
    // class *method* does not: `class K { m(x): boolean %checks {} }` is a
    // parse error ("'{' expected in method definition"), so it is not in
    // this list.
    test_roundtrip_flow("var g = (x): boolean %checks => !!x;");
}
