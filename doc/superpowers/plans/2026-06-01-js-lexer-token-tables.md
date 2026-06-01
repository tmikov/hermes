# JS Lexer — Token Tables (subsystem ①) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port Hermes's token vocabulary (`include/hermes/Parser/TokenKinds.def`) to Rust: the `TokenKind` enum, its name/precedence tables, the range-marker predicates, and `matchReservedWord`.

**Architecture:** A new `parser` crate under the existing `rust/` workspace. `TokenKind` is a `#[repr(u16)]` enum whose variants appear in **exact `.def` order** (including the `RANGE_MARKER` variants), so `ord(kind) == kind as u16` and the marker-range predicates (`isResWord`, `_first_binary..=_last_binary`) stay integer comparisons — identical to C++. A declarative `define_tokens!` macro is fed one row per `.def` line and emits the enum, the `token_kind_str` name table, the binop precedence table, and `is_punctuator`. No `unsafe`.

**Tech Stack:** Rust (edition 2021), cargo workspace, std only.

**Reference spec:** `doc/superpowers/specs/2026-06-01-js-lexer-design.md`.
**C++ source of truth:** `include/hermes/Parser/TokenKinds.def`, `include/hermes/Parser/JSLexer.h` (`TokenKind`, `ord`, `isPunctuatorDbg`, `NUM_JS_TOKENS`, `tokenKindStr`), `lib/Parser/JSLexer.cpp:1858` (`matchReservedWord`).

**Porting rule (every task):** keep the Rust structure close to the C++ original and **copy the comments**. When a step says "transcribe `TokenKinds.def`", read the actual `.def` and reproduce every row **in order**, copying its section comments (`// Future reserved words`, `// Strict mode future reserved words`, the `=`/`==`/`===` grouping comments are in `JSLexer.cpp`, not the `.def` — only copy comments present in the `.def`).

**Scope note:** `scanReservedWord`'s strict-mode future-reserved-word filter (`JSLexer.cpp:1865`) is **NOT** in this subsystem — it needs `strictMode_` and belongs to lexer-core. `matchReservedWord` (the pure `StringSwitch` on bytes+length) **is** in scope. `HTMLEntities.def` is deferred to the JSX work (only consumed by JSX scanning); not in this subsystem.

---

## File structure

```
rust/
  Cargo.toml                       # workspace — add "crates/parser" to members
  crates/
    parser/
      Cargo.toml                   # new crate, edition 2021, no deps
      src/
        lib.rs                     # pub mod token_kinds;
        token_kinds.rs             # define_tokens! macro + TokenKind + tables + matchReservedWord
```

Reference facts (computed from the current `.def`, used as test anchors):
- **123** `TokenKind` entries, ordinals `0..=122`. `NUM_JS_TOKENS == 123`, `ord(_last_token) == 122`.
- First/last reswords bracket `rw_function .. rw_yield`. `_first_resword` precedes `rw_function`; `_last_resword` follows `rw_yield`.
- `_first_binary` precedes `starstar`; `_last_binary` follows `questionquestion`.
- Binop precedences: `starstar`=12; `star`/`percent`/`slash`=11; `plus`/`minus`=10; `lessless`/`greatergreater`/`greatergreatergreater`=9; `less`/`greater`/`lessequal`/`greaterequal`=8; `equalequal`/`exclaimequal`/`equalequalequal`/`exclaimequalequal`=7; `amp`=6; `caret`=5; `pipe`=4; `ampamp`=3; `pipepipe`=2; `questionquestion`=1. `as_operator` (IDENT_OP)=8.

---

## Task 0: Workspace member + parser crate scaffold

**Files:**
- Modify: `rust/Cargo.toml` (workspace `members`)
- Create: `rust/crates/parser/Cargo.toml`
- Create: `rust/crates/parser/src/lib.rs`

- [ ] **Step 1: Add the crate to the workspace**

Edit `rust/Cargo.toml` `members` from `["crates/support"]` to:

```toml
members = ["crates/support", "crates/parser"]
```

