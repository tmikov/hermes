/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! ES statements and patterns: `Program`/`Empty`/`Metadata` through
//! `AssignmentPattern`, plus the `visit_stmt_or_block`/`visit_stmt_list`/
//! `visit_stmt_in_block` block-printing helpers.
//!
//! Ported from juno `gen_js.rs:364-372` (`Empty`, `Metadata`, `Program`;
//! `Module` is skipped — we have no such kind), `gen_js.rs:521-827`
//! (`WhileStatement` through `IfStatement`), `gen_js.rs:1335-1387`
//! (`CatchClause`, `VariableDeclaration`, `VariableDeclarator`),
//! `gen_js.rs:1942-1999` (`ObjectPattern` through `AssignmentPattern`), and
//! `gen_js.rs:3525-3589` (`visit_stmt_or_block`, `visit_stmt_list`,
//! `visit_stmt_in_block`). This is the plan's Task 6.
//!
//! # Two juno correctness fixes made here, not transcribed
//!
//! **The `Program`-in-`gen_root` shortcut is gone.** Task 2 special-cased
//! an empty `Program` body directly in `gen_root` (`gen.rs`) rather than
//! adding a real dispatch arm, because nothing existed yet to print a
//! statement list. That shortcut called raw `gen_node` per top-level
//! statement instead of `visit_stmt_in_block`, so it never printed a
//! trailing `;` or an inter-statement newline — invisible as long as every
//! test used a single top-level statement, which is exactly what every
//! prior task's tests did. [`GenJS::gen_program`] below is the real arm;
//! `gen_root` now just calls `gen_node` like any other root, and
//! `roundtrip.rs`'s `three_statements_get_semicolons_and_separation` is the
//! multi-statement test that would have caught the shortcut.
//!
//! **`ExpressionStatement`'s `directive` field is no longer discarded.**
//! See [`GenJS::gen_expression_statement`]'s doc comment for the full
//! account: juno's arm (`gen_js.rs:749-760`) destructures `directive: _`
//! and always reprints the child `StringLiteral` from its *cooked* value,
//! which can flip ECMA-262's Use Strict Directive determination on
//! reparse. `directive` holds the exact raw source spelling for exactly
//! this reason; this arm uses it.
//!
//! # Adaptations specific to this module
//!
//! **`ForceBlock` is defined here**, not ported from a shared location:
//! juno defines it once (`gen_js.rs:207-210`) in the same option-types
//! region Task 2 already ported into `gen.rs`, but every one of its call
//! sites (`gen_js.rs:534-822`) falls inside this task's own line ranges, so
//! there was nothing outside this module that needed it yet. `pub(crate)`
//! rather than module-private in case a later task's function/method-body
//! arms (Task 7) want it too, matching `precedence.rs`'s `ChildPos`/
//! `NeedParens`.
//!
//! **`VariableDeclaration`/`VariableDeclarator`'s `kind` prints through
//! `try_bytes_str`, not a typed enum**, unlike `arms/expr.rs`'s
//! `PropertyKind`. juno's own `kind` field is a typed
//! `VariableDeclarationKind` enum with exactly 3 variants (`Var`/`Let`/
//! `Const`); ours is a raw `NodeLabel` atom, and the value set is no longer
//! fixed at 3 — `crates/parser/src/js/statements.rs`'s
//! `parse_using_declaration` (porting `JSParserImpl::parseUsingDeclaration`)
//! also produces `"using"` and `"await using"`. A `PropertyKind`-style
//! classifier would need updating every time a new declaration kind lands;
//! since this arm never branches on `kind`'s value (only prints it), there
//! is nothing to classify — `ctx.try_bytes_str` reads it directly, the same
//! way `arms/literal.rs`'s `gen_bigint_literal` reads `BigIntLiteral::bigint`.
//!
//! **The `for (var i = (a in b);;);` hazard fix lives in `precedence.rs`,
//! not here.** `VariableDeclarator`'s `init` now prints through
//! `print_child` (juno: a bare `init.visit(...)`) specifically so
//! `precedence.rs`'s `need_parens` can add parens around a bare `in`
//! `BinaryExpression` — see that module's `VariableDeclarator` branch for
//! the full account of why the fix has to live there (it needs
//! `BinaryExpressionOperator`, already private to that file) and why it
//! parenthesizes unconditionally rather than only inside a `for` head.

use hermes_ast::context::GCLock;
use hermes_ast::node::{
    ArrayPattern, AssignmentPattern, BlockStatement, BreakStatement, CatchClause,
    ContinueStatement, DoWhileStatement, ExpressionStatement, ForInStatement, ForOfStatement,
    ForStatement, IfStatement, LabeledStatement, Node, NodeField, ObjectPattern, Program,
    RestElement, ReturnStatement, SwitchCase, SwitchStatement, ThrowStatement, TryStatement,
    VariableDeclaration, VariableDeclarator, WhileStatement, WithStatement,
};
use hermes_ast::node_child::NodeList;
use hermes_ast::visitor::Path;

