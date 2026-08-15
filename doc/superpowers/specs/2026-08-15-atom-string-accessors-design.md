# Atom → string accessors: design

**Date:** 2026-08-15. **Motivated by:** an external usability review of the
published `hermes-parser` / `hermes-sema` 0.1.0 crates
(`CRATE-REVIEW.md`), whose single biggest first-contact complaint was that
getting an identifier's name as a string is undocumented and non-obvious.

**Target release:** 0.1.1 (additive only — no existing signature changes).

## 1. The problem

`Identifier.name` is a `Cell<NodeLabel>`, and `NodeLabel` is
`hermes_atom_table::AtomBytes` — an index into a table whose contents "need not
be valid UTF-8". Turning a name into a printable string today requires knowing
to write `id.name.get()` and then `gc.bytes(atom)`, yielding `&[u8]`. Nothing in
either crate's quickstart, README, or three shipped examples ever does it;
`walk_ast.rs` counts node *kinds* specifically to avoid it. Meanwhile rustc's
error on the naive attempt suggests `Cell::as_ptr`, a red herring.

"Walk the tree and print the names" is the canonical use of these crates, and it
is the one thing they do not demonstrate.

## 2. Why the bytes are bytes

Identifiers cannot contain unpaired surrogates: Hermes rejects them in the
lexer, with dedicated diagnostics (verified 2026-08-15 —
`var \uD800;` → "Unicode escape \ud800 is not a valid identifier start";
`var a\uD800;` → "…not a valid identifier codepoint"). So for identifier names
the bytes are always well-formed UTF-8 in practice.

The byte orientation comes from elsewhere: `NodeLabel` and `NodeString` are
**the same underlying type** (`AtomBytes`), spanning 32 label fields and 10
string fields, and a JS string literal genuinely can hold a lone surrogate
(`var s = "\uD800";` parses and dumps as `"value": "\ud800"`).

> **Correction (2026-08-15, found during implementation).** An earlier draft of
> this spec claimed the non-UTF-8 path "is empty for every input that comes
> from parsing". **That is false.** The lexer interns astral characters in
> string literals as WTF-8 **surrogate pairs** rather than 4-byte UTF-8
> (`crates/parser/src/lexer/mod.rs:1364-1375`), so every emoji literal reaches
> it. This is not an error condition — a surrogate pair and the character it
> encodes are two spellings of the same string — so both accessors **fold
> pairs back** into the character, matching C++ `convertToCodePointAt`
> (`UTF8.cpp:77-96`). Only an **unpaired** surrogate is unrepresentable.
> Consequently the map below is populated by ordinary input, and the
> exact/inexact distinction (not mere UTF-8 validity) is what `try_*` reports.

**This asymmetry is the core of the design** and drives §5:

- For an **identifier**, invalid UTF-8 means something is wrong — a bug or a
  hand-built AST. Substituting `U+FFFD` is a diagnostic aid for a state that
  should not exist.
- For a **string literal**, un-representable content is *normal and meaningful*.
  A lone surrogate is a legal JS string value, not malformed data. Silently
  substituting `U+FFFD` there is data corruption: a codegen or refactoring tool
  that round-trips through such an accessor would rewrite the user's program
  and never know.

## 3. No validity cache

The initial sketch had a lazily-populated per-atom validity bitset. Measurement
killed it: `std::str::from_utf8` over a realistic identifier mix costs
**10.1 ns/call** (measured with `black_box` on the input, so an inlined real
call is faster). A million-identifier tree walk spends ~10 ms validating —
against printing costs that dwarf it.

So: **validate on every call.** No bitset, no tri-state, no per-atom overhead,
no lazily-populated cache. Validating first is also the right order on its own
merits — 10 ns of SIMD validation beats hashing a key and probing a table.

## 4. The invalid path needs a lifetime anchor, not a cache

On the valid path there is no storage at all: `from_utf8` hands back a `&str`
borrowed straight from the atom's own bytes, zero-copy and already
lifetime-correct.

The lossy replacement string, being newly constructed, cannot be returned by
reference from a temporary. Something must own it. Therefore `Inner` gains:

```rust
lossy_bytes: HashMap<AtomBytes, String>,
```

