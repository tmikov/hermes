# SourceErrorManager — Rust Port Design

Design for porting Hermes's `SourceErrorManager` (and the buffer / source-location
machinery it sits on) to Rust as the first component of the Rust port.

> **STATUS: COMPLETE.** The entire `SourceErrorManager` is implemented in
> `rust/crates/support/` and validated **byte-for-byte against `hermesc` 1.96.0**
> (`tests/golden.rs`). Everything once marked "deferred" below — message
> buffering/coalescing, ranged diagnostics, external message collection, and the
> remaining find/convert/dump helpers — is **done**. This document is the original
> design; for current project state see `doc/superpowers/RustPortRoadmap.md`.

## Context & goals

`SourceErrorManager` is the foundation of the compiler front-end's diagnostics and
source-location handling. It is the natural first port target: it is depended on by
the lexer (the intended next port) and by everything above it, and it can be built
and tested in complete isolation.

The port is **functionally complete** — its entire public surface is implemented
(originally the core landed first and message buffering/coalescing followed; both are
now done).

### Project conventions

- All new Rust code lives under `rust/` using a typical Rust project layout
  (cargo workspace + member crates).
- Code reused from `unsupported/juno/crates/` is **copied** into `rust/` and modified
  there, never referenced in place.
- When re-implementing Hermes functionality, keep the structure close to the original
  where it makes sense, and copy the comments (or keep them close).

## Foundational decisions

These were settled during design and constrain everything below:

1. **Unsafe policy — encapsulated, never leaking.** The goal is minimal `unsafe`, and
   any `unsafe` must be very well encapsulated and must not leak across module or crate
   boundaries. The source-manager crate itself is **zero `unsafe`**.

2. **Lexer scan cursor — parity-first, encapsulated (option "B").** The future lexer
   may use a raw `*const u8` cursor internally for throughput parity with the C++
   lexer, but only inside the lexer module, never exposed in any type crossing a
   function/module/crate boundary. It converts pointer → offset at every boundary.
   This is a *lexer* concern; it does not affect this crate, but it dictates the
   ownership seam in §3.

3. **Location representation — explicit buffer identity (option "i").**
   `SMLoc = (SourceId, u32 offset)`. A location knows its buffer, so the pointer-based
   reverse lookup (`FindBufferContainingLoc`) disappears entirely. Chosen over a packed
   global 32-bit offset (clang-style) for simplicity; the packed form is a self-
   contained later swap behind the same accessor API if AST memory pressure demands it.

4. **Diagnostic rendering — Hermes-byte-compatible (option "A").** The default handler
   reproduces Hermes/LLVH output exactly (so existing lit diagnostic tests can pass),
   by porting LLVH's `SMDiagnostic` / `buildSourceAndCaretLine` rendering.

5. **Scope.** First cut = core + **warning categories** + **virtual source buffers** +
   **`CoordTranslator` hook**. **Message buffering/coalescing** is deferred and staged
   immediately after.

## 1. Crate & module boundaries

