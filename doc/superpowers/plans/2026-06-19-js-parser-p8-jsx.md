# JS Parser P8 — JSX Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Port `lib/Parser/JSParserImpl-jsx.cpp` (505 lines, 12 methods) + its single dispatch site, behind the existing `Context::parse_jsx` flag, so the Rust parser handles JSX byte-for-byte vs `hermesc -dump-ast -parse-jsx`.

**Architecture:** A new `rust/crates/parser/src/js/jsx.rs` (single cohesive file — JSX is small) holds the `impl JSParserImpl` JSX methods. A `jsx_depth: u32` field is added to `JSParserImpl`. The one dispatch site (the `<` case in primary-expression parsing) calls `parse_jsx()` when `self.parse_jsx()`. A new `tests/parser_corpus_jsx/` differential dir runs `hermesc -parse-jsx` vs `ast-dump --parse-jsx`. **No AST nodes added** — all 16 `JSX*` nodes are already generated (`generated_idempotent` stays the guardrail).

**Tech Stack:** Rust 1.96.0, the `parser`/`ast` crates; `hermesc -dump-ast -parse-jsx` as the byte-for-byte oracle; `cargo test -p parser --test parser_differential` (`REQUIRE_DIFFERENTIAL=1`).

---

## Source of truth & conventions (read before any task)

- **C++ JSX file:** `lib/Parser/JSParserImpl-jsx.cpp` lines 22–499 (all under `#if HERMES_PARSE_JSX`). 12 methods + the `tagNamesMatch` static helper, each listed with line ranges in the tasks.
- **C++ header:** `lib/Parser/JSParserImpl.h` — `jsxDepth_` field (251), method decls (1182–1203), `AllowJSXMemberExpression { No, Yes }` enum (1198).
- **The one integration site:** `lib/Parser/JSParserImpl.cpp:2691-2696` — `#if HERMES_PARSE_JSX if (context_.getParseJSX()) { auto optJSX = parseJSX(); ... }`, inside the primary-expression `case TokenKind::less:`. The Rust marker is at `expressions.rs:4910` (`// JSX — context-gated (getParseJSX()). For now emit the C++ error.`). (The TS `<Type>` assertion at `expressions.rs:2435` is already gated `parse_ts() && !parse_jsx()` from P7.5b — JSX takes priority over TS casts when both could apply; verify the dispatch order matches the C++.)
- **Already in place (do NOT re-create):** `Context::parse_jsx` flag + getter/setter (`ast/src/context.rs`, added P7.5b); the `parse_jsx()` parser shorthand (`js/mod.rs:254`); the lexer's `advance_in_jsx_child()` (`lexer/jsx.rs:126`), the `GrammarContext::AllowJSXIdentifier` context, JSX-string HTML-entity decoding, and the `jsx_text` token with its value/raw accessors. **Confirm the jsx-text accessors exist** (grep the Rust `Token` for `jsx_text`/`getJSXTextValue`/`getJSXTextRaw` analogs — likely `jsx_text_value`/`jsx_text_raw`) and use them.
- **Faithful-port rules (NON-NEGOTIABLE):**
  - **C++ default arguments are spec — read the header.** The grammar-context arg is the crux of JSX: nearly every `advance`/`eat`/`checkAndEat` passes **`GrammarContext::AllowJSXIdentifier`** (so `-`-containing JSX identifiers + reserved words lex as JSX identifiers), with deliberate exceptions — `parseJSXChildren`'s `{` and the `parseJSXChildExpression`/`parseJSXSpreadAttribute` inner `advance()`/`eat()` use the DEFAULT (`AllowRegExp`) where the C++ shows no explicit context, and the spread-attribute closing `}` and attribute `}` eat in `AllowJSXIdentifier`. **Copy the EXACT context per call site** — this is the #1 recurring bug class in this port.
  - **The lexer-mode switch is the heart of JSX.** After an opening tag / closing tag / child expression / text, the parser calls EITHER `lexer_.advanceInJSXChild()` (stay in JSX-text mode) OR `advance()` (return to normal JS lexing), chosen by `jsxDepth_`. Port `parseJSXOpeningElement` 156-162, `parseJSXClosing` 392-397 + 414-419 EXACTLY — a wrong branch corrupts the token stream for everything after the JSX.
  - **`jsxDepth_` `SaveAndRestore`** (C++ `llvh::SaveAndRestore<uint32_t>` at jsx.cpp:24, 78, 176): `parseJSX` saves-and-sets it to 0; `parseJSXElement`/`parseJSXFragment` save-and-set it to `jsxDepth_ + 1`. The restore must survive `?` early-returns. Use the SAME pattern this port already uses for `SaveAndRestore` (grep `RecursionGuard`/`ParamFlagGuard`/`SaveAndRestore` in `js/`); a small Drop guard that restores a saved `u32`, or an explicit save/run/restore wrapper. Do NOT leave `jsx_depth` leaked on an error path.
  - **`tagNamesMatch`** (jsx.cpp:34-75): a free `fn` (mirrors the C++ `static` function) that recursively compares opening/closing tag names (JSXIdentifier / JSXNamespacedName / JSXMemberExpression). Port the `dyn_cast` chain as a Rust `match`/`if let`.
  - **`AllowJSXMemberExpression { No, Yes }`** — a Rust enum (faithful), passed to `parseJSXElementName` (Yes for element/closing names, No for attribute names + after which a member-expression name is an error).
  - **Keep comments** (`/// Port of JSParserImpl::parseJSXxxx (jsx.cpp:NNNN-NNNN).` + `// C++ NNNN` markers), matching the `js/flow/`+`js/ts/` density.
  - **Error notes:** the C++ emits `sm_.note(...)` secondary diagnostics (e.g. "location of opening"). The Rust port drops these secondary notes per the established house style (the `need`/`eat`/`error` helpers take only `where_`) — match how `js/ts/` and `js/flow/` handled the dropped `errorExpected` note args; the PRIMARY error text must still match (unobservable in `-dump-ast` on valid input, but keep fidelity).
