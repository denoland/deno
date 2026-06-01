;; tiny.wasm is produced from this file via: wat2wasm tiny.wat
(module
  (import "some_import" "dummy" (func $dummy))
  (func (export "run")
    call $dummy
  )
)
