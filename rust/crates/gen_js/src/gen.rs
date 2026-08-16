/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Options, the [`GenJS`] output struct, and the output primitives it is
//! built on.
//!
//! Ported from juno's `gen_js.rs:26-360` (options, struct, `gen_root`, the
//! `write_*` primitives) and `gen_js.rs:3196-3248` (indentation and spacing
//! helpers). See the plan's Task 2
//! (`doc/superpowers/plans/2026-08-15-gen-js-port.md`) and Adaptation Rules
//! for what changed: the sourcemap/`cur_token` machinery is dropped
//! entirely (spec §6), and `out_token!(self, node, ...)` collapses to
//! `out!(self, ...)` since there is no segment to record.

use std::fmt;
use std::io;
use std::io::BufWriter;
use std::io::Write;
use std::rc::Rc;

use hermes_ast::context::GCLock;
use hermes_ast::node::Node;
#[cfg(feature = "annotate")]
use hermes_sema::sem_context::SemContext;

use crate::precedence::ForceSpace;
use crate::GenJsError;

/// Options for JS generation.
///
/// `'s` is the borrow of the `SemContext` an `Annotation::Sem` carries;
/// it is `'static` in effect whenever `annotation` is [`Annotation::No`].
///
/// juno `gen_js.rs:26-40`.
pub struct Opt<'s> {
    /// Whether to pretty-print the generated JS.
    pub pretty: Pretty,

    /// How to annotate the generated source.
    pub annotation: Annotation<'s>,

    /// Whether to force a space after the `async` keyword in arrow
    /// functions. Kept even though it looks incidental: downstream code
    /// that pattern-matches `async` followed by whitespace depends on it
    /// (juno `gen_js.rs:437-441`).
    pub force_async_arrow_space: bool,

    /// If `Some`, a doc block to print verbatim at the top of the file,
    /// before anything else (see [`GenJS::gen_root`]'s doc-block preamble).
    pub doc_block: Option<Rc<String>>,

    /// Delimiter to use for string literals.
    pub quote: QuoteChar,
}

impl Default for Opt<'_> {
    /// juno `gen_js.rs:42-51`.
    fn default() -> Self {
        Opt {
            pretty: Pretty::Yes,
            annotation: Annotation::No,
            force_async_arrow_space: true,
            doc_block: None,
            quote: QuoteChar::Single,
        }
    }
}

impl Opt<'_> {
    /// Equivalent to [`Default::default`]. juno `gen_js.rs:55-59`.
    pub fn new() -> Self {
        Default::default()
    }
}

/// Whether to pretty-print the generated JS.
///
/// Does not do full formatting of the source, but does add indentation and
/// some extra spaces to make the source more readable.
///
/// juno `gen_js.rs:62-69`.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Pretty {
    /// Emit the most compact source that is still syntactically valid.
    No,
    /// Add indentation and readability spaces.
    Yes,
}

/// Delimiter to use for string literals.
///
/// juno `gen_js.rs:71-76`.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum QuoteChar {
    /// `'...'`
    Single,
    /// `"..."`
    Double,
}

impl QuoteChar {
    /// The character representation of the quote.
    ///
    /// juno `gen_js.rs:78-85`. First called by Task 4's `StringLiteral`/
    /// `DirectiveLiteral` arms (`arms/literal.rs`).
    #[inline]
    pub(crate) fn as_char(self) -> char {
        match self {
            Self::Single => '\'',
            Self::Double => '"',
        }
    }
}

