# Wasm TODO

- Extended constant expressions (`i32.add`/`i32.sub`/`i32.mul`) in data
  segment offsets are rejected by Hermes's Wasm binary reader
  (`BinaryReaderHermesIRGen`). Supporting them would fix 4 data.wast failures
  (lines 178, 183, 188, 195). The remaining 2 data.wast failures (lines 89,
  90) use `global.get` on non-imported globals, which wast2json rejects.
