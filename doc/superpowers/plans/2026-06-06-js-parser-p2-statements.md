# JS Parser — Phase P2 (Statements & Declarations) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) tracking. The C++ in `lib/Parser/JSParserImpl.cpp` IS the spec — port it faithfully from the cited line ranges; the byte-for-byte `parser_differential` vs `hermesc -dump-ast` is the gate.

**Goal:** Parse the full JavaScript *statement and declaration* grammar (blocks, var/let/const/using declarations, if/while/do/for/for-in/for-of, switch, try/catch/finally, return/break/continue/throw/with/debugger, labelled statements, and binding patterns) so programs of statements dump byte-identically to `hermesc -dump-ast`.

**Architecture:** Extend `impl<'gc,'ast,'ctx,'a> JSParserImpl<...>` in `rust/crates/parser/src/js/statements.rs` (and a small amount in `js/mod.rs` for shared helpers), mirroring the C++ recursive-descent statement parser. P1 already built the expression grammar + the statement-list spine + `parseEmptyStatement` + directive prologue + the expression-statement path; P2 fills in every statement keyword case that currently emits an honest `"… (parser phase P2)"` error, plus declarations and binding patterns. Each node is built with the `ast` `new` constructors + `set_location`. Returns `Option<&'gc Node<'gc>>` / `bool` (`None`/`false` = error already reported).

**Tech Stack:** the `ast` + `parser` crates; `hermesc` (`cmake-build-asan/bin/hermesc`) as the differential oracle.

**Spec:** `doc/superpowers/specs/2026-06-06-js-parser-design.md`. **Builds on P0+P1** (`plans/2026-06-06-js-parser-{p0-foundations,p1-expressions}.md`).

