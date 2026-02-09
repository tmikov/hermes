;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir -O0 %t.wasm | %FileCheck %s

;; Test f32 comparison operations (E.3).
;; Same IR pattern as f64/i32 comparisons — boolean result converted to i32.

(module
  ;; f32.eq
  (func $f32_eq (param f32 f32) (result i32)
    local.get 0
    local.get 1
    f32.eq
  )

  ;; f32.ne
  (func $f32_ne (param f32 f32) (result i32)
    local.get 0
    local.get 1
    f32.ne
  )

  ;; f32.lt
  (func $f32_lt (param f32 f32) (result i32)
    local.get 0
    local.get 1
    f32.lt
  )

  ;; f32.gt
  (func $f32_gt (param f32 f32) (result i32)
    local.get 0
    local.get 1
    f32.gt
  )

  ;; f32.le
  (func $f32_le (param f32 f32) (result i32)
    local.get 0
    local.get 1
    f32.le
  )

  ;; f32.ge
  (func $f32_ge (param f32 f32) (result i32)
    local.get 0
    local.get 1
    f32.ge
  )
)

;; f32.eq: BinaryStrictlyEqualInst + BinaryOrInst(cmp, 0)
;; CHECK-LABEL: function wasm_func_0(p0: any, p1: any): any
;; CHECK: BinaryStrictlyEqualInst (:any)
;; CHECK-NEXT: BinaryOrInst (:any)
;; CHECK: ReturnInst

;; f32.ne: BinaryStrictlyNotEqualInst + BinaryOrInst(cmp, 0)
;; CHECK-LABEL: function wasm_func_1(p0: any, p1: any): any
;; CHECK: BinaryStrictlyNotEqualInst (:any)
;; CHECK-NEXT: BinaryOrInst (:any)
;; CHECK: ReturnInst

;; f32.lt: BinaryLessThanInst + BinaryOrInst(cmp, 0)
;; CHECK-LABEL: function wasm_func_2(p0: any, p1: any): any
;; CHECK: BinaryLessThanInst (:any)
;; CHECK-NEXT: BinaryOrInst (:any)
;; CHECK: ReturnInst

;; f32.gt: BinaryGreaterThanInst + BinaryOrInst(cmp, 0)
;; CHECK-LABEL: function wasm_func_3(p0: any, p1: any): any
;; CHECK: BinaryGreaterThanInst (:any)
;; CHECK-NEXT: BinaryOrInst (:any)
;; CHECK: ReturnInst

;; f32.le: BinaryLessThanOrEqualInst + BinaryOrInst(cmp, 0)
;; CHECK-LABEL: function wasm_func_4(p0: any, p1: any): any
;; CHECK: BinaryLessThanOrEqualInst (:any)
;; CHECK-NEXT: BinaryOrInst (:any)
;; CHECK: ReturnInst

;; f32.ge: BinaryGreaterThanOrEqualInst + BinaryOrInst(cmp, 0)
;; CHECK-LABEL: function wasm_func_5(p0: any, p1: any): any
;; CHECK: BinaryGreaterThanOrEqualInst (:any)
;; CHECK-NEXT: BinaryOrInst (:any)
;; CHECK: ReturnInst
