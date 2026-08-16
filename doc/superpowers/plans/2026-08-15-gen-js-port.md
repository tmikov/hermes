# AST → JS Generator Port Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the Rust port a complete AST → JavaScript source generator,
ported from juno's `gen_js.rs` and extended to the 106 node kinds juno's
generator does not print.

**Architecture:** A new `hermes-gen-js` crate. `generate()` takes a `NodeRc`
root and writes JS to a `&mut dyn Write`. Internally a `GenJS` struct holds the
output sink, indentation state, and options; a dispatch `match` over all 271
node kinds routes to one method per kind, grouped into topic modules.
Parenthesization is decided by a precedence table plus a `need_parens`
predicate that inspects the parent via `Path`.

**Tech Stack:** Rust 2021, no new external dependencies. Source of truth:
`unsupported/juno/crates/juno/src/gen_js.rs` (4174 lines, frozen).

**Spec:** `doc/superpowers/specs/2026-08-15-gen-js-port-design.md`. Read it
before Task 1; it records why juno and not C++ `AST2JS`, and why there is no
byte-exact oracle for this component.

## Global Constraints

Every task's requirements implicitly include this section.

1. **Never `cd` from the project root** (CLAUDE.md). Pass paths to commands.
   If unavoidable, use a subshell: `(cd dir; cmd)`.
2. **Commit directly to the `rust` branch.** Never open a PR, never merge.
3. **No new external dependencies.** The workspace stays at bumpalo-only.
   Adding a crate to `Cargo.toml` `[dependencies]` is a task failure.
4. **Exhaustive destructuring. Never `..` in a node pattern.** Every field is
   named, unused ones as `field: _`. A reviewer seeing `..` treats it as a
   defect. This is what makes AST field drift a compile error.
5. **No catch-all `_ =>` arm in the kind dispatch after Task 13.** A temporary
   one is permitted during Tasks 1–12 (see Task 1, Step 4); Task 13 deletes it
   and the build must pass without it. Any task that leaves the crate
   uncompilable is a failure — the temporary arm exists precisely so each task
   lands green.
6. **Identifiers are emitted via `gc.try_bytes_str(atom)`**, never
   `gc.bytes()` (WTF-8 surrogate pairs would emit invalid UTF-8) and never
   `bytes_str_lossy` (U+FFFD substitution would emit a different program).
   `None` → the unsupported-content error.
7. **String literals are escaped through UTF-16 code units** obtained from
   `hermes_support::utf8::convert_utf8_with_surrogates_to_utf16`
   (`crates/support/src/utf8.rs:175`), not from `&str`.
8. **Copyright header** at the top of every new file:
   ```rust
   /*
    * Copyright (c) Meta Platforms, Inc. and affiliates.
    *
    * This source code is licensed under the MIT license found in the
    * LICENSE file in the root directory of this source tree.
    */
   ```
9. **Doc comment on every declaration** (project code style).
10. **Cite juno source lines** in the form `// juno gen_js.rs:1299-1320` on
    each ported method. These are not checked by the citation tool (juno is
    frozen and `.rs` paths cancel the C++ citation context by design), but
    they are how a reviewer finds the original.
11. **Never run `git checkout --` or `git clean`** to undo experiments; they
    have destroyed uncommitted work in this repo before. Commit first, or use
    `git stash`.
12. Run `cargo test` from the workspace manifest:
    `cargo test --manifest-path rust/Cargo.toml -p hermes-gen-js`.

## Adaptation Rules (apply to every ported arm)

These convert juno's code to ours. They are mechanical; deviations need a
reason in the commit message.

| juno | ours |
|---|---|
| `child.visit(ctx, self, Some(path))` | `self.gen_node(ctx, child, Some(path))` |
| `impl Visitor for GenJS` (`gen_js.rs:4000`) | **deleted** — our `Visitor` trait has a different signature (`visit_node(&mut self, node)`, no ctx/path) and GenJS never uses the default child-walk |
| `node.field` (plain) | `node.field.get()` (our fields are `Cell`) |
| `ctx.str(atom)` for an identifier | `gc.try_bytes_str(atom)` (constraint 6) |
| `ctx.str_u16(value)` | `convert_utf8_with_surrogates_to_utf16(gc.bytes(value))` |
| `convert::number_to_string(v)` | `hermes_support::number_to_string(v)` (Task 1 makes it public) |
| `out_token!(self, node, ...)` | `out!(self, ...)` — the sourcemap is dropped (spec §6) |
| `self.add_segment(node)` | **deleted** (5 sites) |
| `Node::Module(..)` arm | **deleted** — we have no `Module` kind |
| `_ => unimplemented!(...)` | **deleted in Task 13** (constraint 5) |

`contains_call`'s `CallFinder` (`gen_js.rs:4155-4174`) is the one visitor that
*does* map onto our `Visitor` trait — it needs neither ctx nor path. Port it as
`fn visit_node(&mut self, node)` + `node.visit_children(self)`.

## File Structure