## Conventions (carry over from P0/P1 — do not relitigate)
- Faithful port; keep C++ structure + comments; `Option<T>`/`None` = error-reported with `?` propagation; `bool`/`false` for the `parse*List`-style helpers that push into a `Vec`. **C++ `template`s → Rust generics** (see P2.4 `parse_statement_list` below — the variadic `template<typename...Tail>` becomes a `const N: usize` generic over `[TokenKind; N]`, NOT a runtime `&[TokenKind]` param; this preserves the monomorphization). **RAII → explicit.** Zero `cargo build` warnings. Commit directly to `rust`; trailer `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`.
- **Node construction:** use `ast::node::<Name>::new(metadata, ...)` wrapped in `Node::<Name>(...)`, then `set_location(start, end, node)`. Look up the exact generated constructor signature + field order in `rust/crates/ast/src/node.rs` (snake_case fields; the `new` defaults decorations/`Cell`s). All P2 statement/pattern nodes already exist in the generated set (`BlockStatement`, `VariableDeclaration`, `VariableDeclarator`, `IfStatement`, `WhileStatement`, `DoWhileStatement`, `ForStatement`, `ForInStatement`, `ForOfStatement`, `ContinueStatement`, `BreakStatement`, `ReturnStatement`, `WithStatement`, `SwitchStatement`, `SwitchCase`, `ThrowStatement`, `TryStatement`, `CatchClause`, `DebuggerStatement`, `LabeledStatement`, `ArrayPattern`, `ObjectPattern`, `RestElement`, `AssignmentPattern`, `Property`, `Empty`, `Identifier`, `PrivateName`).
- **`NodeList` fields** (e.g. `BlockStatement.body`, `VariableDeclaration.declarations`, `SwitchStatement.cases`, `SwitchCase.consequent`, `ArrayPattern.elements`, `ObjectPattern.properties`): build a `Vec<&'gc Node<'gc>>` then `NodeList::from_iter(self.gc, vec)` exactly as `parse_program`/the array & object literals already do. Check P1 `expressions.rs` for the established pattern.
- **`set_location` start/end accept `SMLoc`.** The C++ `setLocation` also accepts a token or a node for start/end (`node->getStartLoc()`/`getEndLoc()`, `tok_->getStartLoc()`). In Rust use `node.metadata().range.get().start` / `.end` for node endpoints and `self.lexer.token().start_loc()/end_loc()` for token endpoints, or add a small `set_location_n(start_node, end_node, node)` helper in `js/mod.rs` if it reduces noise (port faithfully from `JSParserImpl.h:368-414`). The 4-arg `set_location_d` (explicit debug loc) already exists for the `VariableDeclarator` initializer case.
- **`eat`/`need`/`error_expected`:** P1 uses the simplified `need(kind, where_)` / `eat(kind, gc, where_)` (no second "note" source-range argument). The C++ `eat`/`need`/`errorExpected` take extra `what`/`sourceRange` note args used to print a secondary "location of …" note. **The differential corpus is valid JS, so these error paths never fire** — keep the simplified signatures and DO NOT thread the note args. Where C++ calls `errorExpected(k1, k2, …)` (two expected tokens), add a small `error_expected2(k1, k2, where_)` helper in `js/mod.rs` that prints `"'{k1}' or '{k2}' expected{where_}"` and reports at the current token (faithful message shape; note suppressed). Error-message-note fidelity is a tracked carry-forward (see P1's identical note), NOT a P2 blocker.
- **`eat_semi` gains the `optional` flag.** C++ `eatSemi(bool optional=false)` (lines 323-338): when `optional==true` it does NOT report `"';' expected"` on failure (used by do-while / continue / break / return where the ASI rule is applied leniently). The current Rust `eat_semi(&mut self) -> bool` matches `optional=false`. Change it to `eat_semi(&mut self, optional: bool) -> bool` and update the single existing call site (`parse_expression_or_labelled_statement`) to pass `false`. (NB: the existing `eat_semi` advances with `GrammarContext::AllowRegExp`; keep that — C++ `advance()` defaults to `AllowRegExp`.)
- **Keyword identifiers** (`let`, `using`, `await`, `of`, `async`): there is no `Keywords kw_` struct in the Rust port; use the existing `check_unescaped_name(b"let")` helper (`expressions.rs:1979`) which checks the current token is an unescaped `identifier` with the given bytes. For the `VariableDeclaration.kind` atom and `using`/`await using` kind strings, intern via `self.gc.ctx().atom_table.atom_bytes(b"…")` exactly as P1 interns `b"init"` for `Property.kind`. The kind atom for `var`/`let`/`const` is the token's own identifier/keyword text — get it from `self.lexer.token()` (for `rw_const`/`rw_var` use the keyword spelling; mirror C++ `tok_->getResWordOrIdentifier()`; if no direct Rust accessor exists, intern the literal `b"var"`/`b"const"`/`b"let"` matching the matched token — add a tiny helper and comment it).

## Deferral policy (IMPORTANT — honest stubs, not silent fallthrough)
P2 implements statements + var/let/const/using declarations + binding patterns. These remain DEFERRED; where a C++ branch reaches one, keep/emit an explicit `self.error_cur("<thing> not yet supported (parser phase <PN>)")` + `return None`, covered by a test:
- **Function / async-function / class / `@decorator` declarations** (`parseDeclaration` cases `rw_function`, async-function, `at`/`rw_class`) → **P3**. In `parseDeclaration`, port the dispatch structure but the function/class branches call `parse_function_declaration`/`parse_class_declaration` which do not exist yet → emit the P3 error there (do NOT silently skip). `checkDeclaration()` must still RETURN TRUE for these so the statement-list routes into `parseDeclaration` (matching C++) — the honest error fires inside `parseDeclaration`.
- **`import` / `export` declarations** → **P4**. Keep the existing honest errors in `parse_statement_list_item` for the `import`/`export` paths, BUT port the C++ `import` lookahead (lines 898-923): if `import` is followed by `(` or `.`, route to `parseStatement` (expression statement → which itself errors on `import()`/`import.meta`, already deferred to P4); otherwise the import-declaration path errors (P4). Net: every `import`/`export` form still errors, but via the faithful branch.
- **The `if`/labelled `function`-as-body sub-cases** (`parseStatementOrFunctionDeclaration` lambda in `parseIfStatement` lines 1709-1737; the `rw_function` branch in `parseExpressionOrLabelledStatement` labelled path lines 1642-1656) call `parseFunctionDeclaration` → **P3**: where the C++ checks `check(TokenKind::rw_function)` inside these, emit the P3 honest error + `return None`. The non-function path (a normal statement body) is fully implemented.
- **Flow / TS declaration branches** in `parseDeclaration`/`parseStatementListItem`/`checkDeclaration` → **P6/P7**, already `context_.getParseFlow()/getParseTS()`-gated (off in the P2 corpus). Leave the gates out (or behind `false` consts) — they never fire for plain JS.
- **Type annotations on binding identifiers/patterns** (`#if HERMES_PARSE_FLOW || HERMES_PARSE_TS` blocks in `parseBindingIdentifier` 1065-1080, `parseArrayBindingPattern` 1344-1354, `parseObjectBindingPattern` 1475-1485) → **P6/P7**, `context_.getParseTypes()`-gated (off). Pass `type = nullptr`/`None` and `optional = false`; do not port the `?`/`:` type-annotation reads.
- **`for await`** (`paramAwait_` true) and **`yield`/`await` as binding identifier** validations (`validateBindingIdentifier` 1008-1045): port the validation structure (it reads `is_strict_mode()`/`param_yield`/`param_await`), but `param_await`/`param_yield` are false at the program top level until P3, so the `for await`/await-using branches won't fire in the P2 corpus. Port them faithfully anyway (they're cheap and structurally required); just don't add corpus files that need them.

