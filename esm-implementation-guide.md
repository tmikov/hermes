# ESM Implementation Guide for Engine Implementors

## Overview

ECMAScript Modules (ESM) differ fundamentally from CommonJS in their loading model, binding semantics, and interoperability characteristics. This document describes the key implementation concerns from an engine perspective.

## The Three Phases

The ESM specification separates module processing into three distinct phases:

### 1. Parse (Load)

- Fetch and parse the module source
- Extract `import` and `export` declarations statically
- Build the list of dependencies (import specifiers)
- Create the Module Record structure

At this stage, no code has executed. The module graph structure is being discovered.

### 2. Link

- Recursively ensure all dependencies are parsed
- Create binding storage (environment record) for each module
- Wire up import bindings to their corresponding export bindings
- All export slots are created but left **uninitialized**

After linking completes, the entire module graph is connected. Every import binding points to an export slot, even if that slot doesn't yet contain a value.

### 3. Evaluate

- Execute module bodies in post-order (dependencies before dependents)
- Assignments to exported variables populate the export slots
- Imported bindings become readable once their source module initializes them

The phase separation matters primarily for async environments (browsers fetching over network), where you want to discover and fetch all dependencies before executing anything. In a synchronous environment, Parse and Link can be fused into a single pass, as long as export slots are created before recursing into dependencies.

## Module Records and Binding Storage

A Module Record contains:

- **Environment Record**: The binding storage for the module's variables
- **Export Entries**: Mapping from export names to local binding slots
- **Import Entries**: Mapping from local names to (source module, export name) pairs

The Environment Record is not a JavaScript object—it's an internal structure that the engine controls. This distinction is important because:

1. Bindings can be uninitialized (TDZ state), which is not representable as a normal JS value
2. Import bindings are indirect references, not copies
3. The engine can optimize access patterns

## Export Bindings and Live Binding Semantics

ESM exports are **live bindings**. The exporting module can mutate them; importers see updates.

```javascript
// counter.mjs
export let count = 0;
export function increment() { count++; }

// main.mjs
import { count, increment } from './counter.mjs';
console.log(count);  // 0
increment();
console.log(count);  // 1 — updated value visible
```

### Implementation Strategy

The exported variable cannot live in a local stack slot. It must reside in shared storage that importers can reference. Conceptually:

```
// Source
export let x = 0;
++x;

// Effective semantics
moduleBindings.x = 0;
++moduleBindings.x;
```

The importer's reference to `x` compiles to an indirection through the source module's binding storage, not a local variable access.

Importers receive **read-only** bindings. The engine should reject (at parse time or compile time) any assignment to an imported identifier.

## Temporal Dead Zone (TDZ) for Exports

Export bindings declared with `let`, `const`, or `class` are subject to TDZ. The slot exists after linking, but accessing it before the declaration executes must throw a `ReferenceError`.

For local variables, TDZ is often statically checkable. For cross-module access, it depends on evaluation order, which depends on the import graph:

```javascript
// a.mjs
import { y } from './b.mjs';
console.log(y);  // Does this throw?
export let x = 1;

// b.mjs
import { x } from './a.mjs';
console.log(x);  // Or does this throw?
export let y = 2;
```

One of these will throw. Whichever module evaluates first will access the other's uninitialized binding.

### Implementation Options

1. **Sentinel value**: Store a special internal marker (e.g., `TDZ_HOLE` or `empty`) in uninitialized slots. Every read checks for it.

2. **Initialized bit**: Maintain a boolean flag alongside each binding slot.

