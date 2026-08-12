# hermes-atom-table

String interner for the Hermes Rust front-end. Part of the `hermes-parser`
crate family.

Interns UTF-8 strings and WTF-8 byte strings into stable `Atom` / `AtomBytes`
handles that compare and hash in O(1). Copied from the juno project and
extended with the byte-intern path the lexer needs for surrogate-bearing
string literals.

This is a support crate: it is published because `hermes-parser`'s dependency
closure requires it, and it carries no stability guarantee of its own.

**Version:** 0.1.0 — API docs at [docs.rs/hermes-atom-table](https://docs.rs/hermes-atom-table).

See [the project README](https://github.com/tmikov/hermes/blob/rust1/rust/README.md) for the full
documentation of the crate family.
