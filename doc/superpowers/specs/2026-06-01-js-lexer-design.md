# JS Lexer — Rust Port Design

Design for porting Hermes's JavaScript lexer (`include/hermes/Parser/JSLexer.h` +
`lib/Parser/JSLexer.cpp`, ~3,700 LOC) to Rust as the second component of the Rust
port, sitting directly on top of the completed `support` crate (`SourceErrorManager`).

> **STATUS: IN PROGRESS — design complete, implementation not started.** The
> `support` crate (everything the lexer needs from `SourceErrorManager`) is done and
> validated byte-for-byte against `hermesc` 1.96.0. This document is the settled
> design for the lexer and its support-layer prerequisites; for live project state see
> `doc/superpowers/RustPortRoadmap.md`. Per-subsystem implementation plans live under
> `doc/superpowers/plans/`.

## Context & goals

The lexer is the natural next port after the diagnostics foundation: it depends only
on the support layer, is self-contained, and is trivially differential-testable
(bytes in → tokens out). We port its **entire public surface in one pass** — including
the JSX and Flow lexing paths, which live *inside* `JSLexer` (they are gated by
`HERMES_PARSE_JSX` / `HERMES_PARSE_FLOW` macros in C++; in Rust they are part of the
lexer, behind cargo features if we want to mirror the gating).

The lexer has four **support-layer prerequisites** that are separate ports, sequenced
before the lexer proper. They are *the lexer's own deps*, not part of
`SourceErrorManager`, and not "everything in the world": string interning, Unicode
character properties, number parsing, and the token tables.

### Project conventions

See `doc/superpowers/RustPortRoadmap.md` and the project memories. In brief: all Rust
code lives under `rust/` (cargo workspace); juno code is **copied** into `rust/`, never
referenced in place; keep the Rust structure close to the C++ and **copy the comments**;
work stays on the `rust` branch (base `static_h`), no PRs/merges; docs under `doc/`.

## Foundational decisions

These were settled during design (this session) and constrain everything below.

1. **Unsafe policy — encapsulated, never leaking.** Goal is minimal `unsafe`. The
   `support` crate stays zero-`unsafe`. New crates are zero-`unsafe` *except* the two
   places below, each well-encapsulated and converted to safe types at every boundary.

2. **Scan cursor — raw `*const u8`, option "B" (CONFIRMED).** The lexer uses a raw
   `*const u8` cursor internally for throughput parity with the C++ lexer's pointer
   arithmetic (`curCharPtr_`, `curCharPtr_[1]`, `curCharPtr_[2]`, etc.). It is confined
   to the lexer's cursor module, never exposed in any type crossing a function/module
   boundary, and converted pointer → offset (`SMLoc`) at every boundary. The buffer is
   held as an `Rc<SourceBuffer>` (stable heap address; no borrow fight with the
   manager). `NullTerminatedBuf`'s trailing `\0` guarantees the lookahead reads are
   in-bounds (each deeper index is only read after the previous matched a non-NUL byte).
   *(A fully-safe `usize`-offset cursor was considered and is sound given the NUL
   terminator, but we chose structural fidelity to the C++ pointer code.)*

3. **String interner — copy juno `atom_table` verbatim, add a byte/WTF-8 path
   (CONFIRMED).** `unsupported/juno/crates/juno_support/src/atom_table.rs` is copied as
   the base, retaining its encapsulated `unsafe` (`UnsafeCell` + `'static`-lifetime
   self-referential map). It is extended with a **byte intern path**: JS string literals
   can hold lone surrogates encoded as ill-formed UTF-8 (see `appendUnicodeToStorage`,
   `JSLexer.cpp`), which a Rust `String` cannot hold, so the interner must accept
   `&[u8]` (WTF-8-ish bytes) and return a handle. `UniqueString*` → an interned handle
   (an `Atom`-like index/pointer) compared by identity.

4. **Number parsing — pure Rust, no C/FFI (CONFIRMED).** The lexer's decimal/real path
   calls `fastStrToDouble` → **`fast_float`** (`lib/Support/FastStrToDouble.cpp`), *not*
   `dtoa`. Rust std's `str::parse::<f64>()` *is* the fast_float algorithm
   (Eisel–Lemire, correctly-rounded round-to-nearest-even); correct rounding has a
   unique answer, so the two are **bit-identical** on the well-formed buffer the lexer
   hands over. The integer-radix paths (hex/octal/binary, the ≤9-digit fast int path,
   bigint-digit validation) are hand-rolled Hermes code (`parseIntWithRadix*`,
   `Conversions.h`), ported directly. No `dtoa`, no FFI, no third-party crate — the
   token-dump harness proves per-literal f64-bit equality.

