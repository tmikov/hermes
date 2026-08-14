# Sema S4a — standalone-front-end sema: design

**Date:** 2026-08-03. **Parent spec:** `2026-07-26-sema-untyped-design.md` (§6 phase
list; S4a/S4b split decided 2026-08-02, commit `aba3f9341`). **Status of the port:**
Sema S0–S3 DONE; gate at 173 corpus files matched (97 succeeding on hermesc); upstream
sweep 1416 = 1209 identical / 190 mismatch / 17 panic.

**Executed (2026-08-03), commits `041959a07..57221f7de`:** all 6 tasks landed as
planned; gate 173 → **192** files (97 → **103** succeeding on hermesc), new
parser-entry gate **7** files (**2** succeeding), upstream sweep → **1218/190/8**
— see the roadmap's Sema row for the full what-shipped.

## 1. Goal and boundary

S4a serves the **standalone parser/front-end** (the publication goal): make the
resolver complete and byte-verified for every input a parser-consumer can present —
including files containing `import`/`export` and untyped flow syntax — and port
`resolve_ast_for_parser`, the entry the standalone tooling actually uses.

**S4b (VM modules) is a genuinely separate, much later phase** (near IRGen): the
`$SHBuiltin.moduleFactory`/`export`/`import` protocol, `runCommonJSModule`/CJS
wrapping, and rewrite #4's *corpus pinning* (see §4 — its *code* lands here, by
explicit ruling). The shared "S4" number avoids renumbering S5. The `$SHBuiltin`
branches in `resolver/calls.rs` keep their loud phase-tagged panics through S4a.

**Not S4a:** the 178 `test/Sema/flow/**` files (all `-typed` → the future FlowChecker
component); `-lazy` (S5); `-commonjs` (S4b).

## 2. Validation infrastructure (new)

### 2.1 The `sema-parser-dump` oracle pair

`hermesc -dump-sema` always runs the driver path (`compile = true`) and skips the dump
when errors were emitted (verified 2026-08-02: `export default function(){}` in plain
mode prints only the error, exit 2, no dump). Consequently neither the
`compile = false` behavior nor `Decl::Kind::Import` decls (only present in a
*successful* dump, which the driver only produces under `-commonjs` — S4b) are
observable through the existing differential.

New C++ tool **`tools/sema-parser-dump/`** (the `js-lexer-dump`/`json-parse-dump`/
`preparse-dump` precedent; registered via `add_hermes_tool`): parse, then call
`resolveASTForParser` (`SemResolve.cpp:295` — `compile = false`,
`ambientDecls = nullptr`, `saveDecls = nullptr`; the exact call
`tools/hermes-parser-wasm.cpp:104` makes), then print the same
`SemContextDumper::printSemContext` + `ASTPrinter` output shape as `-dump-sema` —
**unconditionally**, errors on stderr, dump on stdout even when errors were reported,
exit code still reflecting error status.

Rust side: `sema-dump` grows a **`--parser-entry`** mode calling the new
`sema::resolve_ast_for_parser`, mirroring the tool byte-for-byte. A second corpus dir
**`rust/crates/sema/tests/sema_corpus_parser/`** runs through the pair with the same
three-channel raw-byte comparison (stdout + stderr + exit), in the same
`sema_differential.rs` (or a sibling test — implementation choice), with
`REQUIRE_DIFFERENTIAL=1` semantics.

What `compile = false` changes (each byte-verified by this differential for the first
time): no ambient globals in scope %s.1, no export module-mode errors, no rewrite #4,
no import-assertions error, no `+`/`-` constant folding — every `compile_` guard
ported in S0–S3 goes live.

### 2.2 The `// FLAGS:` per-file-flag harness

`sema_differential.rs` reads a corpus file's **first line only**; if it is exactly
`// FLAGS: <args>`, the whitespace-split args are appended verbatim to **both**
binaries' invocations. Spellings are hermesc's own; `sema-dump` grows matching options
the way it grew `--ferror-limit` in S2: `-parse-flow`, `-enable-eval=false`,
`-fno-std-globals`. Files without the line run flagless — the existing 173 files are
untouched. `-commonjs` is NOT implemented in S4a; when S4b lands it is one more flag
through the same mechanism. (lit was considered and is viable; `// FLAGS:` was chosen
as the minimal extension of the existing cargo-test harness.)

Immediate payoff: the `type-alias-children.js` deferred MANIFEST row (blocked on
`-parse-flow` since S2) becomes importable.

## 3. Port content

### 3.1 The four module visits — new `resolver/modules.rs`

Faithful ports (S2 pattern: one module per C++ cluster):

- `visit(ImportDeclarationNode)` (cpp:874-890): the **unconditional** module-mode
  error (cpp:876-879 — NOT `compile_`-gated, unlike the export visits; bug-for-bug),
  the `compile_`-gated import-assertions error (cpp:881-885), the
  `curFunctionInfo()->imports` push (cpp:887), children.