use crate::precedence::{stmt_skip_semi, ChildPos, ForceSpace};
use crate::{out, GenJS, GenJsError, Pretty};

/// Whether a loop/`if` body that isn't already a `BlockStatement` must be
/// wrapped in `{ ... }` anyway.
///
/// juno `gen_js.rs:207-210`. See the module doc comment for why it lives
/// here rather than a shared location.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum ForceBlock {
    /// Print the body as-is (a `BlockStatement` still prints as a block;
    /// this only affects a *non*-block body).
    No,
    /// Wrap a non-block body in `{ ... }`.
    Yes,
}

impl<'s, 'w> GenJS<'s, 'w> {
    /// `Program`: the statement list, with juno's separators, semicolons,
    /// and directive-prologue handling (via [`GenJS::visit_stmt_list`]).
    ///
    /// juno `gen_js.rs:367-369`. See the module doc comment's first fix:
    /// this is the real arm `gen_root`'s Task 2 shortcut stood in for.
    pub(crate) fn gen_program<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &Program<'gc>,
    ) -> Result<(), GenJsError> {
        let Program {
            metadata: _,
            body,
            scope: _,
            sem_info: _,
            strictness: _,
            is_method_definition: _,
            decorations: _,
            dummy_param_list: _,
        } = inner;
        self.visit_stmt_list(ctx, *body, Path::new(node, NodeField::body))
    }

    /// `Empty`: a cover-grammar/array-hole placeholder with no source
    /// syntax of its own (see `arms::expr`'s `ArrayExpression` arm, which
    /// prints its own `Node::Empty` elements as nothing inline rather than
    /// dispatching through here).
    ///
    /// juno `gen_js.rs:364`.
    pub(crate) fn gen_empty(&mut self) -> Result<(), GenJsError> {
        Ok(())
    }

    /// `Metadata`: prints nothing. juno `gen_js.rs:365`.
    pub(crate) fn gen_metadata(&mut self) -> Result<(), GenJsError> {
        Ok(())
    }

    /// `WhileStatement`: `while (test) body`.
    ///
    /// juno `gen_js.rs:521-536`.
    pub(crate) fn gen_while_statement<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &WhileStatement<'gc>,
    ) -> Result<(), GenJsError> {
        let WhileStatement {
            metadata: _,
            body,
            test,
            label_index: _,
        } = inner;
        out!(self, "while");
        self.space(ForceSpace::No);
        out!(self, "(");
        self.gen_node(ctx, test, Some(Path::new(node, NodeField::test)))?;
        out!(self, ")");
        self.visit_stmt_or_block(ctx, body, ForceBlock::No, Path::new(node, NodeField::body))?;
        Ok(())
    }

    /// `DoWhileStatement`: `do body while (test)`.
    ///
    /// juno `gen_js.rs:537-557`.
    pub(crate) fn gen_do_while_statement<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &DoWhileStatement<'gc>,
    ) -> Result<(), GenJsError> {
        let DoWhileStatement {
            metadata: _,
            body,
            test,
            label_index: _,
        } = inner;
        out!(self, "do ");
        let block =
            self.visit_stmt_or_block(ctx, body, ForceBlock::No, Path::new(node, NodeField::body))?;
        if block {
            self.space(ForceSpace::No);
        } else {
            self.newline();
        }
        out!(self, "while");
        self.space(ForceSpace::No);
        out!(self, "(");
        self.gen_node(ctx, test, Some(Path::new(node, NodeField::test)))?;
        out!(self, ")");
        Ok(())
    }

    /// `ForInStatement`: `for(left in right) body`.
    ///
    /// juno `gen_js.rs:559-576`.
    pub(crate) fn gen_for_in_statement<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ForInStatement<'gc>,
    ) -> Result<(), GenJsError> {
        let ForInStatement {
            metadata: _,
            left,
            right,
            body,
            label_index: _,
            scope: _,
        } = inner;
        out!(self, "for(");
        self.gen_node(ctx, left, Some(Path::new(node, NodeField::left)))?;
        out!(self, " in ");
        self.gen_node(ctx, right, Some(Path::new(node, NodeField::right)))?;
        out!(self, ")");
        self.visit_stmt_or_block(ctx, body, ForceBlock::No, Path::new(node, NodeField::body))?;
        Ok(())
    }

    /// `ForOfStatement`: `for(left of right) body`, or `for await(...)`.
    ///
    /// juno `gen_js.rs:577-595`.
    pub(crate) fn gen_for_of_statement<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ForOfStatement<'gc>,
    ) -> Result<(), GenJsError> {
        let ForOfStatement {
            metadata: _,
            left,
            right,
            body,
            r#await,
            label_index: _,
            scope: _,
        } = inner;
        out!(self, "for{}(", if r#await.get() { " await" } else { "" });
        self.gen_node(ctx, left, Some(Path::new(node, NodeField::left)))?;
        out!(self, " of ");
        self.gen_node(ctx, right, Some(Path::new(node, NodeField::right)))?;
        out!(self, ")");
        self.visit_stmt_or_block(ctx, body, ForceBlock::No, Path::new(node, NodeField::body))?;
        Ok(())
    }

    /// `ForStatement`: `for(init;test;update) body`.
    ///
    /// juno `gen_js.rs:596-622`. `init`'s dangling-`in` hazard
    /// (`for((a in b);;)`) is protected by `precedence.rs`'s existing
    /// `ForStatement` branch of `need_parens`, via `print_child` — see the
    /// module doc comment for the *nested* case
    /// (`for (var i = (a in b);;);`), fixed in `VariableDeclarator`'s arm
    /// instead.
    pub(crate) fn gen_for_statement<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ForStatement<'gc>,
    ) -> Result<(), GenJsError> {
        let ForStatement {
            metadata: _,
            init,
            test,
            update,
            body,
            label_index: _,
            scope: _,
        } = inner;
        out!(self, "for(");
        self.print_child(ctx, *init, Path::new(node, NodeField::init), ChildPos::Left)?;
        out!(self, ";");
        if let Some(test) = test {
            self.space(ForceSpace::No);
            self.gen_node(ctx, test, Some(Path::new(node, NodeField::test)))?;
        }
        out!(self, ";");
        if let Some(update) = update {
            self.space(ForceSpace::No);
            self.gen_node(ctx, update, Some(Path::new(node, NodeField::update)))?;
        }
        out!(self, ")");
        self.visit_stmt_or_block(ctx, body, ForceBlock::No, Path::new(node, NodeField::body))?;
        Ok(())
    }

    /// `DebuggerStatement`: `debugger`.
    ///
    /// juno `gen_js.rs:624-626`.
    pub(crate) fn gen_debugger_statement(&mut self) -> Result<(), GenJsError> {
        out!(self, "debugger");
        Ok(())
    }

    /// `EmptyStatement`: prints nothing — its `;` comes from
    /// [`GenJS::visit_stmt_in_block`] the same way any other statement's
    /// does, since it is not in [`stmt_skip_semi`]'s skip set.
    ///
    /// juno `gen_js.rs:627`.
    pub(crate) fn gen_empty_statement(&mut self) -> Result<(), GenJsError> {
        Ok(())
    }

    /// `BlockStatement`: `{ stmt0; stmt1; ... }`, or `{}` when empty.
    ///
    /// juno `gen_js.rs:629-640`. Decoration fields (`implicit`, `scope`,
    /// `buffer_id`, lazy-body bookkeeping, arrow-function-capture flags)
    /// have no juno counterpart and don't affect printing — this crate
    /// always prints every statement, lazily-pruned or not, and always
    /// prints real `{`/`}` regardless of whether the source omitted them
    /// for some construct — so they're all discarded with `_`.
    pub(crate) fn gen_block_statement<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &BlockStatement<'gc>,
    ) -> Result<(), GenJsError> {
        let BlockStatement {
            metadata: _,
            body,
            implicit: _,
            scope: _,
            buffer_id: _,
            is_lazy_function_body: _,
            param_yield: _,
            param_await: _,
            contains_arrow_functions: _,
            may_contain_arrow_functions_using_arguments: _,
        } = inner;
        if body.is_empty() {
            out!(self, "{{}}");
        } else {
            out!(self, "{{");
            self.inc_indent();
            self.newline();
            self.visit_stmt_list(ctx, *body, Path::new(node, NodeField::body))?;
            self.dec_indent();
            self.newline();
            out!(self, "}}");
        }
        Ok(())
    }

    /// `BreakStatement`: `break` or `break label`.
    ///
    /// juno `gen_js.rs:642-649`.
    pub(crate) fn gen_break_statement<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &BreakStatement<'gc>,
    ) -> Result<(), GenJsError> {
        let BreakStatement {
            metadata: _,
            label,
            label_index: _,
        } = inner;
        out!(self, "break");
        if let Some(label) = label {
            self.space(ForceSpace::Yes);
            self.gen_node(ctx, label, Some(Path::new(node, NodeField::label)))?;
        }
        Ok(())
    }

    /// `ContinueStatement`: `continue` or `continue label`.
    ///
    /// juno `gen_js.rs:650-657`.
    pub(crate) fn gen_continue_statement<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ContinueStatement<'gc>,
    ) -> Result<(), GenJsError> {
        let ContinueStatement {
            metadata: _,
            label,
            label_index: _,
        } = inner;
        out!(self, "continue");
        if let Some(label) = label {
            self.space(ForceSpace::Yes);
            self.gen_node(ctx, label, Some(Path::new(node, NodeField::label)))?;
        }
        Ok(())
    }

    /// `ThrowStatement`: `throw argument`.
    ///
    /// juno `gen_js.rs:659-664`.
    pub(crate) fn gen_throw_statement<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ThrowStatement<'gc>,
    ) -> Result<(), GenJsError> {
        let ThrowStatement {
            metadata: _,
            argument,
        } = inner;
        out!(self, "throw ");
        self.gen_node(ctx, argument, Some(Path::new(node, NodeField::argument)))
    }

    /// `ReturnStatement`: `return` or `return argument`.
    ///
    /// juno `gen_js.rs:665-673`.
    pub(crate) fn gen_return_statement<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ReturnStatement<'gc>,
    ) -> Result<(), GenJsError> {
        let ReturnStatement {
            metadata: _,
            argument,
        } = inner;
        out!(self, "return");
        if let Some(argument) = argument {
            out!(self, " ");
            self.gen_node(ctx, argument, Some(Path::new(node, NodeField::argument)))?;
        }
        Ok(())
    }

    /// `WithStatement`: `with (object) body`.
    ///
    /// juno `gen_js.rs:674-687`.
    pub(crate) fn gen_with_statement<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &WithStatement<'gc>,
    ) -> Result<(), GenJsError> {
        let WithStatement {
            metadata: _,
            object,
            body,
        } = inner;
        out!(self, "with");
        self.space(ForceSpace::No);
        out!(self, "(");
        self.gen_node(ctx, object, Some(Path::new(node, NodeField::object)))?;
        out!(self, ")");
        self.visit_stmt_or_block(ctx, body, ForceBlock::No, Path::new(node, NodeField::body))?;
        Ok(())
    }

    /// `SwitchStatement`: `switch (discriminant) { case0 case1 ... }`.
    ///
    /// juno `gen_js.rs:689-706`.
    pub(crate) fn gen_switch_statement<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &SwitchStatement<'gc>,
    ) -> Result<(), GenJsError> {
        let SwitchStatement {
            metadata: _,
            discriminant,
            cases,
            label_index: _,
            scope: _,
        } = inner;
        out!(self, "switch");
        self.space(ForceSpace::No);
        out!(self, "(");
        self.gen_node(
            ctx,
            discriminant,
            Some(Path::new(node, NodeField::discriminant)),
        )?;
        out!(self, ")");
        self.space(ForceSpace::No);
        out!(self, "{{");
        self.newline();
        for case in cases.iter() {
            self.gen_node(ctx, case, Some(Path::new(node, NodeField::cases)))?;
            self.newline();
        }
        out!(self, "}}");
        Ok(())
    }

    /// `SwitchCase`: `case test:` / `default:`, followed by its indented
    /// `consequent` statements (if any).
    ///
    /// juno `gen_js.rs:707-726`.
    pub(crate) fn gen_switch_case<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &SwitchCase<'gc>,
    ) -> Result<(), GenJsError> {
        let SwitchCase {
            metadata: _,
            test,
            consequent,
        } = inner;
        match test {
            Some(test) => {
                out!(self, "case ");
                self.gen_node(ctx, test, Some(Path::new(node, NodeField::test)))?;
            }
            None => {
                out!(self, "default");
            }
        }
        out!(self, ":");
        if !consequent.is_empty() {
            self.inc_indent();
            self.newline();
            self.visit_stmt_list(ctx, *consequent, Path::new(node, NodeField::consequent))?;
            self.dec_indent();
        }
        Ok(())
    }

    /// `LabeledStatement`: `label: body`.
    ///
    /// juno `gen_js.rs:728-736`.
    pub(crate) fn gen_labeled_statement<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &LabeledStatement<'gc>,
    ) -> Result<(), GenJsError> {
        let LabeledStatement {
            metadata: _,
            label,
            body,
            label_index: _,
        } = inner;
        self.gen_node(ctx, label, Some(Path::new(node, NodeField::label)))?;
        out!(self, ":");
        self.newline();
        self.gen_node(ctx, body, Some(Path::new(node, NodeField::body)))
    }

    /// `ExpressionStatement`: an expression followed by (its caller's)
    /// `;`, or a directive-prologue entry (`"use strict";`).
    ///
    /// juno `gen_js.rs:738-760` destructures `directive: _` and always
    /// prints `expression` (the child `StringLiteral`) through the normal
    /// cooked-value path.
    ///
    /// **DEVIATION from juno — a correctness fix, not a transcription.**
    /// ECMA-262's Use Strict Directive rule (and the Directive Prologue
    /// production generally, 14.1.1) is defined on the exact *source
    /// spelling* between the quotes, not on the string's semantic value
    /// (SV): `"use strict"` has SV `"use strict"` but is spelled with
    /// an escape, so it is *not* a use-strict directive. Our parser mirrors
    /// this exactly: `directive` is populated, with the raw source text
    /// between the quotes, only for a leading directive-prologue statement
    /// (`crates/parser/src/js/statements.rs`'s `parse_directive`, porting
    /// `JSParserImpl::parseDirective`, `lib/Parser/JSParserImpl.cpp:
    /// 7469-7509`); a string-literal statement outside prologue position
    /// always carries `INVALID_ATOM_BYTES` (`statements.rs:920`) regardless
    /// of its own spelling — `ExpressionStatement::directive`'s own doc
    /// comment in `crates/ast/src/node.rs` covers `try_directive_str`.
    ///
    /// Reprinting only the cooked `StringLiteral` value discards exactly
    /// the distinction that rule turns on: a plain string-literal statement
    /// in prologue position whose *cooked* value happens to equal `"use
    /// strict"` but was spelled with an escape (so it was *not* a
    /// directive) would come back out as literal `"use strict"` on
    /// reparse — now genuinely strict, a silent semantics change. This
    /// arm instead prints `directive`'s raw text verbatim
    /// ([`GenJS::print_directive_raw`]) whenever it is present, bypassing
    /// the child `StringLiteral`'s cooked-value escaper entirely; a
    /// non-directive spelling stays a non-directive spelling and a real
    /// one stays byte-identical. `ctx.try_bytes_str(directive)` doubles as
    /// the "is this a directive at all" test: it returns `None` for
    /// `INVALID_ATOM_BYTES` (confirmed by `hermes_atom_table`'s own test,
    /// `try_bytes_str(INVALID_ATOM_BYTES) == None`) and — per the field's
    /// construction, a raw source slice or an escape-free cooked value —
    /// can never return `None` for a genuine directive's raw text, so the
    /// non-directive path is never taken by mistake.
    pub(crate) fn gen_expression_statement<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ExpressionStatement<'gc>,
    ) -> Result<(), GenJsError> {
        let ExpressionStatement {
            metadata: _,
            expression,
            directive,
        } = inner;
        match ctx.try_bytes_str(directive.get()) {
            Some(raw) => {
                self.print_directive_raw(raw);
                Ok(())
            }
            None => self.print_child(
                ctx,
                Some(*expression),
                Path::new(node, NodeField::expression),
                ChildPos::Anywhere,
            ),
        }
    }

    /// Print an `ExpressionStatement`'s directive-prologue entry using
    /// `raw`'s exact source spelling, rather than the cooked-value escaper
    /// [`GenJS::print_escaped_string_literal`] used for an ordinary
    /// `StringLiteral`. See [`GenJS::gen_expression_statement`]'s doc
    /// comment for why this exists at all.
    ///
    /// Quoting: prefers [`GenJS::quote`]'s configured character unless
    /// `raw` contains it, in which case the other quote character is used;
    /// if `raw` contains both, the configured one is used with its own
    /// occurrences backslash-escaped. That one addition is safe: a raw
    /// directive's existing escapes (if any) are untouched literal
    /// backslashes already present in the source, and inserting one more
    /// before a bare quote character cannot turn a non-`"use strict"`
    /// spelling into `"use strict"`, nor the reverse — neither spelling
    /// contains a quote character. Not a juno function: juno's arm
    /// discards `directive` entirely (see the deviation this fixes, on
    /// `gen_expression_statement`'s doc comment).
    ///
    /// A raw `\n` (from a source-level `LineContinuation` — a backslash
    /// immediately followed by an actual newline inside the string) is
    /// forced via `force_newline_without_indent` rather than written as a
    /// literal byte, the same treatment `arms/literal.rs`'s
    /// `gen_template_literal` gives a raw `\n` in a quasi.
    fn print_directive_raw(&mut self, raw: &str) {
        let configured = self.quote().as_char();
        let other = if configured == '\'' { '"' } else { '\'' };
        let quote = if !raw.contains(configured) {
            configured
        } else if !raw.contains(other) {
            other
        } else {
            configured
        };
        out!(self, "{}", quote);
        let mut buf = [0u8; 4];
        for c in raw.chars() {
            if c == '\n' {
                self.force_newline_without_indent();
                continue;
            }
            if c == quote {
                out!(self, "\\");
            }
            self.write_char(c, &mut buf);
        }
        out!(self, "{}", quote);
    }

    /// `TryStatement`: `try block [catch handler] [finally finalizer]`.
    ///
    /// juno `gen_js.rs:762-786`.
    pub(crate) fn gen_try_statement<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TryStatement<'gc>,
    ) -> Result<(), GenJsError> {
        let TryStatement {
            metadata: _,
            block,
            handler,
            finalizer,
        } = inner;
        out!(self, "try");
        self.visit_stmt_or_block(
            ctx,
            block,
            ForceBlock::Yes,
            Path::new(node, NodeField::block),
        )?;
        if let Some(handler) = handler {
            self.gen_node(ctx, handler, Some(Path::new(node, NodeField::handler)))?;
        }
        if let Some(finalizer) = finalizer {
            out!(self, "finally");
            self.space(ForceSpace::No);
            self.visit_stmt_or_block(
                ctx,
                finalizer,
                ForceBlock::Yes,
                Path::new(node, NodeField::finalizer),
            )?;
        }
        Ok(())
    }

    /// `IfStatement`: `if (test) consequent [else alternate]`.
    ///
    /// The dangling-else hazard (`if (a) if (b) c(); else d();`, where
    /// `else` must bind to the inner `if`) is handled by forcing the
    /// *consequent* into a block whenever it is itself an `if` with no
    /// `else` of its own and this `IfStatement` has an `alternate` — see
    /// [`is_if_without_else`].
    ///
    /// juno `gen_js.rs:788-827`.
    pub(crate) fn gen_if_statement<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &IfStatement<'gc>,
    ) -> Result<(), GenJsError> {
        let IfStatement {
            metadata: _,
            test,
            consequent,
            alternate,
        } = inner;
        out!(self, "if");
        self.space(ForceSpace::No);
        out!(self, "(");
        self.gen_node(ctx, test, Some(Path::new(node, NodeField::test)))?;
        out!(self, ")");
        let force_block = if alternate.is_some() && is_if_without_else(consequent) {
            ForceBlock::Yes
        } else {
            ForceBlock::No
        };
        self.visit_stmt_or_block(
            ctx,
            consequent,
            force_block,
            Path::new(node, NodeField::consequent),
        )?;
        if let Some(alternate) = alternate {
            out!(self, "else");
            // `is_implicit_block` rather than a bare `BlockStatement` match:
            // an implicit block prints its body bare (see
            // [`GenJS::visit_stmt_or_block`]), so `else` is followed by a
            // keyword, not by `{`, and without the forced space
            // `Pretty::No` would run them together into `elsefunction`.
            self.space(if matches!(alternate, Node::BlockStatement(_)) && !is_implicit_block(alternate)
            {
                ForceSpace::No
            } else {
                ForceSpace::Yes
            });
            self.visit_stmt_or_block(
                ctx,
                alternate,
                ForceBlock::No,
                Path::new(node, NodeField::alternate),
            )?;
        }
        Ok(())
    }

    /// `CatchClause`: `catch [(param)] body`.
    ///
    /// juno `gen_js.rs:1335-1353`.
    pub(crate) fn gen_catch_clause<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &CatchClause<'gc>,
    ) -> Result<(), GenJsError> {
        let CatchClause {
            metadata: _,
            param,
            body,
            scope: _,
        } = inner;
        self.space(ForceSpace::No);
        out!(self, "catch");
        if let Some(param) = param {
            self.space(ForceSpace::No);
            out!(self, "(");
            self.gen_node(ctx, param, Some(Path::new(node, NodeField::param)))?;
            out!(self, ")");
        }
        self.visit_stmt_or_block(ctx, body, ForceBlock::Yes, Path::new(node, NodeField::body))?;
        Ok(())
    }

    /// `VariableDeclaration`: `kind decl0, decl1, ...` (e.g. `var x, y = 1`).
    ///
    /// juno `gen_js.rs:1356-1367`. See the module doc comment for why
    /// `kind` prints through `try_bytes_str` rather than a typed enum.
    pub(crate) fn gen_variable_declaration<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &VariableDeclaration<'gc>,
    ) -> Result<(), GenJsError> {
        let VariableDeclaration {
            metadata: _,
            kind,
            declarations,
        } = inner;
        let kind_str = ctx
            .try_bytes_str(kind.get())
            .ok_or(GenJsError::UnrepresentableIdentifier)?;
        out!(self, "{} ", kind_str);
        for (i, decl) in declarations.iter().enumerate() {
            if i > 0 {
                self.comma();
            }
            self.gen_node(ctx, decl, Some(Path::new(node, NodeField::declarations)))?;
        }
        Ok(())
    }

    /// `VariableDeclarator`: `id` or `id = init`.
    ///
    /// juno `gen_js.rs:1369-1387`: `init.visit(ctx, self, ...)` — a bare
    /// visit, not a `print_child`. See the module doc comment's third
    /// deviation for why this one routes `init` through `print_child`
    /// instead (the `for (var i = (a in b);;);` hazard, fixed in
    /// `precedence.rs`'s `need_parens`).
    pub(crate) fn gen_variable_declarator<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &VariableDeclarator<'gc>,
    ) -> Result<(), GenJsError> {
        let VariableDeclarator {
            metadata: _,
            init,
            id,
        } = inner;
        self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))?;
        if let Some(init) = init {
            let eq = match self.pretty() {
                Pretty::Yes => " = ",
                Pretty::No => "=",
            };
            self.space_before_equals(eq);
            out!(
                self,
                "{}",
                match self.pretty() {
                    Pretty::Yes => " = ",
                    Pretty::No => "=",
                }
            );
            self.print_child(
                ctx,
                Some(*init),
                Path::new(node, NodeField::init),
                ChildPos::Anywhere,
            )?;
        }
        Ok(())
    }

    /// `ObjectPattern`: `{prop0, prop1, ...}[: type_annotation]`.
    ///
    /// juno `gen_js.rs:1942-1957`.
    pub(crate) fn gen_object_pattern<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ObjectPattern<'gc>,
    ) -> Result<(), GenJsError> {
        let ObjectPattern {
            metadata: _,
            properties,
            type_annotation,
        } = inner;
        self.visit_props(ctx, *properties, Path::new(node, NodeField::properties))?;
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

    /// `ArrayPattern`: `[elem0, elem1, ...][: type_annotation]`.
    ///
    /// juno `gen_js.rs:1958-1978`.
    pub(crate) fn gen_array_pattern<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ArrayPattern<'gc>,
    ) -> Result<(), GenJsError> {
        let ArrayPattern {
            metadata: _,
            elements,
            type_annotation,
        } = inner;
        out!(self, "[");
        let mut last_is_hole = false;
        for (i, elem) in elements.iter().enumerate() {
            if i > 0 {
                self.comma();
            }
            last_is_hole = matches!(elem, Node::Empty(_));
            // An element the parser would have *reparsed* into a pattern has
            // to keep the parens that stopped it doing so. See
            // [`survives_pattern_reparse_only_in_parens`].
            let parens = survives_pattern_reparse_only_in_parens(elem);
            if parens {
                out!(self, "(");
            }
            self.gen_node(ctx, elem, Some(Path::new(node, NodeField::elements)))?;
            if parens {
                out!(self, ")");
            }
        }
        // A trailing elision needs its own comma.
        //
        // **DEVIATION from juno — a correctness fix found by the Tier 2
        // sweep** (`test/Parser/es6/arrow-non-simple-params.js`'s
        // `let bar = ([,,]) => {}`). An `Empty` element prints as nothing, so
        // `n` elements separated by `n - 1` commas spell only `n - 1` holes
        // when the last one is a hole: `[,,]` (two `Empty`s) regenerated as
        // `[,]`, which reparses to a *one*-element pattern — a different tree,
        // no diagnostic. `ArrayExpression` is immune because it carries the
        // parser's `trailingComma` flag and prints the extra comma from it;
        // `ArrayPattern` has no such field, so the hole must be recovered
        // structurally.
        if last_is_hole {
            self.comma();
        }
        out!(self, "]");
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

    /// `RestElement`: `...argument`.
    ///
    /// juno `gen_js.rs:1979-1985`.
    pub(crate) fn gen_rest_element<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &RestElement<'gc>,
    ) -> Result<(), GenJsError> {
        let RestElement {
            metadata: _,
            argument,
        } = inner;
        out!(self, "...");
        self.gen_node(ctx, argument, Some(Path::new(node, NodeField::argument)))
    }

    /// `AssignmentPattern`: `left = right` (a destructuring default).
    ///
    /// juno `gen_js.rs:1986-1999`.
    pub(crate) fn gen_assignment_pattern<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &AssignmentPattern<'gc>,
    ) -> Result<(), GenJsError> {
        let AssignmentPattern {
            metadata: _,
            left,
            right,
        } = inner;
        self.gen_node(ctx, left, Some(Path::new(node, NodeField::left)))?;
        self.space(ForceSpace::No);
        self.space_before_equals("=");
        out!(self, "=");
        self.space(ForceSpace::No);
        self.gen_node(ctx, right, Some(Path::new(node, NodeField::right)))
    }

    /// Print `node`, the body of a loop or a clause of an `if`/`try`,
    /// which may or may not already be a `BlockStatement`. Returns whether
    /// it printed as a block (`{ ... }`), which callers like
    /// [`GenJS::gen_do_while_statement`] use to decide between a space or
    /// a newline before a following keyword.
    ///
    /// juno `gen_js.rs:3525-3550`, plus the [`is_implicit_block`] guard —
    /// see that function for why an implicit block must not print braces.
    pub(crate) fn visit_stmt_or_block<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        force_block: ForceBlock,
        path: Path<'gc>,
    ) -> Result<bool, GenJsError> {
        if let Node::BlockStatement(BlockStatement {
            metadata: _,
            body,
            implicit,
            scope: _,
            buffer_id: _,
            is_lazy_function_body: _,
            param_yield: _,
            param_await: _,
            contains_arrow_functions: _,
            may_contain_arrow_functions_using_arguments: _,
        }) = node
        {
            // An implicit block is not in the source and must not be printed:
            // its brace-less body is the only spelling that reparses to it.
            // `force_block` still wins, because the caller only asks for it
            // where a brace-less statement would reparse as something else
            // entirely (a dangling `else`), and a wrong `implicit` flag is
            // the lesser corruption of the two.
            if !(*implicit).get() || force_block == ForceBlock::Yes {
                if body.is_empty() {
                    self.space(ForceSpace::No);
                    out!(self, "{{}}");
                    return Ok(true);
                }
                self.space(ForceSpace::No);
                out!(self, "{{");
                self.inc_indent();
                self.newline();
                self.visit_stmt_list(ctx, *body, Path::new(node, NodeField::body))?;
                self.dec_indent();
                self.newline();
                out!(self, "}}");
                return Ok(true);
            }
            // An implicit block: print its statements bare, exactly as the
            // source had them.
            self.inc_indent();
            self.newline();
            self.visit_stmt_list(ctx, *body, Path::new(node, NodeField::body))?;
            self.dec_indent();
            return Ok(false);
        }
        if force_block == ForceBlock::Yes {
            self.space(ForceSpace::No);
            out!(self, "{{");
            self.inc_indent();
            self.newline();
            self.visit_stmt_in_block(ctx, node, path)?;
            self.dec_indent();
            self.newline();
            out!(self, "}}");
            self.newline();
            Ok(true)
        } else {
            self.inc_indent();
            self.newline();
            self.visit_stmt_in_block(ctx, node, path)?;
            self.dec_indent();
            Ok(false)
        }
    }

    /// Print every statement in `list`, each through
    /// [`GenJS::visit_stmt_in_block`] (so each gets its own `;` and the
    /// list gets a newline between elements).
    ///
    /// juno `gen_js.rs:3552-3559`. Takes `list: NodeList<'gc>` by value —
    /// `NodeList` is `Copy` — matching `arms/expr.rs`'s `visit_props`
    /// rather than juno's `&NodeList<'gc>`.
    pub(crate) fn visit_stmt_list<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        list: NodeList<'gc>,
        path: Path<'gc>,
    ) -> Result<(), GenJsError> {
        for (i, stmt) in list.iter().enumerate() {
            if i > 0 {
                self.newline();
            }
            self.visit_stmt_in_block(ctx, stmt, path)?;
        }
        Ok(())
    }

    /// Print one statement, followed by `;` unless [`stmt_skip_semi`] says
    /// its own printing already ends in something that doesn't need one
    /// (e.g. a `BlockStatement`'s closing `}`, or an `if` whose consequent
    /// already got its own `;`).
    ///
    /// juno `gen_js.rs:3561-3568`.
    pub(crate) fn visit_stmt_in_block<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        stmt: &'gc Node<'gc>,
        path: Path<'gc>,
    ) -> Result<(), GenJsError> {
        self.gen_node(ctx, stmt, Some(path))?;
        if !stmt_skip_semi(Some(stmt)) {
            out!(self, ";");
        }
        Ok(())
    }
}