5. **Diagnostics — Hermes-byte-compatible (inherited).** The lexer emits through the
   `support` crate's `SourceErrorManager`, which already renders byte-compatibly with
   LLVH/`hermesc`. The lexer's job is to produce the *same messages at the same ranges*
   as the C++ lexer; its `error`/`errorRange` helpers map onto
   `manager.error`/`error_range`/`warning`.

6. **Locations — offset-based (inherited).** `SMLoc = (SourceId, u32)`. The C++ Token's
   raw-pointer `SMRange` and `UniqueString*` become an offset `SMRange` + interned
   handles. `Token::inputStr()`, `checkFollowingCharacter`, `StoredComment::getString`
   (pointer arithmetic in C++) become offset + buffer-slice operations.

## Scope — the entire `JSLexer` public surface

- **`TokenKind`** — every entry of `TokenKinds.def`: range markers (`_first_resword` …
  `_last_token`), reserved words, punctuators (incl. `PUNCTUATOR_FLOW`), binops with
  precedence, templates, ident-ops (`as`). Plus `tokenKindStr`, `isPunctuatorDbg`,
  reserved-word range predicates.
- **`Token`** — every accessor/setter: kind, range/start/end, numeric, identifier /
  private identifier / resword identifier, string literal (+ raw + contains-escapes),
  template value/raw (+ contains-not-escapes), bigint (+ raw), regexp, JSX text (+ raw),
  `isResWord`, `isTemplateLiteral`, `inputStr`, `checkFollowingCharacter`.
- **`RegExpLiteral`**, **`StoredComment`** (Line/Block/Hashbang + `getString`/
  `getFullString`), **`StoredToken`**.
- **`JSLexer`** — `advance` (all four `GrammarContext`s: `AllowRegExp`, `AllowDiv`,
  `AllowJSXIdentifier`, `Type`), `advanceInJSXChild`, `lookahead1`/`lookahead2`,
  `SavePoint`, `seek`/`forceEOF`/`getCurLoc`/`getPrevTokenEndLoc`/`setPrevTokenEndLoc`,
  `isCurrentTokenADirective`, `rescanRBraceInTemplateLiteral`,
  `isLetFollowedByDeclStart`, `isUsingFollowedByIdentifier`,
  `isAwaitUsingFollowedByIdentifier`, `convertCurTokenToIdentOp`, magic-comment URLs
  (`getSourceURL`/`getSourceMappingURL`), comment storage (`setStoreComments`,
  `getStoredComments`, `moveStoredComments`), token storage (`setStoreTokens`,
  `getStoredTokens`, `storeCurrentToken`), strict-mode, `convertSurrogates`,
  `getIdentifier`/`getStringLiteral`, buffer accessors, the constructors.
- **Internals** (ported, not public): `optimisticSkipWhitespace`, UTF-8 decode helpers,
  unicode escapes (`\uXXXX`, `\u{...}`), identifier fast path + slow path (JS/JSX/Flow
  modes), `scanReservedWord`/`matchReservedWord`, `scanNumber`, `scanString<JSX>`,
  `scanTemplateLiteral`, `scanRegExp`, `scanPrivateIdentifier`, line/block comments +
  magic comments, HTML entities (`consumeHTMLEntityOptional`), `convertSurrogatesInString`.

## Crate / file structure

```
rust/
  Cargo.toml                         # workspace (add new members)
  crates/
    support/                         # DONE — untouched, zero-unsafe
    atom_table/                      # string interner (copied juno; encapsulated unsafe)
      src/lib.rs                     #   AtomTable + Atom + AtomU16 + byte/WTF-8 path
    unicode/                         # CharacterProperties (zero-unsafe)
      src/
        lib.rs                       #   isUnicodeOnly{Letter,IDStart,IDContinue,Space}, etc.
        tables.rs                    #   ported UnicodeData.inc ranges + binary search
    parser/                          # the lexer crate
      src/
        lib.rs
        token_kinds.rs               # TokenKind enum + name/precedence tables (from .def)
        html_entities.rs             # HTMLEntities.def -> map (JSX)
        token.rs                     # Token, RegExpLiteral, StoredComment, StoredToken
        number.rs                    # scanNumber + parseIntWithRadix* (pure Rust)
        cursor.rs                    # encapsulated *const u8 cursor over Rc<SourceBuffer>
        lexer.rs                     # JSLexer: advance, identifiers, comments, lookahead, SavePoint
        string_literal.rs            # scanString / template / regexp / private ident
  tools/
    js-lexer-dump/                   # C++ token-dump harness (the differential oracle)
```