- [ ] **Step 2: Create `rust/crates/parser/Cargo.toml`**

```toml
[package]
name = "parser"
version = "0.0.0"
edition = "2021"
publish = false

[lints.rust]
unsafe_code = "deny"   # the cursor module (later) gets a scoped #[allow]; token tables are unsafe-free

[dependencies]
```

- [ ] **Step 3: Create `rust/crates/parser/src/lib.rs`**

```rust
//! Hermes JavaScript parser (Rust port) — currently the token vocabulary.

pub mod token_kinds;
```

- [ ] **Step 4: Verify it builds**

Run: `cargo build --manifest-path rust/Cargo.toml -p parser`
Expected: compiles (empty `token_kinds` module will be added next; for this step create `src/token_kinds.rs` empty so the `mod` resolves).

Create `rust/crates/parser/src/token_kinds.rs` empty (one line):

```rust
//! Token kinds, ported from include/hermes/Parser/TokenKinds.def.
```

Run: `cargo build --manifest-path rust/Cargo.toml -p parser`
Expected: PASS (compiles clean).

- [ ] **Step 5: Commit**

```bash
git add rust/Cargo.toml rust/crates/parser/Cargo.toml rust/crates/parser/src/lib.rs rust/crates/parser/src/token_kinds.rs
git commit -m "rust(parser): scaffold parser crate for token tables"
```

---

## Task 1: `define_tokens!` macro + `TokenKind` enum

**Files:**
- Modify: `rust/crates/parser/src/token_kinds.rs`

The macro takes one row per `.def` line. Row forms mirror the `.def` macros:
`tok(Name, "str")`, `resword(name)` (→ `rw_name`, `"name"`), `punct(Name, "str")`,
`punct_flow(Name, "str")`, `binop(Name, "str", prec)`, `template(Name, "str")`,
`ident_op(Name, "str", prec)`, `marker(Name)` (→ `"<Name>"`). Each row becomes one
enum variant, in order, with default sequential discriminants.

- [ ] **Step 1: Write the failing test**

Append to `rust/crates/parser/src/token_kinds.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinals_and_count() {
        assert_eq!(TokenKind::none as u16, 0);
        assert_eq!(TokenKind::identifier as u16, 1);
        assert_eq!(TokenKind::private_identifier as u16, 2);
        // _last_token is the final entry; NUM_JS_TOKENS == ord(_last_token)+1.
        assert_eq!(TokenKind::_last_token as u16, 122);
        assert_eq!(NUM_JS_TOKENS, 123);
        // Round-trip ord().
        assert_eq!(ord(TokenKind::eof), TokenKind::eof as u16 as i32);
    }

    #[test]
    fn resword_range() {
        assert!(TokenKind::rw_function.is_res_word());
        assert!(TokenKind::rw_yield.is_res_word());
        assert!(!TokenKind::identifier.is_res_word());
        assert!(!TokenKind::l_brace.is_res_word());
        // Markers themselves are not reswords.
        assert!(!TokenKind::_first_resword.is_res_word());
        assert!(!TokenKind::_last_resword.is_res_word());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path rust/Cargo.toml -p parser`
Expected: FAIL — `TokenKind`, `NUM_JS_TOKENS`, `ord`, `is_res_word` undefined.

- [ ] **Step 3: Implement the macro + enum**

In `rust/crates/parser/src/token_kinds.rs`, above the tests, write the `define_tokens!`
macro and invoke it by **transcribing every row of `include/hermes/Parser/TokenKinds.def`
in order**, copying the `.def`'s section comments. The macro shape:

```rust
macro_rules! define_tokens {
    ( $( $kind:ident { variant: $variant:ident, str: $str:expr, prec: $prec:expr } ),* $(,)? ) => {
        /// JavaScript token kinds, in the exact order of TokenKinds.def so that
        /// `ord()` and the range-marker comparisons match the C++ lexer.
        #[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
        #[repr(u16)]
        #[allow(non_camel_case_types)]
        pub enum TokenKind {
            $( $variant, )*
        }

        /// Number of token kinds = ord(_last_token) + 1.
        pub const NUM_JS_TOKENS: usize = TokenKind::_last_token as usize + 1;

        const TOKEN_NAMES: [&str; NUM_JS_TOKENS] = [ $( $str, )* ];
        const TOKEN_PREC: [u8; NUM_JS_TOKENS] = [ $( $prec, )* ];
    };
}
```