consulted **only in the `Err` arm** — i.e. never on the valid-UTF-8 fast
path, which stays zero-copy. Contrary to an earlier draft it is *not* empty in
practice: every emoji string literal anchors one folded entry (see §2's
correction). Its
soundness is the house pattern already documented for `strings_bytes`:
rehashing moves the `String` structs, never their heap buffers, so a previously
returned `&str` stays valid.

**Lossy conversion must be WTF-8-aware.** `String::from_utf8_lossy` is not: a
lone surrogate is `ED A0 80`, which std replaces as up to three separate invalid
runs, yielding three `U+FFFD` for one character. Use the existing
surrogate-aware decoder in `hermes_support::utf8` and emit exactly one `U+FFFD`
per unpaired surrogate.

## 5. API

### 5.1 `AtomTable` / `GCLock` (shared, since both node kinds are `AtomBytes`)

```rust
/// The atom's bytes as UTF-8, substituting U+FFFD for anything unrepresentable.
pub fn bytes_str_lossy(&self, a: AtomBytes) -> &str;
/// The atom's bytes as UTF-8, or None if they contain an UNPAIRED surrogate.
///
/// Surrogate PAIRS are folded to the character they encode and return
/// `Some` — they are representable, and reporting `None` for an emoji
/// would answer a question about byte encoding rather than about the
/// value, which is not what a caller is asking.
pub fn try_bytes_str(&self, a: AtomBytes) -> Option<&str>;
```

`bytes()` is unchanged and remains the exact-bytes accessor. `GCLock` re-exports
both by delegation, so `gc.bytes_str_lossy(a)` works without reaching for
`gc.ctx().atom_table()`.

### 5.2 Node methods (generated)

The semantic split lives here, because the type system cannot express it.

For each of the 32 **`NodeLabel`** fields — identifiers, operators, kinds:

```rust
pub fn name_str(&self, gc: &GCLock) -> &str          // <field>_str
```

Plain, ergonomic, lossy fallback unreachable in practice. Documented as such,
with a pointer to `bytes()` for callers who need the exact bytes.

For each of the 10 **`NodeString`** fields — string-literal values, cooked
template elements:

```rust
pub fn try_value_str(&self, gc: &GCLock) -> Option<&str>   // try_<field>_str
pub fn value_str_lossy(&self, gc: &GCLock) -> &str         // <field>_str_lossy
```

No plain `value_str()`. A tool author who writes `try_value_str()` and gets an
`Option` will handle it; one handed a silently-substituted `&str` will not
notice until a user reports mangled output. The `_lossy` suffix makes the
destructive choice explicit at the call site.

Generation happens in `gen_nodes.py`, which already knows each field's type, and
is covered by the existing `generated_idempotent` guard.

## 6. Also in scope (the rest of the review)

1. **Bug — `parse_to_estree_json.rs` prints a spurious blank line per
   diagnostic.** It uses `eprintln!("{m}")`, but `messages()` strings are
   already newline-terminated (verified with `cat -A`). Fix the example to
   `eprint!`, and state the trailing newline in `messages()`'s doc comment.
2. **A worked example.** Add `print_bindings.rs` walking a tree and printing
   each identifier with its resolved binding kind — the canonical use, and the
   thing that would have prevented this review's #1 and #2. It also documents
   the `Visitor` + `&GCLock` variance pattern: keep the lock's lifetimes
   independent of the visitor's `'gc`.
3. **Docs.** Say why `with_program`/`to_estree_json` take `&mut self` (forced by
   `Context::lock`), and add the atom→string path to both crates' quickstarts.

## 7. Out of scope

- Any change to `bytes()` or to the WTF-8 representation.
- A validity cache (§3).
- `Cow<str>` returns — rejected: exposes an internal detail and degrades every
  call site for a case that never occurs.
- Repairing the known citation debt, which is unrelated.

## 8. Verification

- The lossy path needs a test with real WTF-8 input, since no parseable source
  reaches it via identifiers: build an atom from bytes directly and assert one
  `U+FFFD` per surrogate, not three.
- `try_*_str` must return `None` (not a substitution) for the same input.
- A parsed `"\uD800"` string literal must round-trip through `bytes()` unchanged
  — the guarantee a codegen tool depends on.
- The new example runs; the doctests compile; gates unmoved.