- **The Flow-type-args-in-JSX wrinkle (jsx.cpp:124-132):** `parseJSXOpeningElement` parses optional `<TypeArgs>` after the element name via `parseTypeArgsFlow(GrammarContext::AllowJSXIdentifier)` — a **Flow** feature (`<El<T> />`). Port it calling the Rust `parse_type_args_flow` with the `AllowJSXIdentifier` context. This path is only reachable when Flow is ALSO on; the standalone `-parse-jsx` corpus won't exercise it. **Check whether `parse_type_args_flow` debug-asserts `parse_flow()`** — if so, either guard the call on `parse_flow()` (only if the C++ effectively does, i.e. it's inert without Flow) OR test it only in a flow+jsx corpus file. Verify against the oracle (`hermesc -parse-jsx -parse-flow` on `<El<T} />`-style input) and do whatever is byte-faithful; document the decision.

### Validation commands (every task ends green)

```bash
cmake --build cmake-build-asan --target hermesc            # oracle (should already exist)
cargo build --manifest-path rust/Cargo.toml -p parser --bin ast-dump
cargo build --manifest-path rust/Cargo.toml                # ZERO warnings
cargo test  --manifest-path rust/Cargo.toml -p parser
REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p parser --test parser_differential -- --nocapture
# Expect "parser differential (tests/parser_corpus_jsx): N corpus files matched" AND
# all other corpora (plain 76, flow 42, flow_component 8, flow_records 5, flow_match 7, ts 20) UNCHANGED.
cargo clippy --manifest-path rust/Cargo.toml -p parser     # no NEW lints
```
Hand-check a file: `(! cmake-build-asan/bin/hermesc -dump-ast -dump-source-location=both -parse-jsx FILE 2>&1)` vs `./rust/target/debug/ast-dump --parse-jsx FILE`.

---

## File structure

