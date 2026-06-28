# Hermes Rust front-end — Publication Readiness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Rust port of the Hermes front-end publication-ready as an MIT `hermes-*` crate family on crates.io — metadata, license/attribution, documented public API, examples, repo docs, CI, a comparison harness vs SWC/OXC/Biome/Boa, and a blog-post draft — with the one disruptive change (the package rename) fully specified but deferred to a launch runbook.

**Architecture:** Two phases. **Phase A (Tasks 1–9)** is everything non-disruptive: it adds metadata fields, files, docs, a new `tools` crate, a new `comparison` crate, and CI — without renaming any existing package, so `cargo test -p parser …` and the parallel implementation workstream are unaffected. **Phase B (Task 10)** is the launch runbook: rename packages to `hermes-*`, finish moving the differential bins out of the library, and validate with `cargo publish --dry-run`. Phase B is specified now, executed at publish time (ideally after the implementation merges), and is partly manual (crates.io credentials).

**Tech Stack:** Rust (workspace pinned to 1.96.0 via `rust/rust-toolchain.toml`), Cargo, `criterion` for benchmarks, GitHub Actions for CI, the existing C++ `hermesc`/`ast-dump` differential oracle.

## Global Constraints

- **Source location:** the workspace is `rust/` inside the `tmikov/hermes` fork; published in-place. No separate repo. `repository = "https://github.com/tmikov/hermes"` for all crates.
- **License:** MIT for all published crates. Hermes (and juno, for copied `atom_table`/`unicode`) credited in `NOTICE`.
- **Provenance wording (verbatim, used in README banner / crate descriptions / NOTICE):** "A Rust port of the Hermes front-end by Tzvetan Mikov, the architect of Hermes. Not an official Meta project and not supported by Meta."
- **Support level (verbatim, README + CONTRIBUTING):** best-effort — "Issues and PRs are welcome and addressed as time permits. There is no SLA."
- **Published public surface:** `hermes-parser` and `hermes-ast` are the stable API (`#![warn(missing_docs)]`). `hermes-support`, `hermes-atom-table`, `hermes-unicode` are support crates published only to satisfy the dependency closure. `command_line` and the new `tools`/`comparison` crates are `publish = false`.
- **Non-disruption rule (Phase A):** do NOT rename any `[package] name`, do NOT change any `use <crate>::` path, do NOT alter the meaning of `cargo {test,build} -p parser`. Phase A is additive only.
- **Toolchain pin:** do not bump `rust/rust-toolchain.toml` (1.96.0). Any dependency added must build on 1.96.0.
- **Crates.io metadata limits:** max 5 `keywords`, each ≤20 chars; `categories` must be from the canonical slug list (`parser-implementations`, `development-tools`, `compilers`).
- **Verification gate:** every task ends green on `cargo build --manifest-path rust/Cargo.toml` with **zero warnings** and, where it touches testable code, its tests. Commit after each task.

---

### Task 1: Enrich crate metadata (non-disruptive)

Add publishing metadata to every crate's `Cargo.toml` **without renaming packages**. This is safe because `description`/`license`/`keywords`/etc. do not affect `-p` selection or `use` paths. Keep `publish = false` everywhere for now (Phase B flips it); add it to `ast` (currently missing) so nothing can be published accidentally during prep.

**Files:**
- Modify: `rust/crates/parser/Cargo.toml`
- Modify: `rust/crates/ast/Cargo.toml`
- Modify: `rust/crates/support/Cargo.toml`
- Modify: `rust/crates/atom_table/Cargo.toml`
- Modify: `rust/crates/unicode/Cargo.toml`

**Interfaces:**
- Produces: each crate's `Cargo.toml` carries `description`, `license = "MIT"`, `repository`, `authors`, `keywords`, `categories`, `readme`. Package **names stay bare** (`parser`, `ast`, …). Task 10 consumes these unchanged (only adds the `name`/`[lib]`/`package=` rename on top).

- [ ] **Step 1: Add a shared metadata block to `parser/Cargo.toml`**

Insert into the existing `[package]` table (keep `name = "parser"`, `version`, `edition`, `publish = false`):

```toml
description = "A Rust port of the Hermes JavaScript/Flow/TypeScript parser (front-end) by Tzvetan Mikov, the architect of Hermes. Not an official Meta project."
license = "MIT"
repository = "https://github.com/tmikov/hermes"
authors = ["Tzvetan Mikov <tmikov@gmail.com>"]
keywords = ["javascript", "parser", "flow", "ecmascript", "ast"]
categories = ["parser-implementations", "development-tools"]
readme = "README.md"
```

- [ ] **Step 2: Add the analogous block to `ast/Cargo.toml`**

Same fields, plus add `publish = false` (it is currently missing). Use an AST-specific `description`:

```toml
description = "ESTree-compatible AST and JSON dumper for the Hermes Rust front-end."
keywords = ["javascript", "ast", "estree", "parser", "flow"]
```
and the shared `license`/`repository`/`authors`/`categories`/`readme` from Step 1.

