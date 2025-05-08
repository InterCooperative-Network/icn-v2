(module
  (import "env" "log" (func $log (param i32 i32)))
  (memory (export "memory") 1)
  (data (i32.const 0) "Hello from ICN Mesh!")
  (func (export "_start")
    i32.const 0  ;; Pointer to message
    i32.const 19 ;; Length of message
    call $log
  )
) 