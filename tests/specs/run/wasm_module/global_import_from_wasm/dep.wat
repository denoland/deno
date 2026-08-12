(module
  (global $counter (export "counter") (mut i32) (i32.const 1))
  (func (export "bump")
    (global.set $counter
      (i32.add (global.get $counter) (i32.const 1))))
)
