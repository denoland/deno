// Re-exports from a Wasm module so that importing this package forces the
// loader to handle a Wasm module type from inside an npm package.
export { add, subtract } from "./math.wasm";
