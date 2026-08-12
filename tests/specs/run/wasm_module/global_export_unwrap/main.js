// Per the Wasm ESM integration, global exports are unwrapped to their
// underlying value instead of being exposed as `WebAssembly.Global` objects.
import { f32val, f64val, i32val, i64val, mutcount } from "./globals.wasm";

console.log("i32val", i32val, typeof i32val);
console.log("i64val", i64val, typeof i64val);
console.log("f32val", f32val, typeof f32val);
console.log("f64val", f64val, typeof f64val);
console.log("mutcount", mutcount, typeof mutcount);
