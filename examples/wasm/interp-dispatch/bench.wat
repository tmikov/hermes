(module $bench.wasm
  (type (;0;) (func (param f64)))
  (type (;1;) (func))
  (type (;2;) (func (param i32 i32) (result f64)))
  (import "env" "print" (func $print (type 0)))
  (func $__wasm_call_ctors (type 1))
  (func $bench (type 2) (param i32 i32) (result f64)
    (local i32 f64 f64 i32 i32 i32 i32 f64 i32)
    block  ;; label = @1
      local.get 0
      i32.const 1
      i32.ge_s
      br_if 0 (;@1;)
      f64.const 0x0p+0 (;=0;)
      return
    end
    local.get 1
    i32.const 6
    i32.add
    i32.const 7
    i32.and
    local.set 2
    local.get 1
    f64.convert_i32_s
    local.set 3
    f64.const 0x0p+0 (;=0;)
    local.set 4
    local.get 1
    i32.const 3
    i32.lt_s
    local.set 5
    local.get 1
    i32.const 7
    i32.and
    i32.const 2
    i32.eq
    local.set 6
    local.get 1
    i32.const -3
    i32.add
    i32.const 7
    i32.lt_u
    local.set 7
    loop  ;; label = @1
      local.get 0
      local.set 8
      local.get 3
      local.set 9
      block  ;; label = @2
        local.get 5
        br_if 0 (;@2;)
        local.get 3
        local.set 9
        local.get 1
        local.set 0
        block  ;; label = @3
          local.get 6
          br_if 0 (;@3;)
          local.get 2
          local.set 10
          local.get 3
          local.set 9
          local.get 1
          local.set 0
          loop  ;; label = @4
            local.get 9
            local.get 0
            i32.const -1
            i32.add
            local.tee 0
            f64.convert_i32_u
            f64.mul
            local.set 9
            local.get 10
            i32.const -1
            i32.add
            local.tee 10
            br_if 0 (;@4;)
          end
        end
        local.get 7
        br_if 0 (;@2;)
        local.get 0
        i32.const -8
        i32.add
        local.set 0
        loop  ;; label = @3
          local.get 9
          local.get 0
          i32.const 7
          i32.add
          f64.convert_i32_u
          f64.mul
          local.get 0
          i32.const 6
          i32.add
          f64.convert_i32_u
          f64.mul
          local.get 0
          i32.const 5
          i32.add
          f64.convert_i32_u
          f64.mul
          local.get 0
          i32.const 4
          i32.add
          f64.convert_i32_u
          f64.mul
          local.get 0
          i32.const 3
          i32.add
          f64.convert_i32_u
          f64.mul
          local.get 0
          i32.const 2
          i32.add
          f64.convert_i32_u
          f64.mul
          local.get 0
          i32.const 1
          i32.add
          local.tee 10
          f64.convert_i32_u
          f64.mul
          local.get 0
          f64.convert_i32_u
          f64.mul
          local.set 9
          local.get 0
          i32.const -8
          i32.add
          local.set 0
          local.get 10
          i32.const 3
          i32.gt_u
          br_if 0 (;@3;)
        end
      end
      local.get 8
      i32.const -1
      i32.add
      local.set 0
      local.get 4
      local.get 9
      f64.add
      local.set 4
      local.get 8
      i32.const 1
      i32.gt_u
      br_if 0 (;@1;)
    end
    local.get 4)
  (func $main_entry (type 1)
    (local f64 i32 f64 i32 i32)
    f64.const 0x0p+0 (;=0;)
    local.set 0
    i32.const 4000
    local.set 1
    loop  ;; label = @1
      f64.const 0x1.9p+6 (;=100;)
      local.set 2
      i32.const 93
      local.set 3
      loop  ;; label = @2
        local.get 2
        local.get 3
        i32.const 6
        i32.add
        f64.convert_i32_u
        f64.mul
        local.get 3
        i32.const 5
        i32.add
        f64.convert_i32_u
        f64.mul
        local.get 3
        i32.const 4
        i32.add
        f64.convert_i32_u
        f64.mul
        local.get 3
        i32.const 3
        i32.add
        f64.convert_i32_u
        f64.mul
        local.get 3
        i32.const 2
        i32.add
        f64.convert_i32_u
        f64.mul
        local.get 3
        i32.const 1
        i32.add
        local.tee 4
        f64.convert_i32_u
        f64.mul
        local.get 3
        f64.convert_i32_u
        f64.mul
        local.set 2
        local.get 3
        i32.const -7
        i32.add
        local.set 3
        local.get 4
        i32.const 3
        i32.gt_u
        br_if 0 (;@2;)
      end
      local.get 0
      local.get 2
      f64.add
      local.set 0
      local.get 1
      i32.const 1
      i32.gt_u
      local.set 3
      local.get 1
      i32.const -1
      i32.add
      local.set 1
      local.get 3
      br_if 0 (;@1;)
    end
    local.get 0
    call $print)
  (table (;0;) 1 1 funcref)
  (memory (;0;) 2)
  (global $__stack_pointer (mut i32) (i32.const 66560))
  (global (;1;) i32 (i32.const 1024))
  (global (;2;) i32 (i32.const 1024))
  (global (;3;) i32 (i32.const 1024))
  (global (;4;) i32 (i32.const 66560))
  (global (;5;) i32 (i32.const 1024))
  (global (;6;) i32 (i32.const 66560))
  (global (;7;) i32 (i32.const 131072))
  (global (;8;) i32 (i32.const 0))
  (global (;9;) i32 (i32.const 1))
  (global (;10;) i32 (i32.const 65536))
  (export "memory" (memory 0))
  (export "__wasm_call_ctors" (func $__wasm_call_ctors))
  (export "bench" (func $bench))
  (export "main" (func $main_entry))
  (export "__indirect_function_table" (table 0))
  (export "__dso_handle" (global 1))
  (export "__data_end" (global 2))
  (export "__stack_low" (global 3))
  (export "__stack_high" (global 4))
  (export "__global_base" (global 5))
  (export "__heap_base" (global 6))
  (export "__heap_end" (global 7))
  (export "__memory_base" (global 8))
  (export "__table_base" (global 9))
  (export "__wasm_first_page_end" (global 10)))
