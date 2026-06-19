# JS Parser P7 — TypeScript Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port `lib/Parser/JSParserImpl-ts.cpp` (1,437 lines) + the ~16 TS-only integration sites in `JSParserImpl.cpp`, behind a `Context::parse_ts` flag, so the Rust parser handles the full TypeScript type grammar byte-for-byte vs `hermesc -dump-ast -parse-ts`.

**Architecture:** Mirror the Flow port exactly. A new `rust/crates/parser/src/js/ts/` module directory (`mod`, `types`, `function_types`, `object_types`, `declarations`, `params`) holds `impl JSParserImpl` blocks, gated by a new `parse_ts()` that reads a real `Context::parse_ts` flag (replacing the `false` stub). All TS-only branches already carry `// P7` markers at their Rust sites (grep `// P7`). A new `tests/parser_corpus_ts/` differential dir runs `hermesc -parse-ts` vs `ast-dump --parse-ts`. **No AST nodes are added** — the generated 271-node set already includes every TS node (`generated_idempotent` stays the guardrail).

**Tech Stack:** Rust 1.96.0, the `parser`/`ast`/`support` crates; `hermesc -dump-ast -parse-ts` as the byte-for-byte oracle; `cargo test -p parser --test parser_differential` (`REQUIRE_DIFFERENTIAL=1`) as the gate.

---

## Source of truth & conventions (read before any task)

