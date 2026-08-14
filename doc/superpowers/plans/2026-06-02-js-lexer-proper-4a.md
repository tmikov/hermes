# JS Lexer Proper — Phase 4a: lexer state (storage, magic comments, SavePoint, directive, rescanRBrace)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Wire up the self-contained lexer-state surface of `JSLexer`: comment + token storage, magic comments (`//# sourceURL=`/`sourceMappingURL=`), `SavePoint` (save/restore for backtracking), `seek`/`forceEOF`, `isCurrentTokenADirective`, and `rescanRBraceInTemplateLiteral` (which enables `template_middle`/`template_tail`). Validated by Rust unit tests (these are stateful APIs the harness can't drive from a plain `advance` loop).

**Architecture:** Extend `parser::lexer` (`mod.rs` + a `state.rs` child module). The `JSLexer` struct already carries the fields (`store_comments`/`comment_storage`/`store_tokens`/`token_storage`/`source_url`/`source_mapping_url`, currently `#[allow(dead_code)]`). Wire `StoredComment` emplacement into `scan_line_comment`/`skip_block_comment`, magic-comment parsing into `scan_line_comment`, `store_current_token` into `finish_token`, and add the public state API + `SavePoint`.

**Tech Stack:** Rust 2021; `unsafe` only in `cursor.rs`. Uses `support::SourceErrorManager` (suppress-messages + `set_source_url`/`set_source_mapping_url`), `StoredComment`/`StoredToken` (token.rs).
**Reference spec:** `doc/superpowers/specs/2026-06-01-js-lexer-design.md`.
**C++ source of truth:** `include/hermes/Parser/JSLexer.h` — `SavePoint` (the class), `unsafeSetPunctuator`/`unsafeSetIdentifier`/`unsafeSetReservedWord`, `finishToken`/`storeCurrentToken`, `setStoreComments`/`setStoreTokens`/`getStoreTokens`/`getStoredComments`/`moveStoredComments`/`getStoredTokens`, `getSourceURL`/`getSourceMappingURL`, `seek`/`forceEOF`. `lib/Parser/JSLexer.cpp:1482–1510` (`scanLineComment` — comment storage + magic comments), `:1564–1570` (`skipBlockComment` storage), `:911–1021` (`isCurrentTokenADirective`), `:1023–1035` (`rescanRBraceInTemplateLiteral`).

**Porting rule:** faithful port; copy comments. The C++ RAII `make_scope_exit` for comment-storage cleanup and `SaveAndSuppressMessages` become explicit save/restore (the support crate exposes message-suppression as set/restore methods — find and use them).

**Do NOT** `cd` out of the project root.

---

## Task 0: comment + token storage

**Files:** `rust/crates/parser/src/lexer/mod.rs`, `lexer/dump.rs`/wherever `scan_line_comment`/`skip_block_comment`/`finish_token` live.

- [ ] **Step 1: failing tests:**

```rust
#[test]
fn token_storage() {
    // with store_tokens on, every advanced token (kind+range) is recorded.
    let mut sm = SourceErrorManager::new(); let id = sm.add_buffer("t", "a + b");
    let tab = AtomTable::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
    lex.set_store_tokens(true);
    while lex.advance(GrammarContext::AllowDiv).kind() != TokenKind::eof {}
    let toks: Vec<TokenKind> = lex.get_stored_tokens().iter().map(|t| t.kind()).collect();
    assert_eq!(toks, vec![TokenKind::identifier, TokenKind::plus, TokenKind::identifier]); // eof not stored until advanced-to? match C++
}

#[test]
fn comment_storage() {
    let mut sm = SourceErrorManager::new(); let id = sm.add_buffer("t", "a /*c*/ // line\nb");
    let tab = AtomTable::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
    lex.set_store_comments(true);
    while lex.advance(GrammarContext::AllowDiv).kind() != TokenKind::eof {}
    let cs = lex.get_stored_comments();
    assert_eq!(cs.len(), 2);
    // Block and Line kinds; getString() strips delimiters.
    assert_eq!(cs[0].get_string(lex_buffer_bytes), b"c"); // adjust to the real StoredComment API
}
```
(Adjust to the real `StoredComment`/`StoredToken` API — read `token.rs`. `store_current_token`
records `(kind, range)` in `finish_token` when `store_tokens`. The C++ stores the token that was
JUST FINISHED, i.e. the previous token — match `finishToken` exactly.)

