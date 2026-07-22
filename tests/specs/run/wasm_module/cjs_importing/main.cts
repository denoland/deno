import WasmModule = require("./math.wasm");

console.log(WasmModule.add(1, 2));
console.log(WasmModule.subtract(9, 3));