- [ ] **Step 3: Add metadata to the three support crates**

For `support`, `atom_table`, `unicode`: add `license = "MIT"`, `repository`, `authors`, `categories = ["development-tools"]`, and a one-line `description` each (e.g. support → "Diagnostics and source-management support crate for the Hermes Rust front-end."; atom_table → "String interner for the Hermes Rust front-end (ported from juno)."; unicode → "Unicode property tables for the Hermes Rust front-end."). Keep their existing `publish = false`. Preserve the existing `atom_table` unsafe-rationale comment.

- [ ] **Step 4: Verify the workspace still builds clean and metadata parses**

Run: `cargo build --manifest-path rust/Cargo.toml 2>&1 | tail -5`
Expected: finishes, zero warnings.
Run: `cargo metadata --manifest-path rust/Cargo.toml --no-deps --format-version 1 >/dev/null && echo OK`
Expected: `OK` (metadata is valid TOML/JSON).

- [ ] **Step 5: Commit**

```bash
git add rust/crates/*/Cargo.toml
git commit -m "rust(publish): add crate metadata (description/license/keywords) — non-disruptive

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: License & attribution files

Provide the MIT license and a `NOTICE` crediting Hermes and juno. The repo root already has a `LICENSE` (the Hermes MIT license); add one scoped to `rust/` plus attribution so the published crates carry correct provenance.

**Files:**
- Create: `rust/LICENSE` (MIT, copyright Tzvetan Mikov; retains Hermes/Meta MIT terms)
- Create: `rust/NOTICE`

**Interfaces:**
- Produces: `rust/LICENSE` + `rust/NOTICE`. Task 5's README links to them; Task 10's `cargo publish --dry-run` packages `LICENSE`.

- [ ] **Step 1: Create `rust/LICENSE`**

Copy the existing repo-root `LICENSE` text (MIT) verbatim into `rust/LICENSE` so each crate has a license file in its package tree. (crates.io reads `license = "MIT"` from metadata; the file is for the package tarball and GitHub.)

Run: `cp LICENSE rust/LICENSE && head -3 rust/LICENSE`
Expected: the MIT header.

- [ ] **Step 2: Create `rust/NOTICE`**

```text
Hermes Rust front-end
=====================

A Rust port of the Hermes front-end by Tzvetan Mikov, the architect of Hermes.
Not an official Meta project and not supported by Meta.

This work is a faithful port of the Hermes JavaScript engine front-end:
  https://github.com/facebook/hermes  (MIT License, (c) Meta Platforms, Inc.)

The `atom_table` and `unicode` crates are derived from the juno project:
  https://github.com/facebook/hermes/tree/main/unsupported/juno  (MIT License)

Licensed under the MIT License. See LICENSE.
```

- [ ] **Step 3: Verify and commit**

Run: `test -f rust/LICENSE && test -f rust/NOTICE && echo OK`
Expected: `OK`

```bash
git add rust/LICENSE rust/NOTICE
git commit -m "rust(publish): add LICENSE and NOTICE (Hermes + juno attribution)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: Public-API audit for `hermes-ast`

Turn on `missing_docs` for the AST crate and document every public item it exposes. Split from the parser audit (Task 4) so a reviewer can gate them independently. Do this on the bare `ast` crate (no rename yet).

**Files:**
- Modify: `rust/crates/ast/src/lib.rs` (add the lint attribute)
- Modify: the public modules under `rust/crates/ast/src/` flagged by the lint

**Interfaces:**
- Consumes: nothing.
- Produces: `ast` builds with `#![warn(missing_docs)]` and **zero missing-docs warnings**. Generated files (`node.rs`) must get their docs from the generator, not by hand-editing the `@generated` output.

- [ ] **Step 1: Add the lint and see the gaps**

Add to the top of `rust/crates/ast/src/lib.rs`:
```rust
#![warn(missing_docs)]
//! ESTree-compatible AST for the Hermes Rust front-end.
//!
//! See the crate `README` and `ARCHITECTURE.md` for the GC-arena design and the
//! `ESTree.def`-driven node generation.
```

- [ ] **Step 2: Enumerate the warnings**

Run: `cargo build --manifest-path rust/Cargo.toml -p ast 2>&1 | grep -c "missing documentation"`
Expected: a count > 0 (the work list). Capture the list:
`cargo build --manifest-path rust/Cargo.toml -p ast 2>&1 | grep "missing documentation" | sort -u`

- [ ] **Step 3: Document hand-written public items**

For each non-generated public item (modules `context`, `dump`, the hand-written parts of the visitor), add a doc comment describing what it is and how it is used. Match the existing doc-comment density and style in the file.

- [ ] **Step 4: Document generated items at the generator**

For warnings in `rust/crates/ast/src/node.rs` (the `// @generated` file), add the doc strings in `rust/crates/ast/gen_nodes.py` (emit `///` lines), then regenerate:
```bash
python3 rust/crates/ast/gen_nodes.py
REQUIRE_GEN=1 cargo test --manifest-path rust/Cargo.toml -p ast --test generated_idempotent
```
Expected: idempotency test passes (committed `node.rs` matches the generator).

