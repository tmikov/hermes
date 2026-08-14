# JS Lexer Proper — Phase 1a: skeleton + punctuators/trivia + live differential

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up the real `JSLexer` skeleton in Rust — UTF-8 decode, the encapsulated `*const u8` cursor, `Token`, the `JSLexer` struct + construction + error helpers — and implement `advance()` for **punctuators, whitespace, line/block comments, and EOF**, validated **byte-for-byte live against `cmake-build-asan/bin/js-lexer-dump`**. Identifiers, numbers, and literals (strings/templates/regexp/private-id/bigint) are deferred to phases 1b+.

**Architecture:** New modules in the existing `parser` crate: `utf8` (decode side of `Support/UTF8.h`), `cursor` (the one place with scoped `unsafe` — a `*const u8` triple over an `Rc<SourceBuffer>`, offset at every boundary), `token` (`Token` with offset `SMRange` + `AtomBytes` handles; `RegExpLiteral`/`StoredComment`/`StoredToken`), and `lexer` (`JSLexer`). The lexer borrows `&mut SourceErrorManager` (for diagnostics) and `&AtomTable` (interior-mutable interner) and clones the buffer as `Rc<SourceBuffer>`. A `dump_token` helper formats a token exactly like the C++ `js-lexer-dump`, and a differential test shells out to that binary and compares.

**Tech Stack:** Rust 2021. Crate `parser` is `unsafe_code = "deny"`; ONLY `cursor.rs` gets a scoped `#[allow(unsafe_code)]` (documented). Depends on `support` (SMLoc/SMRange/SourceBuffer/SourceErrorManager), `atom_table` (AtomTable/AtomBytes), and (later phases) `unicode`/`number`.

**Reference spec:** `doc/superpowers/specs/2026-06-01-js-lexer-design.md`.
**C++ source of truth (READ THESE):**
- `include/hermes/Parser/JSLexer.h` — `Token`, `JSLexer`, `GrammarContext`, `finishToken`, accessors.
- `lib/Parser/JSLexer.cpp:61–132` (ctors, `initializeWithBufferId`, `optimisticSkipWhitespace`), `:255–745` (`advance` — port the punctuator/whitespace/comment/EOF arms; the identifier/number/string/template/regexp/`#` arms are STUBBED in 1a), `:1430–1571` (`lineCommentHelper`/`scanLineComment`/`skipBlockComment`), `:2497–2522` (`error` helpers).
- `include/hermes/Support/UTF8.h:24–193` — `isUTF8Start`, `isUTF8LeadingByte`, `isUTF8ContinuationByte`, `_decodeUTF8SlowPath`, `decodeUTF8`. Port the **decode** side (encode side is phase 1b).
- `tools/js-lexer-dump/js-lexer-dump.cpp` — the exact dump format to mirror in `dump_token`.

**Porting rule:** keep structure close to the C++, copy comments. Where this plan says "port `JSLexer.cpp:N–M`", read and translate faithfully.

