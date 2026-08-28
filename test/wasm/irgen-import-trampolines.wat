;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test IR generation for import trampoline functions (I.2).
;; Verifies argument marshaling and return value conversion for
;; various imported function signatures.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

(module
  ;; Import 1: void function with one i32 param.
  (import "env" "log" (func $log (param i32)))

  ;; Import 2: i32 function with two i32 params.
  (import "env" "add" (func $add (param i32 i32) (result i32)))

  ;; Import 3: void function with no params.
  (import "env" "init" (func $init))

  ;; Import 4: f64 function with two f64 params.
  (import "env" "f64_add" (func $f64_add (param f64 f64) (result f64)))

  ;; Import 5: i64 function with one i64 param (BigInt conversion).
  (import "env" "i64_id" (func $i64_id (param i64) (result i64)))

  ;; A defined function that calls all imports.
  (func (export "test")
    i32.const 42
    call $log

    i32.const 3
    i32.const 4
    call $add
    drop

    call $init

    f64.const 1.5
    f64.const 2.5
    call $f64_add
    drop

    i64.const 100
    call $i64_id
    drop
  )
)

;; Import trampoline 1: $log(i32) -> void.
;; Loads the imported JS function, passes the i32 param, returns undefined.
;; CHECK-LABEL: function wasm_func_0(p0: any): any
;; CHECK: %BB0:
;; CHECK:   %[[PARENT0:.*]] = GetParentScopeInst (:environment)
;; CHECK:   %[[FUNC0:.*]] = LoadFrameInst (:any) %[[PARENT0]]: environment, [%VS0.import_func_0]: any
;; CHECK:   %[[P0:.*]] = LoadParamInst (:any) %p0: any
;; CHECK:   CallInst (:any) %[[FUNC0]]: any, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %[[P0]]: any
;; CHECK:   ReturnInst undefined: undefined
;; CHECK-NEXT: function_end

;; Import trampoline 2: $add(i32, i32) -> i32.
;; Two params, AsInt32Inst on return value.
;; CHECK-LABEL: function wasm_func_1(p0: any, p1: any): any
;; CHECK: %BB0:
;; CHECK:   %[[PARENT1:.*]] = GetParentScopeInst (:environment)
;; CHECK:   %[[FUNC1:.*]] = LoadFrameInst (:any) %[[PARENT1]]: environment, [%VS0.import_func_1]: any
;; CHECK:   %[[PA:.*]] = LoadParamInst (:any) %p0: any
;; CHECK:   %[[PB:.*]] = LoadParamInst (:any) %p1: any
;; CHECK:   %[[CALL1:.*]] = CallInst (:any) %[[FUNC1]]: any, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %[[PA]]: any, %[[PB]]: any
;; CHECK:   %[[RES1:.*]] = AsInt32Inst (:number) %[[CALL1]]: any
;; CHECK:   ReturnInst %[[RES1]]: number
;; CHECK-NEXT: function_end

;; Import trampoline 3: $init() -> void.
;; No params, returns undefined.
;; CHECK-LABEL: function wasm_func_2(): any
;; CHECK: %BB0:
;; CHECK:   GetParentScopeInst (:environment)
;; CHECK:   %[[FUNC2:.*]] = LoadFrameInst (:any) %{{.*}}: environment, [%VS0.import_func_2]: any
;; CHECK:   CallInst (:any) %[[FUNC2]]: any, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined
;; CHECK:   ReturnInst undefined: undefined
;; CHECK-NEXT: function_end

;; Import trampoline 4: $f64_add(f64, f64) -> f64.
;; Float params pass through, result returned directly.
;; CHECK-LABEL: function wasm_func_3(p0: any, p1: any): any
;; CHECK: %BB0:
;; CHECK:   %[[FUNC3:.*]] = LoadFrameInst (:any) %{{.*}}: environment, [%VS0.import_func_3]: any
;; CHECK:   %[[FA:.*]] = LoadParamInst (:any) %p0: any
;; CHECK:   %[[FB:.*]] = LoadParamInst (:any) %p1: any
;; CHECK:   %[[CALL3:.*]] = CallInst (:any) %[[FUNC3]]: any, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %[[FA]]: any, %[[FB]]: any
;; CHECK:   ReturnInst %[[CALL3]]: any
;; CHECK-NEXT: function_end

;; Import trampoline 5: $i64_id(i64) -> i64.
;; i64 param splits into two JS params (lo, hi). Trampoline converts to BigInt.
;; i64 return: BigInt converted back to split (lo, hi).
;; CHECK-LABEL: function wasm_func_4(p0_lo: any, p0_hi: any): any
;; CHECK: %BB0:
;; CHECK:   %[[FUNC4:.*]] = LoadFrameInst (:any) %{{.*}}: environment, [%VS0.import_func_4]: any
;; CHECK:   %[[LO:.*]] = LoadParamInst (:any) %p0_lo: any
;; CHECK:   %[[HI:.*]] = LoadParamInst (:any) %p0_hi: any
;; CHECK:   %[[BIGINT:.*]] = CallBuiltinInst (:any) {{.*}}[HermesBuiltin.wasmI64ToBigInt]{{.*}}%[[LO]]{{.*}}%[[HI]]
;; CHECK:   %[[CALL4:.*]] = CallInst (:any) %[[FUNC4]]: any, empty: any, false: boolean, empty: any, undefined: undefined, undefined: undefined, %[[BIGINT]]: any
;; CHECK:   %[[RES_LO:.*]] = CallBuiltinInst (:any) {{.*}}[HermesBuiltin.wasmBigIntToI64]{{.*}}%[[CALL4]]
;; CHECK:   ReturnInst %[[RES_LO]]: any
;; CHECK-NEXT: function_end