- `visit(ExportNamedDeclarationNode)` (cpp:1510): `compile_`-gated
  `'export' statement requires module mode`; children.
- `visit(ExportDefaultDeclarationNode)` (cpp:1519): same gate/message; **rewrite #4
  inline** (§4); children.
- `visit(ExportAllDeclarationNode)` (cpp:1549): `compile_`-gated — note the DIFFERENT
  message text: `'export' statement requires CommonJS module mode`; children.

Preserve exact diagnostic strings including the Named/Default vs All wording
difference.

### 3.2 Import specifier declarations

The `extractDeclaredIdents` `ImportDeclarationNode` arm (cpp:2334+) makes
`Decl::Kind::Import` decls materialize — the one reachable decl kind with no corpus
exercise. Pinned by module-bearing files in the **parser-entry** corpus (the driver
dump can never show them in S4a).

### 3.3 `FunctionInfo::imports` backref

`SemContext.h:303-306` ("imports that need to be hoisted and materialized"). The push
happens mid-walk on nodes this port may rebuild → the S1-T7 `hoisted_functions`
post-rebuild fixup pattern applies verbatim. Whether the list is visible in the
parser-entry dump is verified during planning; if dump-blind, unit tests pin it (the
`mayReachImplicitReturn` precedent).

### 3.4 Untyped `-parse-flow` resolver paths

`typecast not allowed in this context` (cpp:1576) and
`'this' parameter requires typed mode` (cpp:1771), plus whatever their surrounding
visits need. Pinned by purpose-written `// FLAGS: -parse-flow` corpus files.

## 4. Rewrite #4 ruling (explicit, supersedes the 2026-08-02 S4b placement of the code)

Rewrite #4 (anonymous `export default function` → `FunctionExpression`,
cpp:1526-1544) sits **inline** in `visit(ExportDefaultDeclarationNode)` and is
`compile_`-gated but NOT module-mode-gated: plain-mode hermesc emits the module-mode
error and still performs the rewrite as the walk continues. In every S4a-reachable
mode it is oracle-invisible (plain mode: errors suppress the driver dump;
`compile = false`: the C++ skips it too). **Ruling (2026-08-03): S4a ports the rewrite
inline and faithfully** — no stub, no panic — carrying the C++ "cleaner IRGen"
comment plus a note that **S4b owns its corpus pinning** (`// FLAGS: -commonjs`
files once CJS wrapping exists). The S4b docs bullet is amended accordingly by T6.

## 5. Corpus strategy

- The clearable upstream sweep module-panic files (~9 of 16 — the non-`$SHBuiltin`
  ones) imported as plain error-corpus rows (error text + exit 2; dump suppressed).
- `type-alias-children.js` imported via `// FLAGS: -parse-flow`.
- New FLAGS batteries: `-parse-flow` (the two diagnostics), `-enable-eval=false`,
  `-fno-std-globals`.
- Seed parser-entry corpus: a clean plain file (shows no-ambient-globals + no
  folding), a module file (import/export — Import decls visible, no export errors),
  a flow file.
- MANIFEST accounting exact at every step, per the established discipline.

## 6. Task decomposition (6 tasks, S0–S3 conventions)

1. **T1 — `// FLAGS:` harness** + `sema-dump` options; proof = `type-alias-children.js`
   import + `-enable-eval=false`/`-fno-std-globals` mini-battery.
2. **T2 — the oracle pair**: `resolve_ast_for_parser` in `resolve.rs`, C++
   `tools/sema-parser-dump/`, `sema-dump --parser-entry`, seed `sema_corpus_parser/`
   (plain-JS only — module files arrive with T3) + its differential.
3. **T3 — module visits**: `resolver/modules.rs` (four visits, rewrite #4 inline),
   `imports` backref fixup, `Decl::Kind::Import` materialization; sweep-file imports;
   module files added to the parser-entry corpus.
4. **T4 — untyped `-parse-flow` paths** + battery.
5. **T5 — upstream re-probe**: expected panic drop 17 → ≤8 (the ≤7 `$SHBuiltin`
   protocol files — grep over the 8 sweep dirs, 2026-08-02 — plus
   `computed-fn-name.js`); exact arithmetic derived at execution, shown in MANIFEST.
6. **T6 — docs**: roadmap/parent-spec/handoff/MANIFEST; amend the S4b bullet per §4.

**Gates:** the driver differential (grows past 173), the new parser-entry
differential, full workspace suite, zero warnings both feature configs; T5's sweep
bucket arithmetic.

## 7. Explicitly deferred

- `$SHBuiltin` module protocol, CJS wrapping, rewrite #4 pinning → **S4b**.
- `-lazy`, `resolve_ast_lazy`/`resolve_ast_in_scope`, `runInScope` promotion site →
  **S5**.
- The 178 `test/Sema/flow/**` (`-typed`) files → **FlowChecker component**.
- Regex validation → the regex-engine component.