To keep each `.def` row terse at the call site, add thin internal helper macros that
expand each `.def`-style row into the canonical `{ variant, str, prec }` form, then call
`define_tokens!`. Implement these helper arms so the call site reads one line per `.def`
entry, e.g.:

```rust
// Helper that turns .def-style rows into canonical rows, preserving order.
macro_rules! tokens {
    () => {
        define_tokens! {
            tok!(none, "<none>"),
            tok!(identifier, "identifier"),
            tok!(private_identifier, "private identifier"),

            marker!(_first_resword),
            resword!(function),
            resword!(for),
            // ... transcribe EVERY remaining row of TokenKinds.def here, in order,
            //     copying the // Future reserved words and
            //     // Strict mode future reserved words comments ...
            resword!(yield),
            marker!(_last_resword),

            punct!(l_brace, "{"),
            punct_flow!(l_bracepipe, "{|"),
            // ... etc. through ...
            tok!(eof, "<eof>"),
            marker!(_last_token),
        }
    };
}
```

Define `tok!`/`resword!`/`punct!`/`punct_flow!`/`binop!`/`template!`/`ident_op!`/`marker!`
to expand to the canonical `IDENT { variant, str, prec }` row (`resword!(x)` → variant
`rw_x`, str `"x"`, prec `0`; `marker!(x)` → variant `x`, str `"<x>"`, prec `0`;
`binop!`/`ident_op!` carry the precedence; everything else prec `0`). Invoke `tokens!{}`.

Then add `ord`, `is_res_word`, and binary-range constants:

```rust
/// \return the integer ordinal of `kind` (matches C++ `ord`).
#[inline]
pub const fn ord(kind: TokenKind) -> i32 {
    kind as u16 as i32
}

impl TokenKind {
    /// True if this is a reserved word (strictly between the range markers).
    #[inline]
    pub fn is_res_word(self) -> bool {
        (self as u16) > (TokenKind::_first_resword as u16)
            && (self as u16) < (TokenKind::_last_resword as u16)
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path rust/Cargo.toml -p parser`
Expected: PASS (both `ordinals_and_count` and `resword_range`).

- [ ] **Step 5: Commit**

```bash
git add rust/crates/parser/src/token_kinds.rs
git commit -m "rust(parser): TokenKind enum from TokenKinds.def (ordinals + resword range)"
```

---

## Task 2: `token_kind_str` name table

**Files:**
- Modify: `rust/crates/parser/src/token_kinds.rs`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module:

```rust
#[test]
fn names_match_def() {
    assert_eq!(token_kind_str(TokenKind::none), "<none>");
    assert_eq!(token_kind_str(TokenKind::identifier), "identifier");
    assert_eq!(token_kind_str(TokenKind::private_identifier), "private identifier");
    assert_eq!(token_kind_str(TokenKind::rw_function), "function");
    assert_eq!(token_kind_str(TokenKind::l_brace), "{");
    assert_eq!(token_kind_str(TokenKind::starstar), "**");
    assert_eq!(token_kind_str(TokenKind::as_operator), "as");
    assert_eq!(token_kind_str(TokenKind::eof), "<eof>");
    assert_eq!(token_kind_str(TokenKind::_first_resword), "<_first_resword>");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path rust/Cargo.toml -p parser -- names_match_def`
Expected: FAIL — `token_kind_str` undefined.

- [ ] **Step 3: Implement**

