# SourceErrorManager Rust Port — Implementation Plan

> **STATUS: COMPLETE (all tasks done).** Tasks T0–T10 below were implemented, plus a
> follow-on pass that finished the *entire* component — message buffering/coalescing,
> ranged diagnostics, subsystem suppression, external message collection, the remaining
> find/convert/dump helpers — and a live **byte-for-byte differential against `hermesc` 1.96.0**
> (`rust/crates/support/tests/golden.rs`). The follow-on plan lived in a transient session
> file; its design rationale is captured in `doc/superpowers/RustPortRoadmap.md`. Nothing in
> `SourceErrorManager` remains deferred. This plan is the historical record of the build.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port Hermes's `SourceErrorManager` and its buffer/source-location machinery to a zero-`unsafe` Rust crate. The full public surface is implemented (the original plan staged message buffering/coalescing after the T0–T10 core; it and the other follow-on features are now done — see the status note above).

**Architecture:** A new `support` crate under a `rust/` cargo workspace. Locations are offset-based `(SourceId, u32)` with explicit buffer identity. The manager owns buffers as `Rc<SourceBuffer>`, resolves offsets to line/col via a cached per-buffer line index, and dispatches resolved diagnostics to a pluggable `DiagHandler` (default handler is byte-compatible with LLVH `SMDiagnostic` rendering). No `unsafe` anywhere in this crate.

**Tech Stack:** Rust (edition 2021), cargo workspace, std only (`Rc`, `RefCell`, `NonZeroU32`); no external deps for the core.

**Reference spec:** `doc/superpowers/specs/2026-06-01-source-error-manager-design.md`. **C++ source of truth:** `include/hermes/Support/SourceErrorManager.h`, `lib/Support/SourceErrorManager.cpp`, `unsupported/juno/crates/juno_support/src/nullbuf.rs`.

**Porting rule (applies to every task):** keep the Rust structure close to the C++ original where it makes sense, and **copy the comments** (or keep them close). When a step says "port `file.cpp:N-M`", read that range and translate it faithfully, including its comments.

---

## File structure

```
rust/
  Cargo.toml                       # workspace
  crates/
    support/
      Cargo.toml
      src/
        lib.rs                     # re-exports
        buffer.rs                  # NullTerminatedBuf (copied) + SourceBuffer
        location.rs                # SourceId, SMLoc, SMRange, SourceCoords, LineCoord
        line_index.rs              # LineIndex: line-start table, offset<->(line,col)
        diag.rs                    # DiagKind, Diagnostic, ResolvedDiagnostic, DiagHandler,
                                   #   CollectingHandler, OutputOptions, Warning, Subsystem
        render.rs                  # buildSourceAndCaretLine + StderrHandler (byte-compat)
        manager.rs                 # SourceErrorManager facade
```

One responsibility per file; `manager.rs` is the only stateful façade and ties the rest together.

---

## Task 0: Workspace & crate scaffold

**Files:**
- Create: `rust/Cargo.toml`
- Create: `rust/rust-toolchain.toml`
- Create: `rust/.gitignore` (contents: `/target`)
- Create: `rust/crates/support/Cargo.toml`
- Create: `rust/crates/support/src/lib.rs`

> Create `rust/.gitignore` with `/target` **before** the first `git add rust/`, so build
> artifacts are never committed.

- [ ] **Step 1: Create the workspace manifest and toolchain pin**

`rust/Cargo.toml`:
```toml
[workspace]
resolver = "2"
members = ["crates/support"]
```

`rust/rust-toolchain.toml` (pins the toolchain for reproducible builds):
```toml
[toolchain]
channel = "1.96.0"
components = ["rustfmt", "clippy"]
```

- [ ] **Step 2: Create the support crate manifest**

`rust/crates/support/Cargo.toml`:
```toml
[package]
name = "support"
version = "0.0.0"
edition = "2021"
publish = false

[lints.rust]
unsafe_code = "forbid"

[dependencies]
```

The `unsafe_code = "forbid"` lint mechanically enforces the zero-`unsafe` budget for this crate.

- [ ] **Step 3: Create an empty lib.rs**

`rust/crates/support/src/lib.rs`:
```rust
//! Hermes compiler support library (Rust port).
```

- [ ] **Step 4: Verify it builds**

Run: `cargo build --manifest-path rust/Cargo.toml`
Expected: compiles, `Finished` with no errors.

- [ ] **Step 5: Commit**

```bash
git add rust/
git commit -m "rust: scaffold workspace and support crate"
```

---

## Task 1: `buffer` module — `NullTerminatedBuf` + `SourceBuffer`

**Files:**
- Create: `rust/crates/support/src/buffer.rs`
- Modify: `rust/crates/support/src/lib.rs`
- Reference to copy: `unsupported/juno/crates/juno_support/src/nullbuf.rs`

- [ ] **Step 1: Write the failing test**

Append to `rust/crates/support/src/buffer.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_terminated_and_named() {
        let b = SourceBuffer::from_str("foo.js", "abc");
        assert_eq!(b.name(), "foo.js");
        // bytes() excludes the terminator; raw includes it.
        assert_eq!(b.bytes(), b"abc");
        assert_eq!(b.raw()[b.raw().len() - 1], 0u8);
    }

    #[test]
    fn already_terminated_input_not_doubled() {
        let b = SourceBuffer::from_slice_check("x", b"ab\0");
        assert_eq!(b.bytes(), b"ab");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path rust/Cargo.toml -p support buffer`
Expected: FAIL — `SourceBuffer` not found.

- [ ] **Step 3: Implement `NullTerminatedBuf` (copied) and `SourceBuffer`**

Copy `NullTerminatedBuf` from `unsupported/juno/crates/juno_support/src/nullbuf.rs` into `buffer.rs`, keeping its comments, with two changes: (a) drop the two `unsafe`-marked `as_ptr`/`as_c_char_ptr` FFI accessors (not needed; would violate `forbid(unsafe_code)`); (b) keep `from_reader`/`from_file`/`from_slice_copy`/`from_slice_check`/`from_str_copy`/`from_str_check`/`len`/`is_empty`/`as_bytes`. Then add:

```rust
use std::cell::RefCell;
use crate::line_index::LineIndex;

/// A named source buffer: its file name, its NUL-terminated bytes, and a lazily
/// built line index. This is the Rust analog of an `llvh::MemoryBuffer`
/// registered in a `SourceMgr`, but it carries its own name.
pub struct SourceBuffer {
    name: String,
    buf: NullTerminatedBuf,
    /// Lazily built on first line/col resolution. Interior mutability so that
    /// resolution can happen through a shared `&SourceBuffer`.
    line_index: RefCell<Option<LineIndex>>,
}

impl SourceBuffer {
    pub fn from_str(name: impl Into<String>, contents: &str) -> SourceBuffer {
        SourceBuffer {
            name: name.into(),
            buf: NullTerminatedBuf::from_str_copy(contents),
            line_index: RefCell::new(None),
        }
    }

    pub fn from_slice_check(name: impl Into<String>, contents: &[u8]) -> SourceBuffer {
        SourceBuffer {
            name: name.into(),
            buf: NullTerminatedBuf::from_slice_check(contents),
            line_index: RefCell::new(None),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// The source bytes, excluding the trailing NUL terminator.
    pub fn bytes(&self) -> &[u8] {
        let raw = self.buf.as_bytes();
        &raw[..raw.len() - 1]
    }

    /// The raw bytes, including the trailing NUL terminator.
    pub fn raw(&self) -> &[u8] {
        self.buf.as_bytes()
    }
}
```

Note: `line_index` is wired up in Task 3; for now `use crate::line_index::LineIndex` will not resolve, so define a placeholder empty `line_index.rs` with `pub struct LineIndex;` and `mod line_index;` in lib.rs to keep this task compiling, OR reorder to do Task 3's type first. **Do Step 3a below first.**

- [ ] **Step 3a: Add module declarations**

`rust/crates/support/src/lib.rs`:
```rust
//! Hermes compiler support library (Rust port).

pub mod buffer;
pub mod line_index;
```

And create a minimal `rust/crates/support/src/line_index.rs`:
```rust
//! Line-start index for a source buffer (filled in Task 3).

/// Placeholder; replaced in Task 3.
pub struct LineIndex;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --manifest-path rust/Cargo.toml -p support buffer`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add rust/crates/support/src/buffer.rs rust/crates/support/src/lib.rs rust/crates/support/src/line_index.rs
git commit -m "rust(support): NullTerminatedBuf (copied from juno) and SourceBuffer"
```

---

## Task 2: `location` module — `SourceId`, `SMLoc`, `SMRange`, `SourceCoords`, `LineCoord`

**Files:**
- Create: `rust/crates/support/src/location.rs`
- Modify: `rust/crates/support/src/lib.rs`
- Reference: `include/hermes/Support/SourceErrorManager.h:77-123` (`SourceCoords`, `LineCoord`), `combineIntoRange`/`convertEndToLocation`.

- [ ] **Step 1: Write the failing test**

`rust/crates/support/src/location.rs` test module:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_id_niche() {
        // Option<SourceId> must be the same size as SourceId (niche optimization).
        assert_eq!(std::mem::size_of::<Option<SourceId>>(), std::mem::size_of::<SourceId>());
        assert_eq!(std::mem::size_of::<SMLoc>(), 8);
    }

    #[test]
    fn coords_ordering_and_same_line() {
        let s = SourceId::from_index(0);
        let a = SourceCoords { buf: s, line: 2, col: 3 };
        let b = SourceCoords { buf: s, line: 2, col: 5 };
        assert!(a.less(&b));
        assert!(a.is_same_source_line_as(&b));
    }

    #[test]
    fn combine_into_range_orders_endpoints() {
        let s = SourceId::from_index(0);
        let lo = SMLoc { source: s, offset: 4 };
        let hi = SMLoc { source: s, offset: 9 };
        let r = SMRange::combine(hi, lo);
        assert_eq!(r.start.offset, 4);
        assert_eq!(r.end.offset, 10); // end is exclusive: max + 1
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path rust/Cargo.toml -p support location`
Expected: FAIL — types not found.

- [ ] **Step 3: Implement the location types**

`rust/crates/support/src/location.rs` (port the docs from `SourceErrorManager.h:77-123`, copying comments):
```rust
use std::num::NonZeroU32;

/// Opaque identifier of a source registered with the manager. 1-based: index 0
/// maps to `NonZeroU32(1)`, so `Option<SourceId>` is the same size as `SourceId`.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct SourceId(NonZeroU32);

impl SourceId {
    pub fn from_index(index: u32) -> SourceId {
        SourceId(NonZeroU32::new(index + 1).expect("index + 1 is nonzero"))
    }
    pub fn index(self) -> u32 {
        self.0.get() - 1
    }
}

/// A location in a source buffer. The "encoded" form: a buffer plus a byte
/// offset into it. Rust analog of `llvh::SMLoc`, but it carries its buffer
/// identity, so finding the containing buffer is trivial.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct SMLoc {
    pub source: SourceId,
    pub offset: u32,
}

/// A half-open range of source locations `[start, end)` within one buffer.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct SMRange {
    pub start: SMLoc,
    pub end: SMLoc,
}

impl SMRange {
    /// Build the smallest range covering both `a` and `b` (both must be in the
    /// same buffer). The end is exclusive, so it is `max(a,b).offset + 1`.
    /// Port of `SourceErrorManager::combineIntoRange`.
    pub fn combine(a: SMLoc, b: SMLoc) -> SMRange {
        debug_assert_eq!(a.source, b.source);
        let (lo, hi) = if a.offset <= b.offset { (a, b) } else { (b, a) };
        SMRange { start: lo, end: SMLoc { source: hi.source, offset: hi.offset + 1 } }
    }
}

/// The "decoded" form of an `SMLoc`: buffer id, 1-based line and column.
/// Port of `SourceErrorManager::SourceCoords`.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct SourceCoords {
    /// 1-based buffer id (i.e. `SourceId::index() + 1`).
    pub buf: SourceId,
    /// 1-based line number.
    pub line: u32,
    /// 1-based column.
    pub col: u32,
}

impl SourceCoords {
    pub fn is_same_source_line_as(&self, o: &SourceCoords) -> bool {
        self.buf == o.buf && self.line == o.line
    }
    pub fn less(&self, o: &SourceCoords) -> bool {
        (self.buf.index(), self.line, self.col) < (o.buf.index(), o.line, o.col)
    }
}

/// Result of looking up a line: buffer, 1-based line number, and a reference to
/// the line itself (including EOL if present). Port of `LineCoord`.
#[derive(Copy, Clone, Debug)]
pub struct LineCoord<'a> {
    pub buf: SourceId,
    pub line: u32,
    pub line_ref: &'a [u8],
}
```

