# C++ defects found by the Rust front-end port

**Date:** 2026-08-08 (repros re-verified this date). **Discovered by:** the port's
byte-for-byte differential validation (S2–S4a sweeps, reviews, and the whole-Sema
capstone). Every item below was reproduced against real binaries; commands and
observed output are verbatim.

**Build config for repros:** `cmake-build-asan` (Debug + ASan, the repo's default
dev build — `CLAUDE.md`). Items 1–5 and 11 are assertion failures, visible only
in assert-enabled builds; Release builds pass through them (with the noted
behavior).
Items 2–3 use the in-repo oracle tool `tools/sema-parser-dump/` (build:
`cmake --build cmake-build-asan --target sema-parser-dump`), which drives the
public `resolveASTForParser` + `semDump` entries — the crashes are in those
library functions, not in the tool.

**Port coordination:** the Rust port preserves each defect bug-for-bug with an
explicit pin that *asserts the buggy behavior* (so silent drift is caught). If a
defect is fixed in C++, flip the corresponding pin (named per item) in the same
change.

**Update (2026-08-10):** all 11 items were fixed upstream on the 2026-08-08
branch, then cherry-picked into this branch (11 upstream commits, Task 1 of
`doc/superpowers/plans/2026-08-10-cpp-defect-fixes-propagation.md`) and
mirrored into the Rust port across four Rust tasks the same day (parser:
JSX/match/JSON; resolver: promoter/dead-code/async-export/export-wording;
resolver: `$SHBuiltin`/field-init scope; dumper + stable-sort note
retirement). Every pin below is flipped; see each item's `Fixed upstream`/
`Pin flipped` lines and the summary table. This document is kept as the
historical record of what the port's own differential testing found — do not
delete the original analyses.

---

## 1. `using` declaration + block-nested function aborts the promoter

**Severity:** debug-abort on valid-looking input (the file has a real error, but
hermesc should report it, not abort).