**Dump format (must match `js-lexer-dump` byte-for-byte):** `"<start> <end> <nl> <KIND>[ fields]\n"`, offsets = byte distance from buffer start, `nl`/`--`, KIND = the `.def` variant name (use `token_kind_str`? NO — use the variant name; in Rust the `TokenKind` `Debug`/a name fn gives it — add a `variant_name(TokenKind)->&str` if not present, matching the harness's `#name`). Phase 1a only emits fieldless kinds (punctuators, `eof`).

**Do NOT** `cd` out of the project root.

---

## File structure

```
rust/crates/parser/
  Cargo.toml        # add dependencies: support, atom_table (path deps)
  src/
    lib.rs          # add: pub mod utf8; pub mod cursor; pub mod token; pub mod lexer;
    utf8.rs         # decode side of Support/UTF8.h
    cursor.rs       # encapsulated *const u8 cursor (scoped unsafe)
    token.rs        # Token, RegExpLiteral, StoredComment, StoredToken
    lexer.rs        # JSLexer: struct, new(), advance() (punct/trivia/eof), errors, dump_token
  tests/
    differential.rs # shells out to js-lexer-dump, compares
```

`rust/crates/parser/Cargo.toml` gains:
```toml
[dependencies]
support = { path = "../support" }
atom_table = { path = "../atom_table" }
```
(`unicode` and `number` are already in-crate / added when 1b needs them.)

---

## Task 0: Crate deps + module scaffold

- [ ] Add the `support` + `atom_table` path deps to `rust/crates/parser/Cargo.toml`.
- [ ] In `lib.rs` add `pub mod utf8;`, `pub mod cursor;`, `pub mod token;`, `pub mod lexer;`.
- [ ] Create empty stub files for each (one `//!` line) so it compiles.
- [ ] `cargo build --manifest-path rust/Cargo.toml -p parser` → clean.
- [ ] Commit: `rust(parser): scaffold lexer modules (utf8/cursor/token/lexer)`

---

## Task 1: `utf8` — decode side of Support/UTF8.h

**Files:** `rust/crates/parser/src/utf8.rs`.

- [ ] **Step 1: failing tests:**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classifiers() {
        assert!(!is_utf8_start(b'a'));
        assert!(is_utf8_start(0xc3));
        assert!(is_utf8_leading_byte(0xc3));
        assert!(!is_utf8_leading_byte(0x80));
        assert!(is_utf8_continuation_byte(0x80));
    }
    #[test]
    fn decode_ascii_and_multibyte() {
        // ASCII fast path advances by 1.
        let buf = b"a";
        let mut i = 0usize;
        assert_eq!(decode_utf8::<false>(buf, &mut i, |_| {}), 'a' as u32);
        assert_eq!(i, 1);
        // é = U+00E9 = c3 a9
        let buf = b"\xc3\xa9";
        let mut i = 0usize;
        assert_eq!(decode_utf8::<false>(buf, &mut i, |_| {}), 0x00E9);
        assert_eq!(i, 2);
        // U+1F600 = f0 9f 98 80
        let buf = b"\xf0\x9f\x98\x80";
        let mut i = 0usize;
        assert_eq!(decode_utf8::<false>(buf, &mut i, |_| {}), 0x1F600);
        assert_eq!(i, 4);
    }
    #[test]
    fn line_terminator_match() {
        assert!(match_unicode_line_terminator_offset1(b"\xe2\x80\xa8")); // U+2028
        assert!(match_unicode_line_terminator_offset1(b"\xe2\x80\xa9")); // U+2029
        assert!(!match_unicode_line_terminator_offset1(b"\xe2\x80\xaa"));
    }
    #[test]
    fn invalid_reports_error_and_replacement() {
        let buf = b"\xc3"; // truncated -> needs a following byte; here buf[1] is OOB
        // (Use a NUL-terminated style slice in real lexer; here test the lead+bad cont.)
        let buf = b"\xc3\x20"; // 0x20 is not a continuation byte
        let mut i = 0usize;
        let mut errs = 0;
        let cp = decode_utf8::<false>(buf, &mut i, |_| errs += 1);
        assert_eq!(cp, 0xFFFD);
        assert_eq!(errs, 1);
    }
}
```

- [ ] **Step 2:** run → FAIL.
- [ ] **Step 3: implement** by porting `UTF8.h`. Use index-based signatures (the cursor will
  pass a slice + offset; raw-pointer parity lives only in `cursor.rs`). Port faithfully:

```rust
//! UTF-8 decode helpers, ported from include/hermes/Support/UTF8.h (decode side).

use unicode::{UNICODE_REPLACEMENT_CHARACTER, UNICODE_SURROGATE_FIRST, UNICODE_SURROGATE_LAST,
              UNICODE_MAX_VALUE};

/// First byte of the UTF-8 encoding of U+2028/U+2029 (e2 80 a8/a9).
pub const UTF8_LINE_TERMINATOR_CHAR0: u8 = 0xe2;

