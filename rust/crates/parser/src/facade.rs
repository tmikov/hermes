/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The convenience front door: [`parse`] a string, get a [`ParsedJS`].
//!
//! This module adds no parsing behavior. It is a thin assembly of the pieces
//! the `ast-dump` bin wires up by hand — an [`ast::context::Context`] (the AST
//! arena), a [`SourceErrorManager`] (source buffers + diagnostics), a
//! [`JSLexer`], and a [`JSParserImpl`] — into one call, so that a consumer who
//! only wants "source in, AST out" does not have to know the assembly order.
//!
//! Everything it uses stays public: for lazy parsing, a custom
//! [`support::diag::DiagHandler`], a shared `Context` across several files, or
//! any other control the façade does not expose, drive
//! [`crate::js::JSParserImpl`] directly the way
//! `crates/tools/src/bin/ast_dump.rs` does.
//!
//! # Lifetime model
//!
//! AST nodes live in the `Context` arena and are only reachable while a
//! [`GCLock`] is held, so [`ParsedJS`] owns the `Context` and keeps the
//! `Program` node pinned with an [`ast::context::NodeRc`]. Reading the AST
//! therefore goes through [`ParsedJS::with_program`], which takes the lock for
//! the duration of a closure. Only one `GCLock` may exist per thread at a
//! time, so `with_program` calls must not be nested.

use ast::context::{Context, GCLock, NodeRc};
use ast::dump::dump_estree_json_with_sm;
use ast::dump::{ESTreeDumpMode, ESTreeRawProp, LocationDumpMode};
use ast::node::Node;
use support::diag::ResolvedDiagnostic;
use support::diag::{CollectingHandler, DiagKind, OutputOptions};
use support::manager::SourceErrorManager;
use support::render::render_diagnostic;

use crate::js::JSParserImpl;
use crate::lexer::{GrammarContext, JSLexer};

/// Which dialect(s) the parser accepts, plus forced strict mode.
///
/// `Default` is plain ECMAScript, non-strict — every flag `false`. Each field
/// maps to the identically-named [`ast::context::Context`] flag, which is
/// where the parser reads it from; the C++ `Context` getters cited in that
/// module are the authoritative semantics. Two fields set more than their own
/// flag, as documented below.
///
/// ```
/// use parser::ParseFlags;
/// let flags = ParseFlags { parse_flow: true, ..Default::default() };
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ParseFlags {
    /// Parse the Flow type grammar (`hermesc -parse-flow`).
    ///
    /// This also enables the Flow *ambiguous-expression* grammar (typed
    /// arrows, `as`, type-args on call/new, type casts), because `hermesc`'s
    /// `-parse-flow` means `ParseFlowSetting::ALL`. Flow and TypeScript are
    /// mutually exclusive dialects: do not set this together with
    /// [`Self::parse_ts`].
    pub parse_flow: bool,

    /// Parse Flow `component`/`hook` declarations
    /// (`hermesc -Xparse-component-syntax`). Implies [`Self::parse_flow`].
    pub parse_flow_component_syntax: bool,

    /// Parse Flow `record` declarations and expressions
    /// (`hermesc -Xparse-flow-records`). Implies [`Self::parse_flow`].
    pub parse_flow_records: bool,

    /// Parse Flow `match` expressions and statements
    /// (`hermesc -Xparse-flow-match`). Implies [`Self::parse_flow`].
    pub parse_flow_match: bool,

    /// Parse the TypeScript type grammar (`hermesc -parse-ts`). Mutually
    /// exclusive with [`Self::parse_flow`] and its extensions.
    pub parse_ts: bool,

    /// Parse JSX (`hermesc -parse-jsx`). Independent of the type dialect:
    /// combines with Flow, with TypeScript, or with neither. Note that
    /// enabling JSX disables the TypeScript `<Type>expr` assertion grammar,
    /// exactly as in the C++ parser.
    pub parse_jsx: bool,

    /// Force strict mode for the whole source, as if it began with a
    /// `"use strict"` directive. Sets `Context::enable_strict_mode`.
    pub strict_mode: bool,
}

impl ParseFlags {
    /// Apply these flags to a fresh `Context`.
    ///
    /// Mirrors the flag wiring in `crates/tools/src/bin/ast_dump.rs`,
    /// including the two implications documented on the fields: the three
    /// `parse_flow_*` extensions turn on `parse_flow`, and `parse_flow` turns
    /// on the ambiguous-expression grammar.
    fn apply(&self, ctx: &mut Context<'_>) {
        let parse_flow = self.parse_flow
            || self.parse_flow_component_syntax
            || self.parse_flow_records
            || self.parse_flow_match;
        ctx.set_parse_flow(parse_flow);
        ctx.set_parse_flow_ambiguous(parse_flow);
        ctx.set_parse_flow_component_syntax(self.parse_flow_component_syntax);
        ctx.set_parse_flow_records(self.parse_flow_records);
        ctx.set_parse_flow_match(self.parse_flow_match);
        ctx.set_parse_ts(self.parse_ts);
        ctx.set_parse_jsx(self.parse_jsx);
        if self.strict_mode {
            ctx.enable_strict_mode();
        }
    }
}

