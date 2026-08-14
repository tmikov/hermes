# JS Lexer Proper — Phase 1b-i: identifiers, reserved words, `\u` escapes

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Lex JavaScript **identifiers** (ASCII fast path, non-ASCII via Unicode ID properties, and `\u`/`\u{}` escapes) and **reserved words** (with strict-mode future-reserved-word filtering), interning names through `atom_table`, and extend the live `js-lexer-dump` differential to cover them. This adds the UTF-8 **encode** side and the escape/identifier scanners. Numbers are phase 1b-ii; strings/templates/regexp/private-id and JSX/Flow identifier modes are later phases.

**Architecture:** Extend the `parser` crate's `utf8`/`cursor`/`lexer` modules. `utf8` gains `encode_utf8` + `append_unicode_to_storage` (the WTF-8 surrogate-pair encoding for cp ≥ 0x10000). `cursor` gains `peek_utf8` (decode at the cursor without advancing). `lexer` gains the escape consumers, the identifier scanners (fast + slow), `scan_reserved_word` (+ strict filter), reserved-word pre-interning, the `advance` identifier arms, and the `ident=` dump field with byte-exact quoting. The lexer interns identifier bytes via `strtab.atom_bytes(...)` and stores `AtomBytes` in the token.

**Tech Stack:** Rust 2021; `unsafe` stays confined to `cursor.rs`. Uses `atom_table` (AtomBytes), `unicode` (`is_unicode_id_start/continue`, `is_unicode_only_id_start`, `is_unicode_only_space`, surrogate consts), and `parser::token_kinds` (`match_reserved_word`, the resword range).

**Reference spec:** `doc/superpowers/specs/2026-06-01-js-lexer-design.md`; prior phase `plans/2026-06-01-js-lexer-proper-1a.md`.
**C++ source of truth (READ):**
- `lib/Support/UTF8.cpp:12+` (`encodeUTF8`), `include/hermes/Parser/JSLexer.h:1120–1143` (`initStorageWith`, `appendUnicodeToStorage` — note the surrogate-pair split for cp ≥ 0x10000).
- `lib/Parser/JSLexer.cpp:1159–1309` (`consumeUnicodeEscape`, `consumeUnicodeEscapeOptional`, `consumeIdentifierStart`, `consumeOneIdentifierPartNoEscape`, `consumeIdentifierParts`), `:1329–1428` (`consumeHex`, `consumeBracedCodePoint`), `:1865–1949` (`scanReservedWord`, `scanIdentifierFastPath`, `scanIdentifierParts`), `:111–115` (`initializeReservedIdentifiers`), and the `advance` arms `:650–735` (digits→number is 1b-ii; the letter/`_`/`$` arm, the `\\` arm, the `@` Flow arm, and the non-ASCII `default` arm).
- `tools/js-lexer-dump/js-lexer-dump.cpp` — the `ident=` field + `quoteBytes` (port `Q` exactly).

**Porting rule:** faithful port; copy comments. For 1b-i implement **`IdentifierMode::JS`** only; include the `Mode` enum and the JSX(`-`)/Flow(`@`) conditions in `consume_one_identifier_part_no_escape` (cheap, forward-compatible) but only JS is exercised now (the differential stays `--context=div`).

**Do NOT** `cd` out of the project root.

---

## Task 0: UTF-8 encode side (`utf8.rs`)

- [ ] **Step 1: failing tests:**

```rust
#[test]
fn encode_basic() {
    let mut v = vec![];
    encode_utf8(&mut v, 'a' as u32); assert_eq!(v, b"a");
    let mut v = vec![];
    encode_utf8(&mut v, 0x00E9); assert_eq!(v, b"\xc3\xa9");      // é
    let mut v = vec![];
    encode_utf8(&mut v, 0x4E2D); assert_eq!(v, b"\xe4\xb8\xad");  // 中
}

#[test]
fn append_storage_surrogate_pair() {
    // BMP: plain UTF-8.
    let mut v = vec![];
    append_unicode_to_storage(&mut v, 0x00E9);
    assert_eq!(v, b"\xc3\xa9");
    // Astral U+1F600: split into surrogate pair, each encoded as 3-byte WTF-8.
    // high = 0xD83D, low = 0xDE00 -> ed a0 bd  ed b8 80
    let mut v = vec![];
    append_unicode_to_storage(&mut v, 0x1F600);
    assert_eq!(v, b"\xed\xa0\xbd\xed\xb8\x80");
}
```

