#!/usr/bin/env bash
# Build and run the ICU4X timezone demo through the Hermes Wasm frontend,
# then verify the output against expected.txt. See README.md.
#
# Usage: ./run.sh            (uses ../../cmake-build-release by default)
#        BUILD=../../cmake-build-debug ./run.sh
set -euo pipefail
cd "$(dirname "$0")"

BUILD="${BUILD:-../../cmake-build-release}"
HERMESC="$BUILD/bin/hermesc"
HERMES="$BUILD/bin/hermes"
for tool in "$HERMESC" "$HERMES"; do
    [ -x "$tool" ] || { echo "error: $tool not found; build the hermesc and hermes targets first (see README.md)" >&2; exit 1; }
done

# 1. Port the Node/ESM bundle to a Hermes script (fast, always redone).
python3 port.py timezone-demo.bundle.mjs timezone-demo.js

# 2. AOT-compile the Wasm module to bytecode. This is the slow step
#    (~76 s / 5.2 GB RSS with a Release hermesc; ~4.5 h with ASan).
#
#    This is deliberately not cached. The .wasm is a checked-in blob that
#    never changes, so the only input that moves is hermesc itself — and the
#    whole point of this example is to exercise a new hermesc. A cache would
#    spend its time skipping the one step worth running. Run the commands
#    under "Manual steps" in README.md if you want to reuse an existing .hbc.
echo "== compiling icu_capi.wasm (slow; Release ~76s, ASan ~4.5h) =="
time "$HERMESC" --wasm -emit-binary -out icu_capi.hbc icu_capi.wasm

# 3. Run the demo. hermescli.* (loadFile/getScriptArgs) is gated behind
#    -Xhermes-internal-test-methods, and WebAssembly.Module.fromHermesBytecode
#    behind -Xenable-untrusted-bytecode-from-js, because loading Hermes
#    bytecode handed over by script is a trust boundary the embedder opts into.
#    The .hbc path is passed as the script argument.
"$HERMES" -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js \
    timezone-demo.js -- icu_capi.hbc > out.txt

# 4. Verify.
if diff -u expected.txt out.txt; then
    echo "== OK: output matches expected.txt =="
else
    echo "== FAIL: output differs from expected.txt (see out.txt) ==" >&2
    exit 1
fi