- [ ] **Step 5: Verify zero missing-docs warnings and tests pass**

Run: `cargo build --manifest-path rust/Cargo.toml -p ast 2>&1 | grep -c "missing documentation"`
Expected: `0`
Run: `cargo test --manifest-path rust/Cargo.toml -p ast 2>&1 | tail -3`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add rust/crates/ast/
git commit -m "rust(publish): document hermes-ast public API; enable missing_docs

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 4: Public-API audit for `hermes-parser`

Same treatment for the parser crate. Scope `missing_docs` to the **library** only (the bins move out in Task 6/10; do not document the soon-to-move bins here).

**Files:**
- Modify: `rust/crates/parser/src/lib.rs`
- Modify: public modules under `rust/crates/parser/src/` flagged by the lint (`js`, `lexer`, `json`, token/number public types)

**Interfaces:**
- Consumes: nothing.
- Produces: `parser` library builds with `#![warn(missing_docs)]` and zero missing-docs warnings. The differential gate still passes.

- [ ] **Step 1: Add the lint + crate doc**

Add to the top of `rust/crates/parser/src/lib.rs`:
```rust
#![warn(missing_docs)]
//! A Rust port of the Hermes JavaScript/Flow/TypeScript parser (front-end).
//!
//! Faithful 1:1 port of the C++ Hermes `JSLexer`/`JSParser`, validated
//! byte-for-byte against `hermesc -dump-ast`. Produces an ESTree-compatible AST
//! (see the `hermes-ast` crate). Flow is fully supported; TypeScript and JSX are
//! in progress (see the support matrix in the README).
```

- [ ] **Step 2: Enumerate and document**

Run: `cargo build --manifest-path rust/Cargo.toml -p parser 2>&1 | grep "missing documentation" | sort -u`
Document every listed public item in the library modules, matching surrounding style. Leave generated files' docs to their generators if any.

- [ ] **Step 3: Verify zero warnings + differential still green**

Run: `cargo build --manifest-path rust/Cargo.toml -p parser 2>&1 | grep -c "missing documentation"`
Expected: `0`
Run (oracle must be built — `cmake --build cmake-build-asan --target ast-dump` first):
`REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p parser --test parser_differential 2>&1 | tail -3`
Expected: all corpora pass.

- [ ] **Step 4: Commit**

```bash
git add rust/crates/parser/
git commit -m "rust(publish): document hermes-parser public API; enable missing_docs

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: Examples with doctests

Add runnable examples that double as docs.rs landing content and `cargo test --doc` coverage. Examples reference the crates by their **current bare names** (`parser`, `ast`); Task 10 fixes the names if it renames lib targets.

**Files:**
- Create: `rust/crates/parser/examples/parse_to_estree_json.rs`
- Create: `rust/crates/parser/examples/walk_ast.rs`
- Modify: `rust/crates/parser/src/lib.rs` (add a `//!` example block that is a doctest)

**Interfaces:**
- Consumes: the public parse entry points documented in Task 4 (use the exact signatures from `ast_dump.rs`: `JSParserImpl`, `JSLexer`, `GrammarContext`, `SourceErrorManager`, `dump_estree_json_with_sm`).
- Produces: two `examples/` binaries + one crate-level doctest, all green.

- [ ] **Step 1: Write `parse_to_estree_json.rs`**

A `main` that reads a `.js` path from argv, parses it, and prints ESTree JSON — mirroring the minimal path in `crates/parser/src/bin/ast_dump.rs` but without `command_line` (use `std::env::args`). Keep it ~30 lines.

- [ ] **Step 2: Write `walk_ast.rs`**

A `main` that parses a snippet and walks the AST with the read `Visitor`, counting node kinds, printing a small histogram. Demonstrates the visitor API.

- [ ] **Step 3: Add a crate-level doctest**

