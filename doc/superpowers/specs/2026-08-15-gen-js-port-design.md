# AST → JS generator: port design

**Date:** 2026-08-15.
**Goal:** give the Rust port the ability to turn an AST back into JavaScript
source — the one major front-end capability the port still lacks.
**Source:** `unsupported/juno/crates/juno/src/gen_js.rs` (4174 lines).

## 1. Which generator we port

Two implementations exist in this repo:

| | `unsupported/juno/crates/juno/src/gen_js.rs` | `lib/AST2JS/AST2JS.cpp` |
|---|---|---|
| Size | 4174 lines | 1239 lines + 28-line header |
| Coverage | ES + Flow + JSX; **no TS** (§2) | ES only; 0 type-annotation sites |
| Unknown node | `unimplemented!()` panic | `abort()` (`AST2JS.cpp:107`) |
| Driver | `juno` CLI | `hermesc -dump-js [-pretty]` |

**We port juno's.** The C++ `AST2JS` is not used by Hermes and has not been
extensively tested, so its behavior is not a specification and byte-matching it
buys nothing. This is a deliberate exception to the port's usual
fidelity-to-C++ rule (`rust_port_conventions`), made because the C++ artifact
is not load-bearing.

Consequence: **there is no oracle.** Every other component of this port was
validated by byte-comparison against a C++ binary. This one cannot be, so
correctness rests entirely on the round-trip property in §7 and on review.
Assume juno's generator has bugs; the plan's job is to find them, not to
transcribe faithfully and declare victory.

## 2. Why the adaptation is tractable

`crates/ast` was derived from `juno_ast` and kept the same shape: an
`enum Node<'gc>` with one struct per kind, plus `Path`, `NodeField`, `Visitor`
and `VisitorMut` in `crates/ast/src/visitor.rs`. `gen_js` is one large `match`
over that enum driven by `Path`, so it transfers structurally rather than
needing a redesign.

Node-kind census (measured 2026-08-15):

**Count what `gen_node` prints, not what `def.rs` declares.** An earlier draft
of this section diffed the two AST *definitions* and reported a 50-kind gap.
That was wrong: juno's `juno_ast` declares many kinds its generator never
prints. Measuring the actual match arms in `gen_node` (`gen_js.rs:362-3195`):

- juno's generator prints **165 of our 271 kinds**. The gap is **106** (§4).
- **`Node::TS` occurs 0 times in all 4174 lines.** juno's generator has no
  TypeScript support at all, though `juno_ast` defines the nodes.
- **juno-only: `Module`** — juno's separate ESM root. We have no such kind;
  `Program` covers both. Its arm is dropped.

**The gap is a runtime panic, not a compile error.** juno's match ends in
`_ => unimplemented!("Cannot generate node kind: {}", node.name())`
(`gen_js.rs:3189-3191`). **We delete that catch-all** and enumerate every
kind explicitly. Combined with §5's exhaustive destructuring, that makes both
halves of AST drift — a new *kind* and a changed *field* — compile errors
rather than crashes in front of a user. This is the single most valuable
change we make to juno's design.

## 3. Crate shape and public API

New workspace member `rust/crates/gen_js`, package **`hermes-gen-js`**,
version 0.1.0.

**Dependencies:** `hermes-ast`, `hermes-support`, `hermes-sema`. **No new
external dependencies** — the workspace stays at bumpalo-only (§6).

The sema dependency exists solely for `Annotation::Sem`. That places
`hermes-gen-js` above `hermes-sema` in the dependency order.

```rust
/// Generate JS for `root` and write it to `out`.
pub fn generate(
    out: &mut dyn Write,
    ctx: &GCLock,
    root: &Node,
    opt: Opt,
) -> Result<(), GenJsError>;

/// Why generation failed: a sink `Io` error, an `UnsupportedKind` (§4's seven
/// internal kinds), or an `UnrepresentableIdentifier` (§5).
pub enum GenJsError { Io(io::Error), UnsupportedKind(&'static str), UnrepresentableIdentifier }

pub struct Opt<'s> {
    pub pretty: Pretty,
    pub annotation: Annotation<'s>,
    pub force_async_arrow_space: bool,
    pub doc_block: Option<Rc<String>>,
    pub quote: QuoteChar,
}

pub enum Pretty { No, Yes }
pub enum QuoteChar { Single, Double }
pub enum Annotation<'s> { No, Sem(&'s SemContext) }
```

`Opt`'s defaults match juno's: `Pretty::Yes`, `Annotation::No`,
`force_async_arrow_space: true`, `doc_block: None`, `QuoteChar::Single`.