Add `pub mod location;` to `lib.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --manifest-path rust/Cargo.toml -p support location`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add rust/crates/support/src/location.rs rust/crates/support/src/lib.rs
git commit -m "rust(support): offset-based source locations"
```

---

## Task 3: `line_index` — line-start table and offset↔(line,col)

**Files:**
- Modify: `rust/crates/support/src/line_index.rs` (replace placeholder)
- Reference: `lib/Support/SourceErrorManager.cpp:239-341` (`findBufferAndLine`, `FindLineCache::fillCoords`, `findUntranslatedBufferLineAndLoc`) for the line/col semantics, including how the EOL and last-line-without-EOL are handled.

- [ ] **Step 1: Write the failing test**

`rust/crates/support/src/line_index.rs` test module:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lf_line_and_col() {
        // "ab\ncde\nf"  offsets: a0 b1 \n2 c3 d4 e5 \n6 f7
        let idx = LineIndex::build(b"ab\ncde\nf");
        assert_eq!(idx.line_col(0), (1, 1)); // 'a'
        assert_eq!(idx.line_col(1), (1, 2)); // 'b'
        assert_eq!(idx.line_col(3), (2, 1)); // 'c'
        assert_eq!(idx.line_col(5), (2, 3)); // 'e'
        assert_eq!(idx.line_col(7), (3, 1)); // 'f' (last line, no EOL)
    }

    #[test]
    fn crlf_column_counts_bytes() {
        // "a\r\nb": a0 \r1 \n2 b3 -> 'b' is line 2 col 1
        let idx = LineIndex::build(b"a\r\nb");
        assert_eq!(idx.line_col(0), (1, 1));
        assert_eq!(idx.line_col(3), (2, 1));
    }

    #[test]
    fn line_ref_excludes_nothing_before_eol() {
        let bytes = b"ab\ncde\nf";
        let idx = LineIndex::build(bytes);
        // 1-based line number -> the line slice including its EOL if present.
        assert_eq!(idx.line_ref(bytes, 1), b"ab\n");
        assert_eq!(idx.line_ref(bytes, 3), b"f");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path rust/Cargo.toml -p support line_index`
Expected: FAIL — `LineIndex::build` not found.

- [ ] **Step 3: Implement `LineIndex`**

Replace `line_index.rs` with the implementation. `line_starts[i]` is the byte offset where 1-based line `i+1` begins. Column counting is **byte distance from the line start, 1-based**, matching LLVH (so caret output lines up). Port the line-scanning/EOL handling from `SourceErrorManager.cpp:239-341`, keeping the comments about EOL and the last line.

```rust
//! Line-start index for a source buffer: maps byte offsets to 1-based
//! (line, column) and back. Column is the byte distance from the line start,
//! matching LLVH `SourceMgr` so that caret rendering is byte-compatible.

/// Cached table of line-start byte offsets for one buffer.
pub struct LineIndex {
    /// `line_starts[i]` = byte offset of the start of (1-based) line `i + 1`.
    /// Always begins with 0.
    line_starts: Vec<u32>,
}

impl LineIndex {
    /// Build the index over `bytes` (the buffer contents, excluding the NUL
    /// terminator). A line starts at offset 0 and after each `\n`.
    pub fn build(bytes: &[u8]) -> LineIndex {
        let mut line_starts = Vec::with_capacity(64);
        line_starts.push(0u32);
        for (i, &b) in bytes.iter().enumerate() {
            if b == b'\n' {
                line_starts.push((i + 1) as u32);
            }
        }
        LineIndex { line_starts }
    }

    /// Return 1-based `(line, col)` for `offset`. `col` is 1-based byte distance
    /// from the line start.
    pub fn line_col(&self, offset: u32) -> (u32, u32) {
        // Largest line whose start is <= offset.
        let line0 = match self.line_starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i - 1,
        };
        let col = offset - self.line_starts[line0] + 1;
        ((line0 + 1) as u32, col)
    }

    /// Return the slice of `bytes` for 1-based `line`, including its trailing
    /// EOL if present. The final line may have no EOL.
    pub fn line_ref<'a>(&self, bytes: &'a [u8], line: u32) -> &'a [u8] {
        let start = self.line_starts[(line - 1) as usize] as usize;
        let end = self
            .line_starts
            .get(line as usize)
            .map(|&e| e as usize)
            .unwrap_or(bytes.len());
        &bytes[start..end]
    }

    /// Number of lines.
    pub fn line_count(&self) -> u32 {
        self.line_starts.len() as u32
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --manifest-path rust/Cargo.toml -p support line_index`
Expected: PASS (3 tests). If `crlf_column_counts_bytes` reveals Hermes counts columns differently (e.g. excluding `\r`), adjust to match `SourceErrorManager.cpp` and update the test comment to document the decision.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/support/src/line_index.rs
git commit -m "rust(support): per-buffer line index (offset <-> line/col)"
```

---

## Task 4: `manager` — buffer registration, names, URLs, virtual buffers

**Files:**
- Create: `rust/crates/support/src/manager.rs`
- Modify: `rust/crates/support/src/lib.rs`
- Reference: `lib/Support/SourceErrorManager.cpp:85-122` (`addNewSourceBuffer`, `addNewVirtualSourceBuffer`, `getBufferFileName`), `.h:161-208, 364-416` (virtual-id tag, URL maps).

- [ ] **Step 1: Write the failing test**

`rust/crates/support/src/manager.rs` test module:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_real_and_lookup() {
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("a.js", "let x = 1;");
        assert_eq!(sm.buffer_file_name(id), "a.js");
        assert_eq!(sm.lookup_name("a.js"), Some(id));
        assert!(!sm.is_virtual(id));
    }

    #[test]
    fn virtual_buffer_is_tagged() {
        let mut sm = SourceErrorManager::new();
        let id = sm.add_virtual_buffer("<native>");
        assert!(sm.is_virtual(id));
        assert_eq!(sm.buffer_file_name(id), "<native>");
    }

    #[test]
    fn source_urls_roundtrip() {
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("a.js", "x");
        sm.set_source_url(id, "https://example/a.js");
        sm.set_source_mapping_url(id, "a.js.map");
        assert_eq!(sm.source_url(id), Some("https://example/a.js"));
        assert_eq!(sm.source_mapping_url(id), Some("a.js.map"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path rust/Cargo.toml -p support manager::tests`
