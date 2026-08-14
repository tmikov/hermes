# JS Parser — Design Spec

> Component: **JS Parser** (`lib/Parser/JSParserImpl*`). Consumes the completed **JS lexer**,
> **AST**, and **`Context`**. This is the next component after the AST (see
> `doc/superpowers/RustPortRoadmap.md`). Date: 2026-06-06.

## 1. Goal

Port the Hermes JavaScript parser to Rust, faithfully and completely, on the `rust` branch.
It is a standard recursive-descent LL(1) parser (with one- and two-token lookahead in a few
spots) that turns a token stream from `JSLexer` into an ESTree AST allocated in the `ast`
crate's GC arena. The validation gate is a **byte-for-byte `-dump-ast` differential against the
real `hermesc`** binary.

## 2. Source of truth

Port directly from the C++ — **there is no Rust parser to crib from.** juno's `hparser`
(`unsupported/juno/crates/juno/src/hparser/`) is *not* a parser: it calls the C++ Hermes parser
over FFI (`hermes::parser::HermesParser`) and converts the resulting C++ AST into juno's Rust
AST (`convert.rs` + `generated_cvt.rs`). So the parser logic comes straight from:

| C++ file | Lines | Maps to |
|----------|-------|---------|
| `lib/Parser/JSParserImpl.cpp` | 7,603 | core JS (`expressions`/`statements`/`functions`/`bindings`/`modules`) |
| `lib/Parser/JSParserImpl-flow.cpp` | 5,438 | Flow types (`flow`) |
| `lib/Parser/JSParserImpl-ts.cpp` | 1,437 | TS types (`ts`) |
| `lib/Parser/JSParserImpl-jsx.cpp` | 505 | JSX (`jsx`) |
| `lib/Parser/JSParserImpl.h` | 1,769 | the `JSParserImpl` class decl |
| `include/hermes/Parser/JSParser.h` | 132 | public surface |
| `include/hermes/Parser/PreParser.h` | 87 | lazy-parse support types |
| `unittests/Parser/JSParserTest.cpp` | 8.9 KB | ported unit tests |

~16,900 lines total — the largest component in the port so far (the lexer was ~3,700).

## 3. Scope

**Everything** — the whole public surface, in one component pass, per the
"implement-components-completely" rule:

- **All three parser passes:** `FullParse`, `PreParse` (index functions, no AST), and
  `LazyParse` (skip + reparse functions on demand), plus `parseLazyFunction` / `preParseBuffer`.
  Decided explicitly (the user chose "all three passes now") even though lazy parsing has no
  consumer in the Rust port yet (no VM / compiler driver) and the `-dump-ast` differential gate
  only exercises `FullParse`.
- **All dialects:** core JS + JSX + Flow + TS. The AST already generated every node family, and
  the lexer already does the JSX and Flow `Type` grammar contexts.
- **Full public API:** `parse`, `seek`, strict-mode get/set, magic URLs (`getSourceURL`/
  `getSourceMappingURL`/`registerMagicURLs`), stored comments/tokens, `getUseStaticBuiltin`,
  `ParserPass`, `MCFlag`.

## 4. Core structural model

### 4.1 Crate placement

The parser proper lives in the **existing `parser` crate**, alongside the lexer it drives
(mirroring C++: `JSLexer` and `JSParserImpl` both live in `lib/Parser/`). The `parser` crate
gains a dependency on the **`ast` crate**. New module tree under `parser/src/js/`.

### 4.2 Lifetime / arena model

`JSParserImpl<'gc>` holds a `&'gc GCLock<'ast, 'ctx>` for the entire parse — **one lock, no GC
mid-parse**, matching C++ where nodes come from a `Context`-owned bump allocator and live
forever. Nodes are allocated via `gc.alloc(Node::Foo(Foo::new(meta, …)))` → `&'gc Node<'gc>`.
Every `parse*` method returns `Option<&'gc Node<'gc>>` (or `Option<&'gc SpecificNode<'gc>>`);
`'gc` threads through all of them.

### 4.3 Node construction + `set_location`

A `set_location`-style helper mirrors C++ `setLocation(start, end, node)` and its overloads
(accepting tokens, nodes, `SMLoc`, or `SMRange` for start/end): it computes an `SMRange`, builds
`NodeMetadata`, calls the phase-2 `new` constructor, and `gc.alloc`s.

**Preliminary AST change (small, contained):** extend `ast` `NodeMetadata`
(`node_child.rs`) with a debug location so the parser can set it faithfully (C++ `setLocation`
sets start, end, *and* debug loc):

- Add `pub debug_loc: Cell<SMLoc>` to `NodeMetadata`.
- `NodeMetadata::new(range)` defaults `debug_loc` to `range.Start` (matching C++, where the
  3-arg `setLocation` sets `debugLoc = start`); add a constructor/setter variant for the C++
  4-arg `setLocation(start, end, debugLoc, node)`.
- `duplicate()` copies it.
- `NodeMetadata::new(range)` is the **single construction entry point** and all `new`
  constructors take `metadata` whole, so node-`new` signatures are unchanged.
