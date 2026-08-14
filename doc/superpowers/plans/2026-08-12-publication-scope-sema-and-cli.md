# Publication Scope Extension — `hermes-sema` + `hermes-command-line`

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the crates.io launch from five crates to **seven**: publish the
semantic-analysis crate as `hermes-sema` (so the published family is the full
front end, not just parsing) and the CLI-option helper as
`hermes-command-line`, with a combined parse+resolve façade living in
`hermes-sema`.

**Architecture:** `hermes-sema` (~17k LOC, 10 public modules) takes a real
dependency on `hermes-parser` and offers `resolve(ParsedJS) -> ResolvedJS`,
the semantic counterpart of the parser's `parse()` façade. Its `sema-dump`
binary moves to the unpublished `tools` crate, matching the precedent set when
the parser's four bins moved — published libraries ship no binaries and no
`command_line` dependency. `hermes-command-line` is dependency-free and
publishes unchanged in behavior, renamed only.

**Tech Stack:** Rust 1.96 workspace in `/home/tmikov/work/hermes-rust1` (branch
`rust1`), C++ oracle binaries in that worktree's `cmake-build-asan/bin/`.

## Global Constraints

- **NEVER `cd`.** Use `cargo --manifest-path /home/tmikov/work/hermes-rust1/rust/Cargo.toml`,
  `git -C /home/tmikov/work/hermes-rust1`, absolute paths.
- **User decisions (locked, do not relitigate):** CLI crate publishes as
  `hermes-command-line` (lib `hermes_command_line`); the combined façade lives
  in `hermes-sema`, which depends on `hermes-parser` (NOT a parser feature).
- **Naming convention:** package `hermes-x`, lib name defaults to `hermes_x`,
  directory name unchanged (`crates/sema`, `crates/command_line`). Dependent
  manifests alias: `hermes_sema = { path = "../sema", package = "hermes-sema" }`.
- **Published crates ship no binaries and no `command_line` dependency.**
- **Doc accuracy is a defect class.** Every doc comment must be verifiable
  against the code; no invented rationale. `#![warn(missing_docs)]` with zero
  warnings is the gate on both newly published crates.
- **No performance claims** anywhere in user-facing docs (standing decision
  2026-08-12).
- Zero warnings in every feature config (`cargo build --workspace --all-targets`
  and `--all-features`) plus `RUSTFLAGS="-D warnings"`. fmt rule: per-file
  **no NEW** rustfmt diffs (the workspace is not rustfmt-default-clean).
- Gates that must stay green throughout: parser differential 8/8, json 1/1,
  preparse 4/4, lexer 6/6, sema driver corpus **219/109** + parser-entry
  **13/5**, full workspace tests.
- Commit trailers on every commit:
  `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>` and
  `Claude-Session: https://claude.ai/code/session_01ERsmFoVAnZCRwfapbPMibv`.
- Publication order after this plan (7 crates, single multi-package
  invocation): `hermes-unicode`, `hermes-atom-table`, `hermes-command-line`
  (dependency-free), `hermes-support`, `hermes-ast`, `hermes-parser`,
  `hermes-sema`.

---

### Task 1: `hermes-command-line` — rename, document, publish-enable

**Files:**
- Modify: `rust/crates/command_line/Cargo.toml`, `src/lib.rs`, `src/{cl,opt,parser}.rs`
- Create: `rust/crates/command_line/README.md`, `LICENSE`, `NOTICE`
- Modify: `rust/crates/tools/Cargo.toml`, `rust/crates/sema/Cargo.toml` (dep key)
- Modify: every `use command_line::` site (bins under `crates/tools/src/bin/`,
  `crates/sema/src/bin/sema_dump.rs`)

**Interfaces:**
- Consumes: nothing (this crate has no dependencies).
- Produces: package `hermes-command-line` v0.1.0, lib `hermes_command_line`,
  publishable, `missing_docs`-clean.

- [ ] **Step 1: Rename the package and sweep importers**

In `rust/crates/command_line/Cargo.toml`: `name = "hermes-command-line"`,
`version = "0.1.0"`, remove `publish = false`, and add the same metadata block
the other five published crates carry (description, license, repository,
readme, keywords, categories, edition — copy the shape from
`rust/crates/unicode/Cargo.toml`, adapting the description). Then rewrite every
importer: `use command_line::` → `use hermes_command_line::`, and the dep keys
in `tools/Cargo.toml` and `sema/Cargo.toml` to
`hermes_command_line = { path = "../command_line", package = "hermes-command-line" }`.
Hand-check the sweep: a local variable or module named `command_line` must not
be rewritten.

