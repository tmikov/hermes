# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---

## 2026-08-16

### `hermes-gen-js` 0.1.0 — new crate

AST → JavaScript, Flow or TypeScript source. A port of juno's `gen_js.rs`,
extended to the 106 node kinds it never printed. All 271 node kinds are
handled, and that is compiler-enforced: the dispatch match has no catch-all,
so a kind added to the AST without a printing arm is a build failure.

The correctness bar is the round-trip property — generated source reparses to
the same AST. There is no byte-exact C++ oracle for this component, unlike the
rest of the family; `crates/gen_js/MANIFEST.md` records what was run, the 41
defects found and fixed during the port, and what is deliberately not covered.

`hermes-sema` is an optional `annotate` feature, off by default. It exists
only for `Annotation::Sem`, which prints each identifier's resolved binding
inline.

### `hermes-parser` 0.1.2

Two Flow parser fixes, both found by generating source from real trees and
reparsing it:

- `%checks` predicates after `declare function` / `declare hook` were never
  recognized, so `declare function isString(x: mixed): boolean %checks;` did
  not parse at all. Fixing it also repaired the `declare hook` diagnostic,
  which had been reporting the wrong error.
- A `match` statement whose case body was not a block panicked instead of
  reporting an error. The equivalent assertion failure existed in C++ Hermes
  and was fixed there too.

### Documentation

Crate READMEs no longer carry a hand-written version number — crates.io and
docs.rs already show it, and a typed one goes stale the moment a crate is
bumped (0.1.2 shipped a README saying 0.1.1). Links into the repository now
follow the `rust` branch instead of being pinned to a release tag, so they do
not rot against the next release.

The project README drops the "only complete Flow parser in Rust" claim and the
feature-matrix comparison against other Rust parsers.

---

## [0.1.1] — 2026-08-15

Prompted by an external usability review of the published 0.1.0 crates
(`doc/superpowers/2026-08-15-crate-usability-review.md`), whose findings were
all documentation-level. **Additive:** no existing signature changes, no
behavior change to parsing or resolution — everything below is new API, a new
example, a fixed example, or documentation. Code written against 0.1.0
compiles unchanged.

**This release covers four of the seven crates.** Only the crates whose
packaged content actually changed are bumped:

| crate | 0.1.1 | why |
|---|---|---|
| `hermes-atom-table` | **yes** | the two new `AtomTable` accessors |
| `hermes-ast` | **yes** | the `GCLock` mirrors + the generated node accessors |
| `hermes-parser` | **yes** | quickstart, `messages()`/`&mut self` docs, the `eprint!` fix, a new test |
| `hermes-sema` | **yes** | quickstart, `&mut self` docs, `print_bindings.rs` |
| `hermes-unicode` | no — stays **0.1.0** | byte-identical to the published 0.1.0 |
| `hermes-support` | no — stays **0.1.0** | byte-identical to the published 0.1.0 |
| `hermes-command-line` | no — stays **0.1.0** | byte-identical to the published 0.1.0 |

Two inter-crate dependency pins were raised along with the versions, because
`^0.1.0` would otherwise permit a resolution that does not compile:
`hermes-ast` → `hermes-atom-table` **0.1.1** (its `GCLock` delegates to the
new `AtomTable` methods) and `hermes-parser`/`hermes-sema` → `hermes-ast`
**0.1.1** (their doctests, tests and example call the generated node
accessors). `hermes-sema` → `hermes-parser` stays at `0.1.0`: `hermes-parser`
0.1.1 adds no API.

### Added

#### Atom → string accessors (`hermes-atom-table`, `hermes-ast`)
- `AtomTable::bytes_str_lossy` / `AtomTable::try_bytes_str`, mirrored on
  `GCLock` so `gc.bytes_str_lossy(atom)` needs no trip through
  `gc.ctx().atom_table()`. Atom bytes are WTF-8: a surrogate **pair** is
  folded back into the supplementary-plane character it encodes, so `"😀"`
  converts exactly and `try_bytes_str` returns `Some`. Only an **unpaired**
  surrogate — a legal JS string value with no UTF-8 form — yields `None`, or
  exactly one `U+FFFD` from the lossy form. `bytes()` is unchanged and remains
  the exact-bytes accessor.
- Generated per-field accessors on AST nodes (`gen_nodes.py`): `<field>_str`
  for each of the 32 `NodeLabel` fields (identifier names, operators,
  keyword-like kinds), and `try_<field>_str` + `<field>_str_lossy` — with no
  plain `<field>_str` — for each of the 10 `NodeString` fields (string-literal
  values, cooked template elements). The asymmetry is deliberate: a name with
  no UTF-8 form means something is broken, while a string value with none is
  legal JS that a codegen tool must not silently corrupt.

