# hermes-unicode

Unicode property tables for the Hermes Rust front-end. Part of the
`hermes-parser` crate family.

Provides the character-property predicates the JavaScript lexer needs
(identifier start/continue, whitespace, combining marks, digits, connector
punctuation) plus the surrogate/code-point helpers, backed by range tables
generated from `UnicodeData.inc` (Unicode 17.0.0). Zero `unsafe`
(`unsafe_code = "forbid"`) and no dependencies.

This is a support crate: it is published because `hermes-parser`'s dependency
closure requires it, and it carries no stability guarantee of its own.

**Status:** pre-release, not yet on crates.io.

See [the project README](https://github.com/tmikov/hermes/blob/rust1/rust/README.md) for the full
documentation of the crate family.
