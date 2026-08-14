# The `cpp:NNN` citation checker

This repository holds the C++ Hermes front end and a faithful, bug-for-bug Rust
port of it. Because faithfulness is the premise, the Rust sources say which C++
lines each piece was ported from, by line number:

```rust
//! Ports `SemanticResolver::visit(ClassDeclarationNode *)` (cpp:891-907).
```

There are **3183** such citations, naming **53** C++ files from **84** Rust
files. They are how a reader answers "why does the Rust do this odd thing?" and
how a reviewer checks that a mirror is complete rather than approximate.

They are also line numbers into files that move. Cherry-pick an upstream commit
that inserts three lines near the top of `JSParserImpl-flow.cpp` and every
citation below the insertion starts naming the wrong code. Nothing fails to
compile; the comments just quietly lie. That has happened three times across
two plans.

This directory is the tool that stops it. A snapshot
(`../../citations.snapshot.json`) records, per citation, the C++ file it
resolves to, the lines it names, and **a hash of those lines' exact bytes**.
`check` re-hashes every site against the working tree and names the ones that
moved; `remap` repairs the ones that only moved; `bless` re-records after a
human has looked.

**This is not hypothetical.** Extending the scanner to the colon-less
`NNNN in File.cpp` banner shape found 34 citations in
`lib/Parser/JSParserImpl-flow.cpp` that were all short by exactly 3, caused by
two commits from this repository's own recent upstream sync (`bfeeb404f`, +1
line; `be443ad10`, +2). They were verified to have been correct when written.
They rotted silently, in-tree, for the length of two tasks, because no tool
could see that shape.

## Running it

From the repo root (never `cd rust`, per `CONTRIBUTING.md`):

```bash
# Verify. Exits non-zero and names every site that moved.
cargo run --manifest-path rust/Cargo.toml -p tools --bin citations -- check

# Repair what merely shifted. --dry-run reports and writes nothing.
cargo run --manifest-path rust/Cargo.toml -p tools --bin citations -- remap --dry-run
cargo run --manifest-path rust/Cargo.toml -p tools --bin citations -- remap

# Re-record the current tree, after a reviewed change.
cargo run --manifest-path rust/Cargo.toml -p tools --bin citations -- bless
```

`check` is what the standing test runs, so this is the same thing:

```bash
cargo test --manifest-path rust/Cargo.toml -p tools --test citations
```