- The dumper does **not** emit `debug_loc` (neither does C++ `-dump-ast`), so all existing AST
  golden/idempotency tests are byte-unchanged; only sites that build `NodeMetadata` directly are
  touched.

### 4.4 Error propagation

Keep the C++ idiom faithfully: `Option<T>` where **`None` means "an error was already
reported"** (exactly C++ `llvh::Optional<T>`), propagated with `?`. No `Result`. Error text goes
to the `SourceErrorManager` at the point of detection, same as C++. The `error` / `need` / `eat`
/ `errorExpected` / `eatSemi` helpers port directly onto the `sm_` the support crate already
provides. The "too many errors → move to EOF, return false" behavior of `error(loc, range, msg)`
is preserved.

### 4.5 Param threading & generics (locked conventions)

- `Param` stays a **value/bitflags struct** (`ParamIn` / `ParamReturn` / `ParamDefault` /
  `ParamTagged`) with `+` / `-` / `has` / `get`.
- `paramYield_` / `paramAwait_` stay **runtime `bool` fields** (they are not templates in C++).
- The **variadic templates stay Rust generics** — `checkN<…>`, `parseStatementList<Tail…>` —
  **never flattened to runtime params** (the lexer's flattening was the one mistake we keep
  catching; see roadmap). Any other `template <…>` in the four C++ files stays a generic.
- The small `enum class` flag types (`IsConstructorCall`, `AllowImportExport`,
  `CoverTypedParameters`, `AllowTypedArrowFunction`, `VariableDeclAllowPattern`,
  `OfEndsAssignment`, `ClassParseKind`, `AllowJSXMemberExpression`, `AllowAnonFunctionType`, …)
  become Rust enums, same as C++.

### 4.6 Recursion guard

