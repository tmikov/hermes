# hermes-sema

Semantic analysis for the Hermes Rust front-end, by Tzvetan Mikov, the
architect of Hermes. Not an official Meta project and not supported by Meta.
Part of the `hermes-parser` crate family.

Parsing gives you a tree; semantic analysis tells you what the names in it
mean. This crate is the Rust port of Hermes' `lib/Sema` — the pass that runs
between the parser and the compiler back end:

- **Declaration collection** (`decl_collector`) — walks each function body and
  groups the declarations that belong to every scope, distinguishing
  hoisted `var`/function declarations from lexically scoped
  `let`/`const`/`class`.
- **Scope and binding resolution** (`resolver`) — builds the lexical scope
  tree, creates a `Decl` for every binding, and resolves every identifier
  reference to the declaration it names (or marks it as an undeclared global).
  Function parameter lists get their own scope when the ES2017 rules require
  it; `eval` and `with` make the scopes they touch unresolvable.
- **Validation** — the diagnostics `SemanticResolver` is responsible for:
  redeclaration errors, invalid assignment targets, `delete` of a variable in
  strict mode, restricted `arguments`/`eval` declarations, `break`/`continue`
  outside a loop or without a matching label, duplicate labels, `super` and
  `return` outside a function, class-field and private-name rules, generator
  and `await` context rules, and the strict-mode-directive checks.
- **AST rewrites** — the transformations sema is allowed to perform on the
  compile path: constant folding of `+`/`-` chains and of unary operators, an
  expression-bodied arrow rewritten to a block with a `return`, `try`/`catch`/
  `finally` split into nested `try`s, `$SHBuiltin.x` collapsed to an
  `SHBuiltin` node, an anonymous `export default function` turned into a
  function expression, and the block-scoped-function promotion of
  `ScopedFunctionPromoter`.

The result is a `SemContext`: the `Decl`, `LexicalScope` and `FunctionInfo`
records, plus the side tables mapping AST nodes to them.

## Entry points

Two, mirroring the two the C++ has:

- `resolve::resolve_ast` — the **compile** path (`compile = true`, C++
  `sema::resolveAST`). It takes a list of ambient declaration files, performs
  the AST rewrites above, and returns `None` when resolution fails, so a
  caller can stop before code generation.
- `resolve::resolve_ast_for_parser` — the **parser** path (`compile = false`,
  C++ `sema::resolveASTForParser`). This is what a parser-only consumer wants:
  no ambient declarations, no constant folding and no other compile-only
  rewrite, and it always hands back a tree — the resolution errors are
  reported through the `SourceErrorManager` but do not suppress the result.

A one-call façade over `hermes-parser` + this crate is planned for the 0.1.0
release; until it lands, parse with `hermes_parser` and pass the tree to one
of the two entry points above.

## Validation

Correctness is established by differential testing against the C++ Hermes
binaries, comparing stdout, stderr and process exit status byte-for-byte:

- **219 corpus files** against `hermesc -dump-sema` (the compile path), of
  which 109 are files the C++ compiler succeeds on; the remaining ones pin the
  diagnostics, their source-line-and-caret rendering, and the exit code on the
  error path.
- **13 corpus files** against the C++ `sema-parser-dump` tool (the parser
  path), 5 of them successes.

The corpora and their provenance are recorded in
[`tests/sema_corpus/MANIFEST.md`](https://github.com/tmikov/hermes/blob/rust1/rust/crates/sema/tests/sema_corpus/MANIFEST.md)
and
[`tests/sema_corpus_parser/MANIFEST.md`](https://github.com/tmikov/hermes/blob/rust1/rust/crates/sema/tests/sema_corpus_parser/MANIFEST.md).
On top of that, `SemContext`, `DeclCollector`, the AST folder and the dumpers
carry unit tests transcribed switch-arm-for-switch-arm from the C++ sources
they port.

Zero `unsafe` (`unsafe_code = "forbid"`).

**Version:** 0.1.0 — API docs at [docs.rs/hermes-sema](https://docs.rs/hermes-sema).

See [the project README](https://github.com/tmikov/hermes/blob/rust1/rust/README.md) for the full
documentation of the crate family.