#[inline] pub fn is_utf8_start(ch: u8) -> bool { (ch & 0x80) != 0 }
#[inline] pub fn is_utf8_leading_byte(ch: u8) -> bool { (ch & 0xC0) == 0xC0 }
#[inline] pub fn is_utf8_continuation_byte(ch: u8) -> bool { (ch & 0xC0) == 0x80 }

/// \return true if `bytes` starts with the UTF-8 encoding of U+2028 or U+2029.
/// `bytes[0]` is assumed to be UTF8_LINE_TERMINATOR_CHAR0 (the caller checked).
#[inline]
pub fn match_unicode_line_terminator_offset1(bytes: &[u8]) -> bool {
    bytes.len() >= 3 && bytes[0] == 0xe2 && bytes[1] == 0x80 && (bytes[2] == 0xa8 || bytes[2] == 0xa9)
}

/// Decode the UTF-8 sequence in `bytes` starting at `*i` (which must be a UTF-8
/// start byte), advancing `*i` past it; calls `error` and returns the replacement
/// character on malformed input. Port of `_decodeUTF8SlowPath` (UTF8.h).
/// `ALLOW_SURROGATES`: when false, surrogate-range results are errors.
pub fn decode_utf8_slow_path<const ALLOW_SURROGATES: bool>(
    bytes: &[u8],
    i: &mut usize,
    mut error: impl FnMut(&str),
) -> u32 {
    // PORT UTF8.h:77–162 faithfully. Reads bytes[*i], bytes[*i+1], ... The buffer
    // is NUL-terminated, so out-of-range continuation reads see 0x00 (a non-
    // continuation byte) and are correctly rejected — but guard indexes against
    // bytes.len() defensively (return replacement + error if past end).
    // ... transcribe the 2/3/4-byte cases, the non-canonical and surrogate/max
    // checks, advancing *i by 1/2/3/4 and returning UNICODE_REPLACEMENT_CHARACTER
    // on each error path exactly as the C++ does ...
    unimplemented!("port UTF8.h:77-162")
}

/// Decode the UTF-8 sequence at `*i`, ASCII fast path. Port of `decodeUTF8`.
#[inline]
pub fn decode_utf8<const ALLOW_SURROGATES: bool>(
    bytes: &[u8],
    i: &mut usize,
    error: impl FnMut(&str),
) -> u32 {
    if *i < bytes.len() && (bytes[*i] & 0x80) == 0 {
        let c = bytes[*i] as u32;
        *i += 1;
        return c;
    }
    decode_utf8_slow_path::<ALLOW_SURROGATES>(bytes, i, error)
}
```

(Port the slow path body exactly; the test `decode_ascii_and_multibyte` + `invalid_*`
pin it. `parser`'s `Cargo.toml` already has `unicode` available in-workspace — add
`unicode = { path = "../unicode" }` to `[dependencies]`.)

- [ ] **Step 4:** run → PASS. Zero warnings.
- [ ] **Step 5:** commit `rust(parser): port UTF-8 decode helpers (UTF8.h)`.

---

## Task 2: `cursor` — encapsulated raw-pointer cursor

**Files:** `rust/crates/parser/src/cursor.rs`. This is the ONLY module with `unsafe`.

- [ ] **Step 1: failing tests** (offsets, peek with NUL terminator, slicing):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;
    use support::buffer::SourceBuffer;

    fn cur(s: &str) -> Cursor { Cursor::new(Rc::new(SourceBuffer::from_str("t", s))) }

    #[test]
    fn basic() {
        let mut c = cur("ab");
        assert_eq!(c.offset(), 0);
        assert_eq!(c.peek(), b'a');
        assert_eq!(c.peek_at(1), b'b');
        assert_eq!(c.peek_at(2), 0);      // NUL terminator (in-bounds, always present)
        assert!(!c.at_end());
        c.advance(1);
        assert_eq!(c.offset(), 1);
        assert_eq!(c.peek(), b'b');
        c.advance(1);
        assert_eq!(c.peek(), 0);
        assert!(c.at_end());
    }

    #[test]
    fn slicing_and_seek() {
        let mut c = cur("hello");
        c.advance(2);
        assert_eq!(c.slice_from(0), b"he"); // bytes [0, offset)
        c.seek(4);
        assert_eq!(c.offset(), 4);
        assert_eq!(c.peek(), b'o');
    }
}
```

