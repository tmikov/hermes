# hermes-parser

A Rust port of the Hermes JavaScript parser by Tzvetan Mikov, the architect of
Hermes. Not an official Meta project and not supported by Meta.

Provides the lexer, the JSON parser, and the JS parser — with the Flow type
grammar, TypeScript, and JSX all complete and opt-in per dialect. Produces an
ESTree-compatible AST (`hermes-ast`) byte-for-byte matching `hermesc -dump-ast`.

`parse(source, ParseFlags)` is the front door; the low-level `JSLexer` /
`JSParserImpl` API is public too.

**Status:** pre-release, not yet on crates.io.

See [`../../README.md`](../../README.md) for the full documentation, language
support matrix, quickstart, and comparison with other Rust parsers.
