// This wasm file imports `./math.ts`, which doesn't exist but shouldn't be
// graph-traversed.
import source mathSource from "./math_with_import.wasm";
console.log(mathSource);
