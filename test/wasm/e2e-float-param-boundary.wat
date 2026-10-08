;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; The JS -> Wasm float parameter boundary, which is the export wrapper and
;; nothing else.
;;
;; A Wasm function's internal closure declares its f32/f64 parameters
;; `:number`, and the float backend trusts that: FBinaryMathInst reads the raw
;; double bits. The conversion that makes it true is ToWebAssemblyValue, which
;; is ToNumber for f64 and ToNumber-then-round-to-f32 for f32, and it belongs
;; at the boundary the value crosses.
;;
;; This file used to be `e2e-escapable-float-param.wat` and tested something
;; else: an interim fix for finding J4 that typed float params of "escapable"
;; functions `:any` and coerced them at the internal function's ENTRY, because
;; an element segment put the closure itself into a table and
;; WebAssembly.Table.prototype.get handed it to script. The table now hands out
;; the canonical Exported Function, so that route -- and every other one, see
;; e2e-no-closure-escape.wat -- yields the wrapper, the annotation is honest
;; again and the entry coercion is gone.
;;
;; Two consequences are pinned here, and the second is a bug the interim was
;; masking:
;;
;;   * The rounding follows the value, not the function. It used to happen only
;;     for functions whose closure could escape, so every OTHER exported
;;     f32-parameter function -- the common case -- silently skipped it, and
;;     `id_f32(1.1)` answered 1.1 instead of 1.100000023841858. Now the wrapper
;;     rounds and both kinds agree.
;;   * The internal closure's parameters are `:number` again for every
;;     function, escapable or not, so an escapable float function pays nothing
;;     on the Wasm-to-Wasm path.
;;
;; The spec suite cannot see the f32 half at all: every f32 literal it passes
;; is already exactly representable, so the rounding is a no-op on all of them.
;; 1.1 is used here for exactly that reason.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm 2>/dev/null && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-float-param-boundary-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck --check-prefix=IR %s

(module
  (table (export "tbl") 2 funcref)

  ;; Escapable: named by an element segment, so a funcref for it exists.
  (func $id32 (param f32) (result f32) (local.get 0))
  (func $addf64 (param f64 f64) (result f64)
    (f64.add (local.get 0) (local.get 1)))
  (elem (i32.const 0) $id32 $addf64)

  ;; NOT escapable, and exported: the case the entry coercion never covered.
  (func (export "id_f32") (param f32) (result f32) (local.get 0))
  (func (export "id_f64") (param f64) (result f64) (local.get 0))

  ;; A direct call, so the Wasm-to-Wasm path into an escapable function is
  ;; exercised too. It must still add correctly and must not coerce.
  (func (export "add_direct") (param f64 f64) (result f64)
    (call $addf64 (local.get 0) (local.get 1))))

;; An f32 parameter is rounded to single precision, whether the function is
;; escapable or not. The first line is what the old arrangement got wrong.
;; CHECK: id_f32(1.1) = 1.100000023841858
;; CHECK-NEXT: tbl.get(0)(1.1) = 1.100000023841858
;; CHECK-NEXT: id_f32("1.1") = 1.100000023841858

;; ToNumber, so a non-number is NaN by ordinary JS rules rather than a raw
;; double read. This is the J4 crash repro; it is unreachable through a raw
;; closure because there is no route to one, not because a coercion survives.
;; CHECK-NEXT: id_f32("x") = NaN
;; CHECK-NEXT: id_f64("x") = NaN
;; CHECK-NEXT: tbl.get(0)("x") = NaN
;; CHECK-NEXT: tbl.get(1)("x", "y") = NaN

;; An f64 parameter is NOT rounded -- it is already a double.
;; CHECK-NEXT: id_f64(1.1) = 1.1

;; The Wasm-to-Wasm path is unchanged.
;; CHECK-NEXT: add_direct(10.5, 0.25) = 10.75
;; CHECK-NEXT: done

;; --- Where the conversion lives in the IR, and where it does not ---

;; $id32 is ESCAPABLE -- an element segment names it -- and its parameter is
;; `p0: number` again. That declaration is the whole subject of J4: the float
;; backend trusts it, the interim had to give it up, and it is honest only
;; because nothing hands this closure to script.
;;
;; The body is pinned line by line rather than by a CHECK-NOT, so a
;; re-introduced entry coercion fails on the sequence rather than needing to be
;; anticipated by name. Restoring the interim turns the signature into
;; `(p0: any)` AND inserts AsNumberInst + Math.fround before the StoreStackInst;
;; both halves break these lines.
;; IR-LABEL: function wasm_func_0(p0: number): number
;; IR-NEXT: %BB0:
;; IR-NEXT: %0 = GetParentScopeInst (:environment) %VS0: any, %parentScope: environment
;; IR-NEXT: %1 = LoadFrameInst (:any) %0: environment, [%VS0.retBufI]: any
;; IR-NEXT: %2 = AllocStackInst (:number) $local_0: any
;; IR-NEXT: %3 = LoadParamInst (:number) %p0: number
;; IR-NEXT: StoreStackInst %3: number, %2: number

;; The escapable f64 adder, same story with two parameters.
;; IR-LABEL: function wasm_func_1(p0: number, p1: number): number

;; The f32 wrapper for the escapable $id32: ToNumber, then Math.fround, then
;; the internal call. `wasm_funcref_N` is the name a wrapper gets when its
;; function has no export name.
;; IR-LABEL: function wasm_funcref_0(p0: any): any
;; IR: %[[F32ARG:[0-9]+]] = AsNumberInst (:number) %{{[0-9]+}}: any
;; IR-NEXT: %[[F32R:[0-9]+]] = CallBuiltinInst (:number) [Math.fround]: number, {{.*}} %[[F32ARG]]: number
;; IR-NEXT: %{{[0-9]+}} = CallInst (:any) %{{[0-9]+}}: any, %wasm_func_0(): functionCode, {{.*}} %[[F32R]]: number

;; The f64 wrapper: ToNumber for each parameter and no rounding -- an f64 is
;; already a double. Pinned adjacently so a stray fround here is a failure.
;; IR-LABEL: function wasm_funcref_1(p0: any, p1: any): any
;; IR: %{{[0-9]+}} = AsNumberInst (:number) %{{[0-9]+}}: any
;; IR-NEXT: %{{[0-9]+}} = LoadParamInst (:any) %p1: any
;; IR-NEXT: %{{[0-9]+}} = AsNumberInst (:number) %{{[0-9]+}}: any
;; IR-NEXT: %{{[0-9]+}} = CallInst (:any)

;; The exported, NON-escapable f32 function gets the identical treatment. This
;; is the pair of lines that was missing before: this wrapper had ToNumber and
;; no rounding, and nothing else rounded either.
;; IR-LABEL: function wasm_export_id_f32(p0: any): any
;; IR: %[[EARG:[0-9]+]] = AsNumberInst (:number) %{{[0-9]+}}: any
;; IR-NEXT: %{{[0-9]+}} = CallBuiltinInst (:number) [Math.fround]: number, {{.*}} %[[EARG]]: number

;; And the exported f64 function is NOT rounded.
;; IR-LABEL: function wasm_export_id_f64(p0: any): any
;; IR: %{{[0-9]+}} = AsNumberInst (:number) %{{[0-9]+}}: any
;; IR-NEXT: %{{[0-9]+}} = CallInst (:any)
