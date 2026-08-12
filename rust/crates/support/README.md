# hermes-support

Diagnostics and source management for the Hermes Rust front-end. Part of the
`hermes-parser` crate family.

Provides `SourceErrorManager` (buffers, source locations, line index, diagnostic
recording and rendering), the `JSONEmitter` used by the ESTree dumper, a
WTF-8 ↔ UTF-16 codec, and the arena-friendly `Deque` / `PersistentScopedMap`
containers. Zero `unsafe` (`unsafe_code = "forbid"`).

This is a support crate: it is published because `hermes-parser`'s dependency
closure requires it, and it carries no stability guarantee of its own.

**Status:** pre-release, not yet on crates.io.

See [`../../README.md`](../../README.md) for the full documentation of the
crate family.
