(module
  (func $add (import "./import_from_wasm.mjs" "add") (param i32) (param i32) (result i32))
  (func (export "exported_add") (result i32)
    i32.const 21
    i32.const 21
    call $add
  )
)