Expected: FAIL — `SourceErrorManager` not found.

- [ ] **Step 3: Implement the manager core (buffers/names/urls/virtual)**

Virtual buffers are stored alongside real ones; `is_virtual` is tracked per buffer (simpler and safer than the C++ high-bit tag, and invisible at the API boundary — the tag is an implementation detail). Port the names/URL behavior from the C++ ranges above, copying comments.

```rust
use std::collections::HashMap;
use std::rc::Rc;

use crate::buffer::SourceBuffer;
use crate::location::SourceId;

struct Entry {
    buffer: Rc<SourceBuffer>,
    is_virtual: bool,
    source_url: Option<String>,
    source_mapping_url: Option<String>,
}

/// A facade that owns source buffers and reports diagnostics against them.
/// Rust port of `hermes::SourceErrorManager`.
pub struct SourceErrorManager {
    entries: Vec<Entry>,
    by_name: HashMap<String, SourceId>,
    // Diagnostic state (counts, error limit, warnings, handler, translator) is
    // added in later tasks.
}

impl SourceErrorManager {
    pub fn new() -> SourceErrorManager {
        SourceErrorManager { entries: Vec::new(), by_name: HashMap::new() }
    }

    /// Register a real source buffer and return its id.
    pub fn add_buffer(&mut self, name: &str, contents: &str) -> SourceId {
        self.push(SourceBuffer::from_str(name, contents), false)
    }

    /// Register a real source buffer from raw (possibly already NUL-terminated)
    /// bytes.
    pub fn add_buffer_bytes(&mut self, name: &str, contents: &[u8]) -> SourceId {
        self.push(SourceBuffer::from_slice_check(name, contents), false)
    }

    /// Register a virtual buffer: a name with no contents, used for synthetic
    /// locations. Port of `addNewVirtualSourceBuffer`.
    pub fn add_virtual_buffer(&mut self, name: &str) -> SourceId {
        self.push(SourceBuffer::from_str(name, ""), true)
    }

    fn push(&mut self, buffer: SourceBuffer, is_virtual: bool) -> SourceId {
        let id = SourceId::from_index(self.entries.len() as u32);
        let name = buffer.name().to_string();
        self.entries.push(Entry {
            buffer: Rc::new(buffer),
            is_virtual,
            source_url: None,
            source_mapping_url: None,
        });
        self.by_name.entry(name).or_insert(id);
        id
    }

    pub fn is_virtual(&self, id: SourceId) -> bool {
        self.entries[id.index() as usize].is_virtual
    }

    pub fn buffer_file_name(&self, id: SourceId) -> &str {
        self.entries[id.index() as usize].buffer.name()
    }

    /// Obtain a buffer by id (cloning the `Rc`, e.g. to hand to a lexer).
    pub fn source_buffer(&self, id: SourceId) -> Rc<SourceBuffer> {
        Rc::clone(&self.entries[id.index() as usize].buffer)
    }

    pub fn lookup_name(&self, name: &str) -> Option<SourceId> {
        self.by_name.get(name).copied()
    }

    pub fn set_source_url(&mut self, id: SourceId, url: &str) {
        self.entries[id.index() as usize].source_url = Some(url.to_string());
    }
    pub fn source_url(&self, id: SourceId) -> Option<&str> {
        self.entries[id.index() as usize].source_url.as_deref()
    }
    pub fn set_source_mapping_url(&mut self, id: SourceId, url: &str) {
        self.entries[id.index() as usize].source_mapping_url = Some(url.to_string());
    }
    pub fn source_mapping_url(&self, id: SourceId) -> Option<&str> {
        self.entries[id.index() as usize].source_mapping_url.as_deref()
    }
}

impl Default for SourceErrorManager {
    fn default() -> Self {
        Self::new()
    }
}
```

Add `pub mod manager;` to `lib.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --manifest-path rust/Cargo.toml -p support manager::tests`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add rust/crates/support/src/manager.rs rust/crates/support/src/lib.rs
git commit -m "rust(support): manager buffer registration, names, urls, virtual buffers"
```

---

## Task 5: Location resolution on the manager (offset → coords, line ref, translator)

**Files:**
- Modify: `rust/crates/support/src/manager.rs`
- Modify: `rust/crates/support/src/buffer.rs` (expose a `with_line_index` helper that lazily builds and caches the index)
- Reference: `lib/Support/SourceErrorManager.cpp:288-355` (`findUntranslatedBufferLineAndLoc`, `findBufferLineAndLoc`, `findBufferIdForLoc`).
- **Byte-compat note (carried from Task 3):** `SourceErrorManager.cpp:255-272` `adjustSourceLocation` backs the location pointer off a `\r` and off UTF-8 continuation bytes *before* computing the column (`col = ptr - lineStart + 1`). For token-start locations this is a no-op, but for exact byte-compatibility, resolution here should apply the same adjustment: before computing the column, move the offset back while it points at a `\r` immediately preceding `\n`, or at a UTF-8 continuation byte (`0b10xxxxxx`). Add a test for an offset landing on a `\r` and on a mid-UTF-8 byte.

- [ ] **Step 1: Write the failing test**

Add to `manager.rs` tests:
```rust
#[test]
fn resolves_loc_to_coords() {
    use crate::location::SMLoc;
    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer("a.js", "ab\ncde");
    let loc = SMLoc { source: id, offset: 4 }; // 'd' on line 2 col 2
    let coords = sm.find_coords(loc);
    assert_eq!((coords.buf, coords.line, coords.col), (id, 2, 2));
    assert_eq!(sm.find_buffer_id(loc), id);
}

