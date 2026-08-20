# Benchmark Results — Internal Working Notes

> **NOT FOR PUBLICATION (decision 2026-08-12):** public docs and READMEs carry
> no performance claims. These numbers are directional (single machine, no CPU
> pinning; the C++ baseline showed ±30% session-to-session swing) and are kept
> only as internal working data for a future hardened re-measurement.

**Current numbers:** see [Re-measurement (2026-08-19, matched harness)](#re-measurement-2026-08-19-matched-harness).  
**Platform:** Linux x86-64, single machine.

The 2026-06-19 run below is kept as history. Its C++ Hermes column was built
with GCC and is **superseded**; its Rust column mixed two process-isolation
modes. Both problems are corrected in the 2026-08-12 re-measurement.

---

## Re-measurement (2026-08-19, matched harness)

**Date:** 2026-08-19. **Tree:** `9c5ae6228`. Same machine as the two runs
below. Port vs C++ Hermes only — OXC, SWC and Boa were not re-measured, so
their 2026-08-12 figures stand.

### What changed since 2026-08-12

**Both sides now run the same measurement loop.** The 2026-08-12 port numbers
came from Criterion while the C++ came from `parse-bench`; two different
harnesses timing what is nominally the same thing. This run uses
`examples/port_parse_bench.rs`, a direct mirror of
`tools/parse-bench/parse-bench.cpp`'s default mode: one untimed warm-up, N
timed iterations, median reported. Each timed iteration includes context
setup, the parse, and teardown on both sides — the C++ `parseOnce` destroys
its `Context` and parser before the clock is read, and dropping `ParsedJS`
frees the arena the same way.

The C++ baseline was rebuilt Clang/Release. The `cmake-build-release`
directory found in the tree at the start of this run was configured with
`/usr/bin/c++`, i.e. GCC — the exact confound corrected in the 2026-08-12 run,
reappeared because `CLAUDE.md`'s documented Release recipe carried no compiler
flags. That GCC directory has been deleted, `cmake-build-release` now holds a
Clang build, and the recipe in `CLAUDE.md` now specifies the compiler, so the
canonical name can be trusted. A GCC Release directory is worse than none: it
looks authoritative and silently yields wrong numbers.

### Results (MiB/s, median, FullParse/eager, one process per measurement)

| Parser | react 107K | jquery 278K | three.min 654K | typescript 8.7M |
|---|---|---|---|---|
| **Hermes Rust port** | 94.8 | 73.4 | 42.1 | 61.5 |
| **C++ Hermes (Clang, Release)** | 118.2 | 87.8 | 50.4 | 102.1 |

Port as a fraction of the Clang-built C++ baseline:

| | react | jquery | three.min | typescript |
|---|---|---|---|---|
| port / C++ Hermes | 80% | 84% | 84% | **60%** |

Five isolated runs per parser on typescript (`--iters=20`), three on the other
fixtures (`--iters=50`); the median of the per-run medians is reported.
Run-to-run spread was tight on both sides: C++ typescript 101.3–103.9 MiB/s,
port typescript 61.0–62.2 MiB/s. Nothing resembling the ±30% session swing
that made the 2026-06-19 numbers untrustworthy. react's first C++ run (108.0)
is a cold outlier against 118.5/118.2.

### Reading

1. **The port is 1.66× slower than C++ Hermes on the 8.7 MB typescript
   fixture**, and 1.20–1.25× slower on the fixtures under a megabyte. The gap
   is size-dependent, not uniform.
2. **This reproduces 2026-08-12 closely** — 61.5 vs 61.5 MiB/s on typescript,
   60% vs 61% of baseline — across a week, a squashed history, and a switch
   from Criterion to the matched harness. Two independent harnesses agreeing
   is the strongest evidence in this file that the large-file figure is real
   and not an artifact of either one.
3. **The AST-footprint root cause in §3 below still stands.** It was measured
   on the port and is independent of both the C++ compiler and the harness.
4. Do not compare MiB/s *across* fixtures. three.min looks slowest at 50 MiB/s
   because minification packs more syntax into each byte, not because it is
   harder to parse.

### Reproducing this run

```bash
cmake -B cmake-build-release -G Ninja -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++
cmake --build cmake-build-release --target parse-bench
bash rust/crates/comparison/fetch_fixtures.sh
cargo build --release --manifest-path rust/crates/comparison/Cargo.toml \
  --example port_parse_bench

F=rust/crates/comparison/fixtures/typescript.js
./cmake-build-release/bin/parse-bench --iters=20 $F        # repeat, fresh process each time
./rust/crates/comparison/target/release/examples/port_parse_bench --iters=20 $F
```

---

## Re-measurement (2026-08-12, Clang baseline)

**Date:** 2026-08-12. **Tree:** `f39215889` (then on the since-deleted `rust1`). Same machine as the
2026-06-19 run, nothing else running, C++ and Rust measured serially.

### What changed since 2026-06-19

1. **The C++ baseline is now Clang-built.** The original baseline came from a
   bare `cmake -DCMAKE_BUILD_TYPE=Release`, which on Ubuntu silently selects
   GCC (`/usr/bin/c++`). GCC-built Hermes is materially slower than
   Clang-built Hermes, and the Rust parsers it is compared against are built
   by rustc's LLVM — a compiler confound. The project rule is to always
   configure the C++ tree with `-DCMAKE_C_COMPILER=clang
   -DCMAKE_CXX_COMPILER=clang++`; see
   [`doc/superpowers/2026-06-30-hermes-vs-oxc-parser-perf.md`](../../../doc/superpowers/2026-06-30-hermes-vs-oxc-parser-perf.md).
   Clang raises the C++ baseline on every fixture, by +43% on react.
2. **Every measurement is now process-isolated.** One fresh process per
   (parser, fixture). The Criterion harness turned out to be sensitive to what
   ran earlier in the same process: running the react benchmark first slows the
   subsequent port jquery benchmark from 3.81 ms to 5.00 ms (73 → 55 MiB/s),
   reproducibly, and speeds up the bump-allocator parsers. The C++ tool shows
   no such sensitivity (multi-file and single-file runs agree within 2%). The
   2026-06-19 table mixed the two modes: its port numbers are isolated-mode
   values, its OXC/SWC numbers are shared-process values.

### Results (MiB/s, median, FullParse/eager, one process per measurement)

| Parser | react 107K | jquery 278K | three.min 654K | typescript 8.7M |
|---|---|---|---|---|
| **Hermes Rust port** | 95.6 | 72.5 | 42.1 | 61.5 |
| **C++ Hermes (Clang, Release)** | 113.1 | 86.9 | 49.9 | 100.7 |
| OXC 0.137.0 | 192.9 | 124.1 | 75.6 | 149.1 |
| SWC 41.1.1 | 97.1 | 70.5 | 36.2 | 62.4 |
| Boa 0.21.1 | 12.1 | 10.6 | 4.7 | 5.0 |

Port as a fraction of the Clang-built C++ baseline:

| | react | jquery | three.min | typescript |
|---|---|---|---|---|
| port / C++ Hermes | 85% | 83% | 84% | 61% |

C++ medians are over 6 runs of `parse-bench --iters=30` per fixture; port,
OXC and SWC medians are over 3 (port) and 2 (OXC, SWC, Boa) Criterion runs of
100 samples each. Run-to-run spread was under 3% for every entry except the
port's `three.min`, which is bimodal (see below).

### Shared-process figures, for transparency

Running the whole Criterion group in one process (the mode the 2026-06-19
OXC/SWC numbers came from) gives: port 95.4 / 54.6 / 28.8 / 61.4, OXC
229.5 / 154.0 / 101.3 / 175.9, SWC 93.5 / 67.2 / 33.8 / 59.7. The C++ tool run
over all four files in one process gives 114.0 / 86.5 / 50.3 / 98.2 — i.e.
unchanged. Isolated mode is the figure published above because it is the only
mode in which every parser is measured on the same footing, and because it is
what a caller parsing one file actually gets.

The port's `three.min` number is the least stable measurement in the set: in
isolation it lands at 42 MiB/s, after other benchmarks in the same process at
29–35 MiB/s. Treat it as ~42 MiB/s with a wide band.

### Conclusions after re-measurement

1. **The port does not beat C++ Hermes anywhere.** It reaches 83–85% of the
   Clang-built C++ baseline on the small and medium fixtures and 61% on the
   8.7 MB typescript fixture. The earlier "faster than C++ Hermes on react"
   claim was an artifact of the GCC baseline and does not survive the correct
   compiler.
2. **The large-file gap is unchanged in kind** (61% of C++ on typescript,
   previously reported as ~32% slower). The AST-footprint root cause below
   still stands — it was measured on the port and is independent of how the
   C++ side was built.
3. **The port is comparable to SWC, not uniformly ahead of it.** Isolated:
   ahead on jquery (72.5 vs 70.5) and three.min (42.1 vs 36.2), within ~2%
   behind on react (95.6 vs 97.1) and typescript (61.5 vs 62.4).
4. **OXC's parse-only lead over this port is 1.7–2.4×** (react 2.0×, jquery
   1.7×, three.min 1.8×, typescript 2.4×). Parse-vs-parse overstates the
   design gap; on the equal-work comparison (parse + binding/semantic) OXC is
   1.3–1.7× faster than C++ Hermes. See the investigation linked above.

