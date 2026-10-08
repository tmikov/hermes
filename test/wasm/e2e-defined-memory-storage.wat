;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A module's OWN linear memory must be the storage of the WebAssembly.Memory
;; it exports, and that storage must be reached the same way an imported
;; memory's is: through a brand check, out of the internal field.
;;
;; createMemoryViews() used to build the memory with
;; `new globalThis.WebAssembly.Memory(descriptor)` and then read `.buffer` off
;; it as an ordinary property. That accessor is a CONFIGURABLE property of
;; WebAssembly.Memory.prototype, so replacing it substituted the module's
;; entire linear memory with storage script chose, while the Memory object the
;; module exported -- which an importing module brand-checks and therefore
;; TRUSTS -- still pointed at its own, untouched buffer. The consumer module
;; below is the cross-module consequence: it links against a genuine
;; WebAssembly.Memory, the brand check passes, and it is handed a buffer that
;; is provably not the exporter's linear memory.
;;
;; The constructor's result is brand-checked with the same wasmLinkMemory call
;; the import path uses, and the buffer comes back from that call rather than
;; from a second, replaceable property read. The constructor itself is no
;; longer replaceable either -- it is read from
;; HermesInternal.intrinsics.WebAssembly -- but the DESCRIPTOR it is handed
;; still is, which is what the limits rows at the bottom exercise.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %wat2wasm %S/e2e-defined-memory-storage-consumer.wat_ -o %t-c.wasm && %hermesc --wasm -emit-binary -out %t-c.hbc %t-c.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-defined-memory-storage-driver.js_ -- %t.hbc %t-c.hbc | %FileCheck --match-full-lines %s

(module
  (memory (export "mem") 1 4)
  (func (export "poke") (param i32 i32)
    local.get 0
    local.get 1
    i32.store)
  (func (export "peek") (param i32) (result i32)
    local.get 0
    i32.load)
  (func (export "size") (result i32) memory.size))

;; The hijack really was installed: the property read the old code performed
;; answered with the decoy while the module was being instantiated. Without
;; this line the three that follow could all pass because nothing was
;; replaced.
;; CHECK: hijacked buffer accessor was in force: true

;; The module writes into its own memory, not into the storage the accessor
;; offered.
;; CHECK-NEXT: wasm wrote into the script-supplied decoy: false
;; CHECK-NEXT: wasm wrote into the exported memory: true

;; And the cross-module consequence. The exported object is a genuine Memory,
;; so the consumer's brand check passes; what it must then read is the
;; exporter's data.
;; CHECK-NEXT: exported memory is a genuine Memory: true
;; CHECK-NEXT: consumer links against the exported Memory: true
;; CHECK-NEXT: exporter wrote 43981 at 512; consumer reads: 43981

;; A replaced WebAssembly.Memory constructor no longer decides anything: the
;; constructor comes from HermesInternal.intrinsics.WebAssembly, so the module
;; builds its memory from the pristine one and links normally. The second line
;; proves the replacement would have been used by anything that read it.
;; CHECK-NEXT: replaced Memory constructor: linked, size = 1
;; CHECK-NEXT: replaced Memory constructor was live: true

;; The level below that is NOT closed by a pristine constructor, and this is
;; what the exact-limits check is for. The descriptor generated code hands the
;; constructor is an ordinary object literal whose stores walk the prototype
;; chain, so an `initial`/`maximum` accessor on Object.prototype swallows what
;; the module wrote and answers the constructor's read with a number of its
;; own. What comes back is a GENUINE WebAssembly.Memory carrying limits nobody
;; declared, while memory.grow on a defined memory keeps using the
;; compile-time literal -- so a smaller maximum gets grown past its own
;; ceiling. Measured before the limits check existed, on this same module:
;;
;;   substituted maximum is 2; module grow(3) -> 1
;;   buffer now 4 pages
;;   mem.grow(0) at 4 pages -> RangeError: would exceed maximum
;;
;; Never memory-unsafe -- accesses are bounds-checked against the real buffer
;; -- but the module ran on limits nobody agreed to and left the exported
;; object internally inconsistent. Each of the three rows below differs from
;; the declaration in a different place, so a check that compared only one of
;; the two numbers would let one of them through.
;; CHECK-NEXT: descriptor said 1 page, no maximum: LinkError: WebAssembly.Memory did not construct a memory with this module's declared limits for memory 0
;; CHECK-NEXT: descriptor said 1 page, maximum 2: LinkError: WebAssembly.Memory did not construct a memory with this module's declared limits for memory 0
;; CHECK-NEXT: descriptor said 2 pages, maximum 4: LinkError: WebAssembly.Memory did not construct a memory with this module's declared limits for memory 0

;; And the control: a descriptor matching the declaration exactly still links,
;; so "everything is a LinkError" is not the reason the three above are.
;; CHECK-NEXT: descriptor said exactly what was declared: linked, size = 1
;; CHECK-NEXT: done