- [ ] **Step 2:** run → FAIL.
- [ ] **Step 3: implement** (port `UTF8.cpp:encodeUTF8` and `JSLexer.h:appendUnicodeToStorage`):

```rust
/// Encode a Unicode code point as UTF-8 (up to the legacy 6-byte form, matching
/// `encodeUTF8`), appending the bytes to `out`.
pub fn encode_utf8(out: &mut Vec<u8>, cp: u32) {
    // PORT UTF8.cpp:12+ faithfully (the <=0x7F / 0x7FF / 0xFFFF / 0x1FFFFF /
    // 0x3FFFFFF / else branches). Push bytes in order.
}

/// Encode `cp` into `storage` like the lexer's `appendUnicodeToStorage`: code
/// points above 0xFFFF are first split into a UTF-16 surrogate pair, and each
/// surrogate is encoded individually into UTF-8 (technically invalid UTF-8 /
/// WTF-8, which JS string & identifier storage allows).
pub fn append_unicode_to_storage(storage: &mut Vec<u8>, cp: u32) {
    use unicode::{UTF16_HIGH_SURROGATE, UTF16_LOW_SURROGATE, UNICODE_MAX_VALUE};
    if cp < 0x10000 {
        encode_utf8(storage, cp);
    } else {
        debug_assert!(cp <= UNICODE_MAX_VALUE);
        let cp = cp - 0x10000;
        encode_utf8(storage, UTF16_HIGH_SURROGATE + ((cp >> 10) & 0x3FF));
        encode_utf8(storage, UTF16_LOW_SURROGATE + (cp & 0x3FF));
    }
}
```

- [ ] **Step 4:** run → PASS. **Step 5:** commit `rust(parser): UTF-8 encode + appendUnicodeToStorage (WTF-8)`.

---

## Task 1: cursor `peek_utf8` (`cursor.rs`)

The C++ `_peekUTF8()` decodes the multibyte char at the cursor WITHOUT advancing and
returns `(cp, next_ptr)`. Add the offset-based equivalent.

- [ ] **Step 1: failing test:**

```rust
#[test]
fn peek_utf8_no_advance() {
    let c = cur("\u{4e2d}x"); // 中 = e4 b8 ad
    let (cp, next) = c.peek_utf8();
    assert_eq!(cp, 0x4E2D);
    assert_eq!(next, 3);       // offset of 'x'
    assert_eq!(c.offset(), 0); // cursor did not move
}
```

- [ ] **Step 2:** FAIL. **Step 3: implement** using `utf8::decode_utf8`:

