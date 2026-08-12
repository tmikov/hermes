/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! JSONParser: recursive-descent parser driving JSLexer.
//!
//! Value nesting is depth-limited, matching C++ `JSONParser` (`lib/Parser/
//! JSONParser.cpp:202-212`, `JSONParser.h:636-651`): `parse_value` checks
//! `MAX_RECURSION_DEPTH` and reports "Too many nested JSON values" instead
//! of overflowing the native stack. Historically NEITHER side had a limit
//! (parity by absence, and both died on e.g. 100000 `[`); upstream added one
//! in `b21856de4` ("Add a recursion limit to the compiler-side JSONParser")
//! and this is the mirror of that fix.

use hermes_atom_table::AtomTable;
use hermes_support::diag::Subsystem;
use hermes_support::location::{SMRange, SourceId};
use hermes_support::manager::SourceErrorManager;

use crate::lexer::{GrammarContext, JSLexer};
use crate::token::Token;
use crate::token_kinds::TokenKind;

use super::factory::Prop;
use super::{JSONFactory, JSONValue};

/// JSON grammar uses `/` as division (never regexp).
const CTX: GrammarContext = GrammarContext::AllowDiv;

/// The maximum depth of value nesting, to avoid stack overflow on deeply
/// nested input. Port of `JSONParser::MAX_RECURSION_DEPTH`
/// (JSONParser.h:638-651), whose `#ifdef` ladder is the same one as
/// `JSParserImpl::MAX_RECURSION_DEPTH` ("The values match
/// JSParserImpl::MAX_RECURSION_DEPTH"), so the Rust mapping is the same too:
/// key off `debug_assertions`, which pairs a DEBUG Rust build with the
/// project's standard ASan C++ oracle (both take the 128 branch) and a
/// RELEASE Rust build with C++'s release value. See
/// `crate::js::MAX_RECURSION_DEPTH` for the full ladder and the
/// profile-pairing caveat.
const MAX_RECURSION_DEPTH: u32 = if cfg!(debug_assertions) { 128 } else { 1024 };

/// Port of `JSONParser` (JSONParser.h:630). Drives `JSLexer`; errors go through
/// the lexer's single `&mut SourceErrorManager`.
pub struct JSONParser<'a> {
    factory: &'a JSONFactory<'a>,
    lexer: JSLexer<'a>,
    /// The current depth of value nesting during parsing. Port of
    /// `JSONParser::recursionDepth_` (JSONParser.h:637).
    recursion_depth: u32,
}