- [ ] **Step 2:** FAIL. **Step 3: implement**: `set_store_comments`/`set_store_tokens`/`get_store_tokens`,
  `store_current_token` (called from `finish_token` when `store_tokens`), `get_stored_comments`/
  `move_stored_comments`/`get_stored_tokens`. Emplace a `StoredComment` in `scan_line_comment`
  (Line or Hashbang kind) and `skip_block_comment` (Block) when `store_comments` (port the C++
  `commentStorage_.emplace_back` with the comment range). Match `finishToken` (`JSLexer.h`): it
  records the token being finished.
- [ ] **Step 4:** PASS. **Step 5:** commit `rust(parser): comment + token storage`.

---

## Task 1: magic comments (`//# sourceURL=` / `sourceMappingURL=`)

**Files:** `rust/crates/parser/src/lexer/mod.rs`.

- [ ] **Step 1: failing test:**

```rust
#[test]
fn magic_comments() {
    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer("t", "a\n//# sourceURL=http://x/y.js\n//# sourceMappingURL=z.map\nb");
    let tab = AtomTable::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
    while lex.advance(GrammarContext::AllowDiv).kind() != TokenKind::eof {}
    assert_eq!(lex.get_source_url(), Some("http://x/y.js"));
    assert_eq!(lex.get_source_mapping_url(), Some("z.map"));
}
```

- [ ] **Step 2:** FAIL. **Step 3: implement** in `scan_line_comment` (port `JSLexer.cpp:1494–1509`):
  after a line comment, if it starts with `//# ` then `sourceURL=`→`source_url`, `sourceMappingURL=`
  →`source_mapping_url` (the value is the rest of the comment), and also call
  `sm.set_source_url(buf_id, value)`/`set_source_mapping_url(...)`. Add `get_source_url`/
  `get_source_mapping_url` accessors. (Hashbang `#!` is excluded — only `//# `.)
- [ ] **Step 4:** PASS. **Step 5:** commit `rust(parser): magic comments (sourceURL / sourceMappingURL)`.

---

## Task 2: `SavePoint` + `seek`/`forceEOF`

**Files:** `rust/crates/parser/src/lexer/state.rs` (new; `mod state;`).

- [ ] **Step 1: failing test:**

```rust
#[test]
fn save_point_restore() {
    let mut sm = SourceErrorManager::new(); let id = sm.add_buffer("t", "a . b");
    let tab = AtomTable::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
    lex.advance(GrammarContext::AllowDiv); // 'a' (identifier)
    let sp = lex.save_point();
    lex.advance(GrammarContext::AllowDiv); // '.'
    lex.advance(GrammarContext::AllowDiv); // 'b'
    sp.restore(&mut lex);
    // current token is back to 'a'; next advance gives '.'
    assert_eq!(lex.token().kind(), TokenKind::identifier);
    assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), TokenKind::period);
}
```

- [ ] **Step 2:** FAIL. **Step 3: implement** (port `JSLexer.h` `SavePoint`): `unsafe_set_punctuator`/
  `unsafe_set_identifier`/`unsafe_set_reserved_word` (set token kind/range + `seek(loc)`), a
  `SavePoint` struct snapshotting `kind`/`ident`/`loc`(curLoc)/`range`/`prev_token_end`/
  `comment_storage_len`/`token_storage_len`, and `restore(&mut JSLexer)` that re-sets the token,
  restores `prev_token_end`, and truncates comment/token storage to the saved lengths. (C++ `SavePoint`
  asserts the current token is a punctuator/identifier/`rw_extends` — keep a `debug_assert`.) Add
  lexer-level `seek(SMLoc)` and `force_eof()` (`cursor.seek_end()`). Since Rust can't hold a `&mut`
  borrow across `advance`, model `SavePoint` as a plain value + `restore(&mut lexer)` (not an RAII
  guard) — note the deviation.