## Validation (every sub-task)
Extend `rust/crates/parser/tests/parser_corpus/` with `*.js` files exercising the new forms, then:
```bash
cargo build --manifest-path rust/Cargo.toml -p parser --bin ast-dump
REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p parser --test parser_differential -- --nocapture
cargo test  --manifest-path rust/Cargo.toml -p parser    # unit/ported tests
cargo build --manifest-path rust/Cargo.toml              # ZERO warnings
```
For each new corpus file, first confirm `hermesc -dump-ast -dump-source-location=both <file>` succeeds (valid JS) before adding it. Keep each construct's corpus file small and focused; the harness diffs byte-for-byte.

---

## Sub-tasks (ordered; each = one implementer + spec review + quality review + commit)

### P2.1 — Simple statements (no binding patterns) + labelled statements + `eatSemi(optional)`
Wire up every statement keyword case that does not need binding patterns or declarations. After this task `parseStatement`'s `error_cur("… (parser phase P2)")` arms for these kinds are REPLACED by real parse calls; the remaining deferred arms (`l_brace` block → P2.4, `rw_var` → P2.3, `rw_if`/`rw_while`/`rw_do`/`rw_for`/`rw_switch`/`rw_try` → later sub-tasks) keep their honest errors until their sub-task lands.

**C++:**
- `eatSemi` (323-338) — add the `optional` param (see Conventions).
- `parseExpressionOrLabelledStatement` (1600-1677) — replace the P1 "labelled not supported" stub with the real labelled-statement path: `id ":" Statement` → `LabeledStatementNode(id, body)`. The `rw_function`-as-labelled-item sub-case (1642-1656) → P3 honest error. Keep the P1 "declaration not allowed as expression statement" warning (1609-1615) and the `let`-disambiguation (1617-1627) — the `let` seek/re-lex (port `lexer_.seek` + `advance`).
- `parseDebuggerStatement` (2467-2479) → `DebuggerStatementNode`.
- `parseThrowStatement` (2342-2364) → `ThrowStatementNode` (note the same-line check via `is_new_line_before_current_token`).
- `parseReturnStatement` (2160-2181) → `ReturnStatementNode` (uses `eat_semi(true)` for the no-argument form).
- `parseBreakStatement` (2128-2158) / `parseContinueStatement` (2095-2126) → `BreakStatementNode`/`ContinueStatementNode` (optional label identifier; `eat_semi(true)` first).
- `parseWithStatement` (2183-2218) → `WithStatementNode` (body via `parseStatement`).

**Files:** `js/statements.rs` (replace the stub arms in `parse_statement` + rewrite `parse_expression_or_labelled_statement`; add the new fns), `js/mod.rs` (`eat_semi` signature is in statements.rs — just update its callers; add `error_expected2` helper if needed by later tasks — optional here).
**Nodes:** `LabeledStatement`, `DebuggerStatement`, `ThrowStatement`, `ReturnStatement`, `BreakStatement`, `ContinueStatement`, `WithStatement`, `Identifier`.