In `lib.rs`, a fenced ```` ```rust ```` block that parses `"1 + 2;"` and asserts the program has one statement. Make it compile-and-run under `cargo test --doc`.

- [ ] **Step 4: Verify**

Run: `cargo run --manifest-path rust/Cargo.toml -p parser --example walk_ast 2>&1 | tail -5`
Expected: prints a histogram, exits 0.
Run: `cargo test --manifest-path rust/Cargo.toml -p parser --doc 2>&1 | tail -3`
Expected: doctest passes.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/parser/examples/ rust/crates/parser/src/lib.rs
git commit -m "rust(publish): add examples + crate doctest (parse, ESTree JSON, walk)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 6: Extract the differential/CLI bins into an unpublished `tools` crate

Move the three bins (`ast-dump`, `json-parse-dump`, `gen-json`) out of `parser` into a new `publish = false` crate so the published library has no `command_line` dependency and ships no binaries. This is non-disruptive to *behavior* — the bins keep their names and the differential test invokes `ast-dump` the same way — but it removes the publish blocker. **Update the differential test's binary-resolution path if it locates the bin via the parser crate's target dir.**

**Files:**
- Create: `rust/crates/tools/Cargo.toml`
- Create: `rust/crates/tools/src/bin/ast_dump.rs` (moved)
- Create: `rust/crates/tools/src/bin/json_parse_dump.rs` (moved)
- Create: `rust/crates/tools/src/bin/gen_json.rs` (moved)
- Modify: `rust/Cargo.toml` (add `crates/tools` to `members`)
- Modify: `rust/crates/parser/Cargo.toml` (remove the three `[[bin]]` entries and the `command_line` dependency)
- Modify: `rust/crates/parser/tests/parser_differential.rs` and `tests/json_differential.rs` (point binary lookup at the `tools` package target if needed)

**Interfaces:**
- Consumes: `parser`, `ast`, `support`, `atom_table`, `unicode`, `command_line`, `bumpalo` (the bins' existing deps — now `tools`'s deps).
- Produces: `tools` crate building all three bins; `parser` library with no bins and no `command_line` dep. Differential tests still pass and still find `ast-dump`.

- [ ] **Step 1: Create `tools/Cargo.toml`**

```toml
[package]
name = "tools"
version = "0.0.0"
edition = "2021"
publish = false

[[bin]]
name = "ast-dump"
path = "src/bin/ast_dump.rs"
[[bin]]
name = "json-parse-dump"
path = "src/bin/json_parse_dump.rs"
[[bin]]
name = "gen-json"
path = "src/bin/gen_json.rs"

[dependencies]
parser = { path = "../parser" }
ast = { path = "../ast" }
support = { path = "../support" }
atom_table = { path = "../atom_table" }
unicode = { path = "../unicode" }
command_line = { path = "../command_line" }
bumpalo = "3.16"
```

- [ ] **Step 2: Move the bin sources**

```bash
git mv rust/crates/parser/src/bin/ast_dump.rs rust/crates/tools/src/bin/ast_dump.rs
git mv rust/crates/parser/src/bin/json_parse_dump.rs rust/crates/tools/src/bin/json_parse_dump.rs
git mv rust/crates/parser/src/bin/gen_json.rs rust/crates/tools/src/bin/gen_json.rs
```
(Create `rust/crates/tools/src/bin/` first.) Their `use parser::`, `use ast::`, `use command_line::` lines work unchanged because the bins now depend on those crates from `tools`.

- [ ] **Step 3: Add `tools` to the workspace and clean up `parser/Cargo.toml`**

Edit `rust/Cargo.toml` `members` to include `"crates/tools"`. In `rust/crates/parser/Cargo.toml`, delete the three `[[bin]]` blocks and remove the `command_line = { path = "../command_line" }` dependency line.

- [ ] **Step 4: Fix differential binary resolution if needed**

Inspect how `tests/parser_differential.rs` finds `ast-dump`:
`grep -n "ast-dump\|ast_dump\|CARGO_BIN\|target" rust/crates/parser/tests/parser_differential.rs`
If it relies on `CARGO_BIN_EXE_ast-dump` (only set for bins in the *same* crate), switch it to locate the binary in the workspace target dir (`env!("CARGO_MANIFEST_DIR")/../../target/<profile>/ast-dump`) or build `tools` first and reference that path. Apply the same fix to `tests/json_differential.rs`.

- [ ] **Step 5: Verify the library lost its bins/dep and tests pass**

Run: `cargo build --manifest-path rust/Cargo.toml -p parser 2>&1 | tail -3`
Expected: builds; `parser` has no bins (`cargo build -p parser --bins` produces nothing).
Run: `cargo build --manifest-path rust/Cargo.toml -p tools 2>&1 | tail -3`
Expected: all three bins build.
Run (oracle built):
`REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p parser --test parser_differential 2>&1 | tail -3`
Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add rust/Cargo.toml rust/crates/parser/Cargo.toml rust/crates/tools/ rust/crates/parser/tests/
git commit -m "rust(publish): move differential/CLI bins to unpublished tools crate

Removes the command_line dependency from the publishable parser library so
hermes-parser ships as a pure library.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 7: Repo documentation (README, ARCHITECTURE, CHANGELOG, CONTRIBUTING)

Author the human-facing docs, distilled from `RustPortRoadmap.md` and the specs. The README is `hermes-parser`'s docs.rs/readme target referenced in Task 1.

**Files:**
- Create: `rust/README.md`
- Create: `rust/ARCHITECTURE.md`
- Create: `rust/CHANGELOG.md`
- Create: `rust/CONTRIBUTING.md`
- Create: `rust/crates/parser/README.md` and `rust/crates/ast/README.md` (short, point to `rust/README.md`)

**Interfaces:**
- Consumes: the feature matrix produced in Task 8 (link to it from the README).
- Produces: the four repo docs + two crate READMEs. README leads with the provenance banner (Global Constraints) and the support-level statement.

- [ ] **Step 1: Write `README.md`**

Sections: provenance banner (verbatim wording from Global Constraints) → one-paragraph what/why → **support matrix** (JS ✅, Flow ✅, TypeScript 🚧, JSX 🚧 — keep in sync with reality) → quickstart (`cargo add hermes-parser` + a 10-line parse example) → "Why this parser" (faithful port, differential testing, Flow, ESTree) → link to the comparison (Task 8) → support level (verbatim) → license. Keep claims honest: mark in-progress features as such.

- [ ] **Step 2: Write `ARCHITECTURE.md`**

Distill from `doc/superpowers/RustPortRoadmap.md` and the specs: crate map (the 5 + tools), the GC-arena AST (juno lineage), faithful-port conventions (templates→generics, RAII→guards, cursor decision), and a prominent **"Differential testing methodology"** section (byte-for-byte vs `hermesc -dump-ast`, the corpora, the gate command). Do not restate the whole roadmap; link to it.

- [ ] **Step 3: Write `CHANGELOG.md`**

Keep-a-Changelog format with an `## [Unreleased]` section listing the current capabilities. No version dates yet (set at launch).