```rust
/// Decode the (non-ASCII) UTF-8 char at the cursor WITHOUT advancing. Errors are
/// swallowed (matches `_peekUTF8`). Returns `(code_point, offset_after)`.
pub fn peek_utf8(&self) -> (u32, u32) {
    let bytes = self.buffer.raw();
    let mut i = self.offset() as usize;
    let cp = crate::utf8::decode_utf8::<true>(bytes, &mut i, |_| {});
    (cp, i as u32)
}
```
(Also add `decode_utf8_at(&self) -> u32` advancing form if the lexer's `decodeUTF8`/default
arm needs it — or keep that in the lexer using the cursor's offset; choose the minimal seam.)

- [ ] **Step 4:** PASS. **Step 5:** commit `rust(parser): cursor peek_utf8 (decode without advancing)`.

---

## Task 2: escape consumers (`lexer.rs`)

Port `consumeHex`, `consumeBracedCodePoint`, `consumeUnicodeEscape`, `consumeUnicodeEscapeOptional`
(`JSLexer.cpp:1159–1227, 1329–1428`). They advance the cursor and report errors via the
lexer's `error`/`error_range`.

- [ ] **Step 1: failing tests** (drive via a small harness that lexes a single identifier or
  calls the escape directly — simplest is to test through `advance` once Task 3 lands; for
  Task 2 alone, add `pub(crate)` test hooks or unit-test the consumers on a constructed lexer):

```rust
// e.g. exposed via advance() in Task 3; for Task 2, a targeted test:
#[test]
fn unicode_escape_4hex_and_braced() {
    assert_eq!(consume_escape_for_test("\\u0041"), Some(0x41)); // 'A'
    assert_eq!(consume_escape_for_test("\\u{1F600}"), Some(0x1F600));
    assert_eq!(consume_escape_for_test("\\u{}"), None);         // empty -> error
    assert_eq!(consume_escape_for_test("\\uXY"), None);         // bad hex -> error
}
```
(Provide a `#[cfg(test)]` helper `consume_escape_for_test(&str) -> Option<u32>` that builds a
lexer over the string, calls `consume_unicode_escape`, and returns the cp unless an error was
emitted — count `sm.error_count()`.)

- [ ] **Step 2:** FAIL. **Step 3: implement** the four consumers faithfully. Notes:
  `consume_hex(required_len, error_on_fail) -> Option<u32>`; `consume_braced_code_point(error_on_fail)`
  loops to `}` accumulating hex, reporting empty/invalid/too-large/non-terminated exactly as
  the C++ (incl. the `failed` flag + `errorOnFail` gating); `consume_unicode_escape()` assumes
  `\` at cursor, requires `u`, dispatches `{`→braced or 4-hex, returns the cp (or
  `UNICODE_REPLACEMENT_CHARACTER` on error); `consume_unicode_escape_optional()` resets the
  cursor to the start on any failure and returns `None` (used later by regexp/templates — port
  it now for completeness).
- [ ] **Step 4:** PASS. **Step 5:** commit `rust(parser): unicode escape consumers (\u / \u{})`.

---

## Task 3: identifier scanners + reserved words + advance arms (`lexer.rs`)

- [ ] **Step 1: failing test** (through `advance`):

```rust
#[test]
fn identifiers_and_reswords() {
    use TokenKind::*;
    // helper `idents(src)` returns Vec<(TokenKind, Option<Vec<u8>>)> of (kind, ident-bytes)
    assert_eq!(kinds("foo _bar $x9"), vec![identifier, identifier, identifier, eof]);
    assert_eq!(kinds("function for yield"),
               vec![rw_function, rw_for, rw_yield, eof]); // strict mode default
    // non-strict: yield is an identifier
    assert_eq!(kinds_nonstrict("yield"), vec![identifier, eof]);
    // unicode identifier
    assert_eq!(kinds("\u{00e9}tude"), vec![identifier, eof]); // étude
    // escaped identifier start
    assert_eq!(kinds("\\u0041bc"), vec![identifier, eof]);    // 'Abc'
    // ident value round-trips through the interner
    assert_eq!(ident_bytes("caf\u{00e9}"), b"caf\xc3\xa9");
}
```

- [ ] **Step 2:** FAIL. **Step 3: implement:**
  - `IdentifierMode { JS, JSX, Flow }` (only JS exercised now).
  - `initialize_reserved_identifiers(&mut self)`: for each RESWORD, intern its name and store
    the `AtomBytes` indexed by `ord(kind) - ord(_first_resword)` (a `Vec<AtomBytes>`). Add
    `res_word_ident(&self, kind) -> AtomBytes`. (Port `JSLexer.cpp:111–115`; call from `new()`.)
  - `consume_identifier_start`, `consume_one_identifier_part_no_escape::<Mode>`,
    `consume_identifier_parts::<Mode>` (port `:1228–1309`) — building into `tmp_storage` via
    `append_unicode_to_storage`, using `cursor.peek_utf8()` for non-ASCII and
    `consume_unicode_escape()` for `\`.
  - `scan_reserved_word(bytes) -> TokenKind` (port `:1865–1887`): `match_reserved_word(bytes)`
    then the strict-mode filter (in non-strict mode, downgrade implements/interface/package/
    private/protected/public/static/yield to `identifier`).
  - `scan_identifier_fast_path::<Mode>(start_offset)` (port `:1889–1933`): scan ASCII ident
    bytes via the cursor; on `\`/UTF8-start, copy `[start, here)` into `tmp_storage` and fall to
    `scan_identifier_parts`; else intern the buffer slice directly. Set the token to a reserved
    word (`res_word_ident`) or `identifier` (`strtab.atom_bytes(slice)`).
  - `scan_identifier_parts::<Mode>()` (port `:1935–1949`): consume parts, `scan_reserved_word`
    on `tmp_storage`; if a resword, also emit the warning "scanning identifier with unicode
    escape as reserved word" (Subsystem::Lexer). Intern `tmp_storage`.
  - Wire `advance` arms (replace the 1a stubs): the ASCII letter/`_`/`$` arm →
    `scan_identifier_fast_path_in_context`; the `\\` arm (port `:683–698`):
    `consume_unicode_escape`, if `!is_unicode_id_start(cp)` errorRange else
    `append_unicode_to_storage` then `scan_identifier_parts`; the non-ASCII `default` arm (port
    `:711–735`): `decode_utf8` the char, if `is_unicode_only_id_start` → identifier slow path,
    elif `is_unicode_only_space` → skip(continue), else the unrecognized-character errors. The
    `@` arm stays `at` for non-Type context (Flow `@`-ident is phase 3).
- [ ] **Step 4:** PASS. **Step 5:** commit `rust(parser): identifier + reserved-word lexing (JS mode)`.

---

## Task 4: dump `ident=` field + extend the differential

- [ ] **Step 1:** Extend `dump_token` to emit fields, starting with identifiers/reswords. Port
  the harness `Q` (`quoteBytes`) exactly into a `quote_bytes(out: &mut String, bytes: &[u8])`:
  double-quote; `"`→`\"`, `\`→`\\`, `\n`→`\n`, `\t`→`\t`, `\r`→`\r`; printable ASCII
  `0x20..=0x7e` literal; else `\xHH` lowercase. For `identifier`/`private_identifier`/any
  reserved word, emit ` ident=` + `quote_bytes(strtab.bytes(token.ident))`.

- [ ] **Step 2:** Extend `tests/differential.rs` corpus with identifier/resword inputs (still
  `--context=div`), e.g.:

```rust
let corpus = [ /* ...existing 1a punct/trivia cases... */
    "foo bar baz",
    "_x $y a1 Z9",
    "function for while return yield static implements", // strict-mode reswords
    "caf\u{00e9} \u{4e2d}\u{6587} na\u{00ef}ve",          // unicode identifiers
    "\\u0041\\u0042 ab\\u0063",                            // escaped identifiers
    "x;y\nz",                                             // newline flag between idents
];
```
Add a second pass that lexes the same corpus with a **non-strict** lexer and compares against
`js-lexer-dump` run **without** strict mode — BUT the harness constructs the lexer strict by
default and has no `--non-strict` flag yet. So for 1b-i keep the differential in the harness's
default (strict) mode only; non-strict resword downgrade is covered by the in-Rust unit test in
Task 3. (Optionally: add a `--non-strict` flag to `js-lexer-dump` as a tiny follow-up; not
required here — note it.)

- [ ] **Step 3:** `cmake --build cmake-build-asan --target js-lexer-dump` (unchanged) then
  `cargo test --manifest-path rust/Cargo.toml -p parser --test differential` → PASS byte-for-byte.
- [ ] **Step 4:** full `cargo test -p parser` → all pass; zero warnings; `unsafe` only in `cursor.rs`.
- [ ] **Step 5:** commit `rust(parser): dump ident= field + identifier/resword differential`.

---

## Self-review checklist

- [ ] `encode_utf8`/`append_unicode_to_storage` match the C++ (incl. the astral surrogate-pair
  WTF-8 split — `U+1F600` → `ed a0 bd ed b8 80`).
- [ ] Escape consumers reproduce every error path of `consumeHex`/`consumeBracedCodePoint`/
  `consumeUnicodeEscape` (empty/invalid/too-large/non-terminated; `errorOnFail` gating).
- [ ] Identifier fast path interns the buffer slice directly when pure-ASCII; slow path builds
  WTF-8 in `tmp_storage`; reserved words resolved via `scan_reserved_word` + strict filter;
  the unicode-escape-resword warning is emitted.
- [ ] `advance` identifier/`\\`/non-ASCII arms match `JSLexer.cpp`; `is_unicode_only_space` in
  the default arm skips correctly.
- [ ] Rust dump `ident=` equals `js-lexer-dump` byte-for-byte (incl. `\xHH` for non-ASCII).
- [ ] Deferred-and-noted: numbers (1b-ii), JSX/Flow ident modes + `@`/`%checks` (phase 3),
  private identifiers `#` + strings/templates/regexp (phase 2), `--non-strict` harness flag.
- [ ] `unsafe` only in `cursor.rs`; zero warnings; all tests pass.

## Next
Phase 1b-ii: numbers — port `scanNumber` (`JSLexer.cpp:1573–1856`) wiring `parser::number`
(`str_to_double` + `parse_int_with_radix`), with `bits=`/bigint dump fields and a numeric
differential corpus. Then phase 2 (literals), 3 (JSX/Flow), 4 (savepoint/lookahead).
```
