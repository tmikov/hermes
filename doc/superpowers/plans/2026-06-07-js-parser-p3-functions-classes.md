# JS Parser — Phase P3 (Functions, Classes, Arrows, Async/Generators) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) tracking. The C++ in `lib/Parser/JSParserImpl.cpp` IS the spec — port it faithfully from the cited line ranges; the byte-for-byte `parser_differential` vs `hermesc -dump-ast` is the gate.

**Goal:** Parse the full JavaScript *function, class, and concise-body* grammar — function declarations/expressions, generators, async functions, arrow functions, `yield` expressions, object methods/getters/setters, `super`, classes (methods, fields, static blocks, heritage) and decorators — so programs using them dump byte-identically to `hermesc -dump-ast`. This unblocks most of the P1/P2 honest-error deferrals.

**Architecture:** Extend `impl<'gc,'ast,'ctx,'a> JSParserImpl<...>` across `rust/crates/parser/src/js/{expressions,statements}.rs` (consider a new `js/functions.rs` and/or `js/classes.rs` sibling `impl` block if `expressions.rs`/`statements.rs` grow unwieldy — match the existing split-by-responsibility convention). Each P3 production currently emits an honest `"… (parser phase P3)"` error (grep them — there are ~12 in expressions.rs + 2 in statements.rs); P3 REPLACES each with the real parse. Nodes are built with the `ast` `new` constructors + `set_location`. Returns `Option<&'gc Node<'gc>>` / `bool`.

**Tech Stack:** the `ast` + `parser` crates; `hermesc` (`cmake-build-asan/bin/hermesc`) as the differential oracle.

**Spec:** `doc/superpowers/specs/2026-06-06-js-parser-design.md`. **Builds on P0+P1+P2** (`plans/2026-06-06-js-parser-{p0-foundations,p1-expressions,p2-statements}.md`).