#[test]
fn translator_is_applied() {
    use crate::location::{SMLoc, SourceCoords};
    use std::rc::Rc;
    struct Shift;
    impl crate::diag::CoordTranslator for Shift {
        fn translate(&self, c: &mut SourceCoords) { c.line += 100; }
    }
    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer("a.js", "ab\ncde");
    sm.set_translator(Some(Rc::new(Shift)));
    let coords = sm.find_coords(SMLoc { source: id, offset: 4 });
    assert_eq!(coords.line, 102);
}
```

(The `CoordTranslator` trait is defined in Task 6; if executing strictly in order, move the `translator_is_applied` test and the `set_translator`/translator field to the end of Task 6 and keep only `resolves_loc_to_coords` here.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path rust/Cargo.toml -p support manager::tests::resolves_loc_to_coords`
Expected: FAIL — `find_coords` not found.

- [ ] **Step 3: Add the lazy line-index helper to `SourceBuffer`**

Add to `impl SourceBuffer` in `buffer.rs`:
```rust
/// Run `f` with this buffer's line index, building and caching it on first use.
pub fn with_line_index<R>(&self, f: impl FnOnce(&LineIndex, &[u8]) -> R) -> R {
    {
        let mut slot = self.line_index.borrow_mut();
        if slot.is_none() {
            *slot = Some(LineIndex::build(self.bytes()));
        }
    }
    let slot = self.line_index.borrow();
    f(slot.as_ref().unwrap(), self.bytes())
}
```

- [ ] **Step 4: Implement resolution on the manager**

Add to `impl SourceErrorManager` (port `findUntranslatedBufferLineAndLoc`/`findBufferLineAndLoc` semantics, copying comments; apply the translator after computing untranslated coords):
```rust
use crate::location::{SMLoc, SourceCoords};

impl SourceErrorManager {
    /// The buffer containing `loc`. Trivial: the location carries its buffer.
    /// Port of `findBufferIdForLoc`.
    pub fn find_buffer_id(&self, loc: SMLoc) -> SourceId {
        loc.source
    }

    /// Decode `loc` to 1-based (buffer, line, col), applying the coordinate
    /// translator if one is installed. Port of `findBufferLineAndLoc`.
    pub fn find_coords(&self, loc: SMLoc) -> SourceCoords {
        let mut coords = self.find_untranslated_coords(loc);
        if let Some(t) = &self.translator {
            t.translate(&mut coords);
        }
        coords
    }

    /// Decode `loc` without applying the translator. Port of
    /// `findUntranslatedBufferLineAndLoc`.
    pub fn find_untranslated_coords(&self, loc: SMLoc) -> SourceCoords {
        let entry = &self.entries[loc.source.index() as usize];
        let (line, col) = entry.buffer.with_line_index(|idx, _bytes| idx.line_col(loc.offset));
        SourceCoords { buf: loc.source, line, col }
    }
}
```

Add a `translator: Option<Rc<dyn crate::diag::CoordTranslator>>` field to the struct (default `None`) and a `set_translator` setter. If Task 6 is not yet implemented, temporarily define the trait inline and move it to `diag.rs` in Task 6.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --manifest-path rust/Cargo.toml -p support manager::tests`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add rust/crates/support/src/manager.rs rust/crates/support/src/buffer.rs
git commit -m "rust(support): offset->coords resolution with lazy line index + translator hook"
```

---

## Task 6: `diag` — diagnostic types, handler trait, collecting handler, warnings, options

**Files:**
- Create: `rust/crates/support/src/diag.rs`
- Modify: `rust/crates/support/src/lib.rs`
- Reference: `.h:25-52` (`SourceErrorOutputOptions`), `.h:55-75` (`DiagKind`, `Subsystem`), `.h:180-208` (warning bitsets, urls), warning enum (search the codebase for the `Warning` enum definition before implementing — `grep -rn "enum class Warning" include/`).

- [ ] **Step 1: Find the `Warning` enum**

Run: `grep -rn "enum class Warning" include/hermes/`
Use the discovered variants verbatim in Step 3 (do not invent them).

- [ ] **Step 2: Write the failing test**

`rust/crates/support/src/diag.rs` test module:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collecting_handler_records() {
        let mut h = CollectingHandler::new();
        h.handle(&ResolvedDiagnostic {
            kind: DiagKind::Error,
            line: 3,
            col: 5,
            file_name: "a.js".into(),
            message: "boom".into(),
            source_line: Some("  let x".into()),
        });
        assert_eq!(h.messages().len(), 1);
        assert_eq!(h.messages()[0].kind, DiagKind::Error);
        assert_eq!((h.messages()[0].line, h.messages()[0].col), (3, 5));
    }

    #[test]
    fn output_options_defaults() {
        let o = OutputOptions::default();
        assert!(o.show_colors);
        assert_eq!(OutputOptions::TAB_STOP, 8);
    }
}
```

- [ ] **Step 3: Implement `diag.rs`**

Port `SourceErrorOutputOptions` (`.h:33-51`) and `DiagKind`/`Subsystem` (`.h:55-75`) faithfully, copying comments. `ResolvedDiagnostic` is the new boundary type so handlers never touch buffers.

```rust
use crate::location::SourceCoords;

/// Kind of diagnostic. Port of `SourceErrorManager::DiagKind`.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum DiagKind { Error, Warning, Note }

/// Subsystem that produced a message. Port of `Subsystem`.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Subsystem { Unspecified, Lexer, Parser }

/// Options for outputting errors. Port of `SourceErrorOutputOptions`.
#[derive(Copy, Clone, Debug)]
pub struct OutputOptions {
    /// Determine whether errors should be colorized.
    pub show_colors: bool,
    /// Soft limit on how wide errors should be (None = unlimited).
    pub preferred_max_error_width: Option<usize>,
}
impl OutputOptions {
    /// Width of a tab.
    pub const TAB_STOP: usize = 8;
    /// Minimum context (in source characters) around a highlighted range.
    pub const MINIMUM_SOURCE_CONTEXT: usize = 16;
}
impl Default for OutputOptions {
    fn default() -> Self {
        OutputOptions { show_colors: true, preferred_max_error_width: None }
    }
}