- [ ] **Step 4: Write `CONTRIBUTING.md`**

How to build/test (the commands from the handoff §3), the differential-gate requirement for parser changes, the faithful-port conventions (point to ARCHITECTURE), and the **support level** statement (verbatim).

- [ ] **Step 5: Write the two crate READMEs**

3–5 lines each + a link to `../../README.md`. These satisfy the `readme = "README.md"` metadata per crate (adjust the `readme` path in Task 1 if pointing at the crate-local file).

- [ ] **Step 6: Verify links and commit**

Run: `grep -RoE "\]\([^)]+\.md\)" rust/*.md rust/crates/parser/README.md rust/crates/ast/README.md` and eyeball that each target exists.
```bash
git add rust/README.md rust/ARCHITECTURE.md rust/CHANGELOG.md rust/CONTRIBUTING.md rust/crates/parser/README.md rust/crates/ast/README.md
git commit -m "rust(publish): add README, ARCHITECTURE, CHANGELOG, CONTRIBUTING

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 8: Comparison harness + feature matrix

A `publish = false` `comparison` crate that benchmarks parse-to-AST throughput vs SWC/OXC/Biome/Boa over a shared corpus, plus a hand-built-but-source-verified feature matrix. Perf is secondary and **caveated**; the matrix is the headline.

**Files:**
- Create: `rust/crates/comparison/Cargo.toml`
- Create: `rust/crates/comparison/benches/parse_throughput.rs`
- Create: `rust/crates/comparison/fixtures/.gitignore` + `rust/crates/comparison/fetch_fixtures.sh`
- Create: `rust/crates/comparison/FEATURE-MATRIX.md`
- Modify: `rust/Cargo.toml` (add `crates/comparison` to the workspace **`exclude`** list — NOT `members`)

**CRITICAL — workspace isolation:** `comparison` pulls heavy external parsers (OXC, Biome, SWC, Boa). It must be **excluded** from the main workspace so those deps cannot break `cargo build --manifest-path rust/Cargo.toml` (the gate every other task and CI uses) and so it resolves its own `Cargo.lock`. Achieve this by (a) adding `exclude = ["crates/comparison"]` to the `[workspace]` table in `rust/Cargo.toml`, and (b) giving `rust/crates/comparison/Cargo.toml` its own empty `[workspace]` table so it is a standalone workspace root. Path deps (`parser = { path = "../parser" }`) still work across workspace boundaries. Build/run it with its own manifest: `--manifest-path rust/crates/comparison/Cargo.toml` (NOT `-p comparison` against the root).

**This task is split into two sequential sub-agents:** Part A = Steps 1–4 + Step 6 (the harness: crate, fixtures, benches, run, commit). Part B = Step 5 (the feature matrix, committed separately).

**Interfaces:**
- Consumes: `parser`/`ast` (this port), and `swc_ecma_parser`, `oxc_parser`+`oxc_allocator`, `biome_js_parser`+`biome_js_syntax`, `boa_parser` as dev/bench deps.
- Produces: a `criterion` bench group with one function per parser over each fixture; `FEATURE-MATRIX.md` with version-pinned cells; README (Task 7) links to both.

- [ ] **Step 1: Scaffold the crate (pin compatible versions for Rust 1.96.0)**

```toml
[package]
name = "comparison"
version = "0.0.0"
edition = "2021"
publish = false

# Standalone workspace root: isolates the heavy external-parser deps and lock
# from the main rust/ workspace. Build with --manifest-path on THIS file.
[workspace]

[dependencies]
parser = { path = "../parser" }
ast = { path = "../ast" }
support = { path = "../support" }
atom_table = { path = "../atom_table" }

