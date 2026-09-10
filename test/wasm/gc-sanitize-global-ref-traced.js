/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// JSWebAssemblyGlobal::value_ became a GC reference when reference-typed
// globals landed: it used to be a `double` plus an `int64_t`, neither of
// which the metadata registered, and it now holds a BigInt for an i64 global
// and an arbitrary JS value for an externref one. This test is the executable
// form of `mb.addField("value", &self->value_)` in
// JSWebAssemblyGlobalBuildMeta.
//
// Measured with that line deleted, in this build, with each half of the file
// run on its own: neither half prints anything, because the process aborts
// first. The lone subject trips
// `"Setting an invalid pointer into a GCHermesValue"` in
// GCHermesValue::set (HermesValue-inline.h); the shared subject trips
// `"A pointer is pointing outside of the valid region"` in
// CheckHeapWellFormedAcceptor::accept. Both are Debug-only checks, so what
// the CHECK lines below catch in this configuration is the missing output,
// not a printed `false`.
//
// The two subjects are aimed separately at two properties of a registered
// field that a JS-level test can observe:
//
//   Marking. The lone subject is reachable through nothing but the Global's
//   value_, so an unregistered field means the collector never marks it.
//
//   Relocation. The shared subject is ALSO held by an ordinary JS array,
//   which keeps it alive and gives the test a correctly-relocated pointer to
//   compare identity against; an unregistered value_ is left pointing at the
//   pre-move address.
//
// Both subjects are IMMUTABLE globals, and both are built by the JS
// constructor rather than exported by a module. Each choice removes a
// competing holder of the referent:
//
//   - An immutable Global stores its value in value_ and has no getter or
//     setter, so within the cell that field is the only place the referent
//     can be.
//   - A module-exported MUTABLE global is unusable as a subject here for a
//     reason specific to that path: it is published live, backed by getter
//     and setter closures over the module's own frame slot, and the slot is
//     traced as part of the frame. Its referent would survive with value_
//     unregistered, and the test would report green.
//
// value_ is one field of one class, so the metadata it needs does not vary
// with the route that filled it; the constructor is used because it is the
// route on which the caller can be left holding the Global and nothing else.
//
// It must run with -gc-sanitize-handles=1 EXPLICITLY: a
// HERMESVM_SANITIZE_HANDLES=ON build samples at 1% by default, so without the
// flag the heap mostly does not move and the identity check would compare a
// pointer against itself. In a build without HERMESVM_SANITIZE_HANDLES the
// flag is ignored, gc() still collects, and this is a weaker but still
// meaningful test of the marking half.

// RUN: %hermes -Xhermes-internal-test-methods -gc-sanitize-handles=1 %s | %FileCheck --match-full-lines %s
// REQUIRES: wasm

// The subject is allocated inside this function and handed straight to the
// constructor, so the caller never names it. Writing the literal at the call
// site instead would leave it in a live register of the calling frame, which
// the collector scans, and that register would root it whatever the metadata
// said.
function makeLoneSubject() {
  return new WebAssembly.Global(
      {value: 'externref'}, {tag: 'lone', n: 4321});
}

// A second Global whose referent is also held by a plain JS array. The array
// element is traced by ArrayStorage, so it reports where the object actually
// lives after a collection.
function makeSharedSubject() {
  var o = {tag: 'shared'};
  return {global: new WebAssembly.Global({value: 'externref'}, o), keep: [o]};
}

var lone = makeLoneSubject();
var shared = makeSharedSubject();

// Allocation under -gc-sanitize-handles=1 is itself what moves the heap; the
// explicit collections are here so this file still exercises the marking half
// in a build where the flag is ignored.
gc();
for (var i = 0; i < 100; i++) {
  var junk = {i: i, s: 'filler' + i};
}
gc();

// The marking half. Read back through the property values rather than through
// `typeof` or a truthiness test: an untraced field that happened to survive
// as a valid cell of some other shape would satisfy those.
var v = lone.value;
print('lone subject survives: ' +
      (v !== null && v.tag === 'lone' && v.n === 4321));
// CHECK: lone subject survives: true

// The relocation half. Identity, not contents: contents would also match if value_
// pointed at a stale copy of the same object at its pre-move address.
print('shared subject keeps its identity: ' +
      (shared.global.value === shared.keep[0]));
// CHECK-NEXT: shared subject keeps its identity: true

print('done');
// CHECK-NEXT: done