/// A successful parse: the AST arena, the source manager, and the `Program`
/// node, owned together.
///
/// The AST is only valid while its arena is alive, so this value owns the
/// arena; dropping it frees the AST. Read the tree with
/// [`with_program`](Self::with_program), or dump it with
/// [`to_estree_json`](Self::to_estree_json).
///
/// **Not `Send`.** The arena uses `Cell`/`UnsafeCell` and the `GCLock` that
/// guards it is thread-local by design, so a `ParsedJS` cannot be moved to
/// another thread — parse on the thread that will read the AST. (The name
/// keeps the crate's `JSParserImpl`/`JSLexer` casing rather than Rust's
/// `ParsedJs`; that is deliberate, for consistency inside the port.)
pub struct ParsedJS {
    /// The `Program` node, pinned so it survives outside a `GCLock`.
    ///
    /// Always `Some` once [`parse_named`] has returned `Ok`; the `Option`
    /// exists only so the value can be built before the parse runs (the
    /// `NodeRc` cannot be created until a `GCLock` over `ctx` exists, and
    /// `ctx` must not move afterwards).
    ///
    /// **Must be declared before `ctx`**: fields drop in declaration order and
    /// `Context::drop` panics if a `NodeRc` into it is still alive.
    program: Option<NodeRc>,

    /// The arena owning every node of the AST.
    ctx: Context<'static>,

    /// Owns the source buffer and recorded the diagnostics. Needed by the
    /// ESTree dumper for `loc`/`range`/`raw`.
    sm: SourceErrorManager,
}

impl ParsedJS {
    /// Run `f` with the arena locked and the `Program` node in hand.
    ///
    /// This is the read path for the AST: walk it with an
    /// [`ast::visitor::Visitor`], match on [`Node`] arms, or read
    /// [`Node::kind`]. References into the arena cannot escape the closure —
    /// their lifetime ends with the lock — so return owned data instead. The
    /// one thing that *can* escape is an [`ast::context::NodeRc`], which is
    /// refcounted rather than borrowed; dropping this `ParsedJS` while such a
    /// handle is still alive panics inside `Context::drop`.
    ///
    /// The bound is higher-ranked because [`Node`] is *invariant* in its
    /// lifetime: a walker (`ast::visitor::Visitor<'gc>`) needs the node
    /// reference and the node's own lifetime to be the same `'gc`, which only
    /// a `for<'gc>` closure can promise.
    ///
    /// # Panics
    ///
    /// Panics if another [`GCLock`] is active on this thread — in particular
    /// if `with_program` is called from inside another `with_program`.
    pub fn with_program<R, F>(&mut self, f: F) -> R
    where
        F: for<'gc> FnOnce(&'gc GCLock<'static, '_>, &'gc Node<'gc>) -> R,
    {
        // Disjoint field borrows: `program` immutably, `ctx` mutably.
        let program = self.program.as_ref().expect("ParsedJS without program");
        let gc = self.ctx.lock();
        let node = program.node(&gc);
        f(&gc, node)
    }

    /// Dump the AST as ESTree JSON: empty fields hidden, no `loc`/`range`,
    /// and `"raw"` source text on numeric literals (the only node the dumper
    /// emits it for). That is `hermesc -dump-ast` plus
    /// `-include-raw-ast-prop`.
    ///
    /// `pretty` selects indented output. For other dumper settings use
    /// [`to_estree_json_with`](Self::to_estree_json_with).
    ///
    /// # Panics
    ///
    /// Takes the arena lock, so it panics if another [`GCLock`] is live on
    /// this thread — in particular when called from inside
    /// [`with_program`](Self::with_program).
    pub fn to_estree_json(&mut self, pretty: bool) -> String {
        self.to_estree_json_with(
            pretty,
            ESTreeDumpMode::HideEmpty,
            LocationDumpMode::None,
            ESTreeRawProp::Include,
        )
    }

    /// Dump the AST as ESTree JSON with full control over the dumper, which
    /// is [`ast::dump::dump_estree_json_with_sm`] — see it for what each
    /// argument does.
    ///
    /// # Panics
    ///
    /// Takes the arena lock, so it panics if another [`GCLock`] is live on
    /// this thread — in particular when called from inside
    /// [`with_program`](Self::with_program).
    pub fn to_estree_json_with(
        &mut self,
        pretty: bool,
        mode: ESTreeDumpMode,
        loc_mode: LocationDumpMode,
        raw_prop: ESTreeRawProp,
    ) -> String {
        let sm = &self.sm;
        let program = self.program.as_ref().expect("ParsedJS without program");
        let gc = self.ctx.lock();
        let mut out = String::new();
        dump_estree_json_with_sm(
            &mut out,
            program.node(&gc),
            pretty,
            mode,
            sm,
            loc_mode,
            raw_prop,
            &gc.ctx().atom_table,
        );
        out
    }

    /// The diagnostics recorded while parsing.
    ///
    /// An `Ok` parse reported no errors, so these are warnings and notes.
    /// Render one with [`support::render::render_diagnostic`].
    pub fn diagnostics(&self) -> &[ResolvedDiagnostic] {
        collected(&self.sm)
    }

    /// The source manager that owns the parsed buffer, for coordinate lookups
    /// (`find_coords`) and for driving the `ast` dumper by hand.
    pub fn source_manager(&self) -> &SourceErrorManager {
        &self.sm
    }
}

