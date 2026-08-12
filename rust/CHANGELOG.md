# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Version numbers and release dates will be set at launch.

---

## [Unreleased]

### Added

#### Lexer (`hermes-parser`)
- Complete port of `JSLexer` (~3,700 lines of C++): punctuators, trivia,
  identifiers, keywords, all numeric literals (decimal, hex, octal, binary,
  legacy octal, BigInt, separators), string literals (all escape sequences
  including WTF-8 / `convertSurrogates`), template literals (TV/TRV dual
  buffers, `NotEscapeSequence` → null cooked), regular expression literals,
  private identifiers.
- JSX lexing: `advanceInJSXChild`, HTML entity table (253 entries, generated
  from `HTMLEntities.def`), JSX string mode.
- Flow `Type` grammar context: `{|`/`|}`, `%checks`, `@`-prefixed identifiers,
  Type-context `<`/`>`/`?`.
- Stateful/parser-facing APIs: comment and token storage, magic comments
  (`sourceURL`/`sourceMappingURL`), `SavePoint`, `seek`/`force_eof`,
  `isCurrentTokenADirective`, `rescanRBraceInTemplateLiteral`.
- Parser lookahead: `lookahead1`/`lookahead2` with `RequireNoNewLine` const
  generics; `isLetFollowedByDeclStart`, `isUsing`/`isAwaitUsing`.
- `IdentifierMode` via the `IdMode` marker trait + `JsMode`/`JsxMode`/`FlowMode`
  ZSTs; `scanString<const JSX: bool>` const generic — C++ templates preserved
  as Rust generics.
- Non-strict mode corpus: future-reserved-word downgrade + legacy octal paths.
- Byte-for-byte differential validated against the real `js-lexer-dump` C++
  oracle (div 58 / regexp 5 / type 6 / jsx 4 / jsx-child 10 / nonstrict 7
  corpus entries).

#### JSON parser (`hermes-parser`)
- Complete port of `JSONParser` + `JSONEmitter` + `JSONSharedValue`: recursive
  descent over `JSLexer`, `bumpalo` arena, string/number uniquing, hidden-class
  sharing, `emit_into`, WTF-8 key/value emission via `emit_u16`/`emit_key_u16`.
- `number_to_string` — ECMAScript `Number::toString`, shortest-decimal via Rust
  `{:e}`.
- Byte-for-byte differential validated against the real `json-parse-dump` C++
  oracle (16-file corpus including astral, lone-surrogate, hidden-class-shape,
  number edge cases, and 6 error cases).
- JSON parse throughput within ~1.5% of C++ Release build on an 11.6 MB corpus.

#### AST (`hermes-ast`)
- GC-arena AST copied and adapted from juno: `Context`/`GCLock`/`NodeRc` +
  mark-sweep collector.
- Full 271-node ESTree-compatible node set generated from `include/hermes/AST/ESTree.def`
  (all dialect flags: Flow, JSX, TypeScript, Cover) by committed `gen_nodes.py`.
- `NodeKind` enum mirroring the C++ enum (`#[repr(u32)]`, `.def` order, range
  sentinels); `is_*`/`as_*` predicates and accessors.
- Immutable children (`&'gc Node`/`Option`/`NodeList`) + `Cell<…>` attributes.
- Transforming visitor: `VisitorMut`/`TransformResult`/`Path`/`NodeField` +
  `visit_children_mut` (functional rebuild); generated `builder` module.
- `ESTreeJSONDumper`: byte-for-byte matching `hermesc -dump-ast`; supports
  compact, hide-empty, and dump-all modes; location/range/raw output.
- Idempotency gate (`generated_idempotent` test, forced by `REQUIRE_GEN=1`).

#### JS parser — standard ECMAScript (`hermes-parser`)
- Complete standard-ECMAScript grammar (P0–P4): value expressions, statements
  and declarations, functions/classes/arrows/async/generators/methods/`super`/
  `yield`/decorators, modules (`import`/`export` + `import()`/`import.meta`).
