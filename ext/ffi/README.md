# deno_ffi

This crate implements dynamic library ffi.

## Performance

Deno FFI calls have extremely low overhead (~1ns on M1 16GB RAM) and perform on
par with native code. Deno leverages V8 fast api calls and JIT compiled bindings
to achieve these high speeds.

`Deno.dlopen` generates an optimized and a fallback path. Optimized paths are
triggered when V8 decides to optimize the function, hence call through the Fast
API. Fallback paths handle types like function callbacks and implement proper
error handling for unexpected types, that is not supported in Fast calls.

Optimized calls enter a JIT compiled function "trampoline" that translates Fast
API values directly for symbol calls. JIT compilation itself is super fast,
thanks to `tinycc`. Currently, the optimized path is only supported on Linux and
MacOS.

To run benchmarks:

```bash
target/release/deno bench --allow-ffi --allow-read --unstable ./test_ffi/tests/bench.js
```
