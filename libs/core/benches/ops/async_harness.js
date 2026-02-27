// Copyright 2018-2025 the Deno authors. MIT license.
// deno-lint-ignore-file no-unused-vars, prefer-const, require-await

// This harness is dynamically generated for each individual bench run.
async function run() {
  const LARGE_STRING_1000000 = "*".repeat(1000000);
  const LARGE_STRING_1000 = "*".repeat(1000);
  const LARGE_STRING_UTF8_1000000 = "\u1000".repeat(1000000);
  const LARGE_STRING_UTF8_1000 = "\u1000".repeat(1000);
  const BUFFER = new Uint8Array(1024);
  const ARRAYBUFFER = new ArrayBuffer(1024);
  const { __OP__: op } = Deno.core.ops;
  const { op_make_external } = Deno.core.ops;
  const EXTERNAL = op_make_external();

  // TODO(mmastrac): Because of current v8 limitations, these ops are not always fast unless we do this.
  // The reason is not entirely clear.
  function __OP__(__ARGS__) {
    return op(__ARGS__);
  }

  let accum = 0;
  let __index__ = 0;
  __PERCENT__PrepareFunctionForOptimization(__OP__);
  __CALL__;
  __PERCENT__OptimizeFunctionOnNextCall(__OP__);
  __CALL__;

  async function bench() {
    let accum = 0;
    for (let __index__ = 0; __index__ < __COUNT__; __index__++) __CALL__;
    return accum;
  }

  __PERCENT__PrepareFunctionForOptimization(bench);
  await bench();
  __PERCENT__OptimizeFunctionOnNextCall(bench);
  await bench();

  return bench;
}