- [ ] **Step 2:** run → FAIL.
- [ ] **Step 3: implement.** Top of file:

```rust
//! The lexer's scan cursor. This is the one place the port uses `unsafe`
//! (decision "B"): a raw `*const u8` cursor over the source buffer for parity
//! with the C++ lexer's pointer arithmetic. The buffer is held as an
//! `Rc<SourceBuffer>` (stable heap address; kept alive for the cursor's life),
//! and every public method converts to/from a byte offset, so nothing `unsafe`
//! escapes this module. The buffer is NUL-terminated, so `peek_at` one past the
//! last real byte reads the terminating 0 (in-bounds).
#![allow(unsafe_code)]

use std::rc::Rc;
use support::buffer::SourceBuffer;

pub struct Cursor {
    buffer: Rc<SourceBuffer>,
    start: *const u8,
    cur: *const u8,
    end: *const u8, // points at the terminating NUL
}

impl Cursor {
    pub fn new(buffer: Rc<SourceBuffer>) -> Cursor {
        // `bytes()` excludes the NUL; `raw()` includes it. We need the NUL to be
        // addressable, so base pointers on the NUL-terminated storage.
        let raw = buffer.raw();            // includes trailing NUL
        let n = buffer.bytes().len();      // logical length (without NUL)
        // SAFETY: raw is a contiguous slice with a trailing NUL at index n.
        let start = raw.as_ptr();
        let end = unsafe { start.add(n) }; // -> the NUL
        Cursor { buffer, start, cur: start, end }
    }

    #[inline] pub fn offset(&self) -> u32 {
        // SAFETY: cur is within [start, end].
        (unsafe { self.cur.offset_from(self.start) }) as u32
    }
    #[inline] pub fn at_end(&self) -> bool { self.cur >= self.end }
    /// Byte at the cursor (or NUL at end).
    #[inline] pub fn peek(&self) -> u8 { unsafe { *self.cur } }
    /// Byte `n` ahead. Only valid while the previous byte was non-NUL (the C++
    /// lookahead invariant); reading the terminating NUL is always in-bounds.
    #[inline] pub fn peek_at(&self, n: usize) -> u8 { unsafe { *self.cur.add(n) } }
    #[inline] pub fn advance(&mut self, n: usize) { unsafe { self.cur = self.cur.add(n); } }
    /// Seek to an absolute byte offset.
    #[inline] pub fn seek(&mut self, offset: u32) {
        unsafe { self.cur = self.start.add(offset as usize); }
    }
    /// Move the cursor to EOF (forceEOF).
    #[inline] pub fn seek_end(&mut self) { self.cur = self.end; }
    /// Bytes in [from_offset, current offset).
    #[inline] pub fn slice_from(&self, from_offset: u32) -> &[u8] {
        &self.buffer.raw()[from_offset as usize..self.offset() as usize]
    }
    /// Bytes in [from_offset, to_offset).
    #[inline] pub fn slice(&self, from_offset: u32, to_offset: u32) -> &[u8] {
        &self.buffer.raw()[from_offset as usize..to_offset as usize]
    }
    pub fn buffer(&self) -> &Rc<SourceBuffer> { &self.buffer }
}
```

Confirm `SourceBuffer` exposes `raw()` (NUL-terminated bytes) and `bytes()` (without NUL) —
they do (`rust/crates/support/src/buffer.rs`). If `offset_from`/`add` need a newer toolchain
API, they are stable on 1.96; otherwise compute with `as usize` pointer arithmetic.

- [ ] **Step 4:** run → PASS. Zero warnings. Confirm `unsafe` appears ONLY in `cursor.rs`.
- [ ] **Step 5:** commit `rust(parser): encapsulated *const u8 scan cursor (decision B)`.

---

## Task 3: `token` — Token and friends

