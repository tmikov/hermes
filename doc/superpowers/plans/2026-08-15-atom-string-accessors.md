# Atom → String Accessors Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the first-contact friction the external crate review found: make "get an identifier's name as a string" obvious, without letting anyone silently corrupt a string literal that holds a lone surrogate.

**Architecture:** Two accessors on `AtomTable` (mirrored on `GCLock`), validating on every call with no cache; a `HashMap` behind the error arm purely as a lifetime anchor for lossy replacements; and generated per-field node methods whose *shape differs by field kind* — plain for labels, `try_`/`_lossy` for strings — because the type system cannot express that distinction (`NodeLabel` and `NodeString` are both `AtomBytes`).

**Tech Stack:** Rust (`hermes-atom-table`, `hermes-ast`, `hermes-parser`, `hermes-sema`), the `gen_nodes.py` node generator.

**THE SPEC IS AUTHORITATIVE:** `doc/superpowers/specs/2026-08-15-atom-string-accessors-design.md`. Read it before Task 1. Its §2 (why the bytes are bytes) and §5 (why the two field kinds get different shapes) are the reasoning behind decisions that will look arbitrary otherwise.

## Global Constraints

- **NEVER `cd`.** `git -C /home/tmikov/work/hermes-rust …`, `cargo --manifest-path /home/tmikov/work/hermes-rust/rust/Cargo.toml …`, absolute paths. Branch `rust`.
- **Additive only.** No existing signature changes, no behavior changes to `bytes()`. This ships as **0.1.1**.
- **`missing_docs` is on** in the published crates: every new public item needs an accurate doc comment. Doc accuracy is treated as a defect class in this project — verify claims against code, don't invent rationale.
- **Juno divergence is free** — `atom_table` is a copy of an unmaintained reference. Never weigh it as a cost.
- Zero warnings in both feature configs and under `RUSTFLAGS="-D warnings"`; per-file **no NEW** rustfmt diff hunks.
- Gates that must stay green: sema **224 (111)** + parser-entry **17 (9)**; parser 8/8; json 1/1; preparse 4/4; lexer 6/6; full workspace; `cargo publish --dry-run` all seven in ONE call; the citation check (3183, clean).
- `generated_idempotent` guards `node.rs` — regenerate via the generator, never hand-edit `// @generated` output.
- Commit trailers:
  `Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>` and
  `Claude-Session: https://claude.ai/code/session_01ERsmFoVAnZCRwfapbPMibv`

---

### Task 1: The two accessors on `AtomTable` and `GCLock`

**Files:**
- Modify: `rust/crates/atom_table/src/lib.rs`
- Modify: `rust/crates/ast/src/context.rs` (the `GCLock` delegation)
- Test: `rust/crates/atom_table/src/lib.rs` unit tests

**Interfaces:**
- Consumes: `hermes_support::utf8`'s surrogate-aware decoder (see Step 2 — confirm the exact entry point before using it; `decode_utf8<ALLOW_SURROGATES>` and `convert_utf8_with_surrogates_to_utf16` both exist).
- Produces: `AtomTable::{bytes_str_lossy, try_bytes_str}` and the same two on `GCLock`.

- [ ] **Step 1: Write the failing tests first**

In `atom_table`'s tests. These must construct atoms **from raw bytes**, because no parseable JS reaches the lossy path through an identifier:

```rust
#[test]
fn lone_surrogate_becomes_exactly_one_replacement_char() {
    let t = AtomTable::new();
    // WTF-8 for U+D800, i.e. `"\uD800"` as Hermes stores it.
    let a = t.atom_bytes(vec![0xED, 0xA0, 0x80]);
    assert_eq!(t.try_bytes_str(a), None);
    let s = t.bytes_str_lossy(a);
    assert_eq!(s.chars().filter(|c| *c == '\u{FFFD}').count(), 1,
               "std::from_utf8_lossy would give 3 here; we must be WTF-8 aware");
    assert_eq!(s, "\u{FFFD}");
}

#[test]
fn valid_utf8_is_borrowed_unchanged() {
    let t = AtomTable::new();
    let a = t.atom_bytes("greet".as_bytes().to_vec());
    assert_eq!(t.try_bytes_str(a), Some("greet"));
    assert_eq!(t.bytes_str_lossy(a), "greet");
    // Zero-copy: the returned str points into the table's own bytes.
    assert_eq!(t.bytes_str_lossy(a).as_ptr(), t.bytes(a).as_ptr());
}

#[test]
fn surrogates_mixed_with_text_replace_only_the_surrogate() {
    let t = AtomTable::new();
    let mut v = b"a".to_vec();
    v.extend_from_slice(&[0xED, 0xA0, 0x80]);
    v.extend_from_slice("b".as_bytes());
    let a = t.atom_bytes(v);
    assert_eq!(t.bytes_str_lossy(a), "a\u{FFFD}b");
}

#[test]
fn the_lossy_result_is_stable_across_calls() {
    let t = AtomTable::new();
    let a = t.atom_bytes(vec![0xED, 0xA0, 0x80]);
    let p1 = t.bytes_str_lossy(a).as_ptr();
    // Interning more atoms must not invalidate an earlier result.
    for i in 0..1000 { t.atom_bytes(format!("filler{i}").into_bytes()); }
    let p2 = t.bytes_str_lossy(a).as_ptr();
    assert_eq!(p1, p2, "the anchored String must not be rebuilt or moved");
}
```