```bash
printf 'using x = 1;\n{ function f(){} }\n' > /tmp/bug1.js
cmake-build-asan/bin/hermesc -dump-sema /tmp/bug1.js
# => Assertion `varDeclaration->_kind == resolver_.keywords().identVar' failed.
#    SIGABRT, exit 134
```

**Root cause:** `ScopedFunctionPromoter::extractDeclaredIdents`
(`lib/Sema/ScopedFunctionPromoter.cpp:255-262`) asserts a `VariableDeclaration`'s
kind is `let`/`const`/`var`, but the promoter runs (from
`SemanticResolver::visit(ProgramNode*)`, cpp:224-227) *before* the resolver visit
that rejects `using`, so a `using` declaration reaches the assert. Any loose-mode
scope containing both a `using` declaration and a block-scoped function trips it.

**Port pin:** `rust/crates/sema/src/resolver/promoter.rs` mirrors with a
`debug_assert!` (release treats it as `Var`, matching Release C++).

**Fixed upstream:** `4ad67c992` (2026-08-08 branch), cherry-picked to `rust`
2026-08-10 as `e4408f849`.
**Pin flipped:** `promoter.rs`'s `extract_declared_idents` no longer asserts
`kind == ident_var`; it now branches `var`/`let`/else-lexical (matching the
C++ fix's own branch order), so `using`/`await using` return `Const` instead
of aborting. Live corpus pin: `using-scoped-fn-promotion.js`. Rust mirror
commit `044b815d1` (Task 3 of the propagation plan).

---

## 2. Anonymous `export default function` + `semDump` → null-pointer cast abort

**Severity:** debug-abort in the dumper; affects any caller combining
`resolveASTForParser` (`compile = false` — the `hermes-parser` entry) with
`semDump`.

```bash
cmake --build cmake-build-asan --target sema-parser-dump
printf 'export default function () {}\n' > /tmp/bug2.js
cmake-build-asan/bin/sema-parser-dump /tmp/bug2.js
# => Assertion `Val && "isa<> used on a null pointer"' failed.   (llvh Casting.h:106)
#    SIGABRT, exit 134
```

**Root cause:** under `compile = false` the anonymous-default-export rewrite
(item 6) does not run, so a hoisted `FunctionDeclaration` with `_id == nullptr`
lands on `LexicalScope::hoistedFunctions`; `SemContextDumper::printScope`
(`lib/Sema/SemContext.cpp:493-494`) does an unconditional
`llvh::cast<ESTree::IdentifierNode>(fd->_id)`. The driver's `-dump-sema` path
can't reach this (it runs `compile = true`, and plain-mode export errors suppress
the dump), which is why it survived.

**Port pin:** `rust/crates/sema/src/dump_context.rs:304` panics identically
("a hoisted FunctionDeclaration always has an id"); the parser-corpus MANIFEST
carries the landmine row.

**Fixed upstream:** `918158cb0` (2026-08-08 branch), cherry-picked to `rust`
2026-08-10 as `179fb8ca3`.
**Pin flipped:** `dump_context.rs`'s `print_scope` no longer `.expect()`s a
hoisted `FunctionDeclaration`'s id; on `None` it now prints `*default*`,
matching the C++ fix's `SemContext.cpp:493-501`. Live corpus pin (previously
excluded, now imported): `sema_corpus_parser/anon-export-default.js`. Rust
mirror commit `400f108ae` (Task 5 of the propagation plan).

---

## 3. `with(o){x;}` + `resolveASTForParser` + `semDump` → unresolvable-decl assert

**Severity:** debug-abort in the dumper (Release prints ` UNR` and continues).

```bash
printf 'with(o){x;}\n' > /tmp/bug3.js
cmake-build-asan/bin/sema-parser-dump /tmp/bug3.js
# => Assertion `!node->isUnresolvable() && "Attempt to read decl for
#    unresolvable identifier"' failed.   (SemContext.h:559)
#    SIGABRT, exit 134
```

**Root cause:** the `Unresolver` (`lib/Sema/SemanticResolver.cpp:3192-3206`)
stores a null expression decl and sets `unresolvable`; the dumper's
`enter(IdentifierNode*)` (`lib/Sema/SemResolve.cpp:96-102`) then calls
`getExpressionDecl` (`SemContext.h:557`), whose debug assert at `:559` rejects
unresolvable identifiers. Release C++ returns null and the dumper prints ` UNR`
(SemResolve.cpp:121-122). Unreachable via the driver (`with` errors under
`compile = true`, suppressing the dump); reachable via the parser entry.

**Port pin:** the Rust dumper deliberately reproduces the *Release* behavior;
deviation argued at `rust/crates/sema/src/dump.rs:82-101`.

**Fixed upstream:** `918158cb0` (2026-08-08 branch), cherry-picked to `rust`
2026-08-10 as `179fb8ca3` — same commit as item 2 (two hunks, one file).
**Pin flipped:** no Rust code change was needed (`dump.rs`'s
`enter_identifier` already checked `unresolvable` before calling
`get_expression_decl`); what changed is that the deviation is now
**retired** — `918158cb0` taught the C++ dumper the same guard
(`SemResolve.cpp:99-106`), so debug C++ now matches release C++ matches this
port, and the "deliberately reproduces Release behavior" argument at
`dump.rs:82-101` is rewritten to record the guard as *matching* upstream,
not diverging from it. Live corpus pin (previously excluded, now imported):
`sema_corpus_parser/with-statement.js`. Rust mirror commit `400f108ae`
(Task 5).

---

## 4. `class C { x = class {}; }` aborts the sema dumper

**Severity:** debug-abort on valid input, stock driver path.

```bash
printf 'class C { x = class {}; }\n' > /tmp/bug4.js
cmake-build-asan/bin/hermesc -dump-sema /tmp/bug4.js
# => Assertion `processedCount == f.getScopes().size() && "not all scopes were
#    visited"' failed.   (SemContext.cpp:478)
#    SIGABRT, exit 134
```

**Root cause (from the S2 analysis, `tests/sema_corpus/MANIFEST.md`):** the inner
class expression's `LexicalScope` is created with `parentFunction =` the synthetic
field-initializer `FunctionInfo` but `parentScope = curScope_`, so the dumper's
scope walk over the initializer function never reaches it and the
processed-count invariant fails.

**Port pin:** shape excluded from the corpus; documented in the MANIFEST landmine
list (S2). The Rust side reproduces the mismatch class via its own
`debug_assert_eq!` at `rust/crates/sema/src/dump_context.rs:241` (see also the
`--release` masking note in the MANIFEST).

**Fixed upstream:** `b351e1184` (2026-08-08 branch), cherry-picked to `rust`
2026-08-10 as `48d221fb2`.
**Pin flipped:** `resolver/classes.rs`'s `visit_class_private_property` and
`visit_class_property` now save/restore `cur_scope` to the initializer
function's body scope (matching the C++ fix's `SaveAndRestore`) around the
value visit, so a class expression in a field initializer's scope is created
under the initializer function, not the enclosing class. Live corpus pin
(previously excluded, now imported): `class-field-class-expr.js`; also
un-broke the pre-existing upstream sweep witness
`test/hermes/computed-fn-name.js` (moved from the sweep's panic bucket to
byte-identical). Rust mirror commit `7f8fd8f17` (Task 4).

---

## 5. `$SHBuiltin.#x()` aborts on a `PrivateName` property