- **Create** `rust/crates/parser/src/js/jsx.rs` — all 12 JSX methods + `tag_names_match` + `AllowJSXMemberExpression`.
- **Modify** `rust/crates/parser/src/js/mod.rs` — `mod jsx;`; add the `jsx_depth: u32` field to `JSParserImpl` (init 0 in `new`); the `jsx_depth` save/restore guard if you put it here.
- **Modify** `rust/crates/parser/src/js/expressions.rs` — replace the `// JSX` marker at ~4910 (the primary-expression `<` case) with `if self.parse_jsx() { return self.parse_jsx_entry(); }` (name the entry method to avoid colliding with the `parse_jsx()` bool shorthand — e.g. the entry is `parse_jsx()` in C++ but the Rust bool accessor is already `parse_jsx()`; name the JSX entry method `parse_jsx_root` or `parse_jsx_element_or_fragment`, and document the rename).
- **Modify** `rust/crates/parser/src/bin/ast_dump.rs` — add `--parse-jsx` → `ctx.set_parse_jsx(true)`.
- **Modify** `rust/crates/parser/tests/parser_differential.rs` — add `run_differential("tests/parser_corpus_jsx", &["-parse-jsx"], &["--parse-jsx"])`.
- **Create** `rust/crates/parser/tests/parser_corpus_jsx/*.js` — the growing JSX corpus.

---

## Task P8.0 — Foundations + gate

**Goal:** the `jsx_depth` field + save/restore guard, the `jsx.rs` skeleton, `--parse-jsx` + the `parser_corpus_jsx` differential, the dispatch wire, and a minimal self-closing `<div />` round-tripping byte-for-byte.

**Files:** `js/jsx.rs` (create), `js/mod.rs`, `js/expressions.rs`, `bin/ast_dump.rs`, `tests/parser_differential.rs`, `tests/parser_corpus_jsx/self_closing.js`.

- [ ] **Step 1 — field + guard.** Add `jsx_depth: u32` to `JSParserImpl` (`js/mod.rs`), init 0 in `new`. Add a `jsx_depth` save/restore mechanism mirroring the port's existing `SaveAndRestore` pattern (find it: grep `RecursionGuard`/`SaveAndRestore`/`ParamFlagGuard` in `rust/crates/parser/src/js/`). It must restore on `?` early-return.

- [ ] **Step 2 — ast-dump + differential wiring.** `ast_dump.rs`: `--parse-jsx` Opt → `ctx.set_parse_jsx(*opt.parse_jsx)` (mirror `--parse-ts`; do NOT OR into flow/ts — JSX is orthogonal and may combine, but the flag is independent). `parser_differential.rs`: add `run_differential("tests/parser_corpus_jsx", &["-parse-jsx"], &["--parse-jsx"]);`.

