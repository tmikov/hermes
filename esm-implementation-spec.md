# Hermes ESM Implementation Specification

## Overview

This document specifies the implementation of native ECMAScript Modules (ESM) support in Hermes. The implementation prioritizes spec compliance (test262), performance, and interoperability with the existing CommonJS (CJS) module system.

## Design Decisions Summary

| Aspect | Decision |
|--------|----------|
| Module Graph | Compile-time static, encoded as IR |
| CJS Interop | Full bidirectional |
| Top-Level Await | Deferred (not in v1) |
| Dynamic import() | Not supported in v1 |
| Strict Mode | Always (per spec) |
| Test262 Compliance | Full except TLA and dynamic import |

---

## 1. Module Graph and Compilation

### 1.1 Graph Location

The module dependency graph is **fully static and resolved at compile time**. The compiler traverses imports starting from the entry point, resolves all dependencies, and generates a single bytecode bundle.

There is **no runtime module graph** - all module relationships are encoded directly in the generated IR as initialization code.

### 1.2 Module Resolution

**Hermes compiler performs Node.js-style module resolution:**

- Relative paths (`./foo`, `../bar`)
- Package resolution via `node_modules`
- File extensions: `.mjs` (ESM), `.cjs` (CJS), `.js` (determined by `package.json`)
- `package.json` fields: `main`, `type`
- Package.json `exports` field: TBD (needs further investigation)

**Module type detection follows Node.js behavior:**
- `.mjs` → ESM
- `.cjs` → CJS
- `.js` → Determined by nearest `package.json` `"type"` field
- Explicit `--esm` / `--cjs` flags override auto-detection

**Unresolved specifiers are compile-time errors.** No stubs or runtime resolution.

### 1.3 Compilation Model

```bash
# Single-step bundling from entry point
hermesc entry.mjs -o bundle.hbc
```

The compiler:
1. Parses the entry module
2. Recursively resolves and parses all imports (ESM and CJS)
3. Builds dependency graph
4. Validates all export references at compile time
5. Generates single bytecode bundle

---

## 2. Bytecode and IR Generation

### 2.1 No New Bytecode Sections

Module metadata is **not stored in new bytecode sections**. Instead, the compiler generates an **initialization function encoded in IR** that:

1. Creates namespace objects for each module
2. Links imports to exports (closure capture)
3. Evaluates modules in dependency order

### 2.2 Generated Init Structure

```
__esmInit():
  // Create all namespace objects (frozen, empty initially)
  ns_A = createFrozenObject()
  ns_B = createFrozenObject()
  ns_C = createFrozenObject()

  // Nested module functions capture namespaces via closure
  // Modules receive their own namespace as a parameter

  function module_C(exports):
    // Module C body
    PrStore(exports, "x", value)  // Bypasses frozen

  function module_B(exports):
    // Can access ns_C via closure capture
    let imported_x = LoadModuleSlot(ns_C, 0)  // Live binding
    PrStore(exports, "y", ...)

  function module_A(exports):
    let imported_y = LoadModuleSlot(ns_B, 0)
    ...

  // Execute in dependency order (post-order traversal)
  module_C(ns_C)
  module_B(ns_B)
  module_A(ns_A)
```

### 2.3 Module Scopes

Each module is a **nested function** within the init function:
- Module receives its own namespace as a **parameter**
- Imported namespaces accessed via **closure capture**
- Clear function boundaries for debugging

### 2.4 Existing Instructions

- **PrStore**: Already exists, bypasses frozen flag. No modifications needed.
- **LoadModuleSlot**: New instruction with immediate moduleId operand

```
LoadModuleSlot <moduleId:imm>, <slotIndex:imm>
```

Where moduleId identifies the module and slotIndex is the export's position (declaration order).

---

## 3. Export Binding Storage

### 3.1 Namespace Objects

Each module's exports are stored in a **regular frozen JSObject**:

- Standard hidden class (no dedicated namespace class)
- `Object.freeze()` applied after creation
- Export properties are the binding slots