```
rust/crates/gen_js/
  Cargo.toml
  src/
    lib.rs          public API: generate(), Opt, Pretty, QuoteChar,
                    Annotation, GenJsError; crate-level docs
    gen.rs          GenJS struct, output primitives, gen_root, doc_block
    precedence.rs   precedence table, Assoc, NeedParens, ChildPos,
                    get_precedence, need_parens, root/expr_starts_with,
                    is_unary_op, stmt_skip_semi, contains_call
    dispatch.rs     the exhaustive match over all 271 kinds → one method each
    arms/
      literal.rs    literals, identifiers, templates, string escaping
      expr.rs       ES expressions
      stmt.rs       ES statements, patterns, block/stmt-list helpers
      func.rs       functions, classes, methods, properties
      module.rs     import/export
      jsx.rs        JSX
      flow_type.rs  Flow type annotations
      flow_decl.rs  Flow declarations, object types, enums
      newer.rs      the 53 ES/Flow kinds juno lacks
      ts.rs         the 46 TypeScript kinds
    annotate.rs     Annotation::Sem support
  tests/
    roundtrip.rs    juno's ported unit cases + the Tier 1 corpus gate
    exhaustive.rs   every kind is reachable; no kind returns Unsupported
                    except the 7 internal ones
```

**Why dispatch-to-method rather than juno's single 2800-line `match`:** the
files above are reviewable in isolation, and the user has asked for a serious
review. Exhaustiveness is preserved because `dispatch.rs` still matches every
kind; the per-kind method destructures exhaustively inside. This is a
deliberate divergence from juno's shape and the only structural one.

---

### Task 1: Crate scaffold, `number_to_string`, and the dispatch skeleton

**Files:**
- Create: `rust/crates/gen_js/Cargo.toml`, `rust/crates/gen_js/src/lib.rs`,
  `rust/crates/gen_js/src/dispatch.rs`
- Modify: `rust/Cargo.toml:3` (workspace `members`)
- Modify: `rust/crates/support/src/lib.rs`,
  `rust/crates/support/src/json_emitter.rs:19`
- Test: `rust/crates/gen_js/tests/exhaustive.rs`

**Interfaces:**
- Produces: `hermes_support::number_to_string(f64) -> String`;
  `hermes_gen_js::{generate, Opt, Pretty, QuoteChar, GenJsError}`;
  `GenJS::gen_node(&mut self, ctx, node, path)` as the dispatch entry.

- [ ] **Step 1: Lift `number_to_string` to the public support API**

It is currently private in `crates/support/src/json_emitter.rs:19` and is
already covered by `number_to_string_matches_ecmascript`
(`json_emitter.rs:591`). Make it `pub` and re-export it from
`crates/support/src/lib.rs`. Do **not** copy it into the new crate — one
implementation, one test.

- [ ] **Step 2: Run the existing support tests to confirm nothing moved**

Run: `cargo test --manifest-path rust/Cargo.toml -p hermes-support`
Expected: PASS, including `number_to_string_matches_ecmascript`.

- [ ] **Step 3: Create the crate and wire it into the workspace**

`Cargo.toml` mirrors the sibling crates' style (see
`rust/crates/sema/Cargo.toml` for the `package = "hermes-…"` convention):
name `hermes-gen-js`, version `0.1.0`, edition 2021, dependencies on
`hermes-ast`, `hermes-support`, `hermes-sema` by path with `package =`
renaming. Append `"crates/gen_js"` to `members` in `rust/Cargo.toml:3`.

- [ ] **Step 4: Write the dispatch skeleton with the TEMPORARY catch-all**

`dispatch.rs` gets `fn gen_node(&mut self, ctx: &GCLock, node: &Node, path: Option<Path>)`
containing a `match node` with the 7 internal kinds (spec §4) returning the
unsupported-kind error, and — for now only — a catch-all:

```rust
// TEMPORARY (plan Task 1, deleted in Task 13). Until every kind has an arm,
// this keeps the crate compiling so each task can land green. Task 13
// deletes it; the compiler then proves all 271 kinds are handled.
_ => self.unsupported_kind(node),
```

- [ ] **Step 5: Write the exhaustiveness test**

```rust
/// Task 13 deletes the temporary catch-all in `dispatch.rs`. Until then this
/// test records that the deletion has not happened yet, so nobody can forget.
#[test]
fn temporary_catch_all_is_gone() {
    let src = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/src/dispatch.rs"),
    )
    .expect("dispatch.rs is readable");
    assert!(
        !src.contains("_ => self.unsupported_kind(node)"),
        "the temporary catch-all from Task 1 is still present; Task 13 must \
         delete it so the compiler proves all 271 kinds are handled"
    );
}
```

- [ ] **Step 6: Run it and confirm it FAILS**

Run: `cargo test --manifest-path rust/Cargo.toml -p hermes-gen-js temporary_catch_all_is_gone`
Expected: FAIL, message naming the catch-all. Mark it `#[ignore]` with the
reason `"un-ignored by Task 13"` so the suite is green meanwhile — Task 13
removes the `#[ignore]`.

- [ ] **Step 7: Build and commit**

Run: `cargo build --manifest-path rust/Cargo.toml -p hermes-gen-js`

```bash
git add rust/Cargo.toml rust/crates/gen_js rust/crates/support
git commit -m "rust(gen_js): crate scaffold, public number_to_string, dispatch skeleton"
```

---

### Task 2: Core machinery — options, `GenJS`, output primitives

**Files:**
- Modify: `rust/crates/gen_js/src/lib.rs`
- Create: `rust/crates/gen_js/src/gen.rs`
- Test: `rust/crates/gen_js/tests/roundtrip.rs` (created here, grown later)

**Interfaces:**
- Consumes: Task 1's dispatch entry.
- Produces: `generate(out, ctx, root, opt) -> Result<(), GenJsError>`;
  `Opt { pretty, annotation, force_async_arrow_space, doc_block, quote }`;
  `GenJS` with `out!`, `write_ascii`, `write_utf8`, `write_char`,
  `inc_indent`, `dec_indent`, `comma`, `space`, `newline`, `force_newline`,
  `force_newline_without_indent`.