impl std::fmt::Debug for ParsedJS {
    /// Summarizes the arena rather than printing the AST, which can be huge.
    /// (Hand-written because `SourceErrorManager` is not `Debug`.)
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParsedJS")
            .field("nodes", &self.ctx.num_nodes())
            .field("diagnostics", &self.diagnostics().len())
            .finish_non_exhaustive()
    }
}

/// A parse that reported at least one error.
///
/// There is no AST to carry: the parser returns no tree once it has reported
/// an error, so only the diagnostics survive.
#[derive(Debug, Clone)]
pub struct ParseError {
    /// Every diagnostic recorded, in emission order: errors, warnings, notes.
    diagnostics: Vec<ResolvedDiagnostic>,
    /// How many of them were errors.
    error_count: u32,
}

impl ParseError {
    /// Every diagnostic recorded during the parse, in emission order.
    pub fn diagnostics(&self) -> &[ResolvedDiagnostic] {
        &self.diagnostics
    }

    /// How many of the diagnostics are errors. Greater than zero for every
    /// `ParseError` the parser produces — it fails only after reporting.
    pub fn error_count(&self) -> u32 {
        self.error_count
    }

    /// The diagnostics rendered one string each, LLVM-style (location line,
    /// message, source line, caret), without ANSI colors.
    pub fn messages(&self) -> Vec<String> {
        let opts = OutputOptions {
            show_colors: false,
            ..OutputOptions::default()
        };
        self.diagnostics
            .iter()
            .map(|d| render_diagnostic(d, &opts))
            .collect()
    }
}