- [ ] **Step 2: Document the public API (`#![warn(missing_docs)]`)**

Add `#![warn(missing_docs)]` **below** the `//!` crate doc (the convention
settled in the parser crate). Rewrite the crate doc: it is a Meta-authored,
LLVM-`cl`-**style** option parser copied verbatim from
`unsupported/juno/crates/command_line` in the Hermes repo — it is NOT derived
from LLVM source. Document every item re-exported from `lib.rs`
(`CommandLine`, `Opt`, `OptDesc`, `OptHolder`, `OptValue`, `EnumDesc`,
`ExpectedValue`, `Hidden`, `parse_bool`, `parse_disallowed`,
`CommandLineIntent`, and anything else `pub use`d) plus their public methods.
Verify each doc claim against the implementation; do not invent behavior.

- [ ] **Step 3: Resolve the module-level `#[allow(dead_code)]`**

`lib.rs` puts `#[allow(dead_code)]` on `mod cl`, `mod opt`, `mod parser`
because in-tree callers use only part of the surface. For a published crate the
blanket allow hides real dead code. Determine what it is actually suppressing
(remove the allows, build, read the warnings). Then either (a) drop the allows
if nothing fires now that the crate is a library others consume, or (b) keep
them with a comment naming the specific reason. Record which and why in your
report. Do not delete functionality — this is a faithful juno copy.

- [ ] **Step 4: Clippy**

`cargo clippy -p hermes-command-line --all-targets` currently reports 2
`type_complexity` lints. Either fix them or add a scoped `#[allow]` with a
one-line justification at the site (the crate's convention is faithful-copy
idioms; a crate-level blanket allow is not acceptable for a published crate).

- [ ] **Step 5: README, LICENSE, NOTICE**

Copy `rust/LICENSE` and `rust/NOTICE` into the crate directory (byte-identical,
as the other five crates do). Write a short `README.md` following the shape of
`rust/crates/unicode/README.md`: what it is, the juno provenance, that it is
published because the Hermes Rust tools use it and it is independently useful,
and the standard "no stability guarantee of its own" line if that matches the
other support crates. Absolute GitHub links only (relative links break on
crates.io). No performance claims.

- [ ] **Step 6: Verify**

```bash
cargo build --manifest-path /home/tmikov/work/hermes-rust1/rust/Cargo.toml --workspace --all-targets
cargo test --manifest-path /home/tmikov/work/hermes-rust1/rust/Cargo.toml
```
Zero warnings (also with `RUSTFLAGS="-D warnings"` and `--all-features`); zero
missing-docs; workspace tests green. The four `tools` bins and `sema-dump` must
still build and the differentials still pass.

- [ ] **Step 7: Commit**

`rust(publish): rename command_line to hermes-command-line; document + publish-enable`

---

### Task 2: `hermes-sema` — rename, move `sema-dump` to `tools`, publish plumbing

**Files:**
- Modify: `rust/crates/sema/Cargo.toml` (name, version, publish, drop bin +
  `dump-bin` feature + optional deps)