- [ ] **Step 3 — jsx.rs skeleton + minimal element (TDD).** Create `tests/parser_corpus_jsx/self_closing.js`:
  ```jsx
  var x = <div />;
  var y = <Foo.Bar />;
  var z = <a:b />;
  ```
  Run the differential — FAILS (the `<` case errors). Then create `js/jsx.rs` with the copyright header + module doc, and implement the path needed for self-closing elements:
  - `parse_jsx_root` (the C++ `parseJSX`, jsx.cpp:22-30): assert `check(less)`, save `jsx_depth`=0, `advance(AllowJSXIdentifier)`, dispatch `check(greater)` → fragment (P8.1 — honest error for now) else `parse_jsx_element`.
  - `parse_jsx_element` (jsx.cpp:77-115): save `jsx_depth`=`jsx_depth+1`, `parse_jsx_opening_element`; if `self_closing` → `JSXElement{opening, [], None}`; else parse children (P8.1 — honest error for self-closing-only this task is fine since `<div/>` is self-closing).
  - `parse_jsx_opening_element` (jsx.cpp:117-169): `parse_jsx_element_name(Yes)`; optional `<TypeArgs>` (P8.1/Flow — honest error or skip if not `check(less)`); the attributes loop (P8.1 — for now require immediate `/` or `>`); `self_closing = checkAndEat(slash)`; `need(greater, ...)`; the **lexer-mode switch** (`if self_closing && jsx_depth <= 1 { advance() } else { advanceInJSXChild() }`); build `JSXOpeningElement{name, attrs, self_closing, type_args}`.
  - `parse_jsx_element_name` (jsx.cpp:425-499): the JSXIdentifier / `:` namespaced / `.` member-expression name parser + the `AllowJSXMemberExpression::No` error. Implement FULLY now (it's needed by names and is self-contained).
  - `AllowJSXMemberExpression { No, Yes }` enum + `tag_names_match` (implement `tag_names_match` now even though only P8.1 uses it, OR defer to P8.1 — your call; it's pure).
  - Wire the dispatch at `expressions.rs:4910`: `if self.parse_jsx() { return self.parse_jsx_root(); }`.

- [ ] **Step 4 — verify + commit.** Differential green for `parser_corpus_jsx` (1 file), all other corpora unchanged, zero warnings. Commit:
  ```
  rust(parser): P8.0 JSX foundations + gate (self-closing elements + element names)

  Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
  ```

---

## Task P8.1 — full JSX: children, fragments, attributes, expression containers, closing-tag matching

**Goal:** everything else — non-self-closing elements with children, fragments, JSX text, expression containers, spread children/attributes, attributes (string + expression values), namespaced/member names in all positions, and closing-tag name matching.

**Files:** `js/jsx.rs`; corpus files.

**C++ to port (jsx.cpp):** `parseJSXFragment` (171-201), `parseJSXChildren` (203-270, incl. the JSXText / `{expr}` / `{}` empty / `<` element-or-closing dispatch + the per-iteration `advanceInJSXChild()`), `parseJSXChildExpression` (272-287, `...spread` child + expression container), `parseJSXSpreadAttribute` (289-319), `parseJSXAttribute` (321-384, string-literal value via `JSXStringLiteral` + raw, or `{expr}` container, or bare boolean attr), `parseJSXClosing` (386-423, fragment vs element closing + the depth-driven lexer-mode switch), the attributes loop + `parseJSXSpreadAttribute` dispatch inside `parseJSXOpeningElement` (134-148), and `tagNamesMatch` (34-75) wired into `parseJSXElement` (98-108, the closing-must-match / must-not-be-fragment errors). Also wire the opening-element `<TypeArgs>` (124-132) via `parse_type_args_flow` (see the Flow wrinkle in conventions).

- [ ] **Step 1 — elements with children + text + closing match (TDD).** Corpus `jsx_children.js`: `var a = <div>hello</div>;`, `var b = <a><b>x</b></a>;` (nesting → depth switch), `var c = <p>text &amp; more</p>;` (HTML entity in text). Run differential, implement `parse_jsx_children` + `parse_jsx_closing` + `tag_names_match` + the `parse_jsx_element` children branch. Green.
- [ ] **Step 2 — fragments (TDD).** Corpus `jsx_fragments.js`: `var a = <></>;`, `var b = <><div/>text</>;`, nested fragment-as-child. Implement `parse_jsx_fragment` + the fragment dispatch in `parse_jsx_root`/`parse_jsx_children`. Green.
- [ ] **Step 3 — expression containers + spread children (TDD).** Corpus `jsx_expressions.js`: `var a = <div>{x}</div>;`, `var b = <div>{}</div>;` (empty → `JSXEmptyExpression`), `var c = <div>{...items}</div>;` (spread child), `var d = <div>{a}{b}</div>;`. Implement `parse_jsx_child_expression` + the `{` branch in `parse_jsx_children`. Green.
- [ ] **Step 4 — attributes (TDD).** Corpus `jsx_attributes.js`: `var a = <div id="x" />;`, `var b = <div onClick={f} />;`, `var c = <div disabled />;` (bare bool), `var d = <div {...props} />;` (spread), `var e = <a:b c:d="x" />;` (namespaced attr name), `var f = <div data-x="y" />;` (hyphenated). Implement `parse_jsx_attribute` + `parse_jsx_spread_attribute` + the attributes loop. Green.
- [ ] **Step 5 — commit** `rust(parser): P8.1 JSX children, fragments, attributes, expression containers, closing-tag matching`.

**Fidelity notes:** the per-child `advanceInJSXChild()` calls (jsx.cpp:257, 267, and after the opening tag) are what keep the lexer in JSX-text mode between children; the closing tag's depth check returns to JS mode at depth 1. JSX text value vs raw: use the lexer's `jsx_text` value/raw accessors (HTML entities decoded in value, raw preserved). String-literal attribute values build `JSXStringLiteral{value, raw}` where raw = `lexer.get_string_literal(tok.input_str())` — find the Rust analog. Empty `{}` child builds a `JSXExpressionContainer` wrapping a zero-width `JSXEmptyExpression`.

---

## Task P8.2 — Capstone review + docs

- [ ] **Step 1 — `getParseJSX()` site audit.** Grep the C++ for every `getParseJSX()` + `parseJSX*` call; map each to its Rust production. Confirm the single dispatch site is wired, all 12 methods ported + reachable, zero `// JSX`/`// P8` markers remain in `rust/crates/parser/src/`.
- [ ] **Step 2 — structural-fidelity.** `grep "template <" lib/Parser/JSParserImpl-jsx.cpp` (expect none). Confirm `AllowJSXMemberExpression` is a Rust enum (not bool); confirm the `jsx_depth` save/restore can't leak on `?`; confirm every `advance`/`eat`/`checkAndEat`/`advanceInJSXChild` grammar-context + mode-switch matches the C++ at that site (the highest-risk class — re-verify the `AllowJSXIdentifier`-vs-default contexts and the depth-driven `advance` vs `advanceInJSXChild` at all 4 switch sites).
- [ ] **Step 3 — corpus completeness.** Confirm the corpus exercises: self-closing, element-with-children, nesting (depth switch), fragments (incl. as child), JSX text + HTML entities, expression containers (incl. empty `{}`), spread children, spread attributes, string + expression + bare attributes, namespaced names (element + attribute), member-expression names (`A.B.C`), reserved-word tag names, JSX nested inside a `{expr}` (JS→JSX→JS round-trip), JSX as a sub-expression (`cond ? <a/> : <b/>`), and — under a flow+jsx corpus file or hidden combination — the opening-element `<TypeArgs>` if reachable. Add files for any gap (verify hermesc accepts each first). For the closing-tag-mismatch ERROR path (`<a></b>`), add a test only if the error is observable in `-dump-ast` (it likely recovers; check the oracle).
- [ ] **Step 4 — final verify + docs.** Full differential green (all corpora), `cargo build` zero warnings, `cargo clippy -p parser` no new lints, `generated_idempotent` green (no AST nodes added). Update `doc/superpowers/RustPortRoadmap.md` (P8 DONE block + table row: "P0–P8 DONE — standard JS + Flow + TS + JSX; only the Pre/Lazy passes remain") and `SESSION-HANDOFF.md` (next: the Pre/Lazy passes). Commit `doc(rust): JS Parser P8 complete — JSX; roadmap + handoff updated (next: Pre/Lazy passes)`.

---

## Self-review (done at plan-write time)

- **Spec coverage:** all 12 jsx.cpp methods + `tagNamesMatch` assigned (P8.0: root/element/opening/element-name + enum; P8.1: fragment/children/child-expression/spread-attr/attribute/closing + tag-names-match + attrs loop + type-args). The single dispatch site → P8.0. ✓
- **No AST nodes:** all 16 `JSX*` nodes already in `node.rs`; `generated_idempotent` is the guard. ✓
- **Reused infrastructure:** `Context::parse_jsx` + `parse_jsx()` shorthand + lexer `advance_in_jsx_child`/`AllowJSXIdentifier`/jsx-text + HTML entities all pre-exist (P7.5b + the lexer). ✓
- **Highest-risk items flagged:** the `jsx_depth` save/restore, the depth-driven lexer-mode switch (4 sites), the per-call `AllowJSXIdentifier`-vs-default grammar contexts, and the Flow-type-args-in-JSX wrinkle. ✓
- **Naming caveat:** the C++ `parseJSX()` entry collides with the Rust `parse_jsx()` bool accessor — entry method renamed (`parse_jsx_root`), documented. ✓