**Files:** `rust/crates/parser/src/token.rs`.

Port `JSLexer.h`'s `Token`, `RegExpLiteral`, `StoredComment`, `StoredToken`, but offset-based
(`SMRange`) with `AtomBytes` handles instead of pointers. Phase 1a only needs kind + range +
punctuator setters; include the value fields/accessors (used in later phases) with faithful
shapes.

- [ ] **Step 1: failing test:**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use parser::token_kinds::TokenKind; // adjust to crate-local path
    use support::location::{SMLoc, SMRange, SourceId};
    #[test]
    fn punctuator_token() {
        let id = SourceId::from_index(1);
        let mut t = Token::new();
        t.set_punctuator(TokenKind::l_brace);
        t.set_range(SMRange { start: SMLoc::new(id, 0), end: SMLoc::new(id, 1) });
        assert_eq!(t.kind(), TokenKind::l_brace);
        assert_eq!(t.start_loc().offset(), 0);
        assert_eq!(t.end_loc().offset(), 1);
    }
}
```
(Adjust `SMLoc`/`SMRange`/`SourceId` constructor calls to the real `support::location` API —
read it first; use whatever accessors exist, e.g. `SMLoc`’s offset accessor.)

- [ ] **Step 2:** run → FAIL.
- [ ] **Step 3: implement** `Token` faithfully (port `JSLexer.h:80–316`): fields
  `kind: TokenKind`, `range: SMRange`, `numeric: f64`, `ident: Option<AtomBytes>`,
  `string_literal: Option<AtomBytes>`, `raw_string: Option<AtomBytes>`,
  `regexp: Option<RegExpLiteral>`, `string_literal_contains_escapes: bool`. Methods:
  `kind`, `is_res_word` (delegate to `TokenKind::is_res_word`), `is_template_literal`,
  `start_loc`/`end_loc`/`source_range`, the value getters (assert kind like the C++), and
  the private-ish setters (`set_punctuator`, `set_eof`, `set_range`, `set_start`/`set_end`
  taking offsets, `set_numeric_literal`, `set_identifier`, `set_res_word`, etc.). Make setters
  `pub(crate)` so the lexer can call them. Also `RegExpLiteral { body: AtomBytes, flags: AtomBytes }`,
  `StoredComment { kind: CommentKind, range: SMRange }` (+ `CommentKind::{Line,Block,Hashbang}`),
  `StoredToken { kind, range }`.

- [ ] **Step 4:** run → PASS. Zero warnings.
- [ ] **Step 5:** commit `rust(parser): Token / RegExpLiteral / StoredComment / StoredToken`.

---

## Task 4: `lexer` — JSLexer struct, advance (punct/trivia/eof), dump

**Files:** `rust/crates/parser/src/lexer.rs`.

- [ ] **Step 1: failing test** (construct + advance over punctuators, in-Rust):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use support::manager::SourceErrorManager;
    use atom_table::AtomTable;
    use parser::token_kinds::TokenKind;

    fn kinds(src: &str) -> Vec<TokenKind> {
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("t", src);
        let tab = AtomTable::new();
        let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
        let mut out = vec![];
        loop {
            let k = lex.advance().kind();
            out.push(k);
            if k == TokenKind::eof { break; }
        }
        out
    }

    #[test]
    fn punctuators_and_comments() {
        use TokenKind::*;
        assert_eq!(kinds("{ } ( ) ;"), vec![l_brace, r_brace, l_paren, r_paren, semi, eof]);
        assert_eq!(kinds("a /* c */ ;"), vec![/* 'a' is ident — deferred; use */ semi, eof]); // adjust: use only punct in 1a
    }
}
```
(Use only punctuator/comment/whitespace inputs in 1a tests — identifiers/numbers are stubbed.)