- [ ] **Step 2: Run them; confirm they fail to compile**

`cargo test --manifest-path /home/tmikov/work/hermes-rust/rust/Cargo.toml -p hermes-atom-table`
Expected: the methods don't exist.

Before implementing, **read `hermes_support::utf8`** and decide the conversion route. Note `atom_table` may not currently depend on `hermes-support` — if adding that dependency creates a cycle or is otherwise undesirable, implement the WTF-8 decode locally in `atom_table` (it is small) and say which you chose and why. **Do not use `String::from_utf8_lossy`** — spec §4 explains why (three `U+FFFD` per surrogate).

- [ ] **Step 3: Implement**

Add to `Inner`, beside the existing maps, with a doc comment matching the house style of its neighbours:

```rust
/// Lossy UTF-8 renderings of byte atoms that are not valid UTF-8, built on
/// demand. This is a lifetime anchor, not a cache: `bytes_str_lossy` needs
/// somewhere to own the replacement string so it can hand out a `&str`. It
/// is empty for every input that comes from parsing, because the lexer
/// rejects unpaired surrogates in identifiers; only string literals and
/// hand-built atoms can reach it. Entries are never removed or mutated, so
/// a returned `&str` stays valid (rehashing moves the `String` structs,
/// never their heap buffers) — the same argument as `strings_bytes`.
lossy_bytes: HashMap<AtomBytes, String>,
```

and the two public methods on `AtomTable`, delegating through the `UnsafeCell` exactly as the existing accessors do. The valid path must not touch the map.

- [ ] **Step 4: Mirror on `GCLock`**

`gc.bytes_str_lossy(a)` / `gc.try_bytes_str(a)` delegating to the table, so callers never write `gc.ctx().atom_table()`. Match the existing `bytes()` delegation.

- [ ] **Step 5: Verify**

The four tests pass; workspace green; zero warnings both configs.

- [ ] **Step 6: Commit** — `rust(atom-table): add bytes_str_lossy and try_bytes_str`

---

### Task 2: Generated per-field node methods

**Files:**
- Modify: `rust/crates/ast/gen_nodes.py`
- Regenerate: `rust/crates/ast/src/node.rs`
- Test: `rust/crates/ast/tests/` (a new test, or extend an existing one)

**Interfaces:**
- Consumes: Task 1's `GCLock` methods.
- Produces: `<field>_str` on the 32 `NodeLabel` fields; `try_<field>_str` + `<field>_str_lossy` on the 10 `NodeString` fields.

- [ ] **Step 1: Confirm the field inventory before generating**

The generator already distinguishes the two types (`gen_nodes.py` ~line 189-200). Print the actual per-type field list and counts and put them in your report — the spec says 32 and 10, but **verify**; a miscount means a missing or spurious accessor.

- [ ] **Step 2: Emit the label accessors**

For each `NodeLabel` field, in the node's `impl` block beside the existing `kind()`/`range()` accessors:

```rust
/// The `<field>` label as UTF-8.
///
/// Identifier names are always valid UTF-8 in practice — the lexer rejects
/// unpaired surrogates in identifiers — so this borrows the atom's own bytes
/// and allocates nothing. Anything unrepresentable is rendered as U+FFFD,
/// which indicates a hand-built AST rather than a parsed one. Use
/// [`GCLock::bytes`] for the exact bytes.
pub fn <field>_str<'a>(&self, gc: &'a GCLock) -> &'a str {
    gc.bytes_str_lossy(self.<field>.get())
}
```

Mind the lifetime: the returned `&str` borrows from `gc`, **not** from `self`.

- [ ] **Step 3: Emit the string accessors**

For each `NodeString` field, emit **both**, and **no** plain `<field>_str`:

```rust
/// The `<field>` string value as UTF-8, or `None` if it is not representable.
///
/// A JS string value is a sequence of UTF-16 code units and may legally
/// contain unpaired surrogates (`"\uD800"` parses), which have no UTF-8
/// form. `None` means exactly that — the value is intact, it simply cannot
/// be handed back as `&str`. Use [`GCLock::bytes`] to read it losslessly.
pub fn try_<field>_str<'a>(&self, gc: &'a GCLock) -> Option<&'a str>

/// The `<field>` string value as UTF-8, substituting U+FFFD for anything
/// unrepresentable.
///
/// **This is lossy.** Do not use it to re-emit source: a value containing an
/// unpaired surrogate will be silently altered. Use [`Self::try_<field>_str`]
/// or [`GCLock::bytes`] when the exact value matters.
pub fn <field>_str_lossy<'a>(&self, gc: &'a GCLock) -> &'a str
```

- [ ] **Step 4: Regenerate and prove idempotency**

```bash
python3 /home/tmikov/work/hermes-rust/rust/crates/ast/gen_nodes.py
REQUIRE_GEN=1 cargo test --manifest-path /home/tmikov/work/hermes-rust/rust/Cargo.toml -p hermes-ast --test generated_idempotent
```

