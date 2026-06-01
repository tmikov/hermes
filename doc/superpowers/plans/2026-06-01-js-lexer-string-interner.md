# JS Lexer — String Interner (subsystem ③) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Provide the lexer's string-interning table (`StringTable`/`UniqueString` equivalent) by copying juno's `atom_table` into a new `atom_table` crate and adding a **byte / WTF-8 intern path** — because JS string literals can hold lone surrogates encoded as ill-formed UTF-8 that a Rust `String` cannot store.

**Architecture:** A new `atom_table` crate (mirroring juno's crate name) under the `rust/` workspace. It is a verbatim copy of `unsupported/juno/crates/juno_support/src/atom_table.rs` — keeping its encapsulated `unsafe` (interior mutability via `UnsafeCell`, plus the append-only `Vec` + `HashMap<&'static …>` self-referential map whose `'static` is upheld by never removing/mutating entries) — with two deliberate departures: (1) the juno `HeapSize` impls are dropped (HeapSize is a separate juno memory-accounting component the lexer doesn't need), and (2) a third intern path over raw bytes (`Vec<Vec<u8>>` + `HashMap<&'static [u8], u32>`) is added, yielding `AtomBytes` handles. The lexer will intern all identifiers and string literals through the byte path so a single table backs reserved-word/identifier identity comparison.

**Tech Stack:** Rust (edition 2021), std only. This crate **carries encapsulated `unsafe`** (the one interner crate that does), so it does NOT set `unsafe_code = "forbid"`; the unsafe is confined here and the safety invariants are documented in-code (copied from juno).

**Reference spec:** `doc/superpowers/specs/2026-06-01-js-lexer-design.md` (decision 3).
**Source of truth to copy:** `unsupported/juno/crates/juno_support/src/atom_table.rs` (read in full).
**C++ analog (for intent):** `include/hermes/Support/StringTable.h` (`StringTable`/`UniqueString`: intern bytes, compare by identity).

**Porting rule:** copy juno's file faithfully, keeping its comments. The byte path is a structural clone of juno's existing `u16` path (`Vec<Vec<u16>>` + `map_u16`), which already proves the same pattern: pushing an owned `Vec` into the outer `Vec` moves only the `Vec` struct, never its heap buffer, so the captured `*const [_]` key stays valid.

**Do NOT** `cd` out of the project root.

---

## File structure

```
rust/
  Cargo.toml                       # workspace — add "crates/atom_table"
  crates/
    atom_table/
      Cargo.toml                   # new crate, edition 2021, no deps (unsafe NOT forbidden)
      src/lib.rs                   # copied atom_table (minus HeapSize) + byte/WTF-8 path
```

---

## Task 0: Crate scaffold + verbatim copy (minus HeapSize)

**Files:**
- Modify: `rust/Cargo.toml` (members)
- Create: `rust/crates/atom_table/Cargo.toml`
- Create: `rust/crates/atom_table/src/lib.rs`

- [ ] **Step 1: Add to workspace.** Edit `rust/Cargo.toml` members to
  `["crates/support", "crates/parser", "crates/atom_table"]`.

- [ ] **Step 2: Create `rust/crates/atom_table/Cargo.toml`**

```toml
[package]
name = "atom_table"
version = "0.0.0"
edition = "2021"
publish = false

# This crate intentionally carries encapsulated `unsafe` (interior mutability +
# an append-only self-referential string map copied from juno). It is the one
# interner crate permitted to use unsafe; the invariants are documented inline.

[dependencies]
```

- [ ] **Step 3: Create `rust/crates/atom_table/src/lib.rs` as a verbatim copy of
  `unsupported/juno/crates/juno_support/src/atom_table.rs`, with exactly these edits:**
  - Add a crate-level `//!` doc comment: ported from juno `atom_table` for the Hermes Rust
    lexer; carries encapsulated unsafe; adds a byte/WTF-8 intern path (Task 1).
  - Remove `use crate::HeapSize;`.
  - Remove the `impl HeapSize for Inner { … }` block and the `impl HeapSize for AtomTable { … }`
    block (the only uses of the external juno dep). Keep EVERYTHING else verbatim, including
    the `String` path, the `u16` path, `INVALID_ATOM`, the `Debug` impls, the `DEBUG_TABLE`
    thread-local, `in_debug_context`, `unsafe_set_debug_context`, the `Index` impls, and the
    `#[cfg(test)] mod tests { … test_tab … }`.

