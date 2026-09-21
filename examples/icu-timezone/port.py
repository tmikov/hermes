#!/usr/bin/env python3
"""Port the Node/ESM ICU4X timezone demo bundle to run under Hermes.

The bundle is pure JS apart from a small Node-specific loader header:
  - `import` statements for fs/url/path
  - `import.meta.url` based path resolution
  - `readFileSync` to obtain the wasm bytes

Hermes runs the file as a script, and gets the wasm bytes from
`hermescli.loadFile()` (available under -Xhermes-internal-test-methods).
Everything else -- WebAssembly.Module/Instance, TextDecoder, console,
FinalizationRegistry -- already exists in Hermes.
"""
import sys

SRC = sys.argv[1]
DST = sys.argv[2]

with open(SRC) as f:
    text = f.read()

# 1. Drop the Node module imports.
OLD_IMPORTS = """import { readFileSync } from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';
"""
assert OLD_IMPORTS in text, "import header not found"
text = text.replace(OLD_IMPORTS, "", 1)

# 2. Replace the Node path resolution + runtime instantiation with the Hermes
#    AOT path: the module is compiled to .hbc by `hermesc --wasm` ahead of
#    time, and WebAssembly.Module/Instance load it.
OLD_LOAD = """const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const wasmPath = join(__dirname, 'icu_capi.wasm');
const wasmBytes = readFileSync(wasmPath);
const wasmModule = new WebAssembly.Module(wasmBytes);
const wasmInstance = new WebAssembly.Instance(wasmModule, imports);
wasm = wasmInstance.exports;
"""
NEW_LOAD = """// Hermes: the module is precompiled to .hbc by `hermesc --wasm`; the path is
// passed as the first script argument. It is loaded with the explicit
// fromHermesBytecode() entry point rather than by handing the bytes to
// WebAssembly.Module: that used to work by content-sniffing the .hbc, which
// is off by default because it is a JS-reachable path into a format the
// engine trusts without re-validating. Instantiation is then the standard
// API, with the import object passed to the Instance constructor.
//
// fromHermesBytecode is itself gated on EnableUntrustedBytecodeFromJS, which
// run.sh passes as -Xenable-untrusted-bytecode-from-js.
const hbcPath = hermescli.getScriptArgs()[0];
const wasmModule =
    WebAssembly.Module.fromHermesBytecode(hermescli.loadFile(hbcPath));
wasm = new WebAssembly.Instance(wasmModule, imports).exports;
"""
assert OLD_LOAD in text, "loader block not found"
text = text.replace(OLD_LOAD, NEW_LOAD, 1)

with open(DST, "w") as f:
    f.write(text)

print("wrote %s (%d lines)" % (DST, text.count("\n") + 1))
