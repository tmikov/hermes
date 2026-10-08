/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// new WebAssembly.Module(bytes) reaching a linked native unit through the
// embedder's lookup_native, rather than through fromNativeUnit(). The addon
// answers with the unit named by install(); counts() reports which tiers
// Hermes then asked. A unit runs only under its own codegen configuration:
// lkmod is t262=0 and lkmod262 is t262=1, so each run accepts exactly one,
// and the other falls through to lookup and a compile. throwmod's top level
// throws, which must be the module's error -- not a fallback.
//
// The linked units are NOT the module below: lkmod.wat_ exports the same
// "add" but subtracts. lookup_native never looks at the bytes, so the result
// itself says which tier answered -- add(2, 3) is -1 when a native unit ran
// and 5 when these bytes were compiled -- independently of the counters.
// The counters pin the protocol: a native hit asks neither lookup, store
// nor discard, and a miss ends in exactly one of store or discard.

// REQUIRES: napi, wasm, shermes
// UNSUPPORTED: windows, qemu_mode
// RUN: sed 's#@ADDON@#%napi_addon_dir/napi_wasm_lookup_addon.node#' %s > %t.js
// RUN: %hermes %t.js | %FileCheck --match-full-lines --check-prefix=PLAIN %s
// RUN: %hermes -test262 %t.js | %FileCheck --match-full-lines --check-prefix=T262 %s

var addon = loadNativeModule('@ADDON@');
// The driver's module: add(a, b) = a + b. Not lkmod.wat_, which subtracts.
var bytes = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x07, 0x01, 0x60,
  0x02, 0x7f, 0x7f, 0x01, 0x7f, 0x03, 0x02, 0x01, 0x00, 0x07, 0x07, 0x01,
  0x03, 0x61, 0x64, 0x64, 0x00, 0x00, 0x0a, 0x09, 0x01, 0x07, 0x00, 0x20,
  0x00, 0x20, 0x01, 0x6a, 0x0b]);

function run(mode) {
  var installed = addon.install(mode);
  var result;
  try {
    result = new WebAssembly.Instance(new WebAssembly.Module(bytes), {})
        .exports.add(2, 3);
  } catch (e) {
    result = 'threw ' + e.message;
  }
  var c = addon.counts();
  print(mode + ' (' + installed + '): ' + result + ' native=' + c.native +
      ' lookup=' + c.lookup + ' store=' + c.store + ' discard=' + c.discard +
      ' accepted=' + c.accepted);
}

run('lkmod');
run('lkmod262');
run('none');
run('throwmod');

// PLAIN: lkmod (true): -1 native=1 lookup=0 store=0 discard=0 accepted=0
// PLAIN-NEXT: lkmod262 (true): 5 native=1 lookup=1 store=1 discard=0 accepted=0
// PLAIN-NEXT: none (true): 5 native=1 lookup=1 store=1 discard=0 accepted=0
// PLAIN-NEXT: throwmod (true): threw boom native=1 lookup=0 store=0 discard=0 accepted=0

// T262: lkmod (true): 5 native=1 lookup=1 store=1 discard=0 accepted=0
// T262-NEXT: lkmod262 (true): -1 native=1 lookup=0 store=0 discard=0 accepted=0
// T262-NEXT: none (true): 5 native=1 lookup=1 store=1 discard=0 accepted=0
// T262-NEXT: throwmod (true): 5 native=1 lookup=1 store=1 discard=0 accepted=0