- [ ] **Step 1: Failing differential corpus + unit tests.** Add corpus files (verify each with `hermesc -dump-ast -dump-source-location=both` first):
  - `stmt_labelled.js`: `foo: x; outer: while(0) x;` — NB the `while` needs P2.4; for THIS task use a non-loop body: `foo: x; bar: ;` and `lbl: a.b;`
  - `stmt_throw.js`: `throw x; throw new Error("e");`
  - `stmt_return.js`: wrap in a context hermesc accepts at top level — `return` at top level errors ("'return' not in a function") UNLESS `-commonjs`/allowReturnOutsideFunction. **Do NOT corpus-test bare `return` at top level**; instead unit-test it (see below) and defer a return corpus file to P3 (inside a function). Same for `break`/`continue` (need an enclosing loop/label to be valid — but the PARSER accepts them syntactically; semantic validation is post-parse and `-dump-ast` is pre-Sema, so `break;`/`continue;` at top level DO dump cleanly). Verify: `echo 'break; continue; lbl: break lbl;' | hermesc -dump-ast -dump-source-location=both /dev/stdin`.
  - `stmt_break_continue.js`: `lbl: break lbl; lbl2: continue lbl2; while_placeholder: break while_placeholder;` → simplest valid: `a: break a; b: continue b;`
  - `stmt_with.js`: `with(x) y;` (valid in sloppy mode; ensure no `"use strict"`).
  - `stmt_debugger.js`: `debugger; debugger`
  Run `REQUIRE_DIFFERENTIAL=1 cargo test … --test parser_differential` → these new files FAIL (current code errors on `throw`/`with`/`debugger`, mis-handles labelled).
- [ ] **Step 2: Implement** the C++ functions above in `js/statements.rs`; change `eat_semi` to take `optional: bool`; replace the corresponding `parse_statement` stub arms with the real calls (`rw_continue`→`parse_continue_statement`, `rw_break`→`parse_break_statement`, `rw_return`→ the return-guard + `parse_return_statement`, `rw_with`→`parse_with_statement`, `rw_throw`→`parse_throw_statement`, `rw_debugger`→`parse_debugger_statement`). Port the `parse_statement` `rw_return` guard (lines 698-701: error `"'return' not in a function"` when `!param.has(PARAM_RETURN)` and not `allow_return_outside_function` — the latter is a context flag; thread a `false` const for now with a `// P-future: context_.allowReturnOutsideFunction()` comment).
- [ ] **Step 3:** Build `ast-dump`; run the differential — new files match. Run `cargo test -p parser` (add unit tests: `return` outside function reports the error; `throw\nx` (newline) reports "'throw' argument must be on the same line"; a labelled statement produces `LabeledStatement`).
- [ ] **Step 4:** `cargo build` (zero warnings).
- [ ] **Step 5: Commit** `rust(parser): P2.1 simple statements + labelled + eatSemi(optional)`.

### P2.2 — Binding identifiers & binding patterns
Port the binding-target grammar used by declarations, `for`, and `catch`. No statement wiring yet (these are leaf helpers consumed by P2.3/P2.4/P2.5).

