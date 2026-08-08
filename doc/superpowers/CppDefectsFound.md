# C++ defects found by the Rust front-end port

**Date:** 2026-08-08 (repros re-verified this date). **Discovered by:** the port's
byte-for-byte differential validation (S2–S4a sweeps, reviews, and the whole-Sema
capstone). Every item below was reproduced against real binaries; commands and
observed output are verbatim.

**Build config for repros:** `cmake-build-asan` (Debug + ASan, the repo's default
dev build — `CLAUDE.md`). Items 1–5 are assertion failures, visible only in
assert-enabled builds; Release builds pass through them (with the noted behavior).
Items 2–3 use the in-repo oracle tool `tools/sema-parser-dump/` (build:
`cmake --build cmake-build-asan --target sema-parser-dump`), which drives the
public `resolveASTForParser` + `semDump` entries — the crashes are in those
library functions, not in the tool.

**Port coordination:** the Rust port preserves each defect bug-for-bug with an
explicit pin that *asserts the buggy behavior* (so silent drift is caught). If a
defect is fixed in C++, flip the corresponding pin (named per item) in the same
change.

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

---

## 10. Dead code / stale docs (no repro needed — code reading)

- **`ScopedFunctionPromoter`:** local `newDecls` (`.cpp:174-206`) is populated
  and never read; the header's closing sentence (`.h:28-30` — promoted functions
  "are deleted from their own scope and added to the function scope") describes
  behavior the implementation does not have. Port carries the structure with a
  `DEAD in C++ too` comment (`resolver/promoter.rs`).
- **`SemanticResolver.cpp:1931-1937`:** `if (false && localEval)` — permanently
  dead branch (the port's `unresolver.rs` documents it).
- **`JSParserImpl-jsx.cpp:493`:** the `isa<MemberExpressionNode>` check is dead —
  `JSXMemberExpression` derives from the `JSX` base, not `MemberExpression`
  (found in parser phase P8; mirrored harmlessly).

---

## Summary table

| # | Input | Effect | Site | Release behavior |
|---|-------|--------|------|------------------|
| 1 | `using` + block fn | abort | ScopedFunctionPromoter.cpp:255-262 | treated as `var` |
| 2 | anon export default via parser entry + dump | abort | SemContext.cpp:493-494 | crash (null deref) or UB — untested |
| 3 | `with` via parser entry + dump | abort | SemContext.h:559 | prints ` UNR`, fine |
| 4 | `class C { x = class {}; }` | abort | SemContext.cpp:478 | dump silently wrong/partial — untested |
| 5 | `$SHBuiltin.#x()` | abort | SemanticResolver.cpp:1166-1167 | UB cast — untested |
| 6 | anon `export default async function` (`-commonjs`) | **wrong semantics** | SemanticResolver.cpp:1538 | same (not assert-gated) |
| 7 | deep JSON | stack overflow | JSONParser.cpp | same |
| 8 | export outside module mode | wording | cpp:1511/1553 | same |
| 9 | same-location diagnostics | unstable order | SourceErrorManager.cpp:61-71 | same |
