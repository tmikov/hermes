# `check_implicit_return` mutation survey

`survey.py` re-derives the witness tables that
[`../sema_corpus/MANIFEST.md`](../sema_corpus/MANIFEST.md) ("Task-2 review
round: pin the unwitnessed `CheckImplicitReturn` branches") and
[`../sema_corpus_parser/MANIFEST.md`](../sema_corpus_parser/MANIFEST.md)
("Upstream sync task 4" and "Upstream sync task 3") quote. Those tables were
originally produced by a throwaway script in a session scratchpad, which made
them unreproducible; this is that script, checked in.

## Running it

```bash
python3 rust/crates/sema/tests/implicit_return_survey/survey.py
```

Takes roughly 20 minutes: it rebuilds `sema-dump` once per mutation. Options:

- `--only M1,M14,MATCH-C` — run just those mutations (a fast smoke test).
- `--keep-going` — report a mutation that fails to compile instead of stopping.

No C++ build is needed. The comparison is Rust-vs-Rust: the clean port matches
the C++ oracle byte-for-byte on every corpus file, so "differs from the clean
port" and "differs from the oracle" are the same question. (If that ever stops
being true, the differential gate — `sema_differential.rs` — goes red first,
and the survey's premise should be re-checked before its numbers are trusted.)

## What it does

For each entry in the catalogue at the top of `survey.py` it

1. rewrites `rust/crates/sema/src/check_implicit_return.rs` in place, deleting
   or inverting exactly one decision of the analysis,
2. rebuilds `sema-dump`,
3. re-runs it over both differential corpora — `sema_corpus` through the
   driver entry point and `sema_corpus_parser` through `--parser-entry`, each
   file with its own first-line `// FLAGS:` args, the same way
   `sema_differential.rs` invokes them —
4. and counts the files whose (exit status, stdout, stderr) triple moved away
   from a cached clean baseline.

The file is restored from an on-disk copy on every exit path, including
Ctrl-C, and a clean `sema-dump` is rebuilt afterwards. The survey refuses to
start if `check_implicit_return.rs` already has uncommitted changes.

## Reading the output

Each row is `<id> driver=<n> parser=<n>  <description>`, followed by a
markdown table and a summary line. A count is **the number of corpus files
that notice the mutation**, i.e. the number of witnesses that decision has in
that corpus.

- **A zero means the standing gate does not pin that decision at all** — the
  decision could be deleted from the port today and the corpus would stay
  green. That is the finding the tables exist to record, and closing the zeros
  is why `sema_corpus/implicit-return-shapes.js` and
  `sema_corpus_parser/implicit-return-{shapes,try-catch-finally}.js` were
  added.
- The survey exits non-zero if any decision has zero witnesses in **both**
  corpora.
- A zero in one corpus and not the other is normal and often structural: the
  three try-catch-finally decisions (`M19`-`M21`) can only ever be witnessed
  through the parser entry point, because the compile path splits the combined
  `try`/`catch`/`finally` before the analysis runs
  (`SemanticResolver.cpp:794`).
- **Absolute counts drift as the corpora grow**; the MANIFEST tables record
  them as of the day they were run. What those tables assert, and what a later
  run should reproduce, is the zero-versus-non-zero split, not the exact
  numbers. As of 2026-08-13 (driver corpus 224 files, parser corpus 16) no
  decision has zero witnesses in both corpora. A full run then reproduced the
  parser MANIFEST's task-4 "After" column exactly, all 21 rows.
- The `MATCH-*` rows are **not** directly comparable to the parser MANIFEST's
  task-3 table: that table counts `Func`-line *token flips* inside the single
  file `flow-match-implicit-return.js`, whereas this survey counts *files*, so
  every `MATCH-*` row here reads `parser=1`.
- `THROW` currently reads `parser=0`: the parser-entry corpus has no witness
  for `throw` being terminating (the driver corpus has one,
  `try-catch-finally.js`). Recorded as an observation, not a gap — the
  decision is pinned, just not on both entry points.

## Maintaining the catalogue

Each mutation is an exact `(anchor, replacement)` string pair. The survey
checks **before building anything** that every anchor occurs exactly once in
`check_implicit_return.rs`, and aborts naming the mutation if it does not.
That is deliberate: a mutation whose anchor has silently stopped matching
would otherwise report a zero, which reads like a coverage finding rather than
the catalogue bug it is. If you change the analysis, update the anchors in the
same commit.

Ids match the MANIFEST tables:

| Ids | Where they come from |
|---|---|
| `M1`-`M17` | the task-2 survey of the pre-existing decisions |
| `M18`, `THROW` | controls — known-caught before any survey ran |
| `M19`-`M21` | the three decisions upstream `5ae5260c8` (try-catch-finally) adds |
| `MATCH-A`-`MATCH-F` | the task-3 survey of `653e49c60`'s Flow-`match` arm |
