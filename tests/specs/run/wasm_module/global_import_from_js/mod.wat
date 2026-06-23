(module
  (import "./val.js" "answer" (global $answer i32))
  (func (export "read") (result i32)
    (global.get $answer))
)
