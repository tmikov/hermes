(module $life.wasm
  (type (;0;) (func (param i32)))
  (type (;1;) (func))
  (import "env" "log" (func $env_log (type 0)))
  (func $__wasm_call_ctors (type 1))
  (func $run (type 0) (param i32)
    (local i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32)
    i32.const 1024
    local.set 1
    block  ;; label = @1
      i32.const 16384
      i32.eqz
      local.tee 2
      br_if 0 (;@1;)
      i32.const 1024
      i32.const 0
      i32.const 16384
      memory.fill
    end
    block  ;; label = @1
      local.get 2
      br_if 0 (;@1;)
      i32.const 17408
      i32.const 0
      i32.const 16384
      memory.fill
    end
    i32.const 0
    i32.const 1
    i32.store8 offset=9408
    i32.const 0
    i32.const 257
    i32.store16 offset=9279 align=1
    i32.const 0
    i32.const 257
    i32.store16 offset=9152
    block  ;; label = @1
      local.get 0
      i32.const 1
      i32.lt_s
      br_if 0 (;@1;)
      i32.const 17408
      local.set 3
      i32.const 0
      local.set 4
      i32.const 1024
      local.set 2
      loop  ;; label = @2
        local.get 3
        local.set 1
        i32.const 1
        local.set 5
        local.get 2
        local.tee 3
        i32.const 1
        i32.add
        local.set 6
        local.get 1
        i32.const 1
        i32.add
        local.set 7
        i32.const 127
        local.set 8
        local.get 3
        local.set 9
        i32.const 0
        local.set 10
        loop  ;; label = @3
          local.get 1
          local.get 10
          i32.const 7
          i32.shl
          local.tee 2
          i32.add
          local.get 3
          local.get 2
          i32.const 16256
          i32.add
          i32.const 16256
          i32.and
          i32.add
          local.tee 11
          i32.load8_u
          local.tee 12
          local.get 11
          i32.const 127
          i32.add
          i32.load8_u
          i32.add
          local.get 11
          i32.const 1
          i32.add
          i32.load8_u
          i32.add
          local.get 3
          local.get 2
          i32.add
          local.tee 13
          i32.const 127
          i32.add
          i32.load8_u
          i32.add
          local.get 13
          i32.const 1
          i32.add
          i32.load8_u
          i32.add
          local.get 3
          local.get 2
          i32.const 128
          i32.add
          i32.const 16256
          i32.and
          i32.add
          local.tee 14
          i32.const 127
          i32.add
          i32.load8_u
          i32.add
          local.get 14
          i32.load8_u
          local.tee 15
          i32.add
          local.get 14
          i32.const 1
          i32.add
          i32.load8_u
          i32.add
          local.tee 2
          i32.const -2
          i32.and
          i32.const 2
          i32.eq
          local.get 2
          i32.const 3
          i32.eq
          local.get 13
          i32.load8_u
          select
          i32.store8
          local.get 6
          local.get 8
          i32.const 127
          i32.and
          i32.const 7
          i32.shl
          i32.add
          local.set 16
          local.get 6
          local.get 5
          i32.const 127
          i32.and
          i32.const 7
          i32.shl
          i32.add
          local.set 17
          i32.const -127
          local.set 2
          loop  ;; label = @4
            local.get 7
            local.get 2
            i32.add
            i32.const 127
            i32.add
            local.get 12
            i32.const 255
            i32.and
            local.get 16
            local.get 2
            i32.add
            i32.const 127
            i32.add
            i32.load8_u
            local.tee 12
            i32.add
            local.get 11
            local.get 2
            i32.const 129
            i32.add
            i32.const 127
            i32.and
            local.tee 18
            i32.add
            i32.load8_u
            i32.add
            local.get 9
            local.get 2
            i32.add
            local.tee 19
            i32.const 127
            i32.add
            i32.load8_u
            i32.add
            local.get 13
            local.get 18
            i32.add
            i32.load8_u
            i32.add
            local.get 15
            i32.const 255
            i32.and
            i32.add
            local.get 17
            local.get 2
            i32.add
            i32.const 127
            i32.add
            i32.load8_u
            local.tee 15
            i32.add
            local.get 14
            local.get 18
            i32.add
            i32.load8_u
            i32.add
            local.tee 18
            i32.const -2
            i32.and
            i32.const 2
            i32.eq
            local.get 18
            i32.const 3
            i32.eq
            local.get 19
            i32.const 128
            i32.add
            i32.load8_u
            select
            i32.store8
            local.get 2
            i32.const 1
            i32.add
            local.tee 2
            br_if 0 (;@4;)
          end
          local.get 8
          i32.const 1
          i32.add
          local.set 8
          local.get 5
          i32.const 1
          i32.add
          local.set 5
          local.get 7
          i32.const 128
          i32.add
          local.set 7
          local.get 9
          i32.const 128
          i32.add
          local.set 9
          local.get 10
          i32.const 1
          i32.add
          local.tee 10
          i32.const 128
          i32.ne
          br_if 0 (;@3;)
        end
        local.get 1
        local.set 2
        local.get 4
        i32.const 1
        i32.add
        local.tee 4
        local.get 0
        i32.ne
        br_if 0 (;@2;)
      end
    end
    i32.const 0
    local.set 18
    i32.const 0
    local.set 12
    loop  ;; label = @1
      local.get 12
      local.get 1
      local.get 18
      i32.add
      local.tee 2
      i32.load8_u
      i32.add
      local.get 2
      i32.const 1
      i32.add
      i32.load8_u
      i32.add
      local.get 2
      i32.const 2
      i32.add
      i32.load8_u
      i32.add
      local.get 2
      i32.const 3
      i32.add
      i32.load8_u
      i32.add
      local.set 12
      local.get 18
      i32.const 4
      i32.add
      local.tee 18
      i32.const 16384
      i32.ne
      br_if 0 (;@1;)
    end
    local.get 12
    call $env_log)
  (func $main_entry (type 1)
    i32.const 200
    call $run)
  (table (;0;) 1 1 funcref)
  (memory (;0;) 2)
  (global $__stack_pointer (mut i32) (i32.const 99328))
  (global (;1;) i32 (i32.const 1024))
  (global (;2;) i32 (i32.const 33792))
  (global (;3;) i32 (i32.const 33792))
  (global (;4;) i32 (i32.const 99328))
  (global (;5;) i32 (i32.const 1024))
  (global (;6;) i32 (i32.const 99328))
  (global (;7;) i32 (i32.const 131072))
  (global (;8;) i32 (i32.const 0))
  (global (;9;) i32 (i32.const 1))
  (global (;10;) i32 (i32.const 65536))
  (export "memory" (memory 0))
  (export "__wasm_call_ctors" (func $__wasm_call_ctors))
  (export "run" (func $run))
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