/// How to annotate the generated source.
///
/// juno `gen_js.rs:220-224`.
///
/// `Annotation::Sem` exists only under the `annotate` feature, which is
/// **off by default** — it is the crate's sole reason to depend on
/// `hermes-sema`, and it is a debugging aid rather than part of ordinary
/// generation.
///
/// The lifetime parameter is present in both feature states, so
/// `Annotation<'s>` and the `Opt<'s>` that holds it keep the same arity
/// whether or not `annotate` is enabled: a signature written against one
/// state still compiles under the other. That is what the otherwise-pointless
/// hidden variant below is for.
pub enum Annotation<'s> {
    /// No annotation: plain source text.
    No,
    /// Annotate identifiers with their resolved binding, taken from a
    /// completed semantic-analysis pass.
    #[cfg(feature = "annotate")]
    Sem(&'s SemContext),
    /// Never constructed, and not part of the API. It exists only so `'s`
    /// stays used when `annotate` is off, keeping this type's arity stable
    /// across feature states. Do not match on it.
    #[cfg(not(feature = "annotate"))]
    #[doc(hidden)]
    _Phantom(core::marker::PhantomData<&'s ()>),
}

/// Associativity direction.
///
/// Used by the precedence table (Task 3); ported here alongside the rest of
/// the option types per the plan's Task 2, Step 1.
///
/// juno `gen_js.rs:98-108`.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum Assoc {
    /// Left to right associativity.
    Ltr,
    /// Right to left associativity.
    Rtl,
}

/// Generate JS for `root` and write it to `out`.
///
/// juno `gen_js.rs:87-92`. Two deliberate deviations from juno's signature
/// (`generate(out, ctx: &mut Context, root: &NodeRc, opt) -> io::Result<SourceMap>`,
/// also the shape sketched in spec §3):
///
/// - Returns `Result<(), GenJsError>` rather than `io::Result<SourceMap>`:
///   the sourcemap was a documented FIXME stub (spec §6) and is dropped
///   entirely, and generation failures beyond a sink `io::Error` (an
///   unsupported node kind, an unrepresentable identifier) are reported
///   through [`GenJsError`] rather than being unreachable.
/// - Takes `ctx: &GCLock` and `root: &Node` rather than `ctx: &mut Context`
///   and `root: &NodeRc`. juno's `Context`/`NodeRc` are its own arena types,
///   unrelated to ours; our `hermes_ast::context::ParsedJS` (the crate every
///   caller actually has one of) exposes the AST exclusively through
///   `with_program`'s `&GCLock`/`&Node` closure — there is no public
///   `&mut Context` accessor, so a `&mut Context`-shaped `generate` would be
///   uncallable from outside the parser crate. `dispatch::GenJS::gen_node`
///   already takes `&GCLock`/`&Node` for the same reason, so this keeps the
///   whole crate's node-processing functions consistent in shape. A later
///   task's `ParsedJS::to_js` facade (spec §3) is what calls this from
///   inside a `with_program`-style closure.
///
/// `ctx`'s own lifetime parameters are deliberately independent of `root`'s
/// `'gc` (`&GCLock<'_, '_>`, not `&GCLock<'gc, '_>`) rather than mirroring
/// [`GCLock`]'s two-parameter shape exactly onto `'gc`. `GCLock<'ast, 'ctx>`
/// is invariant in both parameters (see `rust/crates/sema/examples/print_bindings.rs`),
/// and `ParsedJS::with_program`'s closure bound is
/// `for<'gc> FnOnce(&'gc GCLock<'static, '_>, &'gc Node<'gc>) -> R` — a
/// universally-quantified `'gc` that can never be forced to equal `'static`
/// or unified with `GCLock`'s own fixed arena parameter. Tying them together
/// here would make this function, and everything it calls, uncallable from
/// `with_program`.
pub fn generate<'gc>(
    out: &mut dyn Write,
    ctx: &GCLock<'_, '_>,
    root: &'gc Node<'gc>,
    opt: Opt<'_>,
) -> Result<(), GenJsError> {
    GenJS::gen_root(out, ctx, root, opt)
}

/// Generator for output JS. Walks the AST to output real JS.
///
/// `'s` is [`Opt`]'s annotation borrow; `'w` is the output sink's borrow.
///
/// juno `gen_js.rs:226-243`. Dropped relative to juno: the `cur_token`
/// field, the `sourcemap` field, and `position` (a `SourceLoc` that existed
/// solely to feed sourcemap segments) — all sourcemap bookkeeping, per spec
/// §6.
pub struct GenJS<'s, 'w> {
    /// Where to write the generated JS.
    out: BufWriter<&'w mut dyn Write>,

    /// Options for generating JS.
    opt: Opt<'s>,

    /// Size of the indentation step.
    ///
    /// Read by [`GenJS::inc_indent`]/[`GenJS::dec_indent`], first called by
    /// Task 6's `arms/stmt.rs` (`BlockStatement`'s arm and
    /// `visit_stmt_or_block`).
    indent_step: usize,

