# hermes-parser

A Rust port of the Hermes JavaScript/Flow parser by Tzvetan Mikov, the
architect of Hermes. Not an official Meta project and not supported by Meta.

Provides the lexer, JSON parser, and JS/Flow parser. Produces an
ESTree-compatible AST (`hermes-ast`) byte-for-byte matching `hermesc -dump-ast`.

**Status:** pre-release, not yet on crates.io.

See [`../../README.md`](../../README.md) for the full documentation, language
support matrix, quickstart, and comparison with other Rust parsers.