3. **Function hoisting optimization**: `function` declarations are initialized immediately during linking (they're hoisted). Only `let`/`const`/`class` exports need TDZ checks.

### Declaration Behavior in Cycles

Different declaration forms behave differently when accessed before evaluation:

| Declaration | After Link | After Evaluation |
|-------------|------------|------------------|
| `export function f() {}` | callable | callable |
| `export var x = 5` | `undefined` | `5` |
| `export let x = 5` | TDZ | `5` |
| `export const x = 5` | TDZ | `5` |
| `export class C {}` | TDZ | class |

Functions and `var` are hoisted and safe in cycles. `let`/`const`/`class` expose TDZ hazards.

### Partial Initialization via Re-entrancy

While all bindings start either hoisted or in TDZ, function calls during evaluation can expose partially-initialized state:

```javascript
// a.mjs
import { check } from './b.mjs';
export let x = 1;
check();            // call back into b mid-evaluation
export let y = 2;

// b.mjs
import { x, y } from './a.mjs';
export function check() {
  console.log(x);   // 1 — initialized
  console.log(y);   // TDZ error — not yet
}
```

Evaluation proceeds: `b.mjs` completes first, then `a.mjs` starts. When `check()` is called, `x` is initialized but `y` is not yet.

## The Module Namespace Exotic Object

When code uses `import * as ns from './mod.mjs'`, it receives a **Module Namespace Exotic Object**. This is not a regular JavaScript object—it has overridden internal methods.

### Exotic Behavior

The namespace object's `[[Get]]` internal method reads from the module's binding storage, not from normal property storage. This means:

- TDZ is observable through the namespace object
- Live binding updates are visible
- The object appears to have properties that can throw when accessed

```javascript
// In a cycle where a.mjs hasn't finished evaluating:
import * as a from './a.mjs';

'x' in a                              // true
Object.keys(a)                        // throws ReferenceError
Object.getOwnPropertyDescriptor(a, 'x')  // throws ReferenceError
a.x                                   // throws ReferenceError
```

The property exists (passes the `in` check), but any operation that reads its value throws.

### Property Descriptor Illusion

When TDZ is not in effect, `Object.getOwnPropertyDescriptor(ns, 'x')` returns a data descriptor:

```javascript
{ value: 1, writable: true, enumerable: true, configurable: false }
```

The `writable: true` is misleading—external assignment still fails because the namespace object's `[[Set]]` always rejects. The descriptor reflects the binding's characteristics, not normal property semantics.

## CommonJS Interoperability

### ESM Importing CommonJS

This direction involves a phase mismatch. CJS doesn't have static exports—`module.exports` is determined at runtime. But ESM linking wants to know export names before evaluation.

**Parse phase:**
- ESM is parsed, dependencies identified
- CJS is also parsed—`cjs-module-lexer` performs static analysis to *guess* export names based on syntactic patterns like `exports.foo = ...`

**Link phase:**
- ESM binding infrastructure is created
- CJS namespace objects are created with the guessed property *names* (but no values yet)
- The `default` export slot is prepared

**Evaluate phase:**
- Modules execute in post-order (dependencies before dependents)
- CJS and ESM interleave naturally based on the dependency graph
- When CJS finishes executing, named export *values* are snapshotted from `module.exports`
- The `default` export is set to the live `module.exports` object

The key point: CJS does **not** run "before" ESM in any special way. It runs in normal dependency order. If ESM A imports CJS B, and CJS B requires ESM C, the order is: C evaluates → B evaluates (with `require(C)` returning immediately since C is done) → A evaluates.

The module cache is shared across the ESM/CJS boundary. If both ESM and CJS import the same module, it only executes once.

**Live binding behavior:**

Named exports are snapshots—copies made when the CJS module finishes executing:

```javascript
// cjs.cjs
exports.count = 0;
exports.increment = function() { exports.count++; };

// esm.mjs
import { count, increment } from './cjs.cjs';

console.log(count);   // 0
increment();
console.log(count);   // 0 — still 0, it's a snapshot
```

The `default` export is the actual `module.exports` object, so mutations are visible:

```javascript
// esm.mjs
import cjs from './cjs.cjs';

console.log(cjs.count);   // 0
cjs.increment();
console.log(cjs.count);   // 1 — live object
```

This asymmetry is unavoidable. CJS wrote a plain value to a plain object property; there's no binding indirection to preserve. The static analysis can identify *names*, but it can't create live bindings to arbitrary object properties without engine-level changes to CJS semantics.

### CommonJS Requiring ESM (Node.js 22+)

Node.js allows `require()` to load ESM modules that do not use top-level `await`. The returned value is the Module Namespace Exotic Object—the same object ESM `import *` receives.

```javascript
// esm.mjs
export let x = 1;
export function increment() { x++; }

// cjs.js
const ns = require('./esm.mjs');
console.log(ns.x);  // 1
ns.increment();
console.log(ns.x);  // 2 — live binding preserved!
```

Because CJS receives the actual namespace object (not a copy), live binding semantics are preserved in this direction.

### Cycle Restriction

Node.js explicitly forbids `require(esm)` when it would create a cycle with an ESM module that hasn't finished evaluating:

```
Error [ERR_REQUIRE_CYCLE_MODULE]: Cannot require() ES Module in a cycle.
```

This prevents CJS from ever holding a reference to a namespace object with uninitialized bindings (TDZ state). Without this guard, CJS code would encounter the "cursed object" behavior where properties exist but throw on access.

The restriction means `require(esm)` either:
1. Returns a fully-initialized namespace object (no cycle), or
2. Throws an error (cycle detected)

ESM-to-ESM cycles can still observe TDZ on namespace objects, but CJS is protected from this edge case.

## Top-Level Await

Top-level `await` (TLA) allows `await` expressions at module scope, outside of any `async function`. This has significant implications for module loading.

### Static Detection

TLA is syntactically visible at parse time. You scan the AST for `await` at module scope—no control flow analysis, no cross-module reasoning needed. This is just syntax detection, not reachability analysis:

```javascript
// This module has TLA, even if the branch is never taken
if (false) {
    await something();
}
```

### TLA is Viral Upward

If module A imports module B, and B has TLA, then A must await B's evaluation before A can begin. A becomes async even if A contains no `await` itself.

```
a.mjs (no await, but becomes async)
  └── import './b.mjs' (has TLA)
```

The async-ness propagates upward through the import graph to all modules that transitively depend on a TLA module.

### TLA Does Not Propagate Downward

The inverse is not true. If an async module imports a sync one, the sync subgraph resolves immediately and synchronously:

```
a.mjs (has TLA)
  └── import './b.mjs' (sync)
        └── import './c.mjs' (sync)
```

Evaluation order:
1. `c.mjs` — sync, runs to completion immediately
2. `b.mjs` — sync, runs to completion immediately
3. `a.mjs` — async, starts running, hits `await`, suspends

The sync subgraph doesn't become async just because its dependent is async.

### Implications for `require(esm)`

Synchronous `require()` cannot load ESM modules that use TLA. But the check isn't just on the target module—Node.js must verify the **entire transitive dependency closure** is TLA-free:

```
entry.cjs
  └── require('./a.mjs')     ← wants sync
        └── import './b.mjs'
              └── import './c.mjs'  ← has TLA — blocks everything!
```

Node walks the entire graph at parse time, checks every module for TLA, and rejects the `require()` if any module in the subgraph has it. If TLA is found, `require()` throws `ERR_REQUIRE_ASYNC_MODULE`.

The only way to load TLA modules from CJS is via dynamic `import()`, which returns a Promise:

```javascript
// cjs.js
async function main() {
    const ns = await import('./async-module.mjs');
}
main();
```

### Why Phase Separation Matters

This is why the spec's phase separation (Parse → Link → Evaluate) matters even for synchronous implementations. You must parse the entire graph before you can know whether evaluation will be sync or async. A single TLA anywhere in the closure determines the execution model for the whole subgraph.

## Summary of Key Implementation Points

1. **Phase separation**: Parse/Link must complete before Evaluate begins. Export slots exist (uninitialized) before any module code runs. The full graph must be parsed to determine if evaluation will be sync or async.

2. **Binding storage**: Exports live in a shared location, not local slots. Imports are indirect references to that storage.

3. **TDZ for exports**: Cross-module TDZ depends on evaluation order. Consider sentinel values or initialized flags. Note that `function` and `var` exports are safe in cycles; `let`/`const`/`class` are hazardous.

4. **Namespace object is exotic**: Its `[[Get]]` reads from binding storage, making TDZ and live updates observable through what looks like a property access.

5. **CJS interop asymmetry**: ESM→CJS loses live bindings (snapshot). CJS→ESM preserves them (receives namespace object directly).

6. **Cycle guard**: Node prevents `require(esm)` cycles to avoid exposing TDZ-state namespace objects to CJS.

7. **TLA is viral upward**: A module importing a TLA module becomes async itself, even without its own `await`. Sync subgraphs below async modules still resolve synchronously.

8. **TLA blocks synchronous require**: The entire transitive dependency closure must be TLA-free for `require(esm)` to succeed.
