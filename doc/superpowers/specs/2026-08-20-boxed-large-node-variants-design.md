# Boxing the large `Node` variants: design

**Date:** 2026-08-20. **Status:** designed and surveyed, not implemented.
**Motive:** the Rust parser is 1.66× slower than C++ Hermes on the 8.7 MB
typescript fixture but only 1.20–1.25× slower under a megabyte
(`rust/crates/comparison/BENCH-RESULTS.md`, 2026-08-19). The gap is
size-dependent, which points at AST footprint rather than parsing work.

**This design rests on an unvalidated hypothesis.** Nobody has demonstrated
that shrinking the node moves the number. Read §6 before implementing.

## 1. The problem, measured

`size_of::<Node>()` is 128. A Rust enum is sized by its largest variant:

| bytes | variants |
|---|---|
| 120 | 2 (`ClassExpression`, `ClassDeclaration`) |
| 112 | 2 (`FunctionExpression`, `FunctionDeclaration`) |
| 104 | 3 (`HookDeclaration`, `ComponentDeclaration`, `ArrowFunctionExpression`) |
| 96 | 2 (`ClassProperty`, `ClassPrivateProperty`) |
| 80 | 5 | 
| 72 | 9 |
| **64** | **20 — including `Identifier`** |
| ≤56 | 228 |

On `typescript.js`: 897,664 nodes, of which `Identifier` alone is 384,269
(43%) at 64 bytes. `StorageEntry` wraps `Node` in 136 bytes; measured live
storage is 147.8 MiB, about 17× the source.

**23 variants exceed 64 bytes, and they are 22,841 nodes — 2.54%.** Those 2.54%
force the other 97.46% to occupy 128 bytes each.

`Identifier` is exactly 64 and 43% of all nodes, so it must stay inline. That
pins the target: box everything above 64, and `Node` becomes 72,
`StorageEntry` 80.

## 2. The change

Boxed variants hold an arena reference instead of the struct:

```rust
pub enum Node<'gc> {
    Identifier(Identifier<'gc>),                      // 64B, inline
    ClassDeclaration(&'gc ClassDeclaration<'gc>),     //  8B, was 120B
    …
}
```

**An arena reference, never `Box`.** Two independent reasons:

- `Box<T>` **cannot be destructured in a pattern** on stable Rust
  (`box_patterns` is unstable). That would break nested matching through the
  tree, which is this crate's distinguishing feature. `&T` matches
  transparently via match ergonomics, so the property survives.
- A `Box` is a separate allocation the collector knows nothing about. `gc()`
  sweeps the arena and returns slots to free lists; boxed payloads would leak
  or need `Drop` glue the arena deliberately does not have.

## 3. Storage: one enum, one pool, one free list

The payloads live in a single enum, **not** in per-size-class raw slots:

```rust
#[repr(C)]
pub(crate) enum Payload<'gc> {
    FunctionDeclaration(FunctionDeclaration<'gc>),
    ClassDeclaration(ClassDeclaration<'gc>),
    …   // the 23
}

struct PayloadEntry<'ctx> {
    ctx_id_markbit: Cell<u32>,
    inner: Payload<'ctx>,
}
```

and `Context` gains exactly two fields, mirroring what is already there for
nodes (`context.rs:188-198`):

```rust
payloads:      UnsafeCell<Deque<PayloadEntry<'ast>>>,
free_payloads: UnsafeCell<Vec<NonNull<PayloadEntry<'ast>>>>,
```

`size_of::<Payload>()` is 128 — the largest variant plus tag, the very number
being escaped. That is fine: it applies to 2.54% of nodes, costing ~3 MiB,
while 97.46% drop from 136 to 80. Estimated total ~72 MiB against 147.8.

Rejected alternative: six size-class pools (72/80/96/104/112/120) of
`[u64; N]` slots. It avoids the 128-byte payload slot but requires type-erased
storage, `ptr::write`, transmuting raw bytes back to `&FunctionDeclaration`,
and an invariant that a 112-byte payload never comes out of a 72-byte pool.
Three MiB is cheaper than that.

## 4. Collection

`Deque` gives stable addresses, which is why the arena uses it rather than
`Vec`. Mark and sweep mirrors the node path exactly: the marker marks a
payload when it walks a boxed node; the sweep pushes dead entries onto
`free_payloads`, as `context.rs:639` already does for nodes.

**The one delicate piece.** `Node::FunctionDeclaration` holds
`&'gc FunctionDeclaration`, which points *inside* a `PayloadEntry`, so the
sweep recovers the entry from an interior pointer. That is `container_of`, and
there is precedent in the same file: `StorageEntry::from_node` does it for
nodes, which is why `Node` is `#[repr(C)]`. Marking `Payload` `#[repr(C)]`
gives the same defined tag-then-union layout, so the offset is constant.

A latent element-stride pointer-arithmetic bug was found in exactly this area
before the 0.1.0 publish. `from_payload` deserves the same scrutiny: a
`debug_assert` round-tripping the pointer, and a test that allocates all 23
kinds and recovers each entry.

## 5. Scope, from the survey

Smaller than it first appears:

- **Most generated accessors need no change.** `kind()`, `metadata()`,
  `range()`, `visit_children`, `dump_children` all match as
  `Node::X(n) => … n.field`, which compiles identically for `&X` and `&&X`.
- **Generated construction is one line**, in `emit_builder_struct`'s `build()`:
  `gc.alloc(Node::{name}(self.inner))` becomes
  `gc.alloc(Node::{name}(gc.alloc_payload(self.inner)))`.
- `Builder::from_node` must copy the struct out from behind the reference for
  boxed kinds.
- **106 construction sites** in the parser and sema (`Node::K(K::new(…))`).
  Match arms are unaffected.
- `gen_nodes.py` owns the boxed set. It cannot measure sizes, so the list is
  hardcoded with a comment pointing at the measurement, and
  `generated_idempotent` keeps it honest.

## 6. Validate before building this

The justification is entirely the bandwidth hypothesis, and it has never been
tested. A cheaper test than implementing the whole thing: temporarily strip
fields from the 23 large variants until `Node` reaches 72, build, and measure
`typescript.js`. The tree is wrong and tests fail — it is a throwaway branch —
but if throughput does not move, this design should be abandoned and the 1.66×
explained some other way.

Related dead end, recorded so it is not retried: shrinking `NodeMetadata` by
dropping the redundant `debug_loc` looked like a cheap independent win, but
`debug_loc != range.start` on **20.58%** of nodes (the parser's `set_location_d`
sites: member expressions, calls, binary tails, optional chains, tagged
templates). A side table would cost 3–5 MiB to save 7.2 MiB, plus a lookup per
read. Reaching 24-byte metadata needs `debug_loc` packed as a u24 delta beside
a u8 `parens` and a u32 `id` — a real option, worth ~5%, independent of this.
