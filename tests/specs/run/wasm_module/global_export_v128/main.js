// Reading `.value` throws for a `v128` global, so the unwrap helper falls back
// to exposing the original `WebAssembly.Global` object. (Node leaves the export
// as `undefined`; Deno keeps the Global, which never silently yields
// `undefined` and carries strictly more information.)
import { v } from "./v128.wasm";

console.log("is global", v instanceof WebAssembly.Global);
let threw = false;
try {
  v.value;
} catch {
  threw = true;
}
console.log("value throws", threw);