- [ ] **Step 1: Port the option types**

`gen_js.rs:26-108` — `Opt`, its `Default` (`Pretty::Yes`, `Annotation::No`,
`force_async_arrow_space: true`, `doc_block: None`, `QuoteChar::Single`),
`Pretty`, `QuoteChar`, `QuoteChar::as_char`, `Assoc`. Keep every option
(spec §3).

- [ ] **Step 2: Port `GenJS` and the output primitives**

`gen_js.rs:226-360` (struct, `gen_root`, `write_ascii`, `write_char`,
`write_utf8`) and `gen_js.rs:3196-3248` (`inc_indent`, `dec_indent`, `comma`,
`space`, `newline`, `force_newline`, `force_newline_without_indent`).

Drop the `sourcemap`/`cur_token` fields and `flush_cur_token`
(`gen_js.rs:3944`) per spec §6. Keep the deferred-error field: juno records
the first `io::Error` and stops writing rather than propagating from every
call site (`gen_js.rs:353-357`); preserve that.

- [ ] **Step 3: Port the `doc_block` preamble**

`gen_js.rs:291-300` — emitted before anything else, with `\n` mapped to
`force_newline_without_indent`.

- [ ] **Step 4: Define the error type**

```rust
/// Why generation failed.
#[derive(Debug)]
pub enum GenJsError {
    /// The sink returned an error.
    Io(std::io::Error),
    /// A node kind that has no source syntax reached the generator — a cover
    /// node, `SHBuiltin`, or `ImplicitCheckedCast` (see the crate docs).
    UnsupportedKind(&'static str),
    /// An identifier's bytes contain an unpaired surrogate, which has no JS
    /// spelling.
    UnrepresentableIdentifier,
}
```

Implement `Display` and `std::error::Error`. **Do not panic and do not
`abort()`** — spec §4.

- [ ] **Step 5: Write the smoke test**

```rust
#[test]
fn empty_program_generates_empty_output() {
    let parsed = hermes_parser::parse("", Default::default()).expect("parses");
    let js = gen(&parsed, Opt::default());
    assert_eq!(js, "");
}
```

with a `fn gen(parsed: &mut ParsedJS, opt: Opt) -> String` helper that calls
`generate` into a `Vec<u8>`. Add `hermes-parser` as a `[dev-dependencies]`
entry (a dev-dependency on a workspace sibling is not a new external
dependency).

- [ ] **Step 6: Run it**

Run: `cargo test --manifest-path rust/Cargo.toml -p hermes-gen-js`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add rust/crates/gen_js
git commit -m "rust(gen_js): options, GenJS, output primitives"
```

---

### Task 3: Precedence and parenthesization

**Files:**
- Create: `rust/crates/gen_js/src/precedence.rs`
- Test: same file, `#[cfg(test)] mod tests`

**Interfaces:**
- Produces: `get_precedence`, `need_parens`, `root_starts_with`,
  `expr_starts_with`, `print_child`, `print_parens`, `print_comma_expression`,
  `is_unary_op`, `stmt_skip_semi`, `contains_call`, `NeedParens`, `ChildPos`.

This is the task where a JS printer is most often silently wrong (spec §8), so
it is deliberately separated from the arms that use it.

- [ ] **Step 1: Port the precedence module**

`gen_js.rs:110-215` — the `precedence` constants, `get_binary_precedence`,
`get_logical_precedence`, `NeedParens`, `From<bool> for NeedParens`,
`ChildPos`, `ForceSpace`.

- [ ] **Step 2: Port the decision functions**

`gen_js.rs:3590-3684` (`get_precedence`), `gen_js.rs:3685-3823`
(`need_parens`), `gen_js.rs:3824-3926` (`root_starts_with`,
`expr_starts_with`), `gen_js.rs:4106-4174` (`is_unary_op`, `stmt_skip_semi`,
`contains_call`).

`need_parens` and `get_precedence` reference kinds by name; where a kind does
not exist in our AST, delete that alternative rather than inventing a mapping,
and note it in the commit message.

- [ ] **Step 3: Port the printing helpers**

`gen_js.rs:3249-3299` — `print_child`, `print_comma_expression`,
`print_parens`.

- [ ] **Step 4: Write the precedence table tests**

Assert the ordering relationships the table encodes rather than the numeric
values (which are arbitrary): `SEQ < ARROW < YIELD < ASSIGN`, `get_binary_precedence`
for `+` is below `*`, `**` is right-associative, and every
`BinaryExpressionOperator` and `LogicalExpressionOperator` variant maps to
some precedence (a `match` with no wildcard, so a new operator is a compile
error).

- [ ] **Step 5: Run**