/// Whether `node`, sitting in an `ArrayPattern` element slot, is a kind the
/// parser's destructuring reparse would have consumed — so that the only
/// spelling which produces this tree is the parenthesized one.
///
/// **No juno counterpart — a correctness fix found by the Tier 2 sweep**
/// (`test/Parser/es6/reparse-array-destr.js`). When an array literal becomes
/// an assignment target, the parser rewrites it element by element:
/// `ArrayExpression` -> `ArrayPattern`, `ObjectExpression` -> `ObjectPattern`,
/// `AssignmentExpression` -> `AssignmentPattern`. A parenthesized element is
/// *not* rewritten — it is left in place for sema to reject as an invalid
/// assignment target — which is exactly how a tree with one of these kinds in
/// an element slot arises. Printed bare, `[(a = 1)] = t` becomes `[a = 1] = t`,
/// whose element reparses as an `AssignmentPattern`: a different tree, and no
/// diagnostic. As with the sibling rule in `precedence.rs` for the assignment
/// target itself, a valid program never has one of these kinds here, so this
/// only ever adds parens to trees that are already sema errors.
fn survives_pattern_reparse_only_in_parens(node: &Node) -> bool {
    matches!(
        node,
        Node::ArrayExpression(_) | Node::ObjectExpression(_) | Node::AssignmentExpression(_)
    )
}