**Severity:** debug-abort on (invalid) input, stock driver path.

```bash
printf 'class C { #x; m() { $SHBuiltin.#x(); } }\n' > /tmp/bug5.js
cmake-build-asan/bin/hermesc -dump-sema /tmp/bug5.js
# => Assertion `isa<X>(Val) && "cast<Ty>() argument of incompatible type!"' failed.
#    SIGABRT, exit 134
```

**Root cause:** `lib/Sema/SemanticResolver.cpp:1166-1167` does
`llvh::cast<IdentifierNode>(methodCallee->_property)`, but a non-computed member
expression's property may be a `PrivateName`.

**Port pin:** `calls.rs`'s `sh_builtin_property_name` reproduces the failing cast
as an explicit panic (documented in the MANIFEST, S2 T6).

**Fixed upstream:** `07efab88d` (2026-08-08 branch), cherry-picked to `rust`
2026-08-10 as `416aafcd2`.
**Pin flipped:** `sh_builtin_property_name` now returns `Option<Atom>`
instead of panicking (`Node::Identifier(id) => Some(...)`, `_ => None`), and
the call site hoists the check into the recognition condition (`&&
prop_ident.is_some()`), mirroring the C++ fix's `dyn_cast` restructuring —
so `$SHBuiltin.#x()` now falls through to the pre-existing `invalid use of
$SHBuiltin` diagnostic instead of asserting. Live corpus pin (previously
excluded, now imported): `shbuiltin-private-name.js`. Rust mirror commit
`7f8fd8f17` (Task 4).

---

## 6. Anonymous `export default async function` loses `async` — user-visible

**Severity:** **semantic bug on valid input.** Valid ES-module code is rejected
(or, without `await` in the body, silently compiled as a *sync* function) under
CommonJS mode. Release builds affected too — this is not an assert hole.

```bash
printf 'export default async function () { await 0; }\n' > /tmp/bug6a.js
cmake-build-asan/bin/hermesc -commonjs -dump-sema /tmp/bug6a.js
# => /tmp/bug6a.js:1:36: error: 'await' not in an async function      exit 2

