---
id: 01a09d9a-bbdb-7aa1-bfbc-fe79eb2af222
title: "Export names are published with ordinary stores: an Object.prototype
  setter runs during instantiation and swallows the export"
type: bug
status: closed
resolution: fixed
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
- 2026-09-14T04:43:30.191Z  Tzvetan Mikov <tmikov@gmail.com>  status: open -> closed (fixed)
    Fixed 2026-09-14. Every export name and every description-object property is
    DEFINED rather than assigned, via DefineOwnPropertyInst, which does not
    consult the prototype chain.

    Measured on a module exporting one of each kind (function, memory, table,
    global, tag), with an Object.prototype accessor installed under each export
    name and under "name"/"kind"/"module":

      before:  accessors fired during instantiation: 25
               f/m/t/g/e all undefined; Object.keys(exports) empty
      after:   accessors fired during instantiation: 0
               all five present and of the right type; keys e,f,g,m,t; frozen

    Node v24.13.1 on the same probe: 0 fired, the same keys, frozen. So this was
    worse than the description above said -- not "an export may be swallowed", but
    every export of every kind, and the exports object comes out empty.

    Twelve publication sites: seven on the exports object (one per export kind,
    plus the funcref/externref table split and the imported-mutable-global
    re-export) and five on the export/import description objects.

    NOT CHANGED, deliberately: the descriptor objects passed to the
    WebAssembly.Memory and WebAssembly.Table constructors still use ordinary
    stores. Defining those would close the descriptor-tampering route and so make
    the exact-limits checks in createMemoryViews()/createTables() unreachable,
    which is a different decision with its own test (e2e-pristine-descriptor.wat)
    to retire. Worth doing; not folded in here.

    Pinned by test/wasm/e2e-export-store-accessor.wat, shown red by reverting one
    of the twelve.
- 2026-09-14T05:00:34.707Z  Tzvetan Mikov <tmikov@gmail.com>  comment
    FOLLOW-UP, same day, after external review. Three more publication sites and
    one false claim.

    MORE SITES, all the same defect:

      - The module-factory result object's `instantiate`, `exportDescs` and
        `importDescs`. This one is not merely data loss. An
        Object.prototype.instantiate setter swallows the closure and its getter
        supplies a replacement, which the JS API then calls; the replacement can
        invoke the real closure, rewrite the exports object it returns, and hand
        back the rewritten one -- which the JS API FREEZES. Measured on a module
        whose f() returns 42:

            swallowed the engine closure: true
            exports.f() = 1337
            exports frozen: true

        So "the exports object is frozen before the Instance is returned" is not
        an integrity argument by itself, which is how the first version of this
        fix justified leaving that store alone.

      - The NATIVE description objects built by WebAssembly.Module.exports() and
        .imports(), which use putNamed_RJS. With accessors installed they came
        back as `{}` -- every field missing, 14 accessor calls.

      - The `{module, instance}` object WebAssembly.instantiate(bytes) resolves
        with, same mechanism.

    A FALSE CLAIM, retracted. The first version said that defining the
    WebAssembly.Memory/Table constructor descriptors "would close the
    descriptor-tampering route, and with it make the exact-limits checks
    unreachable". It would not. A descriptor OMITS `maximum` when the module
    declares none, and the constructor reads it through the prototype chain, so an
    inherited GETTER supplies one with no store to intercept and nothing for a
    define to overwrite. Measured on `(memory 1)`: a `maximum` getter returning 2
    still reaches the constructor and is caught by the limits check. Closing that
    route needs a null-prototype descriptor or constructors that ignore inherited
    properties -- a separate decision, and the one 01a09e43-b6b6 turns on.
