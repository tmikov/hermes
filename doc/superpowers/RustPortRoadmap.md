# Hermes → Rust Port — Roadmap & Status

The single source of truth for *what* we are porting, *why*, *what is done*, and
*what is next*. Read this first when picking the effort back up. Component-level
specs/plans live under `doc/superpowers/specs/` and `doc/superpowers/plans/`.

## Goal & principles

Port the Hermes JavaScript compiler **front-end** to Rust, faithfully and incrementally.

- **Minimal `unsafe`.** Ideally none; where unavoidable, it must be *very well
  encapsulated* and never leak across module/crate boundaries. Each crate uses
  `unsafe_code = "forbid"` where possible (the `support` crate does).
- **Reuse juno by copying.** Useful code from `unsupported/juno/crates/` is **copied**
  into `rust/` and modified there, never referenced in place. (See `doc/JunoRustCrates.md`
  for the crate-by-crate analysis.)
- **Stay close to Hermes.** Keep the Rust structure close to the C++ where it makes
  sense and **copy the comments** (or keep them close), for traceability.
- **Faithful / byte-compatible where it matters.** Diagnostics are validated *byte-for-byte*
  against the real C++ `hermesc` binary.
- **Implement each component completely** (its whole public surface) in one pass — do not
  defer/stage features. The boundary is the *component*: don't pull in separate components.

## Repo layout & integration

- All Rust code lives under `rust/` — a Cargo workspace (`rust/Cargo.toml`) with member
  crates under `rust/crates/`. Toolchain pinned via `rust/rust-toolchain.toml` (1.96.0).
- Work lives on the **`rust`** branch and **stays there** — no merges, no PRs. The repo's
  main/base branch is **`static_h`** (not `main`).
- Build/test the workspace: `cargo test --manifest-path rust/Cargo.toml -p <crate>`.

## Component order & status

The front-end stratifies (see the dependency analysis below). We port bottom-up.

| Component | Crate / location | Status |
|-----------|------------------|--------|
| **SourceErrorManager** (+ buffer, locations, line index, diagnostics) | `rust/crates/support/` | ✅ **Complete** — entire public surface; **byte-for-byte validated vs `hermesc` 1.96.0** |
| **JS lexer** | `rust/crates/{atom_table,unicode,parser}/` | ✅ **Complete** — entire `JSLexer` public surface; self-validating byte-for-byte vs `js-lexer-dump` (5 differentials); see deps below |
| **JSONParser** (+ `JSONEmitter`, value model, factory, `JSONSharedValue`) | `rust/crates/{parser,support}/` | ✅ **Complete** — first `JSLexer` consumer; entire public surface; self-validating byte-for-byte vs `json-parse-dump` (16-file corpus) + 5 ported `JSONParserTest` + 13 ported `JSONEmitterTest`; **benchmarked within ~1.5% of C++ Release** |
| **AST** (ESTree nodes + GC arena) | `rust/crates/ast/` (+ `support`) | ✅ **Complete — all 4 phases.** Phase 1: juno GC arena copied+adapted; immutable children + `Cell` attributes. Phase 2: **full node set (271 nodes) generated from `ESTree.def`** by committed `gen_nodes.py` → `src/node.rs`; `NodeKind` ranges + `is_*`/`as_*`; generated `visit_children`/`mark_lists`; `new` constructors; idempotency guard. Phase 3: **transforming visitor** — generated `builder` module (clone-with-one-field-changed) + `VisitorMut`/`TransformResult{Unchanged,Removed,Changed,Expanded}` + `Path`/`NodeField` + generated `visit_children_mut` (functional rebuild); 7 transform tests; read `Visitor` unchanged. Phase 4: **`ESTreeJSONDumper`** — generator emits `Node::node_type_str` + `dump_children` (camelCase JSON keys + baked `IGNORE_IF_EMPTY` flags); `src/dump.rs` driver (3 modes, loc/range/raw, WTF-8 label emission via `support::utf8`); 9 golden tests; capstone clean. Spec: `specs/2026-06-03-ast-design.md`; plans: `plans/2026-06-04-ast-{2-node-codegen,3-builders-visitors}.md`, `plans/2026-06-05-ast-4-json-dumper.md` |
| Parser | `rust/crates/parser/src/js/` | ✅ **Complete — P0–P8 + the Pre/Lazy passes DONE (the ENTIRE standard-JS grammar + ALL of Flow + ALL of TypeScript + JSX + the three-pass Full/Pre/Lazy machinery).** P0: scaffold + driver + `ast-dump` bin + live byte-for-byte `parser_differential` vs `hermesc -dump-ast`. P1: value expressions. P2: statements & declarations. P3: functions/classes/arrows/async/generators/methods/`super`/`yield`/decorators. P4: modules (`import`/`export` declarations + `import()`/`import.meta`). P5: the **Flow type grammar** (annotation hierarchy, function/object/tuple types, type-params/args, generics, predicates, `type`/`opaque type`/`interface` declarations, non-ambiguous integration) behind `Context::parse_flow`, in `js/flow/`. P6: **the rest of Flow** — the ambiguous-expression grammar (typed arrows sync+async, `as`/`as const`, `(x:T)` casts + CoverTypedIdentifier, call/`new`/`?.` type-args), plus `enum`, `component`/`hook`, `record`, `match`, the `declare` statement family + `import type`/`export type` clauses, and the class-member `declare` modifier — behind four new flags (`parse_flow_ambiguous`/`_component_syntax`/`_records`/`_match`; `js/flow/match_.rs` is new). P7: **all of TypeScript** (`JSParserImpl-ts.cpp`, 27 methods + 26 integration sites — the type-annotation grammar, function/constructor/object types, interface/enum/namespace, type params/args, `<Type>` casts, `as`, typed arrows, class member modifiers, `import type`) behind `Context::parse_ts` (mutually exclusive with `parse_flow`), in a new `js/ts/` directory mirroring `js/flow/`. P8: **JSX** (`JSParserImpl-jsx.cpp`, 12 methods — elements/fragments/children/attributes/spread/expression-containers/namespaced+member names/closing-tag matching, the `jsx_depth`-driven lexer-mode switch) behind `Context::parse_jsx` (independent flag), in `js/jsx.rs`. **76 plain + 42 Flow + 8 component + 5 records + 7 match + 20 TS + 6 JSX + 1 flow·JSX corpus files, byte-for-byte** (each gated dir runs its hidden/dialect hermesc flag on both binaries). **No AST nodes added** (the generated 271-node set already covered all of Flow + TS + JSX — `generated_idempotent` stays green). **Pre/Lazy passes (L0–L2 + capstone):** `ParserPass{Full,Pre,Lazy}` + the `PreParsedBufferInfo` side-table, `SaveFunctionState` Drop-guard + arrow-bookkeeping, the PreParse store sites, the LazyParse skip-and-stub, and `parse_lazy_function`, gated by **two oracles** — Oracle B (the C++ `preparse-dump` tool + byte-for-byte side-table differential vs hermesc, 13+76 files) and Oracle A (Rust-only reparse-equivalence of deferred bodies vs the eager AST). The capstone caught a real flag-attribution bug (default-param arrows) that both oracles were initially blind to. **Next: Sema.** Spec: `specs/2026-06-06-js-parser-design.md`, `specs/2026-06-28-pre-lazy-passes-design.md`; plans: `plans/2026-06-06-js-parser-{p0-foundations,p1-expressions,p2-statements}.md`, `…p3-functions-classes`, `…p4-modules`, `2026-06-09-js-parser-p5-flow-types.md`, `2026-06-13-js-parser-p6-flow-extensions.md`, `2026-06-19-js-parser-p7-typescript.md`, `2026-06-19-js-parser-p8-jsx.md`, `2026-06-28-js-parser-pre-lazy-passes.md` |
| Sema (scope resolution + FlowChecker) | `rust/crates/sema/` | 🚧 **In progress — S0 (foundations), S1 (declarations & scopes), S2 (rest of the walk) and S3 (`ScopedFunctionPromoter`) DONE.** New crate `rust/crates/sema/` (deps `ast`/`support`/`atom_table`; `parser` + `command_line` optional behind feature `dump-bin`, keeping the published library's layering identical to C++ Sema, which consumes only the AST). S0 shipped: `NodeId(u32)` in `ast::NodeMetadata` (stamped at alloc, monotonic from 1, never reused) + a freed-id log appended by both node-freeing paths (the GC sweep and `AllocationScope::truncate`), drained via `Context::take_freed_node_ids()`; `support::persistent_scoped_map` (a safe-Rust, `Rc`-based port of `hermes/ADT/PersistentScopedMap.h`, all 8 C++ unit tests ported); `ids.rs` (`DeclId`/`ScopeId`/`FunctionInfoId`); `keywords.rs` (**133 atoms** — `Keywords.def`'s true count, corrected from the plan's estimated 136); `sem_context.rs` (records with 19 `DeclKind` variants, the decl-state machine, `NodeId`-keyed side maps); `dump_context.rs` + `dump.rs` (`SemContextDumper` + `ASTPrinter` + `sem_dump`, emitting raw bytes `Vec<u8>` — WTF-8 pass-through, no UTF-8 validation); `decl_collector.rs` (`NodeId`-keyed); `resolver/` (the S0 slice: `FunctionContext`/`ScopeRAII`/`scanDirectives`/`processAmbientDecls`/message-buffering via a Drop-flush guard; declaration/identifier-resolution visits panic explicitly — an intentional S1 boundary, not a silent gap); `resolve.rs` (`resolve_ast`); `libhermes.rs` (a byte-identical transcription of the ambient-globals list); `src/bin/sema_dump.rs` (installs a `StderrHandler`; diagnostics on stderr, matching hermesc). **Gate as of S0 (green then; see the live figure below):** `REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p sema --features dump-bin --test sema_differential -- --nocapture` → "sema differential (tests/sema_corpus): 6 corpus files matched" (stdout + stderr + exit status, vs `cmake-build-asan/bin/hermesc`; six files, not the plan's five — `inline-noinline.js` was added to exercise a real non-empty stderr comparison, a warning + caret matched byte-for-byte). **S1 planning notes (review-caught, carried forward):** (a) hermesc's sema **constant-folds `+`/`-` `BinaryExpression` chains** at `SemanticResolver.cpp:405-436` when `compile=true` — a 5th AST-mutation site beyond spec §3.4's list of four; (b) **exit-code/driver mismatch**: hermesc exits 2 on parse errors and its driver appends an `Emitted N errors. exiting.` stderr line, while the Rust bin currently exits 1 with no such line — the S1 error corpus must reconcile these; (c) `use_strict_node` is carried as `Option<&Node>` for the `SemanticResolver.cpp:1748-1751` diagnostic (an S1/S2 consumer). Spec: `specs/2026-07-26-sema-untyped-design.md`; plan: `plans/2026-07-26-sema-s0-foundations.md`.

