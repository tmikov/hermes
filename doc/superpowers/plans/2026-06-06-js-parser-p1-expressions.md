# JS Parser — Phase P1 (Core Expressions) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) tracking. The C++ in `lib/Parser/JSParserImpl.cpp` IS the spec — port it faithfully from the cited line ranges; the byte-for-byte `parser_differential` vs `hermesc -dump-ast` is the gate.

**Goal:** Parse the full JavaScript *expression* grammar (value expressions) wrapped in expression statements, so programs of expressions dump byte-identically to `hermesc -dump-ast`.

**Architecture:** Add `impl<'gc,'ast,'ctx,'a> JSParserImpl<...>` methods across new sibling files under `rust/crates/parser/src/js/` (`expressions.rs`, plus a minimal statement spine in `statements.rs`), mirroring the C++ recursive-descent operator-precedence chain. Each node is built with the `ast` `new` constructors + `set_location`. Returns `Option<&'gc Node<'gc>>` (`None` = error already reported).

**Tech Stack:** the `ast` + `parser` crates; `hermesc` (`cmake-build-asan/bin/hermesc`) as the differential oracle.

**Spec:** `doc/superpowers/specs/2026-06-06-js-parser-design.md`. **Builds on P0** (`plans/2026-06-06-js-parser-p0-foundations.md`).

## Conventions (carry over from P0 — do not relitigate)
- Faithful port; keep C++ structure + comments; `Option<T>`/`None`=error-reported with `?` propagation; C++ `template`s → Rust generics (none in P1 except the variadic `checkN` already covered; keep `parseStatementList<Tail...>` generic when it lands in P2 — N/A here); RAII → explicit. Zero `cargo build` warnings. Commit directly to `rust`; trailer `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`.
- **Node construction:** use `ast::node::<Name>::new(metadata, ...)` wrapped in `Node::<Name>(...)`, then `set_location(start, end, node)`. Look up the exact generated constructor signature + field order in `rust/crates/ast/src/node.rs` (snake_case fields; the `new` defaults decorations/`Cell`s). The C++ uses `setLocation(a, b, new (context_) FooNode(args))`; the Rust analog passes the same start/end and the same child args.
- **`set_location` overloads:** P0 has `set_location(start: SMLoc, end: SMLoc, node)`. C++ `setLocation` accepts tokens/nodes/ranges for start/end. Add small Rust helpers as needed (e.g. accept `&Node` or `SMRange` for start/end) — port the overloads from `JSParserImpl.h:368-414` faithfully (a node's start/end come from `node.range()`; a token's from `token.start_loc()/end_loc()`). Keep them in `js/mod.rs`.
- **`CHECK_RECURSION`:** the C++ macro increments `recursionDepth_` on entry and decrements on scope exit, erroring if exceeded. Port as an explicit RAII guard (a `RecursionGuard` struct with `Drop`, or an explicit inc/`recursion_depth_check`/dec) used at the top of recursive parse fns. Add it once in `js/mod.rs` and use it where the C++ has `CHECK_RECURSION`.

