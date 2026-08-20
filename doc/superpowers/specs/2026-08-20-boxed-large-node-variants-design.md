# Boxing the large `Node` variants: design

**Date:** 2026-08-20. **Status:** designed and surveyed, not implemented.
**The §6 experiment has been run** — the hypothesis holds, but buys less than
this design implies. Read §6 before §1.
**Motive:** the Rust parser is 1.66× slower than C++ Hermes on the 8.7 MB
typescript fixture but only 1.20–1.25× slower under a megabyte
(`rust/crates/comparison/BENCH-RESULTS.md`, 2026-08-19). The gap is
size-dependent, which points at AST footprint rather than parsing work.

**The hypothesis has now been measured** (§6). Footprint genuinely drives
throughput — about 0.3% per byte removed from the arena entry — but the whole
change buys an estimated +8% to +17%, closing roughly a third of the gap
against C++ Hermes rather than all of it.

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

## 6. The validation, and what it found

Run 2026-08-20 on an idle machine, `typescript.js`, two independent sweeps of
three processes per configuration. Harness and raw results are on the
throwaway branch `bench-node-footprint` (`rust/bench-footprint/run.sh`).

The experiment turned out cheaper than proposed above. Stripping fields from
23 variants breaks 106 construction sites, but the generator already knows
which fields are *decorations* — sema state the parser never reads — and
dropping those takes the widest variant (`ClassDeclaration`) from 120 to 104.
That gives two genuine downward points with a parser that still parses, rather
than a broken tree. Padding `StorageEntry` supplies the upward points.

| entry bytes | how | MiB/s | vs baseline |
|---|---|---:|---:|
| 104 | decorations stripped + `debug_loc` dropped | 68.2 | **+9.5%** |
| 112 | decorations stripped | 66.8 | +7.3% |
| **136** | **baseline** | **62.2** | — |
| 144 | +8 padding | 61.3 | −1.5% |
| 160 | +24 padding | 59.9 | −3.8% |
| 192 | +56 padding | 56.5 | −9.3% |
| 256 | +120 padding | 51.8 | −16.8% |

The baseline ran first and last in each sweep; those two differ by 1.1–1.5%,
which is the noise floor. Every effect above is outside it, and the two sweeps
agree to within a point. LLC-load-misses, dTLB-load-misses and IPC all move
monotonically with entry size, so this is a memory effect rather than a
wall-clock artifact.

**Footprint is real.** The downward slope is 0.30% per byte and strikingly
linear across both points. The upward slope is about half that, 0.15% per
byte. Two things plausibly explain the asymmetry, and they have opposite
implications:

- The curve is genuinely concave — LLC misses per added byte are ~1.4× higher
  in the small region than the large one, so shrinking should help more per
  byte than padding hurts.
- The downward configurations *delete* fields, which also deletes their
  per-node initialization. Boxing keeps every field and only relocates it, so
  it cannot collect that part.

Taking the padding slope as the conservative floor and the downward slope as
the optimistic ceiling, going from 136 to the design's 80 bytes predicts
**+8% to +17%**: roughly 67–73 MiB/s against C++ Hermes' 102.1. The ratio
improves from 1.66× to about 1.40–1.52×.

**So: build it, but not to reach parity.** The design is justified — a tenth
of the runtime for a bounded, self-contained change is worth having — and the
premise that the size-dependent gap is a memory effect is confirmed. What is
*not* supported is the framing at the top of this document, which implied
footprint explains the 1.66×. It explains about a third of it. Two thirds live
somewhere this experiment did not look, and finding them should not wait on
this refactor.

**An alternative worth pricing first.** The 112-byte row is not a simulation:
it is a parser whose nodes carry every ESTree field and have merely lost their
sema decorations, and it ran 7.3% faster. Moving decorations out of the nodes
and into a `HashMap<NodeId, T>` side table would capture much of this design's
benefit, and it is the shape the project already prescribes for everything
else: the reusable-parser principle in
`doc/superpowers/specs/2026-07-26-sema-untyped-design.md:53` grants the AST
"no further annotation fields, ever", and grandfathers the sema decorations
only because sema ships with the parser. They are tolerated, not preferred.
It is not free:
sema would pay a lookup per access, and the `-dump-sema` gate must stay
byte-exact. But it deserves a measurement before 23 variants get boxed, and
the two changes are independent — a side table shrinks every node, boxing
shrinks the enum.

Also worth remembering that the atom table's SipHash is ~8.7% of the profile
and FxHash is a ten-line change. That is the same order of payoff as this
entire design, at a fraction of the cost, and should probably land first.

Related dead end, recorded so it is not retried: shrinking `NodeMetadata` by
dropping the redundant `debug_loc` looked like a cheap independent win, but
`debug_loc != range.start` on **20.58%** of nodes (the parser's `set_location_d`
sites: member expressions, calls, binary tails, optional chains, tagged
templates). A side table would cost 3–5 MiB to save 7.2 MiB, plus a lookup per
read. Reaching 24-byte metadata needs `debug_loc` packed as a u24 delta beside
a u8 `parens` and a u32 `id` — a real option, independent of this design.

The sweep now prices that option directly: the 104 and 112 rows differ by
exactly those 8 metadata bytes and by nothing else, and they differ by 2.2%.
So the u24-delta packing is worth about 2%, not the ~5% guessed earlier.
