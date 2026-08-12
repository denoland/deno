(module
  (import "./dep.wasm" "counter" (global $counter (mut i32)))
  (func (export "read") (result i32)
    (global.get $counter))
)