The remaining sections below are the 2026-06-19 record.

---

## Methodology (2026-06-19 run)

### Rust port (hermes-parser)

Measured with [Criterion.rs](https://github.com/bheisler/criterion.rs) via
`cargo bench --manifest-path rust/crates/comparison/Cargo.toml` at `opt-level = 3`.
Each iteration creates a fresh `Context` (GC arena) and discards it after parsing,
so allocation and deallocation are included in each sample. Parse mode: `FullParse`
(eager — all AST nodes built). Criterion reports the median; that is the figure used
here.

### C++ Hermes baseline

Measured with the `parse-bench` tool built from the same Hermes tree, in Release
configuration (`cmake -DCMAKE_BUILD_TYPE=Release`). Each iteration creates a fresh
`Context` and parses eagerly. Median reported. Run on the same machine, same OS, same
fixtures.

> **Superseded.** That bare configure line selects GCC on Ubuntu. The resulting
> C++ column is not a valid baseline; use the Clang numbers in the 2026-08-12
> re-measurement above.

### Fixture sizes

| Fixture | Size |
|---|---|
| `react.development.js` | ~107 KB |
| `jquery-3.7.1.js` | ~278 KB |
| `three.min.js` | ~654 KB |
| `typescript.js` | ~8.7 MB |

Fixture sizes are after fixture download via `fetch_fixtures.sh`; exact byte counts
may vary slightly by version.

### Trailing-error fairness guard

A `.err` variant of each fixture (truncated to provoke a parse error) was run through
all parsers. All parsers reported the error — confirming that all parsers were running
in eager/FullParse mode and that no parser was short-circuiting before reaching the
truncation point. Boa's error-path throughput is not listed because Boa short-circuits
at the first error in its engine-internal error path; its number would not be
comparable to the others.

---

## Results — 2026-06-19, SUPERSEDED (MiB/s, median, FullParse/eager, same machine)

> **Superseded by the 2026-08-12 re-measurement above.** The C++ row is
> GCC-built; the port row is isolated-mode while the OXC/SWC/Boa rows are
> shared-process. Kept for history only.

| Parser | react 107K | jquery 278K | three.min 654K | typescript 8.7M |
|---|---|---|---|---|
| **Hermes Rust port** | 97.8 | 73.8 | 42.4 | 63.0 |
| **C++ Hermes (Release)** | 78.9 | 82.6 | 47.5 | 92.4 |
| OXC 0.137.0 | 230.5 | 152.2 | 101.7 | 176.7 |
| SWC 41.1.1 | 93.9 | 66.4 | 34.0 | 60.3 |
| Boa 0.21.1 | 12.0 | 10.5 | 4.8 | 4.9 |

Biome (`biome_js_parser 0.5.7`) was excluded from benchmarking due to a crates.io
publish mismatch (`biome_js_syntax 0.5.7` vs `biome_rowan 0.5.8`) that prevents it
from being used as a standalone Cargo dependency.

---

## Conclusions (2026-06-19; items 1 and 2 corrected above)

### 1. Rust port is at C++ Hermes parity on small and medium files — WITHDRAWN

> **Withdrawn.** Against the Clang-built baseline the port is at 83–85% of C++
> Hermes on the small and medium fixtures, not at or above parity. See the
> 2026-08-12 conclusions.

The Rust port outperforms C++ Hermes on the react fixture (97.8 vs 78.9 MiB/s) and
is within ~11% on the jquery and three.min fixtures (73.8 vs 82.6 and 42.4 vs 47.5).
On small and medium inputs the Rust port matches or beats the C++ baseline.

On the 8.7 MB typescript fixture the Rust port is ~32% slower than C++ Hermes
(63.0 vs 92.4 MiB/s). This is a real gap; the root cause is discussed below.

### 2. The OXC gap is inherent to Hermes design — not a port regression

> **Restated.** The ~2.4–2.8× figure below is parse-vs-parse and mixes process
> modes; measured consistently it is 1.7–2.4×, and on the equal-work
> comparison (parse + binding/semantic) OXC leads C++ Hermes by 1.3–1.7×. The
> "beats SWC on every fixture" claim is likewise corrected above. The design
> reasons stated in this section are unaffected.

OXC is roughly 2.4–2.8× faster than this port across fixtures. This gap exists
between OXC and C++ Hermes as well, and would exist for any faithful port of Hermes:

- Hermes performs full atom interning (string deduplication) during lex and parse.
  Every identifier is looked up in an atom table on every occurrence. OXC uses a
  zero-copy `Atom` type backed by source text slices — no interning during parse.
- Hermes builds a full ESTree-compatible AST with 271 node kinds, GC-arena-allocated.
  OXC uses a bump allocator, `u32` spans, and a more compact node representation.

These are design constraints of a faithful port — they are what Hermes is. The Rust
port also beats SWC on every fixture (97.8 vs 93.9 on react, 73.8 vs 66.4 on jquery,
42.4 vs 34.0 on three.min, 63.0 vs 60.3 on typescript).

### 3. Root cause of large-file gap: AST node footprint, not GC collection

Decomposition of parse time on the typescript fixture:

- The lexer alone runs at ~216 MiB/s on typescript — faster than C++ full-parse.
  The lexer is not the bottleneck.
- AST construction dominates at scale: ~44% of parse time on react, rising to ~71%
  on typescript.
- Every AST node is a uniform 128-byte `Node` enum (confirmed via `std::mem::size_of`)
  wrapped in a 136-byte storage entry. The typescript fixture produces approximately
  904,000 nodes, totalling ~123 MiB of live AST memory — roughly 14× the source size.
  At this scale the working set far exceeds CPU cache, and memory bandwidth becomes
  the bottleneck.
- The parser never runs a GC collection during parse (the arena is freed in one drop
  after parse completes). The bottleneck is allocation and write bandwidth, not
  collection.

This is a candidate fixable inefficiency: boxing the large `Node` variants (rather
than making every node pay the cost of the largest variant) would shrink the average
node size and likely recover a significant fraction of the large-file gap. This
hypothesis has not been validated — boxing trades footprint for indirection, and the
net effect on throughput must be measured. This is a follow-up optimization for the
maintainer, not a blocker for the port's correctness story.

---

## Reproducing

```bash
# Download fixtures (once)
bash rust/crates/comparison/fetch_fixtures.sh

# Rust benchmarks — one process per (parser, fixture); the group as a whole is
# process-state sensitive, so filter down to a single benchmark per run.
cargo bench --manifest-path rust/crates/comparison/Cargo.toml -- '^parse/hermes/react$'

# C++ baseline — Clang Release build (a bare configure picks GCC on Ubuntu and
# understates C++ Hermes)
cmake -B cmake-build-release-clang -G Ninja -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++
cmake --build cmake-build-release-clang --target parse-bench
cmake-build-release-clang/bin/parse-bench --iters=30 <fixture>
```
