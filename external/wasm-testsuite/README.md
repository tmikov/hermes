# WebAssembly Spec Test Suite

The `tests/` subdirectory is a snapshot of
https://github.com/WebAssembly/testsuite, checked into the Hermes
repository so that `check-hermes` can run the tests without network access.

## Updating from upstream

    rm -rf external/wasm-testsuite/tests
    git clone --depth 1 https://github.com/WebAssembly/testsuite external/wasm-testsuite/tests
    rm -rf external/wasm-testsuite/tests/.git

Review the diff and commit.

## Running the tests

Individual lit wrappers in `test/wasm/spec/*.wast` run as part of
`check-hermes`. To run all tests with a summary table:

    python3 test/wasm/spec/run-all-spec-tests.py \
      --wast2json cmake-build-debug/external/wabt/wabt/wast2json \
      --hermes cmake-build-debug/bin/hermes \
      --testsuite external/wasm-testsuite/tests

## Scope

The test suite includes tests for features beyond what Hermes currently
supports (SIMD, GC types, 64-bit memory, tail calls, etc.). These files
are kept for future use. Only the subset listed in
`test/wasm/spec/run-all-spec-tests.py` is actively exercised.
