# Contributing

> A Rust port of the Hermes front-end by Tzvetan Mikov, the architect of Hermes.
> Not an official Meta project and not supported by Meta.

Issues and PRs are welcome and addressed as time permits. There is no SLA.

---

## Before you start

Read [ARCHITECTURE.md](ARCHITECTURE.md) for the crate map, AST model, and the
faithful-port conventions. The conventions are binding — they describe how C++
constructs map to Rust, and deviating from them without approval changes the
structural correspondence with the C++ source.

## Setting up

You need:
- Rust 1.96.0 (pinned via `rust/rust-toolchain.toml`; `rustup` will pick it up
  automatically).
- A C++ build of the Hermes `hermesc` binary for the differential gate (see
  below). This is only needed if you are changing the parser.

## Build and test

Do **not** `cd` into `rust/`. Use `--manifest-path` from the repo root:

```bash
# Build the whole workspace (expect zero warnings):
cargo build --manifest-path rust/Cargo.toml

# Test the whole workspace:
cargo test --manifest-path rust/Cargo.toml

# Test a single crate:
cargo test --manifest-path rust/Cargo.toml -p hermes-parser
cargo test --manifest-path rust/Cargo.toml -p hermes-ast
cargo test --manifest-path rust/Cargo.toml -p hermes-sema
cargo test --manifest-path rust/Cargo.toml -p hermes-support

# Clippy (only pre-existing faithful-C-idiom lints are allowed):
cargo clippy --manifest-path rust/Cargo.toml
```

The CLI drivers (`ast-dump`, `json-parse-dump`, `gen-json`, `preparse-dump`,
`sema-dump`) live in the unpublished `tools` crate, not in `parser` or `sema`:
the published libraries ship no binaries and no `hermes-command-line`
dependency.

```bash
cargo build --manifest-path rust/Cargo.toml -p tools
```

The differential tests build the driver they need on demand, so the gate below
does not need a separate `-p tools` build first.

### Regenerate and verify the AST node set

If you modify `include/hermes/AST/ESTree.def` or `rust/crates/ast/gen_nodes.py`:

```bash
# Regenerate src/node.rs:
python3 rust/crates/ast/gen_nodes.py

# Verify the committed output matches the generator (idempotency gate):
REQUIRE_GEN=1 cargo test --manifest-path rust/Cargo.toml \
    -p hermes-ast --test generated_idempotent
```

The committed `src/node.rs` must always be byte-for-byte identical to what the
generator produces.

## The differential gate (required for parser and sema changes)

Any change to the parser, lexer, AST dumper, or semantic resolver must pass the
byte-for-byte differential gate before it can be merged.

### Build the C++ oracle

Configure an ASan + Debug + `-O1` build once (this is the standard Hermes
development build; it is git-ignored):

```bash
cmake -B cmake-build-asan -G Ninja -DCMAKE_BUILD_TYPE=Debug \
  -DHERMES_ENABLE_ADDRESS_SANITIZER=ON \
  -DCMAKE_CXX_FLAGS="-O1" -DCMAKE_C_FLAGS="-O1"

# Build the hermesc oracle (for the JS parser differential):
cmake --build cmake-build-asan --target hermesc

# Build the lexer oracle (for the lexer differential):
cmake --build cmake-build-asan --target js-lexer-dump

# Build the JSON oracle (for the JSON parser differential):
cmake --build cmake-build-asan --target json-parse-dump

# Build the pre-parse oracle (for the pre-parse differential):
cmake --build cmake-build-asan --target preparse-dump

# Build the parser-entry sema oracle (hermesc above is the driver-path one):
cmake --build cmake-build-asan --target sema-parser-dump
```

### Run the gate