# Control — the NAMED form is fine (no rewrite happens):
printf 'export default async function named() { await 0; }\n' > /tmp/bug6b.js
cmake-build-asan/bin/hermesc -commonjs -dump-sema /tmp/bug6b.js
# => clean, exit 0
```

**Root cause:** the anonymous-default-export rewrite
(`lib/Sema/SemanticResolver.cpp:1526-1544`, "change it to a FunctionExpression
node for cleaner IRGen") passes a literal **`/* async */ false`** at `cpp:1538`
instead of `funcDecl->_async`. The rewritten `FunctionExpression` is then visited
as non-async, so its own `await` is rejected; an awaitless async body compiles as
a sync function (the returned value would not be a Promise).

**Suggested fix:** pass `funcDecl->_async` (and confirm `_generator` really is
intended to be forwarded, which it is at cpp:1537).

**Port pin:** `rust/crates/sema/tests/resolver.rs:2602`
(`export_default_anonymous_function_is_rewritten_to_an_expression`) asserts the
*buggy* `async == false` outcome with a non-degeneracy check — flip it together
with the C++ fix.

**Fixed upstream:** `6b59daf0d` (2026-08-08 branch), cherry-picked to `rust`
2026-08-10 as `4a0fe2bfd`.
**Pin flipped:** `resolver/modules.rs`'s rewrite #4 now passes
`func_decl.r#async.get()` instead of a literal `false` when building the
`FunctionExpression`. `tests/resolver.rs`'s
`export_default_anonymous_function_is_rewritten_to_an_expression` now
asserts `async == true` (the correct, non-buggy outcome), and a new
companion test, `export_default_anonymous_non_async_function_stays_non_async`,
pins both sides of the async forwarding. The corpus file
`export-default-anon-async.js` (run flagless) exercises the plain-mode
export-error path, while the async-forwarding fix is pinned by these
two unit tests. Rust mirror commit `044b815d1` (Task 3).

---

## 7. Compiler-side `JSONParser` has no recursion limit

**Severity:** robustness — stack overflow (process death) on deep input. The JS
parser has `CHECK_RECURSION`; `JSONParser` has no analog.

```bash
cmake --build cmake-build-asan --target json-parse-dump
python3 -c "print('['*100000 + ']'*100000)" > /tmp/bug7.json
cmake-build-asan/bin/json-parse-dump /tmp/bug7.json
# => AddressSanitizer:DEADLYSIGNAL (stack overflow), non-zero exit
```

**Root cause:** `lib/Parser/JSONParser.cpp` `parseValue`/`parseArray`/
`parseObject` recurse unbounded. (This is the compiler-side JSON parser — source
maps/metadata — not the runtime `JSON.parse`.)

**Port pin:** parity-by-absence, documented in
`rust/crates/parser/src/json/parser.rs` module doc; if C++ gains a limit, port it.

**Fixed upstream:** `b21856de4` (2026-08-08 branch), cherry-picked to `rust`
2026-08-10 as `0b8bbd1fc`.
**Pin flipped:** `json/parser.rs` gained a `recursion_depth: u32` field and
`parse_value` is now a depth-checking wrapper around the old body
(`parse_value_impl`) — `error("Too many nested JSON values")` at the limit,
mirroring the C++ fix's split. `MAX_RECURSION_DEPTH` is profile-selected
(128 debug/ASan, 1024 release), matching `JSParserImpl::MAX_RECURSION_DEPTH`'s
own `#ifdef` ladder. Live gate: `parser/tests/json_corpus/err_deep_nesting.json`
(json differential corpus 16 → 17). Rust mirror commit `ad4d7eb68` (Task 2).

---

## 8. Export diagnostics: inconsistent wording

```bash
printf 'export {a}; export * from "m";\nvar a;\n' > /tmp/bug8.js
cmake-build-asan/bin/hermesc -dump-sema /tmp/bug8.js
# => 1:1:  error: 'export' statement requires module mode
#    1:13: error: 'export' statement requires CommonJS module mode
```

`ExportNamed`/`ExportDefault` (cpp:1511/1520) say "module mode";
`ExportAll` (cpp:1553) says "CommonJS module mode". Cosmetic; the port preserves
both strings exactly (`resolver/modules.rs`).