Run: `cargo test --manifest-path rust/Cargo.toml -p hermes-gen-js precedence`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git commit -m "rust(gen_js): precedence table and parenthesization decisions"
```

---

### Task 4: Literals, identifiers, templates, and string escaping

**Files:**
- Create: `rust/crates/gen_js/src/arms/literal.rs`, `rust/crates/gen_js/src/arms/mod.rs`
- Modify: `rust/crates/gen_js/src/dispatch.rs`

**Interfaces:**
- Consumes: Task 2's primitives, Task 3's helpers.
- Produces: arms for `BooleanLiteral`, `NullLiteral`, `StringLiteral`,
  `NumericLiteral`, `BigIntLiteral`, `RegExpLiteral`, `ThisExpression`,
  `Super`, `Identifier`, `PrivateName`, `MetaProperty`, `Directive`,
  `DirectiveLiteral`, `TemplateLiteral`, `TaggedTemplateExpression`,
  `TemplateElement`; plus `print_escaped_string_literal`.

- [ ] **Step 1: Port the arms**

`gen_js.rs:828-867` (literals through `Super`), `gen_js.rs:1260-1266`
(directives), `gen_js.rs:1299-1334` (`Identifier`, `PrivateName`,
`MetaProperty`), `gen_js.rs:1388-1441` (templates).

`Identifier`'s `self.annotate_identifier(ctx, node)` call
(`gen_js.rs:1307`) is stubbed to a no-op here; Task 14 fills it in.

- [ ] **Step 2: Port `print_escaped_string_literal` with our UTF-16 source**

`gen_js.rs:3300-3351`, replacing `ctx.str_u16(value)` per the adaptation
table. The escape set (`\\`, `\b`, `\f`, `\n`, `\r`, `\t`, `\v`, the active
quote) and the "printable is `0x20..=0x7f`, everything else is `\u{:04x}`"
rule carry over unchanged.

- [ ] **Step 3: Write the encoding tests** (both halves of spec §5)

```rust
/// An astral identifier is legal JS and our atoms hold it as a WTF-8
/// surrogate PAIR, so emitting raw atom bytes would produce invalid UTF-8.
#[test]
fn astral_identifier_round_trips_as_valid_utf8() {
    let parsed = hermes_parser::parse("var \u{1D465} = 1;", Default::default())
        .expect("astral identifiers parse");
    let js = gen(&parsed, Opt::default());
    assert!(js.contains('\u{1D465}'), "{js}");
    assert!(std::str::from_utf8(js.as_bytes()).is_ok());
}