- [ ] **Step 4:** PASS. **Step 5:** commit `rust(parser): SavePoint + seek/forceEOF`.

---

## Task 3: `isCurrentTokenADirective` + `rescanRBraceInTemplateLiteral`

**Files:** `rust/crates/parser/src/lexer/state.rs`.

- [ ] **Step 1: failing tests:**

```rust
#[test]
fn is_directive() {
    fn directive(src: &str) -> bool {
        let mut sm = SourceErrorManager::new(); let id = sm.add_buffer("t", src);
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        lex.advance(GrammarContext::AllowDiv); // the string literal
        lex.is_current_token_a_directive()
    }
    assert!(directive("\"use strict\";"));
    assert!(directive("\"use strict\"\n"));
    assert!(directive("\"x\" /*c*/ ;"));
    assert!(directive("\"x\""));            // eof
    assert!(!directive("\"x\" + y"));        // followed by an operator
    assert!(!directive("foo"));              // not a string literal
}

#[test]
fn rescan_rbrace_template() {
    use TokenKind::*;
    // `a${b}c` : template_head, identifier(b), r_brace, then rescan -> template_tail cooked="c"
    let mut sm = SourceErrorManager::new(); let id = sm.add_buffer("t", "`a${b}c`");
    let tab = AtomTable::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
    assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), template_head);
    assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), identifier);
    assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), r_brace);
    assert_eq!(lex.rescan_rbrace_in_template_literal().kind(), template_tail);
}
```

- [ ] **Step 2:** FAIL. **Step 3: implement** `is_current_token_a_directive` (port `:911–1021`): if the
  current token isn't a `string_literal` → false; else scan forward from the current cursor offset
  (using a local offset, NOT moving the real cursor for the non-comment cases) through whitespace /
  unicode space / BOM, returning true at `;`/`}`/newline/EOF/line-comment, consuming block comments
  (suppress messages + save/restore comment storage around `skip_block_comment`), and false otherwise.
  And `rescan_rbrace_in_template_literal` (port `:1023–1035`): assert current is `r_brace`, back the
  cursor up one (to the `}`), pop the stored token if `store_tokens`, set token start, `scan_template_literal`
  (the `}`-start path → `template_middle`/`template_tail`), `finish_token`.
- [ ] **Step 4:** PASS. **Step 5:** commit `rust(parser): isCurrentTokenADirective + rescanRBraceInTemplateLiteral`.

---

## Self-review checklist

- [ ] Storage: `store_current_token` matches `finishToken`; comment emplacement matches the C++; the
  getters/movers behave; `store_comments`/`store_tokens` flags gate correctly.
- [ ] Magic comments parse `//# sourceURL=`/`sourceMappingURL=` and set both the lexer fields and the
  manager URLs; `get_source_url`/`get_source_mapping_url` return them.
- [ ] `SavePoint` round-trips token + cursor + storage lengths; `seek`/`force_eof` work.
- [ ] `isCurrentTokenADirective` matches `:911–1021` (the whitespace/comment/terminator cases); it does
  not corrupt lexer state for the caller (block-comment scan saved/restored; messages suppressed).
- [ ] `rescanRBraceInTemplateLiteral` produces `template_middle`/`template_tail` (the `}`-start template
  path), validated by a unit test.
- [ ] `unsafe` only in `cursor.rs`; zero warnings; all tests pass.

## Next
Phase 4b: parser-lookahead helpers — `lookahead1`/`lookahead2` (save/advance/restore + suppress),
`isLetFollowedByDeclStart`, `isUsing/AwaitUsingFollowedByIdentifier` (pass the needed atoms, not the
parser `Keywords`). Then 4c `convertSurrogates`. See the roadmap.
```
