// Copyright 2018-2025 the Deno authors. MIT license.

const { ops } = Deno.core;

// The Wasm module is generated using assemblyscript@0.24.1
// asc core/examples/wasm.ts --exportStart --initialMemory 6400 -O -o core/examples/wasm.wasm

const bytes = ops.op_get_wasm_module();

const module = new WebAssembly.Module(bytes);
const instance = new WebAssembly.Instance(module, { wasm: ops });
ops.op_set_wasm_mem(instance.exports.memory);

instance.exports.call();

const memory = instance.exports.memory;
const view = new Uint8Array(memory.buffer);

if (view[0] !== 69) {
  throw new Error("Expected first byte to be 69");
}

instance.exports.call_mem(instance.exports.memory);

if (view[0] !== 68) {
  throw new Error("Expected first byte to be 68");
}