```rust
/// \return the human-readable name of `kind` (matches C++ `tokenKindStr`).
#[inline]
pub fn token_kind_str(kind: TokenKind) -> &'static str {
    TOKEN_NAMES[kind as usize]
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path rust/Cargo.toml -p parser -- names_match_def`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/parser/src/token_kinds.rs
git commit -m "rust(parser): token_kind_str name table"
```

---

## Task 3: binop precedence + `is_punctuator`

**Files:**
- Modify: `rust/crates/parser/src/token_kinds.rs`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module:

```rust
#[test]
fn binop_precedence() {
    assert_eq!(binop_precedence(TokenKind::starstar), Some(12));
    assert_eq!(binop_precedence(TokenKind::star), Some(11));
    assert_eq!(binop_precedence(TokenKind::plus), Some(10));
    assert_eq!(binop_precedence(TokenKind::lessless), Some(9));
    assert_eq!(binop_precedence(TokenKind::less), Some(8));
    assert_eq!(binop_precedence(TokenKind::equalequal), Some(7));
    assert_eq!(binop_precedence(TokenKind::amp), Some(6));
    assert_eq!(binop_precedence(TokenKind::caret), Some(5));
    assert_eq!(binop_precedence(TokenKind::pipe), Some(4));
    assert_eq!(binop_precedence(TokenKind::ampamp), Some(3));
    assert_eq!(binop_precedence(TokenKind::pipepipe), Some(2));
    assert_eq!(binop_precedence(TokenKind::questionquestion), Some(1));
    // Non-binary tokens have no precedence.
    assert_eq!(binop_precedence(TokenKind::l_brace), None);
    assert_eq!(binop_precedence(TokenKind::eof), None);
}