The tool's own messages spell the commands `cargo run -p tools --bin citations
-- …`, without `--manifest-path`; that form works only from inside `rust/`.

`bless` also records `HEAD` in the snapshot as `cpp_commit`, the base `remap`
maps *from*. Blessing an unchanged tree therefore rewrites exactly one line of
the snapshot, the commit hash, and nothing else — hashes and coordinates are
reproduced byte for byte.

A clean tree prints:

```text
3183 citation sites checked against 3183 blessed at C++ commit ddf36884d236;
0 stale, 0 re-pointed, 0 unblessed, 0 missing, 0 unresolvable, 114 skipped by config
```

followed by the identity of every skipped site (see `[unresolved]` below) —
every run names them, so a config entry that silences a citation cannot hide.

## Reading the output

The summary line has one number per failure mode, and each non-zero one gets a
block naming the sites:

| Category | What happened | What to do |
|---|---|---|
| `stale` | the cited C++ span no longer hashes to what was blessed: it moved, or its text changed | `remap`; read by hand whatever remap declines |
| `re-pointed` | the site now resolves to a different C++ file or range than the snapshot records — a `citations.toml` edit, not C++ drift | check the config change was intended, then `bless` |
| `unblessed` | a citation in the tree that the snapshot has no entry for — you wrote a new one | read it against the C++, then `bless` |
| `missing` | a snapshot entry with no citation in the tree — you deleted or reworded one | `bless` |
| `unresolvable` | the citation cannot be resolved to a file, or names lines the file does not have | fix the config (below) or the citation |
| `skipped` | `[unresolved]` says not to check it; never a failure | nothing |

`remap` prints three lists: `repaired`, `declined — needs a human`, and `not a
line shift, left alone`. It re-runs `check` afterwards and prints the result as
`after:`, and it exits non-zero while anything is left to do — a green `remap`
means the standing test is green too.

## When `remap` declines a site

**That is not a bug and not something to work around.** `remap` moves the
digits of a citation only when it can *prove* the text is the same: the blessed
hash must reproduce both at the site's base coordinates in the blessed commit
and at the mapped coordinates in the working tree. A decline means one of those
proofs failed — usually because the cited C++ was *edited*, not moved.

Where the text changed, which lines the comment should now name is a semantic
question about what the Rust mirrors, and no line map can answer it. So:

1. Read the decline reason. It names the site, the blessed span, and which
   proof failed: *"was edited or deleted since `<commit>`, so the cited lines
   have no image in the file today"*; *"the text there is not the text that
   was blessed"*; *"a range of a different length: the span itself changed"*;
   *"past the end of the file as it is now"*; or *"the blessed text is not at
   … in `<commit>`"* — the last one meaning the snapshot was blessed against
   content that commit does not have (uncommitted C++, most likely), so
   re-verify by hand and `bless`.
2. Read the citation against the C++ and re-point it by hand.
3. `bless` to re-record.

`bless` accepts whatever the citations currently point at. It does not check
that a citation is *right*; it records that a human has looked.

The full safety argument — hypothesis versus proof, why base coordinates make
remapping idempotent and reversible, and the table of shapes it declines — is
the module doc of [`remap.rs`](remap.rs). Do not weaken it.

## Drift is not wrongness

`remap` repairs citations that **drifted**: the same C++ text at a new
position. It can do nothing at all for citations that were **wrong when
written**, and those are a separate, larger problem. Measured against the 20
sites known to be provably wrong: **0 of 20 are repairable by `remap`**. They
are not stale — the C++ at the lines they name is exactly what was blessed, so
`check` is green on all 20 and `remap` never considers them; and if they were
flagged, the text at the intended location differs from the text at the cited
location in every one, so no destination passes the hash proof.

This is the assumption a future repair decision would most easily get wrong.
Repairing the port's wrong citations needs a *different* instrument — one that
compares each cited span against the Rust body claiming to mirror it — and its
output has to be read site by site before blessing. What this tool buys is that
the next cherry-pick does not add five hundred more.

## Citation forms

The scanner ([`scan.rs`](scan.rs)) recognises these:

```text
cpp:891                          bare, single line
cpp:891-907                      bare, range
SemanticResolver.cpp:891         qualified by basename
flow.cpp:1232                    qualified by a shorthand basename
lib/Parser/JSONParser.cpp:202-211   qualified with a path prefix
ESTree.def:697-750               .def files too
cpp:86-88, 160-245               a comma continuation: `160-245` inherits the file
ESTree.def:697-750 … :677        an implicit file: `:677` inherits it too
C++ 4890-4896                    the parser port's spelling of `cpp:4890-4896`
2886 in JSParserImpl-flow.cpp    the section-banner spelling, no colon
```

A citation may also wrap across two consecutive comment lines, in four places
(after a directory prefix, after the colon, after the range dash, after a
continuation comma). `scan.rs`'s module doc has the guards each shape uses and
why the `:NNN` and `NNN in File.cpp` shapes are deliberately narrow.

Adding a citation in any of these forms needs no ceremony: write it, run
`bless`, and it is checked from then on. A citation nobody blessed is reported
as `unblessed` and fails the standing test, so a new one cannot slip in
unrecorded — and neither can prose the scanner misreads as a citation.

## How a citation resolves to a file

Only [`../../citations.toml`](../../citations.toml) says which C++ file a
citation names. The tool never parses the prose of a module header to guess.
Four tables, consulted in this order:

| Table | Keyed by | Meaning |
|---|---|---|
| `[unresolved]` | site key, cited basename, or Rust-path glob | deliberately not checked, with the reason |
| `[site_override]` | one exact site key | this one site means a different C++ file than its module's rule |
| `[qualified]` | cited basename (`flow.cpp`) | the repo-relative path it stands for |
| `[bare]` | Rust-path glob | the C++ file a bare `cpp:NNN` means in those Rust files |

A **site key** is `<rust path relative to rust/>#<citation text as written>`,
with runs of whitespace inside the citation collapsed to one space (a citation
may wrap):

```text
crates/sema/src/check_implicit_return.rs#cpp:1969-1974
```

Because a site key contains the digits, `remap` renames the `[site_override]`
keys of any citation it moves, and refuses to write anything if a rename would
collide. If you edit a citation by hand and it has an override, move the
override with it.