```bash
# JS parser differential (166 corpus files: 77 plain + 42 Flow + 8 component
# + 5 records + 7 match + 20 TS + 6 JSX + 1 JSX/Flow); fails hard if hermesc
# is absent:
REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml \
    -p hermes-parser --test parser_differential

# Lexer differential (div 58 / regexp 5 / type 6 / jsx 4 / jsx-child 10 /
# nonstrict 7 corpus entries):
REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml \
    -p hermes-parser --test differential -- --nocapture

# JSON parser differential (17 corpus files):
REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml \
    -p hermes-parser --test json_differential -- --nocapture

# Pre-parse differential (lazy 13 / plain 77 / Flow 42 / TS 20 corpus files):
REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml \
    -p hermes-parser --test preparse_differential -- --nocapture

# Sema differential — both pairs at once: `hermesc -dump-sema` over 219 corpus
# files (109 of them hermesc successes), and `sema-parser-dump` vs `sema-dump
# --parser-entry` over 13 (5 successes):
REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml \
    -p hermes-sema --test sema_differential -- --nocapture
```

**The differentials are debug-only against this oracle.** Run them with a
plain `cargo test`, never `cargo test --release`. The recursion limits are
profile-selected on both sides: the ASan oracle takes C++'s
`HERMES_LIMIT_STACK_DEPTH` branch (parser limit 128, resolver limit 512),
while the port mirrors that under `cfg!(debug_assertions)` and uses C++'s
release values (1024/1024) otherwise. Debug-vs-ASan agrees; crossing the
profiles does not. Concretely, `cargo test --release … sema_differential`
fails on `nested-expressions.js` — the oracle reports the recursion-limit
error at `12:46` and the port at `23:62` — and it always will. That is
expected, not a port defect: do not "fix" it by changing a limit on either
side. (`--release` additionally compiles out an assertion that one corpus
file exists to reproduce.) See the module doc of
`crates/sema/tests/sema_differential.rs`.

The `REQUIRE_DIFFERENTIAL=1` environment variable causes the test to fail hard
if the oracle binary is absent, rather than silently skip. Always set it when
running the gate before submitting a change.

## Faithful-port conventions (summary)

Full detail is in [ARCHITECTURE.md](ARCHITECTURE.md). The short version:

**C++ templates → Rust generics.** Every C++ `template <…>` specialization must
stay a Rust generic (`const` generic or marker-trait type parameter). Never
flatten a template to a runtime `bool` or enum parameter — the differential test
cannot detect the difference, but it is a structural deviation.

**C++ RAII guards → explicit guard types.** `SaveAndRestore`, `SaveFunctionState`,
`SavePoint`, and message-suppression scopes all become either `Drop`-based guard
types (for parser-flag saves) or explicit begin/end API calls (for manager-level
suppress/collect). The full feature is always present; only the syntactic sugar
differs.

**C++ default arguments are spec.** Look up every default in the C++ header
before porting a call site. Assuming a default has caused real bugs (see the P4,
P5, P6 entries in the roadmap for examples).

**Keep comments.** Copy or closely paraphrase the C++ comments at each ported
site. They provide traceability to the original and often document non-obvious
invariants.

**Zero new warnings.** The workspace must build with zero warnings. Zero new
Clippy lints.

## Adding corpus files

Corpus files live under `rust/crates/parser/tests/`:

| Directory | Used for |
|---|---|
| `parser_corpus/` | Plain JS (no dialect flags) |
| `parser_corpus_flow/` | `-parse-flow` |
| `parser_corpus_flow_component/` | `-parse-flow -Xparse-component-syntax` |
| `parser_corpus_flow_records/` | `-parse-flow -Xparse-flow-records` |
| `parser_corpus_flow_match/` | `-parse-flow -Xparse-flow-match` |

Add a `.js` file; the differential test runner picks it up automatically and
runs both the Rust `ast-dump` binary and `hermesc` on it, comparing output
byte-for-byte.

## Code style

- 4-space indent (Rust standard).
- Line limit: follow `rustfmt` defaults.
- Naming: Rust standard (`snake_case` functions/methods/fields, `PascalCase`
  types, `SCREAMING_SNAKE_CASE` constants).
- `unsafe` is forbidden except in the three sanctioned locations
  (`cursor.rs` in `parser`; `context.rs` in `ast`; `atom_table` for the
  interner). Adding `unsafe` elsewhere requires explicit discussion.

## License

By contributing, you agree that your contributions are licensed under the
MIT License, the same license as this project. See [LICENSE](LICENSE).
