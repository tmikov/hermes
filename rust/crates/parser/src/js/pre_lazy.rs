/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The Pre/Lazy parser passes. Port of the `ParserPass` machinery in
//! `lib/Parser/JSParserImpl.{h,cpp}` and `include/hermes/Parser/JSParser.h`.

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
}