/// Whether `node` is a `BlockStatement` the parser synthesized rather than
/// read from the source.
///
/// **No juno counterpart — a correctness fix found by the Tier 2 sweep.**
/// juno's AST has no `implicit` flag, so its generator prints every
/// `BlockStatement` with real braces. Hermes's parser sets `implicit` on
/// exactly one construct: ES2022 B.3.3's `if (x) function f() {}`, whose
/// function declaration it wraps in a synthetic block so that function
/// promotion sees a block scope (`parse_statement_or_function_declaration`,
/// `crates/parser/src/js/statements.rs:1324-1358`, the only site that passes
/// `true`, reached only from `parse_if_statement`'s consequent and
/// alternate). Printing braces there produces `if (x) { function f() {} }`,
/// which is a *real* block: the reparse has `implicit: false`, a different
/// tree with no diagnostic (`test/Parser/if-function.js`).
fn is_implicit_block(node: &Node) -> bool {
    match node {
        Node::BlockStatement(block) => block.implicit.get(),
        _ => false,
    }
}

/// Whether `node` is an `IfStatement` with no `else` of its own.
///
/// Used by [`GenJS::gen_if_statement`] to detect the dangling-else hazard:
/// `if (a) if (b) c(); else d();` must force the *outer* `if`'s consequent
/// (the inner `if`) into a block, `if (a) { if (b) c(); } else d();`, since
/// an `else` always binds to the nearest `if` — printed unparenthesized,
/// this source's `else` would silently move from the outer `if` to the
/// inner one on reparse.
///
/// juno `gen_js.rs:4049-4058`.
fn is_if_without_else(node: &Node) -> bool {
    match node {
        Node::IfStatement(IfStatement {
            metadata: _,
            test: _,
            consequent: _,
            alternate,
        }) => alternate.is_none(),
        _ => false,
    }
}