[dev-dependencies]
criterion = "0.5"
swc_ecma_parser = "*"      # Step 1a: resolve each to a concrete version
swc_common = "*"           # (SWC needs a SourceMap/Lrc to parse)
oxc_parser = "*"
oxc_allocator = "*"
oxc_span = "*"             # SourceType for oxc_parser::Parser::new
biome_js_parser = "*"
biome_js_syntax = "*"
boa_parser = "*"
boa_interner = "*"         # boa_parser needs an Interner

[[bench]]
name = "parse_throughput"
harness = false
```
**Step 1a:** resolve each `"*"` to a concrete version. From the comparison dir's own manifest: `cargo generate-lockfile --manifest-path rust/crates/comparison/Cargo.toml` then `cargo build --manifest-path rust/crates/comparison/Cargo.toml --benches`. rustc here is **1.96.0 (2026-05-25)** — newer than these crates' MSRVs, so version conflicts are unlikely; if one still fails to build, pin it down to the last compatible release and record the version. If a competitor genuinely cannot build, drop it from the *bench* and note "not benchmarked (build)" — do NOT let it block the task. Record the final chosen versions (write them into a comment at the top of the bench file; Part B's matrix will cite them).

- [ ] **Step 2: Write `fetch_fixtures.sh`**

A script that downloads a fixed **plain-JavaScript** corpus into `fixtures/` by pinned URL + version: `react.development.js`, `jquery.js`, and one large minified bundle (e.g. a pinned `vue.global.js` or `three.min.js`). **Do NOT include TypeScript or JSX fixtures** — this port's TS/JSX are still in progress and would error; note in the script's header comment that TS/JSX fixtures are a follow-up once those land. `fixtures/.gitignore` ignores `*.js` (don't vendor large third-party files; the script reproduces them). Print sizes at the end.

- [ ] **Step 3: Write the benchmark**

`benches/parse_throughput.rs`: load each fixture once; a `criterion_group` with one `bench_function` per (parser, fixture) parsing source → that parser's native AST. Add a top-of-file comment with the **apples-to-oranges caveat** (different AST/CST shapes do different work). Use `Throughput::Bytes(len)` so criterion reports MB/s.

- [ ] **Step 4: Run benches (informational; not a pass/fail gate)**

Run: `bash rust/crates/comparison/fetch_fixtures.sh`
Run: `cargo bench --manifest-path rust/crates/comparison/Cargo.toml 2>&1 | tail -40`
Expected: criterion prints throughput per parser/fixture. **Record the numbers** (paste a summary table into the Part A report). (If this port is not fastest, that is fine and expected — see spec §4.)

- [ ] **Step 5: Write `FEATURE-MATRIX.md`**

A markdown table: rows = {this port, SWC, OXC, Biome, Boa}; columns = {ECMAScript, JSX, TypeScript, **Flow**, AST model (ESTree?/own/CST), error recovery, comments+locations, allocator model, conformance methodology, maturity}. **Verify every cell against each project's current docs/source** (cite the version checked). Lead the surrounding prose with the three differentiators (faithful port + differential testing + Flow). Pin the competitor versions used.

- [ ] **Step 6: Commit (Part A — harness only; matrix lands in Part B)**

```bash
git add rust/Cargo.toml rust/crates/comparison/
git commit -m "rust(publish): add comparison benchmark harness vs SWC/OXC/Biome/Boa (excluded crate)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```
Note: `rust/crates/comparison/Cargo.lock` is generated; commit it (it's the standalone crate's lock, useful for reproducible benches). Do NOT commit `fixtures/*.js`.

---

### Task 9: CI workflow

GitHub Actions: Rust workspace tests on every push (fast), the differential gate nightly (slow — builds `hermesc`). Uses the pinned toolchain.

**Files:**
- Create: `.github/workflows/rust.yml`
- Create: `.github/workflows/rust-differential-nightly.yml`

