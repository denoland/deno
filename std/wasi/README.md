# wasi

This module provides an implementation of the WebAssembly System Interface

## Supported Syscalls

## Usage

```typescript
import WASI from "https://deno.land/std/wasi/snapshot_preview1.ts";

const wasi = new WASI({
  args: Deno.args,
  env: Deno.env,
});

const binary = Deno.readAll("path/to/your/module.wasm");
const module = await WebAssembly.compile(binary);
const instance = await WebAssembly.instantiate(module, {
  wasi_snapshot_preview1: wasi.exports,
});

wasi.memory = module.exports.memory;

if (module.exports._start) {
  instance.exports._start();
} else if (module.exports._initialize) {
  instance.exports._initialize();
} else {
  throw new Error("No entry point found");
}
```

## Testing

The test suite for this module spawns rustc processes to compile various example
Rust programs. You must have wasm targets enabled:

```
rustup target add wasm32-wasi
rustup target add wasm32-unknown-unknown
```
