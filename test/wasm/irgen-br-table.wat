;; RUN: %wat2wasm %s -o %t.wasm && %hermesc --wasm --dump-ir %t.wasm | %FileCheck %s
;; REQUIRES: wasm

;; Test br_table with 4 cases returning different values.
;; br_table dispatches index 0,1,2,3 to four blocks, default returns -1.
(module
  ;; switch_case: returns 10, 20, 30, or 40 based on index, or -1 for default
  (func (export "switch_case") (param i32) (result i32)
    (block $b0 (result i32)
      (block $b1 (result i32)
        (block $b2 (result i32)
          (block $b3 (result i32)
            (block $b4 (result i32)
              (i32.const -1)  ;; default value
              (local.get 0)
              (br_table $b4 $b3 $b2 $b1 $b0)  ;; 0->$b4, 1->$b3, 2->$b2, 3->$b1, default->$b0
            )
            ;; case 0
            (drop)
            (i32.const 10)
            (br $b0)
          )
          ;; case 1
          (drop)
          (i32.const 20)
          (br $b0)
        )
        ;; case 2
        (drop)
        (i32.const 30)
        (br $b0)
      )
      ;; case 3
      (drop)
      (i32.const 40)
    )
  )

  ;; simple_switch: br_table all targets go to same block
  (func (export "simple_switch") (param i32) (result i32)
    (block $out (result i32)
      (i32.const 42)
      (local.get 0)
      (br_table $out $out $out)  ;; all cases go to $out
    )
  )

  ;; loop_switch: br_table targeting a loop header
  (func (export "loop_switch") (param i32) (result i32)
    (local i32)
    (i32.const 0)
    (local.set 1)
    (block $break
      (loop $loop
        (local.get 0)
        (br_table $loop $break $break)  ;; 0 -> continue loop, 1,default -> break
      )
    )
    (local.get 1)
  )
)

;; CHECK-LABEL: function wasm_func_0(p0: any): any
;; CHECK: SwitchInst
;; CHECK: BranchInst

;; CHECK-LABEL: function wasm_func_1(p0: any): any
;; CHECK: SwitchInst

;; CHECK-LABEL: function wasm_func_2(p0: any): any
;; CHECK: SwitchInst