## Deferral policy (IMPORTANT — honest stubs, not silent fallthrough)
P1 implements **value expressions only**. These productions are DEFERRED to later phases; where a C++ branch reaches one, emit an explicit `self.error_cur("<thing> not yet supported (parser phase <PN>)")` and return `None` (NOT a silent fallthrough), and cover it with a test asserting the error. Track them in the roadmap.
- **Function/async-function/class expressions** (`parsePrimaryExpression` cases `rw_function`, `async`-function, `at`/`rw_class`) → P3 (functions & classes).
- **Arrow functions** + arrow-cover paren forms (`()` `CoverEmptyArgsNode`, `(...rest)` `CoverRestElementNode`) and the `=>` detection in `parseAssignmentExpression` → P3.
- **Object methods / get / set / async / generator methods** in `parsePropertyAssignment` → P3 (P1 object literals do DATA properties, shorthand, computed-key data, spread only).
- **`yield` expressions** → unreachable at program top level (`paramYield_` false) until generator functions exist (P3); port the `parseAssignmentExpression` yield branch guarded by `param_yield` (it simply won't fire in P1 corpus).
- **JSX** (`less` case) / **Flow match / CoverTypedIdentifier / TypeCast** → already `context_.getParseFlow()/JSX()`-gated; leave the gates but the bodies call P5/P6 functions — for P1, gate them behind the context flags (off in P1 corpus) and `self.error_cur(... "pass -parse-jsx/-parse-flow")` faithfully where C++ does, OR leave a `// P5/P6` stub returning an error. The P1 differential corpus uses plain JS (no `-parse-*`), so these never fire.
- **Private names** (`parsePrivateName`, `#x in obj`) → reachable only via member access / `in`; port `#x` in member select (P1.6) since it's a value-expression form; if it pulls in class-field semantics, defer with an error + test.

## Validation (every sub-task)
Extend `rust/crates/parser/tests/parser_corpus/` with `*.js` files exercising the new forms, then:
```bash
cargo build --manifest-path rust/Cargo.toml -p parser --bin ast-dump
REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p parser --test parser_differential -- --nocapture
cargo test  --manifest-path rust/Cargo.toml -p parser    # unit/ported tests
cargo build --manifest-path rust/Cargo.toml              # ZERO warnings
```
Add corpus files **with `-dump-source-location=both`-relevant content**; the harness already diffs byte-for-byte. For each new construct, confirm `hermesc -dump-ast -dump-source-location=both <file>` succeeds (valid JS) before adding it.

---

## Sub-tasks (ordered; each = one implementer + spec review + quality review + commit)

### P1.1 — Expression-statement spine + operator-chain skeleton + primary literals
**C++:** `parseStatementList` (949-971), `parseStatementListItem` (879-947 — P1: declarations deferred, so it falls through to `parseStatement`), `parseStatement` (669-738 — P1: only `EmptyStatement` + the expression-statement default; all other statement kinds → error "… (parser phase P2)"), `parseExpressionOrLabelledStatement` (1600-1677 — P1: expression-statement path only; labelled → P2 error), `parseEmptyStatement` (1591-1598), `parseExpression` (6552-6609 — incl. comma `SequenceExpression`), `parseAssignmentExpression` (6233-6551 — P1.1: implement as pass-through to `parseConditionalExpression`; assignment ops in P1.5; yield/arrow deferred), `parseConditionalExpression` (4477 — P1.1 pass-through to binary), `parseBinaryExpression` (4262 — P1.1 pass-through to unary), `parseUnaryExpression` (4112 — P1.1 pass-through to postfix), `parsePostfixExpression` (4091 — pass-through to LHS), `parseLeftHandSideExpression`/`Tail` (4014-4089 — P1.1 pass-through to `parseNewExpressionOrOptionalExpression` → `parseOptionalExpressionExceptNew` → `parsePrimaryExpression`, with call/member tails deferred to P1.6: implement the chain to reach primary but skip the member/call tail loop until P1.6), `parsePrimaryExpression` (2481-2709 — literals subset: `rw_this`, `identifier` (plain), `rw_null`, `rw_true`/`rw_false`, `numeric_literal`, `bigint_literal`, `string_literal`, `l_paren` simple `(expr)` with `incParens`; defer function/class/array/object/template/regexp to later sub-tasks; defer async/arrow/JSX/Flow per policy).
**Nodes:** `ExpressionStatement`, `EmptyStatement`, `Program` (already), `ThisExpression`, `Identifier`, `NullLiteral`, `BooleanLiteral`, `NumericLiteral`, `BigIntLiteral`, `StringLiteral`, `SequenceExpression`. **`incParens`:** C++ `expr->incParens()` → set/bump the node's `metadata.parens` Cell (0/1/2-capped) — port `Node::incParens` semantics (ESTree.h) as a helper.
**Files:** create `js/statements.rs` + `js/expressions.rs` (both `impl` blocks; add `mod statements; mod expressions;` to `js/mod.rs`). Move `parse_program` to call `parse_statement_list`.
**Corpus:** `expr_literals.js` (`42; "hi"; true; false; null; this; x;`), `expr_paren.js` (`(1); ((a));`), `expr_sequence.js` (`1, 2, 3;`), `empty_stmt.js` (`;;;`). Plus a unit test asserting a deferred kind (e.g. `if (x);`) errors.

### P1.2 — Binary expression (precedence table)
**C++:** `parseBinaryExpression` (4262-4475) — the precedence-climbing/stack algorithm + `BinaryExpression` vs `LogicalExpression` (`&&`/`||`/`??`) split + `getBinOpPrecedence`/`isLeftAssoc` (from `JSLexer`/`TokenKinds.def`). Port the stack machine faithfully (it avoids recursion depth). Handle `ParamIn` (the `in` operator gating).
**Nodes:** `BinaryExpression`, `LogicalExpression`.
**Corpus:** `expr_binary.js` (`1+2*3-4/5%6; a<<b>>c; a&b|c^d; a==b!=c; a<b>c<=d; a instanceof b; a&&b||c; a??b;`), `expr_in.js` (`a in b;`).

### P1.3 — Unary, update (postfix/prefix) + convertIdentOpIfPossible
**C++:** `parseUnaryExpression` (4112-4260), `parsePostfixExpression` (4091-4110), `convertIdentOpIfPossible` (search; converts `of`/`as` etc. — but for unary it's the `delete/void/typeof/+/-/~/!/await` + `++`/`--`). `UnaryExpression` (prefix `delete void typeof + - ~ !`), `UpdateExpression` (prefix/postfix `++`/`--`). `await` unary → guarded by `param_await` (unreachable in P1 top level; port the branch). 
**Nodes:** `UnaryExpression`, `UpdateExpression`.
**Corpus:** `expr_unary.js` (`-a; +a; !a; ~a; typeof a; void a; delete a;`), `expr_update.js` (`++a; --a; a++; a--;`).

### P1.4 — Conditional (?:)
**C++:** `parseConditionalExpression` (4477-4650) — P1: the plain `test ? consequent : alternate` path; the Flow `CoverTypedParameters`/optional-`?` typed-param cover is `context_.getParseFlow()`-gated (off in P1).
**Nodes:** `ConditionalExpression`.
**Corpus:** `expr_conditional.js` (`a ? b : c; a ? b ? c : d : e;`).

### P1.5 — Assignment operators + sequence completion
**C++:** `parseAssignmentExpression` (6233-6551) — fill in the assignment-operator path (`= += -= *= /= %= <<= >>= >>>= &= |= ^= **= &&= ||= ??=`), `checkAssign`, the LHS-validity/`reparseAssignmentPattern` for destructuring-assignment targets (`[a]=b`, `{a}=b` → `reparseAssignmentPattern` 5913 + `reparseArrayAsignmentPattern` 5990 + `reparseObjectAssignmentPattern` 6054). Yield branch (guarded `param_yield`) + arrow detection → DEFER arrow (`=>` → error "… (parser phase P3)"); yield won't fire.
**Nodes:** `AssignmentExpression`, and `ArrayPattern`/`ObjectPattern`/`AssignmentPattern`/`Property`/`RestElement` for assignment-target reparse.
**Corpus:** `expr_assign.js` (`a=1; a+=1; a-=2; a*=3; a**=2; a<<=1; a&&=b; a||=b; a??=b;`), `expr_assign_destructure.js` (`[a,b]=c; ({a,b}={}); [a,...b]=c;`).

### P1.6 — Member / call / new / optional chaining
**C++:** `parseLeftHandSideExpression`/`Tail` (4014-4089), `parseMemberSelect` (3649-3793), `parseCallExpression` (3795-3918), `parseArguments` (3594-3647), `parseNewExpressionOrOptionalExpression` (3920-4012), `parseOptionalExpressionExceptNew`/`_tail` (3424-3592), `parsePrivateName` (1182-1195) for `a.#x`. Includes `new.target` (meta-property), `?.`/`?.[`/`?.(` optional chaining (`OptionalMemberExpression`/`OptionalCallExpression` + the `ChainExpression` wrapping if used — check ESTree.def), spread args in calls.
**Nodes:** `MemberExpression`, `CallExpression`, `NewExpression`, `OptionalMemberExpression`, `OptionalCallExpression`, `MetaProperty`, `PrivateName`, `SpreadElement`, (`ChainExpression`?).
**Corpus:** `expr_member.js` (`a.b.c; a[b][c]; a.b[c].d;`), `expr_call.js` (`f(); f(a,b); f(...a); a.b(c); f()();`), `expr_new.js` (`new A; new A(); new A(x); new a.B(x); new.target;`), `expr_optional.js` (`a?.b; a?.[b]; a?.(b); a?.b.c; a?.b().c;`), `expr_private.js` (`a.#x;` — if class-field-dependent, defer with test).

### P1.7 — Array literal + spread/elision
**C++:** `parseArrayLiteral` (2711-2763), `parseSpreadElement` (2815-2827).
**Nodes:** `ArrayExpression`, `SpreadElement` (elisions = holes/null elements — match C++).
**Corpus:** `expr_array.js` (`[]; [1,2,3]; [1,,3]; [,,]; [...a]; [1,...a,2];`).

### P1.8 — Object literal (data subset)
**C++:** `parseObjectLiteral` (2792-2813), `parseObjectProperties` (2765-2790), `parsePropertyAssignment` (2829-... — P1: DATA props `key: value`, shorthand `{a}`, computed `[k]: v`, spread `...x`; DEFER methods/`get`/`set`/`async`/`*` generator → error "… (parser phase P3)"), `parsePropertyName` (3268-3340).
**Nodes:** `ObjectExpression`, `Property`, `SpreadElement`.
**Corpus:** `expr_object.js` (`({}); ({a:1, b:2}); ({a, b}); ({[x]:1}); ({...a}); ({"s":1, 0:2});`).

### P1.9 — Template literals (tagged + untagged)
**C++:** `parseTemplateLiteral` (3342-3422), and the tagged form via the member/call tail (`parseLeftHandSideExpressionTail` recognizing a template after a member expr → `TaggedTemplateExpression`).
**Nodes:** `TemplateLiteral`, `TemplateElement`, `TaggedTemplateExpression`.
**Corpus:** `expr_template.js` (`` `hello`; `a${1}b${2}c`; `${x}`; ``), `expr_tagged_template.js` (`` tag`x`; a.b`y${1}`; ``).

### P1.10 — RegExp literal in primary
**C++:** `parsePrimaryExpression` `regexp_literal` case (2573-2582). (The lexer already produces regexp tokens in `AllowRegExp` context.)
**Nodes:** `RegExpLiteral`.
**Corpus:** `expr_regexp.js` (`/abc/; /a.c/gi; x = /re/;`).

### P1.11 — Capstone: roadmap update + whole-P1 review
- Update `doc/superpowers/RustPortRoadmap.md` (P1 done; next P2 statements).
- Whole-phase review (build/test/differential green; structural-fidelity grep `template <` over the ported ranges; confirm every deferral emits an honest error covered by a test; confirm no node's `incParens`/locations drift from hermesc).

---

## Self-review (run after the plan, before executing)
- **Coverage:** every value-expression production in `parsePrimaryExpression`/the operator chain maps to a sub-task; every non-value production (function/class/arrow/yield/JSX/Flow/object-methods) has an explicit deferral with an error+test. ✓
- **Ordering:** the chain is built top-skeleton-first (P1.1) then filled level-by-level so each sub-task keeps the differential green. ✓
- **No silent stubs:** deferral policy mandates `error_cur(... "phase PN")` + a test, per the capstone lesson from the lexer. ✓
