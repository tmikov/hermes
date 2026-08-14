# JS Lexer Proper — Phase 4b: parser-lookahead helpers

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Port the parser-facing lookahead helpers — `optimisticSkipWhitespace`, `lookahead1`, `lookahead2`, `isLetFollowedByDeclStart`, `isUsingFollowedByIdentifier`, `isAwaitUsingFollowedByIdentifier`. Unit-tested (parser-driven; no differential).

**Architecture:** Add these to `parser::lexer/state.rs` (or a `lookahead.rs` child module). The C++ `template <bool RequireNoNewLine>` becomes a runtime `require_no_newline: bool` param; the parser `Keywords` dependency is replaced by passing the needed pre-interned atom(s) (`ident_using`). The C++ `make_scope_exit` restore + `SaveAndSuppressMessages` become explicit save/restore.

**Tech Stack:** Rust 2021; `unsafe` only in `cursor.rs`. Uses the 4a `seek`/storage/suppress machinery and `unsafe_set_*` token setters.
**Reference spec:** `doc/superpowers/specs/2026-06-01-js-lexer-design.md`.
**C++ source of truth:** `lib/Parser/JSLexer.cpp:117–132` (`optimisticSkipWhitespace`), `:1038–1095` (`lookahead1`), `:1100–1154` (`lookahead2`), `:134–176` (`isLetFollowedByDeclStart`), `:178–204` (`isUsingFollowedByIdentifier`), `:206–253` (`isAwaitUsingFollowedByIdentifier`). The `Token` accessors `getResWordOrIdentifier`/`isResWord`; `isNewLineBeforeCurrentToken`.