impl std::fmt::Display for ParseError {
    /// A single line — count plus the first error's location and text — as
    /// error types are expected to produce. The full LLVM-style rendering
    /// (source line and caret) is [`messages`](Self::messages); the
    /// structured form is [`diagnostics`](Self::diagnostics).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let plural = if self.error_count == 1 { "" } else { "s" };
        match self.diagnostics.iter().find(|d| d.kind == DiagKind::Error) {
            Some(d) => write!(
                f,
                "{} parse error{plural}; first at {}:{}:{}: {}",
                self.error_count, d.file_name, d.line, d.col, d.message
            ),
            None => write!(f, "{} parse error{plural}", self.error_count),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parse `source` as a script named `"input"` in diagnostics.
///
/// See [`parse_named`], which this calls, for the details.
///
/// ```
/// use parser::{parse, ParseFlags};
///
/// let parsed = parse("let x = 1;", ParseFlags::default()).unwrap();
/// # let _ = parsed;
/// ```
pub fn parse(source: &str, flags: ParseFlags) -> Result<ParsedJS, ParseError> {
    parse_named(source, "input", flags)
}

/// Parse `source`, calling it `file_name` in diagnostics.
///
/// The whole source is parsed eagerly (`ParserPass::FullParse`) as a Program,
/// in the dialect `flags` selects. Returns [`ParseError`] if the parser
/// reported any error — that is the same success condition the `ast-dump` bin
/// applies: a `Program` was produced *and* the error count is zero.
///
/// Diagnostics are recorded in memory rather than printed; nothing is written
/// to stderr.
///
/// ```
/// use parser::{parse_named, ParseFlags};
///
/// let err = parse_named("1 +", "bad.js", ParseFlags::default())
///     .expect_err("should not parse");
/// assert_eq!(err.error_count(), 1);
/// assert!(err.to_string().contains("bad.js"), "{err}");
/// ```
pub fn parse_named(
    source: &str,
    file_name: &str,
    flags: ParseFlags,
) -> Result<ParsedJS, ParseError> {
    let mut sm = SourceErrorManager::new();
    // Record diagnostics instead of printing them: a library must not write to
    // stderr behind its caller's back.
    sm.set_handler(Box::new(CollectingHandler::new()));
    let buf_id = sm.add_buffer(file_name, source);

    let mut ctx = Context::new();
    flags.apply(&mut ctx);

    let mut parsed = ParsedJS {
        program: None,
        ctx,
        sm,
    };

    {
        // The `GCLock` borrows `parsed.ctx`; the lexer borrows `parsed.sm`.
        // Disjoint fields, so both borrows coexist.
        let gc = parsed.ctx.lock();
        let lexer = JSLexer::new(
            buf_id,
            &mut parsed.sm,
            &gc.ctx().atom_table,
            GrammarContext::AllowRegExp,
        );
        let mut parser = JSParserImpl::new(&gc, lexer);
        if let Some(program) = parser.parse() {
            parsed.program = Some(NodeRc::from_node(&gc, program));
        }
    }

    // `JSParserImpl::parse` already returns `None` whenever an error was
    // reported; the error-count check mirrors what every in-tree driver does.
    let error_count = parsed.sm.error_count();
    if parsed.program.is_some() && error_count == 0 {
        Ok(parsed)
    } else {
        Err(ParseError {
            diagnostics: collected(&parsed.sm).to_vec(),
            error_count,
        })
    }
}

/// The diagnostics the [`CollectingHandler`] installed by [`parse_named`] has
/// accumulated in `sm`.
fn collected(sm: &SourceErrorManager) -> &[ResolvedDiagnostic] {
    sm.handler_as::<CollectingHandler>()
        .expect("collecting handler was replaced")
        .messages()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ast::node::NodeKind;

    #[test]
    fn parses_and_reports_program() {
        let mut parsed = parse("1 + 2;", ParseFlags::default()).unwrap();
        let (kind, len) = parsed.with_program(|_gc, program| match program {
            Node::Program(p) => (program.kind(), p.body.iter().count()),
            _ => panic!("root is not a Program"),
        });
        assert_eq!(kind, NodeKind::Program);
        assert_eq!(len, 1);
        assert!(parsed.diagnostics().is_empty());
    }

    #[test]
    fn dumps_estree_json() {
        let mut parsed = parse("0x10;", ParseFlags::default()).unwrap();
        let json = parsed.to_estree_json(false);
        assert!(json.starts_with(r#"{"type":"Program""#), "{json}");
        // The source manager reached the dumper, so `raw` resolved.
        assert!(json.contains(r#""raw":"0x10""#), "{json}");
        // Locations are off by default.
        assert!(!json.contains("\"loc\""), "{json}");
        let with_loc = parsed.to_estree_json_with(
            false,
            ESTreeDumpMode::HideEmpty,
            LocationDumpMode::LocAndRange,
            ESTreeRawProp::Exclude,
        );
        assert!(with_loc.contains("\"loc\""), "{with_loc}");
        assert!(!with_loc.contains("\"raw\""), "{with_loc}");
    }

    #[test]
    fn reports_errors_without_printing() {
        let err = parse("1 +", ParseFlags::default()).unwrap_err();
        assert_eq!(err.error_count(), 1);
        assert_eq!(err.diagnostics().len() as u32, err.error_count());
        assert_eq!(err.diagnostics()[0].kind, DiagKind::Error);
        // The re-export at the crate root names the same type.
        let _: &[crate::ResolvedDiagnostic] = err.diagnostics();
        // `Display` is a one-line summary; the full rendering is `messages`.
        let shown = err.to_string();
        assert!(!shown.contains('\n'), "{shown}");
        let want = "1 parse error; first at input:1:";
        assert!(shown.starts_with(want), "{shown}");
        assert_eq!(err.messages().len(), 1);
        assert!(err.messages()[0].contains('\n'), "{:?}", err.messages()[0]);
    }

    #[test]
    fn flow_needs_its_flag() {
        let src = "type T = number;";
        assert!(parse(src, ParseFlags::default()).is_err());
        let flags = ParseFlags {
            parse_flow: true,
            ..Default::default()
        };
        assert!(parse(src, flags).is_ok());
    }

    #[test]
    fn typescript_and_jsx() {
        let ts = ParseFlags {
            parse_ts: true,
            ..Default::default()
        };
        assert!(parse("let x: number = 1;", ts).is_ok());
        let jsx = ParseFlags {
            parse_jsx: true,
            ..Default::default()
        };
        assert!(parse("<a b={c} />;", jsx).is_ok());
    }

    #[test]
    fn strict_mode_flag_is_honored() {
        // A legacy octal literal: legal in sloppy mode, an error in strict.
        let src = "01;";
        assert!(parse(src, ParseFlags::default()).is_ok());
        let strict = ParseFlags {
            strict_mode: true,
            ..Default::default()
        };
        assert!(parse(src, strict).is_err());
    }
}
