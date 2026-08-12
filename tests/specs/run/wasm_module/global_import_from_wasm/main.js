// A Wasm module importing a global from another Wasm module links against
// the original `WebAssembly.Global` object, so mutable globals stay direct
// references between the two instances. The JS binding for the same export
// is an unwrapped snapshot taken at instantiation.
import { bump, counter } from "./dep.wasm";
import { read } from "./main.wasm";

console.log("js snapshot", counter, typeof counter);
console.log("wasm read", read());
bump();
console.log("wasm read after bump", read());
console.log("js snapshot after bump", counter);
