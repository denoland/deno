The test suite for this module spawns rustc processes to compile various example
Rust programs. You must have wasm targets enabled:

```
rustup target add wasm32-wasi
rustup target add wasm32-unknown-unknown
```
