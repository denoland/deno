(module
  (import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (memory 1)
  (export "memory" (memory 0))
  ;; "hello world\n" at offset 8
  (data (i32.const 8) "hello world\n")
  (func $main (export "_start")
    ;; iov_base = 8
    (i32.store (i32.const 0) (i32.const 8))
    ;; iov_len = 12
    (i32.store (i32.const 4) (i32.const 12))
    ;; fd_write(fd=1, iovs=0, iovs_len=1, nwritten=20)
    (call $fd_write
      (i32.const 1)
      (i32.const 0)
      (i32.const 1)
      (i32.const 20))
    drop))