**S1 — declarations & scopes DONE (2026-07-28), commits `53ddf2e92..77a41ed3e`.** T1: the resolver implements `ast::VisitorMut` directly — one phase earlier than spec §3.4 planned, because the C++ replaces children generically via `Node **ppNode` (constant folds included), not just at the four named rewrite sites; `resolve_ast` returns the possibly-new root; recursion-depth brackets (`kASTMaxRecursionDepth` = 1024, `resolver/mod.rs`) plus `MAX_NESTED_BINARY`/`MAX_NESTED_ASSIGNMENTS` (30000, `linearize.rs`) with shared `linearize_left`/`linearize_right`. T2: hermesc error-epilogue parity (exit 2, `Emitted N errors. exiting.`); error files are first-class corpus members (3-channel raw-byte stdout+stderr+exit-status comparison). Found+fixed a **parser-phase bug**: mis-ported `errorExpected` semantics (`JSParserImpl.cpp:175-226`) — 33 call sites corrected. T3: `ASTEval.cpp` constant folding ported (untyped scope, correcting the design spec's §1 table). T4: identifier-resolution core (cpp:277-323, 1967-2086) incl. the strict undefined-variable warning; the resolver is split into `resolver/{mod,identifiers}.rs` (`declarations`/`expressions`/`functions` added by T5-T7). T5: declarations — the full redeclaration matrix (cpp:2407-2639, ported as a verbatim comment block), extract/validate machinery, `VariableDeclaration` + `BlockStatement`. T6: expressions — folds wired through the functional rebuild (a failed fold stops folding but the rebuild still continues), assignment/update/unary validation. T7: functions — parameter scopes incl. the dual-scope layout, `arguments`, `FunctionExprName`, `return`; the §3.4(a) `hoisted_functions` backref fixup implemented + unit-tested (the differential is blind to it). T8: corpus sweep — **69 corpus files matched (42 succeed on hermesc)**; `rust/crates/sema/tests/sema_corpus/MANIFEST.md` records every `test/Sema` file (14 imported + 40 deferred with reasons/target phases; the 178 `flow/**` typed-dialect files are deferred wholesale).

**S2 — rest of the walk DONE (2026-07-28), commits `94b4695f1..dc2fb1661`.** T1: loops, labeled statements, `break`/`continue`, `switch` (cpp:520-756) in a new `resolver/statements.rs`; the `Switch` **decorate-after-children exception is discharged** — the label is written on the node that is RETURNED (the rebuilt one, seeded with the already-visited discriminant), not only on the original. T2: arrows + **§3.4 rewrite #1** (expression body → block + `return`, cpp:249-275) plus `yield`/`await`/spread/meta-property and the `Cover*` error visits (cpp:837-872, 1455-1509, 1558-1578). T3: try/catch/finally + **§3.4 rewrite #2** (the `catch`+`finally` → nested-try split, cpp:757-835), `with` + a new `resolver/unresolver.rs`, and the `RegExpLiteral` visit as an explicit **REGEX-ENGINE DEFERRED** stub (real pattern validation needs `lib/Regex/` — its own future component; the stub never reports an invalid pattern, so `test/AST/regexp.js` is the standing witness). `with` is the `Unresolver`'s ONLY live call site: the local-`eval` site at cpp:1931-1937 is `if (false && …)` in the C++ too, so that branch stays dead here as well. T4: classes core — a new `resolver/classes.rs` with `ClassContext`, class-as-expression, class properties, method definitions and `super` (cpp:891-1115, 3081-3186); the synthetic `FunctionInfo` ids live on the class node's `ClassLikeDecoration` (the static-block one on the `StaticBlock` node), and the class visit hand-drives its own children so the returned node is built LAST — **the second decorate-after-children exception found anywhere in the port**. It also fixed two parser-phase sites that reported `invalid expression` over a token RANGE where C++ uses the bare-caret `error(SMLoc, Twine)` overload (`JSParserImpl.cpp:2699-2706`); the fix generalizes past class fields (`var x = ;` has the same shape), which also says the parser differential's corpus has a hole there. T5: private names — the `collectDeclaredPrivateIdentifiers` declaration matrix, `#`-mangled decl names, the member-access restriction branches (cpp:952-1006, 1053-1084, 1207-1320, 2033-2066, 2143-2261) — and static blocks (`enter_function_static_block`; the plan's claim that it already existed from S0 was wrong). Pinned the `typeof arguments` double-fire quirk in both a static block and a class field. T6: call specials (cpp:1117-1205) in a new `resolver/calls.rs` — direct-`eval` detection + `registerLocalEval` (which needed a new `ast::Context::enable_eval`), **§3.4 rewrite #3** (`$SHBuiltin.prop(...)` → `SHBuiltin`, resolution-dependent), and the `super()` check. The sweep's single biggest unlock: **16** MANIFEST rows imported at once. T7: `CheckImplicitReturn` — all 335 lines of `lib/Sema/CheckImplicitReturn.cpp` as `src/check_implicit_return.rs`, wired at cpp:1939-1944 behind the `error_count() == 0` guard the C++ comment demands (which is why T1 had to land first). `FunctionInfo::mayReachImplicitReturn` is **invisible to `-dump-sema`** (read only by the FlowChecker and IRGen), so the net is `tests/check_implicit_return.rs` — 22 tests / 85 rows, every row independently confirmed against `hermesc -typed`. Two documented deviations: the entry point takes the *visited* body (this port rebuilds nodes, so `node->_body` is stale — the try-split has not happened on it yet), and `checkTerminationLoopOrLabeledStatement` takes the label index rather than the `LabelDecorationBase *` it only reads it from. T8: corpus sweep round 2 — all eight remaining Deferred rows re-probed (**none** unblockable; every stated reason confirmed), then coverage measured from the other end: six exhaustive inventories of the dump's own vocabulary (node kinds, `Decl::Kind`, `Decl::Special` and their pairs, the `[D:…]` printer's three branches, the resolver's 54 distinct diagnostics, `set_node_scope`'s 15 scope-bearing kinds) plus a differential run of both binaries over the **1416** `.js` files in the rest of `test/` — **1203 byte-identical, 190 mismatched, 23 panicking on known S3/S4 deferrals, and not one file hermesc compiles successfully disagrees** (i.e. every finding is on a diagnostic path). Three TDD'd fixes came out of it: the generic visit arm panicked on `BigIntLiteral`/`TaggedTemplateExpression`/`ImportExpression` (25 upstream files); `sema-dump` never applied the driver's `-ferror-limit` = 20 (`CompilerDriver.cpp:555-559`, now a real `--ferror-limit` option, `0` = unlimited); and `support` rendered `:0:0:` instead of `SourceMgr`'s `<unknown>:0:` for location-less diagnostics. **All three of S2's in-scope §3.4 rewrites shipped** (arrow body, try-split, `$SHBuiltin`); rewrite #4 (anonymous `export default function`) is S4 with the rest of modules.

**S3 — `ScopedFunctionPromoter` DONE (2026-07-29), commits `36593518b..274fa63b8`.** T1: ports all 328 lines of `lib/Sema/ScopedFunctionPromoter.cpp` (+ its 37-line header) as `resolver/promoter.rs` — the `ScopedFunctionPromoter` struct (binding scope over a `PersistentScopedMap<Atom, bool>`, `func_names`/`func_decls`), `run`/`process_parameters` (ES2022 B.3.2.1 29.a.ii)/`process_declarations` (the let-like-but-not-`ES5Catch` rule, ES14.0 B.3.4) and `extract_declared_idents` — the promoter's OWN copy, kept separate from `declarations.rs`'s decl-matrix extractor exactly as the C++ keeps them separate. Both S3 assert seams replaced: `visit_program` (cpp:224-227) and `visit_function_body_after_params_visited` (cpp:1904-1910); the new `process_promoted_func_decls` (cpp:2129-2141) lands in `resolver/mod.rs`, making S1 T5's dormant `promotedFuncDecls` redeclaration rows live. The dead `newDecls` local (cpp:174-206) is preserved with a `DEAD in C++ too` comment (verified: `grep newDecls` = 3 hits, all writes, no reader). A NEW hermesc self-abort landmine was found and reproduced faithfully: `using x = 1; { function f(){} }` trips `ScopedFunctionPromoter.cpp:260`'s `identVar` assertion before the `using`-rejection visit can fire, mirrored as a `debug_assert!`. The third C++ call site (`runInScope`, `SemanticResolver.cpp:158`) is left to S5 with a note at the `mod.rs` site. Forced deviations: `extract_declared_idents_from_id`'s body hoisted to a free function over `&mut SourceErrorManager` (the promoter holds three disjoint field borrows — `&DeclCollector`/`&SemContext`/`&mut SourceErrorManager` — never `&mut SemanticResolver`); `get_promoted_scoped_func_decls` returns `Vec<NodeRc>`, not `Vec<&'gc Node>` (`NodeRc::node`'s lifetime ties to the call's borrow); `process_promoted_func_decls` `expect`s a non-null decl where C++ can store a null `Decl *` (argued unreachable at the site — the port's `HashMap<Atom, DeclId>` map shape can't represent null anyway). Gate: 160 → **162** corpus files (88 → **90** succeeding on hermesc); 3 dump-blind unit tests cover the `promotedFunctionDecls_` side table the differential can't see. T2: imports the three S3-blocked `test/Sema` rows (`break-in-nested-func.js`, `function-redeclaration-error.js`, `regress-function-promotion-decl.js`; Imported 46→49, Deferred 8→5) plus a promotion battery — six files, then a seventh (`promotion-es5catch-var-shadows.js`) added after a review round found the first attempt's `promotion-var-shadows-promoted.js` didn't isolate `prevIsLexicalBindingOfPromotedFunc` (cpp:365-374) as the error's SOLE cause (its shape had `prevKind == ScopedFunction`, independently let-like via cpp:392's ordinary check); the new file uses `prevKind == ES5Catch`, the one case cpp:392 explicitly excludes (`!= ES5Catch`, the B.3.5 exemption), so only the flag can produce the error. The battery also covers destructuring-`Catch` blocking, nested-scope `let` visibility, `Var`/`ScopedFunc` same- and cross-scope reuse, and the ES5Catch cross-scope reuse row S1-T5 left dormant. The `with`-arm is confirmed not corpus-reachable (`SemanticResolver` always sets `compile_`, so `with` itself errors before the promoter's `With` arm is ever reached) and documented instead of stubbed; the `cpp:2551-2552` same-scope `Var`/`ScopedFunction` sub-arm is documented structurally unreachable (`processPromotedFuncDecls` never declares with `kind = ScopedFunction`). Gate: 162 → **172** corpus files (90 → **96** succeeding on hermesc). T3: re-ran the S2-T8 upstream sweep (the same 1416 files across `test/{Parser,IRGen,BCGen,Optimizer,hermes,AST,Driver,RA}`) now that both seams are real: **1209 byte-identical / 190 mismatch / 17 panic** (was 1203/190/23) — the +6/−6 move traces to exactly six files that hit the old promoter assert and are now byte-identical (each named and hermesc-confirmed), reconciled against a worktree build of the pre-S3 commit (`5aab87d1d`) that reproduces the documented S2-T8 baseline exactly before applying the move forward. All 17 remaining panics traced individually to S4 (16: the `export default`/`export *`/`import`/`xmod` module branches) or the pre-existing `computed-fn-name.js` C++-defect reproduction (1) — zero S3-attributable. The five Deferred `test/Sema` rows were re-probed; none newly unblocked (Imported/Deferred stays 49/5). Gate unchanged at 172/96; full workspace suite green. **Final-review follow-up:** the whole-branch reviewer found the `ForStatement`/`ForInStatement`/`ForOfStatement` `visitScope` arms (cpp:53-61) correct but uncovered by any corpus file (a `let` in a loop's HEAD scope blocking a candidate in the BODY block is the only observable shape, since a bare `FunctionDeclaration` can never be a loop body); `promotion-for-family-let-blocker.js` (one function per arm) closes that gap, hermesc-verified byte-identical, exit 0. Three more findings were comment-only corrections (a misdescribed derivation in `promotion-es5catch-cross-scope-reuse.js`, an incomplete unreachability argument in `resolver/mod.rs`'s `process_promoted_func_decls`, two citation-range nits) with no dump-visible behavior change. Gate: 172 → **173** corpus files (96 → **97** succeeding on hermesc).

**Gate (live, green):** `REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p sema --features dump-bin --test sema_differential -- --nocapture` → "sema differential (tests/sema_corpus): **173 corpus files matched (97 succeeded on hermesc)**" (160 at the end of S2; 172/96 at the end of S3 T1-T3, +1/+1 from the S3 final-review follow-up's `promotion-for-family-let-blocker.js`).

**S4-and-later carry-items (from the S1 + S2 + S3 reviews):**
- **S4a — standalone-front-end sema (module visit skeletons + flavors).** S4a and S4b are VERY DIFFERENT PHASES — S4a serves the standalone parser/front-end (the publication goal), S4b is Hermes-VM/compiler module lowering sequenced much later, near IRGen; the shared "S4" prefix is numbering convenience only (avoids renumbering S5). S4a scope: the four module visits' SKELETONS — the module-mode error paths + children traversal + the import-specifier declarations (`Decl::Kind::Import` via `extractDeclaredIdents` cpp:2334, the one reachable decl kind with no corpus exercise). Bug-for-bug subtlety: the guards are ASYMMETRIC — import's `requires module mode` error is unconditional (cpp:876), export's is `compile_`-gated (cpp:1511/1520/1550). `FunctionInfo::imports` backref fixup rides along (one push inside a visit being ported; dump-blind — IRGen is the reader — so unit-test pinned, the `mayReachImplicitReturn` precedent; reuse S1 T7's `hoisted_functions` pattern). **`resolve_ast_for_parser`** (SemResolve.cpp:295) is the centerpiece, not an afterthought: it is the entry `tools/hermes-parser-wasm.cpp` uses — the actual standalone-parser product — and it constructs the resolver with `compile = false`, making every ported `compile_` guard live for the first time (no ambient decls, no export module-mode errors, no rewrite #4, no constant folding). **Flavors:** the UNTYPED `-parse-flow` resolver paths (`typecast not allowed in this context` cpp:1576, `'this' parameter requires typed mode` cpp:1771 — flow syntax parsed WITHOUT `-typed`; pinned by corpus files written for the purpose), and the per-file-flag harness debt — `sema_differential.rs` passes no per-file flags, which currently keeps three behaviors out of the corpus (`-enable-eval=false` for `eval() is disabled at runtime`, `-parse-flow` for the two diagnostics above, `-lazy`) plus the `-fno-std-globals` ambient-decl path; a `// FLAGS:` convention would retire those notes. **NOT S4a: the 178 `test/Sema/flow/**` files** — every one of them runs `-typed` (verified 2026-07-29: `grep -L typed` over all 178, incl. `flow/ffi/`, matches zero files), i.e. they exercise the FlowChecker and the resolver's `typed_` branches; that corpus belongs to the separate future FlowChecker component (an earlier S4 bullet mis-scoped it here).
- **S4b — VM modules (a genuinely separate later phase, sequenced near IRGen; "S4" prefix is numbering convenience only):** the `$SHBuiltin.moduleFactory`/`export`/`import` protocol branches (calls.rs keeps their loud phase-tagged panics through S4a; `xmod-errors.js` stays the deferred MANIFEST row), `runCommonJSModule`/CJS wrapping (cpp:167), and **§3.4 rewrite #4** (anonymous `export default function` → `FunctionExpression`, cpp:1526-1544) — rewrite #4 belongs HERE, not S4a, on three verified grounds (2026-08-02): it is `compile_`-gated and `resolveASTForParser` passes `compile = false`, so the standalone path never performs it; its C++ comment says its purpose ("for cleaner IRGen"); and it is differential-INVISIBLE in plain mode (`export default function(){}` errors `requires module mode` and hermesc exits 2 WITHOUT printing the dump — verified) — it only becomes observable under `-commonjs` (dump shows `FunctionExpression` vs `FunctionDeclaration` under `ExportDefaultDeclaration`), so it cannot even be corpus-tested until CJS mode + the S4a `// FLAGS:` harness exist (`// FLAGS: -commonjs` files land with S4b). Honest cost of the split: "zero module panics" is NOT an S4a exit criterion — at most 7 of the 16 module-branch sweep panic files use the `$SHBuiltin` protocol (grep over the 8 sweep dirs, 2026-08-02) and keep their documented panics until S4b; the rest clear with S4a's visits.
- **S5 — lazy + `eval` entry points:** `resolve_ast_lazy`/`resolve_ast_in_scope`, and `visitProgram`'s unported `SaveAndRestore` of `globalScope_` (cpp:216-217), which only becomes observable once `Program` can recur; plus the THIRD `ScopedFunctionPromoter` call site, `runInScope` (`SemanticResolver.cpp:158`), the lazy/`eval` entry point that promotes BEFORE `processCollectedDeclarations` rather than after — it arrives with `resolve_ast_in_scope` (S3 T1 left a note at the `resolver/mod.rs` site; not ported there).
- **Regex validation** (`Invalid regular expression: …`) needs the **regex engine** (`lib/Regex/`) — its own future component, not a Sema phase.
- **Documented landmines carried by the corpus, not bugs to chase:** (i) same-location diagnostic ties are unfixable by construction — C++ sorts with an unstable `std::sort`, we sort stably (`invalid-args-eval.js`, one pair at `89:9`); (ii) **hermesc itself aborts** on `class C { x = class {}; }` (`SemContext.cpp:478`, witnessed upstream by `test/hermes/computed-fn-name.js:71`), on `$SHBuiltin.#x()` (`cast<IdentifierNode>` at cpp:1167), and on `using x = 1; { function f(){} }` (`ScopedFunctionPromoter.cpp:260`'s `identVar` assertion, found in S3 T1 — the promoter runs before `visit(VariableDeclarationNode *)` can reject the `using` declaration) — all three are pre-existing C++ defects, reproduced faithfully (the third as a `debug_assert!` at the same site) and kept out of the corpus; (iii) `Decl::Special::Eval` is a dead enumerator nothing in the C++ tree ever sets or reads.
- **Parser-phase follow-up — TWO TRACKED TASKS, both measured by S2 T8's sweep** (`sema-dump` vs `hermesc -dump-sema` over the 1416 `.js` files in `test/{Parser,IRGen,BCGen,Optimizer,hermes,AST,Driver,RA}`: 1203 byte-identical, 190 mismatched, 23 panicked on known S3/S4 deferrals — and **not one file hermesc compiles successfully disagrees**, i.e. every finding below is on a diagnostic path):
  - **(a) `errorExpected`'s same-line range is lost at `need`/`eat` call sites — 180 of the 190 mismatch files.** C++ `errorExpected` (`JSParserImpl.cpp:175-226`) folds the `what` location into the error's RANGE when it is on the same source line (`combineIntoRange(whatLoc, errorLoc)`), and emits a separate `note:` ONLY when it is not. Two distinct defects produce the 180: (i) many Rust `need`/`eat` call sites drop C++'s `what`/`whatLoc` arguments altogether, so no range is ever built — representative pair: `parser/src/js/functions.rs:411-415` vs `eat(r_paren, AllowRegExp, "at end of function parameter list", "start of parameter list", lparenLoc)` at `JSParserImpl.cpp:657-662` (111 files: caret geometry only); (ii) a handful of sites hand-roll `error_cur` + an UNCONDITIONAL `note_at` instead of calling the helper — `parser/src/js/classes.rs:1031-1039` (`';' expected after class property`, vs `JSParserImpl.cpp:5457-5462`), `classes.rs:289`, `classes.rs:108`, `modules.rs:905` — which also drop the quotes C++ puts around the token name (`identifier expected in decorator` vs `'identifier' expected in decorator`, `classes.rs:1102`/`:1261` for the two parser-only class messages) (69 files: geometry + an extra note, 4 of them also unquoted). This SUBSUMES the older "some notes are dropped (house style)" phrasing: the dropped different-line note is the small half; the missing same-line range is the large one. Fixing it is a call-site audit against the C++ signatures (the `C++ default args are spec` rule) with `parser_differential.rs` as the harness. Three further files differ in genuine message text (e.g. `unexpected token after yield expression` vs `';' expected`, `test/Parser/es6/yield-paren-error.js`) and four are one-off geometry cases, including the reverse shape (`test/Parser/escaped-this.js`: C++ a bare caret, ours a range).
  - **(b) The unported parser recursion limit is a CRASH, not silence.** The old wording ("hermesc errors on deep chains, ours is silent") is **disproved**: on deep input `sema-dump` overflows its stack and aborts (SIGABRT, exit 134) — `test/Parser/nested-expressions.js` and `test/hermes/far-environment-access.js` (250-odd nested arrows; hermesc diagnoses cleanly at `28:510`). Where both sides do diagnose, the trip points differ (`regress-nested-expressions-error.js`: hermesc col 3052, ours col 6124), because the two trackers increment at different rates per grammar production even though both caps agree on this platform (`JSParserImpl::MAX_RECURSION_DEPTH`, JSParserImpl.h:189-202, and `ESTree::kASTMaxRecursionDepth`, RecursiveVisitor.h:686-692, are both 1024 for a non-MSVC build without `HERMES_LIMIT_STACK_DEPTH`) — i.e. 1024 of our (larger) debug frames outrun the 8 MB stack while C++'s do not. So this is a robustness bug on valid input, not only a column mismatch; it should be prioritised above (a). `regress-nested-expressions-error.js` stays deferred on it.

**Next: S4a — standalone-front-end sema** (see the S4a bullet above for the full scope: module visit skeletons + `Decl::Kind::Import` + `FunctionInfo::imports` + `resolve_ast_for_parser` + the untyped `-parse-flow` paths + the `// FLAGS:` per-file-flag harness). S4b (VM modules: `$SHBuiltin` protocol + CJS wrapping + rewrite #4) is a genuinely separate, much later phase — near IRGen — despite the shared number; the 178-file `test/Sema/flow/**` corpus is FlowChecker-component scope. **No S4a plan exists yet — brainstorm it, then write it just-in-time** (`superpowers:brainstorming` → `superpowers:writing-plans`) and execute subagent-driven. Spec: `specs/2026-07-26-sema-untyped-design.md`; prior plans: `plans/2026-07-26-sema-s0-foundations.md`, `plans/2026-07-28-sema-s1-declarations-scopes.md`, `plans/2026-07-28-sema-s2-rest-of-walk.md`, `plans/2026-07-29-sema-s3-scoped-function-promoter.md`. |
| IR / IRGen | — | future |
| Optimizer | — | future |
| Inst / BCGen | — | future (BCGen couples to the VM — last) |

### Done: `support` crate (SourceErrorManager)

Modules under `rust/crates/support/src/`: `buffer` (copied `NullTerminatedBuf` + named
`SourceBuffer` with lazy line index), `location` (offset-based `SMLoc`/`SMRange`/`SourceId`/
`SourceCoords`), `line_index` (offset↔line/col), `diag` (`DiagKind`/`Subsystem`/`OutputOptions`/
`ResolvedDiagnostic`/`DiagHandler`/`CollectingHandler`/`Warning`), `render` (byte-compatible
`build_source_and_caret_line` + `render_diagnostic` + `StderrHandler`), `manager`
(`SourceErrorManager` façade). Tests: `tests/golden.rs` includes the live `hermesc`
differential. **Zero `unsafe`, zero warnings.** Spec: `specs/2026-06-01-source-error-manager-design.md`;
plan: `plans/2026-06-01-source-error-manager.md`.

### ✅ JS lexer — COMPLETE

The entire `JSLexer` (`include/hermes/Parser/JSLexer.h` + `lib/Parser/JSLexer.cpp`, ~3,700 LOC)
is ported to `rust/crates/{atom_table,unicode,parser}/` and **self-validates byte-for-byte
against the real `JSLexer`** via the `js-lexer-dump` oracle (`rust/crates/parser/tests/
differential.rs`, 5 grammar contexts: `div` 58 / `regexp` 5 / `type` 6 / `jsx` 4 / `jsx-child` 10).
Full public surface: all token lexing (punctuators, trivia, identifiers, keywords, numbers,
strings, templates, regexp, private identifiers), **JSX** (`advanceInJSXChild`, HTML entities)
and **Flow** (`Type` context), all literals + escapes (incl. WTF-8 / `convertSurrogates`), and
the stateful/parser-facing APIs (comment+token storage, magic comments, `SavePoint`, `lookahead1/2`,
`isCurrentTokenADirective`, `rescanRBraceInTemplateLiteral`, `isLet/isUsing/isAwaitUsing`, the
`Token`/`StoredComment` accessors). **178 workspace tests, zero warnings.** Validation is two-pronged:
the byte-for-byte differential (token streams) **and** all **39 `unittests/Parser/JSLexerTest.cpp`
cases ported** to `rust/crates/parser/tests/jslexer_ported.rs` (faithfully — error/warning counts,
message text via a `CollectingHandler`, recovery streams, concrete values; porting them surfaced **no
lexer bugs**, closing the earlier error-path / `prevTokenEndLoc` coverage gaps). Real `unsafe` only in
`atom_table` (the interner) and `parser/cursor.rs` (the scoped `*const u8` cursor, decision B).
Sole deviation: `getAllocator` has no Rust analog (no bump allocator). Design spec:
`specs/2026-06-01-js-lexer-design.md`; the per-subsystem/per-phase plans are under `plans/`
(`js-lexer-*` and `js-lexer-proper-*`). **A capstone review caught a stubbed `advance` fallthrough
(`0xc2`/`0xe2`/`0xef` non-special lead bytes errored instead of falling into the default arm) and
a few missing accessors — both fixed and tested before declaring complete.**

> **Next component: the Parser** (`lib/Parser/JSParserImpl*`), which consumes this lexer. The
> previously-tracked optional lexer follow-up — a `--non-strict` flag for `js-lexer-dump` — is now
> **DONE** (`differential_nonstrict`, 7 corpus entries, exercises the future-reserved-word downgrade
> + legacy octal / leading-zero / octal-escape paths). The lexer has no remaining open items.

### Historical: JS lexer build log

Port plan/progress as it was built (kept for traceability). What it needed from
`SourceErrorManager` was **done** first. Full design: `specs/2026-06-01-js-lexer-design.md`.
Per-subsystem implementation plans landed under `plans/` just-in-time as each was built.

**Locked decisions (this design pass):**
- **Scan cursor:** raw `*const u8` (option "B"), confined to the cursor module, offset
  at every boundary; `Rc<SourceBuffer>` backing, `NullTerminatedBuf` NUL makes lookahead
  in-bounds.
- **String interner:** copy juno `atom_table` **verbatim** (keep its encapsulated unsafe)
  and add a byte/WTF-8 intern path.
- **Number parsing:** **pure Rust, no FFI.** The lexer's decimal path uses `fast_float`
  (NOT `dtoa`), and Rust std's `str::parse::<f64>()` *is* that algorithm (correctly-rounded
  → bit-identical). Integer radix paths port `parseIntWithRadix*` directly.
- **Validation:** a small C++ token-dump harness (`tools/js-lexer-dump/`) linking the real
  `JSLexer` is the byte-for-byte oracle.

**Support-layer prerequisites** (separate ports, sequenced before the lexer proper — NOT
part of SourceErrorManager; in build order):

| # | Dep | Hermes source | Note |
|---|-----|---------------|------|
| 1 | Token tables | `Parser/TokenKinds.def`, `HTMLEntities.def` | ✅ **Done** — `rust/crates/parser/src/token_kinds.rs` (`TokenKind`, `token_kind_str`, `binop_precedence`, `is_res_word`/`is_punctuator`, `match_reserved_word`; 6 tests). `HTMLEntities.def` deferred to JSX. Plan: `plans/2026-06-01-js-lexer-token-tables.md`. |
| 2 | C++ token-dump harness | links `JSLexer` | ✅ **Done** — `tools/js-lexer-dump/` (`add_hermes_tool`, build `cmake --build cmake-build-asan --target js-lexer-dump`). Emits `<start> <end> <nl> <KIND>[ fields]`; KINDs are `.def` variant names; numbers as f64 bits; byte-exact `\xHH` quoting (WTF-8 round-trips); `--context=regexp\|div`. Plan: `plans/2026-06-01-js-lexer-dump-harness.md`. **Known oracle limits** (documented in-tool, revisit when porting those paths): `template_middle`/`template_tail` and IDENT_OP (`as_operator`) need parser-driven rescans so a plain `advance()` loop never emits them; JSX/Flow contexts not yet wired. |
| 3 | String interning (`StringTable`/`UniqueString`) | `Support/StringTable.h` | ✅ **Done** — `rust/crates/atom_table/` (juno `atom_table` verbatim minus HeapSize + `AtomBytes`/`atom_bytes` WTF-8 path; 4 tests, ill-formed-UTF-8 round-trips). Encapsulated `unsafe` confined here. Plan: `plans/2026-06-01-js-lexer-string-interner.md`. |
| 4 | Unicode char properties | `Platform/Unicode/CharacterProperties.{h,cpp}`, `UnicodeData.inc` | ✅ **Done** — `rust/crates/unicode/` (zero-unsafe). 8 ID/letter/space range tables generated from `UnicodeData.inc` (Unicode **17.0.0**) by committed `gen_tables.py`; ported `lookup` + predicates + constants/helpers; 8 tests (idempotent generation verified). RegExp canonicalization excluded. Plan: `plans/2026-06-01-js-lexer-unicode.md`. |
| 5 | Number parsing | `Support/Conversions.h`, `FastStrToDouble.cpp` (`fast_float`) | ✅ **Done** — `rust/crates/parser/src/number.rs` (zero-unsafe). Faithful `parseIntWithRadix*` port incl. the power-of-2 rounding path (validated vs a `u128`→f64 correctly-rounded oracle); `str_to_double` is pure-Rust `str::parse::<f64>()` (C++-confirmed bit patterns). Plan: `plans/2026-06-01-js-lexer-number-parsing.md`. |
|   | Bump `Allocator` | `Support/Allocator.h` | **droppable** — Rust owns the decoded strings. |

> **✅ MILESTONE (all 5 support-layer prerequisites complete).** Token tables, the C++
> token-dump differential oracle, the WTF-8 string interner, Unicode CharacterProperties,
> and number parsing are all done, reviewed (spec + code-quality), and committed on `rust`.
>
> **🚧 Lexer proper — phase 1a DONE.** The lexer skeleton is up in the `parser` crate:
> `utf8` (decode side of `Support/UTF8.h`), `cursor` (the encapsulated `*const u8`,
> decision B — the *only* `unsafe`, scoped + sound), `token` (`Token`/`RegExpLiteral`/
> `StoredComment`/`StoredToken`), and `lexer` (`JSLexer` + `advance` for
> **punctuators/whitespace/comments/EOF**). Validated **byte-for-byte live against
> `js-lexer-dump`** (`tests/differential.rs`, `--context=div`). Plan:
> `plans/2026-06-01-js-lexer-proper-1a.md`. Workspace: **~106 Rust tests passing, zero
> warnings**.
>
> **🚧 Lexer phase 1b-i DONE.** Identifiers (ASCII fast path + Unicode + `\u`/`\u{}` escapes),
> reserved words (pre-interned via `atom_table` + strict-mode future-reserved-word filter), the
> UTF-8 **encode** side + `appendUnicodeToStorage` WTF-8 surrogate split, and the `ident=` dump
> field. The differential corpus now covers identifiers/reswords/Unicode/escapes (31 entries).
> Plan: `plans/2026-06-02-js-lexer-proper-1b-i.md`. **NOTE:** review caught that the differential
> test was *silently skipping* (oracle binary resolved relative to the crate dir, not the repo
> root) — fixed to resolve via `CARGO_MANIFEST_DIR`, assert every entry when the binary is
> present, and honor `REQUIRE_DIFFERENTIAL=1` to force a hard failure if absent. It now genuinely
> runs for phases 1a+1b-i.
>
> **🚧 Lexer phase 1b-ii DONE.** Numbers — `scanNumber` ported branch-for-branch (decimal/hex/
> octal/binary/legacy-octal/fractions/exponents/separators/BigInt), wiring `parser::number`
> (`str_to_double` == fast_float; `parse_int_with_radix` incl. the >2^53 rounding path). `bits=`/
> bigint dump fields; numeric differential at 40 entries. Plan: `plans/2026-06-02-js-lexer-proper-1b-ii.md`.
> **The lexer now lexes punctuators, trivia, identifiers, keywords, and all numeric literals,
> self-validating byte-for-byte vs `js-lexer-dump`.**
>
> **🚧 Lexer phase 2a DONE.** String literals (`scanString` non-JSX — `\b\f\n\r\t\v`, octal/`\x`/
> `\u`/`\u{}` escapes, line continuations, `containsEscapes`) + private identifiers
> (`scanPrivateIdentifier`). `string_literal escapes=/value=` dump field; differential at 48 entries
> (verified vs 14 independent oracle checks incl. NUL/octal/WTF-8). Plan: `plans/2026-06-02-js-lexer-proper-2a.md`.
> Deferred: `convertSurrogates` re-encoding (needs UTF-16 conversion utils — tracked).
>
> **🚧 Lexer phase 2b DONE.** Template literals (`scanTemplateLiteral` — `no_substitution_template`/
> `template_head`, TV/TRV dual buffers, `NotEscapeSequence`→null cooked, CR→LF). `cooked=`/`raw=` dump;
> differential at 55 entries. Plan: `plans/2026-06-02-js-lexer-proper-2b.md`. **`lexer.rs` was split**
> into `lexer/{mod,escape,identifier,number,string,template,dump}.rs` (pure-move refactor, differential
> unchanged).
>
> **🚧 Lexer phase 2c DONE — all standard-JS token lexing complete.** Regexp literals (`scanRegExp` +
> the `AllowRegExp` `/` arm). The differential harness is now parameterized by `GrammarContext`
> (`--context=div` 55 entries + `--context=regexp` 5). Plan: `plans/2026-06-02-js-lexer-proper-2c.md`.
> **The Rust lexer now lexes every standard ECMAScript token** — punctuators, trivia, identifiers,
> keywords, numbers, strings, templates, regexp, private identifiers — self-validating byte-for-byte
> vs `js-lexer-dump`.
>
> **🚧 Lexer phase 3a DONE.** Flow `Type` grammar context — `{|`→`l_bracepipe`, `|}`→`piper_brace`,
> `%checks`, `@`-Flow-identifiers, Type-context `<`/`>`/`?` (no `??`). Harness `--context=type`;
> `--context=type` differential (6 entries). Crate is now `int_plus_one`-clean. Plan:
> `plans/2026-06-02-js-lexer-proper-3a.md`.
>
> **🚧 Lexer phase 3b DONE — JSX complete.** `HTMLEntities.def` table (253 entries, generated +
> binary-search lookup) + `consumeHTMLEntityOptional`, the JSX `scanString<true>` branches, `advanceInJSXChild`,
> JSX identifier mode (`-`). Harness `--context=jsx` + `--jsx-child`; `jsx_text value=/raw=` dump. Five
> differentials now run: div 55 / regexp 5 / type 6 / jsx 4 / jsx-child 10. Plan:
> `plans/2026-06-02-js-lexer-proper-3b.md`. **The lexer now lexes the full JSLexer surface bar the
> stateful/parser-facing APIs.**
>
> **🚧 Lexer phase 4a DONE.** Self-contained lexer state: comment + token storage, magic comments
> (`//# sourceURL=`/`sourceMappingURL=`), `SavePoint` (value struct + `restore`), `seek`/`force_eof`,
> `isCurrentTokenADirective` (non-corrupting), `rescanRBraceInTemplateLiteral` (→ `template_middle`/
> `template_tail`). Unit-tested; the 5 differentials still pass. Plan: `plans/2026-06-02-js-lexer-proper-4a.md`.
>
> **🚧 Lexer phase 4b DONE.** Parser-lookahead: `optimisticSkipWhitespace`, `lookahead1`/`lookahead2`
> (save/advance/restore + message suppression; the `template<bool>`→`const`-generic and `Keywords`→atom-param
> adaptations), `isLetFollowedByDeclStart`, `isUsing/AwaitUsingFollowedByIdentifier`. Unit-tested
> (incl. a comment-rollback regression found in review). Plan: `plans/2026-06-02-js-lexer-proper-4b.md`.
> **(Correction, post-lexer:** these and the `IdentifierMode`/`scanString<JSX>` scanners were initially
> flattened to runtime params; they are now restored to Rust generics — `const` generics for `bool`,
> the `IdMode` marker trait + `JsMode`/`JsxMode`/`FlowMode` ZSTs for `IdentifierMode` — to preserve the
> C++ template monomorphization. Faithful-port rule: keep C++ templates as generics, never flatten.**
>
> **🚧 Lexer phase 4c DONE — `convertSurrogates`, the LAST `JSLexer` feature.** `getStringLiteral` now
> branches on the flag: when set it re-encodes the WTF-8 internal form to valid UTF-8 via
> `convertSurrogatesInString` (ported `encodeUTF16` + `convertUTF8WithSurrogatesToUTF16` +
> `convertToCodePointAt` + `convertUTF16ToUTF8WithReplacements` into `parser::utf8`). All string/template/
> regexp/jsx-text/bigint value interning routes through `get_string_literal` (matching the C++), so with
> the flag OFF behavior is byte-identical (the 5 differentials still pass unchanged). Plan:
> `plans/2026-06-02-js-lexer-proper-4c.md`.
>
> **✅ JS LEXER COMPLETE.** The full `JSLexer` public surface is ported and validated: token lexing,
> trivia, identifiers, keywords, all numeric/string/template/regexp/bigint literals, Flow `Type` context,
> JSX, storage, magic comments, `SavePoint`/`seek`/`force_eof`, lookahead, directives,
> `rescanRBraceInTemplateLiteral`, and `convertSurrogates`. **No remaining tracked items** — the
> optional `--non-strict` harness flag is now done (`differential_nonstrict`).

### ✅ JSONParser — COMPLETE

The entire `JSONParser` component (`include/hermes/Parser/JSONParser.h` + `lib/Parser/JSONParser.cpp`,
plus `Support/JSONEmitter.{h,cpp}` and `numberToString` from `Conversions.cpp`) is ported — the **first
consumer of the completed `JSLexer`**. Spec: `specs/2026-06-02-json-parser-design.md`; plan:
`plans/2026-06-02-json-parser.md`.

Code map:
- **`rust/crates/support/src/json_emitter.rs`** — `JSONEmitter` (full surface: state stack, dict/array,
  `emit_key`/`emit_key_u16`, all value overloads, escaping, pretty, JSONL) + `number_to_string`
  (ECMAScript `Number::toString`, shortest-decimal via Rust `{:e}`). Zero `unsafe` (support `forbid`s it).
- **`rust/crates/parser/src/json/`** — `mod.rs` (value model `JSONValue`/`JSONHiddenClass`, RTTI→enum,
  `ArrayView`/`ObjectView` accessors, `emit_into`, `JSONSharedValue`), `factory.rs` (`JSONFactory`:
  string/number uniquing + shared hidden classes via `bumpalo` arena), `parser.rs` (recursive descent
  over `JSLexer`).

Representation: `&'a JSONValue<'a>` == the C++ `JSONValue*`, nodes in a `bumpalo` arena; uniquing via
`HashMap` (not `FoldingSet`); hidden classes shared by sorted-key set. New dep: `bumpalo` (first
third-party crate). The **sole hand-written `unsafe`** in the component is the `JSONSharedValue::get`
deref (`Rc<Bump>` + lifetime-erased pointer, the `shared_ptr<Allocator>`+`JSONValue*` analog).

Validation (two-pronged, like the lexer): the **byte-for-byte differential** — a C++ `json-parse-dump`
oracle (`tools/json-parse-dump/`) vs the Rust `json-parse-dump` bin, over a 16-file corpus
(`tests/json_differential.rs`, incl. astral/lone-surrogate/hidden-class-shape/number-edge cases + 6
error cases; force-runs under `REQUIRE_DIFFERENTIAL=1`) — **and** the ported unittests (all **5
`JSONParserTest`** cases in `tests/json_parser_ported.rs` + all **13 `JSONEmitterTest`** cases inline).
The differential caught a real bug the per-phase reviews missed: `emit_into` panicked on WTF-8 strings
(non-BMP / lone surrogates) via `from_utf8().expect()` — fixed to route keys/values through
`convert_utf8_with_surrogates_to_utf16` + `emit_u16`/`emit_key_u16`, matching C++ `primitiveEmitString`.
A capstone review confirmed the full public surface, branch-for-branch control/error fidelity, and the
sole-sound `unsafe`. Sole deviations (per the design doc): fat-enum node layout (uniform 32-byte nodes +
separate value slices vs C++ inline `Pack`), `getAllocator`/`getStringTable` → `arena()`/`atoms()`.

Benchmark (first datapoint, see "Benchmark" section below): on an 11.6 MB JSON file × 50, **Rust
`--release` ≈ 69.5 MB/s vs C++ Release ≈ 70.6 MB/s — within ~1.5%**. (The default ASan+`-O1` C++ build
is ~12× slower and is not a fair speed baseline.)

> **Next component: the Parser** (`lib/Parser/JSParserImpl*`) — needs the AST + `Context`, now under way.

### ✅ AST — COMPLETE (storage/GC spine + generated node set + transforming visitor + JSON dumper)

The `ast` crate (`rust/crates/ast/`) has the storage spine **and the full generated node set**. Design spec:
`specs/2026-06-03-ast-design.md`; per-phase plans: `plans/2026-06-04-ast-1-storage-and-spine.md` (phase 1),
`plans/2026-06-04-ast-2-node-codegen.md` (phase 2).

**Model (locked):** copy **juno's GC arena** (`Context`/`GCLock`/`NodeRc` + mark-sweep), `#[repr(C)]`
enum `Node<'gc>` for deep `match`. **Child fields are immutable** (`&'gc Node`, `Option`, `NodeList`) and
**rebuilt on change** via a functional recursive walk; **all other attributes are `Cell<…>`** (mutated in
place). The split falls out of `ESTree.def`'s type tags. Verified there are **no `Cell<&Node>` cross-edges**
in `ESTree.h` (only two decoration `NodeList`s), so the GC marker traces decoration lists explicitly and the
invariance landmine juno hit is avoided. Rationale recorded in the spec, incl. why **references, not index
handles** (in a never-freed arena both are equally UB-free; references are *more* logically robust and read
close to the C++ `node->field`).

**Phase 1 delivered:** the GC arena copied + adapted to our crates (`support::location`, our `atom_table`,
`core::mem::offset_of!`, no `source_mgr` in `Context`; the marker rewrite was reviewed **sound** — cleaner than
juno's `GCLock` re-entrancy); a minimal hand-written 4-kind node model (`NumericLiteral`/`Identifier`/
`BinaryExpression`/`Program`) exercising deep `match`, `Cell` in-place mutation, immutable children, a decoration
`NodeList`, functional rebuild, GC orphan reclamation, and decoration-list tracing. **`unsafe` is confined to
`context.rs`** (the crate `deny`s it). `Deque`/`HeapSize` were moved into **`support`** (shared utilities; the
deque test rewritten `unsafe`-free to keep `support`'s `forbid`). The decoration-tracing test was **mutation-verified**
(disabling the marker's decoration walk makes it fail). Two-stage reviewed per task; whole workspace green, zero warnings.

**Phase 2 delivered:** a committed Python generator (`rust/crates/ast/gen_nodes.py`) parses
`include/hermes/AST/ESTree.def` (all FLOW/JSX/TS/Cover families ON) plus a hand-transcribed decoration table
(from `ESTree.h`'s `Decoration` classes + `DecoratorTrait` map) and emits the committed, `// @generated`
`src/node.rs` — the full **271-node** set (282 `ESTREE_NODE_*_ARGS` tokens − 11 `#undef` lines), replacing the
hand-written 4-kind model. Per node: the `#[repr(C)]` struct (metadata-first; child fields `&'gc`/`Option`/`NodeList`,
value + decoration fields `Cell<…>`), the `Node<'gc>` enum arm, the `NodeKind` entry, a minimal `new` constructor
(defaults decorations), and the `visit_children`/`mark_lists` arms. `NodeKind` mirrors the C++ enum exactly
(`#[repr(u32)]`, `.def` order, interleaved `_NAME_First`/`_NAME_Last` sentinels) so base-range `isa` is the same
`_First < kind < _Last` check; generated `is_<range>()` predicates (12 ranges) + `as_<leaf>()` accessors give the
`dyn_cast` surface. **Fields are idiomatic snake_case** (acronym-aware: `is_jsx`/`as_sh_builtin`), Rust-keyword fields
`r#`-escaped (`r#await`/`r#async`); the original camelCase `.def` name is retained by the generator to bake as the
literal JSON key when the phase-4 dumper is generated (so JSON stays byte-identical with no `non_snake_case` allow).
The two decoration `NodeList`s (`decorations` on all 6 FunctionLike nodes, `dummy_param_list` on Program) are
`Cell<NodeList>` and traced in **both** `visit_children` and `mark_lists`. **Validation (no differential until parser
time):** anti-drift count guard (`EXPECTED_NODES = 271`) + every-leaf-resolves-a-decoration / every-range-has-a-decoration
asserts; a **byte-for-byte regenerate-and-diff idempotency test** (`tests/generated_idempotent.rs`, `REQUIRE_GEN=1`);
structural tests (`tests/node_model.rs` — range predicates, deep match, `visit_children` counts); and a
**mutation-verified base-range decoration-tracing test** (`gc_traces_decorations_on_function_declaration` — proves the
marker traces `decorations` where it attaches via the FunctionLike base range, not just on Program; disabling the
generator's `declist` emission makes it fail). Two-stage reviewed; whole workspace green (229 tests), zero warnings.
**Deliberate scope notes:** `node.rs` fully generated; snake_case fields with camelCase names retained for the dumper;
the `new` constructors are plain field-init (NOT the phase-3 Builder); `visit_children_mut`/functional-rebuild is
phase 3 (it needs the Builder to allocate); `ESTREE_IGNORE_IF_EMPTY` is parsed but emitted in phase 4.

**Phase 3 delivered:** the transforming-visitor surface, ported from juno (`unsupported/juno/crates/juno_ast/`) and adapted to
our immutable-children-+-`Cell`-attributes model. Hand-written (`visitor.rs`/`node_child.rs`): `TransformResult{Unchanged, Removed,
Changed(T), Expanded(Vec<T>)}`, `Path{parent, field}`, the `VisitorMut` trait, the `NodeChild` field trait (`visit_child_mut` +
`duplicate`) impl'd for `&Node`/`Option<&Node>`/`NodeList`, `NodeMetadata::duplicate`, and `Node::visit_mut`. Generated (extending
`gen_nodes.py`): the `NodeField` enum (106 structural-child field names), a `pub mod builder` with a `Builder<'gc>` enum + per-node
builder (`from_node` copies **children by ref via `.duplicate()` and `Cell` attributes by value into fresh `Cell`s**; setters ONLY
for structural children; `build`/`build_forced`), and `Node::visit_children_mut` (rebuilds a node **only when a child changed** —
required-child `Removed`→zero-width `EmptyStatement`, optional→`None`, list remove/expand/splice). The two decoration `Cell<NodeList>`s
(`decorations`/`dummy_param_list`) are copied-by-value and mutated in place — never threaded through the transform walk nor given a
setter (consistent with the `Cell`-attribute model). The **read `Visitor` is unchanged** (phase 1; still used by the GC marker);
parent/`Path`-aware read traversal is deferred to when a consumer (Sema) needs it. Validation: 7 `tests/transform.rs` cases
(change-rebuilds-and-shares, unchanged-is-pointer-identical, list remove, list expand, required-removed→`EmptyStatement`, GC orphan
reclamation, builder clone-with-one-field-changed) + the idempotency guard; whole-component capstone review found **zero issues**.
The generator's shared field-classification makes the threading↔setter correspondence drift-proof.

**Phase 4 delivered:** `ESTreeJSONDumper` port. The generator (`gen_nodes.py`) now emits, into the `@generated` `node.rs`,
`Node::node_type_str` (the JSON `"type"` == the variant name) and `Node::dump_children` — walking ONLY the `.def`-arg fields in
declaration order (no decorations), baking the retained **camelCase `.def` names** as literal JSON keys and a per-field
`ESTREE_IGNORE_IF_EMPTY` flag (validated against real nodes/fields). `src/dump.rs` is the hand-written driver:
`ESTreeDumpMode{Compact,HideEmpty,DumpAll}`/`LocationDumpMode`/`ESTreeRawProp`, the `field_*`/`dump_*` helpers, `visit`,
`print_source_location`, `dump_raw`, and the public `dump_estree_json` (no-sm) + `dump_estree_json_with_sm` entry points. Labels/strings
resolve via the atom table and emit WTF-8→UTF-16 (`support::utf8::convert_utf8_with_surrogates_to_utf16` + `emit_u16`), byte-matching
C++ `primitiveEmitString`. **Validation:** 9 `tests/dump_golden.rs` cases (3 modes, IGNORE_IF_EMPTY distinction, nested+lists, pretty,
WTF-8/astral, loc/range/raw with a `SourceErrorManager`, raw-Exclude + no-sm omission, out-of-range skip) + the idempotency guard;
whole-component capstone review **APPROVED**. **Deliberate deviations (2, model-driven):** (a) `"raw"` requires the buffer (offset
model has no location pointer), so it is omitted in the no-sm overload; (b) the C++ `StackOverflowGuard` → a plain 128-depth counter.
**Tracked follow-up (not a blocker):** the C++ third overload (caller-owned `JSONEmitter` + a public `includeSourceLocs`/`NodeKindSet`
setter) is not exposed — `include_source_locs` is plumbed + tested internally; add the thin wrapper when a consumer (LSP/debugger) needs it.
The byte-for-byte `-dump-ast` differential vs `hermesc` lands as the **Parser's** gate (the AST has no producer until the Parser).
Plan: `plans/2026-06-05-ast-4-json-dumper.md`.

> **Next component: the JS Parser** (`lib/Parser/JSParserImpl*`) — consumes the lexer + AST + `Context`; the `-dump-ast` differential
> vs `hermesc` is its validation gate.

### ✅ JS Parser — COMPLETE

The largest component (~16,900 lines of C++: core `JSParserImpl.cpp` 7,603 + `-flow` 5,438 + `-ts` 1,437 + `-jsx` 505 +
headers). **No Rust parser to crib from** — juno's `hparser` is an FFI-to-C++ + AST converter, not a parser; we port the C++
directly. Scope (locked in the design spec): all three passes (Full/Pre/Lazy) + all dialects (JSX/Flow/TS) + full public API.
Design spec: `specs/2026-06-06-js-parser-design.md`. Built core-first, sliced into phases P0→P8 + capstone; each phase extends a
byte-for-byte `-dump-ast` differential corpus.

**Validation gate:** `hermesc -dump-ast` IS the oracle (verified pre-Sema in `CompilerDriver.cpp:867`, so it dumps the raw parse
AST) — no dedicated C++ tool needed. A Rust `ast-dump` bin is diffed byte-for-byte against `hermesc -dump-ast -dump-source-location=both`
(both pretty by default). This gate is also the deferred end-to-end exercise of the AST's `ESTreeJSONDumper`.

> **🚧 P0 — Foundations + gate DONE.** (1) Added a `debug_loc` to AST `NodeMetadata` (set by the parser; dumper ignores it, golden
> output unchanged). (2) `parser` crate now depends on `ast`. (3) `JSParserImpl<'gc,'ast,'ctx,'a>` scaffold
> (`rust/crates/parser/src/js/mod.rs`): `Param` flags, `new` (advances to first token), `check`/`advance`/`eat`/`need`/error helpers,
> recursion guard, `set_location`. The lexer + AST share one `AtomTable` via the `GCLock`; `Option<&'gc Node>` mirrors C++ `Optional`.
> (4) Minimal `parseProgram` (trivia-only → empty `Program` covering `[start..EOF]`; non-EOF errors). (5) `ast-dump` bin
> (`src/bin/ast_dump.rs`). (6) Live `parser_differential` (`tests/parser_differential.rs`, `REQUIRE_DIFFERENTIAL=1`) — **4 trivia-only
> corpus files match byte-for-byte**. Each task two-stage reviewed (spec + quality); zero warnings. Plan:
> `plans/2026-06-06-js-parser-p0-foundations.md`.
>
> **🚧 P1 — core expressions DONE.** The full JS *value*-expression grammar, wrapped in expression statements, dumps byte-for-byte
> vs `hermesc -dump-ast` over a **27-file corpus**. Sub-tasks (each two-stage reviewed + a whole-phase capstone, all green, zero
> warnings): P1.1 expr-stmt spine + operator-chain skeleton + primary literals; P1.2 binary precedence parser; P1.3 unary/update;
> P1.4 conditional; P1.5 assignment ops + right-assoc chain; P1.6 member/call/new/optional-chaining/new.target/private-member;
> P1.7 array literals + spread/elision; P1.8 object literals (data) + P1.8b destructuring-assignment reparse (fresh-node, immutable
> model); P1.9 templates (tagged+untagged); P1.10 regexp. Plan: `plans/2026-06-06-js-parser-p1-expressions.md`.
> **Deferred to later phases (all HONEST errors with tests, no silent fallthrough):** functions/classes/arrow/async/generator/getters/
> setters/object-methods/`super`/`yield` → P3; `import()`/`import.meta` → P4; statements (block/var/if/loops/switch/try/…)/labelled/
> declarations/import-export → P2; Flow/TS branches → P6/P7 (context-gated). **Tracked carry-forwards (none blocking):** error-recovery
> fidelity (a few spots `return None` where C++ continues after a non-fatal error — unobservable in `-dump-ast`; tied to the
> `error`-limit/`force_eof` TODO); `parse_statement_list`'s single `until` token grows to 2–3 for switch-case in P2; `in_decl` threading
> into reparse helpers in P2; the `let`-in-sloppy-mode over-eager error needs `isLetFollowedByDeclStart` lookahead in P2.
>
> **🚧 P2 — statements & declarations DONE.** The full statement + declaration grammar dumps byte-for-byte vs `hermesc -dump-ast`
> over a **47-file corpus**. Sub-tasks (each two-stage reviewed + a whole-component capstone, all green, zero warnings):
> P2.1 simple statements (throw/return/break/continue/with/debugger) + labelled + `eatSemi(optional)`; P2.2 binding identifiers &
> destructuring patterns (`validateBindingIdentifier`/`parseBinding{Identifier,Pattern,Element,RestElement,Initializer,Property,
> RestProperty}`); P2.3 var/let/const/using declarations + `checkDeclaration`/`parseDeclaration`/`parseStatementListItem` dispatch;
> P2.4 block/if/while/do-while/switch/try + the `parseStatementList` multi-`until` const-generic; P2.5 for / for-in / for-of (incl.
> `using` heads + destructuring-pattern reparse). Plan: `plans/2026-06-06-js-parser-p2-statements.md`. **Bugs caught by review (fixed):**
> (a) the parser ctor didn't forward `Context::isStrictMode()` to the lexer (C++ `JSParserImpl` ctor passes it) — sloppy-mode default
> was wrong, breaking loose-`let`-as-identifier; (b) P2.5 dropped the `[In]` flag in the for-in right / C-style test+update (C++ bare
> `parseExpression()` defaults to `ParamIn`), rejecting `in` as an operator there. **Deferred (all HONEST errors with tests):**
> function/class declarations → P3; `import`/`export` declarations → P4 (the `import(`/`import.meta` lookahead routes to expression);
> Flow/TS declaration + type-annotation branches → P6/P7 (context-gated off). **Carry-forwards RESOLVED in P2:** `parseStatementList`
> single-`until` → multi-`until` const-generic; the `let`-in-sloppy approximation → real `lexer.is_let_followed_by_decl_start()`.
> **Remaining carry-forwards (none blocking):** error-message-note fidelity (the simplified `need`/`eat`/`errorExpected` drop the
> secondary "location of …" `sm_.note`, and some P1 `eat`/`need` `where_` strings lack a leading space — unobservable in `-dump-ast`,
> tied to the `error`-limit/`force_eof` TODO); `in_decl` threading into reparse helpers (still `false` at the for-in/of reparse site).
>
> **🚧 P3 — functions, classes, arrows, async/generators, methods, `super`, `yield`, decorators DONE.** The full function/class grammar
> dumps byte-for-byte vs `hermesc -dump-ast` over a **62-file corpus**. Sub-tasks (each two-stage reviewed + a whole-component capstone,
> all green, zero warnings): P3.1 function decls/exprs + formal params + body (generators & async via `parseFunctionHelper`); P3.2 `yield`
> (`await` was P1.3, now reachable); P3.3 arrow functions + the cover-paren reparse (`reparseArrowParameters`); P3.4 object methods/getters/
> setters/async-&-generator methods; P3.5 `super` member & call; P3.6 classes (methods/fields/static-blocks/heritage/private members) +
> decorators. Plan: `plans/2026-06-07-js-parser-p3-functions-classes.md`. **RAII conversions:** C++ `SaveAndRestore<bool>` on
> `paramYield_`/`paramAwait_` → an `Rc<Cell<bool>>` + `ParamFlagGuard` Drop-guard (restores on every `?` path, mirrors `RecursionGuard`);
> C++ `SaveFunctionState`'s strict-mode restore → explicit save/restore wrappers (classes force strict; functions/arrows/object-methods
> save+restore). **Bugs caught by review (fixed):** (a) P3.1 had three `lookahead1::<false>` that should be `<true>` (C++ default
> `RequireNoNewLine=true`) — async-arrow/async-function/import-call detection mis-fired across a newline; (b) the capstone caught a
> `"use strict"`-leak: nested function/arrow/object-method bodies didn't restore the lexer strict-mode flag (C++ `SaveFunctionState` dtor
> does), leaking strictness to enclosing sloppy code. Both have regression corpus files. **Full-pass only:** the C++ `pass_`/PreParse/Lazy
> blocks and the `SaveFunctionState` arrow-bookkeeping are omitted (inert in eager parse); the `_param_yield`/`_param_await` args threaded
> into `parse_function_body` are dormant (a future lazy-parse item). **Deferred (HONEST errors):** `import`/`export`/`import()`/`import.meta`
> → P4; Flow/TS (type params, `implements`, annotations, TS modifiers, variance) → P6/P7 (context-gated off). **No `phase P3` stubs remain.**
>
> **🚧 P4 — modules: `import`/`export` declarations + `import()` / `import.meta` DONE.** The full standard-JS module grammar dumps
> byte-for-byte vs `hermesc -dump-ast` over a **75-file corpus**. Sub-tasks (each two-stage reviewed + a whole-component capstone, all
> green, zero warnings, zero new clippy lints): P4.1 `import(...)` (`ImportExpression`) + `import.meta` (`MetaProperty`) expression forms in
> `parse_optional_expression_except_new` (expressions.rs); P4.2 `import` declarations — default/namespace/named specifiers, `from` clause,
> `with` import-attributes clause, bare specifier (`parseImportDeclaration`/`parseImportClause`/`parseNameSpaceImport`/`parseNamedImports`/
> `parseImportSpecifier`/`parseFromClause`/`parseWithClause`) in the **new `rust/crates/parser/src/js/modules.rs`**; P4.3 `export`
> declarations — `export {…}`, `export {…} from`, `export * from`, `export * as ns from`, `export default …`, `export var/let/const/
> function/class …` (`parseExportDeclaration`/`parseExportClause`/`parseExportSpecifier`). Top-level dispatch wired in `statements.rs` with the
> C++ push asymmetry (import ALWAYS pushes then errors if disallowed; export pushes ONLY when allowed). Plan: `plans/2026-06-08-js-parser-p4-modules.md`.
> **Bugs caught by review (fixed):** (a) `import.meta`'s `meta` keyword was checked with the escape-SENSITIVE `check_unescaped_name`, but C++
> `check(metaIdent_)` (the `check(UniqueString*)` overload) is escape-INsensitive — swapped to `check_name` (so `import.meta` parses),
> with a regression test; the same correction applied to all `from`/`as`/`meta` contextual checks (the one genuine `checkUnescaped` is
> `export default async function`). (b) the `with`-clause comma used `AllowDiv`; the C++ `checkAndEat` default is `AllowRegExp` (unobservable
> on valid input, fixed for fidelity). **Documented deviation:** `parse_import_clause` returns `Option<Vec<&Node>>` instead of C++'s
> `Optional<UniqueString* kind>` + by-ref `specifiers`, since `kind` is monomorphic `value` until Flow/TS (reintroduce with `import type`).
> **Deferred (HONEST omissions with `// P5/P6/P7` markers):** the `import type`/`import typeof` kind, per-specifier Flow `type`/`typeof`,
> `export type`, Flow component/hook/enum/record default exports, the Flow type export-kind detection → P5/P6/P7 (context-gated off).
> **No `phase P4` stubs remain — the parser now handles the entire standard-ECMAScript grammar (no Flow/TS/JSX).**
>
> **🚧 P5 — the Flow TYPE GRAMMAR + declarative integration DONE** (2026-06-09). The entire Flow type-annotation grammar from
> `lib/Parser/JSParserImpl-flow.cpp` (~the type-grammar half of its 5,438 lines) dumps byte-for-byte vs `hermesc -dump-ast -parse-flow`
> over a **24-file Flow corpus** (plus the 76-file plain corpus, unchanged — Flow does NOT leak into plain JS). Sub-tasks (each two-stage
> reviewed + a whole-phase capstone, all green, zero warnings, zero new clippy): P5.0 foundations — `Context::parse_flow` flag,
> `--parse-flow` on `ast-dump` + a second differential corpus dir, the `allow_anon_function_type`/`allow_conditional_type` flags as
> `ParamFlagGuard` Drop-guards, minimal `type X = <primitive>` gate; P5.1 the full annotation hierarchy (conditional/union/intersection/
> anon-fn/prefix/postfix/primary, keyof/`infer`-with-SavePoint-backtrack/typeof/tuple/literal/generic, `parse_type_args_flow`,
> `parse_generic_type_flow`, reparse helpers); P5.2 function types (incl. the `(T)`-group-vs-`(params)=>R` cover), object types (incl.
> the speculative `proto`/`static`/`get`/`set` modifier-to-name reparse, `[[slots]]`, indexer-vs-mapped), type-param declarations
> (`const`, deferred `in`/`out` variance-vs-name), predicates (`asserts`/`implies`/`is`/`%checks`), return types; P5.3 `opaque type`
> (super/extends/legacy bounds) + `interface` declarations + interface-as-type + `parse_class_implements_flow`, after a pure-move split
> of `flow.rs` into **`js/flow/{mod,declarations,types,function_types,object_types,params}.rs`**; P5.4 non-ambiguous integration —
> function/method/class type-params, return types + predicates, leading `this` param, binding/array/object-pattern annotations, class
> heritage (`extends B<T>` super-type-args, `implements`), member variance, field types, object-literal method types. Plan:
> `plans/2026-06-09-js-parser-p5-flow-types.md`. **Bugs caught by review (fixed):** (a) spec review — `parse_type_args_flow`'s trailing
> grammar context must be `Type` (the C++ default, JSParserImpl.h:1506), not `AllowRegExp`: in `Type` context the lexer SPLITS `>>`, so
> nested generics `Foo<Bar<Baz<U>>>` failed (the corpus' max 1-level nesting hid it); (b) **capstone** — a SILENT `exportKind`
> divergence: P5 made `export type A = …`/`export opaque type …`/`export interface …` reachable through two P4-era stubs, parsing with
> `exportKind:"value"` where hermesc emits `"type"`; fixed by porting the C++ export-kind `isa` detection (JSParserImpl.cpp:7361-7368)
> + the alias branch of `parseExportTypeDeclarationFlow` (flow.cpp:2557-2566), with `export type {…}`/`export type * …` clauses now
> honest P6 errors. **Port-wide lesson: C++ DEFAULT ARGUMENTS are spec** — `checkAndEat`'s default is `AllowRegExp`,
> `parseTypeArgsFlow`'s is `Type`; always read the header defaults, never assume. **Deferred (honest errors / `// P6` markers):** the
> ambiguous-expression Flow (typed arrows, `as`/`as const`, `(x:T)` casts + cover-typed-identifier, call/`?.`/`new` type-args — all the
> SavePoint+suppression sites), `declare` statements, `enum`, `component`/`hook` (+ hook types + component-syntax before-colon/renders
> paths), `match`, `record`, `import type`/`typeof` kinds, `export type` clause forms → P6; TS → P7.
>
> **🚧 P6 — the REST OF FLOW DONE** (2026-06-13). The remaining ~2,500 lines of `JSParserImpl-flow.cpp` + the ambiguous-expr call sites
> in `JSParserImpl.cpp`, byte-for-byte vs `hermesc -dump-ast` (+ the matching hidden flag) over a 42-file Flow + 8 component + 5 records
> + 7 match corpus (plus the 76-file plain corpus, unchanged). Plan: `plans/2026-06-13-js-parser-p6-flow-extensions.md`. Sub-tasks
> (each one implementer + spec review + structural-fidelity + quality review + capstone): **P6.0** ambiguous-expr foundations — four new
> `Context` flags (`parse_flow_ambiguous` set by `-parse-flow`'s ALL default; `parse_flow_component_syntax`/`_records`/`_match` =
> `-Xparse-component-syntax`/`-Xparse-flow-records`/`-Xparse-flow-match`), the two message-suppression mechanisms (pure-suppress vs
> defer-then-commit, first used here), SavePoint speculation, type-args on call/`new`/`?.`, and `as`/`as const`; **P6.1** typed arrows
> (sync+async) + return-type/predicate backtracking + `(x:T)` type-cast + `CoverTypedIdentifier` (re-grew the
> `parse_assignment`/`conditional`/`arrow` signatures with the runtime control enums); **P6.2** `enum`; **P6.3** `component`/`hook`
> declarations + type annotations + the hook-type completion; **P6.4** `record` declarations + expressions; **P6.5** `match`
> expressions + statements (new `js/flow/match_.rs`; the call-vs-match reparse); **P6.6** the `declare` statement family +
> `import type`/`export type` clauses + Flow default exports (reverted the P4 import-kind erasure); capstone added the class-member
> `declare` modifier. **No AST nodes added** — the generated 271-node set already covered all of Flow (`generated_idempotent` stays the
> guardrail). **Review-caught bugs worth remembering** (the differential corpus missed all four — the two-stage review is why P6 is
> correct): (a) P6.1 — three call sites passed `CoverTypedParameters::Yes` where C++ passes explicit `No`, breaking `switch`/`case` and
> labelled statements under `-parse-flow`; (b) P6.4 — a record property initializer dropped the `[In]` grammar param
> (`parse_assignment_expression(Param::default())` vs C++ default `ParamIn`), so `record R { x: T = a in b }` errored; (c) **P6.5 — a
> CROSS-CUTTING infidelity: `lexer.lookahead1(None)` defaults to `RequireNoNewLine = true` (JSLexer.h:658), but 8 Flow sites (match +
> component/hook/record decl-checks + before-colon + hook-param, across P5/P6.3/P6.4/P6.5) used `::<false>`** — `match\n(x){…}` mis-parsed,
> `record\nFoo{}` became a RecordDeclaration instead of three statements; flipped all 8 to `::<true>`. **Lesson re-confirmed: C++ default
> args are spec — read the header.** And: a capstone that maps EVERY `getParseFlow*()`-gated C++ site to its Rust production catches the
> silently-dropped feature (the class-member `declare` modifier) that no per-sub-task review did.
>
>
> **🚧 P7 — TypeScript DONE** (2026-06-19). The entire `JSParserImpl-ts.cpp` (1,437 lines, 27 TS methods) + the 26 TS-only / shared
> integration sites in `JSParserImpl.cpp`, byte-for-byte vs `hermesc -dump-ast -parse-ts` over a **20-file TS corpus** (plus the 76-file
> plain + all Flow corpora, unchanged — TS does NOT leak). Plan: `plans/2026-06-19-js-parser-p7-typescript.md`. Behind a new
> `Context::parse_ts` flag (mutually exclusive with `parse_flow`; `-parse-ts` on hermesc, `--parse-ts` on `ast-dump`) in a new
> `js/ts/{mod,types,function_types,object_types,declarations,params}.rs` directory mirroring `js/flow/`. Sub-tasks (each implementer +
> spec review w/ adversarial diffing + structural-fidelity + quality review, then a whole-component capstone): **P7.0** foundations +
> gate (the flag end-to-end, the `ts/` skeleton, `type X = string;`); **P7.1** the type-annotation core (`parseTypeAnnotationTS` w/
> predicate backtrack + conditional types, union/intersection/postfix/primary, all keyword/literal types, type references, qualified
> names, type queries, tuples, type params/args); **P7.2** the function/constructor/parenthesized-type cover (the trickiest method —
> the `is_function`/`has_rest` state machine + the `dyn_cast` param/type disambiguation) + parameter properties; **P7.3** object types
> (call/method/property/index signatures, the index-sig `lookahead1::<true>` disambiguation, the wrapped-vs-bare returnType asymmetry);
> **P7.4** interface (+ heritage w/ the `_typeParameters` re-parenting, adapted to the immutable AST) / enum / namespace; **P7.5a**
> function & class integration (type params, return types, super type-args, member modifiers + `TSModifiers`, optional `?`); **P7.5b**
> expression & module integration (call/`new`/`?.` type args, `<Type>` casts, `as`/`as const`, typed arrows, `import type`) — OR-ing
> `parse_ts()` into the existing Flow-ambiguous gates without changing Flow behavior; added a dormant `Context::parse_jsx` flag (read by
> the cast gate, set in the JSX phase). **No AST nodes added** — the generated 271-node set already covered all of TS
> (`generated_idempotent` stays the guardrail). **The capstone confirmed completeness:** all 26 `getParseTS()` sites map to a Rust
> production, zero `// P7` markers remain, and every P6-style silent-drop candidate (`import typeof`, `export type`, `abstract`,
> `declare`, `satisfies`, definite-assignment `x!: T`, per-specifier `import { type X }`) is **rejected by hermesc itself** under
> `-parse-ts`, so the Rust port correctly omits them (legitimate non-features, not drops). **Lessons re-confirmed:** the grammar-context
> exceptions are spec (interface `extends` + enum-member `=` eat in `AllowRegExp`, type-assertion `>` in `AllowRegExp` so it splits, vs
> `Type` everywhere else), and `lookahead1(None)` = `RequireNoNewLine=true` → `::<true>`. **The parser now handles the ENTIRE
> standard-ECMAScript grammar + ALL of Flow + ALL of TypeScript.**
>
>
> **🚧 P8 — JSX DONE** (2026-06-19). The entire `JSParserImpl-jsx.cpp` (505 lines, 12 methods + the `tagNamesMatch` helper) + its single
> dispatch site, byte-for-byte vs `hermesc -dump-ast -parse-jsx` over a **6-file JSX corpus** + a **1-file flow+jsx corpus** (plus all prior
> corpora, unchanged — JSX does NOT leak). Plan: `plans/2026-06-19-js-parser-p8-jsx.md`. Behind the (pre-existing, from P7.5b)
> `Context::parse_jsx` flag (`-parse-jsx` on hermesc, `--parse-jsx` on `ast-dump`; independent of flow/ts), in a new single `js/jsx.rs`.
> Sub-tasks (each implementer + spec review w/ adversarial diffing + structural-fidelity + quality review, then a whole-component capstone):
> **P8.0** foundations + gate (`jsx_depth` field + the `JsxDepthGuard` RAII guard mirroring the C++ `SaveAndRestore<uint32_t>`, the `js/jsx.rs`
> skeleton, self-closing `<div/>` + the full `parse_jsx_element_name`); **P8.1** the rest — children, fragments, attributes (incl. spread),
> expression containers (incl. empty `{}`), `tag_names_match` closing-tag matching, and the opening-tag Flow `<TypeArgs>`. **No AST nodes
> added** — the generated 271-node set already covered all 16 `JSX*` nodes. **Capstone confirmed completeness:** all 12 methods + the single
> dispatch site map to a Rust production, zero `// P8` deferral markers remain. **The crux was the lexer-mode switch** — after each tag /
> child / closing, the parser calls EITHER `lexer.advance_in_jsx_child()` (stay in JSX-text mode) OR `advance()` (return to JS), chosen by the
> `jsx_depth` counter at 4 sites; and the grammar-context mix (`AllowJSXIdentifier` almost everywhere, with `AllowRegExp` at the `{`-child /
> spread / attribute-value advances). Two faithful-port subtleties verified correct: the C++ `isa<MemberExpressionNode>` member-name check
> (jsx.cpp:493) is **dead code** (`JSXMemberExpression` derives from the `JSX` base, not `MemberExpression`, per `ESTree.def` — disjoint
> hierarchies), mirrored harmlessly; and `<a/> / N` is a parse error under JSX (the post-self-close `advance()` is `AllowRegExp`, so `/` lexes
> as a regex), faithful to hermesc. **The parser now handles the ENTIRE standard-ECMAScript grammar + ALL of Flow + ALL of TypeScript + JSX.**
>
> **🚧 Pre/Lazy passes DONE** (2026-06-28). The three-pass Full/Pre/Lazy machinery + the on-demand `parse_lazy_function`, completing the
> Parser. Behind the design's resolution of an open question: the `-dump-ast` differential is BLIND to lazy parsing (the eager AST equals the
> lazy AST by construction), so two complementary oracles gate the phase instead. **Sub-tasks (each implementer + spec/structural/quality review):
> L0** foundations — `ParserPass{PreParse,LazyParse,FullParse}` enum + `pass` field, `PreParsedFunctionInfo`/`PreParsedBufferInfo` side-table,
> the `Context` preemptive-compilation threshold, the `SaveFunctionState` Drop-guard (3 arrow-bookkeeping `Rc<Cell<bool>>` flags +
> `seen_directives`) wired at every function-scope entry, the `arguments` site; **L1** PreParse — the two side-table store sites (`parseFunctionBody`
> body-start/overwrite + the arrow arrow-start/insert-if-absent) + `pre_parse_buffer`, plus **Oracle B**: a C++ `tools/preparse-dump/` tool +
> Rust `preparse-dump` bin + byte-for-byte `preparse_differential` of the side-table vs hermesc (13 lazy + 76 plain files); **L2** LazyParse —
> the skip-and-stub in `parse_function_body` (seek past bodies ≥ threshold → stub `BlockStatement{is_lazy_function_body}` with synthesized
> directives + decorations) + `parse_lazy_function` (the 5-kind demand dispatch) + the parser `seek`, plus **Oracle A**: a Rust-only
> `lazy_reparse` test proving deferred bodies reparse to the eager (hermesc-verified) AST (offset-set subset + per-leaf body-dump equality over a
> threshold sweep). **One remaining faithful-port deviation:** the side-table is threaded on the parser (the `GCLock` borrows `Context`
> immutably during parse) rather than stored on `Context`. The `AllocationScope` discipline IS ported: `support::Deque::truncate`
> + `iter_from` give arena-truncate semantics; `ast::AllocationScope` (a `GCLock`-scoped bump truncate with a documented no-escape
> contract) gates the two reclamation sites — see `specs/2026-07-15-preparse-scoped-reclamation-design.md`.
> **No AST nodes added** (the `BlockStatement` lazy decorations pre-existed). **Review-caught bugs (the oracles + capstone are why the port is
> correct):** (a) **L2.3 — `parse_lazy_function` set strict mode internally**, but C++ leaves that to the CALLER (`HBC.cpp:158`
> `setStrictMode(lazyData.strictMode)` before the call); Oracle A surfaced it (a class method's `static` mis-lexed in a sloppy caller context),
> fixed by moving strict-setting to the caller. (b) **Capstone — `SaveFunctionState` was constructed BEFORE formal-parameter parsing** in
> `parse_function_helper`, but C++ constructs it AFTER params (cpp:510), so a default-parameter arrow (`function f(x = () => arguments){}`) had its
> arrow-bookkeeping flags wrongly attributed to `f` instead of the enclosing scope — a real divergence from hermesc that BOTH oracles were blind
> to (no default-param arrow in the corpus); reproduced byte-for-byte by the opus whole-branch review, fixed by relocating the guard after params,
> and locked by a new `13_default_param_arrow.js` corpus file. **Lesson: a corpus-gated differential only proves what the corpus exercises; the
> structural-fidelity capstone (mapping every C++ pass/lazy site to its Rust production) is what found the gap.** Spec:
> `specs/2026-06-28-pre-lazy-passes-design.md`; plan: `plans/2026-06-28-js-parser-pre-lazy-passes.md`.
>
> **PreParse scoped reclamation DONE (2026-07-15).** The `AllocationScope` discipline is now fully ported (spec:
> `specs/2026-07-15-preparse-scoped-reclamation-design.md`; plan: `plans/2026-07-15-preparse-scoped-reclamation.md`).
> Two C++ scope sites ported: (1) the keeper-with-blank-body branch in `parse_function_helper_inner` (cpp:516-560),
> and (2) the whole-pass scope in `pre_parse_buffer` (cpp:7523). Peak PreParse AST is now ≈ skeleton + open function
> nest (measured: 251 vs 8,351 nodes mid-pass = 33× reduction; 0 residual post-pass), gated by
> `tests/preparse_memory.rs`. Oracle hardening: Oracle B (`preparse_differential`) now covers Flow/TS too — 13+76+42+20
> = 151 files byte-identical vs hermesc, with a non-degeneracy guard; Oracle A (`lazy_reparse`) now compares LOCATED
> dumps (78/78), covering the lazy source-range machinery. The parser `unsafe` surface gained two statement-scoped
> `#[allow(unsafe_code)]` sites (the two `alloc_scope` calls in `functions.rs` and `pre_lazy.rs`); the `AllocationScope`
> no-escape contract is documented in `ast/src/context.rs`.
>
> **The Parser component is COMPLETE. Next component: Sema** (scope resolution + FlowChecker). Write each phase plan just-in-time and execute
> subagent-driven.
>
> **Sema S0 (foundations, 2026-07-26), S1 (declarations & scopes, 2026-07-28), S2 (rest of the walk, 2026-07-28) and S3
> (`ScopedFunctionPromoter`, 2026-07-29) are DONE.** See the roadmap's Sema row above for what shipped, the gate command
> (173 corpus files, 97 succeeding on hermesc) and the S4a/S4b/S5 carry-items. **Next: S4a — standalone-front-end sema
> (module visit skeletons + `resolve_ast_for_parser` + untyped `-parse-flow` + the flags harness); S4b (VM modules:
> `$SHBuiltin` protocol/CJS/rewrite #4) is a separate much-later phase despite the shared number. No S4a plan exists
> yet, brainstorm then write it just-in-time.**

## Key cross-cutting design decisions

- **Locations are offset-based with explicit buffer identity** (`SMLoc = (SourceId, u32)`),
  not raw pointers. A location knows its buffer, so the C++ pointer reverse-lookup vanishes.
  Chosen over a packed global 32-bit offset (clang-style) for simplicity; that's a later
  swap behind the same accessors if AST memory pressure demands it.
- **The lexer's scan cursor is the one place encapsulated `unsafe` is allowed** (decision
  "B"): a raw `*const u8` cursor *inside the lexer module only*, converted to an offset at
  every boundary so nothing unsafe escapes. The buffer is handed in as an `Rc<SourceBuffer>`
  (stable heap address + no borrow fight with the manager). The `support` crate itself is
  zero-`unsafe`.
- **Diagnostics are byte-compatible** with LLVH/`hermesc` (decision "A"): column = byte
  distance from line start; caret columns are *code points* with tab expansion (TabStop 8);
  the caret line is shown **only for all-ASCII source lines** (Hermes punts on non-ASCII
  widths); `adjustSourceLocation` backs the column off `\r`/UTF-8 continuation bytes.
  Rendering goes through a pluggable `DiagHandler` trait, validated against captured
  `hermesc` output.
- **C++ RAII guards → explicit methods.** `SaveAndSuppressMessages`/`SaveAndBufferMessages`/
  `CollectMessagesRAII` can't be literal guards in safe Rust (a `&mut`-holding guard can't
  coexist with emitting through the manager, and the crate forbids `unsafe`), so each is an
  explicit set-restore / enable-disable / begin-end API — the full feature, minus the sugar.
- **Translator vs. rendering.** A `CoordTranslator` affects *displayed* coordinates
  (`find_coords`, `dump_coords`) but the rendered diagnostic resolves its source line and
  caret column from the **untranslated** location, matching the C++ primary diagnostic.

## Dependency analysis (why the lexer is next)

The front-end is a clean DAG up to bytecode generation. `BCGen` is the chokepoint — it
reaches *into the VM* (`Runtime`, `VMLayouts`), so it comes last. Everything in Layers 0–3
(Support → AST/Parser/Sema → IR/IRGen → Optimizer) is VM-independent. The lexer sits at the
very bottom (depends only on the support layer), is self-contained, and is trivially
differential-testable (bytes in → tokens out), so it is the natural first real port after
the diagnostics foundation. Full analysis was done conversationally; `doc/JunoRustCrates.md`
covers what juno already provides.

## Benchmark (first datapoint) — 2026-06-02

**Generator:** `rust/crates/parser/src/bin/gen_json.rs` (`gen-json` binary in the `parser` crate).
Deterministic (index-derived, no RNG); record shape:
`{"id":<i>,"name":"item-<i>","price":<f>,"active":<bool>,"tags":["a","b","c"],"nested":{"x":<i>,"y":<i*2>}}`
where `price = i / 7.0` (2 decimal places), `active = i % 2 == 0`.

**File:** 100,000 records → **11.6 MB** (`/tmp/big.json`; not committed).

**N = 50** iterations (each with a fresh arena/atom-table/parser factory).

| Build | Total (ms) | Throughput (MB/s) |
|-------|-----------|-------------------|
| **Rust `--release`** (`cargo build --release`) | **8,329.5** | **69.53** |
| **C++ Release** (`cmake -DCMAKE_BUILD_TYPE=Release`) | **8,205.6** | **70.58** |
| C++ ASan+Debug+-O1 (default dev build) | 98,167.2 | 5.90 |

**Interpretation:** Rust release and C++ release are essentially the same speed (~70 MB/s,
within 1.5% of each other) on this workload — a strong result for the port.  The
ASan+Debug+-O1 number (5.90 MB/s) is ~12x slower due to AddressSanitizer instrumentation
plus no optimisation; it is **not** a fair baseline and is included only for completeness.

To reproduce:
```bash
# Build tools
cargo build --manifest-path rust/Cargo.toml -p parser --release --bin gen-json
cargo build --manifest-path rust/Cargo.toml -p parser --release --bin json-parse-dump
cmake -B cmake-build-release -G Ninja -DCMAKE_BUILD_TYPE=Release
cmake --build cmake-build-release --target json-parse-dump

# Generate corpus (outside repo; not committed)
./rust/target/release/gen-json 100000 > /tmp/big.json

# Benchmark
./rust/target/release/json-parse-dump --bench=50 /tmp/big.json
cmake-build-release/bin/json-parse-dump --bench=50 /tmp/big.json
```

## How to validate

- Tests: `cargo test --manifest-path rust/Cargo.toml -p support`.
- Diagnostic differential: build the reference binary once —
  `cmake -B cmake-build-asan -G Ninja -DCMAKE_BUILD_TYPE=Debug -DHERMES_ENABLE_ADDRESS_SANITIZER=ON -DCMAKE_CXX_FLAGS="-O1" -DCMAKE_C_FLAGS="-O1"`
  then `cmake --build cmake-build-asan --target hermesc` — and capture references with
  `(! cmake-build-asan/bin/hermesc -dump-ast FILE 2>&1)` (stderr is color-free when piped).
  `cmake-build-asan/` is git-ignored.