/// A fully resolved diagnostic handed to a `DiagHandler`. All buffer lookups
/// have already happened, so handlers are free of the source manager.
#[derive(Clone, Debug)]
pub struct ResolvedDiagnostic {
    pub kind: DiagKind,
    pub file_name: String,
    /// 1-based line/col.
    pub line: u32,
    pub col: u32,
    pub message: String,
    /// The source line text (without buffer access), if available.
    pub source_line: Option<String>,
}

/// Sink for resolved diagnostics. Default impls print; the collecting impl
/// captures for tests. Replaces the hardcoded-stderr model.
pub trait DiagHandler {
    fn handle(&mut self, diag: &ResolvedDiagnostic);
}

/// Hook to translate coordinates (e.g. via a source map) before display.
/// Port of `ICoordTranslator`.
pub trait CoordTranslator {
    fn translate(&self, coords: &mut SourceCoords);
}

/// A `DiagHandler` that records diagnostics in memory for tests.
pub struct CollectingHandler {
    messages: Vec<ResolvedDiagnostic>,
}
impl CollectingHandler {
    pub fn new() -> CollectingHandler {
        CollectingHandler { messages: Vec::new() }
    }
    pub fn messages(&self) -> &[ResolvedDiagnostic] {
        &self.messages
    }
}
impl Default for CollectingHandler {
    fn default() -> Self { Self::new() }
}
impl DiagHandler for CollectingHandler {
    fn handle(&mut self, diag: &ResolvedDiagnostic) {
        self.messages.push(diag.clone());
    }
}

/// Per-category warning state. Variants ported verbatim from the C++
/// `enum class Warning` discovered in Step 1.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Warning {
    // TODO(executor): replace with the verbatim variants from Step 1's grep.
    Misc,
}
```

Note for the executor: the `Warning` enum is the **only** place a real list must be filled from Step 1's grep; the `Misc` placeholder exists so the file compiles if you implement top-down, but you MUST replace it with the actual variants (Hermes has e.g. `DirectEval`, `UndefinedVariable`, …) before finishing this task. Add `pub mod diag;` to `lib.rs`. Move the `CoordTranslator` field/trait from Task 5 here if you stubbed it.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --manifest-path rust/Cargo.toml -p support diag::tests`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add rust/crates/support/src/diag.rs rust/crates/support/src/lib.rs
git commit -m "rust(support): diagnostic types, handler trait, collecting handler, options"
```

---

## Task 7: `render` — byte-compatible source line + caret + `StderrHandler`

**Files:**
- Create: `rust/crates/support/src/render.rs`
- Modify: `rust/crates/support/src/lib.rs`
- Reference: `lib/Support/SourceErrorManager.cpp:441-548` (`buildSourceAndCaretLine`) and `:549-668` (`printDiagnosticHelper`, `printDiagnostic`). This is the byte-compatibility-critical port (design decision A).

- [ ] **Step 1: Write the failing test (caret geometry)**

**Signature:** `build_source_and_caret_line(source_line: &str, col: u32, ranges: &[(u32, u32)], opts: &OutputOptions) -> (String, String)`. `col` is the **1-based** caret column (= C++ `columnNo + 1`); `ranges` are **0-based** byte `[start, end)` column pairs within the line (empty for the no-range case).

**Faithful semantics (from `SourceErrorManager.cpp:441-544`), which the tests below encode:**
- Columns are measured in **Unicode code points**: decode the line to chars, building a byte→column map; widen `col-1` and the range endpoints through it. (For all-ASCII lines this is the identity.)
- caret line = `numColumns+1` spaces; fill each range `[first,last)` with `~`; place `^` at `min(widened_col, numColumns)`; then **erase trailing spaces**.
- **Tabs are expanded to spaces** (TabStop=8) in BOTH source and caret: at each tab, `expandCount = 8 - (pos % 8)`; in the caret line, replicate the existing caret char (so a tab under `~` becomes more `~`).
- Width-trim to `max(preferred_max_error_width, focusLength + MINIMUM_SOURCE_CONTEXT)`, focusing on the caret/intersecting range, inserting `...` on trimmed sides.

`rust/crates/support/src/render.rs` test module:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::OutputOptions;

    #[test]
    fn caret_under_single_column() {
        // 1-based col 5 ('x' in "let x = 1;") -> 4 spaces then '^', trailing trimmed.
        let (src, caret) =
            build_source_and_caret_line("let x = 1;", 5, &[], &OutputOptions::default());
        assert_eq!(src, "let x = 1;");
        assert_eq!(caret, "    ^");
    }

    #[test]
    fn tabs_expand_to_spaces_tabstop_8() {
        // Tabs are expanded to spaces (TabStop=8) in BOTH lines, matching LLVH.
        // "\tx" with caret on 'x' (col 2) -> 8 spaces + 'x' / 8 spaces + '^'.
        let (src, caret) =
            build_source_and_caret_line("\tx", 2, &[], &OutputOptions::default());
        assert_eq!(src, "        x");
        assert_eq!(caret, "        ^");
    }

    #[test]
    fn range_underlined_with_tildes() {
        // Range [4,9) underlines "x = 1" with '~', caret '^' at col 5 sits within it.
        let (_src, caret) =
            build_source_and_caret_line("let x = 1;", 5, &[(4, 9)], &OutputOptions::default());
        assert_eq!(caret, "    ^~~~");
    }

    #[test]
    fn non_ascii_columns_are_codepoints() {
        // "é" is 2 bytes but 1 column. Caret on the char after it lands at column 2.
        // (col is 1-based code-point column here; resolution supplies code-point col
        // for callers that need caret display — see the handler's ASCII gate.)
        let (_src, caret) =
            build_source_and_caret_line("éx", 2, &[], &OutputOptions::default());
        assert_eq!(caret, " ^");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path rust/Cargo.toml -p support render`
Expected: FAIL — `build_source_and_caret_line` not found.

- [ ] **Step 3: Port `buildSourceAndCaretLine` and `StderrHandler`**

Implement `build_source_and_caret_line` as a faithful port of `SourceErrorManager.cpp:441-544`, copying its comments. Then the handler — note the **caret is only shown when the source line is all-ASCII** (`printDiagnosticHelper:624`; Hermes punts on non-ASCII caret widths), and the header column is the byte-based 1-based `diag.col`:

```rust
use crate::diag::{DiagHandler, DiagKind, OutputOptions, ResolvedDiagnostic};

/// Default handler: prints `file:line:col: kind: message`, the source line, and
/// (for all-ASCII lines) a caret/underline. Byte-compatible with LLVH
/// `printDiagnosticHelper`. Color (`opts.show_colors`) is honored.
pub struct StderrHandler {
    opts: OutputOptions,
}
impl StderrHandler {
    pub fn new(opts: OutputOptions) -> StderrHandler {
        StderrHandler { opts }
    }
}
impl DiagHandler for StderrHandler {
    fn handle(&mut self, diag: &ResolvedDiagnostic) {
        let kind = match diag.kind {
            DiagKind::Error => "error",
            DiagKind::Warning => "warning",
            DiagKind::Note => "note",
        };
        eprintln!("{}:{}:{}: {}: {}", diag.file_name, diag.line, diag.col, kind, diag.message);
        if let Some(src) = &diag.source_line {
            let (line, caret) = build_source_and_caret_line(src, diag.col, &[], &self.opts);
            eprintln!("{}", line);
            // Hermes only shows the caret line for all-ASCII source lines.
            if src.is_ascii() {
                eprintln!("{}", caret);
            }
        }
    }
}
```

Note: ranges are not yet carried on `ResolvedDiagnostic` (the handler passes `&[]`); Task 8 threads SMRange columns through. Color output and the exact ANSI sequences are validated in Task 10's golden tests; for now honor `show_colors` by gating any escape sequences. Do NOT add a `Remark` kind (Hermes has it, but our `DiagKind` is Error/Warning/Note — out of scope).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --manifest-path rust/Cargo.toml -p support render`
Expected: PASS (2 tests). Validate against C++ in Task 11.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/support/src/render.rs rust/crates/support/src/lib.rs
git commit -m "rust(support): byte-compatible source line + caret rendering and StderrHandler"
```

---

## Task 8: `message`/`error`/`warning`/`note` dispatch + counts + error limit

**Files:**
- Modify: `rust/crates/support/src/manager.rs`
- Reference: `lib/Support/SourceErrorManager.cpp:124-238` (`countAndGenMessage`, `doGenMessage`, `doPrintMessage`, the `message` overloads) and `.h:265-360, 480-540` (error limit, message counts, `setDiagHandler`).

- [ ] **Step 1: Write the failing test**

Add to `manager.rs` tests:
```rust
#[test]
fn error_count_and_limit() {
    use crate::location::SMLoc;
    use crate::diag::CollectingHandler;
    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer("a.js", "abc\ndef");
    sm.set_handler(Box::new(CollectingHandler::new()));
    sm.set_error_limit(1);
    sm.error(SMLoc { source: id, offset: 0 }, "first");
    assert!(sm.is_error_limit_reached());
    assert_eq!(sm.error_count(), 1);
    // After the limit, a "too many errors" message is emitted once and further
    // errors are suppressed (port of sTooManyErrors behavior).
    sm.error(SMLoc { source: id, offset: 1 }, "second");
    assert_eq!(sm.error_count(), 1);
}