    /// Current indentation level, used in pretty mode.
    indent: usize,

    /// `Some(err)` if the sink has returned an error, else `None`. Once set,
    /// every `write_*` call becomes a no-op instead of propagating from
    /// every call site (juno `gen_js.rs:353-357`); the deferred error is
    /// surfaced once, at the end of [`GenJS::gen_root`].
    error: Option<io::Error>,

    /// The last byte written, or `0` if nothing has been written yet.
    ///
    /// **No juno counterpart.** Task 17 added it for
    /// [`GenJS::space_before_equals`], the one place this crate needs to
    /// know what it just printed: in `Pretty::No` there is no whitespace to
    /// keep two adjacent punctuators from being lexed as one longer one.
    /// Only the last *byte* is kept, not the last token — that is all the
    /// maximal-munch question needs, and a token stack would have to be
    /// maintained by all 271 arms.
    last_byte: u8,
}

/// Print to the output stream if no errors have been seen so far.
///
/// `$gen_js` is a mutable reference to the [`GenJS`] struct. `$arg`
/// arguments follow the format pattern used by `format!`. The output must be
/// ASCII and contain no newlines.
///
/// juno `gen_js.rs:245-250`. juno also has `out_token!`, which additionally
/// records a sourcemap segment before delegating to `out!`; since the
/// sourcemap is dropped (spec §6), every `out_token!(self, node, ...)` call
/// site in the ported arms collapses to plain `out!(self, ...)`.
#[macro_export]
macro_rules! out {
    ($gen_js:expr, $($arg:tt)*) => {{
        $gen_js.write_ascii(format_args!($($arg)*));
    }}
}

