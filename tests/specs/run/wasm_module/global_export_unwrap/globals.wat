(module
  (global (export "i32val") i32 (i32.const 42))
  (global (export "i64val") i64 (i64.const 9000000000))
  (global (export "f32val") f32 (f32.const 1.5))
  (global (export "f64val") f64 (f64.const 3.5))
  (global (export "mutcount") (mut i32) (i32.const 7))
)
