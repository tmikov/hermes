# `tests/sema_corpus_parser` corpus (S4a Task 2)

Companion corpus to `tests/sema_corpus/MANIFEST.md`, but for a DIFFERENT
oracle pair: the C++ `tools/sema-parser-dump/sema-parser-dump.cpp` tool vs
Rust `sema-dump --parser-entry`, exercised by `sema_parser_differential` in
`sema_differential.rs`. Both sides resolve via `resolveASTForParser`
(`SemResolve.cpp:295-306` / `resolve::resolve_ast_for_parser`) — the
`compile = false` entry point `hermes-parser-wasm.cpp:104` uses — instead of
`resolveAST`/`resolve_ast` (`compile = true`), and the C++ tool dumps
UNCONDITIONALLY (even when diagnostics were emitted), which `hermesc
-dump-sema` never does (`CompilerDriver.cpp:960-974` skips the dump on a
`resolveAST` failure). Every file here was run as `sema-parser-dump <file>`
vs `sema-dump --parser-entry <file>` (no extra flags — matching what
`sema_parser_differential` actually invokes) before being imported, per the
global constraint that every corpus file is verified against the C++ side
FIRST with the raw stdout+stderr+exit triple.

`read_dir` in `run_differential` is non-recursive, so the `pending/`
subdirectory below is automatically excluded from the walk — no extra
filtering code needed.

## Imported (live differential gate)

| File | Covers |
|---|---|
| `plain.js` | `var x = 1 + 2; print(x);`. Pins two `compile = false` behaviors at once: (1) NO ambient globals — `resolveASTForParser` passes `ambientDecls = nullptr`, so the dump's `Scope %s.1` contains only `x` (declared) and `print` (resolves `UndeclaredGlobalProperty`, not one of `libhermes`'s 63 ambient decls); (2) NO constant folding — `+`/`-` folding is gated on `compile_` (cpp:405-436), so the dump shows the unfolded `BinaryExpression`/`BinOp +` tree via `ASTPrinter`, not a folded `NumericLiteral 3`. Verified: `sema-parser-dump plain.js` exit 0, no stderr; `sema-dump --parser-entry plain.js` byte-identical stdout. |

## Pending (excluded from the walk — `pending/` subdirectory)

| File | Blocked on | Target phase |
|---|---|---|
| `compile-false-basics.js` | `export default function f(){}`. On the C++ side this pins that NO module-mode error is emitted under `compile = false` (the export gate is `compile_ &&`, cpp:1511) — verified: `sema-parser-dump compile-false-basics.js` exits 0 with a full dump (`ExportDefaultDeclaration` → `FunctionDeclaration`). On the Rust side `sema-dump --parser-entry` PANICS: `resolver/mod.rs`'s catch-all hits `Node::ExportDefaultDeclaration`, which is one of the four module-visit arms explicitly reserved for S4a Task 3 (per the plan's global constraints: "ONLY the four module-visit arms replace catch-all panics in this phase"). Move to the parent directory (out of `pending/`) once Task 3 lands `Import`/`Export*` visits. | S4a Task 3 |

## Gate

`sema differential (tests/sema_corpus_parser): 1 corpus files matched (1
succeeded on the oracle)` — as of this task. The non-degeneracy guard in
`run_differential` (at least one oracle success) is satisfied by `plain.js`
itself.

## Known gap (not exercised by the current gate)

`sema-dump --parser-entry`'s doc (`src/bin/sema_dump.rs`) flags a structural
divergence from the C++ oracle: if `resolve_ast_for_parser` reports an error
ANYWHERE in the tree, `SemanticResolver::run` (`resolver/mod.rs`) returns
`None` and discards the fully-rewritten tree — there is no way for this port
to recover it for dumping. The C++ oracle mutates in place, so its
partially-annotated tree survives a `false` return from `resolveASTForParser`
and still gets dumped. Every file in this corpus today resolves with zero
errors, so the gap is latent; a future corpus file that legitimately produces
a `compile = false` resolution error (as opposed to a parked-pending panic)
would need this addressed first — flagged here rather than guessed at, per
the task's escalation guidance.