- Move: `rust/crates/sema/src/bin/sema_dump.rs` → `rust/crates/tools/src/bin/sema_dump.rs`
- Modify: `rust/crates/tools/Cargo.toml` (add the bin + `hermes_sema` dep)
- Modify: `rust/crates/sema/tests/sema_differential.rs` (drop the feature gate;
  locate the binary the way the parser's differential does)
- Modify: every `use sema::` site; `rust/crates/sema/tests/*.rs`;
  corpus MANIFESTs' recorded commands
- Create: `rust/crates/sema/README.md`, `LICENSE`, `NOTICE`

**Interfaces:**
- Consumes: Task 1's `hermes_command_line` (the moved bin uses it).
- Produces: package `hermes-sema` v0.1.0, lib `hermes_sema`, no bins, no
  `command_line` dependency; `sema-dump` built from `tools`.

- [ ] **Step 1: Move the binary**

```bash
git -C /home/tmikov/work/hermes-rust1 mv rust/crates/sema/src/bin/sema_dump.rs rust/crates/tools/src/bin/sema_dump.rs
```
Add to `tools/Cargo.toml`: `[[bin]] name = "sema-dump"`, `path = "src/bin/sema_dump.rs"`,
and the `hermes_sema` + `hermes_command_line` dependencies it needs. Keep the
bin's behavior byte-identical — including the hidden `--Xparse-flow-match`
alias, which the flow-match corpus file depends on.

- [ ] **Step 2: Strip the publish blockers from `sema/Cargo.toml`**

`name = "hermes-sema"`, `version = "0.1.0"`, remove `publish = false`, remove
the `[[bin]]` block, remove the `dump-bin` feature, and remove the now-unused
optional `command_line` dependency. Add the standard published-crate metadata
block. `hermes_parser` becomes a **real** (non-optional) dependency in Task 3;
in this task keep whatever form leaves the tree building, and say which you
chose in your report. Keep `unsafe_code = "forbid"`.

- [ ] **Step 3: Fix the differential harness**

`rust/crates/sema/tests/sema_differential.rs` is `#![cfg(feature = "dump-bin")]`
and invokes the in-crate `sema-dump`. The feature is gone, so: drop the gate and
locate the binary the way `rust/crates/parser/tests/common/mod.rs::tools_bin()`
does (memoised nested `cargo build -p tools --bin sema-dump --message-format=json`,
reading the artifact path). Reuse that helper's approach; if duplicating it into
the sema crate's tests, keep the two copies obviously identical or factor it
somewhere both can reach. The test must still skip cleanly when
`REQUIRE_DIFFERENTIAL` is unset.

- [ ] **Step 4: Sweep the rename**

`use sema::` → `use hermes_sema::` across `rust/crates/*/src`, `tests`,
`examples`, `benches`, and `crates/comparison`. Hand-check: `mod sema`, a field
named `sema`, C++ `sema::` namespace references in doc comments, and
`SemContext`-related prose must NOT be rewritten. Update recorded commands in
`rust/crates/sema/tests/sema_corpus*/MANIFEST.md` (`-p sema --features dump-bin`
→ the new invocation) — live rows only; historical/frozen rows follow the
MANIFEST's existing annotation convention. Update `.github/workflows/*` if they
name `-p sema`.

- [ ] **Step 5: README, LICENSE, NOTICE**

Copy `rust/LICENSE` + `rust/NOTICE` in. Write `rust/crates/sema/README.md`:
what semantic resolution does (declaration collection, scope/binding
resolution, the validations `SemanticResolver` performs), the two entry points
and how they differ (`resolve_ast` = compile path, `resolve_ast_for_parser` =
parser path, `compile = false`), the byte-for-byte differential story against
`hermesc -dump-sema` (219 corpus files) and `sema-parser-dump` (13), and a
pointer to the façade Task 3 adds. Absolute links. No perf claims.

- [ ] **Step 6: Verify**

Workspace build + tests, zero warnings all configs, and — critically — the sema
differentials still report **219 corpus files matched (109 succeeded)** and
**13 (5 succeeded)**, now driven through the relocated binary:
```bash
REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path /home/tmikov/work/hermes-rust1/rust/Cargo.toml -p hermes-sema --test sema_differential
```
(no `--features dump-bin` any more — note the invocation change everywhere it
was recorded).

- [ ] **Step 7: Commit**

`rust(publish): rename sema to hermes-sema; move sema-dump to tools`

---

### Task 3: The `hermes-sema` façade — `resolve(ParsedJS) -> ResolvedJS`

**Files:**
- Create: `rust/crates/sema/src/facade.rs` (private module, root re-exports)
- Modify: `rust/crates/sema/src/lib.rs`, `rust/crates/sema/Cargo.toml`
- Modify: `rust/crates/parser/src/facade.rs` (minimal accessor(s), if needed)
- Create: `rust/crates/sema/examples/resolve_and_dump.rs`

**Interfaces:**
- Consumes: `hermes_parser::{ParsedJS, ParseFlags}`, `hermes_sema::resolve::{resolve_ast, resolve_ast_for_parser}`.
- Produces: `hermes_sema::resolve(...) -> Result<ResolvedJS, ResolveError>` and
  whatever minimal `ParsedJS` accessor the parser had to grow.

- [ ] **Step 1: Design within these constraints**

Mirror the parser façade's shape and quality bar (read
`rust/crates/parser/src/facade.rs` first — it is the reference for lifetime
handling, docs, and error type). Constraints:

* **Consume, don't mutate.** `resolve` takes `ParsedJS` **by value** and
  returns a `ResolvedJS` that owns the arena `Context`, the resolved root, and
  the `SemContext`. This sidesteps the "resolution returns a possibly-new root"
  problem: the resolver is a transforming visitor, so the root that comes out is
  the one callers must use.
* **Soundness is the gate.** The `Context` must not be dropped while a `NodeRc`
  is live (it panics), and the arena's `GCLock` may not be nested on one thread.
  Whatever accessor `hermes-parser` grows to hand its `Context` + root over must
  preserve those invariants and be documented with `# Panics`. Add the smallest
  such API — no new `unsafe` in either crate (`sema` is
  `#![forbid(unsafe_code)]`).
* **Both entry points reachable.** Default to the parser path
  (`resolve_ast_for_parser`: no compile-specific validation or transforms).
  Expose the compile path too (`resolve_ast`, which can fail and takes ambient
  declarations), via an options struct or a second function — your call, but
  document the difference exactly as the two C++ entry points differ.
* **Errors and diagnostics** follow `ParseError`'s precedent: structured
  `ResolvedDiagnostic`s plus rendered messages, one-line `Display`.
* `ResolvedJS` exposes at minimum: AST access (the `with_program` pattern),
  the `SemContext`, and diagnostics. Re-export what a user needs so
  `hermes-sema` + `hermes-parser` is the whole dependency set.

- [ ] **Step 2: Write it, with docs**

`missing_docs` is on; document every new public item accurately, including
`# Panics` where a lock or drop-order rule can fire. Follow the
`ParsedJS`/`ParseError` doc style.

- [ ] **Step 3: Example + doctest**

`rust/crates/sema/examples/resolve_and_dump.rs`: read a path from argv, parse,
resolve, print the sema dump (`hermes_sema::dump`) or a resolution summary.
Add a crate-level doctest in `lib.rs` that resolves `"var x = 1; x;"` and
asserts something real about the result (e.g. the declaration is found). Both
must run under `cargo test --doc -p hermes-sema` / `cargo run --example`.

- [ ] **Step 4: Prove the façade agrees with the low-level path**

Add a test asserting the façade's resolved output matches what the existing
low-level call produces for the same input (e.g. identical sema dump bytes for
a corpus file). A façade that silently diverges from the ported entry points
would be the worst failure mode here.

- [ ] **Step 5: Verify**

Zero warnings all configs; workspace tests; both sema differentials still
219/109 and 13/5; parser differentials 8/8 (the parser accessor addition must
not disturb them); doctests and the example run.

- [ ] **Step 6: Commit**

`rust(publish): add hermes-sema resolve() façade over ParsedJS`

---

### Task 4: `hermes-sema` public-API documentation audit

**Files:**
- Modify: `rust/crates/sema/src/lib.rs` and the ten public modules
  (`ast_eval`, `decl_collector`, `dump`, `dump_context`, `ids`, `keywords`,
  `libhermes`, `resolve`, `resolver`, `sem_context`)

**Interfaces:**
- Consumes: Tasks 2 and 3 (crate renamed, façade present).
- Produces: `hermes-sema` builds with `#![warn(missing_docs)]` and **zero**
  missing-docs warnings.

- [ ] **Step 1: Enable the lint and enumerate**

Add `#![warn(missing_docs)]` below the crate `//!` doc. Capture the work list:
```bash
cargo build --manifest-path /home/tmikov/work/hermes-rust1/rust/Cargo.toml -p hermes-sema 2>&1 | grep "missing documentation" | sort -u
```
Report the count before/after.

- [ ] **Step 2: Document, accurately**

This crate is ~17k LOC ported 1:1 from `lib/Sema/`. Docs must describe what the
Rust code does and may cite the C++ counterpart the way surrounding comments
already do — **verify every citation's line range against the current C++ tree**
(the tree moved when the defect fixes landed; stale citations are a known
recurring defect here). Match each file's existing comment density. Where an
item exists only to serve the differential harness or another crate, say so
plainly rather than inventing a user-facing rationale.

- [ ] **Step 3: Flag, don't change, questionable visibility**

Items that look like they should not be public (internals exposed only for the
dump binary or tests) get **listed in your report** for the controller to
decide. Do not change visibility in this task — an API-shape change needs its
own gate.

- [ ] **Step 4: Verify**

