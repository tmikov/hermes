# hermes-ast

ESTree-compatible AST and JSON dumper for the Hermes Rust front-end. Part of
the `hermes-parser` crate family.

Provides the GC-arena AST (271 ESTree nodes generated from `ESTree.def`),
the transforming visitor, and the `ESTreeJSONDumper` — byte-for-byte matching
`hermesc -dump-ast -dump-source-location=both`.

**Status:** pre-release, not yet on crates.io.

See [the project README](https://github.com/tmikov/hermes/blob/rust1/rust/README.md) for the full documentation,
language support matrix, and the project story.