- Byte-for-byte differential validated against `hermesc -dump-ast` over a
  76-file plain-JS corpus.

#### JS parser — Flow type grammar (`hermes-parser`, P5)
- Full Flow annotation hierarchy: conditional, union, intersection, anonymous
  function types, prefix/postfix, primary types (keyof, infer, typeof, tuple,
  literal, generic).
- Function types including the `(T)`-group-vs-`(params)=>R` cover ambiguity.
- Object types: speculative modifier-to-name reparse, `[[slots]]`, indexers,
  mapped types.
- Type parameter declarations (`const`, variance); predicates (`asserts`,
  `implies`, `is`, `%checks`); return types.
- `opaque type`, `interface` declarations, `parse_class_implements_flow`.
- Non-ambiguous integration: function/method/class type-params, return types,
  binding/pattern annotations, class heritage (`extends B<T>`, `implements`),
  member variance, field types.
- Byte-for-byte differential over a 42-file Flow corpus (via `-parse-flow`).

#### JS parser — Flow extensions (`hermes-parser`, P6)
- Ambiguous-expression Flow grammar: typed arrows (sync + async), `as`/`as const`
  casts, `(x:T)` type casts, `CoverTypedIdentifier`; type-args on call/`new`/`?.`.
- `enum` declarations.
- `component`/`hook` declarations and type annotations.
- `record` declarations and expressions.
- `match` expressions and statements.
- `declare` statement family.
- `import type`/`export type` clauses; Flow default exports.
- Class-member `declare` modifier.
- Four `Context` flags (`parse_flow_ambiguous`/`_component_syntax`/`_records`/
  `_match`) that do not leak into plain-JS parsing.
- Byte-for-byte differential over component (8 files), records (5), match (7)
  corpora.

#### JS parser — TypeScript (`hermes-parser`, P7 — in progress)
- Type annotations: primary types, type references, type parameters and
  arguments, conditional types, type predicates.
- Function/constructor/parenthesized types; parameter properties.
- Work in progress; TS parsing is off by default.

#### Support (`hermes-support`)
- `SourceErrorManager` façade: `SourceBuffer` (lazy line index), offset-based
  `SMLoc`/`SMRange`/`SourceId`/`SourceCoords`, `DiagKind`/`Subsystem`/
  `OutputOptions`/`DiagHandler`/`CollectingHandler`.
- Byte-compatible diagnostic rendering: column = byte distance from line start,
  caret columns = code points with tab expansion (TabStop 8), caret line shown
  only for all-ASCII source.
- `JSONEmitter` (full surface: state stack, dict/array, all value overloads,
  escaping, pretty, JSONL).
- WTF-8 ↔ UTF-16 codec (`support::utf8`).
- `Deque`/`HeapSize` shared utilities.
- Byte-for-byte validated against captured `hermesc` output (`tests/golden.rs`).

#### Tooling
- The CLI drivers live in the unpublished `tools` crate (`publish = false`),
  so the published library ships no binaries and no `command_line` dependency.
- `ast-dump` binary: parses a JS/Flow file and dumps ESTree JSON, matching
  `hermesc -dump-ast` byte-for-byte.
- `json-parse-dump` binary: parses a JSON file and re-emits it.
- `gen-json` binary: generates deterministic JSON corpora for benchmarking.
- `preparse-dump` binary: dumps the pre-parse side table, matching the C++
  `preparse-dump` oracle byte-for-byte.
- C++ differential oracle tools (`tools/js-lexer-dump/`, `tools/json-parse-dump/`)
  registered via `add_hermes_tool`.

### Not yet available

- TypeScript parsing (full grammar) — P7 in progress.
- JSX parsing — planned.
- Pre-parse and lazy-parse passes — planned.
- `hermes-sema`, `hermes-ir`, `hermes-optimizer`, `hermes-bcgen` — future
  components in the port roadmap.