A `recursion_depth: u32` counter + `MAX_RECURSION_DEPTH` constant with an inline
`recursion_depth_check()`, faithful to C++ (and consistent with the AST dumper's depth counter).
The C++ RAII increment/decrement becomes an explicit scoped guard or inc/dec pair, per the
"RAII → explicit" convention.

## 5. Module layout

Under `parser/src/js/`, mirroring the C++ files — all `impl<'gc> JSParserImpl<'gc>` blocks
sharing one struct, exactly like the lexer's split:

- `mod.rs` — the `JSParserImpl<'gc>` struct + driver helpers (`advance`/`check`/`checkN`/`eat`/
  `checkAndEat`/`need`/`error`/`errorExpected`/`eatSemi`/`recursion_depth_check`), `Param`, the
  interned idents (`initializeIdentifiers`), `set_location`, `parse`/`parseProgram`,
  `processDirective`, `checkDeclaration`/`checkAssign`/etc.
- `expressions.rs` — primary, member/call/new, optional chaining, unary/postfix, the
  precedence-table binary parser, conditional, assignment, yield, arrow + arrow reparse, array/
  object literals, templates, spread, `convertIdentOpIfPossible`.
- `statements.rs` — block, if, while/do/for (+ for-in/of), switch, try, return/break/continue/
  throw/with/debugger, labelled, var/let/const, `using`/`await using`, expression statements,
  directives.
- `functions.rs` — formal params, function decl/expr, class decl/expr/body/elements, decorators.
- `bindings.rs` — binding identifier/array/object patterns, binding elements/rest, the array/
  object expression → pattern reparse (`reparseAssignmentPattern` family),
  `ensureDestructuringInitialized`.
- `modules.rs` — import/export declarations, from-clause, with-clause/attributes, specifiers.
- `jsx.rs` ← `JSParserImpl-jsx.cpp`.
- `flow.rs` ← `JSParserImpl-flow.cpp` (5,438 lines — likely sub-split into a `flow/` dir).
- `ts.rs` ← `JSParserImpl-ts.cpp`.
- `lazy.rs` — PreParse / LazyParse machinery + the ported `PreParser.h` types
  (`PreParsedFunctionInfo` / `PreParsedBufferInfo` / `PreParsedData`) and a
  `getPreParsedBufferInfo`-style hook on the `ast` `Context` (it has none today).

## 6. Validation gate

### 6.1 `hermesc -dump-ast` is the oracle (no new C++ tool)

Verified in `lib/CompilerDriver/CompilerDriver.cpp:867`: the `-dump-ast` target calls
`dumpESTreeJSON(...)` **immediately after parsing and returns** — *before* `convertTSToFlow`,
`transformASTForCompilation`, and `resolveAST` (Sema). So on a plain file it dumps the **raw
parse AST**, which is exactly what our Rust parser produces. Consequences:

- The differential is **parser-only** — no Sema entanglement (Sema isn't ported, and isn't
  needed here).
- The oracle **already exists** as `hermesc -dump-ast`, routed through the very C++
  `dumpESTreeJSON` (`ESTreeJSONDumper`) we ported byte-for-byte in AST phase 4. So this gate is
  *also* the deferred end-to-end exercise of our AST dumper. **No dedicated C++ oracle tool is
  built** (unlike `js-lexer-dump` / `json-parse-dump`).

### 6.2 The Rust `ast-dump` bin + differential test

- **`ast-dump` bin** (parser crate, like `json-parse-dump`): reads a file + flags, parses with a
  `Context`, dumps via `ast::dump::dump_estree_json_with_sm`, prints to stdout. Flags mirror
  `hermesc`: `--dump-source-location`, `-include-empty-ast-nodes` (DumpAll vs HideEmpty),
  `-parse-flow` / `-parse-jsx` / `-parse-ts`, `-pretty`, strict-mode.
- **`tests/parser_differential.rs`**: for each corpus file, run `hermesc -dump-ast <flags>` and
  the Rust `ast-dump` and compare **byte-for-byte**. Resolve the `hermesc` binary via
  `cmake-build-asan` (`CARGO_MANIFEST_DIR`), and honor `REQUIRE_DIFFERENTIAL=1` to force a hard
  failure if absent (same pattern as lexer/JSON, which silently-skipped once until fixed).
- **Corpus** grows per phase under `tests/parser_corpus/` (JS-only first; JSX/Flow/TS/error-
  recovery added with their phases). Error-recovery cases compare **stderr** too (as the lexer
  did).

### 6.3 Ported unit tests

A faithful port of `unittests/Parser/JSParserTest.cpp` → `tests/jsparser_ported.rs`: error/
warning counts, message text via a `CollectingHandler`, recovery, and concrete AST shape — same
discipline as the `JSLexerTest` / `JSONParserTest` ports.

## 7. Phasing (build order — core-first, sliced)

Each phase extends the differential corpus and is independently two-stage reviewed (spec +
quality) before moving on.

- **P0 — Foundations + gate.** Add `debug_loc` to AST `NodeMetadata`; scaffold `JSParserImpl<'gc>`
  + driver helpers + `set_location`; stand up the `ast-dump` bin + differential harness on a
  Program-only corpus (gate live from day one, like lexer 1a).
- **P1–P4 — Core JS:** expressions ↔ statements ↔ functions/classes/bindings ↔ modules. These
  productions are mutually recursive (function bodies hold statements; expressions appear
  everywhere), so the slices land behind a **growing JS-only corpus** rather than as hard walls;
  exact cut-points are set in the implementation plan. End state: a complete core-JS parser
  passing the differential.
- **P5 — JSX.**
- **P6 — Flow types** (large; sub-sliced in the plan).
- **P7 — TS types.**
- **P8 — Lazy parsing:** PreParse / LazyParse, the `PreParser.h` port, the `Context`
  `getPreParsedBufferInfo` hook, `parseLazyFunction` / `preParseBuffer`, and the `forceEagerly` /
  `pass_` plumbing exercised end to end.
- **Capstone — whole-component review** + a **structural-fidelity grep** over all four C++ files
  for `template <`, confirming every one survived as a Rust generic (never flattened to a runtime
  param), plus the usual template↔runtime / layout / RAII-beyond-the-agreed-list checks.

## 8. Faithful-port conventions (inherited; do not relitigate)

- Keep C++ `template`s as Rust generics; keep C-idiom comparisons faithful; gate on zero
  `cargo build` warnings; fix genuine new clippy lints with a scoped `#[allow]` + comment or a
  clean rewrite.
- C++ RAII guards → explicit set/restore methods (e.g. `SaveStrictModeAndSeenDirectives`,
  `SaveFunctionState`, the JSX-depth guard) — value structs + `restore`, or scoped guards.
- Diagnostics byte-compatible with `hermesc` (inherited from `support`).
- Commit directly to `rust`; never open a PR / merge. Commit messages end with the
  `Co-Authored-By: Claude Opus 4.8 (1M context)` trailer.

## 9. Known gaps / tracked follow-ups

- **Debug location** is now *added* to the AST and set by the parser (§4.3) — no longer a gap.
- The AST dumper's **third overload** (caller-owned `JSONEmitter` + `NodeKindSet` filter +
  public `includeSourceLocs` setter) remains unexposed from AST phase 4; not needed by the
  parser differential. Still tracked there, not here.
- Lazy parsing is ported but has **no in-tree consumer** until the VM / compiler driver is
  ported; it is validated by its own unit tests + reparse round-trips, not by the `-dump-ast`
  differential (which only drives `FullParse`).

## 10. Risks / things to verify during implementation

- **Flag mapping precision.** Confirm `hermesc`'s default dump mode (HideEmpty) and the exact
  flag spellings/semantics so the Rust `ast-dump` matches byte-for-byte (validated by the
  differential itself once P0 lands).
- **`convertSurrogates` / WTF-8 in identifiers & string-literal nodes.** The dumper already
  emits WTF-8→UTF-16 faithfully; confirm the parser stores literal values consistently with what
  the dumper expects (the JSON port hit this exact class of bug).
- **Flow type parsing volume** (5,438 lines) is the single biggest risk to schedule; sub-slice
  aggressively in the plan.
- **`-dump-ast` `loc` vs the dumper's translated `find_coords`.** AST phase 4 noted the dumper
  uses the translated `find_coords` for `loc` where C++ `printSourceLocation` uses
  `findBufferLineAndLoc`; the parser differential (with real source locations) is where this
  equivalence gets confirmed.