- **C++ TS file:** `lib/Parser/JSParserImpl-ts.cpp` (lines 19–1433, all under `#if HERMES_PARSE_TS`). This is the *entire* dedicated TS grammar — 27 methods. Every method is listed with its C++ line range in the tasks below.
- **C++ integration sites:** `lib/Parser/JSParserImpl.cpp` (16 TS-only `#if HERMES_PARSE_TS` blocks + 2 shared `parseTypeArguments` dispatchers). Mapped in Task P7.5.
- **C++ header:** `lib/Parser/JSParserImpl.h` — method decls at 1599–1635; the `parseTypeAnnotation`/`parseReturnTypeAnnotation`/`parseTypeArguments` dispatchers at 1209–1248; `checkDeclaration`'s TS branch at 629–641; `IsConstructorType { No, Yes }` at 1599.
- **Faithful-port rules (from the handoff, non-negotiable):**
  - **C++ default arguments are spec — read the header.** Critical recurring traps:
    - `parseTypeAnnotationTS(Optional<SMLoc> wrappedStart = None)` — the wrapped-start arg.
    - `advance(...)` / `checkAndEat(...)` / `eat(...)` grammar-context arg: TS code uses **`GrammarContext::Type`** almost everywhere (it splits `>>`, needed for nested generics `A<B<C>>`). Copy the exact context per call site; do NOT default to `AllowRegExp`/`AllowDiv`. Note the deliberate exceptions: interface `extends` is eaten in `AllowRegExp` (ts.cpp:610); the type-assertion `>` is eaten in `AllowRegExp` (JSParserImpl.cpp:4170-4172).
    - **`lexer_.lookahead1(None)` defaults to `RequireNoNewLine = true`** (JSLexer.h:658) → Rust `lookahead1::<true>(None)`. Used at ts.cpp:244 (`this:`/`this?`) and ts.cpp:1264 (index-signature disambiguation) and the `checkDeclaration` TS branch.
  - **C++ `template`s/enums stay generics/enums** — `IsConstructorType` is a runtime enum in C++; port it as a Rust enum (like Flow's `AllowTypedArrowFunction`), NOT a bool. No template flattening (there are no templates in the TS file, but verify in the structural-fidelity review).
  - **C++ RAII guards → explicit/Drop guards.** `parseTypeAnnotationTS` opens `llvh::SaveAndRestore<bool> saveParam(allowAnonFunctionType_, true)` (ts.cpp:23). `allowAnonFunctionType_` already exists in Rust as the Flow `ParamFlagGuard` (`Rc<Cell<bool>>`); reuse it — set it true for the body of `parse_type_annotation_ts` via the existing guard, restoring on every `?` path.
  - **Contextual keywords:** `check(typeIdent_)`/`check(namespaceIdent_)`/etc. are the escape-INsensitive `check(UniqueString*)` overload → Rust `check_name`, NOT `check_unescaped_name`. The interned idents needed: `anyIdent_`, `booleanIdent_`, `numberIdent_`, `symbolIdent_`, `stringIdent_`, `bigintIdent_`, `neverIdent_`, `undefinedIdent_`, `unknownIdent_`, `typeIdent_`, `interfaceIdent_`, `namespaceIdent_`, `readonlyIdent_`, `staticIdent_`, `publicIdent_`, `privateIdent_`, `protectedIdent_`, `thisIdent_`, `fromIdent_`, `valueIdent_`. Most already exist from Flow/P4 (`type`, `interface`, `from`, `value`, `static`, `this`); add any missing TS-specific ones (`any`/`boolean`/`number`/`symbol`/`string`/`bigint`/`never`/`undefined`/`unknown`/`namespace`/`readonly`/`public`/`private`/`protected`) the same way Flow added its idents.
  - **Keep the comments.** Each Rust method carries a `/// Port of JSParserImpl::parseTSXxx (ts.cpp:NNNN-NNNN).` doc line and inline `// C++ NNNN` markers at non-obvious branches, matching the Flow modules.
  - **Honest deferral.** Anything not yet implemented in a sub-task returns a real parse error (or `unimplemented!` ONLY if unreachable by the current corpus); never a silent fallthrough. The differential corpus only exercises landed features.
- **Existing Rust helpers to reuse** (already ported, used by Flow): `parse_binding_element`, `parse_binding_identifier`, `parse_assignment_expression` (note its `Param`/`CoverTypedParameters` defaults), `parse_statement_list_item`, `set_location`, `eat`/`need`/`check`/`check_and_eat`/`advance`/`error`/`error_expected`, `eat_semi`, `get_prev_token_end_loc`, `cur_range`, `dummy_range`, `NodeList::from_iter`, `is_new_line_before_current_token`, `lookahead1`. The AST node constructors follow the generated `Node::TSXxx(TSXxx::new(NodeMetadata::new(...), field, ...))` pattern (snake_case fields; check `rust/crates/ast/src/node.rs` for exact field names/order per node).

### Validation commands (every task ends green on these)

```bash
# Build hermesc oracle once (if cmake-build-asan missing, configure per CLAUDE.md):
cmake --build cmake-build-asan --target hermesc
# Build ast-dump bin:
cargo build --manifest-path rust/Cargo.toml -p parser --bin ast-dump
# Workspace build (ZERO warnings) + tests:
cargo build --manifest-path rust/Cargo.toml
cargo test  --manifest-path rust/Cargo.toml -p parser
# The TS differential (force-run; expect "parser differential (tests/parser_corpus_ts): N corpus files matched"):
REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p parser --test parser_differential -- --nocapture
# The plain + Flow corpora MUST stay green (TS must not leak into plain JS / Flow):
#   the same parser_differential run covers parser_corpus, parser_corpus_flow{,_component,_records,_match}.
cargo clippy --manifest-path rust/Cargo.toml -p parser   # no NEW lints
```

To capture a reference for a hand check: `(! cmake-build-asan/bin/hermesc -dump-ast -dump-source-location=both -parse-ts FILE 2>&1)`.

---

## File structure

- **Create** `rust/crates/parser/src/js/ts/mod.rs` — module root: `mod types; mod function_types; mod object_types; mod declarations; mod params;` + the `IsConstructorType` enum + any shared helpers. Mirrors `js/flow/mod.rs`.
- **Create** `rust/crates/parser/src/js/ts/types.rs` — `parse_type_annotation_ts`, union/intersection/postfix/primary, type reference, qualified name, type query, tuple, the `reparse_identifier_as_ts_type_annotation` helper.
- **Create** `rust/crates/parser/src/js/ts/function_types.rs` — `parse_ts_function_or_parenthesized_type`, `parse_ts_function_type_params`, `parse_ts_function_type_param`.
- **Create** `rust/crates/parser/src/js/ts/object_types.rs` — `parse_ts_object_type`, `parse_ts_object_type_member`, `parse_ts_index_signature`.
- **Create** `rust/crates/parser/src/js/ts/declarations.rs` — `parse_ts_declaration`, type-alias, interface (+heritage), enum (+member), namespace.
- **Create** `rust/crates/parser/src/js/ts/params.rs` — `parse_ts_type_parameters`, `parse_ts_type_parameter`, `parse_ts_type_arguments`.
- **Modify** `rust/crates/ast/src/context.rs` — add `parse_ts: bool` field + `parse_ts()`/`set_parse_ts()` (mirror `parse_flow` at lines 184/350/356).
- **Modify** `rust/crates/parser/src/js/mod.rs` — `mod ts;`; `parse_ts()` reads `self.gc.ctx().parse_ts()`; wire the TS arm into `parse_type_annotation`/`parse_return_type_annotation`/`parse_type_arguments` dispatchers.
- **Modify** `rust/crates/parser/src/bin/ast_dump.rs` — add `--parse-ts` flag → `ctx.set_parse_ts(true)`.
- **Modify** `rust/crates/parser/tests/parser_differential.rs` — add `run_differential("tests/parser_corpus_ts", &["-parse-ts"], &["--parse-ts"])`.
- **Modify** the existing core files for integration (Task P7.5): `js/functions.rs`, `js/classes.rs`, `js/expressions.rs`, `js/statements.rs`, `js/modules.rs` — replace their `// P7` markers with the real TS dispatch.
- **Create** `rust/crates/parser/tests/parser_corpus_ts/*.ts.js` — the growing differential corpus (named `.js` so the harness picks them up; content is TS). One+ file added per task.

---

## Task P7.0 — Foundations + gate

**Goal:** a real `parse_ts` flag end-to-end, the `ts/` module skeleton, the differential corpus dir, and a minimal `type X = string;` round-tripping byte-for-byte.

**Files:**
- Modify: `rust/crates/ast/src/context.rs` (field + accessors, mirror `parse_flow`)
- Modify: `rust/crates/parser/src/js/mod.rs:248-250` (`parse_ts()` real) + `mod ts;`
- Modify: `rust/crates/parser/src/bin/ast_dump.rs` (`--parse-ts` flag)
- Modify: `rust/crates/parser/tests/parser_differential.rs` (new corpus call)
- Create: `rust/crates/parser/src/js/ts/mod.rs`, `ts/types.rs`, `ts/declarations.rs` (+ empty-ish `function_types.rs`/`object_types.rs`/`params.rs` stubs declared in mod)
- Create: `rust/crates/parser/tests/parser_corpus_ts/type_alias_primitive.js`

- [ ] **Step 1 — Context flag.** In `context.rs`: add `parse_ts: bool` to the struct (next to `parse_flow`), `false` in the constructor, and `pub fn parse_ts(&self) -> bool` / `pub fn set_parse_ts(&mut self, v: bool)`. Mirror lines 184/231/350-357 exactly.

- [ ] **Step 2 — `parse_ts()` real.** In `js/mod.rs`, replace the `false // P7` body of `parse_ts()` with `self.gc.ctx().parse_ts()`. Update the doc comment (drop "always false until P7").

- [ ] **Step 3 — module skeleton.** Add `mod ts;` to `js/mod.rs` (next to `mod flow;`). Create `js/ts/mod.rs` with the copyright header, a module doc comment (mirror `flow/mod.rs:1-33`, describing the child-module split), `mod types; mod function_types; mod object_types; mod declarations; mod params;`, the necessary `use` lines, and:
  ```rust
  /// Whether a parenthesized type is a constructor type (`new (...) => T`).
  /// Port of `JSParserImpl::IsConstructorType` (JSParserImpl.h:1599). Runtime
  /// enum (faithful), NOT a bool.
  #[derive(Clone, Copy, PartialEq, Eq, Debug)]
  pub(super) enum IsConstructorType {
      No,
      Yes,
  }
  ```
  Create the five child files each with the copyright header + `use` + an empty `impl<'gc, 'ast, 'ctx, 'a> JSParserImpl<'gc, 'ast, 'ctx, 'a> {}` block (filled in by later tasks). `function_types.rs`/`object_types.rs`/`params.rs` start empty-impl; `types.rs`/`declarations.rs` get the Step-5 content.

- [ ] **Step 4 — ast-dump + differential wiring.** In `ast_dump.rs`: add a `parse_ts: Opt<bool>` option (`--parse-ts`, mirror `parse_flow` at 44/93-99), and `ctx.set_parse_ts(*opt.parse_ts)`. **TS and Flow are mutually exclusive dialects** — do NOT OR `parse_ts` into `parse_flow`. In `parser_differential.rs`, add inside the differential test fn (after the Flow corpora):
  ```rust
  run_differential("tests/parser_corpus_ts", &["-parse-ts"], &["--parse-ts"]);
  ```

- [ ] **Step 5 — minimal gate (TDD).** Write `tests/parser_corpus_ts/type_alias_primitive.js` containing:
  ```ts
  type X = string;
  type Y = number;
  ```
  Run the differential — it FAILS (TS not parsed). Then implement the minimum:
  - In `ts/declarations.rs`: `parse_ts_declaration` (ts.cpp:516-535) dispatching `type`→`parse_ts_type_alias_declaration`, and a minimal `parse_ts_type_alias_declaration` (ts.cpp:537-578) that parses `id`, **defers type-params** (honest error if `<` present — a later task adds them), eats `=`, calls `parse_type_annotation_ts(None)`, eats semi, builds `TSTypeAliasDeclaration`.
  - In `ts/types.rs`: a minimal `parse_type_annotation_ts(wrapped_start)` that, for this task, handles ONLY the `parse_ts_union_type → … → parse_ts_primary_type` chain restricted to the keyword arms it needs (`string`/`number`), returning honest errors elsewhere. (Task P7.1 fills the rest; keep this minimal but on the real call path so P7.1 extends, not rewrites.)
  - Wire `checkDeclaration`'s TS branch (header 629-641) into the Rust `check_declaration` and `parse_declaration`'s TS dispatch (JSParserImpl.cpp:866-872, the `// P7` at `statements.rs:397`): when `parse_ts()`, `parse_ts_declaration`.
  - Wire the `parse_type_annotation` dispatcher (`js/mod.rs:269`) TS arm: `if self.parse_ts() { return self.parse_type_annotation_ts(wrapped_start); }`.

- [ ] **Step 6 — verify + commit.** Differential green for `parser_corpus_ts` (1 file), all other corpora unchanged, zero warnings.
  ```bash
  git add -A && git commit -m "rust(parser): P7.0 TypeScript foundations + gate (type alias to primitive)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

## Task P7.1 — type annotation grammar core (types.rs + params.rs)

**Goal:** the full type-annotation precedence hierarchy + type references + type params/args, so non-function/non-object types round-trip.

**Files:** `ts/types.rs`, `ts/params.rs`; corpus files exercising each construct.

**C++ to port (ts.cpp):**
- `parseTypeAnnotationTS` — **full** (21-134): identifier-predicate backtrack via `SavePoint` (28-50; `id is T` → `TSTypePredicate`), `new`-constructor-type (53-72), `<`-function-type (73-83), else `parseTSUnionType` (84-89), then trailing `extends ... ? ... : ...` conditional type (92-125), then the `wrappedStart` → `TSTypeAnnotation` wrap (127-131). **The function/constructor-type arms call `parse_ts_function_or_parenthesized_type` (P7.2) and `parse_ts_type_parameters` (this task) — honest-error those two arms here only if P7.2 not yet landed; sequence P7.2 right after.** Open the `allowAnonFunctionType_` guard = true at entry (ts.cpp:23).
- `parseTSUnionType` (136-163), `parseTSIntersectionType` (165-192) — leading-`|`/`&` tolerance, single-element passthrough, `Type` context.
- `parseTSPostfixType` (866-901) — `[]`→`TSArrayType`, `[T]`→`TSIndexedAccessType`; guarded by `!isNewLineBeforeCurrentToken`.
- `parseTSPrimaryType` (903-1058) — `CHECK_RECURSION`; `*`→`ExistsTypeAnnotation`; `(`→`parse_ts_function_or_parenthesized_type` (P7.2 — honest-error until landed); `{`→`parse_ts_object_type` (P7.3 — honest-error until landed); `interface`→`parse_ts_interface_declaration` (P7.4); `typeof`→`parseTSTypeQuery`; `[`→`parseTSTupleType`; `this`→`TSThisType`; the keyword idents (`any`/`boolean`/`number`/`symbol`/`string`/`bigint`/`never`/`undefined`/`unknown`, the `rw_static`/`identifier` arm 928-990) → the matching `TSXxxKeyword`/else `parseTSTypeReference`; `null`/`void`/string/numeric/bigint/`true`/`false` literals → `TSLiteralType` wrapping the literal node; default resword → `parseTSTypeReference`, else error.
- `parseTSTypeReference` (1060-1081) + `parseTSQualifiedName` (1083-1114) + `parseTSTypeQuery` (1116-1158) + `parseTSTupleType` (194-221) + `reparseIdentifierAsTSTypeAnnotation` (1406-1431).
- `parseTSTypeParameters` (801-830), `parseTSTypeParameter` (832-864), `parseTSTypeArguments` (1160-1190) → in `ts/params.rs`. Then **add type-params to the type-alias** (revisit P7.0's deferral: ts.cpp:550-556) and **type-args to type references** (ts.cpp:1069-1075).

- [ ] **Step 1 — params.rs (TDD).** Corpus `ts_type_params.js`: `type Box<T> = T; type Pair<T, U = T> = T; type C<T extends string> = T;`. Implement `parse_ts_type_parameters`/`parse_ts_type_parameter`/`parse_ts_type_arguments`; lift the P7.0 type-alias type-param deferral. Differential green.
- [ ] **Step 2 — union/intersection/postfix (TDD).** Corpus `ts_union_intersection.js`: `type U = string | number; type I = A & B; type Arr = string[]; type Idx = T['k'];`. Differential green.
- [ ] **Step 3 — primary keywords + literals (TDD).** Corpus `ts_primary.js`: all keyword types, `this`, `*` exists, literal types (`'a'`, `1`, `true`, `null`, `void`), `typeof x`, tuples `[A, B]`, qualified `A.B.C`, type refs `Foo<Bar>`. Differential green.
- [ ] **Step 4 — predicate + conditional (TDD).** Corpus `ts_predicate_conditional.js`: `type P = (x) => x is string;` needs P7.2 — instead test the standalone forms reachable now: conditional `type Cond<T> = T extends string ? 1 : 2;` and the identifier-predicate path through a return type once P7.2 lands. For this task, land the `parseTypeAnnotationTS` predicate-backtrack + conditional-type code (it's reachable via `type Cond<T> = T extends U ? X : Y`). Differential green.
- [ ] **Step 5 — commit** `rust(parser): P7.1 TS type-annotation core (union/intersection/postfix/primary, refs, type params/args)`.

**Fidelity notes:** `parseTSTypeReference` returns `TSTypeReferenceNode*` specifically (header 1630) because interface-heritage (P7.4) reaches into `_typeParameters`. The predicate backtrack at ts.cpp:28-50 advances in `Type` context then checks `check(isIdent_)` (the `is` contextual ident — confirm it exists from Flow); restore the `SavePoint` if not a predicate. Conditional-type `extends`/`?`/`:` all eat in `Type` context.

---

## Task P7.2 — function, constructor & parenthesized types (function_types.rs)

**Goal:** the `( Type )`-vs-`(params) => T` cover, constructor types, parameter properties.

**Files:** `ts/function_types.rs`; corpus.

**C++ to port (ts.cpp):**
- `parseTSFunctionOrParenthesizedType` (223-389) — the full cover algorithm: leading `this:`/`this?` param (243-263; `lookahead1::<true>`), `...rest` (265-276), nested `(` type (277-281), empty `()` (282-286), the speculative first-param parse + `TSParameterProperty`/`Identifier`/type disambiguation (287-315), the comma-param loop (319-343), `)` eat (345-351), `=>` decision (353-365), parens-bump-and-return-as-type (367-370), return-type parse (372-374), `TSConstructorType`/`TSFunctionType` build (376-388). `incParens()` → the Rust `inc_parens` equivalent used by Flow.
- `parseTSFunctionTypeParams` (391-417) + `parseTSFunctionTypeParam` (419-514) — the modifier loop (`static`/`export`/`readonly`/`public`/`private`/`protected` → `TSParameterProperty`), then `parseBindingElement`.

- [ ] **Step 1 — function/parenthesized cover (TDD).** Corpus `ts_function_types.js`: `type F = (x: number) => string; type G = () => void; type P = (number); type R = (...args: number[]) => void; type Ctor = new (x: A) => B; type T = (this: X, y: Y) => Z;`. Implement; wire the `parseTypeAnnotationTS` `new`/`<`/`(` arms (P7.0/P7.1 deferrals) to call this. Differential green.
- [ ] **Step 2 — parameter properties (TDD).** Corpus `ts_param_props.js` exercising `readonly`/`public`/`static` param modifiers in a function type. Differential green.
- [ ] **Step 3 — commit** `rust(parser): P7.2 TS function/constructor/parenthesized types + parameter properties`.

**Fidelity notes:** This is the analog of Flow's parenthesized-type cover — the single most error-prone TS method. Port branch-for-branch; do NOT restructure the `isFunction`/`hasRest`/`type`/`params` state machine. `allowAnonFunctionType_` reads (265, 319, 361) must read the live guard flag. The `dyn_cast<TSParameterPropertyNode>` / `dyn_cast<IdentifierNode>` disambiguation (292-314) maps to Rust `match`/`if let` on the returned node; replicate the `_typeAnnotation`/`_optional` field checks and `reparseIdentifierAsTSTypeAnnotation` fallback exactly.

---

## Task P7.3 — object types & signatures (object_types.rs)

**Goal:** `{ ... }` type literals — property/method/call/index signatures.

**Files:** `ts/object_types.rs`; corpus.

**C++ to port (ts.cpp):** `parseTSObjectType` (1192-1224), `parseTSObjectTypeMember` (1226-1363) — call signature (1229-1245), the computed-key / index-signature disambiguation via `lookahead1::<true>` (1260-1269), property signature (1287-1319), method signature (1321-1340), the bare-return-type property fallback (1342-1362); `parseTSIndexSignature` (1365-1404).

- [ ] **Step 1 — object type literals (TDD).** Corpus `ts_object_types.js`: `type O = { a: number; b?: string; readonly c: boolean }; type M = { f(x: number): void }; type C = { (x: A): B }; type I = { [k: string]: number };`. Implement; wire `parseTSPrimaryType`'s `{` arm (P7.1 deferral). Differential green.
- [ ] **Step 2 — commit** `rust(parser): P7.3 TS object types + property/method/call/index signatures`.

**Fidelity notes:** members are separated by `,` OR `;` and the trailing separator is optional (1204-1209). The `init`/`readonly`/`isStatic`/`isExport` fields on `TSPropertySignature` are present in the node but always default here (the C++ has `TODO: Parse modifiers/initializer` at 1250-1258) — replicate the defaults, do not invent modifier parsing.

---

## Task P7.4 — declarations (declarations.rs)

**Goal:** type alias (already minimal — finalize), interface (+heritage), enum, namespace/module.

**Files:** `ts/declarations.rs`; corpus.

**C++ to port (ts.cpp):** `parseTSDeclaration` (516-535, finalize the `interface`/`namespace`/`enum` dispatch), `parseTSTypeAliasDeclaration` (537-578, already landed — verify), `parseTSInterfaceDeclaration` (580-677, incl. `extends` heritage → `TSInterfaceHeritage`, body via `parseTSObjectTypeMember`), `parseTSEnumDeclaration` (679-723) + `parseTSEnumMember` (725-748), `parseTSNamespaceDeclaration` (750-799, body via `parseStatementListItem` with `AllowImportExport::Yes` → `TSModuleBlock`/`TSModuleMember`).

- [ ] **Step 1 — interface (TDD).** Corpus `ts_interface.js`: `interface I<T> extends A, B<T> { x: number; f(): void }`. Differential green.
- [ ] **Step 2 — enum (TDD).** Corpus `ts_enum.js`: `enum E { A, B = 1, C }`. Differential green.
- [ ] **Step 3 — namespace (TDD).** Corpus `ts_namespace.js`: `namespace N { type X = number; export const y = 1; }`. Differential green.
- [ ] **Step 4 — commit** `rust(parser): P7.4 TS interface/enum/namespace declarations`.

**Fidelity notes:** `checkDeclaration`'s TS branch (header 629-641) already gates `type`/`interface`/`namespace` (each requires a following identifier via `lookahead1::<true>`) + bare `rw_interface`/`rw_enum` — confirm the Rust `check_declaration` matches it exactly (it was wired minimally in P7.0; extend for interface/namespace/enum). Interface `extends` is eaten in `AllowRegExp` (610), the one deliberate non-`Type` context. Interface id uses `getResWordOrIdentifier` (resword allowed); enum/namespace names use `parseBindingIdentifier`/`parseTSQualifiedName`.

---

## Task P7.5 — integration into the core JS grammar

**Goal:** make all TS productions reachable from real statements/expressions/classes/functions — replace every `// P7` marker. The corpus grows to cover realistic TS programs.

**Files:** `js/functions.rs`, `js/classes.rs`, `js/expressions.rs`, `js/statements.rs`, `js/modules.rs`, `js/mod.rs`; many corpus files.

**Integration sites (C++ JSParserImpl.cpp → Rust `// P7` marker):**

| C++ lines | Method | Rust marker | What to wire |
|---|---|---|---|
| 440-447 | parseFunctionHelper | `functions.rs:127` | `if parse_ts() && check(<)` → `parse_ts_type_parameters` |
| 488-498 | parseFunctionHelper | `functions.rs:163` | `if parse_ts() && check(:)` → `parse_type_annotation_ts(annotStart)` return type |
| 866-872 | parseDeclaration | `statements.rs:397` | TS declaration dispatch (done in P7.0 — verify) |
| 3184-3191 | (method def) | `expressions.rs:4052` | `if parse_ts() && check(<)` → `parse_ts_type_parameters` |
| 3761-3777 | parseOptionalCall | (find marker) | `if parse_ts() && check(<)` → `parse_ts_type_arguments` on optional-chain call |
| 3957-3975 | new-expr | shared (Flow done) | OR `parse_ts()` into the type-args gate → `parse_type_arguments` dispatcher |
| 4037-4062 | call tail | shared (Flow done) | OR `parse_ts()` into the type-args gate |
| 4162-4189 | parseUnaryExpression | `expressions.rs:2378` | `case <`: `if parse_ts() && !parse_jsx()` → `<Type>expr` `TSTypeAssertion` |
| 4321-4327 | parseBinaryExpression | (find `as` site) | `as_operator`: `if parse_ts()` → `TSAsExpression` (else Flow path) |
| 4855-4862 | parseClassDeclaration | `classes.rs:301` | class type params |
| 4933-4940 | parseClassExpression | `classes.rs:390` | class type params |
| 4978-4985 | parseClassTail | `classes.rs:431` | super-class `parse_ts_type_arguments` |
| 5110-5146 | parseClassBody | `classes.rs:584` | TS member modifiers (accessibility/readonly/static) |
| 5424-5428 | parseClassProperty | `classes.rs:945` | TS optional `?` flag |
| 5476-5500 | parseClassProperty | (find markers) | `TSModifiers` node on class property |
| 6405-6444 | parseAssignmentExpr | (find marker) | typed-arrow return type `parse_type_annotation_ts` |
| 6798-6805 | parseImportClause | `modules.rs:316` | `import type` kind |
| header 1240-1248 | parseTypeArguments | `js/mod.rs` dispatcher | TS arm → `parse_ts_type_arguments` |

- [ ] **Step 1 — dispatchers + type args.** Wire `parse_type_arguments` (js/mod.rs) TS arm and the shared new/call type-args gates (OR in `parse_ts()`). Corpus `ts_call_type_args.js`: `f<number>(x); new C<string>(); o?.m<T>();`. Differential green.
- [ ] **Step 2 — functions.** Type params + return type on function decls/exprs/methods. Corpus `ts_functions.js`: `function f<T>(x: T): T { return x; } const g = <T,>(x: T): T => x;`. Differential green.
- [ ] **Step 3 — classes.** Type params, super type-args, member modifiers (`public`/`private`/`protected`/`readonly`/`static`/`abstract`?), optional `?`, `TSModifiers`. Corpus `ts_classes.js`: `class C<T> extends B<T> implements I { private readonly x: number; m?(): void {} }`. (`implements` is the shared Flow/TS site — verify it OR's TS.) Differential green.
- [ ] **Step 4 — assertions + `as`.** `<Type>expr` type assertion + `expr as T`. Corpus `ts_assertions.js`: `const a = <string>x; const b = y as number; const c = z as const;`. (`as const` — verify the C++ path; ts.cpp `as` produces `TSAsExpression` with the rhs type.) Differential green.
- [ ] **Step 5 — variable & param annotations + import type.** Corpus `ts_annotations.js`: `let x: number = 1; function f(a: string, b?: T) {} import type { A } from 'm'; import type B from 'm';`. (Variable/binding annotations go through the shared `parseTypeAnnotation` at the binding sites already wired for Flow — verify they fire under `parse_ts()`.) Differential green.
- [ ] **Step 6 — verify no leak.** Re-run the FULL differential: `parser_corpus` (plain, 76 files) + all Flow corpora MUST be unchanged. Zero warnings, no new clippy.
- [ ] **Step 7 — commit** `rust(parser): P7.5 TS integration into core grammar (functions/classes/expressions/imports)`.

**Fidelity notes:** the type-assertion `>` eats in `AllowRegExp` (4170-4172), unique among TS `>` eats. `as`/typed-arrow/type-args sites are SHARED with Flow-ambiguous — the C++ gate is `(getParseFlowAmbiguous() || getParseTS())`; ensure the Rust OR's `parse_ts()` into the existing Flow-ambiguous condition rather than duplicating. The `import type` reverts to value-kind when the next token is `from` (the `'type'`-as-default-name case, JSParserImpl.cpp:6807-6811) — port that disambiguation.

---

## Task P7.6 — Capstone review

**Goal:** the whole-component review that caught real bugs in every prior phase.

- [ ] **Step 1 — `getParseTS()` site audit.** Grep the C++ for every `getParseTS()` and every `parseTS`/`parseTypeAnnotationTS` call; map each to its Rust production. Confirm NONE silently dropped (this caught the class-member `declare` modifier in P6). Confirm zero remaining `// P7` markers in `rust/crates/parser/src/`.
- [ ] **Step 2 — structural-fidelity check.** Grep `lib/Parser/JSParserImpl-ts.cpp` for `template <` (expect none) and confirm `IsConstructorType` stayed a Rust enum (not bool). Confirm every `advance`/`eat`/`checkAndEat`/`lookahead1` grammar-context + `RequireNoNewLine` arg matches the C++ default/explicit value (the recurring P5/P6 bug class).
- [ ] **Step 3 — default-args audit.** Re-verify against the header: `parseTypeAnnotationTS` wrapped-start, `lookahead1(None)`=`RequireNoNewLine` at ts.cpp:244/1264, `parseTSTypeArguments` `Type` context (header default analog).
- [ ] **Step 4 — corpus completeness.** Confirm the TS corpus exercises: conditional types, predicates, mapped/index signatures, parameter properties, all keyword types, enums (with/without init), namespaces (nested), interfaces (heritage + type params), `as`/`as const`, `<Type>` assertions, `import type`, class member modifiers, typed arrows. Add files for any gap.
- [ ] **Step 5 — final verify + roadmap/handoff update.** Full differential green (all corpora), `cargo build` zero warnings, `cargo clippy -p parser` no new lints, `generated_idempotent` green (no AST nodes added). Update `doc/superpowers/RustPortRoadmap.md` (P7 DONE block + table row) and `SESSION-HANDOFF.md` (next: JSX). Commit `doc(rust): JS Parser P7 complete — TypeScript; roadmap + handoff updated (next: JSX)`.

---

## Self-review (done at plan-write time)

- **Spec coverage:** all 27 ts.cpp methods are assigned to a task (P7.0 type-alias/declaration-dispatch + minimal annotation; P7.1 types.rs+params.rs 14 methods; P7.2 function_types 3; P7.3 object_types 3; P7.4 declarations 6). All 16 TS-only integration sites + 2 shared dispatchers → P7.5. ✓
- **No AST nodes:** confirmed — `node.rs` already contains every `TS*` node (984 `TS` hits); `generated_idempotent` is the guard. ✓
- **Dependency ordering:** type grammar (P7.1) before function/object types (P7.2/P7.3) which are honest-errored until landed; declarations (P7.4) depend on object-type members (P7.3) and params (P7.1); integration (P7.5) last. Mutually-recursive arms use honest errors between tasks, exactly as P5 did. ✓
- **Mutual exclusivity:** TS and Flow are separate dialects (`-parse-ts` XOR `-parse-flow`); the dispatchers branch on `parse_flow()` first, else TS. Do NOT enable both. ✓
