// Reference-type globals (funcref/externref) unwrap via `.value`, like numeric
// globals, so a null funcref global export is exposed to JS as `null` rather
// than as a `WebAssembly.Global` object.
import { funcval } from "./funcref.wasm";

console.log("funcval", funcval, typeof funcval);
console.log("is global", funcval instanceof WebAssembly.Global);