impl<'a> JSONParser<'a> {
    /// Construct a `JSONParser` over the source buffer identified by `buf_id` in
    /// `sm`. Port of `JSONParser::JSONParser` (JSONParser.h:660-672).
    pub fn new(
        factory: &'a JSONFactory<'a>,
        buf_id: SourceId,
        sm: &'a mut SourceErrorManager,
        atoms: &'a AtomTable,
        convert_surrogates: bool,
    ) -> JSONParser<'a> {
        let lexer =
            JSLexer::new_with_convert_surrogates(buf_id, sm, atoms, CTX, convert_surrogates);
        JSONParser {
            factory,
            lexer,
            recursion_depth: 0,
        }
    }

    /// Returns the number of errors reported so far (via the shared
    /// `SourceErrorManager`).
    pub fn error_count(&self) -> u32 {
        self.lexer.get_source_mgr().error_count()
    }

    /// Report an error at the current token's range. Port of JSONParser.h:685.
    fn error(&mut self, msg: impl Into<String>) {
        let range: SMRange = self.lexer.token().source_range();
        self.lexer.get_source_mgr_mut().error_at(
            range.start,
            Some(range),
            msg.into(),
            Subsystem::Parser,
        );
    }

    /// Return a reference to the current token (immutable borrow of lexer).
    fn cur(&self) -> &Token {
        self.lexer.token()
    }

    /// Advance the lexer and return the new current token.
    fn advance(&mut self) -> &Token {
        self.lexer.advance(CTX)
    }

    /// Parse the whole JSON input. Port of JSONParser.cpp:192.
    pub fn parse(&mut self) -> Option<&'a JSONValue<'a>> {
        self.advance();
        let res = self.parse_value()?;
        if self.lexer.get_source_mgr().error_count() != 0 {
            return None;
        }
        Some(res)
    }

    /// Check and update the recursion depth, then parse any JSON value. Port of
    /// `JSONParser::parseValue` (JSONParser.cpp:202-212).
    fn parse_value(&mut self) -> Option<&'a JSONValue<'a>> {
        if self.recursion_depth >= MAX_RECURSION_DEPTH {
            self.error("Too many nested JSON values");
            return None;
        }
        self.recursion_depth += 1;
        let res = self.parse_value_impl();
        self.recursion_depth -= 1;
        res
    }

    /// Parse any JSON value, assuming the recursion depth has been checked.
    /// Port of `JSONParser::parseValueImpl` (JSONParser.cpp:213).
    fn parse_value_impl(&mut self) -> Option<&'a JSONValue<'a>> {
        let mut needs_negation = false;
        match self.cur().kind() {
            TokenKind::string_literal => {
                // Read the interned atom before advancing (borrow-checker: avoid
                // holding &Token across the mutable advance() call).
                let lit = self.cur().get_string_literal();
                self.advance();
                Some(self.factory.get_string(lit))
            }
            TokenKind::minus => {
                needs_negation = true;
                self.advance();
                if self.cur().kind() != TokenKind::numeric_literal {
                    self.error("No numeric literal following minus (-) token in value");
                    return None;
                }
                self.parse_number(needs_negation)
            }
            TokenKind::numeric_literal => self.parse_number(needs_negation),
            TokenKind::l_brace => {
                self.advance();
                self.parse_object()
            }
            TokenKind::l_square => {
                self.advance();
                self.parse_array()
            }
            TokenKind::rw_true => {
                self.advance();
                Some(self.factory.get_boolean(true))
            }
            TokenKind::rw_false => {
                self.advance();
                Some(self.factory.get_boolean(false))
            }
            TokenKind::rw_null => {
                self.advance();
                Some(self.factory.get_null())
            }
            _ => {
                self.error("JSON object or array expected");
                None
            }
        }
    }

    /// Parse a numeric literal (with optional leading negation).
    /// Reads the f64 value before advancing to satisfy the borrow checker.
    fn parse_number(&mut self, needs_negation: bool) -> Option<&'a JSONValue<'a>> {
        let v = self.cur().get_numeric_literal();
        let res = self.factory.get_number(if needs_negation { -v } else { v });
        self.advance();
        Some(res)
    }

    /// JSONParser.cpp:260 — parse `[ ... ]` (the `[` already consumed).
    fn parse_array(&mut self) -> Option<&'a JSONValue<'a>> {
        let mut storage: Vec<&'a JSONValue<'a>> = Vec::new();
        if self.cur().kind() != TokenKind::r_square {
            loop {
                let val = self.parse_value()?;
                storage.push(val);
                if self.cur().kind() == TokenKind::comma {
                    self.advance();
                    if self.cur().kind() == TokenKind::r_square {
                        break;
                    }
                } else {
                    break;
                }
            }
            if self.cur().kind() != TokenKind::r_square {
                self.error("expected ']'");
                return None;
            }
        }
        self.advance(); // consume ']'
        Some(self.factory.new_array(&storage))
    }

    /// JSONParser.cpp:289 — parse `{ ... }` (the `{` already consumed).
    fn parse_object(&mut self) -> Option<&'a JSONValue<'a>> {
        let mut pairs: Vec<Prop<'a>> = Vec::new();
        if self.cur().kind() != TokenKind::r_brace {
            loop {
                if self.cur().kind() != TokenKind::string_literal {
                    self.error("expected a string");
                    return None;
                }
                let key = self.factory.get_string(self.cur().get_string_literal());
                if self.advance().kind() != TokenKind::colon {
                    self.error("expected ':'");
                    return None;
                }
                self.advance();
                let val = self.parse_value()?;
                pairs.push((key, val));
                if self.cur().kind() == TokenKind::comma {
                    self.advance();
                    if self.cur().kind() == TokenKind::r_brace {
                        break;
                    }
                } else {
                    break;
                }
            }
            if self.cur().kind() != TokenKind::r_brace {
                self.error("expected '}'");
                return None;
            }
        }
        self.advance(); // consume '}'

        if let Some(dup) = self.factory.sort_props(&mut pairs) {
            let name = String::from_utf8_lossy(self.factory.atoms().bytes(dup)).into_owned();
            self.error(format!("key '{name}' is already present"));
            return None;
        }
        // Already sorted + dup-checked: build directly.
        self.factory.new_object_sorted(&pairs)
    }
}