### 3.2 Live Bindings via PrStore

The module's own code uses `PrStore` to update export values:

```javascript
// Source: export let count = 0; count++;
PrStore(namespace, "count", 0)   // Initial
// ... later ...
PrStore(namespace, "count", 1)   // Update bypasses frozen
```

### 3.3 Slot Assignment

Export slots are assigned in **declaration order**:

```javascript
// b.mjs
export const w = 1;  // slot 0
export let x = 2;    // slot 1
export function y()  // slot 2
export class z {}    // slot 3
```

This provides stable, deterministic slot indices across builds.

### 3.4 External Access

Code outside the module accesses the namespace normally:
- Standard property access returns current value
- Mutations rejected (frozen object semantics)
- `delete`/`defineProperty` throw in strict mode (standard frozen behavior)
- Access to non-existent properties returns `undefined` (per spec)

---

## 4. Import Resolution

### 4.1 Compile-Time Validation

All imports are validated at compile time:

```javascript
// a.mjs
import { nonExistent } from './b.mjs';  // COMPILE ERROR
```

### 4.2 Runtime Access - Live Bindings

Every read of an imported binding goes through `LoadModuleSlot`:

```javascript
// Source: import { x } from './b.mjs'
// Every access to x compiles to:
LoadModuleSlot(moduleB, slotX)
```

This ensures **true live binding semantics** - reads always see the current exported value.

### 4.3 Performance Note

The indirection overhead is accepted initially. Optimization (inlining constant exports) deferred pending profiling data.

---

## 5. Star Re-exports

### 5.1 Compile-Time Resolution

`export * from 'x'` is resolved at compile time:

```javascript
// a.mjs
export * from './b.mjs';  // Compiler enumerates b's exports
export * from './c.mjs';
```

The compiler:
1. Enumerates all exports from source modules
2. Generates explicit re-export bindings
3. Errors on ambiguous names (same export from multiple sources)

### 5.2 Conflict Handling

```javascript
// b.mjs: export const x = 1;
// c.mjs: export const x = 2;
// a.mjs: export * from './b.mjs'; export * from './c.mjs';

// COMPILE ERROR: Ambiguous re-export 'x' from b.mjs and c.mjs
```

---

## 6. Default Export Semantics

### 6.1 Hoisting Distinction

The implementation **preserves the semantic difference** between:

```javascript
// Live binding - function is hoisted
export { myFunc as default };
export default function myFunc() {}
export default class MyClass {}

// Snapshot - expression evaluated at execution point
export default expression;
export default { ... };
```

Function/class declarations with `default` are hoisted; expression defaults are not.

### 6.2 Implementation

Compiler tracks whether default export is:
- Hoistable (function/class declaration) → slot initialized during linking
- Expression → slot initialized during execution via PrStore

---

## 7. Module Initialization

### 7.1 Eager Initialization

**All modules initialize eagerly at program start** (per spec):