**`[bare]` is first-match-wins, and that is a hazard.** More specific globs must
come first: `crates/sema/src/resolver/promoter.rs` sits above
`crates/sema/src/resolver/*.rs` because the promoter is a port of a different
C++ file than the rest of `resolver/`. A glob added in the wrong place silently
captures files meant for a later row, and `bless` will happily record whatever
the wrong file says at those lines. `*` does not cross `/`.

### Adding an entry when a new C++ file gets cited

Write the citation, then run `check`. An unmapped file is an error, not a
silent skip:

```text
could not be checked (1):
  crates/tools/src/lib.rs:18 cites NewlyCited.cpp:10-12 — no [qualified] entry
  for "NewlyCited.cpp"; add one to citations.toml
```

Add the row to `[qualified]` (`"NewlyCited.cpp" = "lib/Foo/NewlyCited.cpp"`) and
re-run. Where a basename exists more than once in the tree, read the cited lines
to decide which one is meant and leave a comment saying what you read — several
existing rows do (`HBC.cpp`, `Compiler.h`, `SourceMgr.cpp`, `Allocator.h`).

A new module family that uses bare `cpp:NNN` goes in `[bare]`, and **the
mapping must be verified by reading two or three of that module's citations
against the C++ file before it is added**. A wrong `[bare]` row does not fail
loudly: it resolves, it hashes, and `bless` records nonsense.

### What `[unresolved]` means

"Do not check this, for this stated reason." It is not a mute button: every run
prints the count, the reason, and the site keys. The 114 skips today are the
tool's own sources — every `cpp:NNN` in this directory is a documentation
example or a scanner unit-test fixture, not a citation of the C++ tree — plus
two structurally invalid citations with reversed ranges
(`cpp:266-257`, `CompilerDriver.cpp:2105-2080`), both pre-existing and both
described in the config with what they should have said.

Prefer fixing a citation to skipping it. An `[unresolved]` entry is a promise
that someone looked and decided the site is not checkable, and the reason text
is the whole of the evidence a later reader gets.

## Known citation debt (not repaired, deliberately)

The checker measures debt; it does not mass-repair it, so that a review can
tell a tool bug from a pre-existing wrong citation. This is the state of that
debt, and the single most decision-relevant thing in this README:

- **≥20 citations that resolve, range-check, and name the wrong lines.** This
  is a **floor**, and a weak one: the heuristic that found them covered **473
  of 3045 sites (15.5%)** and **excluded the commonest spelling in the corpus**
  (the inline `// C++ NNN:` form, 42% of all citations). A random sample of 12
  of the excluded form turned up a 4-site drift cluster the heuristic could not
  have seen. The true debt is materially higher and **unmeasured**. A repair
  decision must not budget from 20.
- **23 citations the checker structurally cannot see.** The
  `Port of \`JSParserImpl::foo\` (flow.cpp:NNNN-MMMM)` doc citations that sit
  beside a banner: they were written one line high, then moved 3 lines down by
  the two sync commits above, so they are short by exactly 2. They were blessed
  at trust-on-first-use — the snapshot recorded whatever was under them at
  first bless — and their spans have not moved since, so their hashes match.
  `check` will never flag them and `remap` will never see them. (Audited over
  all 233 `flow.cpp` doc citations: 24 exact, 23 off by −2.) Plus roughly 12
  further mismatches of assorted magnitudes.
- **2 structurally invalid citations**, in `[unresolved]` above.

Nothing here is a defect in the tool; a citation blessed at first use is only
as good as the first reading. What the tool guarantees is that a *correct*
citation stays correct.

## Layout, and why it lives here

| Path | What |
|---|---|
| `mod.rs` | `check`, `bless`, resolution, the reports |
| `scan.rs` | finding citation tokens in a Rust file, including wrapped ones |
| `remap.rs` | the mechanical repair, and its safety argument |
| `snapshot.rs` | the snapshot file format |
| `config.rs` | `citations.toml` parsing |
| `../bin/citations.rs` | the CLI |
| `../../citations.toml` | resolution config |
| `../../citations.snapshot.json` | the blessed snapshot |
| `../../tests/citations.rs` | the standing test |

The tool is part of the `tools` crate, which is `publish = false`. That is
deliberate and load-bearing: it reads across the whole workspace *and* the C++
tree, neither of which may ever end up inside a published crate's tarball —
the same reason `tests/common_copies_identical.rs` lives there. Verified by
`cargo publish --dry-run` on all seven published crates.