Crate split keeps the zero-`unsafe` reusable pieces (`unicode`) separate from the two
`unsafe`-bearing ones (`atom_table` copies juno's; `parser` scopes the cursor). Files
in `parser` split by responsibility (token model, number, cursor, scanning), each small
enough to hold in context.

## Dependency order (CONFIRMED)

All four deps are graph leaves feeding the lexer; order is driven by earliest
validatable slice + risk-first:

1. **Token tables** (`token_kinds.rs`, `html_entities.rs`) — zero deps, mechanical;
   defines the vocabulary everything references.
2. **C++ token-dump harness** (`tools/js-lexer-dump/`) — the differential oracle;
   independent, stood up before validating the first lexer slice.
3. **String interner** (`atom_table`) — copy juno verbatim + byte path; together with
   token tables unblocks the first real slice (punctuators + ASCII identifiers +
   reserved words + EOF, needing only inline ASCII id-helpers, no Unicode tables yet).
4. **Unicode `CharacterProperties`** (`unicode`) — bulkiest table port; sequenced after
   the validation loop is running so a misplaced range surfaces immediately.
5. **Number parsing** (`number.rs`) — most fidelity-sensitive; done last so the harness
   differential-tests every literal as it is written.

Then the lexer proper: cursor + `advance`/whitespace/comments/EOF → identifiers →
literals (strings/templates/regexp/bigint) → JSX + Flow paths → `SavePoint`/lookahead/
directive/`rescanRBrace` → full differential validation.

## Validation strategy

- **Oracle:** `tools/js-lexer-dump/` — a small standalone C++ tool linking `JSLexer`
  that, for a given source buffer, prints one line per token: `kind start-end` plus the
  decoded value for value-bearing tokens (identifier/string/number-as-f64-bits/bigint/
  regexp/template cooked+raw). Built once against the hermes tree (like the
  `SourceErrorManager` differential used `hermesc -dump-ast`).
- **Differential tests:** the Rust lexer runs the same corpus and its dump is compared
  **byte-for-byte** against the harness output. Diagnostics (lexer errors/warnings) are
  validated the same way the `support` crate already validates rendering.
- **Corpus:** punctuators, all reserved words, identifiers (ASCII, Unicode, escapes,
  JSX `-`, Flow `@`), every numeric form (decimal/hex/octal/binary/legacy-octal/bigint/
  separators/exponents/edge roundings), strings (escapes, surrogates, `convertSurrogates`
  on/off), templates (head/middle/tail, raw vs cooked, NotEscapeSequence), regexp, JSX
  text + HTML entities, comments + magic comments, directives, lookahead/savepoint paths.

## Per-subsystem design notes

- **Token tables.** `TokenKind` is a `#[repr]` enum in `.def` order so range-marker
  comparisons (`isResWord`, `_first_binary`..`_last_binary`) stay integer comparisons.
  Generate the enum, the `tokenKindStr` name table, and the binop precedence table from
  `TokenKinds.def` (a small build step or a checked-in generated file mirroring the
  `.def`). `matchReservedWord` ports the `StringSwitch` (length + char match).
- **String interner.** Copy juno `atom_table` (Atom/AtomU16/AtomTable). Add
  `intern_bytes(&[u8]) -> Atom` storing raw bytes (the WTF-8 path) alongside the
  existing `String`/`Vec<u16>` paths; the lexer's tmp/raw storage are `Vec<u8>`.
- **Unicode.** Port `lib/Platform/Unicode/CharacterProperties.cpp` + the range tables in
  `UnicodeData.inc` (sorted ranges, binary search). Pin to Hermes's Unicode version by
  porting the generated tables verbatim — do not pull a Rust unicode crate (version skew
  would break byte-compat). Inline ASCII helpers (`isASCIIIdentifierStart`, etc.) are
  trivial.
- **Number parsing.** Port `scanNumber`'s state machine exactly (radix detection,
  fraction/exponent, legacy-octal `8`/`9` redetection + warnings, separators, bigint).
  Decimal/real → clean buffer → `str::parse::<f64>()`. Integer radix → ported
  `parseIntWithRadix`. Match every error/warning message and range.
- **Lexer core.** `cursor.rs` owns the `*const u8` triple (`bufferStart_`,
  `curCharPtr_`, `bufferEnd_`) over an `Rc<SourceBuffer>`; exposes only safe ops
  returning bytes/offsets. `lexer.rs` ports `advance` (the `PUNC_L*` punctuator macros
  become helper fns/match arms), whitespace/comment skipping, identifier fast/slow
  paths, `SavePoint` (a plain struct snapshot — no RAII needed), and `lookahead1/2`
  (save/restore cursor). `Token` stores `SMRange` + `Atom` handles.
```