- [ ] **Step 5: Test the generated surface**

A test that parses real source and reads a name through the generated method (`id.name_str(gc) == "greet"`); one that reads a string literal both ways; and one asserting a `"\uD800"` literal gives `try_value_str() == None` while `bytes()` round-trips the WTF-8 unchanged — the guarantee a codegen tool depends on (spec §8).

- [ ] **Step 6: Verify + commit**

All gates. `rust(ast): generate per-field string accessors for labels and string values`

---

### Task 3: The review's remaining defects, the example, and the docs

**Files:**
- Modify: `rust/crates/parser/examples/parse_to_estree_json.rs`
- Modify: `rust/crates/parser/src/facade.rs` (the `messages()` doc, the `&mut self` docs)
- Create: `rust/crates/sema/examples/print_bindings.rs`
- Modify: `rust/crates/parser/src/lib.rs`, `rust/crates/sema/src/lib.rs` (quickstarts)
- Modify: `rust/README.md`, `rust/crates/{parser,sema}/README.md` as needed
- Modify: `rust/CHANGELOG.md`

**Interfaces:**
- Consumes: Tasks 1 and 2.
- Produces: the 0.1.1 documentation surface.

- [ ] **Step 1: Fix the `eprintln!` bug**

`parse_to_estree_json.rs:51` uses `eprintln!("{m}")`, but `messages()` strings are already newline-terminated — verified with `cat -A`, which shows a trailing blank line per diagnostic. Change to `eprint!` (as `resolve_and_dump.rs` correctly does) and **state the trailing newline in `messages()`'s doc comment**, since the bug was caused by its absence.

- [ ] **Step 2: Document why `&mut self`**

`ParsedJS::with_program` / `to_estree_json` (and the `ResolvedJS` equivalents) take `&mut self` for logically read-only access. It is forced by `Context::lock(&mut self)`. One sentence at each, so the "why can't I share this?" question is answered where it is asked.

- [ ] **Step 3: Write `print_bindings.rs`**

The canonical use, and the example whose absence caused the review's #1 and #2: read a path from argv, parse, resolve, walk with `Visitor`, and print each identifier with its resolved binding kind (`counter -> Let`, `console -> UndeclaredGlobalProperty`). It must:
- use the new `name_str` accessor, so the atom→string path is demonstrated;
- store `&GCLock` in the visitor struct, **demonstrating the variance pattern** — keep the lock's lifetimes independent of the visitor's `'gc`, and put a comment at that exact spot explaining why the naive tying fails (`GCLock<'ast, 'ctx>` is invariant). This is defect #2 and a comment is the deliverable, not an afterthought.

Run it and put its real output in your report.

- [ ] **Step 4: Quickstarts**

Add the atom→string line to both crates' `//!` quickstarts, pointing at the new accessors and at `bytes()` for exactness. Keep them doctests; they must run under `cargo test --doc`.

- [ ] **Step 5: CHANGELOG**

A `[0.1.1]` section: the new accessors, the example, the `eprintln!` fix, and the doc additions. Note the release is additive.

- [ ] **Step 6: Verify + commit**

All gates including doctests and `cargo publish --dry-run` (7 crates, one call). `doc(rust): 0.1.1 — string accessors, print_bindings example, review fixes`

---

### Task 4: Release 0.1.1

**Files:** the seven `Cargo.toml` version fields; `doc/superpowers/PUBLISH-HANDOFF.md`.

**Interfaces:** consumes Tasks 1-3. Produces a tagged, dry-run-verified 0.1.1. **Publishing itself is the user's manual step — do NOT run `cargo publish` without `--dry-run`.**

- [ ] **Step 1: Decide the version bump per crate**

Only crates whose content changed need a bump, but a mixed-version family is confusing. Recommend bumping all seven to 0.1.1 for a coherent release; **state your recommendation and reasoning in the report and let the controller confirm** rather than deciding unilaterally. Inter-crate dependency versions must move with it.

- [ ] **Step 2: Dry-run**

`cargo publish --dry-run --manifest-path … -p hermes-unicode -p hermes-atom-table -p hermes-command-line -p hermes-support -p hermes-ast -p hermes-parser -p hermes-sema` — all seven pack and verify.

- [ ] **Step 3: Update the runbook**

`PUBLISH-HANDOFF.md`: record that 0.1.0 is published, that the new-crate rate limit cost two ten-minute waits (and does **not** apply to updates of existing crates, so 0.1.1 should publish in one call), and the tag convention for the next release.

- [ ] **Step 4: Commit** — `rust: 0.1.1`

---

## Self-Review

- Spec §3 (no cache), §4 (anchor + WTF-8-aware), §5 (the two API shapes) are each carried into a task with their reasoning attached, so an implementer does not "simplify" them back.
- The lossy path is unreachable from parsed source, so Task 1 Step 1 builds atoms from raw bytes — otherwise that branch ships untested.
- Task 2 Step 1 re-counts the fields rather than trusting the spec's 32/10.
- Task 4 stops at the dry-run; the irreversible step stays with the user.