**Interfaces:**
- Consumes: the test commands from the handoff §3 and Task 6's `tools` bin names.
- Produces: two workflows. Validate locally (can't run Actions here) by asserting the commands they invoke pass.

- [ ] **Step 1: Write `rust.yml`**

Trigger `on: [push, pull_request]`. Steps: checkout → `dtolnay/rust-toolchain` honoring `rust/rust-toolchain.toml` → cache → `cargo build --manifest-path rust/Cargo.toml` (assert zero warnings via `RUSTFLAGS="-D warnings"`) → `cargo test --manifest-path rust/Cargo.toml` (the Rust suite; the differential tests skip without `REQUIRE_DIFFERENTIAL`) → `cargo clippy -p parser`.

- [ ] **Step 2: Write `rust-differential-nightly.yml`**

Trigger `on: schedule: - cron: '0 6 * * *'` + `workflow_dispatch`. Steps: checkout → install CMake/Ninja + toolchain → configure & build the oracle (`cmake -B cmake-build-asan … && cmake --build cmake-build-asan --target ast-dump json-parse-dump`) → `REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p parser --test parser_differential --test json_differential`. Cache the cmake build dir to amortize the engine build.

- [ ] **Step 3: Validate the commands the workflows run**

Run locally (the on-push set):
`RUSTFLAGS="-D warnings" cargo build --manifest-path rust/Cargo.toml 2>&1 | tail -3`
Expected: builds, no warnings (so CI won't fail spuriously).
Run: `cargo test --manifest-path rust/Cargo.toml 2>&1 | tail -3`
Expected: pass (differential auto-skips without the env var).
Lint the YAML: `python3 -c "import yaml,sys; [yaml.safe_load(open(f)) for f in sys.argv[1:]]; print('YAML OK')" .github/workflows/rust.yml .github/workflows/rust-differential-nightly.yml`
Expected: `YAML OK`.

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/rust.yml .github/workflows/rust-differential-nightly.yml
git commit -m "rust(publish): CI — Rust tests on push, differential gate nightly

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 10 (LAUNCH RUNBOOK — deferred; partly manual): rename to `hermes-*`, dry-run, reserve names, publish

> **Execute at publish time, ideally after the implementation workstream has merged**, because the package rename changes `cargo {test,build} -p parser` → `-p hermes-parser`. Steps that touch crates.io are **manual** (require your crates.io token; publishing is irreversible — names can only be yanked, not deleted). This task is fully specified now so launch is mechanical.

**Files:**
- Modify: `rust/crates/{parser,ast,support,atom_table,unicode}/Cargo.toml` (rename `[package] name`, add `[lib] name`, alias path deps)
- Modify: `rust/crates/tools/Cargo.toml` and `rust/crates/comparison/Cargo.toml` (alias their path deps to the renamed packages)
- Create (scratch, outside the workspace): placeholder crates for name reservation

**Interfaces:**
- Consumes: all prior tasks (metadata, docs, bins extracted, no `command_line` dep in the library).
- Produces: a workspace whose published crates are named `hermes-*` and pass `cargo publish --dry-run`; reserved names on crates.io.

- [ ] **Step 1: Rename packages AND lib names to the `hermes_*` family (DECIDED: Option B — clean import path)**

**Decision (locked 2026-06-28):** publish with the conventional Rust experience — users `cargo add hermes-parser` and write `use hermes_parser::`. This requires a full lib-name rename, NOT the short-name aliasing. **Run this ONLY after the parallel `rust` implementation workstream has merged**, because it churns ~49 files and would otherwise conflict.

For each library crate, set both the package name and the matching `hermes_*` lib name (lib names use underscores):
```toml
# parser/Cargo.toml
[package]
name = "hermes-parser"
# (no [lib] name override — defaults to hermes_parser)
```
Apply analogously: `ast`→`hermes-ast` (lib `hermes_ast`), `support`→`hermes-support` (lib `hermes_support`), `atom_table`→`hermes-atom-table` (lib `hermes_atom_table`), `unicode`→`hermes-unicode` (lib `hermes_unicode`).

Then do the one-time mechanical rename of every intra-workspace import + path-dep key across `rust/` (the ~49 files). Use a scripted `sed` over `rust/crates/*/src` and `rust/crates/*/tests` and the `tools`/`comparison` crates:
```bash
# imports + paths in code  (run from repo root; word-boundary anchored)
grep -rl --include='*.rs' -E '\b(parser|ast|support|atom_table|unicode)::' rust/crates | while read f; do
  sed -i -E 's/\bparser::/hermes_parser::/g; s/\bast::/hermes_ast::/g; s/\bsupport::/hermes_support::/g; s/\batom_table::/hermes_atom_table::/g; s/\bunicode::/hermes_unicode::/g' "$f"
done
```
Also update every `extern`/path-dep KEY in the dependent Cargo.tomls so the dependency name matches the new lib name (e.g. in `parser/Cargo.toml`: `ast = { path = "../ast" }` → `hermes_ast = { path = "../ast", package = "hermes-ast" }`; repeat for `support`/`atom_table`/`unicode`, and in `ast`, `tools`, `comparison`). After this, code reads `use hermes_ast::…` and deps resolve by package name. **Caveat:** hand-check the `sed` results — a bare `use ast;` or `mod ast` (module, not crate) or a struct field named `parser` must NOT be rewritten; the `::` anchor avoids most false hits but review the diff. Record in CHANGELOG that the public import paths are `hermes_parser` / `hermes_ast`.

- [ ] **Step 2: Verify the renamed workspace builds and tests pass**

Run: `cargo build --manifest-path rust/Cargo.toml 2>&1 | tail -3` → zero warnings.
Run: `cargo test --manifest-path rust/Cargo.toml -p hermes-parser 2>&1 | tail -3` → pass (note the new `-p` name).
Run (oracle built): `REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p hermes-parser --test parser_differential 2>&1 | tail -3` → pass.

- [ ] **Step 3: Flip `publish` and set versions on the five published crates**

Set `version = "0.1.0"` (or the agreed launch version) and remove `publish = false` on `hermes-parser`, `hermes-ast`, `hermes-support`, `hermes-atom-table`, `hermes-unicode`. Leave `publish = false` on `tools` and `comparison`. Change inter-crate path deps to `{ path = "...", version = "0.1.0", package = "hermes-…" }` (crates.io requires a version on path deps that are also published).

- [ ] **Step 4: Dry-run in dependency order**

Run, in order (each must succeed):
```bash
for c in hermes-unicode hermes-atom-table hermes-support hermes-ast hermes-parser; do
  cargo publish --dry-run --manifest-path rust/Cargo.toml -p "$c" || break
done
```
Expected: each prints "Packaging"/"Verifying"/"Uploading (dry run)" with no errors. Fix any "all path dependencies must have a version" / missing-field errors and re-run.

- [ ] **Step 5: (MANUAL) Reserve the `hermes-*` names now via placeholder crates**

Outside the workspace, for each name not yet ready to publish for real, create a throwaway crate and publish a `0.0.0` placeholder so the name is claimed (irreversible — do this deliberately):
```bash
# requires: cargo login <your crates.io token>
tmp=$(mktemp -d); cargo new --lib "$tmp/hermes-parser-reserve"
# edit Cargo.toml: name="hermes-parser", version="0.0.0",
#   description="Reserved for the Hermes Rust front-end (see https://github.com/tmikov/hermes).",
#   license="MIT", repository="https://github.com/tmikov/hermes"
( cd "$tmp/hermes-parser-reserve" && cargo publish )
```
Repeat for `hermes-ast`, `hermes-support`, `hermes-atom-table`, `hermes-unicode`. (Once the real crates are ready, publishing the real `0.1.0` supersedes the placeholder.)

- [ ] **Step 6: (MANUAL, at real launch) Publish for real**

When ready (out of scope for "prepare now"): `cargo publish -p <crate>` in the Step 4 dependency order, verifying each appears on crates.io/docs.rs before the next.

- [ ] **Step 7: Commit the rename (the manual publish steps produce no repo changes)**

```bash
git add rust/
git commit -m "rust(publish): rename crates to hermes-* family; publish-ready metadata

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 11: Blog post draft (three candidate angles)

Capture all three narratives so the lead can be chosen at drafting time once Task 8's data exists.

**Files:**
- Create: `doc/superpowers/blog/2026-06-19-rust-hermes-parser-DRAFT.md`

**Interfaces:**
- Consumes: Task 8's comparison data + feature matrix; the ARCHITECTURE differential-testing section.
- Produces: one draft file with three angle outlines + a shared "facts/data" appendix.

- [ ] **Step 1: Write the draft skeleton**

Three top-level outlines — (1) Faithful-port methodology, (2) The only complete Flow parser in Rust, (3) AI/subagent-driven port — each with a hook, 4–6 section beats, and which facts/figures it needs. Add a shared appendix: the support matrix, the comparison numbers (from Task 8), and the differential-testing method. Mark venue as TBD.

- [ ] **Step 2: Verify and commit**

Run: `test -f doc/superpowers/blog/2026-06-19-rust-hermes-parser-DRAFT.md && echo OK`
```bash
git add doc/superpowers/blog/2026-06-19-rust-hermes-parser-DRAFT.md
git commit -m "doc(rust): blog post draft — three candidate angles + data appendix

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Self-Review

**Spec coverage** (spec §1–§9 → tasks):
- §2 crate family/structure → Tasks 1, 6, 10 ✓
- §3 docs surface (rustdoc/repo/ARCHITECTURE) → Tasks 3, 4, 5, 7 ✓
- §4 comparison (matrix + caveated perf) → Task 8 ✓
- §5 readiness roadmap items 1–9 → Tasks 1 (metadata), 2 (license), 3+4 (API audit), 5 (examples)/4 (rustdoc), 7 (repo docs), 9 (CI), 8 (harness), 10 (dry-run + reservation), 11 (blog) ✓
- §6 blog (three angles) → Task 11 ✓
- §7 risks: naming → Task 7 banner + Task 10; perf → Task 8 caveats; maintenance → Task 7 support level; copied-code licensing → Task 2 NOTICE ✓
- Decisions table: best-effort support → Tasks 5/7; reserve names early → Task 10 Step 5; nightly/cached differential → Task 9 ✓

**Placeholder scan:** the only `"*"` versions (Task 8) are explicitly resolved in Step 1a; the `command_line`-dep-blocker and differential-bin-resolution are concrete steps (Task 6). No TBD/"handle edge cases" left except the deliberately-deferred launch version number and blog venue (both flagged as launch-time decisions per the spec).

**Type/name consistency:** crate names are bare (`parser`/`ast`/…) through Tasks 1–9 and renamed to `hermes-*` only in Task 10; `-p parser` is used in Tasks 3–9 and `-p hermes-parser` only after the Task 10 rename — consistent with the non-disruption rule. Bin names (`ast-dump`, `json-parse-dump`, `gen-json`) are stable across Task 6's move.

**Known deliberate deferrals:** launch version and blog venue. (The lib-naming question is now DECIDED — Option B, full `use hermes_parser::` rename — see Task 10 Step 1.)
