;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; IR generation test for memory load and store operations.

;; REQUIRES: wasm

;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s --implicit-check-not=buffer

(module
  (memory 1)

  ;; i32 store and load at offset 0 (func 0).
  (func (export "store_load_i32") (param i32) (result i32)
    i32.const 0
    local.get 0
    i32.store offset=0
    i32.const 0
    i32.load offset=0
  )

  ;; f64 store and load (func 1).
  (func (export "store_load_f64") (param f64) (result f64)
    i32.const 8
    local.get 0
    f64.store offset=0
    i32.const 8
    f64.load offset=0
  )

  ;; i32.load with a non-zero offset (func 2).
  (func (export "load_offset") (param i32) (result i32)
    local.get 0
    i32.load offset=4
  )
)

;; Check that the top-level function builds the module info object.
;; CHECK-LABEL: function global(): object
;; CHECK:   CreateFunctionInst {{.*}}__wasm_instantiate__
;; CHECK:   ReturnInst

;; Check the i32 store/load function (wasm_func_0).
;; CHECK-LABEL: function wasm_func_0(p0: number): number 
;; CHECK: LoadFrameInst {{.*}}[%VS0.HEAP32]
;; CHECK: StorePropertyStrictInst
;; CHECK: LoadFrameInst {{.*}}[%VS0.HEAP32]
;; CHECK: LoadPropertyInst
;; CHECK: BinaryStrictlyEqualInst
;; CHECK: CondBranchInst

;; Check the f64 store/load function (wasm_func_1).
;; CHECK-LABEL: function wasm_func_1(p0: number): number 
;; CHECK: LoadFrameInst {{.*}}[%VS0.HEAPF64]
;; CHECK: StorePropertyStrictInst
;; CHECK: LoadFrameInst {{.*}}[%VS0.HEAPF64]
;; CHECK: LoadPropertyInst

;; Check the load with offset function (wasm_func_2).
;; CHECK-LABEL: function wasm_func_2(p0: number): number 
;; CHECK: BinaryAddInst
;; CHECK: BinaryUnsignedRightShiftInst
;; CHECK: LoadFrameInst {{.*}}[%VS0.HEAP32]
;; CHECK: LoadPropertyInst

;; Check that the instantiate function backs a defined memory with a real
;; WebAssembly.Memory and builds the typed array views over *its* buffer --
;; not over a bare ArrayBuffer, which would leave an exported memory pointing
;; at storage the module never writes to.
;;
;; The buffer is taken out of the memory's internal field by wasmLinkMemory,
;; which is also the brand check on what the (replaceable) constructor
;; returned. It used to be a `.buffer` property read, and that accessor is a
;; configurable property of WebAssembly.Memory.prototype: replacing it
;; substituted the module's whole linear memory while the Memory it exported
;; -- which an importer brand-checks and trusts -- still held its own,
;; untouched buffer. `--implicit-check-not=buffer` on the RUN line is the real
;; assertion here: a regression that re-added the property read ALONGSIDE the
;; builtin would satisfy every positive check below.
;; CHECK-LABEL: function __wasm_instantiate__(imports: any): object
;; CHECK: TryLoadGlobalPropertyInst {{.*}}"WebAssembly"
;; CHECK: LoadPropertyInst {{.*}}"Memory"
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmLinkMemory]
;; CHECK: BinaryStrictlyEqualInst {{.*}}null: null
;; CHECK: CondBranchInst
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmLinkError]{{.*}}"WebAssembly.Memory did not construct a memory for this module's memory 0": string

;; The brand is not the whole check. A replaced constructor can return a
;; GENUINE Memory carrying limits of its own, and a defined memory's declared
;; limits are what the module ASKED FOR, not what came back -- memory.grow
;; then uses the compile-time literal and can grow the substitute past its own
;; maximum. Both numbers are compared, by exact equality: this module declares
;; (memory 1), so one page and the -1 that means "no maximum".
;; CHECK: [[PAGES:%[0-9]+]] = LoadPropertyInst {{.*}}0: number
;; CHECK-NEXT: [[MAX:%[0-9]+]] = LoadPropertyInst {{.*}}1: number
;; CHECK-NEXT: BinaryStrictlyEqualInst (:any) [[PAGES]]: any, 1: number
;; CHECK: CallBuiltinInst {{.*}}[HermesBuiltin.wasmLinkError]{{.*}}"WebAssembly.Memory did not construct a memory with this module's declared limits for memory 0": string
;; CHECK: BinaryStrictlyEqualInst (:any) [[MAX]]: any, -1: number

;; Only then is the buffer taken, and the views built over it.
;; CHECK: LoadPropertyInst {{.*}}2: number
;; CHECK: TryLoadGlobalPropertyInst {{.*}}"Int8Array"
;; CHECK: TryLoadGlobalPropertyInst {{.*}}"Int32Array"
