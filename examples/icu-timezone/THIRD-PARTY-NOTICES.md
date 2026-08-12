# Third-party components in this example

Two files in this directory are prebuilt third-party artifacts, checked in as-is
rather than built from this repository. They are not part of Hermes and are not
covered by the Hermes LICENSE at the root of this repo.

## `icu_capi.wasm`

ICU4X's C API (`icu_capi`) compiled to WebAssembly.

- Upstream: <https://github.com/unicode-org/icu4x>
- Version: ICU4X 2.1.1 (the `icu` crate 2.1.1, published 2025-10-28)
- License: Unicode License V3 (`Unicode-3.0`) — see [LICENSE-ICU4X](LICENSE-ICU4X)
- Copyright © 2020-2024 Unicode, Inc.

The module embeds compiled ICU4X code and its locale data in the data segment,
both of which are covered by the Unicode license above.

### How the version was determined

The binary carries no version string. It was identified from:

- the `producers` custom section, which records
  `rustc 1.92.0-nightly (54a8a1db6 2025-09-26)`;
- `docs.rs/icu/2.1.1` and `docs.rs/icu_provider/2.1.1` links in the generated
  JS bindings (see below), which diplomat emits from the crate versions it was
  run against;
- the `_mv1` suffix on the exported FFI symbols, which is ICU4X's ABI version
  marker.

The exact build flags and data configuration used upstream are not recorded, so
this specific binary cannot be reproduced byte-for-byte from this repository.
To refresh it, rebuild `icu_capi` for `wasm32-unknown-unknown` from the ICU4X
repository and regenerate the bindings with diplomat.

## `timezone-demo.bundle.mjs`

A bundle containing ICU4X's diplomat-generated JavaScript bindings, the
diplomat JS runtime support code, and a small hand-written demo driver. Bundling
stripped the original per-file license headers, so the components are recorded
here.

- ICU4X JS bindings (the `icu4x_*` classes): generated from the ICU4X
  repository above, `Unicode-3.0`, see [LICENSE-ICU4X](LICENSE-ICU4X).
- diplomat runtime (`DiplomatBuf` and friends): from
  <https://github.com/rust-diplomat/diplomat>, dual-licensed
  `Apache-2.0 OR MIT` — see [LICENSE-diplomat](LICENSE-diplomat).
- The demo driver at the end of the file (the timezone conversion and world
  clock output) is part of this example and is covered by the Hermes LICENSE.

`port.py`, `run.sh`, `expected.txt` and `README.md` are part of Hermes and are
covered by the Hermes LICENSE.
