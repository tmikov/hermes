# Benchmark v2 — C++ baseline, large fixture, error-recovery variants

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Make the parser performance comparison *trustworthy and interpretable* by adding (1) a **C++ Hermes baseline** (the missing reference that disambiguates "Hermes design is slower than OXC" from "our Rust port has a regression"), (2) a **multi-megabyte fixture** (amortizes per-iteration arena setup → honest steady-state throughput), and (3) **trailing-error variants** (a fairness guard: every parser must actually report the error, proving it did the full eager work the throughput implies, and exposing any lazy/short-circuit parser).

**Architecture:** Extend the existing standalone `rust/crates/comparison` harness and add a new C++ `tools/parse-bench` parse-only timer built in **Release** (the differential oracle is ASan+Debug — useless for perf). Same fixtures, same per-iteration shape (create context/allocator → parse → drop) on both sides for apples-to-apples.

**Tech stack:** criterion (Rust); a small C++ tool linking `parser::JSParser` (FullParse) built via `cmake-build-release`; pinned fixtures fetched by script.

## Global Constraints
- The `comparison` crate stays **excluded** from the main workspace; build/run with `--manifest-path rust/crates/comparison/Cargo.toml`. Main gate (`cargo build --manifest-path rust/Cargo.toml`) must stay clean.
- Fixtures are NOT committed (`fixtures/.gitignore` ignores `*.js`); the fetch script reproduces them by pinned URL.
- **Apples-to-apples shape:** both the Rust `parse_hermes` closure and the C++ tool create their context/allocator fresh per iteration, parse to AST, and drop — no shared/reused arena that would advantage one side.
- C++ parser invocation (crib: `tools/hermes-parser/hermes-parser-wasm.cpp:49,74-80`): `auto ctx = std::make_shared<Context>(); buf = ctx->getSourceErrorManager().addNewSourceBuffer(...); JSParser p(*ctx, bufId, parser::FullParse); auto r = p.parse();` Use **FullParse** (eager) to match the Rust port (which has no lazy pass yet).
- Honesty: all perf numbers remain **directional**; the C++ baseline is the headline interpretive anchor, not a marketing number.

---

### Task 1: Fixtures v2 — large file + trailing-error variants

**Files:** Modify `rust/crates/comparison/fetch_fixtures.sh`; confirm `rust/crates/comparison/fixtures/.gitignore` covers the new files.

- [ ] **Step 1: Add a multi-MB plain-JS fixture.** Append to the fetch script a pinned download of `typescript.js` (the compiled TypeScript compiler from npm — it is *plain JavaScript*, no TS syntax, ~8–9 MB): `https://cdn.jsdelivr.net/npm/typescript@5.4.5/lib/typescript.js` → `fixtures/typescript.js`. Print its size. (Keep react/jquery/three.min.)
- [ ] **Step 2: Generate trailing-error variants.** After downloading, for each fixture `F.js` create `F.err.js` = the full contents of `F.js` followed by a guaranteed top-level syntax error at EOF. Use a single appended line: `\nvar __bench_parse_error__ = ;\n` (missing right-hand side — every JS parser rejects it). Do this in the script (e.g. `printf` append) so the `.err.js` files are reproducible. They are also `*.js`, so already gitignored.
- [ ] **Step 3: Verify.** `bash rust/crates/comparison/fetch_fixtures.sh` then `ls -la rust/crates/comparison/fixtures/` shows: react/jquery/three.min/typescript `.js` (typescript ≈8–9 MB) and a `.err.js` for each. Confirm `git status` shows no fixture `.js` staged. Commit ONLY the script (+ .gitignore if changed).

---

### Task 2: C++ `parse-bench` tool + Release build → C++ baseline numbers

**Files:** Create `tools/parse-bench/parse-bench.cpp`, `tools/parse-bench/CMakeLists.txt`; modify `tools/CMakeLists.txt` (`add_subdirectory(parse-bench)`).

