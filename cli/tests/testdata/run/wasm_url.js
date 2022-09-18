const module = await WebAssembly.compileStreaming(
  fetch("http://localhost:4545/unreachable.wasm"),
);
const instance = new WebAssembly.Instance(module);

// Compare the stack trace with wasm_unreachable.js, which compiles the WASM
// module with synchronous APIs.
instance.exports.unreachable();