**Fixed upstream:** `f90a83146` (2026-08-08 branch), cherry-picked to `rust`
2026-08-10 as `4193b558a`.
**Pin flipped:** `resolver/modules.rs`'s `visit_export_all_declaration` now
emits `"'export' statement requires module mode"`, byte-identical to the
Named/Default sites — the "CommonJS module mode" spelling is gone. The two
files this un-diverged, `export.js` and `module-export-plain.js`, re-green
with no corpus-content change (verified byte-identical against the oracle
before and after). Rust mirror commit `044b815d1` (Task 3).

---

## 9. Same-location diagnostic ties order by unstable `std::sort`

`SourceErrorManager`'s buffered-message flush (`lib/Support/SourceErrorManager.cpp:61-71`)
sorts with `std::sort` keyed on location only; two messages at the same location
have unspecified relative order, dependent in practice on the whole array size
(libstdc++ introsort). Witness: `test/Sema/invalid-args-eval.js` — the `89:9`
pair (strict-mode `cannot declare 'arguments'` error vs `was not declared in
function "global"` warning) comes out in different orders depending on how many
sibling messages are buffered; a 2-message minimization agrees with insertion
order. Fix would be `std::stable_sort`. The port uses a stable sort (documented
deviation, `rust/crates/support/src/manager.rs:903-909`), which is why the file
sits in the corpus's Deferred list as unfixable-by-construction.

**Fixed upstream:** `5f313a13a` (2026-08-08 branch), cherry-picked to `rust`
2026-08-10 as `7805e2103`.
**Pin flipped:** no Rust code change was needed — `manager.rs`'s
`sort_by_key` was already stable, so the "documented deviation" was really a
one-sided divergence (C++ unstable, Rust stable) that happened to agree on
this file's tie. The comment at `manager.rs:903-914` is rewritten to record
the divergence as **retired**: `5f313a13a` changed C++ to
`std::stable_sort`, so both sides now break same-location ties in emission
order by construction, not by luck. `test/Sema/invalid-args-eval.js` is
imported into the corpus (previously Deferred). Rust mirror commit
`400f108ae` (Task 5).

---

## 10. Dead code / stale docs (no repro needed — code reading)

- **`ScopedFunctionPromoter`:** local `newDecls` (`.cpp:174-206`) is populated
  and never read; the header's closing sentence (`.h:28-30` — promoted functions
  "are deleted from their own scope and added to the function scope") describes
  behavior the implementation does not have. Port carries the structure with a
  `DEAD in C++ too` comment (`resolver/promoter.rs`).
  **Fixed upstream:** `9232443cf` (2026-08-08 branch), cherry-picked to `rust`
  2026-08-10 as `ffcdbdd52`. **Pin flipped:** both halves removed —
  `promoter.rs`'s write-only `new_decls`/`ScopeDecls::new()`/two `push`
  sites are gone, and the module doc's header quote now ends with the
  corrected upstream sentence. Rust mirror commit `044b815d1` (Task 3).