#[cfg(test)]
mod parser_tests {
    use super::super::*;
    use bumpalo::Bump;
    use hermes_atom_table::AtomTable;
    use hermes_support::manager::SourceErrorManager;

    /// Helper: parse `src` and return the JSON value (if successful).
    /// `sm` must outlive the call so the returned `&'a JSONValue<'a>` is valid.
    fn parse_ok<'a>(
        arena: &'a Bump,
        atoms: &'a AtomTable,
        sm: &'a mut SourceErrorManager,
        src: &str,
    ) -> Option<&'a JSONValue<'a>> {
        // Mirrors `JSONParser parser(factory, src, sm); parser.parse()`.
        let f = arena.alloc(JSONFactory::new(arena, atoms));
        let id = sm.add_buffer("json", src);
        let mut p = JSONParser::new(f, id, sm, atoms, false);
        p.parse()
    }

    #[test]
    fn scalars() {
        let arena = Bump::new();
        let atoms = AtomTable::new();
        let mut sm = SourceErrorManager::new();
        assert_eq!(
            parse_ok(&arena, &atoms, &mut sm, "true").and_then(|v| v.as_boolean()),
            Some(true)
        );
        assert_eq!(
            parse_ok(&arena, &atoms, &mut sm, "false").and_then(|v| v.as_boolean()),
            Some(false)
        );
        assert_eq!(
            parse_ok(&arena, &atoms, &mut sm, "null").map(|v| v.kind()),
            Some(JSONKind::Null)
        );
        assert_eq!(
            parse_ok(&arena, &atoms, &mut sm, "42").and_then(|v| v.as_number()),
            Some(42.0)
        );
        assert_eq!(
            parse_ok(&arena, &atoms, &mut sm, "-1.5").and_then(|v| v.as_number()),
            Some(-1.5)
        );
        let s = parse_ok(&arena, &atoms, &mut sm, "'hi'")
            .unwrap()
            .as_string()
            .unwrap();
        assert_eq!(atoms.bytes(s), b"hi");
    }

    /// Parse `src` in a self-contained scope and run `f` on the result + atom
    /// table while everything is still alive. Returns whatever `f` returns.
    fn with_parse<R>(src: &str, f: impl FnOnce(Option<&JSONValue<'_>>, &AtomTable) -> R) -> R {
        let arena = Bump::new();
        let atoms = AtomTable::new();
        let factory = arena.alloc(JSONFactory::new(&arena, &atoms));
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("json", src);
        let mut parser = JSONParser::new(factory, id, &mut sm, &atoms, false);
        let result = parser.parse();
        f(result, &atoms)
    }

    #[test]
    fn arrays() {
        with_parse("[-1.0, -1, -0]", |r, _| {
            let v = r.unwrap().as_array().unwrap();
            assert_eq!(v.len(), 3);
            assert_eq!(v.at(0).as_number(), Some(-1.0));
            assert_eq!(v.at(2).as_number(), Some(-0.0));
        });
        with_parse("[]", |r, _| assert!(r.unwrap().as_array().unwrap().is_empty()));
        // trailing comma is accepted (mirror C++: after a comma, ']' breaks the loop).
        with_parse("[1,2,3,]", |r, _| assert!(r.is_some()));
        // unterminated -> failure.
        with_parse("[1,2", |r, _| assert!(r.is_none()));
    }

    #[test]
    fn lone_minus_errors() {
        // NegativeNumbers: "-" -> failure, error count 1.
        let arena = Bump::new();
        let atoms = AtomTable::new();
        let f = arena.alloc(JSONFactory::new(&arena, &atoms));
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("json", "-");
        let mut p = JSONParser::new(f, id, &mut sm, &atoms, false);
        assert!(p.parse().is_none());
        assert_eq!(p.error_count(), 1);
    }
}
