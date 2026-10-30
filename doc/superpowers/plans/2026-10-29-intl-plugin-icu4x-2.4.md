# Intl Plugin: ICU4X 2.4 / CLDR 49 Upgrade Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the plugin's Rust core from ICU4X 2.3 (CLDR 48.2) to ICU4X 2.4 (CLDR 49, Unicode 18, tzdb 2026e) together with the plugin's own CLDR-derived data, so that both come from CLDR 49, fixing the quadratic word segmentation (dz 01a17bdc) and closing dz 01a18652-9cc4.

**Architecture:** The ICU4X crates move as one set (Cargo keeps one 2.x version per crate). The generator `intl/core/data/update-cldr-lists.py` learns CLDR 49's format changes and regenerates every data file from `release-49` / npm `49.0.0`; `locales.txt` follows ICU4X 2.4's compiled-data locales. Word segmentation drops the workaround 2.4 makes unnecessary. test262 expectations, sizes, docs and the spec follow.

**Tech Stack:** Rust (ICU4X 2.4.0, icu_experimental 0.7.0), Python 3 (the generator), JS (the plugin's ECMA-402 layer), CMake/lit/test262.

**Spec:** `doc/superpowers/specs/2026-10-22-intl-plugin-design.md` ("Build" version pin, "Data", "Segmenter", "Risks"). Task 5 writes this plan's decisions into it.

**Research:** `/home/tmikov/work/hermes-intl/.superpowers/p8-research/icu4x-2.4.md` — a trial upgrade with every API change and its fix, cargo test results, the segmentation bug status, data-gap status, sizes, and the generator's three CLDR 49 stops; `trial-code.patch` next to it has the trial's code changes. Implementers start from it; the plan's decisions win.

## Global Constraints

- **Worktree:** `/home/tmikov/work/hermes-intl`, branch `hermes-intl`. **Never `cd`**; absolute paths; a subshell `( cd x; cmd )` only when a tool insists.
- **Build directories:** `/home/tmikov/work/hermes-intl/cmake-build-asan` is the host ASan build; do not reconfigure it by hand. Other builds under `.superpowers/p8-builds/<name>` or the existing `p6-builds`/`msvc-builds` directories.
- **Commands (host):** `check-intl` (incl. plugin-mode test262), `check-hermes`, `check-shermes`, `cargo test` in `intl/core`.
- **Git:** commit each task's files with a plain-English message ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Never push, amend or reset.
- **Intermediate state:** Tasks 1–3 may leave data-dependent tests failing (ICU4X 2.4 code with CLDR 48.2 files, or the reverse); each task report lists every failing test and why. From Task 3 on, `cargo test` passes; from Task 4 on, `check-intl` passes with the re-baselined expectations; `check-hermes` and `check-shermes` keep 0 unexpected throughout.
- **Prove checks can fail:** every new or changed test is shown failing under a stated mutation, named in the report.
- **No new `UNEXPLAINED`:** every test262 outcome change gets a real reason tied to a CLDR 49, Unicode 18 or ICU4X 2.4 change.
- **Never bump `BYTECODE_VERSION`.**
- **Network:** the generator downloads from GitHub (raw files and one API call) and npm; that is expected.
- **Installs:** `rustup toolchain install 1.88 --profile minimal` (user-local) for the MSRV check; nothing else.
- **The Mac:** Task 4 writes "Mac commands"; the controller posts them.
- **Docs rule:** documentation holds only what is not obvious from the source (README is a short introduction; no history).

## Planning decisions

1. **Versions.** `icu ~2.4.0`, every `icu_*` used `~2.4`, `icu_experimental ~0.7`, `icu_pattern ~0.5.1`; others unchanged. A targeted `cargo update -p <icu crates>` keeps the lock diff to ICU4X and what it pulls in. `rust-version` stays 1.88 (every 2.4 crate declares 1.88); confirmed by one `cargo +1.88 build --locked`. Comments naming 2.3/48.2 are updated.
2. **API changes** are the two icu_experimental ones from the research: the relative-time marker renames with the `relatives` map becoming a vector with `zero_index`, and the display-name markers' `…V0` renames. The output of `relativetime::patterns()` and of the display-name lookups must not change for the same data.
3. **Locale set.** `locales.txt` stays "the locales ICU4X's compiled data covers": remove `ht` (gone from ICU4X 2.4's data and from cldr-json 49) and add the 15 locales 2.4 gained (`ady`, `ady-JO`, `ady-TR`, `ckb`, `ckb-IR`, `cop`, `kbd`, `kbd-TR`, `kek`, `ki`, `ksh`, `lld`, `prg`, `quc`, `suz-Sunu`), each written in `locales.txt`'s form (likely script removed: check `suz-Sunu` against likely subtags). Its header names ICU4X 2.4.
4. **Generator, CLDR 49 shapes.** (a) CLDR 49's zh short currency patterns have a `0` pattern with a non-`0` alphaNextToNumber alternate (10^3 `other`: `0` and `¤ 0K`): `compact-currency.txt` and its reader (`core/src/cldr.rs`) must represent it, and the selection in `core/src/number.rs` (which today treats a plain `0` as "does not compact" before it looks at the alternate, and takes the currency position and adjacency from the plain pattern) must choose between the pattern and the alternate first, then decide compaction and the exponent from the chosen one. The JS receives the already-selected table and needs no change unless that contract changes. (b) Root's iso8601 `appendItems` alias to gregorian's is followed. (c) The new `intervalFormatRanges` and `numericSeparators` elements (XML and cldr-json) are skipped explicitly, by name, since ECMA-402 does not use them; any other unknown element stays an error. (d) The run continues through date patterns, day periods and display names; every further CLDR 49 shape change is handled the same way (explicit rule, a generator test, no silent skip). (e) The CLDR 49 POSIX zones (`cst6cdt`, `est5edt`, `mst7mdt`, `pst8pdt`, which no longer have a region) get no region in `time-zones-by-region.txt`, never one taken from the first letters of their names. Each change gets a test in `test_update_cldr_lists.py`.
5. **POSIX zones elsewhere.** `-u-tz-est5edt` canonicalizes as CLDR 49's `bcp47/timezone.xml` says (it is no longer an alias of `usnyc`). `Intl.supportedValuesOf("timeZone")` and time zone canonicalization keep ECMA-402's behaviour (`EST5EDT` resolves through the IANA links to `America/New_York`; the four names are not listed). Tests pin both.
6. **CNX.** It stays in `currencies.txt` (CLDR 49 still lists it, deprecated); CLDR 49 has no display name for it, so `DisplayNames` follows ECMA-402's fallback for it. The Rust test allows codes that CLDR has no name for, listed explicitly.
7. **Segmentation.** Word segmentation goes back to ICU4X's UTF-16 path (2.4 fixed the truncation of characters above U+FFFF), removing the UTF-8 transcoding and index mapping. The repairs for the remaining 2.4 bugs stay, with comments naming which cases remain (soft hyphen after Thai; the word-type re-check; segments of only ignored characters). The Thai `"่ำ"` change (2.4 keeps it one word-like segment, Node splits it) is accepted: ICU4X 2.4 follows Unicode 18's data and ECMA-402 leaves segmentation to the implementation; the test pins 2.4's result with that reason. New regression tests: a 210,000-code-unit Han run and a 200,000-code-unit Thai run segment in well under a second, in Rust and in lit (`containing` at the last index).
8. **Pinned data values.** Tests that pin CLDR 48.2 values (Windows zone rows and the PST/CA example, the 139-row count, cv's explicit bali NaN, the locales missing a region name, and any others) are updated to CLDR 49 values with the source named in a comment, after checking each change is in CLDR 49 itself, not a generator or code error.
9. **test262.** The expectations are re-baselined with `--intl-update-expectations`; every added or removed entry is explained in the task report and in its reason text.
10. **Sizes** are re-measured: Linux stripped `libhermesintl.so` (Release, Clang), Android per ABI, Windows x64, Apple per slice from the Mac; the spec's size tables, the dictionary figures and the README's "about 20 MB" follow.
11. **Close the issues.** `dz close 01a17bdc --as fixed` and `dz close 01a18652-9cc4 --as fixed`, each with a message naming the commit; the README's known-limitations list drops them.

## Review Focus

1. **Data and code from different CLDR releases.** After Task 3, every generated file says `release-49` / `49.0.0` in its header and `cargo test` checks each file against ICU4X's data where the files are compared. Task 3.
2. **zh compact currency with the alphaNextToNumber alternate:** at magnitude 10^3 (e.g. 1234), `Intl.NumberFormat("zh", {notation: "compact", style: "currency", currency: "USD"})` with `currencyDisplay: "symbol"` (`US$`: the character next to the number is `$`, not a letter, so the plain `0` is chosen and 1234 stays uncompacted) and `"code"` (`USD`: a letter next to the number, so the alternate `¤ 0K` is chosen and 1234 compacts), give CLDR 49's results, checked in Rust and in lit, including the scaling, the spacing and `formatToParts`. Task 3.
3. **POSIX zones:** `en-u-tz-est5edt` canonicalization, `supportedValuesOf("timeZone")`, DateTimeFormat with `timeZone: "EST5EDT"` and `Intl.Locale("und-PS").getTimeZones()` (no America/Los_Angeles). Tasks 3 and 4.
4. **Locales that came or went:** with a pinned default locale, `supportedLocalesOf(["ht"])` is empty, `ht` resolves to the default locale and formats like it (lookup finds no prefix of `ht`, so ResolveLocale takes DefaultLocale), and `["ht", "fr"]` selects `fr`; a new locale (`ckb`, `kbd`) resolves to itself and formats numbers and dates. Task 3, lit. The spec states the `ht` fallback change.
5. **Long unbroken runs:** the Han and Thai runs stay fast through `Intl.Segmenter`, including `containing` and iteration. Task 2.

---

### Task 1: The ICU4X 2.4 crates and the Rust API

**Files:** `intl/core/Cargo.toml`, `intl/core/Cargo.lock`, `intl/core/src/relativetime.rs`, `intl/core/src/displaynames.rs`, `intl/core/src/host.rs` and `intl/core/src/lib.rs` (Windows zone tests), `intl/CMakeLists.txt` (comment).

- [ ] **Step 1:** Change the requirements per decision 1; targeted `cargo update`; build.
- [ ] **Step 2:** Apply decision 2's fixes (the research's `trial-code.patch` has them); the relative-time and display-name tests show the same output.
- [ ] **Step 3:** Update the Windows zone tests to CLDR 49 (decision 8): the territory example becomes "Eastern Standard Time" + CA → `America/Toronto`, 139 → 142 rows, with the doc comment in `host.rs`.
- [ ] **Step 4:** `cargo +1.88 build --locked` succeeds (install the toolchain with `rustup toolchain install 1.88 --profile minimal` if missing).
- [ ] **Step 5:** `cargo test`; list every remaining failure with its cause (expected: data-dependent ones fixed in Tasks 2–3). Commit.

### Task 2: Word segmentation on ICU4X 2.4

**Files:** `intl/core/src/segmenter.rs`, `intl/jsi/SegmenterHelpers.cpp` if the UTF-8 detour lives there, `intl/test/lit/` (new long-run test; `segmenter-text.js`, which pins `"\u0e48\u0e33"` as two segments, updated with its comment per decision 7).

- [ ] **Step 1: Tests first:** the Rust and lit long-run tests of decision 7 (Han 210,000 units; Thai `"ก"` + 199,999 × U+0E48, boundaries `[0, 200000]`), each well under a second in the Debug ASan build; a UTF-16 word-segmentation test with a supplementary character in a CJK run (`"日本語𠀀テスト"` is `日本語|𠀀|テスト`).
- [ ] **Step 2:** Remove the UTF-8 detour per decision 7; keep and re-comment the remaining repairs; pin `"่ำ"` per decision 7.
- [ ] **Step 3:** Every segmenter test passes, Rust and lit (`LIT_FILTER=segmenter` on `check-intl-lit`); show each new test failing under a mutation (e.g. the 2.3 lock for the long runs is not possible, so mutate: re-introduce a per-call copy of the pending breaks, or for the UTF-16 test, truncate to `u16`). Commit.

### Task 3: The generator and the CLDR 49 data

**Files:** `intl/core/data/update-cldr-lists.py`, `intl/core/data/test_update_cldr_lists.py`, every generated `intl/core/data/*.txt`, `intl/core/data/locales.txt`, `intl/core/src/cldr.rs` and `intl/core/src/number.rs` (the compact selection), other readers the format change touches, `intl/js/*` only if it reads the compact alternate, the Rust tests with pinned values, `intl/test/lit/` (Review Focus 2–4).

- [ ] **Step 1: Generator tests first** for decision 4 (a)–(e) on fixture XML/JSON in `test_update_cldr_lists.py`.
- [ ] **Step 2:** Edit `locales.txt` per decision 3.
- [ ] **Step 3:** Implement decision 4, run `python3 intl/core/data/update-cldr-lists.py release-49` to completion, and commit the regenerated files. Record each further stop and its rule.
- [ ] **Step 4:** Decisions 5, 6 and 8: update the readers and tests; `cargo test` passes; `check-intl-data` passes.
- [ ] **Step 5:** The lit tests of Review Focus 2–4. Commit.

### Task 4: test262, the platforms and sizes

**Files:** `intl/test/test262-expectations.txt`, lit tests whose expectations move with CLDR 49 (each change explained), and the report's "Mac commands".

- [ ] **Step 1:** `check-intl` with `--intl-update-expectations` per decision 9; then `check-intl`, `check-hermes`, `check-shermes` all green; list every expectation change with its cause.
- [ ] **Step 2:** Sizes per decision 10 for Linux; Android: rebuild the AAR, run the emulator smoke test and device plugin-mode test262 on x86_64 (host totals); Windows: the x64 Release cross build (clang-cl or msvc-wine) and plugin-mode test262 under Wine with Microsoft's runtime (host totals); record sizes.
- [ ] **Step 3:** Write the Mac commands: a macOS host build with `check-intl` (plugin-mode test262), the slices and xcframework with sizes per slice, the iOS simulator smoke test with and without the plugin. Commit.

### Task 5: Spec, docs and the dz issues

**Files:** the spec, `intl/README.md`, `intl/doc/*.md`, `dz/issues/` (closing).

- [ ] **Step 1:** Update the spec everywhere it names ICU4X 2.3, `icu_experimental` 0.6, CLDR 48.2, npm 48.2.0 or `release-48-2`, the Segmenter section (the known limitation goes, the bugs that remain), Risks, the data examples and counts that CLDR 49 changes (re-check each), and the sizes (the Mac's when they arrive); record decisions 3–7.
- [ ] **Step 2:** README: data versions, the known-limitations list (decision 11); `intl/doc/Development.md`'s CLDR example tag.
- [ ] **Step 3:** Close the two dz issues per decision 11. Commit.

## After the tasks

1. Whole-branch review, then Codex code review of the range from the plan commit to the tip, until every finding is CLOSED.
2. The Mac round at the final commit; fix what fails; repeat until Codex and the Mac are both clean on the same commit.
3. Squash into one commit ("Upgrade the Intl plugin to ICU4X 2.4 and CLDR 49"), plan included; tree identical to the backup, then delete the tag.
