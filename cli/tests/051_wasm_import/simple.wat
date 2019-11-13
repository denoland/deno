;; From https://github.com/nodejs/node/blob/bbc254db5db672643aad89a436a4938412a5704e/test/fixtures/es-modules/simple.wat
;; MIT Licensed
;; $ wat2wasm simple.wat -o simple.wasm

(module
  (import "./wasm-dep.js" "jsFn" (func $jsFn (result i32)))
  (import "./wasm-dep.js" "jsInitFn" (func $jsInitFn))
  (import "http://127.0.0.1:4545/cli/tests/051_wasm_import/remote.ts" "jsRemoteFn" (func $jsRemoteFn (result i32)))
  (export "add" (func $add))
  (export "addImported" (func $addImported))
  (export "addRemote" (func $addRemote))
  (start $startFn)
  (func $startFn
    call $jsInitFn
  )
  (func $add (param $a i32) (param $b i32) (result i32)
    local.get $a
    local.get $b
    i32.add
  )
  (func $addImported (param $a i32) (result i32)
    local.get $a
    call $jsFn
    i32.add
  )
  (func $addRemote (param $a i32) (result i32)
    local.get $a
    call $jsRemoteFn
    i32.add
  )
)
