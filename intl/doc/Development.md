# Working on the Intl plugin

This document gives the rules for changing the Hermes Intl plugin that the
code cannot enforce: the rules for its JavaScript, adding a native helper,
and updating the CLDR data. The [README](../README.md) is the
introduction; the design is in
[the spec](../../doc/superpowers/specs/2026-10-22-intl-plugin-design.md).

## Where to start reading

Paths are relative to `intl/`.

- `jsi/Plugin.cpp`: the entry point Hermes calls, which runs the embedded
  bytecode and passes the setup function the native helpers and the
  defaults.
- `js/00-header.js` and `js/01-intrinsics.js`: how the JavaScript files
  are put together, and the intrinsics they may call.
- `jsi/NativeHelpers.def`: every native helper the JavaScript calls.
- `core/src/lib.rs`: the C interface of the Rust core.

The plugin never includes Hermes internals. Code here includes only
`<jsi/...>`, `<hermes/Public/IntlPlugin.h>`, its own headers and the
generated `hermes_intl_core.h`; `test/unit/` is the one exception.

## Writing plugin JS

User code must not be able to change what `Intl` does by tampering with
built-ins (the spec's
[Resistance to tampering](../../doc/superpowers/specs/2026-10-22-intl-plugin-design.md#resistance-to-tampering));
`test/lit/tamper.js` checks this. Every file in `js/` follows these rules,
and reviewers check them:

1. Call only intrinsics captured in `01-intrinsics.js`. Never call a property
   of a user-reachable object except where the spec does a `Get`/`Call` on
   user values.
2. No syntax that consults user-modifiable state: no spread, no `for-of`, no
   array destructuring or array patterns, no tagged templates, no
   `Array.prototype` methods, no `String.prototype.split` (it looks up
   `@@split`).
3. Append to arrays with `listPush` and replace an element with `listSet`,
   never `a[i] = v`, which runs setters on `Array.prototype`. Build every
   property descriptor and internal record with `__proto__: null`.
4. `ToString(v)` is `` `${v}` ``, `ToNumber(v)` is `+v`, and `ToObject`
   throws on `null`/`undefined` and then calls the captured `Object`.
5. Every function cites the spec section and numbers its steps in comments.
   The file header names the ECMA-402 draft it follows: the
   https://tc39.es/ecma402/ draft dated on the day the file is written. If
   the draft's step text differs from the code, follow the draft.
6. Code between `00-header.js` and `99-footer.js` is written at column 0, so
   lit tests can concatenate the files outside the IIFE.

## Adding a native helper

1. Add one line to `jsi/NativeHelpers.def`, as its header comment
   describes, with a comment saying what the helper does.
2. Define `hermes_intl::helpers::name` in a `jsi/*Helpers.cpp` file.
   `NativeHelpers.h` declares it from the table: it takes the
   `jsi::Runtime &` and a `const` reference to each argument's C++ type.
   The host function and the conversions are generated from the table too.
   A new conversion tag needs a `TagType` and a `Convert` in
   `NativeHelpers.h`.
3. JS calls it as `native.name(...)`, after validating every argument.

Internal errors are C++ exceptions; Hermes turns them into an `Error`.

## Updating the CLDR data

The CLDR-derived files in `core/data/` must come from the CLDR release
that the plugin's ICU4X version uses. When ICU4X is upgraded, regenerate
them from that release's tag and commit the result:

```
python3 intl/core/data/update-cldr-lists.py release-48-2
```

The generator's docstring lists the files and where each comes from, and
each file's header comment gives its format. `check-intl-data` tests the
generator. `core/data/LICENSE` is the Unicode License V3 for these files.

Some lists are kept by hand and need checking at an upgrade too:
`core/data/locales.txt` is the set of locales ICU4X's compiled data covers
(ICU4X's "recommended" set) and has no generator; replace it with the new
release's set by hand. The calendar and collation lists are in
`core/src/supported.rs`, and the unit list is in `js/10-Intl.js`.