Zero missing-docs; zero warnings all configs; `cargo doc -p hermes-sema
--no-deps` with no broken intra-doc links; workspace tests + both sema
differentials green.

- [ ] **Step 5: Commit**

`rust(publish): document hermes-sema public API; enable missing_docs`

---

### Task 5: Family docs, dry-run at seven crates, gates

**Files:**
- Modify: `rust/README.md` (crate-family table, quickstart if the resolve path
  belongs there), `rust/ARCHITECTURE.md`, `rust/CHANGELOG.md`,
  `rust/CONTRIBUTING.md` (gate commands), `doc/superpowers/PUBLISH-HANDOFF.md`
  (runbook: seven crates, new order), `rust/crates/comparison/FEATURE-MATRIX.md`
  if it describes the family

**Interfaces:**
- Consumes: Tasks 1–4.
- Produces: docs describing a seven-crate family; `cargo publish --dry-run`
  passing for all seven in one invocation.

- [ ] **Step 1: Update the crate-family story**

`rust/README.md`'s family table gains `hermes-sema` and
`hermes-command-line`. The "stable public API" sentence currently names
`hermes-parser` + `hermes-ast` — extend it to include `hermes-sema`, and place
the CLI crate with the support crates. CHANGELOG: `hermes-sema` and
`hermes-command-line` are part of the 0.1.0 release; remove any line saying
sema is ported-but-unpublished (grep for it — it exists in the "Not yet
available" section). ARCHITECTURE: `sema/`, `command_line/` and `tools/` rows
must reflect the new publish/bin reality.

- [ ] **Step 2: Show the full front end in the README**

The quickstart currently ends at an AST. Add a short parse+resolve snippet
using the Task 3 façade (`hermes-parser` + `hermes-sema`), and state which one
dependency each path needs. The snippet must compile verbatim modulo nothing —
verify by pasting it into a scratch crate outside the workspace with path deps.

- [ ] **Step 3: Update the launch runbook**

`doc/superpowers/PUBLISH-HANDOFF.md`: seven crates, publication order
`hermes-unicode`, `hermes-atom-table`, `hermes-command-line`, `hermes-support`,
`hermes-ast`, `hermes-parser`, `hermes-sema`, single multi-package invocation.
Keep the "skip placeholder reservation" and post-publish checklist items.

- [ ] **Step 4: Dry-run all seven**

```bash
cargo publish --dry-run --manifest-path /home/tmikov/work/hermes-rust1/rust/Cargo.toml \
  -p hermes-unicode -p hermes-atom-table -p hermes-command-line -p hermes-support \
  -p hermes-ast -p hermes-parser -p hermes-sema
```
All seven must pack **and** verify. Inspect `cargo package --list -p hermes-sema`
for junk; the sema corpora are large — report the tarball size and contents
summary, and flag (don't unilaterally apply) an `include`/`exclude` if warranted.
Then unpack the two new tarballs to a temp dir and run `cargo test` in each to
confirm they are clean off-tree (differentials must skip without
`REQUIRE_DIFFERENTIAL`).

- [ ] **Step 5: Full gates**

Workspace tests; parser differential 8/8; json 1/1; preparse 4/4; lexer 6/6;
sema 219/109 + 13/5; zero warnings all configs + `-D warnings`; comparison
crate builds; examples and doctests run; tree clean.

- [ ] **Step 6: Commit**

`doc(rust): seven-crate publication family — sema + command-line`

---

## Self-Review

- Locked decisions honored: `hermes-command-line` (Task 1), façade in
  `hermes-sema` depending on `hermes-parser` (Task 3).
- Every published-crate requirement the earlier plan established is applied to
  both new crates: no bins (Task 2 Step 1), no `command_line` dep in a
  published lib (Task 2 Step 2 — note `hermes-command-line` is now itself
  published, so `tools` keeps using it freely), `missing_docs` (Tasks 1, 4),
  README/LICENSE/NOTICE (Tasks 1, 2), absolute links, no perf claims.
- The riskiest step is Task 3's ownership transfer between two crates' façades;
  its constraints (consume-by-value, drop-order/`GCLock` invariants, no new
  `unsafe`, agreement test vs the low-level path) are stated explicitly.
- Gate figures (219/109, 13/5, 8/8) are repeated in the tasks that could break
  them, and the invocation change (`--features dump-bin` disappears) is called
  out where it is recorded.
