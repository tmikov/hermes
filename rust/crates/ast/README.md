# hermes-ast

ESTree-compatible AST and JSON dumper for the Hermes Rust front-end. Part of
the `hermes-parser` crate family.

Provides the GC-arena AST (271 ESTree nodes generated from `ESTree.def`),
the transforming visitor, and the `ESTreeJSONDumper` — byte-for-byte matching
`hermesc -dump-ast -dump-source-location=both`.

**Version:** 0.1.0 — API docs at [docs.rs/hermes-ast](https://docs.rs/hermes-ast).

See [the project README](https://github.com/tmikov/hermes/blob/rust1/rust/README.md) for the full documentation,
language support matrix, and the project story.