- [ ] **Step 1: Write the tool.** A CLI taking file paths + `--iters=N`. For each file: read bytes once; loop N timed iterations, each: `auto ctx = std::make_shared<Context>();` add the buffer to `ctx->getSourceErrorManager()`; `parser::JSParser p(*ctx, bufId, parser::FullParse);` `auto r = p.parse();` keep `r` from being optimized away (e.g. accumulate `(bool)r`); drop ctx. Time only the loop body (use `std::chrono::steady_clock`). Report per file: best/median ms and MiB/s = bytes/time. Also print whether parse succeeded (so `.err.js` files visibly fail — the C++ side's fairness guard). Mirror the include set + diag handling from `hermes-parser-wasm.cpp` (use a silent/collecting diag handler so error files don't spam). Register via `add_hermes_tool(parse-bench parse-bench.cpp ...)` mirroring `tools/js-lexer-dump/CMakeLists.txt`.
- [ ] **Step 2: Build in Release.** The background configure of `cmake-build-release` (started by the controller) should be done; then `cmake --build cmake-build-release --target parse-bench` (this pulls only parser/AST/Support deps in Release, not the full VM). If configure is incomplete, wait/re-run `cmake -B cmake-build-release -G Ninja -DCMAKE_BUILD_TYPE=Release`.
- [ ] **Step 3: Run + record.** `cmake-build-release/bin/parse-bench --iters=50 rust/crates/comparison/fixtures/*.js` (or an appropriate N per size). Record MiB/s for every fixture incl. the large one and the `.err.js` files (which must report parse failure). Put the table in the report.

---

### Task 3: Rust bench v2 — large fixture + error group + fairness assert

**Files:** Modify `rust/crates/comparison/benches/parse_throughput.rs`.

- [ ] **Step 1: Add the large fixture** `("typescript", "typescript.js")` to the fixture list.
- [ ] **Step 2: Add an error-variants benchmark group.** A second group that runs each parser over the `.err.js` files (react.err.js, jquery.err.js, three.min.err.js, typescript.err.js). Same throughput/black_box pattern.
- [ ] **Step 3: Fairness assert (the trailing-error guard).** Before benchmarking the `.err` group, run each parser once on each `.err` input and ASSERT it reports an error (our port: `.parse()` returns `None`; swc: `result.is_err()` or has parse errors; oxc: `!result.errors.is_empty()`; boa: `result.is_err()`). If any parser returns success on an `.err` file, panic with a clear message — that parser is lazy/short-circuiting and its throughput is not comparable. Document each parser's "did it error" status in the report.
- [ ] **Step 4: Run + record.** `cargo bench --manifest-path rust/crates/comparison/Cargo.toml 2>&1 | tee /tmp/bench-v2.txt`; extract a MiB/s table for valid + large + error variants, all parsers. Confirm the main gate still builds clean. Commit the bench (Cargo.lock if it changed).

---

### Task 4: Synthesize + correct the artifacts

**Files:** Modify `rust/crates/comparison/FEATURE-MATRIX.md` (perf section), `rust/README.md` (perf footnote), `doc/superpowers/blog/2026-06-28-rust-hermes-parser-DRAFT.md` (perf appendix); add a results note `rust/crates/comparison/BENCH-RESULTS.md`.

- [ ] **Step 1: Build the results table** in `BENCH-RESULTS.md`: rows = {this port (Rust), **C++ Hermes**, OXC, SWC, Boa}; columns = {react, jquery, three.min, typescript(large), + error-variant column}. Note the methodology (criterion / Release C++ tool, per-iteration arena, FullParse, fixture sizes) and which parsers actually errored on `.err` files.
- [ ] **Step 2: State the conclusion explicitly.** Compute Rust-port-vs-C++-Hermes ratio. If ≈parity → the OXC gap is inherent to Hermes' design (interning + GC AST vs OXC bump + zero-copy); say so. If the Rust port is materially slower than C++ → flag a port regression to investigate (and trigger Task 5). Update the large-fixture numbers (they amortize setup, so likely differ from the small-file numbers).
- [ ] **Step 3: Replace the "provisional / C++-baseline pending" caveats** in README/FEATURE-MATRIX/blog with the verified numbers + the C++-baseline conclusion, keeping the directional/apples-to-oranges framing. Commit.

---

### Task 5 (CONDITIONAL — only if Task 4 shows the Rust port materially slower than C++ Hermes): decomposition

**Files:** add a `lex_only`/decomposition bench to `parse_throughput.rs` or a sibling bench.

- [ ] **Step 1: Lex-only Rust bench** — run `JSLexer` to EOF (advance through all tokens) without building the AST, on the large fixture; compare to full-parse to estimate the lex (incl. interning) share vs AST-build share.
- [ ] **Step 2: Report** the breakdown and name the leading hotspot (lex/intern vs GC AST alloc). This tells whether the gap-to-C++ is a fixable port inefficiency and where. Do NOT attempt fixes in this plan — report findings for a follow-up.

---

## Self-Review
- Coverage: large fixture (T1/T3), error variants + fairness guard (T1/T3), C++ baseline (T2), synthesis+honest conclusion (T4), decomposition iff regression (T5). ✓
- Apples-to-apples: both sides create context per iteration, FullParse, same fixtures. ✓
- No fixtures committed; comparison stays excluded; main gate protected. ✓
