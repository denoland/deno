// WebAssembly module containing a single function with an unreachable instruction
const binary = await Deno.readFile("./unreachable.wasm");

const module = new WebAssembly.Module(binary);
const instance = new WebAssembly.Instance(module);

// Compare the stack trace with wasm_url.js, which compiles the WASM module with
// streaming APIs.
instance.exports.unreachable();