- [ ] **Step 4: Build & test** (juno's own `test_tab` must pass unchanged)

Run: `cargo test --manifest-path rust/Cargo.toml -p atom_table`
Expected: compiles; `test_tab` passes. Zero warnings:
`cargo build --manifest-path rust/Cargo.toml -p atom_table 2>&1 | grep -i warn` → no output.

- [ ] **Step 5: Commit**

```bash
git add rust/Cargo.toml rust/crates/atom_table/Cargo.toml rust/crates/atom_table/src/lib.rs
git commit -m "rust(atom_table): copy juno atom_table verbatim (minus HeapSize)"
```

---

## Task 1: Byte / WTF-8 intern path

**Files:** Modify `rust/crates/atom_table/src/lib.rs`.

Add a third path mirroring the `u16` path exactly, but over `u8`, plus an `AtomBytes` handle.

- [ ] **Step 1: Write the failing tests** (append to `mod tests`)

```rust
    #[test]
    fn test_bytes() {
        let tab = AtomTable::new();

        let foo = tab.atom_bytes(b"foo".as_slice());
        let bar = tab.atom_bytes(b"bar".as_slice());
        assert_ne!(foo, bar);

        // Same bytes -> same atom (dedup).
        assert_eq!(tab.atom_bytes(b"foo".as_slice()), foo);
        assert_eq!(tab.atom_bytes(Vec::from(*b"bar")), bar);

        // Round-trip the raw bytes.
        assert_eq!(tab.bytes(foo), b"foo");
        assert_eq!(&tab[bar], b"bar");

        // Stable heap address: re-interning does not reallocate the stored bytes.
        let p_foo: *const [u8] = tab.bytes(foo);
        let _ = tab.atom_bytes(b"baz".as_slice());
        assert_eq!(tab.bytes(foo) as *const [u8], p_foo);
    }

    #[test]
    fn test_bytes_ill_formed_utf8() {
        let tab = AtomTable::new();
        // Lone high surrogate U+D800 encoded as WTF-8 (3 bytes) — NOT valid UTF-8,
        // so it could not be stored via the `String` path.
        let lone_surrogate: &[u8] = &[0xed, 0xa0, 0x80];
        let a = tab.atom_bytes(lone_surrogate);
        assert_eq!(tab.bytes(a), lone_surrogate);
        assert_eq!(tab.atom_bytes(lone_surrogate), a); // dedup on raw bytes

        // The byte table is independent of the str table: same logical text via the
        // String path is a different atom space.
        let s = tab.atom("foo");
        let b = tab.atom_bytes(b"foo".as_slice());
        // Different handle types; both round-trip their own value.
        assert_eq!(tab.str(s), "foo");
        assert_eq!(tab.bytes(b), b"foo");
    }

    #[test]
    fn test_bytes_try_and_invalid() {
        let tab = AtomTable::new();
        let a = tab.atom_bytes(b"x".as_slice());
        assert_eq!(tab.try_bytes(a), Some(b"x".as_slice()));
        assert_eq!(tab.try_bytes(INVALID_ATOM_BYTES), None);
    }
```

- [ ] **Step 2: Run — expect FAIL** (`atom_bytes`/`bytes`/`AtomBytes` undefined)

Run: `cargo test --manifest-path rust/Cargo.toml -p atom_table -- test_bytes`
Expected: FAIL (undefined items).

- [ ] **Step 3: Implement the byte path**

In `Inner`, add the fields (next to `strings_u16`/`map_u16`):

```rust
    /// Strings are added here and never removed or mutated.
    strings_bytes: Vec<Vec<u8>>,
    /// Maps from a reference inside [`Inner::strings_bytes`] to the index in
    /// [`Inner::strings_bytes`]. Since byte strings are never removed or modified,
    /// the lifetime of the key is effectively static. Unlike the `String` path,
    /// these bytes need not be valid UTF-8 (JS string literals may contain lone
    /// surrogates encoded as ill-formed UTF-8).
    map_bytes: HashMap<&'static [u8], NumIndex>,
```

Add the handle + invalid sentinel (next to `Atom`/`AtomU16` and `INVALID_ATOM`):

```rust
/// This represents a unique byte-string index in the table.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct AtomBytes(NumIndex);

/// A special value reserved for the invalid byte atom.
pub const INVALID_ATOM_BYTES: AtomBytes = AtomBytes(NumIndex::MAX);
```

Add the `Inner` methods (mirroring `add_atom_u16`/`add_u16`/`str_u16`/`try_str_u16`):

```rust
    /// Add a byte string to the table and return its atom index. The same bytes
    /// always return the same index. The bytes need not be valid UTF-8.
    fn add_atom_bytes<V: Into<Vec<u8>> + AsRef<[u8]>>(&mut self, value: V) -> AtomBytes {
        if let Some(index) = self.map_bytes.get(value.as_ref()) {
            return AtomBytes(*index);
        }
        self.add_bytes(value.into())
    }

    /// Perform the actual addition of the owned bytes.
    fn add_bytes(&mut self, owned: Vec<u8>) -> AtomBytes {
        // Remember the index of the new element.
        let index = self.strings_bytes.len();
        assert!(index < INVALID_ATOM_BYTES.0 as usize, "More than 4GB atoms?");

        // Obtain a reference to the existing bytes on the heap. That reference is
        // valid while `self` is valid: pushing the owned Vec moves only the Vec
        // struct, never its heap buffer.
        let key: *const [u8] = owned.as_slice();

        // Push the new bytes.
        self.strings_bytes.push(owned);

        self.map_bytes.insert(unsafe { &*key }, index as NumIndex);
        AtomBytes(index as NumIndex)
    }

    /// Return the contents of the specified byte atom.
    #[inline]
    fn bytes(&self, ident: AtomBytes) -> &[u8] {
        self.strings_bytes[ident.0 as usize].as_slice()
    }

    fn try_bytes(&self, ident: AtomBytes) -> Option<&[u8]> {
        if (ident.0 as usize) < self.strings_bytes.len() {
            Some(self.bytes(ident))
        } else {
            None
        }
    }
```

Add the `AtomTable` public methods (mirroring `atom_u16`/`str_u16`/`try_str_u16`):

```rust
    /// Add a byte string to the table and return its atom index. The same bytes
    /// always return the same index. The bytes need not be valid UTF-8.
    pub fn atom_bytes<V: Into<Vec<u8>> + AsRef<[u8]>>(&self, value: V) -> AtomBytes {
        unsafe { &mut *self.0.get() }.add_atom_bytes(value)
    }

    /// Return the contents of the specified byte atom.
    #[inline]
    pub fn bytes(&self, ident: AtomBytes) -> &[u8] {
        unsafe { &*self.0.get() }.bytes(ident)
    }

    #[inline]
    pub fn try_bytes(&self, ident: AtomBytes) -> Option<&[u8]> {
        unsafe { &*self.0.get() }.try_bytes(ident)
    }
```

Add the `Index<AtomBytes>` impl (mirroring `Index<AtomU16>`):

```rust
impl std::ops::Index<AtomBytes> for AtomTable {
    type Output = [u8];

    fn index(&self, index: AtomBytes) -> &Self::Output {
        self.bytes(index)
    }
}
```

Add a `Debug` impl for `AtomBytes` mirroring the others, but printing the bytes (use the
table's `try_bytes`):

```rust
// An implementation of Debug which optionally obtains the AtomBytes value from
// the active debug map.
impl std::fmt::Debug for AtomBytes {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut t = f.debug_tuple("AtomBytes");
        t.field(&self.0);
        DEBUG_TABLE.with(|debug_table| {
            let p = debug_table.get();
            if let Some(r) = unsafe { p.as_ref() } {
                if let Some(value) = r.try_bytes(*self) {
                    t.field(&value);
                }
            }
        });
        t.finish()
    }
}
```

- [ ] **Step 4: Run — expect PASS**

Run: `cargo test --manifest-path rust/Cargo.toml -p atom_table`
Expected: PASS (`test_tab`, `test_bytes`, `test_bytes_ill_formed_utf8`,
`test_bytes_try_and_invalid`). Zero warnings.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/atom_table/src/lib.rs
git commit -m "rust(atom_table): add byte/WTF-8 intern path (AtomBytes)"
```

---

## Self-review checklist (after Task 1)

- [ ] `lib.rs` is juno's `atom_table` verbatim except: crate doc added, `HeapSize` use +
  impls removed, byte path added. Diff against the juno source to confirm nothing else drifted.
- [ ] Byte path is a faithful clone of the `u16` path (same unsafe pattern, same comments
  adapted); ill-formed-UTF-8 bytes intern and round-trip; dedup works on raw bytes.
- [ ] `AtomBytes`, `INVALID_ATOM_BYTES`, `atom_bytes`, `bytes`, `try_bytes`,
  `Index<AtomBytes>`, `Debug for AtomBytes` all present and consistent with the existing
  `u16` naming.
- [ ] `unsafe` is confined to this crate and documented; no `unsafe` leaks in the public
  API (handles are opaque `u32` indices; accessors return safe `&[u8]`/`&str`).
- [ ] Zero warnings; all tests pass.

## Next subsystem

After this lands: subsystem ④ Unicode `CharacterProperties` (port `UnicodeData.inc` ranges
+ lookups), then ⑤ number parsing, then the lexer proper. See
`doc/superpowers/RustPortRoadmap.md`.
