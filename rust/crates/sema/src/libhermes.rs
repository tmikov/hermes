/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Port of `include/hermes/Runtime/Libhermes.h:13-72`: the list of built-in
//! symbols declared by the HermesVM runtime, as a JS source string.
//!
//! The C++ side is a single `const char libhermes[]` built from adjacent
//! string literals, with the twelve typed-array constructors spliced in by
//! `#include "hermes/VM/TypedArrays.def"` under a local `TYPED_ARRAY(name,
//! type)` macro expanding to `"function " #name "Array() {}"`. Rust has no
//! preprocessor, so the include is expanded by hand below: the twelve lines
//! from `function Uint8Array() {}` through `function BigInt64Array() {}`,
//! in `TypedArrays.def`'s order (`TYPED_ARRAY_NO_CLAMP` is NOT defined at
//! the include site, so `Uint8Clamped` is included). The empty `""`
//! literals in the C++ source are pure formatting (they separate logical
//! groups and concatenate to nothing) and appear here as blank lines in the
//! string.
//!
//! Two spellings are load-bearing and deliberately preserved verbatim:
//! - `"function escape()  {}"` / `"function unescape()  {}"` have TWO
//!   spaces before the brace in the C++ source.
//! - the whole constant is a single line in C++ (no separators at all
//!   between the concatenated literals); here it is one statement per line.
//!   Both parse to the same AST, and only the declared *names* reach the
//!   dump, so the layout difference is not observable. It is a difference
//!   in the constant's bytes, though, hence this note.
//!
//! This string is parsed by the `sema-dump` bin exactly like CompilerDriver's
//! `loadGlobalDefinition` (CompilerDriver.cpp:762-774, called at :2001-2007)
//! does, and the resulting `Program` becomes the single ambient-declaration
//! file passed to `resolve::resolve_ast`. The 63 `UndeclaredGlobalProperty`
//! decls the empty-input dump contains are exactly this list, deduplicated
//! (`Error` appears both as a `var` and as a `function`), and the
//! `sema_differential` test enforces that byte-for-byte against
//! `hermesc -dump-sema`.
pub const LIBHERMES: &str = "\
var Array;
var BigInt;
var Boolean;
var Date;
var Error;
var Function;
var HermesInternal;
var HermesAsyncIteratorsInternal;
var JSON;
var Map;
var Math;
var Number;
var Object;
var Proxy;
var Reflect;
var RegExp;
var Set;
var String;
var Symbol;
var WeakMap;
var WeakSet;

var $SHBuiltin;
var Hermes;

var Infinity;
var NaN;
var globalThis;
var undefined;

function Error() {}
function AggregateError() {}
function EvalError() {}
function RangeError() {}
function ReferenceError() {}
function SyntaxError() {}
function TypeError() {}
function URIError() {}
function ArrayBuffer() {}
function DataView() {}
function TextEncoder() {}
function Worker() {}
function Uint8Array() {}
function Int8Array() {}
function Uint8ClampedArray() {}
function Uint16Array() {}
function Int16Array() {}
function Uint32Array() {}
function Int32Array() {}
function Float16Array() {}
function Float32Array() {}
function Float64Array() {}
function BigUint64Array() {}
function BigInt64Array() {}

function print() {}
function eval() {}
function parseInt() {}
function parseFloat() {}
function isNaN() {}
function isFinite() {}
function escape()  {}
function unescape()  {}
function decodeURI() {}
function decodeURIComponent() {}
function encodeURI() {}
function encodeURIComponent() {}
function gc() {}";