## Conventions (carry over from P0/P1/P2 — do not relitigate)
- Faithful port; keep C++ structure + comments + cited line ranges; `Option<T>`/`None` = error reported with `?`; `bool`/`false` for list-builders. Zero `cargo build` warnings. Commit directly to `rust`; trailer `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`.
- **C++ `template`s → Rust generics** (none new in P3 — `parseStatementList` is the only one and it's already the const-generic). **C++ RAII guards → explicit set/restore.** The pervasive `llvh::SaveAndRestore<bool>` on `paramYield_`/`paramAwait_` (function/method/arrow bodies flip these) → an explicit save-local + restore-at-end pattern, OR a small `ParamStateGuard` helper struct with `Drop` that restores (mirror the existing `RecursionGuard` in `js/mod.rs` — it owns the restore so it survives early `?` returns). **Prefer a `Drop` guard** here because the C++ relies on RAII restore-on-every-exit-path and the bodies have many `?` early-returns; a manual restore-at-end would leak on `?`. Add `param_yield`/`param_await` save/restore guard(s) in `js/mod.rs`.
- **Node construction:** `Node::<Name>(<Name>::new(NodeMetadata::new(self.dummy_range()), <args in node.rs order>))` + `set_location`. **Verify every ctor arg order against `rust/crates/ast/src/node.rs`** — the C++ ctor arg order is the source of truth (the generated `new` mirrors `ESTree.def` field order). The function/class nodes carry Flow/TS fields (`typeParameters`, `returnType`, `predicate`, `superTypeParameters`, `implements`) — pass `None`/empty for all of them (Flow/TS are P6/P7).
- **`set_location` accepts `SMLoc`**; for node endpoints use `node.metadata().range.get().start/.end`, for tokens `self.lexer.token().start_loc()/end_loc()`. The 4-arg `set_location_d` (explicit debug loc) exists.
- **All P3 AST nodes already exist** in the generated set: `FunctionDeclaration`, `FunctionExpression`, `ArrowFunctionExpression`, `YieldExpression`, `AwaitExpression` (built in P1.3), `ClassDeclaration`, `ClassExpression`, `ClassBody`, `MethodDefinition`, `ClassProperty` (class fields), `StaticBlock`, `Decorator`, `Super`, `MetaProperty` (built in P1.6). No AST regen needed — the `generated_idempotent` guard must stay green.

## Pass model (IMPORTANT — simplifies P3)
The Rust parser is **Full-pass / eager only** — there is NO `PreParse`/`LazyParse` machinery (`pass_`, `preParsed_`, `AllocationScope`, `parseLazyFunction`). Therefore, in every C++ function that branches on `pass_`:
- **Omit the `pass_ == PreParse` blocks** (e.g. `parseFunctionHelper` 516-560, `parseArrowFunctionExpression` 5896-5908) — port ONLY the eager path. Add a one-line comment `// Full-pass only: the C++ PreParse/Lazy path is not ported (no lazy-compile pass in the Rust port yet).`
- **`parseFunctionBody` (740-813):** port ONLY the eager tail (799-812 minus the `pass_==PreParse` store): `parseBlock(ParamReturn, grammarContext, parseDirectives)`. Omit the `pass_ == LazyParse && !eagerly` block (747-797) and the PreParse store (803-810). The `eagerly`/`paramYield`/`paramAwait`/`grammarContext`/`parseDirectives` params are still threaded (callers pass them).
- **`SaveFunctionState`** (header ~1699) saves `containsArrowFunctions_`/`mayContainArrowFunctionsUsingArguments_` — lazy-compile bookkeeping that feeds only un-dumped `BlockStatement` fields. Port as a **no-op** (or a guard that saves/restores those bool fields IF they exist in the Rust `JSParserImpl` struct; if they don't exist, don't add them — just `// SaveFunctionState: lazy-compile bookkeeping, not modeled in the Full-pass port.`). Do NOT let its absence change observable parse output.

## Deferral policy (honest stubs, not silent fallthrough)
- **Flow/TS** everything: type parameters (`<...>` after function/method/class name), return-type/parameter type annotations (`:` blocks), the `this`-param in `parseFormalParameters` (607-633), Flow predicates, `implements`, TS `accessibility`/`abstract`/`declare`/`override`/`readonly` class-member modifiers, TS index signatures — all `context_.getParseFlow()/getParseTS()/getParseTypes()`-gated and OFF in the P3 corpus. **Omit these blocks** (don't port the bodies); leave a `// P6/P7: Flow/TS …` comment where the C++ has the `#if`. They never fire for plain JS.
- **Lazy/PreParse** — omitted per the pass-model section above.
- If P3.6 (classes) surfaces a genuinely separate large feature mid-implementation, it is acceptable to land class **methods + fields + static blocks** first and defer an exotic corner with an honest error + test — but the goal is full class support in P3.6.

## Validation (every sub-task)
Extend `rust/crates/parser/tests/parser_corpus/` with `*.js` files exercising the new forms, then:
```bash
cargo build --manifest-path rust/Cargo.toml -p parser --bin ast-dump
REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p parser --test parser_differential -- --nocapture
cargo test  --manifest-path rust/Cargo.toml -p parser
cargo build --manifest-path rust/Cargo.toml              # ZERO warnings
```
Verify each new corpus file with `cmake-build-asan/bin/hermesc -dump-ast -dump-source-location=both <file>` (exit 0, valid JS) BEFORE adding it. Keep files small/focused.

---

## Sub-tasks (ordered; each = one implementer + spec review + quality review + commit)

### P3.1 — Function declarations & expressions + formal parameters + function body (incl. generators & async functions)
**C++:** `parseFunctionHelper` (383-598; omit PreParse 516-560 + Flow/TS type-param/return-type `#if` blocks 431-447, 470-498), `parseFormalParameters` (600-667; omit the Flow/TS `this`-param block 607-633), `parseFunctionBody` (740-813; eager tail only — see pass-model), the header wrappers `parseFunctionDeclaration` (calls `parseFunctionHelper(param, /*isDecl*/ true, …)`) and `parseFunctionExpression` (`parseFunctionHelper(Param{}, /*isDecl*/ false, …)`). `parseFunctionHelper` already handles `*` (generator) and leading `async` (async function) — so generators + async functions come for free.
**RAII:** the `SaveAndRestore<bool>` on `paramYield_`/`paramAwait_` at 411-413 (name binding), 461-463 (params+body) → explicit `Drop`-guard save/restore (see Conventions). The C++ flips them per the generator/async grammar table (410-413, 461-463) — port that table exactly.
**Nodes:** `FunctionDeclaration(id?, params, body, typeParams=None, returnType=None, predicate=None, generator, async)`, `FunctionExpression(id?, params, body, typeParams=None, returnType=None, predicate=None, generator, async)` — VERIFY arg order in node.rs. `id` is `Option` (anonymous default-export decl can be None; here always Some for non-default decls — but the field is optional).
**Wire:** (1) `parse_declaration` (statements.rs ~254) function/async-function branch → `parse_function_declaration`; (2) `parsePrimaryExpression` `rw_function` (expressions.rs ~3054) + the async-function-as-primary → `parse_function_expression`; (3) `parseStatement`/`parseStatementListItem` already route `function`/`async function` into `parseDeclaration` via `checkDeclaration` (P2.3) — confirm it now reaches the real parse. Also the labelled/if `rw_function` honest-errors (statements.rs) can now call the real `parse_function_declaration` per C++ (labelled function: still emit the C++ "Function declaration not allowed as body of labeled statement" error but PRODUCE the node, C++ 1646-1656; if-statement function: wrap in synthetic implicit BlockStatement, C++ 1709-1737). Implement those two now that `parse_function_declaration` exists.
**Note:** `await` expressions (P1.3) and `yield` (P3.2) become *reachable* inside async/generator bodies. `await` already works; `yield` is still a P3 honest-error until P3.2 — so a generator body containing `yield` errors honestly until P3.2. The P3.1 corpus uses function bodies WITHOUT `yield`.
- [ ] Corpus: `func_decl.js` (`function f(){} function g(a, b){return a;} function* h(){} async function k(){}`), `func_expr.js` (`(function(){}); (function named(){}); var f = function(a){return a;}; (async function(){}); (function*(){});`), `func_params.js` (`function f(a, b = 1, ...rest){} function g({x, y}, [z]){}`). Plus a label/if function test if you implement those branches.
- [ ] TDD: corpus FAIL→PASS; unit tests (generator node `.generator==true`, async `.async==true`, params include RestElement & AssignmentPattern & ObjectPattern); zero warnings.
- [ ] Commit `rust(parser): P3.1 function declarations/expressions + params + body`.

### P3.2 — `yield` expressions
**C++:** `parseYieldExpression` (4652-4687) — `yield`, `yield expr`, `yield* expr`; the ASI/`[no LineTerminator]` rule before the argument; guarded by `paramYield_`. Wire into `parseAssignmentExpression` (the P3 stub at expressions.rs ~270, which is the C++ 6243-6248 `if (paramYield_ && check(rw_yield)) return parseYieldExpression(param)` path). `await` is already done (P1.3) and now reachable.
**Nodes:** `YieldExpression(argument?, delegate)` — VERIFY arg order + the `delegate` bool (`yield*`).
- [ ] Corpus: `expr_yield.js` — must be INSIDE a generator (yield at top level is invalid): `function* g(){ yield; yield 1; yield* a; var x = yield 2; }`. Verify with hermesc.
- [ ] TDD: FAIL→PASS; unit test (`yield* a` → delegate=true; `yield` → argument None). Zero warnings.
- [ ] Commit `rust(parser): P3.2 yield expressions`.

### P3.3 — Arrow functions (incl. async arrows + cover-paren reparse)
**The trickiest sub-task** — the cover grammar. `( a, b ) => …` is first parsed as a parenthesized/sequence expression (or a cover node for `()`/`(...rest)`), THEN the `=>` triggers `reparseArrowParameters` to convert it to a parameter list.
**C++:** `parseArrowFunctionExpression` (5818-5911; omit PreParse 5896-5908), `reparseArrowParameters` (5681-5817), and the **`=>` detection + cover handling in `parseAssignmentExpression`** (6233-6551 — read it fully; the arrow path replaces the P3 stub at expressions.rs ~298). Also the cover-paren primary forms: `()` → `CoverEmptyArgsNode`, `(...rest)`/`(a,)` → `CoverRestElementNode`, in `parsePrimaryExpression`'s `l_paren` branch (the P1 deferral note at expressions.rs ~66; C++ `parsePrimaryExpression` `l_paren` case + `parseParenExpr` style). Async arrows: `async ident =>` and `async (params) =>` — the `async` detection in `parseAssignmentExpression`/the LHS (expressions.rs ~3012, 3020 stubs).
**Signature note (IMPORTANT):** P1 simplified `parse_assignment_expression(param)` (dropped C++'s `eagerly`, `AllowTypedArrowFunction`, `CoverTypedParameters`, and the `leftExpr` reparse hook). P3.3 must restore enough to detect+parse arrows. Minimum faithful surface: thread `eagerly: bool` (default false) where the arrow path needs it; the `AllowTypedArrowFunction`/`CoverTypedParameters` args are Flow/TS — keep them defaulted/omitted with a `// P6/P7` note. Decide whether to widen `parse_assignment_expression`'s signature or add an internal helper; **prefer matching the C++ signature** (add the params) so the structure stays faithful — but the Flow/TS-only params may be omitted. Document the chosen surface in the sub-task.
**Nodes:** `ArrowFunctionExpression(id=None, params, body, typeParams=None, returnType=None, predicate=None, expression: bool, async: bool)` — VERIFY order; `expression`=true for concise-body (`=> expr`), false for block body (`=> {}`). `CoverEmptyArgs`/`CoverRestElement` cover nodes (check node.rs names — they may be `CoverEmptyArgs`/`CoverRestElement` or similar; they're transient, reparsed away).
- [ ] Corpus: `expr_arrow.js` (`a => a; (a) => a; (a, b) => a + b; () => 0; (a, ...b) => b; (a = 1) => a; ({x}) => x; a => { return a; }; async a => a; async (a) => a;`). Verify each with hermesc.
- [ ] TDD: FAIL→PASS; unit tests (concise vs block `expression` flag; async flag; params are patterns; `() =>` empty params; rest param). Zero warnings.
- [ ] Commit `rust(parser): P3.3 arrow functions`.

### P3.4 — Object methods / getters / setters / async & generator methods
**C++:** rewrite `parsePropertyAssignment` (2829-3266) to FULL fidelity, replacing the P1.8 data-property subset + the get/set/async/generator/method honest-errors (expressions.rs ~2278/2337/2398/2448/2497). Branches: `get`/`set` (getter/setter → `PropertyNode` kind `get`/`set`, value = `FunctionExpression` with `isMethodDefinition=true`); `async`/`async *` methods; `*` generator methods; plain `name(){}` methods; data props + shorthand + CoverInitializer (already in P1.8 — preserve). Uses `parseFormalParameters`/`parseFunctionBody` (P3.1) + `parsePropertyName`. The `SaveAndRestore<bool>` on paramYield/paramAwait per branch → the Drop-guard.
**Nodes:** `Property(key, value, kind, computed, method, shorthand)` with kind ∈ {init, get, set}; `FunctionExpression` value with the method flag (`isMethodDefinition` — check if the Rust node has this field / whether it's dumped; if it's a decoration not in `.def`, it won't dump — set it if present, else note).
- [ ] Corpus: `obj_methods.js` (`({ m(){}, *g(){}, async a(){}, async *ag(){}, get x(){return 1;}, set x(v){}, [k](){}, 'str'(){}, 42(){} });`). Verify with hermesc.
- [ ] TDD: FAIL→PASS; unit tests (getter kind=="get"; method flag; generator/async method flags; computed-key method). Zero warnings. Confirm the P1.8 data-property tests still pass.
- [ ] Commit `rust(parser): P3.4 object methods/getters/setters`.

### P3.5 — `super` (member & call)
**C++:** `super` handling in `parseLeftHandSideExpression`/member/call (the P3 stub at expressions.rs ~1603; C++ `parseSuperExpression`/`rw_super` in `parseLeftHandSideExpression` ~4014-4089 and `parseMemberSelect`/`parseCallExpression`). `super.prop`, `super[expr]`, `super(args)` → `Super` node wrapped in `MemberExpression`/`CallExpression`. (`new.target` MetaProperty is already P1.6.)
**Nodes:** `Super` (no children — metadata only).
- [ ] Corpus: `expr_super.js` — must be in valid positions (super call inside a derived-class constructor; super member inside a method). E.g. `class A extends B { constructor(){ super(); } m(){ return super.x + super['y']; } }` — **but classes are P3.6.** Sequencing: if P3.6 lands AFTER P3.5, unit-test `super` here and add the corpus file in P3.6; OR reorder P3.5 after P3.6. **Recommendation:** do P3.5 INSIDE/just-before P3.6 so super has a valid class context for the corpus. If kept separate, unit-test super via a method-context harness and defer the corpus to P3.6.
- [ ] TDD: super member/call node shapes; zero warnings.
- [ ] Commit `rust(parser): P3.5 super member & call`.

### P3.6 — Classes + decorators
**C++:** `parseClassDeclaration` (4793-4874), `parseClassExpression` (4875-4952), `parseClassTail` (4953-5049 — heritage `extends LeftHandSideExpression` via `parseLeftHandSideExpression(IsClassHeritageArgument::Yes)` already plumbed in P1; omit Flow/TS `implements`/type-params), `parseClassBody` (5050-5077), `parseClassBodyImpl` (5078-5202), `parseClassElement` (5203-5680 — methods, get/set, generator/async methods, fields, `static` members, `static {}` blocks, computed keys, private names; omit TS modifiers), `parseDecoratorList` (4688-4700) + `parseDecorator` (4701-4792). Strict-mode is forced inside class bodies (C++ sets it) — port that.
**Nodes:** `ClassDeclaration(id?, superClass?, body, typeParams=None, superTypeParams=None, implements=[])`, `ClassExpression(...)`, `ClassBody(body)`, `MethodDefinition(key, value, kind, computed, static)` (kind ∈ constructor/method/get/set), `ClassProperty(key, value?, computed, static, …)` for fields, `StaticBlock(body)`, `Decorator(expression)`, `Super` (heritage uses LHS, not Super node), `PrivateName` (P1.6) for `#x` members. VERIFY every arg order.
**Wire:** `parse_declaration` class branch (statements.rs ~263) → `parse_class_declaration`; `parsePrimaryExpression` class (expressions.rs ~3062) + `@`-decorated class → `parse_class_expression`/decorator list then class.
- [ ] Corpus (verify each with hermesc): `class_basic.js` (`class A {} class B extends A {} const C = class {}; class D extends (class{}) {}`), `class_methods.js` (`class A { m(){} static s(){} get x(){return 1;} set x(v){} *g(){} async a(){} async *ag(){} ['c'](){} #p(){} static #sp(){} constructor(){} }`), `class_fields.js` (`class A { x; y = 1; static z = 2; #p = 3; static #sp; ['c'] = 4; static { this.q = 1; } }`), `class_decorators.js` (`@dec class A {} class B { @dec m(){} @dec x = 1; }` — verify hermesc accepts decorators; if gated behind a flag, drop or guard).
- [ ] TDD: FAIL→PASS; unit tests (heritage superClass node; MethodDefinition kind=="constructor"/"get"; static flag; ClassProperty field with/without init; StaticBlock; private-name member; decorator list). Zero warnings.
- [ ] Commit `rust(parser): P3.6 classes + decorators`.

---

## P3 capstone (after all sub-tasks)
Per SESSION-HANDOFF §5.7:
- **Structural-fidelity grep:** `grep -n "template <" lib/Parser/JSParserImpl.cpp` over the P3 ranges (383-813, 2829-3266, 4652-5911) — confirm no template→runtime flattening (none expected). Confirm the `SaveAndRestore<bool>` paramYield/paramAwait conversions are faithful Drop-guards (not manual restores that leak on `?`), and `SaveFunctionState`/PreParse/Lazy omissions changed NO observable output.
- **Re-derive the deferral set:** every remaining honest error is now Flow/TS (P6/P7) or lazy-parse — grep for `phase P3` strings and confirm ZERO remain in expressions.rs/statements.rs (every P3 production implemented). Confirm Flow/TS gates omitted (off).
- **Node field-order audit** over all P3 nodes (Function*/Arrow/Yield/Class*/MethodDefinition/ClassProperty/StaticBlock/Decorator/Super), esp. the boolean flags (generator/async/expression/static/computed) and Optional children.
- Full `cargo test` workspace green; `REQUIRE_DIFFERENTIAL=1` differential green over the expanded corpus; zero warnings; `REQUIRE_GEN=1 … generated_idempotent` green (no AST regen).
- Update `RustPortRoadmap.md` (P3 DONE block) and `SESSION-HANDOFF.md` (NEXT: P4 — modules: import/export declarations + `import()`/`import.meta`).

## Self-review notes (author)
- **Spec coverage:** function helper/params/body (P3.1), yield (P3.2), arrows+cover (P3.3), object methods/get/set (P3.4), super (P3.5), classes+decorators (P3.6). `await` was P1.3 (now reachable). PreParse/Lazy explicitly omitted (Full-pass port). Flow/TS explicitly deferred.
- **Highest-risk items:** (a) P3.3 cover-grammar reparse + the `parse_assignment_expression` signature restoration; (b) the paramYield/paramAwait Drop-guard correctness across `?` early-returns (a manual restore would leak — mandate the guard); (c) node field-order on the 8-arg Function nodes and MethodDefinition/ClassProperty flags.
- **Sequencing caveat:** P3.5 (super) needs a class context for a valid corpus file — either fold super's corpus into P3.6 or run P3.5 immediately before P3.6 (the implementer/controller decides; unit tests can cover super in isolation via a method harness).