impl<'s, 'w> GenJS<'s, 'w> {
    /// Generate JS for `root` and flush the output.
    ///
    /// If at any point JS generation resulted in an error, returns
    /// `Err(err)`; otherwise returns `Ok(())`.
    ///
    /// juno `gen_js.rs:259-296` (struct construction and the doc-block
    /// preamble) and `gen_js.rs:298-306` (the walk, final flush, and error
    /// check). Dropped: the source-map construction/iteration
    /// (`gen_js.rs:280-284`) and `flush_cur_token` (`gen_js.rs:301`, itself
    /// dropped — see [`GenJS`]'s doc comment).
    pub(crate) fn gen_root<'gc>(
        writer: &'w mut dyn Write,
        ctx: &GCLock<'_, '_>,
        root: &'gc Node<'gc>,
        opt: Opt<'s>,
    ) -> Result<(), GenJsError> {
        let mut gen_js = GenJS {
            out: BufWriter::new(writer),
            opt,
            indent_step: 2,
            indent: 0,
            error: None,
            last_byte: 0,
        };

        // The doc-block preamble: emitted before anything else, with `\n`
        // mapped to `force_newline_without_indent` rather than a literal
        // newline byte, so indentation state stays consistent afterward.
        // juno `gen_js.rs:291-300`.
        if let Some(doc_block) = gen_js.opt.doc_block.clone() {
            let mut buf = [0u8; 4];
            for c in doc_block.chars() {
                if c == '\n' {
                    gen_js.force_newline_without_indent();
                } else {
                    gen_js.write_char(c, &mut buf);
                }
            }
        }

        // `root` is always a `Program` in practice (the parser facade never
        // hands out anything else as its top-level node); `Program` now has
        // a real dispatch arm (`arms/stmt.rs`'s `GenJS::gen_program`, Task
        // 6), which prints its statement list through `visit_stmt_list` —
        // juno's separators, semicolons, and directive-prologue handling
        // all included. Task 2's version special-cased an empty `Program`
        // body directly here, calling raw `gen_node` per top-level
        // statement instead of `visit_stmt_in_block`; that shortcut is gone
        // now that the real arm exists (see `arms/stmt.rs`'s module doc
        // comment for why it had to go, not just get "reconciled" with the
        // new arm: it silently dropped every statement's trailing `;` and
        // inter-statement newline, invisible only because every prior
        // task's test used a single top-level statement).
        gen_js.gen_node(ctx, root, None)?;
        // juno always appends a trailing newline here, unconditionally
        // (`gen_js.rs:299`) — even for empty input, so the empty-program
        // smoke test's expected output is `"\n"`, not `""`.
        gen_js.force_newline();

        match gen_js.error.take() {
            None => gen_js.out.flush().map_err(GenJsError::Io),
            Some(err) => Err(GenJsError::Io(err)),
        }
    }

    /// Write to the `out` writer if we haven't seen any errors.
    ///
    /// If we have seen any errors, do nothing. Used via the [`out!`] macro.
    /// The output must be ASCII and contain no newlines.
    ///
    /// juno `gen_js.rs:308-320`. `position` tracking is dropped along with
    /// the rest of the sourcemap state (spec §6).
    pub(crate) fn write_ascii(&mut self, args: fmt::Arguments<'_>) {
        if self.error.is_none() {
            let buf = format!("{}", args);
            debug_assert!(buf.is_ascii(), "Output must be ASCII");
            debug_assert!(!buf.contains('\n'), "Output must have no newlines");
            if let Some(b) = buf.as_bytes().last() {
                self.last_byte = *b;
            }
            if let Err(e) = self.out.write_all(buf.as_bytes()) {
                self.error = Some(e);
            }
        }
    }

    /// Write a single unicode character to the `out` writer if we haven't
    /// seen any errors. The character must not be a newline. `dst` is a
    /// scratch buffer for UTF-8 encoding.
    ///
    /// juno `gen_js.rs:322-330`.
    pub(crate) fn write_char(&mut self, ch: char, dst: &mut [u8]) {
        debug_assert!(ch != '\n', "Output must not contain newlines");
        if self.error.is_none() {
            let encoded = ch.encode_utf8(dst);
            if let Some(b) = encoded.as_bytes().last() {
                self.last_byte = *b;
            }
            if let Err(e) = self.out.write_all(encoded.as_bytes()) {
                self.error = Some(e);
            }
        }
    }

    /// Write unicode text to the `out` writer if we haven't seen any
    /// errors. The output must contain no newlines.
    ///
    /// juno `gen_js.rs:332-345`. First called by Task 4's arms
    /// (`arms/literal.rs`) to write already-`try_bytes_str`-decoded
    /// identifier/`BigIntLiteral`/`RegExpLiteral` text.
    pub(crate) fn write_utf8(&mut self, s: &str) {
        debug_assert!(
            !s.chars().any(|c| c == '\n'),
            "Output must not contain newlines"
        );
        if self.error.is_none() {
            if let Some(b) = s.as_bytes().last() {
                self.last_byte = *b;
            }
            if let Err(e) = self.out.write_all(s.as_bytes()) {
                self.error = Some(e);
            }
        }
    }

    /// Emit a space if the byte just written would be lexed together with an
    /// immediately following `=` as one longer punctuator.
    ///
    /// **Call this immediately before writing any token that starts with
    /// `=`** — `=`, `=>`, `==`, `===`. It is deliberately *not* hooked into
    /// [`GenJS::write_ascii`]: the string-literal escaper writes ordinary
    /// literal characters through that same path, and a central hook turned
    /// `"YQ=="` into `"YQ= ="` and `"===="` into `"= = = ="` — measured on
    /// 12 files of the Tier 2 sweep (`test/hermes/atob.js`,
    /// `prohibit-invoke-backends.js`, `global-var-no-clear.js`,
    /// `ffi/fopen.js`, …). Maximal munch is a question about *tokens*, and
    /// only the arms know which of their output is a token.
    ///
    /// **No juno counterpart — a correctness fix (defect 35) found by
    /// `tests/paren_matrix.rs`.** In `Pretty::No` this crate writes `=` and
    /// `=>` with no separator, which is fine after an identifier, a `)` or a
    /// `]`, and wrong after an operator character. Two shapes reach it from
    /// real source:
    ///
    /// | source | was emitted | reparse |
    /// |---|---|---|
    /// | `var v: * = 1;` | `var v:*=1;` | `*=` — "unexpected token" |
    /// | `class K { p: * = 1; }` | `class K{p:*=1;}` | same |
    /// | `function g(a: * = 1) {}` | `function g(a:*=1){}` | same |
    /// | `var f = (x: mixed): * => 1;` | `var f=(x:mixed):*=>1;` | same |
    /// | `t = <a/> == b;` | `t=<a />==b;` | `>=` — "'>' expected at end of JSX tag" |
    ///
    /// The first four are Flow's `ExistsTypeAnnotation`, whose whole
    /// spelling is `*`; the last is a self-closing JSX tag, whose `>` the
    /// JSX-tag lexer munches together with a following `=`.
    ///
    /// The byte set is every punctuator this lexer can extend with `=`
    /// (`*=`, `/=`, `%=`, `+=`, `-=`, `&=`, `|=`, `^=`, `!=`, `==`, `=>`,
    /// `>=`, `<=`), **including** `>` and `<`. Including `>` costs one space
    /// after a type-argument list in compact mode (`var v:A<B> =1;`) where
    /// none is strictly needed — the parser lexes that `>` in
    /// `GrammarContext::Type`, which splits `>=`, so `var v:A<B>=1;` does in
    /// fact reparse (measured). It is included anyway because the JSX case
    /// above proves the same byte is *not* safe in every context, and this
    /// function has no way to know which lexer context its caller's output
    /// will be read in. A redundant space is inert (a space can never
    /// trigger ASI — only a line terminator can); a missing one is a
    /// reparse failure.
    ///
    pub(crate) fn space_before_equals(&mut self, next: &str) {
        if next.starts_with('=')
            && matches!(
                self.last_byte,
                b'*' | b'/' | b'%' | b'+' | b'-' | b'&' | b'|' | b'^' | b'!' | b'=' | b'>' | b'<'
            )
        {
            out!(self, " ");
        }
    }

    /// Increase the indent level.
    ///
    /// juno `gen_js.rs:3196-3199`. First called by Task 6's `arms/stmt.rs`
    /// (`BlockStatement`'s arm and `visit_stmt_or_block`) to nest a block.
    pub(crate) fn inc_indent(&mut self) {
        self.indent += self.indent_step;
    }

    /// Decrease the indent level.
    ///
    /// juno `gen_js.rs:3201-3204`. First called by Task 6's `arms/stmt.rs`
    /// to close a nested block.
    pub(crate) fn dec_indent(&mut self) {
        self.indent -= self.indent_step;
    }

    /// Print a `,`, with a trailing space in pretty mode.
    ///
    /// juno `gen_js.rs:3206-3215`. First called by Task 5's arms
    /// (`arms/expr.rs`) to separate `SequenceExpression`/`ArrayExpression`/
    /// `NewExpression`/`CallExpression`/`OptionalCallExpression` elements and
    /// `visit_props`' properties.
    pub(crate) fn comma(&mut self) {
        out!(
            self,
            "{}",
            match self.opt.pretty {
                Pretty::No => ",",
                Pretty::Yes => ", ",
            }
        )
    }

    /// Print a ' ' if forced by `force` or pretty mode.
    ///
    /// juno `gen_js.rs:3217-3222`. Takes [`ForceSpace`] (`precedence.rs`)
    /// rather than juno's typed enum directly — Task 4's `Identifier` arm
    /// (`arms/literal.rs`) was the first caller, passing `ForceSpace::No` for
    /// every call site since nothing yet needed a space independent of
    /// pretty mode; Task 5's `BinaryExpression`/`UnaryExpression` arms
    /// (`arms/expr.rs`) are the first that do — `a in b`/`typeof x` would
    /// merge into one identifier-like token (`ainb`/`typeofx`) without a
    /// space even in compact mode, unlike `a+b`, which stays unambiguous.
    pub(crate) fn space(&mut self, force: ForceSpace) {
        if self.opt.pretty == Pretty::Yes || force == ForceSpace::Yes {
            out!(self, " ");
        }
    }

    /// Print a newline and indent, if pretty.
    ///
    /// juno `gen_js.rs:3224-3229`. First called by Task 6's `arms/stmt.rs`
    /// for pretty-mode-only line breaks between statements.
    pub(crate) fn newline(&mut self) {
        if self.opt.pretty == Pretty::Yes {
            self.force_newline();
        }
    }

    /// Print a newline and indent, unconditionally.
    ///
    /// juno `gen_js.rs:3231-3235`.
    pub(crate) fn force_newline(&mut self) {
        self.force_newline_without_indent();
        // `self.indent` is copied to a local first: `format_args!` inside
        // `out!` holds a live reference to its arguments for the duration of
        // the `write_ascii` call, which conflicts with that call's own `&mut
        // self` reborrow if the argument still reads through `self`.
        let indent = self.indent;
        out!(self, "{:indent$}", "", indent = indent);
    }

    /// Print a newline without any indent after, unconditionally.
    ///
    /// juno `gen_js.rs:3237-3243`. `position` tracking is dropped along with
    /// the rest of the sourcemap state (spec §6).
    pub(crate) fn force_newline_without_indent(&mut self) {
        if self.error.is_none() {
            if let Err(e) = self.out.write(b"\n") {
                self.error = Some(e);
            }
        }
    }

    /// Whether output is being pretty-printed.
    ///
    /// `opt` itself stays private to this module; `precedence.rs`'s
    /// `get_precedence` (the `NewExpression` arm, juno `gen_js.rs:3624`) and
    /// `need_parens` (juno `gen_js.rs:3754`) are the first callers outside
    /// this file that need to read it. Added for Task 3
    /// (`doc/superpowers/plans/2026-08-15-gen-js-port.md`).
    pub(crate) fn pretty(&self) -> Pretty {
        self.opt.pretty
    }

    /// Delimiter to use for string literals.
    ///
    /// `opt` itself stays private to this module (see [`GenJS::pretty`]);
    /// Task 4's `arms/literal.rs` (`StringLiteral`/`DirectiveLiteral` arms)
    /// is the first caller outside this file that needs it.
    pub(crate) fn quote(&self) -> QuoteChar {
        self.opt.quote
    }

    /// Whether to force a space after `async` in an `ArrowFunctionExpression`
    /// even when compact output would otherwise omit it.
    ///
    /// `opt` itself stays private to this module (see [`GenJS::pretty`]);
    /// Task 7's `arms/func.rs` (`gen_arrow_function_expression`) is the
    /// first caller outside this file that needs it.
    pub(crate) fn force_async_arrow_space(&self) -> bool {
        self.opt.force_async_arrow_space
    }

    /// The completed [`SemContext`] backing [`Annotation::Sem`], or `None`
    /// under [`Annotation::No`].
    ///
    /// `opt` itself stays private to this module (see [`GenJS::pretty`]);
    /// Task 14's `annotate.rs` (`GenJS::annotate_identifier`) is the first
    /// caller outside this file that needs it. Returns `Option<&'s
    /// SemContext>` — tied to `Opt`'s own `'s`, not to `&self`'s borrow —
    /// rather than matching `self.opt.annotation` by value, so the caller
    /// can still take `&mut self` (e.g. for the `out!` calls that print the
    /// annotation) once this call returns; matching `&self.opt.annotation`
    /// and copying out the `Copy` `&'s SemContext` payload achieves that
    /// without requiring [`Annotation`] itself to be `Copy`.
    #[cfg(feature = "annotate")]
    pub(crate) fn sem_context(&self) -> Option<&'s SemContext> {
        match &self.opt.annotation {
            Annotation::No => None,
            Annotation::Sem(sem) => Some(*sem),
        }
    }
}

#[cfg(test)]
impl<'s, 'w> GenJS<'s, 'w> {
    /// Test-only constructor: builds a live [`GenJS`] over `sink` without
    /// going through [`GenJS::gen_root`]'s doc-block/root-node walk.
    ///
    /// `precedence.rs`'s unit tests (Task 3) need a real `GenJS` to call
    /// `get_precedence`/`need_parens` on directly, without generating a full
    /// program; `gen_root` always drives a complete walk and flushes, so it
    /// cannot serve that purpose. Not part of juno, which has no equivalent
    /// need (its tests drive the whole `generate` pipeline — see
    /// `unsupported/juno/crates/juno/tests/gen_js/mod.rs`).
    pub(crate) fn for_test(sink: &'w mut dyn Write, opt: Opt<'s>) -> Self {
        GenJS {
            out: BufWriter::new(sink),
            opt,
            indent_step: 2,
            indent: 0,
            error: None,
            last_byte: 0,
        }
    }
}
