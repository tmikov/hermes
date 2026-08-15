# hermes-command-line

An LLVM-`cl`-style command-line option parser. Part of the `hermes-parser`
crate family.

Options are declared as typed handles registered against a `CommandLine`; each
declaration returns an `Opt` that dereferences to the parsed value once parsing
has finished. It covers long and short names, positional arguments,
list-valued options, enum options (a set of mutually exclusive flags) and
enum-valued options (`--opt=name`), minimum/maximum occurrence counts, values
shared between several options, help categories and hidden options. `--help` is
generated from the declarations.

The crate is Meta-authored and was copied from
[`unsupported/juno/crates/command_line`](https://github.com/tmikov/hermes/tree/hermes-crates-v0.1.1/unsupported/juno/crates/command_line)
in the Hermes repository. It is *styled* after LLVM's `cl` library — it matches
that library's command-line syntax and help layout — but it is not derived from
LLVM source. Zero `unsafe` (`unsafe_code = "forbid"`) and no dependencies.

Three behavioral changes have been made since the copy. Two align with LLVM
`cl` (and therefore with `hermesc`): a single leading dash is accepted as a
synonym for a double dash when it matches an option's full long name
(`-parse-flow` == `--parse-flow`), and `parse_env_args()` exits with status 1,
not 0, on a command-line usage error. The third is a bug fix: sharing one
`OptValue` between several options used to panic at the end of parsing, and now
works.

This is a support crate: it is published because the Hermes Rust front-end's
tools (`ast-dump`, `json-parse-dump`, `gen-json`, `sema-dump`) are built on it,
and it carries no stability guarantee of its own.

**Version:** 0.1.0 — API docs at [docs.rs/hermes-command-line](https://docs.rs/hermes-command-line).

See [the project README](https://github.com/tmikov/hermes/blob/hermes-crates-v0.1.1/rust/README.md) for the full
documentation of the crate family.