- [ ] **Step 2:** run → FAIL.
- [ ] **Step 3: implement.**
  - `GrammarContext { AllowRegExp, AllowDiv, AllowJSXIdentifier, Type }`.
  - `JSLexer<'a>` fields (port `JSLexer.h` members minus the allocator/JSX-entities):
    `sm: &'a mut SourceErrorManager`, `buf_id: SourceId`, `cursor: Cursor`,
    `strtab: &'a AtomTable`, `token: Token`, `prev_token_end: SMLoc`,
    `new_line_before_current_token: bool`, `strict_mode: bool`, `convert_surrogates: bool`,
    plus `tmp_storage: Vec<u8>`, `source_url`/`source_mapping_url: Option<String>`,
    comment/token storage `Vec`s, and a `grammar_context` passed to `advance`.
  - `new(buf_id, sm, strtab, ...) -> JSLexer`: clone `cursor = Cursor::new(sm.source_buffer(buf_id))`,
    init fields. (Reserved-word pre-interning + identifier scanning are 1b — for 1a, skip the
    resword table.)
  - `error(loc, msg)` / `error_range(range, msg)`: port `JSLexer.cpp:2497–2511` — call
    `sm.error*(...)`, then if `sm.is_error_limit_reached()` call `forceEOF` (`cursor.seek_end()`)
    and return false, else true. Use `Subsystem::Lexer`.
  - `advance(&mut self) -> &Token`: port `JSLexer.cpp:255–745`, implementing ONLY: the EOF case
    (`\0` at end → `set_eof`; `\0` mid-buffer → error "unrecognized Unicode character  "),
    ALL punctuator arms (the `PUNC_*` macros + `=`,`!`,`+`,`-`,`&`,`|`,`?`,`*`,`^`,`%`,`<`,`>`,
    `.`,`/` (as `slash`/`slashequal`/comments — NO regexp in 1a; treat AllowRegExp `/` like
    AllowDiv for now and TODO-note it), `{`/`}`/`(`/`)`/`[`/`]`/`;`/`,`/`~`/`:`/`@`(non-Type→at)),
    whitespace (`\r`/`\n` set newline flag, space/tab tight loop, `\v`/`\f`, U+2028/2029 via
    `match_unicode_line_terminator_offset1`, no-break-space `c2 a0`, BOM `ef bb bf`), and line
    (`scan_line_comment`/`line_comment_helper`) + block (`skip_block_comment`) comments
    (port `JSLexer.cpp:1430–1571`; for 1a, comment STORAGE + magic-comment URL parsing may be
    deferred — just skip correctly and track the newline flag; note the deferral). For the
    identifier/number/string/template/`#`/`\\`/non-ASCII-default arms, call `self.scan_*` stubs
    that `self.error(... "not yet implemented (phase 1b)")` and `forceEOF` (so 1a is well-defined
    but those inputs aren't in the corpus). Finish with `finish_token` (port `JSLexer.h:1077`)
    setting `prev_token_end` and the token end offset.
  - `dump_token(&self, out: &mut String)`: format EXACTLY like `js-lexer-dump`:
    `"{start} {end} {nl} {kind}"` where `nl` is `"nl"` if `new_line_before_current_token` else
    `"--"`, `kind` is the `.def` variant name. Add a `pub fn variant_name(TokenKind) -> &'static str`
    in `token_kinds` (a `match` over the `.def`, like the harness) if not already present.
    Phase 1a tokens are fieldless, so no per-kind fields yet.

- [ ] **Step 4:** run → PASS. Zero warnings.
- [ ] **Step 5:** commit `rust(parser): JSLexer skeleton + advance (punctuators/trivia/eof)`.

---

## Task 5: Live differential against `js-lexer-dump`

**Files:** `rust/crates/parser/tests/differential.rs`.

- [ ] **Step 1: write the differential test.** It builds nothing; it assumes
  `cmake-build-asan/bin/js-lexer-dump` exists (skip with a clear message if absent, like the
  support crate's `golden.rs` does for `hermesc`). For each corpus string: run the Rust lexer
  to produce a dump (concatenated `dump_token` lines, one per token incl. the final `eof`), run
  `js-lexer-dump --context=div -` with the string on stdin, and `assert_eq!` the two dumps.

```rust
use std::io::Write;
use std::process::{Command, Stdio};

fn rust_dump(src: &str) -> String {
    use parser::lexer::{JSLexer, GrammarContext};
    use parser::token_kinds::TokenKind;
    use support::manager::SourceErrorManager;
    use atom_table::AtomTable;
    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer("t", src);
    let tab = AtomTable::new();
    let mut lex = JSLexer::new(id, &mut sm, &tab, GrammarContext::AllowDiv);
    let mut out = String::new();
    loop {
        let k = lex.advance().kind();
        lex.dump_token(&mut out);
        out.push('\n');
        if k == TokenKind::eof { break; }
    }
    out
}

fn cpp_dump(src: &str) -> Option<String> {
    let bin = "cmake-build-asan/bin/js-lexer-dump";
    if !std::path::Path::new(bin).exists() { return None; }
    let mut child = Command::new(bin).arg("--context=div").arg("-")
        .stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().ok()?;
    child.stdin.take().unwrap().write_all(src.as_bytes()).ok()?;
    let out = child.wait_with_output().ok()?;
    Some(String::from_utf8(out.stdout).unwrap())
}

#[test]
fn differential_punctuators_and_trivia() {
    let corpus = [
        "{ } ( ) [ ] ; , ~ : @",
        "= == === => != !== ! < <= << <<= > >= >> >>> >>= >>>=",
        "+ ++ += - -- -= * *= ** **= / /= % %= & && &= | || |= ^ ^= ? ?? ?. ??= ...",
        "a;b\n;c", // newline flag (identifiers are deferred — replace 'a'/'b'/'c' with ; runs)
        ";;\n\t ;; \n\n ;",
        "; /* block\ncomment */ ;",
        "; // line comment\n;",
        "\u{feff}; ;", // BOM skipped
        "\u{00a0}; ;", // no-break space skipped
        "; \u{2028} ;", // line separator -> newline flag
    ];
    for src in corpus {
        let Some(cpp) = cpp_dump(src) else {
            eprintln!("skip: js-lexer-dump not built"); return;
        };
        assert_eq!(rust_dump(src), cpp, "mismatch for {src:?}");
    }
}
```
(Replace the identifier examples with punctuator-only equivalents — 1a does not lex
identifiers. Keep the trivia/newline cases.)

- [ ] **Step 2:** ensure the harness exists: `cmake --build cmake-build-asan --target js-lexer-dump`.
- [ ] **Step 3:** run `cargo test --manifest-path rust/Cargo.toml -p parser --test differential`
  → PASS (byte-for-byte equal to the C++ oracle).
- [ ] **Step 4:** full `cargo test --manifest-path rust/Cargo.toml -p parser` → all pass; zero warnings.
- [ ] **Step 5:** commit `rust(parser): live token differential vs js-lexer-dump (punct/trivia)`.

---

## Self-review checklist

- [ ] `unsafe` appears ONLY in `cursor.rs` (scoped `#[allow]`, documented invariants); rest of
  `parser` honours `unsafe_code = "deny"`.
- [ ] UTF-8 decode is a faithful port of UTF8.h (ASCII fast path + 2/3/4-byte + error paths).
- [ ] `advance` punctuator/whitespace/comment/EOF arms match `JSLexer.cpp:255–745` and
  `:1430–1571`; the newline flag is set for `\r`/`\n`/U+2028/U+2029 and inside comments.
- [ ] The Rust dump equals `js-lexer-dump --context=div` byte-for-byte on the corpus.
- [ ] Deferred-and-noted: identifiers, numbers, strings/templates/regexp, private-id, the
  AllowRegExp `/` regexp path, comment storage + magic-comment URLs, JSX/Flow. (Phases 1b+.)
- [ ] Zero warnings; all parser tests pass.

## Next

Phase 1b: identifiers (fast path + unicode + escapes) + reserved words + numbers wired to the
`number` module + the AllowRegExp `/` slash/regex decision (regex itself in phase 2). Then
phase 2 (literals), phase 3 (JSX/Flow), phase 4 (savepoint/lookahead). See the roadmap.
```