Lives in the `support` crate (Rust port of Hermes `Support` / `juno_support`). The
buffer primitive is copied from juno's `NullTerminatedBuf` and extended. The whole
subsystem is **zero `unsafe`**; interior mutability uses `RefCell`/`Cell` (fixing
juno's `UnsafeCell` usage). Modules:

- `buffer` — owned NUL-terminated source bytes + buffer identity (name)
- `location` — `SourceId`, `SMLoc`, `SMRange`, `SourceCoords`, `LineCoord`
- `line_index` — cached line-start table; offset → (line, col)
- `diag` — `DiagKind`, `Diagnostic`, `DiagHandler` trait, built-in handlers, `OutputOptions`
- `manager` — the `SourceErrorManager` facade (counts, error limit, warnings, virtual
  buffers, translator)

Everything crossing in/out of the crate is safe and cheap to `Copy` (`SMLoc` is 8 bytes).
The manager is the only stateful object.

## 2. Core types

```rust
pub struct SourceId(NonZeroU32);                    // 1-based; Option<SourceId> stays 4 bytes
pub struct SMLoc { source: SourceId, offset: u32 }  // 8 bytes, Copy
pub struct SMRange { start: SMLoc, end: SMLoc }
pub struct SourceCoords { buf: SourceId, line: u32, col: u32 }   // decoded SMLoc
pub struct LineCoord<'a> { buf: SourceId, line: u32, line_ref: &'a [u8] }
```

Consequences of decision (3):
- `findBufferIdForLoc(loc)` collapses to `loc.source`.
- `combineIntoRange` / `convertEndToLocation` become offset arithmetic guarded on
  equal `SourceId`.

## 3. Ownership & the encapsulation seam

- The manager owns buffers as `Rc<SourceBuffer>`, where
  `SourceBuffer { name: String, bytes: NullTerminatedBuf, line_index: RefCell<Option<LineIndex>> }`.
- The lexer (later) is handed an `Rc<SourceBuffer>` + its `SourceId`. It runs an
  internal `*const u8` cursor (decision 2) and converts `ptr → offset = ptr - base`
  before producing any `SMLoc`. The manager never sees a pointer.
- The `Rc` provides the stable heap address that makes the lexer's internal `unsafe`
  sound *and* avoids a borrow-checker fight: the manager is not borrowed by the lexer,
  so it stays callable through `&self` (e.g. `error()`) while scanning is in progress.
- **`unsafe` budget for this crate: zero.**

## 4. Diagnostics & rendering

- `Diagnostic { kind: DiagKind, loc: SMLoc, range: SMRange, message: String, warning: Warning, subsystem: Subsystem }`.
- `trait DiagHandler { fn handle(&mut self, d: &ResolvedDiagnostic); }`, where
  `ResolvedDiagnostic` carries the already-resolved `SourceCoords` + source-line text +
  caret span, so handlers never touch buffers.
- Built-in handlers:
  - **`StderrHandler`** (default) — a faithful port of LLVH `SMDiagnostic::print` /
    `buildSourceAndCaretLine`: `file:line:col: kind: msg`, the source line, the
    `^~~~~` caret/underline, `TabStop = 8`, color controlled by `OutputOptions`
    (`showColors`, `preferredMaxErrorWidth`, etc.). This is the reference for the
    byte-compatibility required by decision (4).
  - **`CollectingHandler`** — pushes `ResolvedDiagnostic`s into a `Vec` for tests.
- Column counting matches LLVH exactly (byte distance from line start, 1-based) so the
  caret output is identical.

## 5. Buffer model: real + virtual

- Real buffer: `Rc<SourceBuffer>` with bytes.
- Virtual buffer: name only, no bytes. The `SourceId` carries a virtual tag bit (port
  of `kVirtualBufIdTag`); such locations resolve to a name-only display, exactly as
  Hermes does.
- The manager keeps a `HashMap<String, SourceId>` for `lookup_name`.

## 6. Warning categories + translator

- `Warning` enum + two bitsets (`enabled`, `as_errors`): `setWarningStatus`,
  `setWarningsAreErrors`, `disableAllWarnings`, `isWarningEnabled`, `isWarningAnError`.
  A disabled warning is dropped before reaching the handler; a warning-as-error bumps
  the error count and the error limit.
- `trait CoordTranslator { fn translate(&self, c: &mut SourceCoords); }`, stored as
  `Option<Rc<dyn CoordTranslator>>`, applied during resolution before the handler runs.
- `Subsystem` enum (`Unspecified` / `Lexer` / `Parser`) carried on each message.

## 7. Message buffering/coalescing (DONE — was staged right after core)

- **Message buffering/coalescing** (`enable_buffering` / `disable_buffering`): ref-counted;
  while active, generated messages are buffered; on the final disable they are stable-sorted
  by source order (the "too many errors" sentinel last), notes attached to and emitted right
  after their parent message, then flushed. No dedup (matches C++). It was carved out behind
  the central dispatch and required no type changes — as predicted. **Implemented.**
- Also completed in the same pass: **ranged diagnostics** (`SMRange` → `^~~~~` underline),
  **subsystem suppression** (`SaveAndSuppressMessages` equivalent), **external message
  collection** (`CollectMessagesRAII` equivalent), and the remaining helpers
  (`find_smloc_from_coords`/`find_smrange_for_line`/`combine_into_range`/
  `convert_end_to_location`/`dump_coords`). The C++ RAII guards are implemented as explicit
  enable-disable / begin-end / set-restore methods (safe-Rust + the crate's `forbid(unsafe)`
  make a `&mut`-holding guard impossible). See `RustPortRoadmap.md`.

## 8. Testing

- Unit tests per module: `line_index` offset→(line,col) including CRLF and a last line
  without an EOL; warning enable / warning-as-error; virtual buffers; `SourceId` niche
  (`Option<SourceId>` size).
- **Differential / golden tests** against C++: feed identical `(buffer, loc, msg)` and
  assert byte-identical rendered output (enabled by decision 4). The `CollectingHandler`
  additionally provides structured, format-independent assertions (kind + resolved
  line/col + message), which is a more robust oracle than text-diffing.

## Out of scope

- The lexer itself (next component; this design only fixes the ownership seam it needs).
- Number parsing, Unicode tables, token tables — lexer concerns.
- Replacing the bump `Allocator`: not needed here.