- **`SemanticResolver.cpp:1931-1937`:** `if (false && localEval)` — permanently
  dead branch (the port's `unresolver.rs` documents it). **Not part of the
  11 upstream fixes** — this dead branch is untouched by the 2026-08-08
  cherry-picks and remains dead in both C++ and Rust.
- **`JSParserImpl-jsx.cpp:493`:** the `isa<MemberExpressionNode>` check is dead —
  `JSXMemberExpression` derives from the `JSX` base, not `MemberExpression`
  (found in parser phase P8; mirrored harmlessly).
  **Fixed upstream:** `37520ccef` (2026-08-08 branch), cherry-picked to
  `rust` 2026-08-10 as `51035e8c2`. **Pin flipped:** `jsx.rs`'s
  `parse_jsx_element_name` now matches `Node::JSXMemberExpression(_)`
  instead of the always-false `Node::MemberExpression(_)`, mirroring the
  C++ one-line fix — a JSX attribute name that is a member expression
  (`<foo a.b="1">`) is now correctly rejected. New unit test
  `jsx_member_expression_attribute_name_is_rejected`. Rust mirror commit
  `ad4d7eb68` (Task 2).

---

## 11. Flow `match` binding pattern: bad-token diagnostic then abort

**Severity:** debug-abort on invalid input (hermesc reports a real error first,
then aborts instead of exiting cleanly).

```bash
printf 'const e = match (x) { const [y]: 2 };\n' > /tmp/bug11.js
cmake-build-asan/bin/hermesc -parse-flow -Xparse-flow-match -dump-ast /tmp/bug11.js
# => /tmp/bug11.js:1:29: error: 'identifier' expected in match binding pattern
#    const e = match (x) { const [y]: 2 };
#                          ~~~~~~^
#    hermesc: include/hermes/Parser/JSLexer.h:160:
#    hermes::UniqueString *hermes::parser::Token::getResWordOrIdentifier() const:
#    Assertion `getKind() == TokenKind::identifier || isResWord()' failed.
#    SIGABRT, exit 134
```

**Root cause:** after the match binding-pattern parser reports "'identifier'
expected in match binding pattern" for the current (non-identifier, non-
reserved-word) token, some caller on the error-recovery path still calls
`Token::getResWordOrIdentifier()` on that same token without checking `check
(identifier)`/`isResWord()` first, so the assert at `JSLexer.h:160` fires on
the very next line. Release builds skip the assert and read `ident_`
uninitialized/stale for that token kind (untested — no release `hermesc`
build was available for this repro).

**Port pin:** `token.rs:133`'s `debug_assert!` panics identically on the same
input (`--parse-flow --parse-flow-match /tmp/bug11.js`, `ast-dump` binary) —
bug-for-bug parity, not an independent port bug. Exact panic:
`assertion failed: self.kind == TokenKind::identifier || self.is_res_word()`.
If the C++ side is fixed (guarding the call, or having the binding-pattern
parser bail out before reaching it), mirror the fix in the Rust match-pattern
parser rather than relaxing the `debug_assert!`.

**Fixed upstream:** `550aafe33` (2026-08-08 branch), cherry-picked to `rust`
2026-08-10 as `bfeeb404f`.
**Pin flipped:** `match_.rs`'s `parse_match_binding_pattern_flow` now
`return`s `None` right after reporting the diagnostic, mirroring the C++
fix's early return — the caller never reaches
`get_res_word_or_identifier`/`token.rs:133`'s `debug_assert!` on that token,
so the `debug_assert!` itself is **unchanged** (it stays a faithful port of
`JSLexer.h:160` and must not be relaxed; only the caller stopped reaching
it, exactly as upstream). New unit test
`match_binding_pattern_without_identifier_recovers_cleanly`; live corpus pin
`sema_corpus/flow-match-pattern-binding-error.js`. Rust mirror commit
`ad4d7eb68` (Task 2).

---

## 12. `try/catch/finally` inside a function aborts the parser-entry resolver

**Date found:** 2026-08-12, during the `hermes-sema` façade work (Task 3 of
`doc/superpowers/plans/2026-08-12-publication-scope-sema-and-cli.md`), by the
façade-vs-low-level agreement sweep over the sema corpus.

**Severity:** debug-abort on **valid input** through the public
`resolveASTForParser` entry — and, unlike items 1–5, the Release behavior is a
**silently wrong answer**, not a benign pass-through.

```bash
printf 'function f() { try {} catch (e) {} finally {} }\n' > /tmp/bug12.js
cmake-build-asan/bin/sema-parser-dump /tmp/bug12.js
# => Assertion `!(node->_handler && node->_finalizer) && "try-catch-finally
#    should have been transformed by SemanticResolver"' failed.
#    (lib/Sema/CheckImplicitReturn.cpp:250)   SIGABRT, exit 134
```

Top-level `try/catch/finally` does **not** trip it — the check runs per
function, so the statement must be inside a function body. The stock
`hermesc -dump-sema` path is unaffected (it runs `compile = true`).

**Root cause:** `SemanticResolver` splits `try { } catch { } finally { }` into
nested try-catch/try-finally, but the split is gated on `compile_`
(`lib/Sema/SemanticResolver.cpp:794`). The `mayReachImplicitReturn` call that
runs `CheckImplicitReturn` (~`SemanticResolver.cpp:1950-1957`) is **not**
gated, so under `compile = false` the checker meets an unsplit
try-catch-finally and hits the assert its invariant assumes away.

**Release consequence (worse than a debug abort):** with the assert compiled
out, the unsplit statement takes the handler-only branch and the **finalizer is
ignored**, so `mayReachImplicitReturn` can compute the wrong answer for a
function whose `finally` block affects reachability.

**Fix direction:** either gate the `mayReachImplicitReturn` call on `compile_`
the way the split is, or teach `CheckImplicitReturn` to handle an unsplit
try-catch-finally (walk handler *and* finalizer) instead of asserting.

**Rust pin:** the port reproduces the abort faithfully — panic at
`rust/crates/sema/src/check_implicit_return.rs:340`. The façade agreement sweep
(`rust/crates/sema/tests/facade_agreement.rs`) skips the single corpus file that
would trip it, via a documented named constant; delete that skip when this is
fixed.

**Status:** OPEN — found after the 2026-08-10 propagation of items 1–11, so it
is not in that campaign. Not yet fixed upstream, not yet in
`~/work/hermes-cpp-defects`.

---

## Summary table

**Status (2026-08-10): all 11 items fixed upstream and mirrored into the Rust
port.** **Item 12 was found later (2026-08-12) and is OPEN.** See `doc/superpowers/plans/2026-08-10-cpp-defect-fixes-propagation.md`
for the propagation plan and each item above for its `Fixed upstream`/
`Pin flipped` lines.

| # | Input | Effect | Site | Release behavior | Fixed upstream (in-tree) |
|---|-------|--------|------|------------------|---------------------------|
| 1 | `using` + block fn | abort | ScopedFunctionPromoter.cpp:255-262 | treated as `var` | `4ad67c992` (`e4408f849`) |
| 2 | anon export default via parser entry + dump | abort | SemContext.cpp:493-494 | crash (null deref) or UB — untested | `918158cb0` (`179fb8ca3`) |
| 3 | `with` via parser entry + dump | abort | SemContext.h:559 | prints ` UNR`, fine | `918158cb0` (`179fb8ca3`) |
| 4 | `class C { x = class {}; }` | abort | SemContext.cpp:478 | dump silently wrong/partial — untested | `b351e1184` (`48d221fb2`) |
| 5 | `$SHBuiltin.#x()` | abort | SemanticResolver.cpp:1166-1167 | UB cast — untested | `07efab88d` (`416aafcd2`) |
| 6 | anon `export default async function` (`-commonjs`) | **wrong semantics** | SemanticResolver.cpp:1538 | same (not assert-gated) | `6b59daf0d` (`4a0fe2bfd`) |
| 7 | deep JSON | stack overflow | JSONParser.cpp | same | `b21856de4` (`0b8bbd1fc`) |
| 8 | export outside module mode | wording | cpp:1511/1553 | same | `f90a83146` (`4193b558a`) |
| 9 | same-location diagnostics | unstable order | SourceErrorManager.cpp:61-71 | same | `5f313a13a` (`7805e2103`) |
| 10a | `ScopedFunctionPromoter` dead `newDecls` | dead code | ScopedFunctionPromoter.cpp:174-206 | same | `9232443cf` (`ffcdbdd52`) |
| 10c | `JSParserImpl-jsx.cpp:493` dead `isa<MemberExpressionNode>` | dead code (was harmless) | jsx.cpp:493 | same | `37520ccef` (`51035e8c2`) |
| 11 | `match` binding pattern, bad token | abort | JSLexer.h:160 | untested | `550aafe33` (`bfeeb404f`) |
| 12 | `try/catch/finally` in a function, parser entry | abort | CheckImplicitReturn.cpp:250 | **wrong answer** (finalizer ignored) | **OPEN** (found 2026-08-12) |

(Item 10b, `SemanticResolver.cpp:1931-1937`'s `if (false && localEval)`, is
not part of the 11 fixes and remains open/dead in both languages.)
