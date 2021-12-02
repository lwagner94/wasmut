(module
  (type (;0;) (func (param i32)))
  (type (;1;) (func))
  (type (;2;) (func (param i32 i32) (result i32)))
  (type (;3;) (func (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $__wasi_proc_exit (type 0)))
  (func $_start (type 1)
    (local i32)
    block  ;; label = @1
      call $__original_main
      local.tee 0
      i32.eqz
      br_if 0 (;@1;)
      local.get 0
      call $exit
      unreachable
    end)
  (func $add (type 2) (param i32 i32) (result i32)
    local.get 1
    local.get 0
    i32.add)
  (func $test_add_1 (type 3) (result i32)
    i32.const 1
    i32.const 2
    call $add
    i32.const 3
    i32.eq)
  (func $test_add_2 (type 3) (result i32)
    i32.const 2
    i32.const 2
    call $add
    i32.const 4
    i32.eq)
  (func $__original_main (type 3) (result i32)
    (local i32)
    i32.const 1
    local.set 0
    block  ;; label = @1
      i32.const 1
      i32.const 2
      call $add
      i32.const 3
      i32.ne
      br_if 0 (;@1;)
      i32.const 2
      i32.const 2
      call $add
      i32.const 4
      i32.ne
      local.set 0
    end
    local.get 0)
  (func $_Exit (type 0) (param i32)
    local.get 0
    call $__wasi_proc_exit
    unreachable)
  (func $dummy (type 1))
  (func $__wasm_call_dtors (type 1)
    call $dummy
    call $dummy)
  (func $exit (type 0) (param i32)
    call $dummy
    call $dummy
    local.get 0
    call $_Exit
    unreachable)
  (func $_start.command_export (type 1)
    call $_start
    call $__wasm_call_dtors)
  (func $test_add_1.command_export (type 3) (result i32)
    call $test_add_1
    call $__wasm_call_dtors)
  (func $test_add_2.command_export (type 3) (result i32)
    call $test_add_2
    call $__wasm_call_dtors)
  (table (;0;) 1 1 funcref)
  (memory (;0;) 2)
  (global (;0;) (mut i32) (i32.const 66560))
  (export "memory" (memory 0))
  (export "_start" (func $_start.command_export))
  (export "test_add_1" (func $test_add_1.command_export))
  (export "test_add_2" (func $test_add_2.command_export)))
