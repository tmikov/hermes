/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// WebAssembly.compile and WebAssembly.instantiate hand their result -- a
// Module, an Instance, a {module, instance} object, or a thrown error -- to
// two internal helpers that look up globalThis.Promise and then
// Promise.resolve / Promise.reject before calling either. All three of those
// properties are replaceable, so both lookups can run user script; the helpers
// used to carry the payload across them in an unrooted HermesValue parameter.
// The rejection path is the worse of the two, because every caller reaches it
// by taking the thrown value out of the runtime and calling clearThrownValue,
// so that parameter is the only reference to the error object.
//
// This test replaces globalThis.Promise, Promise.resolve and Promise.reject
// with accessors that ALLOCATE, so both lookups genuinely move the heap, and
// then asserts the payload survived. It must run with -gc-sanitize-handles=1
// EXPLICITLY: a HERMESVM_SANITIZE_HANDLES=ON build samples at 1% by default,
// so without the flag these paths mostly run on a heap that did not move. In a
// build without HERMESVM_SANITIZE_HANDLES the flag is ignored and this is an
// ordinary behavioural test of the same API.

// RUN: %hermes -gc-sanitize-handles=1 %s | %FileCheck --match-full-lines %s
// REQUIRES: wasm

// A module exporting answer() -> 42.
var withExport = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d,
  0x01, 0x00, 0x00, 0x00,
  0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7f,
  0x03, 0x02, 0x01, 0x00,
  0x07, 0x0a, 0x01,
  0x06, 0x61, 0x6e, 0x73, 0x77, 0x65, 0x72,
  0x00, 0x00,
  0x0a, 0x06, 0x01, 0x04, 0x00, 0x41, 0x2a, 0x0b
]);

// A module importing env.add, so a bad import object produces a LinkError
// rejection -- a different rejection call site from the compile failures.
var withImport = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d,
  0x01, 0x00, 0x00, 0x00,
  0x01, 0x07, 0x01, 0x60, 0x02, 0x7f, 0x7f, 0x01, 0x7f,
  0x02, 0x0b, 0x01,
  0x03, 0x65, 0x6e, 0x76,
  0x03, 0x61, 0x64, 0x64,
  0x00, 0x00,
  0x03, 0x02, 0x01, 0x00,
  0x07, 0x0b, 0x01,
  0x07, 0x63, 0x61, 0x6c, 0x6c, 0x41, 0x64, 0x64,
  0x00, 0x01,
  0x0a, 0x0a, 0x01, 0x08, 0x00,
  0x41, 0x03, 0x41, 0x04, 0x10, 0x00, 0x0b
]);

var badBytes = new Uint8Array([0, 0, 0, 0]);

// Everything is captured before the accessors are installed, so the getters
// can hand back the genuine values without recursing.
var realPromise = Promise;
var realResolve = Promise.resolve;
var realReject = Promise.reject;

// Allocate enough to guarantee the heap moves inside the lookup itself.
function churn(tag) {
  var junk = [];
  for (var i = 0; i < 40; i++) {
    junk.push({tag: tag, i: i, s: tag + '-' + i});
  }
  return junk.length;
}

Object.defineProperty(globalThis, 'Promise', {
  configurable: true,
  get: function () { churn('P'); return realPromise; }
});
Object.defineProperty(realPromise, 'resolve', {
  configurable: true,
  get: function () { churn('R'); return realResolve; }
});
Object.defineProperty(realPromise, 'reject', {
  configurable: true,
  get: function () { churn('J'); return realReject; }
});

// --- Fulfil: compile(bytes) -> Module.
WebAssembly.compile(withExport).then(function (mod) {
  var descs = WebAssembly.Module.exports(mod);
  print('compile: ' + (mod instanceof WebAssembly.Module) +
        ' ' + descs.length + ' ' + descs[0].name);
});
// CHECK: compile: true 1 answer

// --- Fulfil: instantiate(bytes) -> {module, instance}. The result object is
// --- built immediately before the helper runs, so it is as young as a
// --- payload gets.
WebAssembly.instantiate(withExport).then(function (result) {
  print('instantiate bytes: ' +
        (result.module instanceof WebAssembly.Module) + ' ' +
        (result.instance instanceof WebAssembly.Instance) + ' ' +
        result.instance.exports.answer());
});
// CHECK-NEXT: instantiate bytes: true true 42

// --- Fulfil: instantiate(module) -> Instance, the other resolve call site.
var mod = new WebAssembly.Module(withExport);
WebAssembly.instantiate(mod).then(function (inst) {
  print('instantiate module: ' +
        (inst instanceof WebAssembly.Instance) + ' ' + inst.exports.answer());
});
// CHECK-NEXT: instantiate module: true 42

// --- Reject: compile of invalid bytes. The error is read back, which
// --- dereferences it, so a stale payload is not merely a wrong identity.
WebAssembly.compile(badBytes).then(
  function () { print('compile bad: FULFILLED, expected rejection'); },
  function (err) {
    print('compile bad: ' + (err instanceof WebAssembly.CompileError) +
          ' ' + (typeof err.message === 'string'));
  }
);
// CHECK-NEXT: compile bad: true true

// --- Reject: instantiate of invalid bytes.
WebAssembly.instantiate(badBytes).then(
  function () { print('instantiate bad: FULFILLED, expected rejection'); },
  function (err) {
    print('instantiate bad: ' + (err instanceof WebAssembly.CompileError) +
          ' ' + (typeof err.message === 'string'));
  }
);
// CHECK-NEXT: instantiate bad: true true

// --- Reject: a link failure rather than a compile failure, which is the
// --- rejection call site after a successful compile.
WebAssembly.instantiate(withImport, {env: {}}).then(
  function () { print('instantiate link: FULFILLED, expected rejection'); },
  function (err) {
    print('instantiate link: ' + (err instanceof WebAssembly.LinkError) +
          ' ' + (typeof err.message === 'string'));
  }
);
// CHECK-NEXT: instantiate link: true true

// --- Reject: instantiate(module, badImports), the overload-2 rejection.
WebAssembly.instantiate(new WebAssembly.Module(withImport), {env: {}}).then(
  function () { print('instantiate module link: FULFILLED, expected'); },
  function (err) {
    print('instantiate module link: ' +
          (err instanceof WebAssembly.LinkError) + ' ' +
          (typeof err.message === 'string'));
  }
);
// CHECK-NEXT: instantiate module link: true true
