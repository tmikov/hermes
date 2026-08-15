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

## The façade

One call, taking `hermes-parser`'s `ParsedJS`:

```rust
use hermes_parser::{parse, ParseFlags};
use hermes_sema::sem_context::DeclKind;

let parsed = parse("var x = 1; x;", ParseFlags::default())?;
let mut resolved = hermes_sema::resolve(parsed)?;

// Every identifier now has a declaration to point at.
let dump = resolved.to_sema_dump();          // `hermesc -dump-sema` text
let sem = resolved.sem_context();            // Decls, scopes, functions
```

`resolve` **consumes** the `ParsedJS` and returns a `ResolvedJS` owning the
arena, the resolved AST and the `SemContext`. That is not just tidiness: the
resolver is a transforming visitor, so the root that comes out is a different
node than the one that went in, and keeping the old one would silently read a
stale tree.

Read the tree with `ResolvedJS::with_program(|gc, root, sem| …)`, which hands
the closure the `SemContext` alongside the node — resolution's whole point is
asking, of a given identifier, which declaration it binds to.
`ResolvedJS::into_parsed` gives the `ParsedJS` back (with the resolved tree)
when you want the ESTree JSON dumper afterwards.

`examples/print_bindings.rs` does exactly that for every identifier in a file
(`counter -> Let`, `by -> Parameter`, `console -> UndeclaredGlobalProperty`).
It is also where the two lifetime-shaped gotchas are written down: names are
interned atoms, read with the generated `id.name_str(gc)` accessor, and a
`Visitor` that keeps the `&GCLock` in a field must give the lock its own
lifetime parameters — `GCLock<'ast, 'ctx>` is invariant in `'ast`, so reusing
the visitor's `'gc` for it does not compile.

## Entry points

Three functions over the two the C++ has:

- `resolve` — the **parser** path plus the error check its C++ callers make by
  hand: `Err(ResolveError)` if resolution reported anything. The default.
- `resolve_for_parser` — the **parser** path exactly as C++
  `sema::resolveASTForParser` defines it (`compile = false`): no ambient
  declarations, no constant folding and no other compile-only rewrite, and it
  *always* hands back a tree — resolution errors are reported through the
  `SourceErrorManager` (`ResolvedJS::error_count`) but do not suppress the
  result.
- `resolve_for_compile` — the **compile** path (`compile = true`, C++
  `sema::resolveAST`). It declares the standard globals (and any
  `GlobalDefinitions` you add) as ambient declarations, performs the AST
  rewrites listed above, rejects what the compiler cannot handle, and fails
  outright rather than returning a tree.

The two underlying entry points, `resolve::resolve_ast` and
`resolve::resolve_ast_for_parser`, stay public: use them directly to share one
`SemContext` across files or to drive a hand-built arena, the way
`sema-dump` does.

## Stability

Pre-1.0, and the port is not finished, so not every public module is equally
settled. The **stable** surface — what 0.1.x means to keep source-compatible —
is the façade (`resolve`, `resolve_for_parser`, `resolve_for_compile`,
`ResolvedJS`, `ResolveError`), the two entry points in the `resolve` module,
and the result model (`sem_context`, `ids`).

The other seven modules — `resolver`, `decl_collector`, `ast_eval`, `dump`,
`dump_context`, `libhermes`, `keywords` — are **advanced / port-internal**.
They are public because `sema-dump` and this crate's integration tests drive
them directly, not because their shape is settled; they may change or be made
private in a 0.x bump. Each says so in its own module documentation.

One honest caveat about that stable surface: it reaches into `hermes-support`.
`SemContext::binding_table()` returns a
`hermes_support::persistent_scoped_map::PersistentScopedMap`, and the public
`LexicalScope::binding_table_scope` field is a `ScopePtr` into it — so
`PersistentScopedMap`, `Scope` and `ScopePtr` are part of what `sem_context`
guarantees, even though the family README tells you to depend on
`hermes-support` directly at your own risk. Those three types are covered by
this crate's 0.1.x source-compatibility promise; the rest of `hermes-support`
is not.

Still unported from `lib/Sema`'s untyped path, and loud rather than silent
where reached: the `$SHBuiltin` module protocol (`visitModuleFactory` /
`visitModuleExport` / `visitModuleImport`, `resolveCommonJSAST`), and the
lazy-compilation and `eval` entry points (`resolveASTLazy`,
`resolveASTInScope`), which need `SemContext`'s parent/child tree and shared
binding table. The FlowChecker is a separate C++ component and not part of
this crate.

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
[`tests/sema_corpus/MANIFEST.md`](https://github.com/tmikov/hermes/blob/hermes-crates-v0.1.1/rust/crates/sema/tests/sema_corpus/MANIFEST.md)
and
[`tests/sema_corpus_parser/MANIFEST.md`](https://github.com/tmikov/hermes/blob/hermes-crates-v0.1.1/rust/crates/sema/tests/sema_corpus_parser/MANIFEST.md).
On top of that, `SemContext`, `DeclCollector`, the AST folder and the dumpers
carry unit tests transcribed switch-arm-for-switch-arm from the C++ sources
they port.

Zero `unsafe` (`unsafe_code = "forbid"`).

**Version:** 0.1.1 — API docs at [docs.rs/hermes-sema](https://docs.rs/hermes-sema).

See [the project README](https://github.com/tmikov/hermes/blob/hermes-crates-v0.1.1/rust/README.md) for the full
documentation of the crate family.
