// Regression test for https://github.com/denoland/deno/issues/34677
//
// `node:vm` creating a new context caused v8 to reset the isolate-wide
// WebAssembly streaming callback, which broke `WebAssembly.compileStreaming`
// and `WebAssembly.instantiateStreaming` (they started rejecting `Response`
// arguments with "Argument 0 must be a buffer source").
import { createContext } from "node:vm";

// A valid empty WebAssembly module (`\0asm` + version 1).
const wasmUrl = "data:application/wasm;base64,AGFzbQEAAAA=";

async function streamingWorks() {
  const mod = await WebAssembly.compileStreaming(await fetch(wasmUrl));
  const { instance } = await WebAssembly.instantiateStreaming(
    await fetch(wasmUrl),
  );
  return mod instanceof WebAssembly.Module &&
    instance instanceof WebAssembly.Instance;
}

console.log("before createContext:", await streamingWorks());

createContext();

console.log("after createContext:", await streamingWorks());