#### Example (`hermes-sema`)
- `examples/print_bindings.rs` — parse, resolve, walk with `Visitor`, and
  print every identifier with the binding kind it resolved to
  (`counter -> Let`, `by -> Parameter`,
  `console -> UndeclaredGlobalProperty`). It demonstrates the atom→string
  path and the pattern for holding a `&GCLock` inside a visitor: give the lock
  its own lifetime parameters, since `GCLock<'ast, 'ctx>` is invariant in
  `'ast` and cannot be tied to the visitor's `'gc`.

### Fixed

- `hermes-parser` `examples/parse_to_estree_json.rs` printed a spurious blank
  line after every diagnostic: `ParseError::messages()` strings are already
  newline-terminated, so the loop needs `eprint!`, not `eprintln!`
  (`resolve_and_dump.rs` was already correct).

### Documentation

- `ParseError::messages` and `ResolveError::messages` now state that each
  returned string ends with a newline — the omission is what caused the bug
  above.
- `ParsedJS::with_program` / `to_estree_json` / `to_estree_json_with` and
  `ResolvedJS::with_program` / `to_sema_dump` explain why they take
  `&mut self` for a logically read-only operation: reading the AST takes the
  arena lock, `Context::lock` takes `&mut self`, and that exclusive borrow is
  what prevents `Context::gc` from invalidating live `&Node`s mid-walk.
- Both crates' quickstarts now show the atom→string path, name the
  `try_*`/`_lossy` split for string values, and point at `GCLock::bytes` for
  the exact bytes.

---

## [0.1.0] — 2026-08-12

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
  oracle (17-file corpus including astral, lone-surrogate, hidden-class-shape,
  number edge cases, and 7 error cases).

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
  77-file plain-JS corpus.

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

#### JS parser — TypeScript (`hermes-parser`, P7)
- Complete port of `JSParserImpl-ts.cpp` (27 TS methods) plus its 26
  integration sites in `JSParserImpl.cpp`.
- Type annotations: primary types, type references, qualified names, type
  queries, tuples, union/intersection/postfix, conditional types, type
  predicates, type parameters and arguments.
- Function/constructor/parenthesized types; parameter properties; object types
  (call/method/property/index signatures); interface (with heritage), enum and
  namespace declarations.
- Expression and module integration: type args on call/`new`/`?.`, `<Type>`
  casts, `as`/`as const`, typed arrows, `import type`, member modifiers.
- Off by default; enabled with the `parse_ts` `Context` flag (`-parse-ts` on
  hermesc, `--parse-ts` on `ast-dump`), mutually exclusive with Flow, and it
  does not leak into plain-JS or Flow parsing.
- Byte-for-byte differential over a 20-file TypeScript corpus.

#### JS parser — JSX (`hermes-parser`, P8)
- Complete port of `JSParserImpl-jsx.cpp` (12 methods + `tagNamesMatch`):
  elements, fragments, children, namespaced and member-expression tag names,
  attributes including spread, expression containers (including empty `{}`),
  closing-tag matching, and the opening-tag Flow `<TypeArgs>` production.
- The `jsx_depth` counter + `JsxDepthGuard` driving the lexer-mode switch
  between JSX-text mode and JS mode.
- Off by default; enabled with the `parse_jsx` `Context` flag, independent of
  the Flow and TS flags, and it does not leak into other dialects.
- Byte-for-byte differential over a 6-file JSX corpus plus a 1-file JSX/Flow
  corpus.

#### Pre-parse and lazy-parse passes (`hermes-parser`)
- The three-pass `ParserPass{FullParse,PreParse,LazyParse}` machinery, the
  `PreParsedFunctionInfo`/`PreParsedBufferInfo` side table, the preemptive-
  compilation threshold, the `SaveFunctionState` guard, and on-demand
  `parse_lazy_function` (the 5-kind demand dispatch).
- Arena reclamation: `support::Deque::truncate`/`iter_from` plus a
  `GCLock`-scoped `ast::AllocationScope`, porting the C++ `AllocationScope`
  discipline.
- Two complementary oracles, since `-dump-ast` is blind to lazy parsing:
  byte-for-byte `preparse_differential` of the side table against a C++
  `tools/preparse-dump/` oracle (152 corpus files), and a `lazy_reparse` test
  proving deferred bodies reparse to the eager, hermesc-verified AST.

#### Semantic analysis (`hermes-sema`)
- Port of `lib/Sema` for the untyped (non-FlowChecker) eager path:
  `DeclCollector`, `SemanticResolver` (scope tree, `Decl` creation, identifier
  resolution, `Unresolver` for `eval`/`with`), `ScopedFunctionPromoter`,
  `SemContext`/`Decl`/`LexicalScope`/`FunctionInfo`, `ASTEval` constant
  folding, and the `semDump`/`SemContextDumper` printers.
- Two of the five C++ resolver entry points: `resolve_ast` (`resolveAST`,
  `compile = true`, ambient declaration files, AST rewrites) and
  `resolve_ast_for_parser` (`resolveASTForParser`, `compile = false`).
- A one-call façade over `hermes-parser`'s `ParsedJS`, matching
  `hermes_parser::parse`'s ergonomics: `resolve` (the parser path plus the
  error check C++ callers make by hand), `resolve_for_parser` and
  `resolve_for_compile`, returning a `ResolvedJS` that owns the arena, the
  rewritten root and the `SemContext`. `hermes-parser` + `hermes-sema` are
  therefore the only two dependencies a full front end needs.
- Not yet ported, and loud rather than silent where reached: the `$SHBuiltin`
  module protocol (`visitModuleFactory`/`visitModuleExport`/`visitModuleImport`
  and `resolveCommonJSAST`) — the three branches in `resolver/calls.rs` panic
  with a pointer at the C++ lines; and the lazy-compilation and `eval` entries
  (`resolveASTLazy`, `resolveASTInScope`), which need `SemContext`'s
  parent/child tree, its shared binding table, and the third
  `getPromotedScopedFuncDecls` call site (`SemanticResolver.cpp:158`, in
  `runInScope`). The FlowChecker is a separate C++ component and out of scope
  for this crate.
- Its public modules are therefore not all equally settled: see the crate
  documentation's *Stability* section for the stable core and the seven
  advanced / port-internal modules.
- The validation diagnostics `SemanticResolver` owns — redeclarations, invalid
  assignment targets, strict-mode restrictions, label/`break`/`continue`
  rules, `super`/`return` placement, class-field and private-name rules,
  generator/`await` context — rendered byte-compatibly with `hermesc`.
- Byte-for-byte differential (stdout, stderr and exit status) against
  `hermesc -dump-sema` over 219 corpus files and against the C++
  `sema-parser-dump` tool over 13.

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

#### Packaging
- The seven published crates are named `hermes-parser`, `hermes-ast`,
  `hermes-sema`, `hermes-support`, `hermes-atom-table`, `hermes-unicode` and
  `hermes-command-line`, so the public import paths are `hermes_parser::…`,
  `hermes_ast::…`, `hermes_sema::…`, `hermes_support::…`,
  `hermes_atom_table::…`, `hermes_unicode::…` and `hermes_command_line::…`.
  There are no `[lib] name` overrides — the lib names are the package names
  with underscores.
- `hermes-command-line` is the LLVM-`cl`-style option parser the project's own
  CLI drivers are built on. It is the one published crate that is not in
  `hermes-parser`'s dependency closure; it is dependency-free itself.
- `hermes-parser` re-exports the AST crate under the short name `ast`, so
  `hermes_parser::ast::node::Node` and `hermes_ast::node::Node` name the same
  item, and depending on `hermes-parser` alone is enough.
- The in-tree directories keep their short names (`rust/crates/parser`, …);
  only the Cargo package names changed. Cargo commands take the package name:
  `cargo test -p hermes-parser`, not `-p parser`.

#### Tooling
- The CLI drivers live in the unpublished `tools` crate (`publish = false`),
  so the published `hermes-parser` library ships no binaries and does not
  depend on `hermes-command-line`.
- `ast-dump` binary: parses a JS/Flow file and dumps ESTree JSON, matching
  `hermesc -dump-ast` byte-for-byte.
- `json-parse-dump` binary: parses a JSON file and re-emits it.
- `gen-json` binary: generates deterministic JSON corpora for benchmarking.
- `preparse-dump` binary: dumps the pre-parse side table, matching the C++
  `preparse-dump` oracle byte-for-byte.
- `sema-dump` binary: resolves a JS file and dumps the `SemContext` +
  annotated AST, matching `hermesc -dump-sema` byte-for-byte; with
  `--parser-entry` it matches the C++ `sema-parser-dump` tool instead.
- C++ differential oracle tools (`tools/js-lexer-dump/`, `tools/json-parse-dump/`)
  registered via `add_hermes_tool`.

### Not yet available

- `hermes-ir`, `hermes-optimizer`, `hermes-bcgen` — future components in the
  port roadmap.
