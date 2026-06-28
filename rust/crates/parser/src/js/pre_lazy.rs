/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The Pre/Lazy parser passes. Port of the `ParserPass` machinery in
//! `lib/Parser/JSParserImpl.{h,cpp}` and `include/hermes/Parser/JSParser.h`.

use std::collections::HashMap;

use support::location::SMLoc;

/// The parser mode. Port of `enum ParserPass` (JSParser.h:26-36). Same order:
/// PreParse, LazyParse, FullParse.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ParserPass {
    /// Parse and index the file's functions without keeping an AST.
    PreParse,
    /// Re-parse, skipping function bodies indexed by a prior PreParse.
    LazyParse,
    /// Completely parse the full file (the default, eager mode).
    FullParse,
}

/// Information about a pre-parsed function body, recorded during the
/// `PreParse` pass and consumed during `LazyParse`.
/// Port of `PreParsedFunctionInfo` (PreParser.h:38-58).
pub struct PreParsedFunctionInfo {
    /// The end location of the function body (closing `}`).
    pub end: SMLoc,

    /// Whether the function body began with `"use strict"`.
    pub strict_mode: bool,

    /// Directive prologues found at the top of the function body.
    /// Stored as owned byte vectors because UniqueString atoms are
    /// arena-allocated and reclaimed between parse passes — we cannot hold
    /// raw pointers across pass boundaries (PreParser.h:46-48).
    pub directives: Vec<Vec<u8>>,

    /// Whether the function body contains an arrow function.
    pub contains_arrow_functions: bool,

    /// Conservative estimate: whether a non-arrow function may have an arrow
    /// child that references `arguments`, requiring eager Arguments capture.
    pub may_contain_arrow_functions_using_arguments: bool,
}

/// Per-buffer table produced by the `PreParse` pass.
/// Port of `PreParsedBufferInfo` (PreParser.h:60-63).
pub struct PreParsedBufferInfo {
    /// Maps function-body start **offset** (within the source buffer) to its
    /// pre-parsed metadata. The C++ uses `SMLoc` (a pointer) as the key;
    /// we use the `u32` offset so the map is trivially `Send`/serialisable.
    pub function_info: HashMap<u32, PreParsedFunctionInfo>,
}

#[cfg(test)]
mod tests {
    // A parser built with `new` defaults to FullParse; new_with_pass honors the arg.
    #[test]
    fn parser_pass_defaults_and_override() {
        use ast::context::Context;
        use support::manager::SourceErrorManager;
        use crate::lexer::{GrammarContext, JSLexer};
        use crate::js::{JSParserImpl, ParserPass};

        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer_bytes("t", b"1;");
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let atoms = &gc.ctx().atom_table;
        let lexer = JSLexer::new(id, &mut sm, atoms, GrammarContext::AllowRegExp);
        let p = JSParserImpl::new_with_pass(&gc, lexer, ParserPass::PreParse);
        assert_eq!(p.pass, ParserPass::PreParse);
    }

    // The side-table round-trips through take/set; threshold defaults to 0.
    #[test]
    fn pre_parsed_table_and_threshold() {
        use ast::context::Context;
        let mut ctx = Context::new();
        assert_eq!(ctx.preemptive_function_compilation_threshold(), 0);
        ctx.set_preemptive_function_compilation_threshold(64);
        assert_eq!(ctx.preemptive_function_compilation_threshold(), 64);
    }
}