> **Correction (2026-08-15, found during Task 2).** An earlier draft gave
> juno's signature verbatim — `ctx: &mut Context, root: &NodeRc`. That shape is
> **uncallable** here: `ParsedJS` keeps `ctx` and `program` private and hands
> out only `for<'gc> FnOnce(&'gc GCLock<'static, '_>, &'gc Node<'gc>)` through
> `with_program` (`crates/parser/src/facade.rs`). Taking the lock and the node
> directly is the shape a caller can actually reach. Note also that `GCLock` is
> invariant in `'ast`, so the lock's lifetimes must stay independent of the
> node's `'gc` — the same trap `print_bindings.rs` documents.

**Façade method.** `ParsedJS` gains
`to_js(&mut self, opt: Opt) -> Result<String, GenJsError>`, beside the existing
`to_estree_json`. The `String` is sound rather than optimistic — see §5's
identifier rule for why output is always valid UTF-8, and §4 for what the
`Err` arm carries. The 0.1.0 usability review found that a
capability absent from the façade and the examples is a capability users do not
find; a generator reachable only by hand-assembling a `GCLock` would repeat
that mistake.

**Every option is kept**, including `doc_block` and
`force_async_arrow_space` (which exists so downstream transforms that
pattern-match `async` followed by whitespace keep working —
`gen_js.rs:437-441`). Dropping juno features because they look incidental is
the deferral this port does not do (`implement-components-completely`).

## 4. The 106 kinds juno's generator does not print

**99 are real syntax our parser produces. They get printing arms.** Full
coverage is the decision: our parser emits these under real `ParseFlags`
(`parse_flow_match`, `parse_flow_records`, `parse_flow_component_syntax`,
`parse_ts`), so the corpus gate (§7) exercises them rather than skipping them.

**53 ES/Flow.** Note `StaticBlock` — `class { static { … } }` is plain
ES2022, not a type feature; juno simply predates it.

`AsConstExpression`, `AsExpression`, `ComponentDeclaration`,
`ComponentParameter`, `ComponentTypeAnnotation`, `ComponentTypeParameter`,
`ConditionalTypeAnnotation`, `DeclareComponent`, `DeclareEnum`, `DeclareHook`,
`DeclareNamespace`, `Decorator`, `EnumBigIntBody`, `EnumBigIntMember`,
`HookDeclaration`, `HookTypeAnnotation`, `InferTypeAnnotation`,
`KeyofTypeAnnotation`, `MatchArrayPattern`, `MatchAsPattern`,
`MatchBindingPattern`, `MatchExpression`, `MatchExpressionCase`,
`MatchIdentifierPattern`, `MatchInstanceObjectPattern`, `MatchInstancePattern`,
`MatchLiteralPattern`, `MatchMemberPattern`, `MatchObjectPattern`,
`MatchObjectPatternProperty`, `MatchOrPattern`, `MatchRestPattern`,
`MatchStatement`, `MatchStatementCase`, `MatchUnaryPattern`,
`MatchWildcardPattern`, `NeverTypeAnnotation`, `ObjectTypeMappedTypeProperty`,
`QualifiedTypeofIdentifier`, `RecordDeclaration`, `RecordDeclarationBody`,
`RecordDeclarationImplements`, `RecordDeclarationProperty`,
`RecordDeclarationStaticProperty`, `RecordExpression`,
`RecordExpressionProperties`, `StaticBlock`, `TupleTypeLabeledElement`,
`TupleTypeSpreadElement`, `TypeOperator`, `TypePredicate`,
`UndefinedTypeAnnotation`, `UnknownTypeAnnotation`.

**46 TypeScript**, none with any juno precedent:

`TSAnyKeyword`, `TSArrayType`, `TSAsExpression`, `TSBigIntKeyword`,
`TSBooleanKeyword`, `TSCallSignatureDeclaration`, `TSConditionalType`,
`TSConstructorType`, `TSEnumDeclaration`, `TSEnumMember`, `TSFunctionType`,
`TSIndexSignature`, `TSIndexedAccessType`, `TSInterfaceBody`,
`TSInterfaceDeclaration`, `TSInterfaceHeritage`, `TSIntersectionType`,
`TSLiteralType`, `TSMethodSignature`, `TSModifiers`, `TSModuleBlock`,
`TSModuleDeclaration`, `TSModuleMember`, `TSNeverKeyword`, `TSNumberKeyword`,
`TSParameterProperty`, `TSPropertySignature`, `TSQualifiedName`,
`TSStringKeyword`, `TSSymbolKeyword`, `TSThisType`, `TSTupleType`,
`TSTypeAliasDeclaration`, `TSTypeAnnotation`, `TSTypeAssertion`,
`TSTypeLiteral`, `TSTypeParameter`, `TSTypeParameterDeclaration`,
`TSTypeParameterInstantiation`, `TSTypePredicate`, `TSTypeQuery`,
`TSTypeReference`, `TSUndefinedKeyword`, `TSUnionType`, `TSUnknownKeyword`,
`TSVoidKeyword`.

Since there is no precedent to port, the TS arms are written against our
parser's own grammar: for each kind, the syntax the parser accepts to produce
it is the syntax the generator must emit, and the round trip is the check.

**7 are internal and must be a hard error**, because no source syntax
corresponds to them and inventing output would hide a caller bug:

- `CoverEmptyArgs`, `CoverInitializer`, `CoverRestElement`,
  `CoverTrailingComma`, `CoverTypedIdentifier` — the cover-grammar group
  (`ESTree.def:1464-1500`), transient by construction.
- `ImplicitCheckedCast` — "FlowChecker generated nodes"
  (`ESTree.def:1509-1512`); not parser-produced.
- `SHBuiltin` — "Static Hermes-specific nodes" (`ESTree.def:1505`); verified
  absent from both `JSParserImpl.cpp` and `crates/parser/src/`.

The error is a returned `io::Error`, not a panic or `abort()`. A library must
not kill its caller's process over a malformed input tree — this is where we
deliberately depart from C++ `AST2JS.cpp:107`.

## 5. Adaptation rules

**Destructure exhaustively; never use `..`, and keep no catch-all arm.**
juno's arms name every field
(`Identifier { metadata: _, name, type_annotation, optional }`); we keep that
and additionally drop juno's `_ => unimplemented!()` (§2). Together these make
a changed *field* and an added *kind* both compile errors. Without them, AST
drift reaches users as wrong output or a panic. This rule is load-bearing and
must survive review — a reviewer seeing `..` or `_ =>` in this crate should
treat it as a defect regardless of how convenient it looks.

**Field access.** Our node fields are `Cell`-wrapped where juno's are plain;
reads go through `.get()`.

**Numbers.** `juno_support::convert::number_to_string` maps to the existing
port at `crates/support/src/json_emitter.rs:19`, already covered by
ECMAScript spot-check tests (`json_emitter.rs:591`). It is currently private to
the JSON emitter and must be lifted to a public `hermes-support` API rather
than duplicated.

**Strings.** juno escapes via `ctx.str_u16(value)` — UTF-16 code units, with
everything outside printable ASCII emitted as `\uXXXX`
(`gen_js.rs:3300-3350`). We have no `str_u16`; use
`hermes_support::utf8::convert_utf8_with_surrogates_to_utf16`
(`crates/support/src/utf8.rs:175`). This is the correct primitive: it decodes
our WTF-8 atoms surrogate-aware, so a lone surrogate in a string literal
becomes a single `\udXXX` escape and survives the round trip.

**Identifiers.** Write them via `gc.try_bytes_str(atom)`:
`Some(s)` → emit `s`; `None` → generator error (§4's `io::Error` path).

Neither of the two tempting alternatives is correct:

- **Not `gc.bytes(atom)` (exact bytes).** Astral identifiers are legal JS —
  `U+1D465` and friends are `ID_Start` — and our atoms hold astral characters
  as WTF-8 **surrogate pairs**, not 4-byte UTF-8. Writing those bytes verbatim
  emits output that is not valid UTF-8. `try_bytes_str` folds pairs back into
  the character, which is exactly the needed re-encoding.
- **Not `bytes_str_lossy`.** A generator that substitutes U+FFFD emits a
  different program than it was given.

`None` means an unpaired surrogate, which has no JS identifier spelling: it
cannot be written literally, and the `\uD800` escape form is rejected by the
lexer as "not a valid identifier start". Refusing to emit is the only honest
option.

Because string literals are escaped to ASCII (above) and identifiers are
re-encoded here, **generator output is always valid UTF-8** — which is what
makes the façade's `String` return type sound rather than optimistic.

## 6. Dropped: the sourcemap stub

juno's `generate` returns `io::Result<SourceMap>` and pulls
`sourcemap = "6.0"`, but its own doc comment says
`FIXME: This currently only returns an empty SourceMap` (`gen_js.rs:89`), and
there are exactly 5 `add_segment` call sites. We drop the return value, the 5
calls, the `SourceMapBuilder` field, the `cur_token` field, and the
dependency; `out_token!` collapses into `out!`.

Real sourcemap support, if wanted later, is a designed feature — not a stub
inherited by accident. Nothing else in the port depends on it.

## 7. Verification

### 7.1 The round-trip fixed point

juno's own harness (`unsupported/juno/crates/juno/tests/gen_js/mod.rs`, 746
lines) checks: parse → generate → re-parse → compare JSON dumps, under **both**
`Pretty::Yes` and `Pretty::No`. Its `test_roundtrip(...)` case list ports over
as unit tests.

This property — *the generated source reparses to the same tree* — is the
correctness bar. It is also the only bar available (§1).

### 7.2 The corpus gate

The instrument that makes this a real review rather than a reading exercise.
For each corpus file, under its dialect's `ParseFlags`, for both `Pretty`
modes:

```
parse(src, flags) → generate → parse(generated, flags) → compare to_estree_json
```

Compare with `raw` omitted — the same normalization C++ applies via
`-Xinclude-raw-ast-prop=0` — because `raw` is source text that regeneration is
not expected to reproduce.

**Two tiers, because the two corpora differ in kind.**

*Tier 1 — the standing gate (`cargo test`).* The checked-in per-dialect
corpora, 420 `.js` files, whose directory names already carry the dialect:

| corpus | files | | corpus | files |
|---|---|---|---|---|
| `sema_corpus` | 224 | | `parser_corpus_lazy` | 13 |
| `parser_corpus` | 77 | | `parser_corpus_flow_component` | 8 |
| `parser_corpus_flow` | 42 | | `parser_corpus_flow_match` | 7 |
| `parser_corpus_ts` | 20 | | `parser_corpus_jsx` | 6 |
| `sema_corpus_parser` | 17 | | `parser_corpus_flow_records` | 5 |
| | | | `parser_corpus_jsx_flow` | 1 |

Hermetic and fast; this is what regressions are caught by later.

*Tier 2 — the wide sweep, run during development and again at review, recorded
in the crate's manifest doc.* All 1934 `.js` under `test/` (the same
methodology the sema port used for its 1232-file sweep over `test/Sema`,
`test/Parser` and `test/hermes` — `sema_corpus/MANIFEST.md:2561`). Not a
standing test: it reads the C++ tree, which a published crate cannot assume.

Tier 1 alone would be thin for TS — 20 files against 46 new arms — so any TS
kind the sweep never exercises must get a hand-written case, and the manifest
must list which kinds those were. An untested arm is an unwritten one.

Every mismatch is a genuine generator bug. Finding them is the point.

### 7.3 Prove the gate can fail

Per `prove-checks-can-fail`: before trusting a green corpus run, mutate the
generator (e.g. drop a parenthesization rule) and show a **named** test fails.
A gate that has never failed has not been shown to test anything.

### 7.4 Output quality

The round trip proves reparse-equivalence, not that the output is idiomatic or
minimal. That cannot be automated cheaply, so the review samples generated
output by hand. Called out explicitly so a green gate is not mistaken for a
statement about readability.

## 8. Review focus areas

Where a JS printer is silently wrong, and where the review should concentrate:

1. **Parenthesization** — the `precedence` module, `ALWAYS_PAREN`, arrow
   bodies, `in` inside `for`-init, sequence expressions, and the recursive
   "does this expression start with a node satisfying `pred`" left-child walk
   that decides statement-level parens.
2. **ASI hazards specific to `Pretty::No`**, where the newlines that made
   pretty output safe are gone.
3. **String escaping** against WTF-8 atoms — including the lone-surrogate case,
   which we know is reachable. Needs named tests for both halves of §5's
   identifier rule: an **astral identifier** (`var 𝑥 = 1`) must round-trip and
   the output must be valid UTF-8; a **lone surrogate in a string literal**
   must survive as a single `\udXXX` escape.
4. **`RegExpLiteral`** — the C++ analogue carries a bare
   `// FIXME: escaping, etc?` (`AST2JS.cpp:126`); juno's arm deserves the same
   suspicion.
5. **The 99 new arms**, which have no juno precedent and are therefore the
   least-reviewed code in the crate — the 46 TS ones most of all, since they
   are written from the parser's grammar rather than ported from anything.

## 9. Out of scope

- Real sourcemap generation (§6).
- Matching `hermesc -dump-js` byte-for-byte (§1).
- Formatting/pretty-printing beyond juno's indentation model. `Pretty::Yes` is
  explicitly "not full formatting" (`gen_js.rs:62-64`).
- Publishing to crates.io — a separate, manual step owned by the user.

## 10. Gates

- Tier 1 corpus round-trip clean: 420 files, both `Pretty` modes, all
  dialects (§7.2), with a demonstrated failure mode (§7.3).
- Tier 2 wide sweep run and its result recorded, including a per-kind coverage
  table naming any arm no corpus file reaches (§7.2).
- Every one of the 271 kinds reachable in `gen_node` — no `_` arm, no `..`
  (§5). This is compiler-enforced, so it is a build gate, not a review item.
- juno's ported unit cases pass.
- Existing gates unmoved: sema 224 (111), parser-entry 17 (9), parser 8/8,
  citations clean.
- `cargo publish --dry-run` succeeds for the new crate.
