;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

;; Test f32 arithmetic operations (E.2).
;; In Phase 1, f32 ops produce the same IR as f64 ops (no per-op rounding).

(module
  ;; f32.add
  (func $f32_add (param f32 f32) (result f32)
    local.get 0
    local.get 1
    f32.add
  )

  ;; f32.sub
  (func $f32_sub (param f32 f32) (result f32)
    local.get 0
    local.get 1
    f32.sub
  )

  ;; f32.mul
  (func $f32_mul (param f32 f32) (result f32)
    local.get 0
    local.get 1
    f32.mul
  )

  ;; f32.div
  (func $f32_div (param f32 f32) (result f32)
    local.get 0
    local.get 1
    f32.div
  )

  ;; f32.neg
  (func $f32_neg (param f32) (result f32)
    local.get 0
    f32.neg
  )

  ;; f32.abs
  (func $f32_abs (param f32) (result f32)
    local.get 0
    f32.abs
  )

  ;; f32.sqrt
  (func $f32_sqrt (param f32) (result f32)
    local.get 0
    f32.sqrt
  )

  ;; f32.ceil
  (func $f32_ceil (param f32) (result f32)
    local.get 0
    f32.ceil
  )

  ;; f32.floor
  (func $f32_floor (param f32) (result f32)
    local.get 0
    f32.floor
  )

  ;; f32.trunc
  (func $f32_trunc (param f32) (result f32)
    local.get 0
    f32.trunc
  )

  ;; f32.nearest
  (func $f32_nearest (param f32) (result f32)
    local.get 0
    f32.nearest
  )

  ;; f32.min
  (func $f32_min (param f32 f32) (result f32)
    local.get 0
    local.get 1
    f32.min
  )

  ;; f32.max
  (func $f32_max (param f32 f32) (result f32)
    local.get 0
    local.get 1
    f32.max
  )

  ;; f32.demote_f64
  (func $f32_demote (param f64) (result f32)
    local.get 0
    f32.demote_f64
  )

  ;; f64.promote_f32
  (func $f64_promote (param f32) (result f64)
    local.get 0
    f64.promote_f32
  )
)

;; CHECK-LABEL: function wasm_func_0(p0: any, p1: any): any
;; CHECK: BinaryAddInst (:any)
;; CHECK: ReturnInst

;; CHECK-LABEL: function wasm_func_1(p0: any, p1: any): any
;; CHECK: BinarySubtractInst (:any)
;; CHECK: ReturnInst

;; CHECK-LABEL: function wasm_func_2(p0: any, p1: any): any
;; CHECK: BinaryMultiplyInst (:any)
;; CHECK: ReturnInst

;; CHECK-LABEL: function wasm_func_3(p0: any, p1: any): any
;; CHECK: BinaryDivideInst (:any)
;; CHECK: ReturnInst

;; CHECK-LABEL: function wasm_func_4(p0: any): any
;; CHECK: UnaryMinusInst (:any)
;; CHECK: ReturnInst

;; CHECK-LABEL: function wasm_func_5(p0: any): any
;; CHECK: CallBuiltinInst {{.*}}[Math.abs]
;; CHECK: ReturnInst

;; CHECK-LABEL: function wasm_func_6(p0: any): any
;; CHECK: CallBuiltinInst {{.*}}[Math.sqrt]
;; CHECK: ReturnInst

;; CHECK-LABEL: function wasm_func_7(p0: any): any
;; CHECK: CallBuiltinInst {{.*}}[Math.ceil]
;; CHECK: ReturnInst

;; CHECK-LABEL: function wasm_func_8(p0: any): any
;; CHECK: CallBuiltinInst {{.*}}[Math.floor]
;; CHECK: ReturnInst

;; CHECK-LABEL: function wasm_func_9(p0: any): any
;; CHECK: CallBuiltinInst {{.*}}[Math.trunc]
;; CHECK: ReturnInst

;; CHECK-LABEL: function wasm_func_10(p0: any): any
;; CHECK: CallBuiltinInst {{.*}}[Math.round]
;; CHECK: ReturnInst

;; CHECK-LABEL: function wasm_func_11(p0: any, p1: any): any
;; CHECK: CallBuiltinInst {{.*}}[Math.min]
;; CHECK: ReturnInst

;; CHECK-LABEL: function wasm_func_12(p0: any, p1: any): any
;; CHECK: CallBuiltinInst {{.*}}[Math.max]
;; CHECK: ReturnInst

;; f32.demote_f64 is a no-op in Phase 1 (no rounding).
;; CHECK-LABEL: function wasm_func_13(p0: any): any
;; CHECK-NOT: CallBuiltinInst
;; CHECK: ReturnInst

;; f64.promote_f32 is a no-op in Phase 1.
;; CHECK-LABEL: function wasm_func_14(p0: any): any
;; CHECK-NOT: CallBuiltinInst
;; CHECK: ReturnInst