**C++:**
- `validateBindingIdentifier` (1008-1045) — strict-mode/`yield`/`await`/`let` checks; returns whether the kind is a valid binding identifier kind. Port the `is_strict_mode()`/`param_yield`/`param_await` reads; intern the relevant idents via `check_unescaped_name`/atom compares. (`yieldIdent_`, `awaitIdent_`, `letIdent_` compares → compare the token's identifier/reserved-word bytes to `b"yield"`/`b"await"`/`b"let"`; for `rw_yield` the kind check at the end returns true for `identifier`/`rw_yield`.)
- `parseBindingIdentifier` (1047-1086) — accepts `identifier` or a reserved word (`tok_->isResWord()`); validates; builds `IdentifierNode(name, type=null, optional=false)`. Skip the `getParseTypes()` `?`/`:` block (P6/P7). Need a `token.is_res_word()` accessor + `token.get_res_word_or_identifier()` (the atom for a keyword's spelling OR an identifier's name) — check the lexer/token API; if absent, add them (faithful to `Token::isResWord`/`getResWordOrIdentifier`).
- `parseBindingPattern` (1281-1296) — dispatch `[`→array, `{`→object.
- `parseArrayBindingPattern` (1298-1360) — elements: elision (`Empty`), `BindingRestElement` (`...`), `BindingElement`; trailing comma; `eat(r_square)`. Skip the type block.
- `parseBindingElement` (1362-1390) — `[`/`{`→pattern else binding-identifier; optional `= Initializer` → `parseBindingInitializer`. Has `CHECK_RECURSION`.
- `parseBindingRestElement` (1392-1413) — `...` + binding-element; error if it has a default initializer (`AssignmentPattern`).
- `parseBindingInitializer` (1415-1432) — `= AssignmentExpression[In]` → `AssignmentPatternNode(left, init)` (uses the 4-arg `set_location_d` with the `=`'s start as debug loc — match C++ `debugLoc`).
- `parseObjectBindingPattern` (1434-1491) — `BindingRestProperty` (`...`) or `BindingProperty`; trailing comma; `eat(r_brace)`. Skip type block.
- `parseBindingProperty` (1493-1561) — `parsePropertyName` (already in `expressions.rs`); `key ":" BindingElement` OR shorthand `SingleNameBinding` (validate the key is a usable binding identifier; clone the key `IdentifierNode`; optional `= Initializer`). Builds `PropertyNode(key, value, kind=init, computed, /*method*/ false, shorthand)`.
- `parseBindingRestProperty` (1563-1589) — `...` + binding-identifier (NOT a pattern, per spec) → `RestElementNode`.

**Files:** `js/statements.rs` (new `impl` methods). Possibly add `token.is_res_word()`/`get_res_word_or_identifier()` to `js/expressions.rs`-adjacent helpers or the lexer's `token.rs` (port faithfully; comment the source).
**Nodes:** `Identifier`, `ArrayPattern`, `ObjectPattern`, `RestElement`, `AssignmentPattern`, `Property`, `Empty`.

- [ ] **Step 1: Unit tests (no corpus yet — these are leaves).** Since binding patterns aren't reachable from a statement until P2.3, drive them with direct unit tests in `statements.rs` `#[cfg(test)]` (or temporarily via `var` once P2.3 lands — but TDD here): write `#[test]`s that construct a parser over `b"[a, , ...b]"` / `b"{a, b: c, d = 1, ...r}"` and call a `pub(crate)`-for-test wrapper around `parse_binding_pattern`, asserting the node shapes (ArrayPattern with Empty hole + RestElement; ObjectPattern with shorthand Property, keyed Property, AssignmentPattern value, RestElement). Mirror the existing destructuring unit tests in `js/mod.rs`.
- [ ] **Step 2: Run** → FAIL (functions don't exist).
- [ ] **Step 3: Implement** all the binding functions above. Reuse `parse_property_name` and `parse_assignment_expression` from P1.
- [ ] **Step 4: Run** → PASS. `cargo build` zero warnings.
- [ ] **Step 5: Commit** `rust(parser): P2.2 binding identifiers & patterns`.

### P2.3 — Variable declarations (`var`/`let`/`const`/`using`/`await using`) + declaration dispatch
**C++:**
- `parseLexicalDeclaration` (1088-1133) — `var`/`const`/`let` + `parseVariableDeclarationList` + `eatSemi` + the const-needs-initializer check + `ensureDestructuringInitialized` → `VariableDeclarationNode(kindIdent, declList)`.
- `parseVariableStatement` (1177-1180) — just `parseLexicalDeclaration(ParamIn)`. Wire `parse_statement`'s `rw_var` arm to call it.
- `parseVariableDeclarationList` (1197-1210) — `do { parseVariableDeclaration } while(eat comma)`.
- `parseVariableDeclaration` (1226-1279) — target = binding-pattern (`[`/`{`, if `allowPattern==Yes`) or binding-identifier; optional `= AssignmentExpression` (4-arg `set_location_d`, debug loc = `=` start) → `VariableDeclaratorNode(init, target)`. `VariableDeclAllowPattern` enum (Yes/No) — port as a Rust enum.
- `ensureDestructuringInitialized` (1212-1224) — error if a pattern declarator has no initializer.
- `parseUsingDeclaration` (1135-1175) — `using`/`await using` + decl list with `VariableDeclAllowPattern::No` + required initializers → `VariableDeclarationNode(identUsing/identAwaitUsing, declList)`.
- `parseDeclaration` (815-877) — dispatch: `rw_function`/async-function → P3 error; `at`/`rw_class` → P3 error; `rw_const`/`let` → `parseLexicalDeclaration(ParamIn)`; `using`/`await using` → `parseUsingDeclaration`; Flow/TS → omit (gated off). Has `CHECK_RECURSION`.
- `checkDeclaration()` (header, JSParserImpl.h ~ the block shown in the plan research) — port faithfully into the existing `check_declaration_start`: `checkN(rw_function, rw_const, rw_class, at)` OR async-function → true; `let` → strict ? true : `lexer.is_let_followed_by_decl_start()`; `using` → `lexer.is_using_followed_by_identifier()`; `await using` (when `param_await`) → `lexer.is_await_using_followed_by_identifier()`. Drop Flow/TS sub-blocks. This REPLACES the P1 approximation in `check_declaration_start` (which always-flagged `let`).
- `parseStatementListItem` (879-946) — replace the P1 stub: `if checkDeclaration() { parseDeclaration }` else import-lookahead/import/export (deferred per policy) else `parseStatement`. The `async`-function path inside `checkDeclaration` routes into `parseDeclaration` → P3 error.

**Files:** `js/statements.rs`. Update `check_declaration_start` → faithful `check_declaration`; rewrite `parse_statement_list_item`; add the var/decl functions; wire `parse_statement` `rw_var` arm.
**Nodes:** `VariableDeclaration`, `VariableDeclarator`, plus P2.2 patterns.

- [ ] **Step 1: Corpus + unit tests** (verify each with hermesc first):
  - `decl_var.js`: `var x; var a = 1, b; var [p, q] = r; var {m, n} = o;`
  - `decl_let_const.js`: `let x = 1; const y = 2; let [a] = b; const {c} = d;`
  - `decl_using.js`: `using x = res;` and `await using` only inside an async context (defer — `await using` at top level needs `param_await`; **do not corpus it**, unit-test the `using` form; mark `await using` for a P3 corpus). Verify `using x = y;` dumps under hermesc (it's `VariableDeclaration` kind "using"). If hermesc rejects top-level `using`, drop this file and unit-test only.
  - `decl_let_loose.js`: sloppy-mode `let` as identifier vs declaration — e.g. `let;` (identifier expr-stmt) vs `let x;` (declaration). This exercises the `isLetFollowedByDeclStart` disambiguation. Verify hermesc output for `let;\nlet x;`.
  Run differential → FAIL.
- [ ] **Step 2: Implement.** Port the functions; replace `check_declaration_start`'s P1 `let` approximation with the real `is_let_followed_by_decl_start` lookahead (removes a P1 carry-forward). Add the `VariableDeclAllowPattern` enum.
- [ ] **Step 3:** Build ast-dump; differential matches. `cargo test -p parser` (unit: `var [a]=b` → VariableDeclaration with ArrayPattern declarator; `const x;` → "missing initializer in const declaration" error; `var [a];` → "destructuring declaration must be initialized").
- [ ] **Step 4:** `cargo build` zero warnings.
- [ ] **Step 5: Commit** `rust(parser): P2.3 var/let/const/using declarations + declaration dispatch`.

### P2.4 — Block, if/while/do-while, switch, try/catch/finally + `parseStatementList` multi-until
**C++:**
- `parseStatementList` (948-971) — **change the signature to support multiple `until` tokens.** C++ is `template<typename...Tail> parseStatementList(param, until, parseDirectives, allowImportExport, stmtList, Tail...otherUntil)` with `checkN(until, otherUntil...)`. Port as a **const generic**: `fn parse_statement_list<const N: usize>(&mut self, param, until: [TokenKind; N], parse_directives, allow_import_export, stmt_list) -> bool` and the loop condition `!self.check(eof) && !until.contains(&self.cur_kind())`. Update the existing callers (`parse_program`, `parse_block`) to pass `[TokenKind::eof]`/`[TokenKind::r_brace]`. This preserves the C++ monomorphization (one instantiation per arity) — do NOT use a runtime `&[TokenKind]`. (Resolves the P1 carry-forward "single `until` grows to 2–3 for switch-case in P2".)
- `parseBlock` (973-1006) — `{` + `parseStatementList(param, [r_brace], parseDirectives, No, …)` + `eat(r_brace)` → `BlockStatementNode(body, /*implicit*/ false)`. Wire `parse_statement`'s `l_brace` arm. (Note the existing P1 `parse_block` honest error is removed.)
- `parseIfStatement` (1679-1762) — `if ( Expression ) Stmt [else Stmt]`. The `parseStatementOrFunctionDeclaration` lambda: the `rw_function` branch → P3 honest error (`return None`); the normal-statement branch is real. → `IfStatementNode(test, consequent, alternate?)`.
- `parseWhileStatement` (1764-1796) → `WhileStatementNode(body, test)` (NB field order: body then test in the node ctor — verify against `node.rs`).
- `parseDoWhileStatement` (1798-1841) → `DoWhileStatementNode(body, test)`; uses `eat_semi(true)`.
- `parseSwitchStatement` (2220-2340) — `switch ( Expr ) { (case Expr : StmtList | default : StmtList)* }`; the duplicate-`default` error + `ignoreClause` recovery; each clause's `parseStatementList(param.get(Return), [rw_default, rw_case, r_brace], false, No, stmtList)` (the 3-until instantiation) → `SwitchCaseNode(test?, consequent)` + `SwitchStatementNode(discriminant, cases)`.
- `parseTryStatement` (2366-2465) — `try Block [catch [( BindingPattern|BindingIdentifier )] Block] [finally Block]`; at least one handler required → `CatchClauseNode(param?, body)` + `TryStatementNode(block, handler?, finalizer?)`. Uses `parse_binding_pattern`/`parse_binding_identifier` (P2.2).
- Wire `parse_statement` arms: `l_brace`→block, `rw_if`→if, `rw_while`→while, `rw_do`→do-while, `rw_switch`→switch, `rw_try`→try.

**Files:** `js/statements.rs` (+ `parse_statement_list` signature change touches `js/mod.rs` callers — actually `parse_statement_list` lives in `statements.rs`; only `parse_program` in `mod.rs` calls it). Add `error_expected2` helper to `js/mod.rs`.
**Nodes:** `BlockStatement`, `IfStatement`, `WhileStatement`, `DoWhileStatement`, `SwitchStatement`, `SwitchCase`, `TryStatement`, `CatchClause`.

- [ ] **Step 1: Corpus** (verify with hermesc first):
  - `stmt_block.js`: `{ } { var x; x; } { ; }`
  - `stmt_if.js`: `if (x) y; if (a) b; else c; if(p){q}else{r}`
  - `stmt_while.js`: `while (x) y; while(a){b;}`
  - `stmt_do.js`: `do x; while (y); do { a } while (b)`
  - `stmt_switch.js`: `switch (x) { case 1: a; break; case 2: default: c; }`
  - `stmt_try.js`: `try { a } catch (e) { b } finally { c } try{x}catch{y} try{z}finally{w}`
  - Update `stmt_labelled.js` from P2.1 to add the now-valid `outer: while(0) x;`.
  Run differential → FAIL.
- [ ] **Step 2: Implement.** Change `parse_statement_list` to the const-generic `[TokenKind; N]` form; update callers; add the statement functions; wire the `parse_statement` arms.
- [ ] **Step 3:** ast-dump + differential match. `cargo test -p parser` (unit: nested block; if/else attaches to nearest if; switch with two `default` reports the duplicate error; try with neither catch nor finally errors).
- [ ] **Step 4:** `cargo build` zero warnings.
- [ ] **Step 5: Commit** `rust(parser): P2.4 block/if/while/do/switch/try + multi-until statement list`.

### P2.5 — `for` / `for-in` / `for-of` (incl. `using` in for, destructuring reparse)
The most complex single function. Port `parseForStatement` (1843-2093) faithfully.

**C++:** `parseForStatement` (1843-2093):
- `for [await] (` prologue; `await` only when `param_await` (won't fire in P2 corpus, but port the structure).
- Init head: `var`/`const`/`let` → `parseVariableDeclarationList` → `VariableDeclarationNode`; `using Identifier` (via `lexer.is_using_followed_by_identifier`, with the `using of` exception building an `IdentifierNode`); `await using Identifier` (via `lexer.is_await_using_followed_by_identifier`); else an `Expression` (or `LeftHandSideExpression` when `await`) or empty.
- Branch on `rw_in` / `of` (`check_unescaped_name(b"of")`):
  - for-in/for-of: the "only one binding" error; destructuring reparse of `expr1` if it's an Array/Object expression (`reparse_assignment_pattern` from P1.8b); `ForInStatementNode(left, right, body)` / `ForOfStatementNode(left, right, body, await)`. `right` is `parseExpression()` for in, `parseAssignmentExpression(ParamIn)` for of.
  - C-style `;` `;`: `await` error if present; `ensureDestructuringInitialized(decl)`; optional test `;` optional update `)` → `ForStatementNode(init?, test?, update?, body)`.
  - else `errorExpected(semi, rw_in, …)`.
- Wire `parse_statement`'s `rw_for` arm.

**Files:** `js/statements.rs`. (`reparse_assignment_pattern` already exists in `expressions.rs` from P1.8b; confirm its visibility is `pub(super)` — if not, widen it.)
**Nodes:** `ForStatement`, `ForInStatement`, `ForOfStatement`, `VariableDeclaration`, `VariableDeclarator`, `Identifier`.

- [ ] **Step 1: Corpus** (verify with hermesc first):
  - `stmt_for.js`: `for (;;) x; for (var i=0; i<10; i++) y; for (let a=1,b=2; ; ) z; for (a; b; c) d; for (i=0;;) ;`
  - `stmt_for_in.js`: `for (var k in o) x; for (a in b) y; for (let p in q) r; for ([a,b] in c) d; for ({x} in y) z;`
  - `stmt_for_of.js`: `for (var v of it) x; for (a of b) y; for (const [c] of d) e; for ({f} of g) h;`
  - `stmt_for_using.js`: `for (using r of res) x;` — verify hermesc accepts (kind "using"); if rejected at top level, drop and unit-test.
  Run differential → FAIL.
- [ ] **Step 2: Implement** `parse_for_statement`; add the `VariableDeclaration` head construction, the for-in/of reparse, and the C-style path; wire the `rw_for` arm. Port the `using`/`await using` head branches faithfully (guard `await using` behind `param_await`).
- [ ] **Step 3:** ast-dump + differential match. `cargo test -p parser` (unit: `for(a in b)` left is Identifier; `for([a] of b)` left is ArrayPattern; `for(var a,b in c)` reports "Only one binding…"; `for(;;)` empty head/test/update are null).
- [ ] **Step 4:** `cargo build` zero warnings.
- [ ] **Step 5: Commit** `rust(parser): P2.5 for / for-in / for-of statements`.

---

## P2 capstone (after all five sub-tasks)
Run the **whole-component capstone review** per SESSION-HANDOFF §5.7:
- **Structural-fidelity grep:** `grep -n "template <" lib/Parser/JSParserImpl.cpp` over the P2 line ranges (669-2479). The only template is `parseStatementList<typename...Tail>` — confirm it became the `const N: usize` generic (NOT a runtime `&[TokenKind]`). Confirm no other silent template→runtime / RAII→explicit-beyond-agreed-list / layout deviations.
- **Re-derive the deferral set:** every `parseFunctionDeclaration`/`parseClassDeclaration`/`parseImportDeclaration`/`parseExportDeclaration`/Flow/TS reference in the P2 range routes to an honest error with a test (P3/P4/P6/P7), no silent fallthrough.
- **Carry-forwards resolved:** the P1 `parse_statement_list` single-`until` and the `let`-in-sloppy `isLetFollowedByDeclStart` are both done in P2.4/P2.3 — verify and strike them from the roadmap.
- Full `cargo test --manifest-path rust/Cargo.toml` (whole workspace) green; `REQUIRE_DIFFERENTIAL=1` differential green over the expanded corpus; zero warnings; `REQUIRE_GEN=1 … generated_idempotent` still green (no AST regen needed — all nodes pre-existed).
- Update `doc/superpowers/RustPortRoadmap.md` (P2 DONE block + remaining-carry-forward list) and `SESSION-HANDOFF.md` (NEXT: P3 — functions, classes, arrow, async, generators, `super`, `yield`).

## Self-review notes (author)
- **Spec coverage:** every `parse*` in the 669-2479 range is assigned to a sub-task (P2.1 simple stmts/labelled; P2.2 binding; P2.3 var/decl; P2.4 block/if/while/do/switch/try; P2.5 for). `parseFunctionBody`/`parseDeclaration` function/class branches are explicitly P3 honest-errors.
- **Type consistency:** node ctor field order (e.g. `WhileStatement(body, test)` vs `(test, body)`) MUST be read from `rust/crates/ast/src/node.rs` at implement time — the C++ ctor arg order is the source of truth and the generated Rust `new` mirrors `ESTree.def` field order; verify per node (the dumper walks `.def` order, so a swapped pair would still dump under the right key only if the field names match — double-check `For*`/`While`/`DoWhile` whose ctor arg order differs from dump order in C++).
- **Corpus validity:** `return` at top level is invalid (unit-test only); `break`/`continue`/`with`/`using` validity under `hermesc -dump-ast` must be confirmed per file before committing (the plan flags each uncertain one to drop-to-unit-test if hermesc rejects it).
