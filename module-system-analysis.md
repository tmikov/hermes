# CommonJS and ESM Implementation Analysis in Hermes

## CommonJS Implementation (Fully Functional)

**Core Infrastructure:**
- **`lib/AST/CommonJS.cpp`** - Wraps modules in `function(exports, require, module)` wrapper
- **`lib/VM/JSLib/require.cpp`** - `runRequireCall()` and `requireFast()` implement module loading
- **`include/hermes/VM/Domain.h`** - `Domain` class manages CJS module tables with caching
- **`include/hermes/VM/RuntimeModule.h`** - `RuntimeModule` tracks module exports

**Compiler Support:**
- **`lib/CompilerDriver/CompilerDriver.cpp:488`** - `-commonjs` flag activates CJS mode
- **`lib/Optimizer/Scalar/ResolveStaticRequire.cpp`** - Optimizes `require()` calls to fast-path IDs
- **`include/hermes/AST/Context.h`** - `useCJSModules_` flag controls module mode

**IR/BCGen:**
- **`include/hermes/IR/IR.h:2500-2507`** - `struct CJSModule` in IR with id, filename, function
- Bytecode includes `cjsModules_` and `cjsModulesStatic_` tables

## ESM Implementation (Partial - Transpiled to CJS)

**Parser Support (complete):**
- **`lib/Parser/JSParserImpl.cpp`** - Full ES6 import/export parsing
- Supports: `import`, `export`, `export default`, `export *`, `import.meta`

**IR Generation (transpiles to CJS):**
- **`lib/IRGen/ESTreeIRGen-stmt.cpp:1373-1570`**:
  - `genImportDeclaration()` - Converts `import` to `require()` calls
  - `genExportNamedDeclaration()` - Stores to `exports` object
  - `genExportDefaultDeclaration()` - Stores to `exports.default`
  - `genExportAllDeclaration()` - Calls `HermesBuiltin_exportAll()`

**Key Limitation:** ESM is transpiled to CJS semantics - no true live bindings, just snapshots.

## Dependency Extraction

- **`lib/DependencyExtractor/DependencyExtractor.cpp`** - Extracts dependencies with `DependencyKind`:
  - `ESM`, `Type`, `Require`, `Async`, `Resource`, `PrefetchedResource`, `GraphQL`

## Documentation

- **`doc/Modules.md`** - CJS module system documentation
- **`esm-implementation-guide.md`** - ESM implementation design (appears to be a planning doc)

## Test Coverage

- `test/AST/cjs/` - CJS error handling tests
- `test/Parser/es6/import.js`, `export.js` - ESM parser tests
- `test/Parser/import-meta.js` - `import.meta` tests
- `test/hermes/xmod-exec-require*.js` - Require execution tests

---

**Summary:** Hermes has a complete CommonJS implementation used in production. ESM syntax is parsed but transpiled to CJS semantics at compile time - there's no native ESM runtime support (no module records, no live bindings, no async module loading). If you want to add true ESM support, you'd need to build the runtime infrastructure; if you want to remove modules entirely, the CJS code is fairly concentrated in the files listed above.
