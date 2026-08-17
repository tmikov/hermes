# hermes-parser

A Rust port of the Hermes JavaScript parser by Tzvetan Mikov, the architect of
Hermes. Not an official Meta project and not supported by Meta.

Provides the lexer, the JSON parser, and the JS parser — with the Flow type
grammar, TypeScript, and JSX all complete and opt-in per dialect. Produces an
ESTree-compatible AST (`hermes-ast`) byte-for-byte matching `hermesc -dump-ast`.

`parse(source, ParseFlags)` is the front door; the low-level `JSLexer` /
`JSParserImpl` API is public too.

API docs at [docs.rs/hermes-parser](https://docs.rs/hermes-parser).

See [the project README](https://github.com/tmikov/hermes/blob/rust/rust/README.md) for the full documentation,
language support matrix, quickstart, and comparison with other Rust parsers.
