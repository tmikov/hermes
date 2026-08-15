# Usability review: hermes-parser 0.1.0 / hermes-sema 0.1.0

Reviewed 2026-08-15, from a first-time external user's perspective: READMEs,
lib.rs docs, shipped examples, and the façade API, exercised by a smoke-test
app in this directory (`src/main.rs`) covering the full pipeline:
parse → AST walk with `Visitor` → ESTree JSON dump → `hermes_sema::resolve` →
per-identifier binding queries via `SemContext::get_expression_decl` → both
error paths (parse error, redeclaration at resolve time).

## What's good

- Clean install, tiny dependency tree (only external dependency is
  `bumpalo`), and docs.rs built both crates successfully.
- Documentation quality is well above the crates.io norm: the lib.rs
  quickstarts compile as written, the stability contract is explicit, and the
  error API (one-line `Display`, full `messages()` LLVM-style rendering,
  structured `diagnostics()`) is exactly what a CLI or an embedder wants.
- Validation behavior matches the C++ placement — e.g. `let let = 1` parses
  but is correctly rejected by sema.
- The smoke test's output correctly classifies bindings: `counter -> Let`,
  `step -> Parameter`, `#name -> PrivateField`,
  `console -> UndeclaredGlobalProperty`, and diagnostics render with proper
  LLVM-style carets.

## Usability defects (all minor, doc-level)

1. **Getting an identifier's name as a string is undocumented and
   non-obvious.** The only real stumbling block. `id.name` is a `Cell<Atom>`,
   so the path is `id.name.get()` → `gc.bytes(atom)` (WTF-8 `&[u8]`) or
   `gc.ctx().atom_table().str(atom)`. Nothing in either quickstart, README,
   or the three shipped examples ever turns a name into a string —
   `walk_ast.rs` counts node *kinds* and sidesteps it. Meanwhile rustc's
   error suggests `Cell::as_ptr`, a red herring. A doc line in the quickstart
   (or a `GCLock::str(atom) -> &str` convenience) would fix the single
   biggest first-contact friction.

2. **Storing `&GCLock` inside a `Visitor` hits a cryptic variance error.**
   The natural first attempt — tying the lock's `'ast` to the visitor's
   `'gc` — fails with "lifetime may not live long enough …
   `GCLock<'ast, 'ctx>` is invariant." The fix is to keep the lock's
   lifetimes independent of the visitor's `'gc` parameter, but nothing
   documents that pattern. Since "walk the tree and print names/bindings" is
   the canonical use of these crates, one example doing exactly that would
   cover both this and defect #1.

3. **Bug in the shipped `parse_to_estree_json.rs` example:** it prints
   diagnostics with `eprintln!("{m}")`, but `messages()` strings already end
   in `\n` (verified in `render.rs` and at runtime), so every diagnostic gets
   a spurious blank line. `resolve_and_dump.rs` uses `eprint!` correctly.
   Worth fixing the example and stating the trailing newline in
   `messages()`'s doc comment.

4. **Nit:** `ParsedJS::with_program` and `to_estree_json` take `&mut self`
   for logically read-only access. The GCLock design explains it, but the
   docs never say *why* it's `&mut`; a sentence would preempt the "why can't
   I share this?" question.

## Verdict

No API-shape defects — the façade held up well under real use, and the test
app needed no workarounds once the atom-to-string path was found.
