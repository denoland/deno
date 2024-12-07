(module
  (func (export "func") unreachable)
  (table (export "table") 0 funcref)
  (memory (export "memory") 0)
  (global (export "global") i32 i32.const 0)
)
