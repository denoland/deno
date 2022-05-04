const resp = fetch(new URL("./test.wasm", import.meta.url));
const module = await WebAssembly.compileStreaming(resp);
const instance = new WebAssembly.Instance(module, {});
console.log(instance.exports.add()); // expect 42