1. Program entry calls generated `__esmInit()`
2. All namespace objects created
3. Modules evaluated in dependency order (post-order DFS)
4. After init completes, user code runs (which is the entry module's body)

### 7.2 Initialization IS User Code

There is no separate "init" phase - the generated init function **is** the program entry point. Module bodies execute as part of this initialization.

### 7.3 Error Handling

**Link errors** (missing exports, resolution failures): Compile-time errors, no bytecode generated.

**Evaluation errors** (module code throws):
- Entire program initialization fails
- No partial initialization observable
- Error propagates, program cannot start

---

## 8. Temporal Dead Zone (TDZ)

### 8.1 Simplified Semantics

Export slots are **initialized to `undefined`** rather than implementing full TDZ:

```javascript
// b.mjs
export let x;
console.log(x);  // undefined (not TDZ error)
x = 42;

// a.mjs
import { x } from './b.mjs';
// If accessed before b.mjs assigns x:
console.log(x);  // undefined (not TDZ error)
```

This diverges from spec but simplifies implementation. Full TDZ tracking may be added later if needed.

---

## 9. import.meta

### 9.1 Supported Properties

Only `import.meta.url` is supported:

```javascript
console.log(import.meta.url);  // Source file path from compilation
```

### 9.2 Object Identity

Each module gets a **unique `import.meta` object**. Not shared, no prototype tricks.

### 9.3 URL Format

The URL is the **compile-time source file path** as provided to the compiler. No runtime transformation.

---

## 10. CJS Interoperability

### 10.1 ESM Importing CJS

When ESM imports from CJS:

```javascript
// a.mjs
import cjsModule from './b.cjs';       // Default = exports object
import { named } from './b.cjs';       // Named = exports.named
import * as ns from './b.cjs';         // Namespace wrapping exports
```

**CJS exports are wrapped as a namespace object:**
- CJS `module.exports` becomes the backing object
- ESM sees it as a namespace with those properties
- `default` export is the entire `exports` object

### 10.2 CJS Requiring ESM

When CJS requires ESM:

```javascript
// b.cjs
const esmModule = require('./a.mjs');
```

**Returns a frozen snapshot** of the ESM namespace at `require()` call time:
- Snapshot copies all export values
- Object is frozen
- Loses live binding semantics (intentional - CJS expects static object)

### 10.3 Initialization Order

1. **ESM modules initialize first** (eager, at program start)
2. **CJS modules initialize on first `require()`**
3. When CJS `require()`s ESM, ESM is already initialized → snapshot is complete

### 10.4 Cycle Handling

**ESM ↔ CJS cycles are detected at runtime and throw an error:**

```javascript
// a.mjs imports from b.cjs
// b.cjs requires a.mjs
// → Runtime error in require()
```

Rationale: Cannot reliably detect `require()` calls at compile time, so cycles must be caught at runtime.

---

## 11. Strict Mode

**ESM modules are always strict mode** per spec:

- No `"use strict"` directive needed
- Non-strict features (e.g., `with`, octal literals) are syntax errors
- `this` at module level is `undefined`

---

## 12. Features NOT Supported in v1

### 12.1 Top-Level Await (TLA)

Deferred. Adds significant complexity:
- Async module evaluation
- Promise-based initialization
- Affects CJS interop

May be added in future version.

### 12.2 Dynamic import()

Not supported:

```javascript
const mod = await import('./dynamic.mjs');  // ERROR
```

Rationale: Conflicts with compile-time static graph. Would require runtime module loading infrastructure.

### 12.3 import.meta.resolve()

Not supported (requires dynamic import infrastructure).

---

## 13. Compiler Interface

### 13.1 New Flags

```bash
# Explicit module type (overrides auto-detection)
hermesc --esm entry.mjs -o bundle.hbc
hermesc --cjs entry.cjs -o bundle.hbc

# Auto-detection (default)
hermesc entry.js -o bundle.hbc  # Uses package.json "type" field
```

### 13.2 Error Messages

Clear compile-time errors for:
- Unresolved module specifiers
- Missing named exports
- Ambiguous star re-exports
- ESM syntax in CJS files (when detected as CJS)

---

## 14. Legacy Code Removal

### 14.1 Remove Transpile-to-CJS Path

The existing code in `lib/IRGen/ESTreeIRGen-stmt.cpp` (lines 1373-1570) that transpiles `import`/`export` to `require()` calls will be **removed entirely**.

### 14.2 Files Affected

- `lib/IRGen/ESTreeIRGen-stmt.cpp`: Remove `genImportDeclaration()` CJS transpilation
- `lib/IRGen/ESTreeIRGen-stmt.cpp`: Remove `genExportNamedDeclaration()` CJS transpilation
- `lib/IRGen/ESTreeIRGen-stmt.cpp`: Remove `genExportDefaultDeclaration()` CJS transpilation
- `lib/IRGen/ESTreeIRGen-stmt.cpp`: Remove `genExportAllDeclaration()` CJS transpilation
- Related: Remove `HermesBuiltin_exportAll()` if no longer needed

### 14.3 Existing CJS Infrastructure

The existing CJS implementation (`require()`, `Domain`, etc.) is **preserved**:
- Still used for `.cjs` files
- Still used for CJS modules in mixed bundles
- ESM has separate initialization path

---

## 15. Test262 Compliance

### 15.1 Target

**Full test262 module test compliance** except:
- Tests requiring top-level await
- Tests requiring dynamic import()

### 15.2 Known Divergences

| Feature | Spec | Hermes v1 |
|---------|------|-----------|
| TDZ for exports | ReferenceError | Returns undefined |
| TLA | Supported | Not supported |
| Dynamic import | Supported | Not supported |
| import.meta.resolve | Supported | Not supported |

---

## 16. Implementation Plan

### 16.1 Development Approach

**Spike then split:**
1. Prototype complete implementation in single branch
2. Verify end-to-end functionality
3. Split into reviewable incremental PRs

### 16.2 Implementation Phases

**Phase 1: Compiler Infrastructure**
- Module resolution (Node.js algorithm)
- Module type detection
- Dependency graph construction
- Compile-time export validation

**Phase 2: IR Generation**
- New `LoadModuleSlot` instruction (if needed)
- Init function generation
- Nested module function generation
- Namespace object creation and freezing
- Live binding codegen (indirect access)

**Phase 3: CJS Interop**
- ESM importing CJS (namespace wrapping)
- CJS requiring ESM (frozen snapshot)
- Cycle detection and error

**Phase 4: Cleanup and Testing**
- Remove legacy transpile-to-CJS code
- test262 compliance testing
- Performance profiling

---

## 17. Open Questions

1. **Package.json `exports` field**: Should conditional exports and subpath exports be supported? Needs investigation of Node.js behavior complexity vs. benefit.

2. **New bytecode instructions**: Exact count and design of new instructions (LoadModuleSlot, possibly others) to be determined during implementation.

3. **Optimization opportunities**: After profiling, consider:
   - Inlining constant exports
   - Dedicated namespace hidden class
   - Reducing LoadModuleSlot overhead

---

## Appendix A: Example Compilation

### Source

```javascript
// entry.mjs
import { greet } from './greeter.mjs';
console.log(greet('World'));

// greeter.mjs
export function greet(name) {
  return `Hello, ${name}!`;
}
```

### Generated IR (Conceptual)

```
function __esmInit():
  // Create namespaces
  ns_greeter = CreateObject()
  ns_entry = CreateObject()

  // Module: greeter.mjs (no dependencies, executes first)
  function greeter_module(exports):
    function greet(name):
      return "Hello, " + name + "!"
    PrStore(exports, "greet", greet)

  // Module: entry.mjs (depends on greeter)
  function entry_module(exports):
    // Live binding - every access goes through LoadModuleSlot
    tmp = LoadModuleSlot(ns_greeter, 0)  // slot 0 = "greet"
    Call(console.log, Call(tmp, "World"))

  // Execute in order
  greeter_module(ns_greeter)
  Freeze(ns_greeter)
  entry_module(ns_entry)
  Freeze(ns_entry)

// Program entry
__esmInit()
```

---

## Appendix B: CJS Interop Example

### Source

```javascript
// main.mjs (ESM)
import legacy from './legacy.cjs';
console.log(legacy.helper());

// legacy.cjs (CJS)
module.exports = {
  helper: function() { return 'from CJS'; }
};
```

### Behavior

1. ESM init creates namespace for `main.mjs`
2. When `main.mjs` evaluates `import legacy from './legacy.cjs'`:
   - CJS module `legacy.cjs` is require()'d (standard CJS execution)
   - CJS `exports` object wrapped as namespace
   - `legacy` bound to `exports` object (default import)
3. `legacy.helper()` accesses property on CJS exports object

### CJS Requiring ESM

```javascript
// app.cjs
const esm = require('./module.mjs');
console.log(esm.value);  // Frozen snapshot
```

1. ESM already initialized at program start
2. `require('./module.mjs')` returns frozen copy of ESM namespace
3. Changes to ESM exports NOT reflected in `esm` (it's a snapshot)