#[test]
fn collecting_handler_receives_resolved() {
    use crate::location::SMLoc;
    use crate::diag::{CollectingHandler, DiagKind};
    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer("a.js", "abc\ndef");
    sm.set_handler(Box::new(CollectingHandler::new()));
    sm.warning_misc(SMLoc { source: id, offset: 4 }, "watch out");
    let h = sm.handler_as::<CollectingHandler>().unwrap();
    assert_eq!(h.messages().len(), 1);
    assert_eq!(h.messages()[0].kind, DiagKind::Warning);
    assert_eq!((h.messages()[0].line, h.messages()[0].col), (2, 1));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path rust/Cargo.toml -p support manager::tests::error_count_and_limit`
Expected: FAIL — `error`/`set_handler` not found.

- [ ] **Step 3: Implement dispatch, counts, limit, handler storage**

Add to the struct: `handler: Option<Box<dyn DiagHandler>>`, `message_count: [u32; 3]` (indexed by `DiagKind`), `error_limit: u32` (default `u32::MAX`), `error_limit_reached: bool`. Provide a `handler_as::<T>()` downcast helper (store as `Box<dyn DiagHandler>` plus keep the concrete type testable — simplest: have `CollectingHandler` accessible by making `handler_as` use `Any`; add `fn as_any(&self) -> &dyn std::any::Any` to the `DiagHandler` trait with a default returning `&()` overridden by `CollectingHandler`). Port `countAndGenMessage`/`doGenMessage`/`doPrintMessage` semantics (counts, the one-shot `sTooManyErrors` message at the limit), copying comments. Resolve the location (Task 5), pull the source line, build a `ResolvedDiagnostic`, hand it to the handler.

Provide the overload set mirroring the C++ API:
```rust
pub fn set_handler(&mut self, h: Box<dyn DiagHandler>) { self.handler = Some(h); }
pub fn set_error_limit(&mut self, limit: u32) { self.error_limit = limit; }
pub fn is_error_limit_reached(&self) -> bool { self.error_limit_reached }
pub fn error_count(&self) -> u32 { self.message_count[DiagKind::Error as usize] }
pub fn warning_count(&self) -> u32 { self.message_count[DiagKind::Warning as usize] }

pub fn error(&mut self, loc: SMLoc, msg: impl Into<String>) { /* message(Error, ...) */ }
pub fn error_range(&mut self, range: SMRange, msg: impl Into<String>) { /* ... */ }
pub fn note(&mut self, loc: SMLoc, msg: impl Into<String>) { /* ... */ }
pub fn warning_misc(&mut self, loc: SMLoc, msg: impl Into<String>) { /* warning(Warning::Misc, ...) */ }
```
The central private `fn message(&mut self, kind, warning, subsystem, loc, range, msg)` does resolution + counting + limit + dispatch (port of `SourceErrorManager.cpp:124-212`).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --manifest-path rust/Cargo.toml -p support manager::tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/support/src/manager.rs
git commit -m "rust(support): message dispatch, counts, error limit, handler storage"
```

---

## Task 9: Warning categories (enable/disable, warnings-as-errors)

**Files:**
- Modify: `rust/crates/support/src/manager.rs`
- Reference: `.h:180-330` (`warningStatuses_`, `warningsAreErrors_`, `setWarningStatus`, `setWarningIsError`, `setWarningsAreErrors`, `disableAllWarnings`, `isWarningEnabled`, `isWarningAnError`).

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn disabled_warning_is_dropped() {
    use crate::location::SMLoc;
    use crate::diag::{CollectingHandler, Warning};
    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer("a.js", "abc");
    sm.set_handler(Box::new(CollectingHandler::new()));
    sm.set_warning_status(Warning::Misc, false);
    sm.warning(Warning::Misc, SMLoc { source: id, offset: 0 }, "x");
    assert_eq!(sm.warning_count(), 0);
    assert_eq!(sm.handler_as::<CollectingHandler>().unwrap().messages().len(), 0);
}

#[test]
fn warning_as_error_counts_as_error() {
    use crate::location::SMLoc;
    use crate::diag::{CollectingHandler, Warning, DiagKind};
    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer("a.js", "abc");
    sm.set_handler(Box::new(CollectingHandler::new()));
    sm.set_warning_is_error(Warning::Misc, true);
    sm.warning(Warning::Misc, SMLoc { source: id, offset: 0 }, "x");
    assert_eq!(sm.error_count(), 1);
    assert_eq!(sm.handler_as::<CollectingHandler>().unwrap().messages()[0].kind, DiagKind::Error);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path rust/Cargo.toml -p support manager::tests::disabled_warning_is_dropped`
Expected: FAIL — `set_warning_status` not found.

- [ ] **Step 3: Implement warning categories**

Add `warning_enabled: Vec<bool>` and `warning_as_error: Vec<bool>` sized to the `Warning` variant count (add a `Warning::COUNT` const and `Warning::index(self) -> usize`). Default all enabled, none as-error. In the central `message()`: for warnings, if disabled → return early (no count, no dispatch); if as-error → promote `kind` to `Error` and route through the error path (count + limit). Port behavior + comments from the C++ ranges.

```rust
pub fn set_warning_status(&mut self, w: Warning, enabled: bool) { self.warning_enabled[w.index()] = enabled; }
pub fn set_warning_is_error(&mut self, w: Warning, v: bool) { self.warning_as_error[w.index()] = v; }
pub fn set_warnings_are_errors(&mut self, v: bool) { self.warning_as_error.iter_mut().for_each(|x| *x = v); }
pub fn disable_all_warnings(&mut self) { self.warning_enabled.iter_mut().for_each(|x| *x = false); }
pub fn is_warning_enabled(&self, w: Warning) -> bool { self.warning_enabled[w.index()] }
pub fn is_warning_an_error(&self, w: Warning) -> bool { self.warning_as_error[w.index()] }
pub fn warning(&mut self, w: Warning, loc: SMLoc, msg: impl Into<String>) { /* central message() */ }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --manifest-path rust/Cargo.toml -p support manager::tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/support/src/manager.rs
git commit -m "rust(support): warning categories and warnings-as-errors"
```

---

## Task 10: Differential / golden tests vs. C++

**Files:**
- Create: `rust/crates/support/tests/golden.rs`
- Reference: build a small C++ harness or reuse existing lit diagnostic outputs.

- [ ] **Step 1: Capture a C++ reference rendering**

Pick 3 representative cases (single caret, multi-byte/tab line, range underline). For each, capture the exact stderr Hermes/LLVH produces for the same `(buffer, loc/range, message)`. Source: write a tiny C++ program linking `hermesSupport` that calls `SourceErrorManager::error(...)`, or extract from an existing lit test under `test/`. Save the expected strings inline in the test with a comment citing where each came from.

- [ ] **Step 2: Write the golden test**

```rust
// rust/crates/support/tests/golden.rs
use support::manager::SourceErrorManager;
use support::location::SMLoc;
use support::diag::{CollectingHandler, DiagKind};

#[test]
fn structured_matches_expected() {
    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer("t.js", "let x = ;\n");
    sm.set_handler(Box::new(CollectingHandler::new()));
    sm.error(SMLoc { source: id, offset: 8 }, "expected expression");
    let h = sm.handler_as::<CollectingHandler>().unwrap();
    let m = &h.messages()[0];
    assert_eq!(m.kind, DiagKind::Error);
    assert_eq!((m.line, m.col), (1, 9));
    assert_eq!(m.message, "expected expression");
    // Byte-compat caret is asserted via render.rs unit tests + the captured
    // reference strings from Step 1.
}
```

- [ ] **Step 3: Run it**

Run: `cargo test --manifest-path rust/Cargo.toml -p support --test golden`
Expected: PASS. Where rendered output differs from the captured C++ reference, fix `render.rs` to match (this is the point of decision A) and note any intentional differences.

- [ ] **Step 4: Commit**

```bash
git add rust/crates/support/tests/golden.rs
git commit -m "rust(support): differential/golden tests against C++ diagnostics"
```

---

## Self-review notes (for the executor)

- **Spec coverage:** core types (T2), buffers incl. virtual (T1,T4), line/col + cache (T3,T5), urls (T4), translator (T5,T6), diag handler + collecting (T6), byte-compat rendering (T7), message/error/warning/note + counts + limit (T8), warning categories (T9), testing (T10). Message buffering/coalescing is intentionally **out** (deferred per spec §7).
- **Ordering caveat:** Tasks 5/6 have a forward reference (`CoordTranslator`). If executing strictly top-down, stub the trait in Task 5 and relocate to Task 6, as noted in both tasks.
- **`Warning` enum:** Task 6 Step 1 requires grepping the real variants; the `Misc`-only placeholder must be replaced before Task 6 is considered done (Tasks 8/9 depend on the real set).
- **Column semantics:** Task 3 pins "column = byte distance from line start." Verify against `SourceErrorManager.cpp` during T3; if Hermes differs (e.g. CRLF), adjust the impl and the test comment, since T7's byte-compat depends on it.
- **No `unsafe`:** enforced crate-wide by `unsafe_code = "forbid"` (T0).
