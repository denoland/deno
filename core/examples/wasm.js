// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// asc wasm.ts --exportStart --initialMemory 6400 -O -o wasm.wasm
// deno-fmt-ignore
const bytes = new Uint8Array([
    0,  97, 115, 109,   1,   0,   0,   0,   1,  4,   1, 96,   0,   0,   2,
   15,   1,   3, 111, 112, 115,   7, 111, 112, 95, 119, 97, 115, 109,   0,
    0,   3,   3,   2,   0,   0,   5,   4,   1,  0, 128, 50,   7,  36,   4,
    7, 111, 112,  95, 119,  97, 115, 109,   0,  0,   4, 99,  97, 108, 108,
    0,   1,   6, 109, 101, 109, 111, 114, 121,  2,   0,  6,  95, 115, 116,
   97, 114, 116,   0,   2,  10,  10,   2,   4,  0,  16,  0,  11,   3,   0,
    1,  11
 ]);

const { ops } = Deno.core;

const module = new WebAssembly.Module(bytes);
const instance = new WebAssembly.Instance(module, { ops });
ops.op_set_wasm_mem(instance.exports.memory);

instance.exports.call();

const memory = instance.exports.memory;
const view = new Uint8Array(memory.buffer);

if (view[0] !== 69) {
  throw new Error("Expected first byte to be 69");
}