/// A lone surrogate is a legal JS string value with no literal spelling; it
/// must survive as exactly one \udXXX escape, not three U+FFFD.
#[test]
fn lone_surrogate_string_literal_survives_as_one_escape() {
    let parsed = hermes_parser::parse(r#"var s = "\uD800";"#, Default::default())
        .expect("parses");
    let js = gen(&parsed, Opt::default());
    assert!(js.contains("\\ud800"), "{js}");
    assert_eq!(js.matches('\u{FFFD}').count(), 0, "{js}");
}
```

- [ ] **Step 4: Run, then commit**

Run: `cargo test --manifest-path rust/Cargo.toml -p hermes-gen-js`
Expected: PASS.

```bash
git commit -m "rust(gen_js): literals, identifiers, templates, string escaping"
```

---

### Task 5: ES expressions

**Files:**
- Create: `rust/crates/gen_js/src/arms/expr.rs`
- Modify: `rust/crates/gen_js/src/dispatch.rs`

**Interfaces:**
- Produces: arms for `SequenceExpression`, `ObjectExpression`,
  `ArrayExpression`, `SpreadElement`, `NewExpression`, `YieldExpression`,
  `AwaitExpression`, `ImportExpression`, `CallExpression`,
  `OptionalCallExpression`, `AssignmentExpression`, `UnaryExpression`,
  `UpdateExpression`, `MemberExpression`, `OptionalMemberExpression`,
  `BinaryExpression`, `LogicalExpression`, `ConditionalExpression`,
  `Property`, `visit_props`.

- [ ] **Step 1: Port `gen_js.rs:868-1259`, `1267-1298`, `1442-1552`, and `3353-3364` (`visit_props`)**

Note `MemberExpression`'s numeric-literal special case
(`gen_js.rs:1168-1173`): `50..toString()` needs the extra `.` unless the
printed number already contains `e`, `E`, or `.`. Keep the comment.

- [ ] **Step 2: Write round-trip tests for the parenthesization-sensitive cases**

At minimum, one `test_roundtrip` case each for: `(a, b) => c`,
`a ** b ** c`, `(a + b) * c`, `a ?? (b || c)`, `new (a.b())()`,
`(function(){})()`, `({}).x`, `(a = b) => c`, `a?.b?.()`, and
`50..toString()`. Use the harness from Task 15 if it exists; otherwise
compare generated output to a literal expected string and convert to
round-trip form in Task 15.

- [ ] **Step 3: Run, then commit**

```bash
git commit -m "rust(gen_js): ES expression arms"
```

---

### Task 6: ES statements and patterns

**Files:**
- Create: `rust/crates/gen_js/src/arms/stmt.rs`
- Modify: `rust/crates/gen_js/src/dispatch.rs`

**Interfaces:**
- Produces: arms for `Program`, `WhileStatement`, `DoWhileStatement`,
  `ForInStatement`, `ForOfStatement`, `ForStatement`, `DebuggerStatement`,
  `EmptyStatement`, `BlockStatement`, `BreakStatement`, `ContinueStatement`,
  `ThrowStatement`, `ReturnStatement`, `WithStatement`, `SwitchStatement`,
  `SwitchCase`, `LabeledStatement`, `ExpressionStatement`, `TryStatement`,
  `IfStatement`, `CatchClause`, `VariableDeclaration`, `VariableDeclarator`,
  `ObjectPattern`, `ArrayPattern`, `RestElement`, `AssignmentPattern`, `Empty`,
  `Metadata`; plus `visit_stmt_or_block`, `visit_stmt_list`,
  `visit_stmt_in_block`.

- [ ] **Step 1: Port `gen_js.rs:364-372`, `521-827`, `1335-1387`, `1942-1999`, and `3525-3589`**

Skip the `Node::Module` arm (`gen_js.rs:370-373`) — we have no such kind.

- [ ] **Step 2: Write the ASI and dangling-else tests**

Round-trip cases covering: `if (a) if (b) c(); else d();` (else binds to the
inner `if`), `for (var i = (a in b);;);` (the `in` must be parenthesized),
`do x(); while (y)`, a labeled statement wrapping a block, and an expression
statement beginning with `(`, `[`, `function`, `class`, and `let[`. Run each
under **both** `Pretty::Yes` and `Pretty::No` — `Pretty::No` is where ASI
hazards bite (spec §8.2).

- [ ] **Step 3: Run, then commit**

```bash
git commit -m "rust(gen_js): ES statement and pattern arms"
```

---

### Task 7: Functions, classes, methods, properties

**Files:**
- Create: `rust/crates/gen_js/src/arms/func.rs`
- Modify: `rust/crates/gen_js/src/dispatch.rs`

**Interfaces:**
- Produces: arms for `FunctionExpression`, `FunctionDeclaration`,
  `ArrowFunctionExpression`, `ClassExpression`, `ClassDeclaration`,
  `ClassBody`, `ClassProperty`, `ClassPrivateProperty`, `MethodDefinition`;
  plus `visit_func_params_body`, `visit_func_type_params`.

- [ ] **Step 1: Port `gen_js.rs:374-520`, `1553-1795`, `3365-3453`**

`force_async_arrow_space` is consumed at `gen_js.rs:437-441`; keep the
comment explaining why the space matters to downstream transforms.

The single-parameter arrow shortcut (`gen_js.rs:461-476`) omits the parens
only when the sole param is an `Identifier` with no type annotation and not
optional, **and** the body is an expression or we are not pretty-printing.
Port the condition exactly — it is easy to over-simplify and produce
`a: T => x`.

- [ ] **Step 2: Write round-trip tests**

`async x => x`, `async (x) => x` under `force_async_arrow_space` both ways,
`(a) => ({})` (object literal body needs parens), a class with a computed
method name, a getter, a setter, a static method, `#private` fields, and a
class expression in expression-statement position.

- [ ] **Step 3: Run, then commit**

```bash
git commit -m "rust(gen_js): function, class, and method arms"
```

---

### Task 8: Modules

**Files:**
- Create: `rust/crates/gen_js/src/arms/module.rs`
- Modify: `rust/crates/gen_js/src/dispatch.rs`

**Interfaces:**
- Produces: arms for `ImportDeclaration`, `ImportSpecifier`,
  `ImportDefaultSpecifier`, `ImportNamespaceSpecifier`, `ImportAttribute`,
  `ExportNamedDeclaration`, `ExportSpecifier`, `ExportNamespaceSpecifier`,
  `ExportDefaultDeclaration`, `ExportAllDeclaration`.

- [ ] **Step 1: Port `gen_js.rs:1796-1941`**

- [ ] **Step 2: Round-trip tests** — default + named in one declaration,
  `export * as ns from`, `import x, {y as z} from`, an import attribute
  (`with { type: "json" }`), `export default function(){}` vs
  `export default (function(){})`.

- [ ] **Step 3: Run, then commit**

```bash
git commit -m "rust(gen_js): import/export arms"
```

---

### Task 9: JSX

**Files:**
- Create: `rust/crates/gen_js/src/arms/jsx.rs`
- Modify: `rust/crates/gen_js/src/dispatch.rs`

**Interfaces:**
- Produces: arms for the 14 JSX kinds (`gen_js.rs:2000-2160`).

- [ ] **Step 1: Port `gen_js.rs:2000-2160`**

`JSXText` (`gen_js.rs:2102`) and `JSXStringLiteral` (`gen_js.rs:2088`) have
their own escaping rules distinct from `print_escaped_string_literal` — port
them as written, do not unify.

- [ ] **Step 2: Round-trip tests** using `ParseFlags { parse_jsx: true, .. }`
  — a self-closing element, a namespaced name, a member-expression name, a
  spread attribute, a fragment, an expression container, and text containing
  `{`, `}`, `<`, `&`, and a newline.

- [ ] **Step 3: Run, then commit**

```bash
git commit -m "rust(gen_js): JSX arms"
```

---

### Task 10: Flow type annotations

**Files:**
- Create: `rust/crates/gen_js/src/arms/flow_type.rs`
- Modify: `rust/crates/gen_js/src/dispatch.rs`

**Interfaces:**
- Produces: arms for `gen_js.rs:2161-2430` — the primitive type annotations,
  `FunctionTypeAnnotation`, `FunctionTypeParam`, `NullableTypeAnnotation`,
  `QualifiedTypeIdentifier`, `TypeofTypeAnnotation`, `TupleTypeAnnotation`,
  `ArrayTypeAnnotation`, `UnionTypeAnnotation`, `IntersectionTypeAnnotation`,
  `GenericTypeAnnotation`, `IndexedAccessType`, `OptionalIndexedAccessType`,
  `InterfaceTypeAnnotation`.

- [ ] **Step 1: Port `gen_js.rs:2161-2430`**

Union and intersection consult `need_parens` via the `ExistsTypeAnnotation` /
`NullableTypeAnnotation` / `UnionTypeAnnotation` / `IntersectionTypeAnnotation`
alternatives at `gen_js.rs:3661-3684` — verify those landed in Task 3.

- [ ] **Step 2: Round-trip tests** with `ParseFlags { parse_flow: true, .. }`
  — `?(a | b)`, `(a & b) | c`, a function type with rest and optional params,
  `typeof x.y`, `Array<?string>`, `A['b']['c']`, and an inline object type.

- [ ] **Step 3: Run, then commit**

```bash
git commit -m "rust(gen_js): Flow type annotation arms"
```

---

### Task 11: Flow declarations, object types, and enums

**Files:**
- Create: `rust/crates/gen_js/src/arms/flow_decl.rs`
- Modify: `rust/crates/gen_js/src/dispatch.rs`

**Interfaces:**
- Produces: arms for `gen_js.rs:2431-3195` — `TypeAlias`, `OpaqueType`,
  `InterfaceDeclaration`, `DeclareOpaqueType`, `DeclareClass`,
  `DeclareFunction`, `DeclareVariable`, `DeclareExportDeclaration`,
  `DeclareExportAllDeclaration`, `DeclareModule`, `DeclareModuleExports`,
  `InterfaceExtends`, `TypeAnnotation`, `ObjectTypeAnnotation` and its five
  member kinds, `Variance`, `TypeParameterDeclaration`, `TypeParameter`,
  `TypeCastExpression`, `InferredPredicate`, `DeclaredPredicate`,
  `EnumDeclaration` and its bodies/members; plus `visit_interface`,
  `visit_enum_body`.

- [ ] **Step 1: Port `gen_js.rs:2431-3195`, `3454-3524`**

- [ ] **Step 2: Round-trip tests** — `declare class` with an extends clause,
  `declare module "x" { declare export default T }`, an opaque type with a
  supertype, an object type with an indexer + internal slot + call property +
  spread, a variance-annotated type parameter with a default, and each enum
  body kind including a defaulted member.

- [ ] **Step 3: Run, then commit**

```bash
git commit -m "rust(gen_js): Flow declaration, object type, and enum arms"
```

---

### Task 12: The 53 ES/Flow kinds juno lacks

**Files:**
- Create: `rust/crates/gen_js/src/arms/newer.rs`
- Modify: `rust/crates/gen_js/src/dispatch.rs`

**Interfaces:**
- Produces: arms for the 53 kinds listed in spec §4 under "53 ES/Flow".

**There is no juno source for these.** Derive the syntax from our parser's own
productions — the parse function that builds each node is the specification
for what must be emitted. Find it with
`grep -rn "NodeKind" rust/crates/parser/src/js/`.

- [ ] **Step 1: Port the ES-level kinds first**

`StaticBlock` (`class { static { … } }`, plain ES2022), `Decorator`,
`AsExpression`, `AsConstExpression`.

- [ ] **Step 2: The Flow `match` family** — 18 kinds
  (`MatchExpression`, `MatchStatement`, their cases, and the 13 pattern
  kinds). Parsed under `ParseFlags { parse_flow_match: true, .. }`.

- [ ] **Step 3: The record family** — 6 kinds, under `parse_flow_records`.

- [ ] **Step 4: Component/hook** — `ComponentDeclaration`,
  `ComponentParameter`, `ComponentTypeAnnotation`, `ComponentTypeParameter`,
  `DeclareComponent`, `DeclareHook`, `HookDeclaration`, `HookTypeAnnotation`,
  under `parse_flow_component_syntax`.

- [ ] **Step 5: The remaining type kinds** — `ConditionalTypeAnnotation`,
  `InferTypeAnnotation`, `KeyofTypeAnnotation`, `NeverTypeAnnotation`,
  `UndefinedTypeAnnotation`, `UnknownTypeAnnotation`, `TypeOperator`,
  `TypePredicate`, `ObjectTypeMappedTypeProperty`, `QualifiedTypeofIdentifier`,
  `TupleTypeLabeledElement`, `TupleTypeSpreadElement`, `DeclareEnum`,
  `DeclareNamespace`, `EnumBigIntBody`, `EnumBigIntMember`.

- [ ] **Step 6: One round-trip test per kind**

53 kinds, 53 named test cases. A kind with no test is not done. Where a kind
also appears in a Tier 1 corpus file, say so in a comment rather than skipping
the test — the corpus is a sweep, not a substitute for a named case.

- [ ] **Step 7: Run, then commit**

```bash
git commit -m "rust(gen_js): the 53 ES/Flow kinds juno's generator lacks"
```

---

### Task 13: TypeScript, and deleting the catch-all

**Files:**
- Create: `rust/crates/gen_js/src/arms/ts.rs`
- Modify: `rust/crates/gen_js/src/dispatch.rs`,
  `rust/crates/gen_js/tests/exhaustive.rs`

**Interfaces:**
- Produces: arms for the 46 TS kinds in spec §4.

- [ ] **Step 1: Port the 46 TS arms**

Same method as Task 12: our parser's TS productions are the specification.
Grouped roughly as keywords (`TSAnyKeyword` … `TSVoidKeyword`), type
constructors (`TSArrayType`, `TSUnionType`, `TSIntersectionType`,
`TSTupleType`, `TSConditionalType`, `TSIndexedAccessType`, `TSTypeQuery`,
`TSLiteralType`, `TSTypeReference`, `TSQualifiedName`, `TSTypeOperator`
equivalents), declarations (`TSInterfaceDeclaration`, `TSInterfaceBody`,
`TSInterfaceHeritage`, `TSEnumDeclaration`, `TSEnumMember`,
`TSTypeAliasDeclaration`, `TSModuleDeclaration`, `TSModuleBlock`,
`TSModuleMember`), signatures (`TSCallSignatureDeclaration`,
`TSIndexSignature`, `TSMethodSignature`, `TSPropertySignature`,
`TSParameterProperty`, `TSModifiers`), and expressions/annotations
(`TSAsExpression`, `TSTypeAssertion`, `TSTypeAnnotation`, `TSTypePredicate`,
`TSFunctionType`, `TSConstructorType`, `TSTypeLiteral`, `TSTypeParameter`,
`TSTypeParameterDeclaration`, `TSTypeParameterInstantiation`).

- [ ] **Step 2: One round-trip test per kind** — 46 named cases, under
  `ParseFlags { parse_ts: true, .. }`. Same rule as Task 12: no test, not done.

- [ ] **Step 3: DELETE the temporary catch-all**

Remove the `_ => self.unsupported_kind(node)` arm from `dispatch.rs` (Task 1,
Step 4). The 7 internal kinds keep their explicit arms returning
`UnsupportedKind`.

- [ ] **Step 4: Build — the compiler now proves exhaustiveness**

Run: `cargo build --manifest-path rust/Cargo.toml -p hermes-gen-js`
Expected: PASS. **If it fails with a non-exhaustive-match error, that is the
task working as designed** — add the named arms it lists. Do not restore the
catch-all.

- [ ] **Step 5: Un-ignore the exhaustiveness test**

Remove `#[ignore]` from `temporary_catch_all_is_gone`.

Run: `cargo test --manifest-path rust/Cargo.toml -p hermes-gen-js`
Expected: PASS, including that test.

- [ ] **Step 6: Commit**

```bash
git commit -m "rust(gen_js): TypeScript arms; delete the catch-all so kinds are compile-checked"
```

---

### Task 14: Sema annotation

**Files:**
- Create: `rust/crates/gen_js/src/annotate.rs`
- Modify: `rust/crates/gen_js/src/arms/literal.rs` (the Task 4 stub),
  `rust/crates/gen_js/src/lib.rs`

**Interfaces:**
- Consumes: `hermes_sema::{SemContext, DeclKind, Resolution}`.
- Produces: `Annotation<'s> { No, Sem(&'s SemContext) }`,
  `GenJS::annotate_identifier`.

- [ ] **Step 1: Port `Annotation` and `annotate_identifier`**

juno's `gen_js.rs:212-215` and its `annotate_identifier` (find it by grepping
the file). Adapt to our `SemContext` API — check
`rust/crates/sema/examples/print_bindings.rs` for the current way to resolve
an identifier to its declaration, since that example is maintained.

- [ ] **Step 2: Test**

Use this source, whose three bindings `print_bindings.rs` already resolves to
`Let`, `Parameter`, and `UndeclaredGlobalProperty` respectively:

```rust
#[test]
fn sem_annotation_labels_each_binding() {
    let src = "let counter = 0; function f(step) { console.log(counter, step); }";
    // parse, resolve, then generate with Annotation::Sem.
    let js = gen_with_sem(src);
    // The exact annotation syntax is juno's; assert each decl kind appears
    // attached to its identifier, and that Annotation::No output for the same
    // source contains none of them.
    assert!(js.contains("Let"), "{js}");
    assert!(js.contains("Parameter"), "{js}");
    assert!(js.contains("UndeclaredGlobalProperty"), "{js}");
    assert!(!gen_plain(src).contains("Parameter"));
}
```

If juno's annotation spelling differs from the bare kind names, adjust the
assertions to match what juno emits — but keep the `Annotation::No` negative
assertion, which is what proves the option is actually doing something.

- [ ] **Step 3: Run, then commit**

```bash
git commit -m "rust(gen_js): sema annotation support"
```

---

### Task 15: The round-trip harness and the Tier 1 corpus gate

**Files:**
- Modify: `rust/crates/gen_js/tests/roundtrip.rs`
- Create: `rust/crates/gen_js/tests/corpus.rs`

**Interfaces:**
- Produces: `fn test_roundtrip(src: &str)`,
  `fn test_roundtrip_with_flags(flags: ParseFlags, src: &str)`.

- [ ] **Step 1: Port juno's harness**

`unsupported/juno/crates/juno/tests/gen_js/mod.rs:13-70` —
`do_gen`, `test_roundtrip_with_flags`, `test_roundtrip`,
`test_roundtrip_flow`. Our version compares
`ParsedJS::to_estree_json(true)` with `raw` omitted (spec §7.2), and runs
both `Pretty::Yes` and `Pretty::No`.

- [ ] **Step 2: Port juno's case list**

The remaining ~680 lines of `tests/gen_js/mod.rs` are `test_roundtrip("…")`
calls. Port them all. Cases naming a kind we lack get deleted with a note in
the commit message; cases that fail get investigated, not deleted — a failure
here is the inherited juno bug this whole plan exists to find.

- [ ] **Step 3: Write the Tier 1 corpus gate**

Iterate the 11 checked-in corpus directories (spec §7.2's table), mapping
directory name → `ParseFlags` the way
`rust/crates/sema/tests/facade_agreement.rs:59-102` maps its `// FLAGS:`
lines. For each file, both `Pretty` modes, assert the round trip. Report
**all** failures with file names, not just the first.

- [ ] **Step 4: Run and record**

Run: `cargo test --manifest-path rust/Cargo.toml -p hermes-gen-js`
Expected: 420 files pass. **If some fail, do not weaken the gate.** Record
each failure, fix the generator, and note the bug in the commit message. If a
failure turns out to be a parser bug rather than a generator bug, stop and
report it — that is a finding about a shipped crate.

- [ ] **Step 5: Prove the gate can fail**

Temporarily break one parenthesization rule in `precedence.rs`, run the gate,
and record which **named** test fails and its message. Restore the rule. Put
the evidence in the commit message. A gate that has never failed has not been
shown to test anything.

- [ ] **Step 6: Commit**

```bash
git commit -m "rust(gen_js): round-trip harness and the 420-file Tier 1 corpus gate"
```

---

### Task 16: Façade, example, and docs

**Files:**
- Modify: `rust/crates/parser/src/facade.rs`, `rust/crates/gen_js/src/lib.rs`
- Create: `rust/crates/gen_js/README.md`,
  `rust/crates/gen_js/examples/print_js.rs`

**Interfaces:**
- Produces: `ParsedJS::to_js(&mut self, opt: Opt) -> Result<String, GenJsError>`.

- [ ] **Step 1: Resolve the dependency direction, then add the entry point**

`hermes-gen-js` depends on `hermes-ast`; `hermes-parser` would need
`hermes-gen-js` to host `to_js` as an inherent method. Check whether that
creates a cycle:

Run: `cargo tree --manifest-path rust/Cargo.toml -p hermes-gen-js`

If `hermes-parser` is already a (non-dev) dependency of `hermes-gen-js`, the
inherent method is impossible — ship instead a free function in
`hermes-gen-js`:

```rust
/// Regenerate JS source from a parsed program.
pub fn to_js(parsed: &mut ParsedJS, opt: Opt) -> Result<String, GenJsError>
```

Either way the call site must be one line for a user who has a `ParsedJS`.
State which shape shipped, and why, in the crate docs.

- [ ] **Step 2: Write `examples/print_js.rs`** — parse a source string, print
  the regenerated JS in both `Pretty` modes. Keep it under 40 lines.

- [ ] **Step 3: Crate docs and README**

A quickstart that compiles as a doctest. State plainly: no byte-exact C++
oracle exists for this component (spec §1); the correctness bar is the round
trip; `Pretty::Yes` is indentation, not formatting. **No performance claims**
— that is a standing rule for this project's public docs.

- [ ] **Step 4: Run the doctests and the example**

Run: `cargo test --manifest-path rust/Cargo.toml -p hermes-gen-js --doc`
Run: `cargo run --manifest-path rust/Cargo.toml -p hermes-gen-js --example print_js`

- [ ] **Step 5: Commit**

```bash
git commit -m "rust(gen_js): facade entry point, example, and docs"
```

---

### Task 17: Tier 2 wide sweep and the coverage manifest

**Files:**
- Create: `rust/crates/gen_js/MANIFEST.md`
- Create: `rust/crates/gen_js/src/bin/sweep.rs` (or a `tools` bin — match
  where the sema sweep lives)

**Interfaces:**
- Produces: the recorded sweep result and the per-kind coverage table.

- [ ] **Step 1: Write the sweep binary**

Walk all 1934 `.js` under `test/`, infer flags per file (default ES; use the
lit `RUN:` line's `-parse-flow` / `-parse-ts` / `-parse-jsx` where present),
and round-trip each under both `Pretty` modes. Files that fail to *parse* are
skipped and counted separately — they are not generator failures.

- [ ] **Step 2: Run it and investigate every mismatch**

This is the task most likely to surface inherited juno bugs. Each mismatch
gets diagnosed: generator bug (fix it), parser bug (report, do not fix here),
or expected divergence (justify in the manifest).

- [ ] **Step 3: Build the per-kind coverage table**

Instrument the sweep to count how many times each of the 271 kinds is
generated. **Any kind with count 0 must be named in the manifest** alongside
the hand-written test that covers it instead (spec §7.2). Silent zero
coverage is the failure mode this table exists to prevent.

- [ ] **Step 4: Write `MANIFEST.md`**

Following `rust/crates/sema/tests/sema_corpus/MANIFEST.md`'s style: what was
run, the exact command, the counts, the outliers with reasons, and the
coverage table. Include the Task 15 Step 5 mutation evidence.

- [ ] **Step 5: Full-workspace regression check**

Run: `cargo test --manifest-path rust/Cargo.toml`
Expected: existing gates unmoved — sema 224 (111), parser-entry 17 (9),
parser 8/8, citations clean at 3186 blessed sites. Any movement is a
regression this port caused; investigate before committing.

- [ ] **Step 6: Commit**

```bash
git commit -m "rust(gen_js): wide sweep results and per-kind coverage manifest"
```

---

## Final Review

After Task 17, dispatch a whole-branch review on the most capable model. Beyond
the standard rubric, direct it at spec §8's five focus areas, and at these
plan-specific invariants:

1. No `..` in any node pattern; no `_ =>` in the kind dispatch (constraints 4–5).
2. No identifier emitted via `bytes()` or `bytes_str_lossy` (constraint 6).
3. No new external dependency (constraint 3).
4. Every one of the 99 new arms has a named test (Tasks 12–13).
5. The mutation evidence in `MANIFEST.md` names a real test and a real message.
6. A sample of generated output read by hand for quality, since the round trip
   cannot judge readability (spec §7.4).