#[test]
fn punctuator_predicate() {
    assert!(TokenKind::l_brace.is_punctuator());
    assert!(TokenKind::starstar.is_punctuator());      // BINOP expands to PUNCTUATOR
    assert!(TokenKind::at.is_punctuator());
    assert!(!TokenKind::identifier.is_punctuator());
    assert!(!TokenKind::rw_function.is_punctuator());
    assert!(!TokenKind::numeric_literal.is_punctuator());
    assert!(!TokenKind::as_operator.is_punctuator());  // IDENT_OP is not a punctuator
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path rust/Cargo.toml -p parser -- binop_ punctuator_`
Expected: FAIL — `binop_precedence` / `is_punctuator` undefined.

- [ ] **Step 3: Implement**

`binop_precedence` reads `TOKEN_PREC` but is only meaningful inside the binary marker
range; return `None` outside it (prec `0` is also the sentinel for non-binops):

```rust
impl TokenKind {
    #[inline]
    fn in_binary_range(self) -> bool {
        (self as u16) > (TokenKind::_first_binary as u16)
            && (self as u16) < (TokenKind::_last_binary as u16)
    }

    /// True if `self` is a punctuator (mirrors `isPunctuatorDbg`). The set is the
    /// PUNCTUATOR/PUNCTUATOR_FLOW/BINOP tokens between `l_brace` and `at` inclusive.
    #[inline]
    pub fn is_punctuator(self) -> bool {
        (self as u16) >= (TokenKind::l_brace as u16)
            && (self as u16) <= (TokenKind::at as u16)
    }
}

/// \return the binary-operator precedence of `kind`, or None if not a binary op.
#[inline]
pub fn binop_precedence(kind: TokenKind) -> Option<u8> {
    if kind.in_binary_range() {
        Some(TOKEN_PREC[kind as usize])
    } else {
        None
    }
}
```

Note: confirm during implementation that `l_brace .. at` is a contiguous punctuator run
in the `.def` (it is: `l_brace` through `at`, including the `PUNCTUATOR_FLOW` and `BINOP`
rows, with `_first_binary`/`_last_binary` markers inside). The markers sit *inside* that
range but are not punctuators — verify `is_punctuator` returns false for them and adjust
to an explicit set if the contiguity assumption fails.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path rust/Cargo.toml -p parser -- binop_ punctuator_`
Expected: PASS.

If the markers break the contiguous-range assumption for `is_punctuator`, add a test
asserting `!_first_binary.is_punctuator()` and switch `is_punctuator` to a generated
`match` over the punctuator variants instead of a range check.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/parser/src/token_kinds.rs
git commit -m "rust(parser): binop precedence table + is_punctuator"
```

---

## Task 4: `match_reserved_word`

**Files:**
- Modify: `rust/crates/parser/src/token_kinds.rs`

Ports `matchReservedWord` (`JSLexer.cpp:1858`): a pure `StringSwitch` over the reserved
words, returning `TokenKind::identifier` for non-matches. Strict-mode filtering is NOT
here (it's lexer-core).

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn reserved_words() {
    assert_eq!(match_reserved_word(b"function"), TokenKind::rw_function);
    assert_eq!(match_reserved_word(b"yield"), TokenKind::rw_yield);
    assert_eq!(match_reserved_word(b"static"), TokenKind::rw_static);
    assert_eq!(match_reserved_word(b"extends"), TokenKind::rw_extends);
    // Non-reserved -> identifier.
    assert_eq!(match_reserved_word(b"fora"), TokenKind::identifier);
    assert_eq!(match_reserved_word(b"Function"), TokenKind::identifier); // case-sensitive
    assert_eq!(match_reserved_word(b""), TokenKind::identifier);
    assert_eq!(match_reserved_word(b"let"), TokenKind::identifier); // 'let' is not a TokenKinds.def resword
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path rust/Cargo.toml -p parser -- reserved_words`
Expected: FAIL — `match_reserved_word` undefined.

- [ ] **Step 3: Implement**

Generate the match from the same reserved-word list. Add a `resword!`-driven companion
that also emits `(b"name", TokenKind::rw_name)` arms, or write the `match` directly over
the byte slice:

```rust
/// Recognise a reserved word by its bytes (mirrors `matchReservedWord`). Returns
/// `TokenKind::identifier` if `bytes` is not a reserved word. Pure: no strict-mode
/// filtering (that lives in lexer-core's scanReservedWord).
pub fn match_reserved_word(bytes: &[u8]) -> TokenKind {
    match bytes {
        b"function" => TokenKind::rw_function,
        b"for" => TokenKind::rw_for,
        // ... transcribe EVERY RESWORD row of TokenKinds.def, in order ...
        b"yield" => TokenKind::rw_yield,
        _ => TokenKind::identifier,
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path rust/Cargo.toml -p parser -- reserved_words`
Expected: PASS.

- [ ] **Step 5: Full crate test + commit**

Run: `cargo test --manifest-path rust/Cargo.toml -p parser`
Expected: PASS (all token-table tests).

```bash
git add rust/crates/parser/src/token_kinds.rs
git commit -m "rust(parser): match_reserved_word"
```

---

## Self-review checklist (run after Task 4)

- [ ] **Spec coverage:** `TokenKind` (✓ Task 1), `tokenKindStr` (✓ Task 2), precedence +
  `isPunctuatorDbg` (✓ Task 3), `matchReservedWord` (✓ Task 4), range predicates
  (`isResWord` ✓ Task 1, binary range ✓ Task 3). `NUM_JS_TOKENS`/`ord` ✓ Task 1.
  Deferred-by-design: strict-mode resword filter (lexer-core), `HTMLEntities.def` (JSX).
- [ ] **No drift:** the 123-count and marker-ordinal asserts (Task 1) guard against a
  missed/duplicated `.def` row. If they fail, a row was mis-transcribed — diff against
  `TokenKinds.def`.
- [ ] **Naming consistency:** `token_kind_str`, `binop_precedence`, `match_reserved_word`,
  `is_res_word`, `is_punctuator`, `ord`, `NUM_JS_TOKENS` used identically across tasks.
- [ ] `cargo build -p parser` emits **zero warnings**; crate has **zero `unsafe`**.

## Next subsystem

After this lands, the next plan is ② the C++ token-dump harness (`tools/js-lexer-dump/`),
then ③ the string interner (copy juno `atom_table` + WTF-8 path). See
`doc/superpowers/RustPortRoadmap.md`.
