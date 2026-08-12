// Reference-type globals (externref/funcref) unwrap via `.value`, like numeric
// globals, so a null externref global export is exposed to JS as `null` rather
// than as a `WebAssembly.Global` object.
import { nullref } from "./externref.wasm";

console.log("nullref", nullref, typeof nullref);
console.log("is global", nullref instanceof WebAssembly.Global);
