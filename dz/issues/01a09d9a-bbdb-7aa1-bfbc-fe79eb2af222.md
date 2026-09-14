---
id: 01a09d9a-bbdb-7aa1-bfbc-fe79eb2af222
title: "Export names are published with ordinary stores: an Object.prototype
  setter runs during instantiation and swallows the export"
type: bug
status: open
resolution: null
component: wasm
assignee: null
created: 2026-09-14T01:49:18.683Z
creator: Tzvetan Mikov <tmikov@gmail.com>
---

Found 2026-09-14 by external review of 01a07f25-046f's fix, while checking a
comment that claimed instantiation runs no user JS.

finalizeModule builds the exports object with AllocObjectLiteralInst
and fills it with StorePropertyStrictInst, one per export name. An ordinary
store walks the prototype chain, and the exports object's prototype is
Object.prototype, so an accessor installed there named after an export
intercepts the store.

MEASURED, on a module exporting a single function "f":

  Hermes:  setter fired: 1 times, exports.f is: undefined
  node 24: setter fired: 0 times, exports.f is: function

Two consequences, the same pair 01a07f25-046f had for __wasm_type__:

  - user JS runs at a point the engine did not intend to yield. Instantiation
    is not indivisible in Hermes today -- import property reads and a start
    function both run script -- but a store of an export NAME is not one of
    the places that should;
  - the store is SWALLOWED, so the export is missing from the exports object
    entirely. A module whose export name collides with an Object.prototype
    accessor silently loses that export.

It is not only functions: the global, memory, table and tag export loops all
publish with the same instruction.

FIX DIRECTION. Define the properties rather than assigning them, so the
prototype chain is not consulted. DefineOwnPropertyInst is the instruction for
that and takes an isEnumerable flag; exports are enumerable own properties, so
that flag is true. Note this is NOT the same problem __wasm_type__ had -- there
the value also had to be unforgeable afterwards, which is why that one needed
an internal property. Here all that is wrong is the store consulting a setter and losing the value.
The object is frozen before the Instance is RETURNED (WebAssembly.cpp:874),
which is not the same as before script sees it: the intercepted setter
receives the exports object as `this`, so script has it in hand while it is
still mutable and still being populated.

A null prototype on the exports object would also settle it, and is worth
weighing against what node does. Freezing earlier would not: the stores that
populate it have to happen first.

## Log

- 2026-09-14T01:49:18.683Z  Tzvetan Mikov <tmikov@gmail.com>  created
