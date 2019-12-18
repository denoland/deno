// big.wasm is a WASM file > 1MB.
await WebAssembly.compile(Deno.readFileSync("./big.wasm"));

throw new Error("e");
// Deno should exit quickly instead of hang forever/too long after error
