// Importing a Wasm module from an npm package exercises the npm load path in
// the embedded module loader. That path used to request a V8 code cache for
// every module type, including Wasm, which never produces one. The unmatched
// request kept the first-run code cache strategy's pending counter above zero
// so the cache was never serialized.
// Regression test for https://github.com/denoland/deno/issues/31766.
import { add, subtract } from "npm:@denotest/wasm-esm";

console.log(add(1, 2));
console.log(subtract(8, 2));