**Porting rule:** faithful port; copy comments. `lookahead1`/`lookahead2` must restore the token (kind/ident/range), the cursor (`seek`), and the comment/token storage exactly as the C++ `make_scope_exit` does; they suppress messages during the lookahead; they do **not** restore `prev_token_end` (the C++ doesn't — match it). Keep the `OptValue<TokenKind>` → `Option<TokenKind>` mapping.

**Do NOT** `cd` out of the project root.

---

## Task 0: `optimistic_skip_whitespace` + `lookahead1`

**Files:** `rust/crates/parser/src/lexer/state.rs` (or new `lexer/lookahead.rs`; `mod lookahead;`).

- [ ] **Step 1: failing tests:**

```rust
#[test]
fn lookahead1_basic() {
    // current token must be identifier/resword/question. lookahead1 peeks the next token
    // and restores state unless it matches `expected`.
    let mut sm = SourceErrorManager::new(); let id = sm.add_buffer("t", "async function");
    let tab = AtomTable::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
    lex.advance(GrammarContext::AllowDiv); // 'async' (identifier)
    // peek: next is 'function' (rw_function), no newline.
    assert_eq!(lex.lookahead1(true, None), Some(TokenKind::rw_function));
    // state restored: current token still 'async', next advance is 'function'
    assert_eq!(lex.token().kind(), TokenKind::identifier);
    assert_eq!(lex.advance(GrammarContext::AllowDiv).kind(), TokenKind::rw_function);
}

#[test]
fn lookahead1_newline_and_expected() {
    let mut sm = SourceErrorManager::new(); let id = sm.add_buffer("t", "async\nx");
    let tab = AtomTable::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
    lex.advance(GrammarContext::AllowDiv); // 'async'
    // RequireNoNewLine=true and there IS a newline -> None
    assert_eq!(lex.lookahead1(true, None), None);
    // when expectedToken matches, the cursor is NOT moved back (consumes the lookahead):
    let mut sm2 = SourceErrorManager::new(); let id2 = sm2.add_buffer("t2", "a b");
    let tab2 = AtomTable::new();
    let mut lex2 = JSLexer::new(id2, &mut sm2, &tab2, GrammarContext::AllowDiv);
    lex2.advance(GrammarContext::AllowDiv); // 'a'
    assert_eq!(lex2.lookahead1(true, Some(TokenKind::identifier)), Some(TokenKind::identifier));
    assert_eq!(lex2.token().kind(), TokenKind::identifier); // now 'b' (consumed)
}
```

- [ ] **Step 2:** FAIL. **Step 3: implement** `optimistic_skip_whitespace` (port `:117–132`: skip
  ` `/`\t`/`\v`/`\f` from the cursor, return the next char; does NOT skip newlines or comments) and
  `lookahead1(require_no_newline: bool, expected: Option<TokenKind>) -> Option<TokenKind>` (port
  `:1038–1095`): assert current is identifier/resword/question; save ident/kind/start/end/cursor;
  suppress messages; save comment-storage len; `advance()`; read kind; if `require_no_newline && new_line`
  → kind=None; else if `expected == Some(kind)` → return kind (do NOT restore); restore the token
  (setStart/setEnd + setIdentifier/setPunctuator(question)/setResWord), `seek(cur)`, pop the stored
  token if store_tokens, truncate comment storage; restore suppression; return kind.
- [ ] **Step 4:** PASS. **Step 5:** commit `rust(parser): optimisticSkipWhitespace + lookahead1`.

---

## Task 1: `lookahead2`

**Files:** same module.

- [ ] **Step 1: failing test:**

```rust
#[test]
fn lookahead2_basic() {
    // lookahead2(expected_ident): skip the next token IF it's `expected_ident`, return the kind
    // of the token after it. Always restores state.
    let mut sm = SourceErrorManager::new(); let id = sm.add_buffer("t", "await using x");
    let tab = AtomTable::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
    lex.advance(GrammarContext::AllowDiv); // 'await'
    let using = tab.atom_bytes(b"using");
    // next is 'using' (matches), the one after is 'x' (identifier).
    assert_eq!(lex.lookahead2(true, using), Some(TokenKind::identifier));
    // state restored to 'await'
    assert_eq!(lex.token().kind(), TokenKind::identifier);
    assert_eq!(lex.token().get_res_word_or_identifier(), tab.atom_bytes(b"await"));
}
```

- [ ] **Step 2:** FAIL. **Step 3: implement** `lookahead2(require_no_newline: bool, expected_ident: AtomBytes)
  -> Option<TokenKind>` (port `:1100–1154`): assert current is identifier/resword; save ident/kind/
  start/end/cursor + comment/token storage lens (the C++ uses a single scope_exit that ALWAYS restores;
  in Rust, compute the result then unconditionally restore at the end). suppress. `advance()`; if
  `require_no_newline && new_line` → result None; else if next isn't `identifier` or its ident !=
  `expected_ident` → None; else `advance()` again; if `require_no_newline && new_line` → None; else
  `Some(token.kind())`. Then ALWAYS restore (token, seek(cur), truncate comment+token storage),
  restore suppression. Return the result.
- [ ] **Step 4:** PASS. **Step 5:** commit `rust(parser): lookahead2`.

---

## Task 2: `is_let_followed_by_decl_start` + `is_using_*`

**Files:** same module.

- [ ] **Step 1: failing tests:**

```rust
#[test]
fn let_decl_start() {
    fn islet(src: &str) -> bool {
        let mut sm = SourceErrorManager::new(); let id = sm.add_buffer("t", src);
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        lex.advance(GrammarContext::AllowDiv); // 'let'
        lex.is_let_followed_by_decl_start()
    }
    assert!(islet("let x"));
    assert!(islet("let {a}"));
    assert!(islet("let [a]"));
    assert!(islet("let\nx"));        // no ASI: still a declaration
    assert!(!islet("let in"));        // 'let in ...' is not a decl ('in' operator)
    assert!(!islet("let = 3"));       // 'let' as identifier
}

#[test]
fn using_decls() {
    fn isusing(src: &str) -> bool {
        let mut sm = SourceErrorManager::new(); let id = sm.add_buffer("t", src);
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        lex.advance(GrammarContext::AllowDiv); // 'using'
        lex.is_using_followed_by_identifier()
    }
    assert!(isusing("using x"));
    assert!(!isusing("using\nx"));    // newline -> not a using decl
    assert!(!isusing("using = 1"));   // 'using' as identifier

    fn isawait(src: &str) -> bool {
        let mut sm = SourceErrorManager::new(); let id = sm.add_buffer("t", src);
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        lex.advance(GrammarContext::AllowDiv); // 'await'
        let using = tab.atom_bytes(b"using");
        lex.is_await_using_followed_by_identifier(using)
    }
    assert!(isawait("await using x"));
    assert!(!isawait("await using\nx"));
    assert!(!isawait("await x"));
}
```

- [ ] **Step 2:** FAIL. **Step 3: implement** (port `:134–253`):
  - `is_let_followed_by_decl_start()` (`:134–176`): `optimistic_skip_whitespace`; if `{`/`[` → true;
    if `is_ascii_identifier_start(c)` and not (`c=='i' && peek_at(1)=='n'`) → true; else
    `lookahead1(false, None)` → kind in {identifier, l_brace, l_square}.
  - `is_using_followed_by_identifier()` (`:178–204`): save cursor, `optimistic_skip_whitespace`,
    restore cursor; if `\r`/`\n` → false; if `is_ascii_identifier_start(c)` → true; else
    `lookahead1(true, None) == Some(identifier)`. (The C++ asserts the token is `kw.identUsing` — keep
    a `debug_assert` that the current identifier bytes are `using`, or drop the assert.)
  - `is_await_using_followed_by_identifier(ident_using: AtomBytes)` (`:206–253`): save cursor;
    `optimistic_skip_whitespace`; if `\r`/`\n` → restore + false; fast path: if the next bytes are
    `using` (5 chars) not followed by an identifier-continue char, skip them, skip whitespace, restore
    cursor, and if no newline and `is_ascii_identifier_start` → true; slow path: restore cursor,
    `lookahead2(true, ident_using) == Some(identifier)`.
- [ ] **Step 4:** PASS. **Step 5:** commit `rust(parser): isLetFollowedByDeclStart + isUsing/isAwaitUsing`.

---

## Self-review checklist

- [ ] `lookahead1`/`lookahead2` restore token + cursor + comment/token storage; suppress messages; do
  NOT restore `prev_token_end` (faithful); the `expected`-match early-return in `lookahead1` does not
  restore (consumes the lookahead).
- [ ] `optimistic_skip_whitespace` skips only ` `/`\t`/`\v`/`\f` (not newlines/comments).
- [ ] `isLet`/`isUsing`/`isAwaitUsing` match the C++ fast/slow paths; the `Keywords` dependency is
  replaced by the `ident_using` atom param; the `let in` and newline edge cases are correct.
- [ ] `unsafe` only in `cursor.rs`; zero warnings; all tests pass (and the 5 differentials still pass).

## Next
Phase 4c: `convertSurrogates` re-encoding (`getStringLiteral` when the flag is set) — needs UTF-8↔UTF-16
conversion utils (`convertUTF8WithSurrogatesToUTF16` + `convertUTF16ToUTF8WithReplacements`). This is the
last `JSLexer` piece. See the roadmap.
```
