# Wasm TODO

- The 2 remaining data.wast failures (lines 89, 90) use `global.get` on
  non-imported globals, which wast2json rejects. These cannot be fixed without
  upgrading or patching wast2json.
