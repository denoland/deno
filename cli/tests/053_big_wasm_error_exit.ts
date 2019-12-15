// big.wasm is a WASM file > 1MB.
const compiled = await WebAssembly.compile(Deno.readFileSync("./big.wasm"));
const instance = new WebAssembly.Instance(compiled, {});
console.log(instance.exports);

throw new Error("e");
// Deno should exit instead of hang forever after error
